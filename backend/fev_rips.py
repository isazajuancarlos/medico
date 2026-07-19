"""FEV-RIPS: el JSON único que se envía al Mecanismo Único de Validación.

Sustituye el paso manual de convertir un XLSX en el conversor web de MinSalud
(https://fevrips.sispro.gov.co/SPConvertidorJSON).

=== QUÉ APORTA ESTE MÓDULO SOBRE `rips_completo.py` ===

`rips_completo.py` ya construye la forma anidada correcta —`usuarios[]` con su
`servicios{}`—, así que NO se reescribe: se reutiliza entera su extracción. Lo
que le falta para ser FEV-RIPS válido es otra cosa:

1. **Devuelve SEIS documentos separados** (AC, AP, AM, AU, AH, CT), cada uno con
   su propia lista de usuarios. FEV-RIPS es UN SOLO documento donde cada usuario
   lleva TODOS sus servicios juntos. Un paciente con consulta y medicamento
   aparece hoy en dos documentos y debe aparecer una vez con las dos cosas.
2. **El CT no existe en FEV-RIPS.** Es un archivo de control del formato plano
   de la Resolución 3374. Aquí se descarta.
3. **El sobre está vacío.** Falta decir qué tipo de documento es.
4. **Codificación de campos**: el esquema usa `null` donde hoy va `""`, y las
   fechas van sin segundos.

=== ADAPTABLE AL TIPO DE PRESTADOR ===

El CUERPO del JSON es idéntico para todos. Lo único que cambia es el SOBRE:

    tipo de documento     numFactura   tipoNota   xmlFevFile        endpoint
    RIPS sin factura      null         "RS"       vacío            CargarRipsSinFactura
    FEV en salud          <número>     null       AttachedDocument CargarFevRips

Un consultorio pequeño (profesional independiente que factura menos de 3.500
UVT) va por RIPS sin factura y no necesita factura electrónica. Una IPS, clínica
u hospital factura electrónicamente y va por FEV, con el XML aprobado por la DIAN
en Base64. Por eso el sobre es una capa aparte: añadir un tipo es una entrada
más, no un módulo nuevo.

Referencia: MANUAL DE CONSUMO API-DOCKER-FEV-RIPS V 4.3 (FEVRM001 v2.0),
Resolución 2275 de 2023, Documento Técnico 1.

Referencias verificadas:
- MANUAL DE CONSUMO API-DOCKER-FEV-RIPS V 4.3 (FEVRM001 v2.0, sep-2024): endpoints,
  autenticación y forma del JSON.
- MANUAL DE USUARIO CONVERTIDOR A JSON: es el más reciente y declara expresamente
  que la plantilla «está estructurada de acuerdo con la Resolución 948 de 2026 y
  su Documento Técnico 1». De ahí salen las nueve pestañas —transacción, usuarios
  y los siete tipos de atención— y las reglas de numeración de consecutivos.

ADVERTENCIA: el manual de la API es anterior a la Resolución 948. Los NOMBRES de
campo de cada atención deben contrastarse contra el Documento Técnico 1 vigente
antes de usar esto en producción; lo que sí está confirmado post-948 es la
estructura general y la numeración.
"""

from __future__ import annotations

from datetime import date
from typing import Any, Dict, List, Optional

from rips_completo import (
    generar_archivo_ac,
    generar_archivo_ah,
    generar_archivo_am,
    generar_archivo_ap,
    generar_archivo_au,
)

# Los siete tipos de atención del esquema. Se listan todos aunque un consultorio
# solo llene uno o dos: el serializador es el mismo para un hospital.
TIPOS_SERVICIO = (
    "consultas",
    "procedimientos",
    "medicamentos",
    "urgencias",
    "hospitalizacion",
    "otrosServicios",
    "recienNacidos",
)

#: Campos que el esquema espera como `null` y que la extracción deja en cadena
#: vacía. Enviar "" donde va null es causal de rechazo en el validador.
_CAMPOS_NULABLES = (
    "numAutorizacion",
    "codDiagnosticoRelacionado1",
    "codDiagnosticoRelacionado2",
    "codDiagnosticoRelacionado3",
    "numFEVPagoModerador",
)

