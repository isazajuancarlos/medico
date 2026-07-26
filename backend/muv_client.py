"""Cliente del Mecanismo Único de Validación (MUV) de MinSalud.

Habla con el contenedor Docker que el PRESTADOR corre en su propia máquina
(`https://localhost:9443` por defecto). Reemplaza los tres pasos manuales de hoy:
subir el archivo a la aplicación de escritorio, esperar la validación y copiar el
CUV a mano.

    login  ->  cargar  ->  CUV

=== LAS CREDENCIALES NO SON NUESTRAS ===

MinSalud confirmó (caso 2026403888) que el acceso al MUV está atado a una PERSONA
registrada en Mi Seguridad Social y SISPRO y VINCULADA a la entidad: no existe
credencial de integrador. Las credenciales son del prestador, viven en su
despliegue y NUNCA deben aparecer en el código, en un log ni en una excepción.
Por eso se reciben en un objeto aparte, con `__repr__` censurado.

=== EL TRANSPORTE VA INYECTADO ===

Igual que el endurecedor de tumbaga: el cliente no importa `requests` ni abre
sockets por su cuenta. Recibe un transporte, de modo que las pruebas corren sin
red y sin contenedor.

=== LO QUE APORTA SOBRE UNA LLAMADA HTTP PELADA ===

Separar NOTIFICACIONES de RECHAZOS. El validador devuelve ambos en la misma
lista `ResultadosValidacion`, y desde el 1-jun-2026 (validador 1.4.8.0) trece
reglas que antes solo notificaban pasan a ser causal de rechazo. Confundirlas
significa creer que un archivo pasó cuando no pasó, o alarmarse por avisos
inocuos. Cada hallazgo trae además `PathFuente` —la ruta exacta dentro del JSON,
p. ej. `usuarios[1].servicios.consultas[0].codConsulta`—, que es lo que permite
señalarle al usuario el campo que debe corregir en vez de mostrarle un volcado.

Referencia: MANUAL DE CONSUMO API-DOCKER-FEV-RIPS V 4.3 (FEVRM001 v2.0).
"""

from __future__ import annotations

import gzip
import json
import ssl
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Protocol

RUTA_LOGIN = "/api/Auth/LoginSISPRO"
RUTA_CARGAR_SIN_FACTURA = "/api/PaquetesFevRips/CargarRipsSinFactura"
RUTA_CARGAR_FEV = "/api/PaquetesFevRips/CargarFevRips"
RUTA_CONSULTAR_CUV = "/api/ConsultasFevRips/ConsultarCUV"

#: `Clase` de un hallazgo que NO impide obtener el CUV.
CLASE_NOTIFICACION = "NOTIFICACION"


# ==============================================================================
# ERRORES
# ==============================================================================

class MuvError(Exception):
    """Base de los fallos del MUV."""


class MuvNoDisponible(MuvError):
    """El contenedor no respondió, o respondió algo ininteligible.

    Es RECUPERABLE: reintentar más tarde puede funcionar. No significa que el
    archivo esté mal.
    """


class MuvCredencialesInvalidas(MuvError):
    """SISPRO rechazó las credenciales del prestador.

    NO es un problema del archivo ni de la red: alguien debe revisar el registro
    en Mi Seguridad Social o la vinculación a la entidad.
    """


class MuvRechazado(MuvError):
    """El archivo no pasó la validación: no hay CUV.

    Trae los hallazgos con su `PathFuente` para poder señalar qué corregir.
    """

    def __init__(self, mensaje: str, hallazgos: List["Hallazgo"] | None = None):
        super().__init__(mensaje)
        self.hallazgos = hallazgos or []


# ==============================================================================
# TIPOS
# ==============================================================================

class Transporte(Protocol):
    """Un POST que devuelve (código_http, cuerpo_texto)."""

    def post(
        self,
        url: str,
        cuerpo: bytes,
        cabeceras: Dict[str, str],
        timeout: float,
    ) -> tuple[int, str]: ...