TIPO_RIPS_SIN_FACTURA = "RS"


# ==============================================================================
# NORMALIZACIÓN
# ==============================================================================

def _vaciar_a_null(servicio: Dict[str, Any]) -> Dict[str, Any]:
    """Cadena vacía -> None en los campos que el esquema declara nulables."""
    limpio = dict(servicio)
    for campo in _CAMPOS_NULABLES:
        if campo in limpio and limpio[campo] == "":
            limpio[campo] = None
    return limpio


def _sin_segundos(servicio: Dict[str, Any]) -> Dict[str, Any]:
    """`fechaInicioAtencion` va como 'YYYY-MM-DD HH:MM', sin segundos.

    La extracción produce '%Y-%m-%d %H:%M:%S'; el manual muestra
    "2025-06-26 11:28". Se recorta en vez de tocar `rips_completo`, que sigue
    sirviendo al formato anterior mientras haga falta.
    """
    limpio = dict(servicio)
    f = limpio.get("fechaInicioAtencion")
    if isinstance(f, str) and len(f) == 19 and f[10] == " ":
        limpio["fechaInicioAtencion"] = f[:16]
    return limpio


def _normalizar(servicio: Dict[str, Any]) -> Dict[str, Any]:
    return _sin_segundos(_vaciar_a_null(servicio))


def _clave_usuario(usuario: Dict[str, Any]) -> tuple:
    """Identidad de un usuario entre documentos distintos.

    NO se usa el `consecutivo`: cada documento numera el suyo desde 1, así que
    el consecutivo 1 del AC y el 1 del AM son personas distintas. Fusionar por
    consecutivo mezclaría los servicios de dos pacientes — y en un sistema
    clínico eso es un dato falso, no una imprecisión.
    """
    return (
        usuario.get("tipoDocumentoIdentificacion"),
        usuario.get("numDocumentoIdentificacion"),
    )


# ==============================================================================
# CONSOLIDACIÓN
# ==============================================================================