@dataclass(frozen=True)
class Credenciales:
    """Credenciales SISPRO DEL PRESTADOR. No se registran nunca."""

    tipo_documento: str
    numero_documento: str
    clave: str
    nit: str
    #: Solo para profesionales independientes; una IPS no lo informa. Los cuatro
    #: valores de la sección 8.1 «LoginSispro» del manual, y lo que los separa:
    #:
    #:   RE    no es facturador electrónico (sea cual sea su NIT)
    #:   PIN   factura electrónicamente y el NIT COINCIDE con el documento
    #:   PINx  factura electrónicamente y el NIT de la DIAN NO coincide
    #:   PIE   documento distinto de cédula de ciudadanía (extranjero)
    #:
    #: El discriminante primero es si factura electrónicamente, y solo después
    #: la igualdad NIT/documento: quien lea «Nacional» vs «de Excepción» sin la
    #: tabla delante elige por el adjetivo y se equivoca.
    #:
    #: No confundirlo con el `tipoUsuario` del RIPS («01».«12», tipo de
    #: afiliado): mismo nombre, documento distinto, valores sin relación.
    tipo_usuario: Optional[str] = None

    def __repr__(self) -> str:  # pragma: no cover - trivial
        return (
            f"Credenciales(tipo_documento={self.tipo_documento!r}, "
            f"numero_documento='***', clave='***', nit='***')"
        )

    def cuerpo_login(self) -> Dict[str, Any]:
        cuerpo: Dict[str, Any] = {
            "persona": {
                "identificacion": {
                    "tipo": self.tipo_documento,
                    "numero": self.numero_documento,
                }
            },
            "clave": self.clave,
            "nit": self.nit,
        }
        if self.tipo_usuario:
            cuerpo["tipoUsuario"] = self.tipo_usuario
        return cuerpo


@dataclass(frozen=True)
class Hallazgo:
    """Un resultado de validación, con la ruta exacta del campo señalado."""

    clase: str
    codigo: str
    descripcion: str
    observaciones: str = ""
    path_fuente: str = ""
    fuente: str = ""

    @property
    def es_notificacion(self) -> bool:
        return self.clase.upper() == CLASE_NOTIFICACION

    @property
    def impide_el_cuv(self) -> bool:
        """Todo lo que no sea notificación bloquea.

        Se decide por exclusión a propósito: si mañana aparece una clase nueva,
        conviene tratarla como bloqueante y equivocarse hacia el lado seguro,
        antes que darla por inocua y creer que un archivo pasó cuando no.
        """
        return not self.es_notificacion

    @classmethod
    def desde_json(cls, d: Dict[str, Any]) -> "Hallazgo":
        return cls(
            clase=str(d.get("Clase", "")),
            codigo=str(d.get("Codigo", "")),
            descripcion=str(d.get("Descripcion", "")),
            observaciones=str(d.get("Observaciones") or ""),
            path_fuente=str(d.get("PathFuente") or ""),
            fuente=str(d.get("Fuente") or ""),
        )


@dataclass(frozen=True)
class ResultadoCarga:
    """Lo que devuelve el MUV tras procesar un paquete."""

    aceptado: bool
    cuv: Optional[str]
    proceso_id: Optional[int]
    fecha_radicacion: Optional[str]
    ambiente: Optional[str]
    hallazgos: List[Hallazgo] = field(default_factory=list)
    crudo: Dict[str, Any] = field(default_factory=dict)

    @property
    def notificaciones(self) -> List[Hallazgo]:
        """Avisos que NO impidieron el CUV. Conviene revisarlos igual: desde el
        1-jun-2026 varias reglas de este grupo pasaron a causal de rechazo."""
        return [h for h in self.hallazgos if h.es_notificacion]

    @property
    def bloqueantes(self) -> List[Hallazgo]:
        return [h for h in self.hallazgos if h.impide_el_cuv]

    def campos_a_corregir(self) -> Dict[str, List[Hallazgo]]:
        """Hallazgos agrupados por la ruta del campo que los produjo.

        Es lo que permite marcar el campo en pantalla en vez de mostrar una
        lista plana de errores.
        """
        por_campo: Dict[str, List[Hallazgo]] = {}
        for h in self.hallazgos:
            if h.path_fuente:
                por_campo.setdefault(h.path_fuente, []).append(h)
        return por_campo


# ==============================================================================
# CLIENTE
# ==============================================================================

class MuvClient:
    """Cliente del contenedor MUV del prestador.

    Args:
        base_url:   dónde escucha el contenedor. El puerto lo asigna el servidor
                    dockerizado; el manual usa 9443.
        transporte: quien hace el POST. Inyectado para poder probar sin red.
        timeout:    segundos por petición. Validar un paquete grande tarda.
    """

    def __init__(
        self,
        base_url: str,
        transporte: Transporte,
        timeout: float = 120.0,
    ):
        if not base_url:
            raise ValueError("base_url es obligatorio")
        if transporte is None:
            raise ValueError("MuvClient necesita un transporte")
        self.base_url = base_url.rstrip("/")
        self._transporte = transporte
        self.timeout = timeout
        self._token: Optional[str] = None

    # --- Interno --------------------------------------------------------

    def _post(
        self,
        ruta: str,
        cuerpo: Dict[str, Any],
        *,
        con_token: bool = False,
        comprimir: bool = False,
    ) -> Dict[str, Any]:
        datos = json.dumps(cuerpo, ensure_ascii=False).encode("utf-8")
        cabeceras = {"Content-Type": "application/json"}

        if comprimir:
            # Obligatorio en los métodos de carga, según el manual.
            datos = gzip.compress(datos)
            cabeceras["Content-Encoding"] = "gzip"

        if con_token:
            if not self._token:
                raise MuvError("hay que autenticarse antes de cargar")
            cabeceras["Authorization"] = f"Bearer {self._token}"

        try:
            codigo, texto = self._transporte.post(
                f"{self.base_url}{ruta}", datos, cabeceras, self.timeout
            )
        except Exception as e:  # noqa: BLE001 — red, DNS, TLS, timeout
            raise MuvNoDisponible(f"no se pudo contactar el MUV: {e}") from e

        if codigo in (401, 403):
            raise MuvCredencialesInvalidas(
                f"SISPRO rechazó las credenciales del prestador (HTTP {codigo}). "
                f"Revisar el registro en Mi Seguridad Social y la vinculación a "
                f"la entidad; no es un problema del archivo."
            )
        try:
            cuerpo_json = json.loads(texto)
        except ValueError:
            cuerpo_json = None

        # Un RECHAZO viaja con HTTP 400 y el resultado completo dentro.
        # Observado el 2026-07-26 contra el contenedor de stage: 400 con
        # {"ResultState":false, "Ambiente":"DockerStage", "ResultadosValidacion":
        # [...]}. Tratarlo como avería tiraba justo los hallazgos que se fueron
        # a buscar, y le decía al usuario «el contenedor no respondió» cuando
        # había respondido perfectamente para decir que el archivo está mal.
        #
        # Lo que separa un resultado de una avería no es el código HTTP sino si
        # el cuerpo ES un resultado. Un 400 con HTML de un proxy sigue siendo
        # una avería, y por eso se exige la forma, no solo que haya JSON.
        if isinstance(cuerpo_json, dict) and any(
            c in cuerpo_json
            for c in ("ResultState", "ResultadosValidacion", "CodigoUnicoValidacion")
        ):
            return cuerpo_json

        if codigo >= 400:
            raise MuvNoDisponible(f"el MUV respondió HTTP {codigo}: {texto[:300]}")
        if cuerpo_json is None:
            raise MuvNoDisponible(
                f"respuesta ininteligible del MUV (HTTP {codigo}): {texto[:200]}"
            )
        return cuerpo_json

    # --- Operaciones ----------------------------------------------------

    def login(self, credenciales: Credenciales) -> None:
        """Autentica y guarda el token. No devuelve nada: el token no sale."""
        respuesta = self._post(RUTA_LOGIN, credenciales.cuerpo_login())

        token = None
        for clave in ("token", "Token", "access_token", "accessToken"):
            if respuesta.get(clave):
                token = respuesta[clave]
                break
        if not token:
            raise MuvNoDisponible(
                "el MUV no devolvió token en la respuesta de autenticación"
            )
        self._token = str(token)

    def cargar(self, paquete: Dict[str, Any], *, ruta: str = RUTA_CARGAR_SIN_FACTURA) -> ResultadoCarga:
        """Envía un paquete y devuelve el resultado de la validación.

        NO levanta cuando el archivo es rechazado: devuelve un `ResultadoCarga`
        con `aceptado=False` y los hallazgos. Un rechazo es un resultado normal
        del proceso —el usuario debe corregir y reenviar—, no una avería.
        """
        respuesta = self._post(ruta, paquete, con_token=True, comprimir=True)
        return self._interpretar(respuesta)

    def consultar_cuv(self, consulta: Dict[str, Any]) -> Dict[str, Any]:
        """Consulta el estado de un documento ya enviado. No requiere token."""
        return self._post(RUTA_CONSULTAR_CUV, consulta)

    # --- Interpretación -------------------------------------------------

    @staticmethod
    def _interpretar(respuesta: Dict[str, Any]) -> ResultadoCarga:
        hallazgos = [
            Hallazgo.desde_json(h)
            for h in (respuesta.get("ResultadosValidacion") or [])
        ]
        cuv = respuesta.get("CodigoUnicoValidacion") or None

        # El CUV manda sobre la bandera: si hay código, el archivo pasó. Se exige
        # ADEMÁS que no haya bloqueantes, para no dar por bueno un resultado
        # contradictorio.
        estado = respuesta.get("ResultState")
        if estado is None:
            estado = respuesta.get("EsValido")
        bloqueantes = [h for h in hallazgos if h.impide_el_cuv]
        aceptado = bool(cuv) and bool(estado) and not bloqueantes

        return ResultadoCarga(
            aceptado=aceptado,
            cuv=cuv if aceptado else None,
            proceso_id=respuesta.get("ProcesoId"),
            fecha_radicacion=respuesta.get("FechaRadicacion"),
            ambiente=respuesta.get("Ambiente"),
            hallazgos=hallazgos,
            crudo=respuesta,
        )