def consolidar(documentos: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Fusiona varios documentos en una sola lista de usuarios.

    Cada usuario queda una vez, con todos sus servicios agrupados, y se renumera
    el `consecutivo` de forma global. Los consecutivos de cada servicio se
    renumeran dentro de su propio tipo, que es como los referencia el validador
    cuando devuelve un error (`usuarios[0].servicios.consultas[2]`).
    """
    fusionados: Dict[tuple, Dict[str, Any]] = {}

    for documento in documentos:
        for usuario in documento.get("usuarios", []):
            clave = _clave_usuario(usuario)
            if clave not in fusionados:
                base = {k: v for k, v in usuario.items() if k != "servicios"}
                base["servicios"] = {}
                fusionados[clave] = base
            else:
                # Un paciente incapacitado en cualquiera de sus atenciones lo
                # está en el documento: el "SI" gana sobre el "NO".
                if usuario.get("incapacidad") == "SI":
                    fusionados[clave]["incapacidad"] = "SI"

            destino = fusionados[clave]["servicios"]
            for tipo in TIPOS_SERVICIO:
                entradas = usuario.get("servicios", {}).get(tipo)
                if entradas:
                    destino.setdefault(tipo, []).extend(entradas)

    usuarios: List[Dict[str, Any]] = []
    for consecutivo, usuario in enumerate(fusionados.values(), start=1):
        usuario["consecutivo"] = consecutivo
        servicios = {}
        for tipo, entradas in usuario["servicios"].items():
            servicios[tipo] = [
                {**_normalizar(e), "consecutivo": i}
                for i, e in enumerate(entradas, start=1)
            ]
        usuario["servicios"] = servicios
        usuarios.append(usuario)

    return usuarios


# ==============================================================================
# SOBRES
# ==============================================================================

def sobre_rips_sin_factura(
    num_documento_obligado: str,
    usuarios: List[Dict[str, Any]],
    num_nota: str,
) -> Dict[str, Any]:
    """Sobre para profesional independiente / entidad de objeto social diferente.

    `xmlFevFile` va VACÍO: esta ruta no exige factura electrónica de la DIAN.
    Aplica a quien factura menos de 3.500 UVT.

    `numNota` es OBLIGATORIO y no se puede repetir entre envíos: el manual del
    convertidor advierte que repetirlo «generará rechazo». Se valida aquí y no
    más adelante porque un rechazo del MUV cuesta un ciclo completo de reenvío,
    y porque el llamante todavía puede corregirlo.
    """
    if not num_nota or not str(num_nota).strip():
        raise ValueError(
            "numNota es obligatorio en un RIPS sin factura, debe ser consecutivo "
            "y no repetirse entre envíos: repetirlo produce rechazo en el MUV"
        )
    return {
        "rips": {
            "numDocumentoIdObligado": num_documento_obligado,
            "numFactura": None,
            "tipoNota": TIPO_RIPS_SIN_FACTURA,
            "numNota": num_nota,
            "usuarios": usuarios,
        },
        "xmlFevFile": "",
    }


def sobre_fev(
    num_documento_obligado: str,
    usuarios: List[Dict[str, Any]],
    num_factura: str,
    xml_fev_base64: str,
) -> Dict[str, Any]:
    """Sobre para prestador que factura electrónicamente: IPS, clínica, hospital.

    `xmlFevFile` es el AttachedDocument de la factura electrónica en Base64, y
    debe estar YA APROBADO POR LA DIAN: el MUV valida después de esa aprobación.
    """
    if not xml_fev_base64:
        raise ValueError(
            "una FEV en salud exige el AttachedDocument de la factura electrónica "
            "en Base64; si el prestador no factura electrónicamente, la ruta "
            "correcta es sobre_rips_sin_factura()"
        )
    return {
        "rips": {
            "numDocumentoIdObligado": num_documento_obligado,
            "numFactura": num_factura,
            "tipoNota": None,
            "numNota": None,
            "usuarios": usuarios,
        },
        "xmlFevFile": xml_fev_base64,
    }


# ==============================================================================
# ENTRADA PRINCIPAL
# ==============================================================================

def generar_paquete(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: Optional[int] = None,
    *,
    num_nota: str = "",
    num_factura: Optional[str] = None,  # noqa: ARG001 — documentado abajo
    xml_fev_base64: Optional[str] = None,
) -> Dict[str, Any]:
    """Construye el paquete FEV-RIPS listo para enviar al MUV.

    Sin `num_factura` produce RIPS sin factura; con él, una FEV en salud, que
    además exige el XML. El tipo de prestador decide cuál, no este módulo.
    """
    documentos = [
        generar_archivo_ac(fecha_inicio, fecha_fin, institucion_id),
        generar_archivo_ap(fecha_inicio, fecha_fin, institucion_id),
        generar_archivo_am(fecha_inicio, fecha_fin, institucion_id),
        generar_archivo_au(fecha_inicio, fecha_fin, institucion_id),
        generar_archivo_ah(fecha_inicio, fecha_fin, institucion_id),
    ]

    obligado = next(
        (d["numDocumentoIdObligado"] for d in documentos if d.get("numDocumentoIdObligado")),
        "",
    )
    usuarios = consolidar(documentos)

    if num_factura:
        return sobre_fev(obligado, usuarios, num_factura, xml_fev_base64 or "")
    return sobre_rips_sin_factura(obligado, usuarios, num_nota)


def contar_atenciones(paquete: Dict[str, Any]) -> Dict[str, int]:
    """Atenciones por tipo. Sirve para cuadrar contra lo que devuelve ConsultarCUV."""
    conteo = {tipo: 0 for tipo in TIPOS_SERVICIO}
    for usuario in paquete["rips"]["usuarios"]:
        for tipo, entradas in usuario.get("servicios", {}).items():
            conteo[tipo] = conteo.get(tipo, 0) + len(entradas)
    return conteo