# ==============================================================================
# TRANSPORTE REAL
# ==============================================================================

class TransporteUrllib:
    """Transporte sobre la biblioteca estándar. Sin dependencias nuevas.

    Sobre `verificar_tls`: el contenedor se sirve por HTTPS con un certificado
    autofirmado, así que la verificación normal falla contra `localhost`. Hay dos
    salidas honestas y el despliegue elige:

    - Dejar la verificación activa y darle el certificado del contenedor por
      `cafile`. Es lo correcto.
    - Desactivarla, que solo es defendible porque la conexión NO SALE DE LA
      MÁQUINA: no hay red por donde interceptarla. Si algún día el contenedor se
      mueve a otro equipo, deja de serlo.

    Se exige decirlo explícitamente: el valor por defecto verifica.
    """

    def __init__(self, verificar_tls: bool = True, cafile: Optional[str] = None):
        if verificar_tls:
            self._contexto = ssl.create_default_context(cafile=cafile)
        else:
            self._contexto = ssl._create_unverified_context()  # noqa: S323

    def post(
        self,
        url: str,
        cuerpo: bytes,
        cabeceras: Dict[str, str],
        timeout: float,
    ) -> tuple[int, str]:
        peticion = urllib.request.Request(
            url, data=cuerpo, method="POST", headers=cabeceras
        )
        try:
            with urllib.request.urlopen(
                peticion, timeout=timeout, context=self._contexto
            ) as r:
                return r.status, r.read().decode("utf-8", errors="replace")
        except urllib.error.HTTPError as e:
            # Un 4xx trae cuerpo útil: lo devuelve para que el cliente decida si
            # es credencial inválida o avería, en vez de perderlo en la excepción.
            return e.code, e.read().decode("utf-8", errors="replace")
