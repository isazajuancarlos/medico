"""El cliente del MUV: qué distingue, qué censura y qué no confunde.

Todo corre con un transporte falso: sin red, sin contenedor y sin credenciales
reales. Las respuestas están calcadas de los ejemplos del MANUAL DE CONSUMO
API-DOCKER-FEV-RIPS V 4.3.
"""
import gzip
import json
import os
import sys

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from muv_client import (  # noqa: E402
    Credenciales, Hallazgo, MuvClient, MuvCredencialesInvalidas, MuvError,
    MuvNoDisponible, RUTA_CARGAR_SIN_FACTURA, RUTA_LOGIN,
)


class TransporteFalso:
    """Devuelve respuestas preparadas y guarda lo que se le pidió enviar."""

    def __init__(self, respuestas):
        self.respuestas = dict(respuestas)
        self.peticiones = []

    def post(self, url, cuerpo, cabeceras, timeout):
        self.peticiones.append(
            {"url": url, "cuerpo": cuerpo, "cabeceras": cabeceras, "timeout": timeout}
        )
        for ruta, (codigo, texto) in self.respuestas.items():
            if url.endswith(ruta):
                return codigo, texto
        return 404, '{"error":"ruta no preparada"}'

    def cuerpo_de(self, ruta):
        for p in self.peticiones:
            if p["url"].endswith(ruta):
                datos = p["cuerpo"]
                if p["cabeceras"].get("Content-Encoding") == "gzip":
                    datos = gzip.decompress(datos)
                return json.loads(datos.decode("utf-8"))
        raise AssertionError(f"no se llamó a {ruta}")


class TransporteCaido:
    def post(self, url, cuerpo, cabeceras, timeout):
        raise ConnectionError("el contenedor no está levantado")


CREDS = Credenciales(
    tipo_documento="CC", numero_documento="79605600",
    clave="no-es-real", nit="530520110", tipo_usuario="PIN",
)

LOGIN_OK = (200, json.dumps({"token": "un-token-de-prueba"}))

CARGA_OK = (200, json.dumps({
    "ResultState": True,
    "ProcesoId": 11,
    "CodigoUnicoValidacion": "8d3992db9c2a86110f1c048dc82c96df",
    "FechaRadicacion": "2026-07-19T21:31:18.3971023+00:00",
    "Ambiente": "DockerTest",
    "ResultadosValidacion": [
        {"Clase": "NOTIFICACION", "Codigo": "RVC019",
         "Descripcion": "El código de CUPS se puede validar con el diagnóstico principal.",
         "Observaciones": "Verificar tabla de referencia Dato (890201)",
         "PathFuente": "usuarios[1].servicios.consultas[0].codConsulta",
         "Fuente": "Rips"},
    ],
}))

CARGA_RECHAZADA = (200, json.dumps({
    "ResultState": False,
    "ProcesoId": 12,
    "CodigoUnicoValidacion": None,
    "ResultadosValidacion": [
        {"Clase": "RECHAZO", "Codigo": "RVC101",
         "Descripcion": "El tipo de documento no corresponde.",
         "Observaciones": "",
         "PathFuente": "usuarios[0].tipoDocumentoIdentificacion",
         "Fuente": "Rips"},
        {"Clase": "NOTIFICACION", "Codigo": "RVC019",
         "Descripcion": "Aviso inocuo.", "Observaciones": "",
         "PathFuente": "usuarios[0].servicios.consultas[0].codConsulta",
         "Fuente": "Rips"},
    ],
}))


def _cliente(respuestas):
    t = TransporteFalso(respuestas)
    return MuvClient("https://localhost:9443", t), t


# --- Autenticación -----------------------------------------------------------

def test_el_login_arma_el_cuerpo_que_pide_el_manual():
    c, t = _cliente({RUTA_LOGIN: LOGIN_OK})
    c.login(CREDS)

    cuerpo = t.cuerpo_de(RUTA_LOGIN)
    assert cuerpo["persona"]["identificacion"] == {"tipo": "CC", "numero": "79605600"}
    assert cuerpo["nit"] == "530520110"
    assert cuerpo["tipoUsuario"] == "PIN"


def test_una_IPS_no_informa_tipoUsuario():
    """`tipoUsuario` aplica solo a profesionales independientes."""
    creds = Credenciales("NI", "900123456", "x", "900123456")
    c, t = _cliente({RUTA_LOGIN: LOGIN_OK})
    c.login(creds)
    assert "tipoUsuario" not in t.cuerpo_de(RUTA_LOGIN)


def test_sin_login_no_se_carga():
    c, _ = _cliente({RUTA_CARGAR_SIN_FACTURA: CARGA_OK})
    with pytest.raises(MuvError, match="autenticarse"):
        c.cargar({"rips": {}, "xmlFevFile": ""})


def test_credenciales_rechazadas_no_se_confunden_con_una_caida():
    """401 no es «reintente más tarde»: alguien debe revisar el registro en
    SISPRO o la vinculación a la entidad."""
    c, _ = _cliente({RUTA_LOGIN: (401, '{"error":"unauthorized"}')})
    with pytest.raises(MuvCredencialesInvalidas):
        c.login(CREDS)


def test_si_el_contenedor_no_esta_levantado_se_dice_claro():
    c = MuvClient("https://localhost:9443", TransporteCaido())
    with pytest.raises(MuvNoDisponible, match="no se pudo contactar"):
        c.login(CREDS)


def test_una_respuesta_sin_token_no_se_da_por_buena():
    c, _ = _cliente({RUTA_LOGIN: (200, '{"mensaje":"ok"}')})
    with pytest.raises(MuvNoDisponible, match="token"):
        c.login(CREDS)


# --- Las credenciales no se filtran ------------------------------------------

def test_las_credenciales_no_aparecen_al_imprimirlas():
    """Son del prestador, no nuestras. No pueden acabar en un log ni en un
    traceback."""
    texto = repr(CREDS)
    assert "no-es-real" not in texto
    assert "530520110" not in texto
    assert "***" in texto


def test_el_token_no_se_devuelve():
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK})
    assert c.login(CREDS) is None, "login no debe devolver el token"


# --- La carga ----------------------------------------------------------------

def test_el_cuerpo_de_carga_va_comprimido():
    """`Content-Encoding: gzip` es obligatorio en los métodos de carga."""
    c, t = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: CARGA_OK})
    c.login(CREDS)
    paquete = {"rips": {"numDocumentoIdObligado": "530520110"}, "xmlFevFile": ""}
    c.cargar(paquete)

    peticion = t.peticiones[-1]
    assert peticion["cabeceras"]["Content-Encoding"] == "gzip"
    assert peticion["cabeceras"]["Authorization"].startswith("Bearer ")
    assert t.cuerpo_de(RUTA_CARGAR_SIN_FACTURA) == paquete, "y debe descomprimirse igual"


def test_una_carga_aceptada_devuelve_el_CUV():
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: CARGA_OK})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})

    assert r.aceptado
    assert r.cuv == "8d3992db9c2a86110f1c048dc82c96df"
    assert r.proceso_id == 11
    assert r.ambiente == "DockerTest"


def test_las_notificaciones_NO_impiden_el_CUV():
    """Distinción central: el validador devuelve avisos y rechazos en la misma
    lista. Tratar un aviso como rechazo bloquearía envíos buenos."""
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: CARGA_OK})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})

    assert r.aceptado
    assert len(r.notificaciones) == 1
    assert r.bloqueantes == []


def test_un_rechazo_no_devuelve_CUV_pero_si_los_hallazgos():
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: CARGA_RECHAZADA})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})

    assert not r.aceptado
    assert r.cuv is None
    assert len(r.bloqueantes) == 1
    assert r.bloqueantes[0].codigo == "RVC101"
    assert len(r.notificaciones) == 1, "los avisos se conservan aunque haya rechazo"


def test_un_rechazo_NO_levanta_excepcion():
    """Corregir y reenviar es parte del proceso normal, no una avería. Si esto
    levantara, la aplicación trataría un flujo esperado como un error."""
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: CARGA_RECHAZADA})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})  # no debe levantar
    assert r.aceptado is False


def test_una_clase_desconocida_se_trata_como_bloqueante():
    """Ante una clase nueva conviene equivocarse hacia el lado seguro, y no dar
    por bueno un archivo que quizá no pasó."""
    h = Hallazgo(clase="ALGO_NUEVO", codigo="X", descripcion="")
    assert h.impide_el_cuv
    assert not h.es_notificacion


def test_un_CUV_con_bloqueantes_no_se_da_por_aceptado():
    """Respuesta contradictoria: hay código pero también un rechazo. No se
    acepta, porque creer que pasó cuando no es el fallo caro."""
    contradictoria = (200, json.dumps({
        "ResultState": True,
        "CodigoUnicoValidacion": "abc123",
        "ResultadosValidacion": [
            {"Clase": "RECHAZO", "Codigo": "RVC101", "Descripcion": "algo"},
        ],
    }))
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: contradictoria})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})

    assert not r.aceptado
    assert r.cuv is None


# --- Corrección asistida -----------------------------------------------------

def test_los_hallazgos_se_agrupan_por_el_campo_que_los_produjo():
    """Es lo que permite marcar el campo en pantalla en vez de mostrar una lista
    plana de errores. Ahí está el diferencial frente a validar a mano."""
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: CARGA_RECHAZADA})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})

    por_campo = r.campos_a_corregir()
    assert "usuarios[0].tipoDocumentoIdentificacion" in por_campo
    assert "usuarios[0].servicios.consultas[0].codConsulta" in por_campo
    assert por_campo["usuarios[0].tipoDocumentoIdentificacion"][0].codigo == "RVC101"


def test_un_hallazgo_sin_path_no_rompe_el_agrupamiento():
    sin_path = (200, json.dumps({
        "ResultState": False,
        "ResultadosValidacion": [
            {"Clase": "RECHAZO", "Codigo": "GEN01", "Descripcion": "error general"},
        ],
    }))
    c, _ = _cliente({RUTA_LOGIN: LOGIN_OK, RUTA_CARGAR_SIN_FACTURA: sin_path})
    c.login(CREDS)
    r = c.cargar({"rips": {}, "xmlFevFile": ""})

    assert r.campos_a_corregir() == {}
    assert len(r.bloqueantes) == 1, "pero el hallazgo no se pierde"


# --- Construcción ------------------------------------------------------------

def test_no_se_construye_sin_transporte_ni_sin_url():
    with pytest.raises(ValueError):
        MuvClient("https://localhost:9443", None)
    with pytest.raises(ValueError):
        MuvClient("", TransporteFalso({}))


# --- Transporte real ---------------------------------------------------------

def test_el_transporte_verifica_TLS_por_defecto():
    """Desactivar la verificación tiene que ser una decisión explícita del
    despliegue, no lo que pasa si nadie mira."""
    from muv_client import TransporteUrllib

    seguro = TransporteUrllib()
    assert seguro._contexto.verify_mode == __import__("ssl").CERT_REQUIRED
    assert seguro._contexto.check_hostname

    inseguro = TransporteUrllib(verificar_tls=False)
    assert inseguro._contexto.verify_mode == __import__("ssl").CERT_NONE


# ---------------------------------------------------------------------------
# Un rechazo llega con HTTP 400 y el resultado dentro
#
# Observado contra el contenedor de stage el 2026-07-26. El cliente lo tomaba
# por una avería —«el contenedor no respondió»— y descartaba los hallazgos, que
# son justo lo que se fue a buscar.
# ---------------------------------------------------------------------------

RECHAZO_400 = {
    "ResultState": False,
    "ProcesoId": 0,
    "Ambiente": "DockerStage",
    "CodigoUnicoValidacion": "-",
    "ResultadosValidacion": [
        {"Clase": "RECHAZADO", "Codigo": "RVG01", "Descripcion": "prueba",
         "PathFuente": "/usuarios/0", "Observaciones": ""},
    ],
}


def _cliente_que_responde(codigo, cuerpo):
    t = TransporteFalso({
        RUTA_LOGIN: (200, json.dumps({"token": "t"})),
        RUTA_CARGAR_SIN_FACTURA: (codigo, json.dumps(cuerpo)),
    })
    c = MuvClient("https://localhost:9443", t)
    c.login(CREDS)
    return c


def test_un_400_con_resultado_es_un_RECHAZO_no_una_averia():
    c = _cliente_que_responde(400, RECHAZO_400)
    r = c.cargar({"rips": {}})          # no levanta
    assert r.aceptado is False
    assert r.cuv is None
    assert [h.codigo for h in r.hallazgos] == ["RVG01"]
    assert r.ambiente == "DockerStage"


def test_un_400_que_NO_es_un_resultado_sigue_siendo_averia():
    """Un proxy caído devuelve HTML con 400. Eso sí es indisponibilidad."""
    t = TransporteFalso({
        RUTA_LOGIN: (200, json.dumps({"token": "t"})),
        RUTA_CARGAR_SIN_FACTURA: (400, "<html>Bad Request</html>"),
    })
    c = MuvClient("https://localhost:9443", t)
    c.login(CREDS)
    with pytest.raises(MuvNoDisponible):
        c.cargar({"rips": {}})


def test_un_401_sigue_siendo_credenciales_aunque_traiga_cuerpo():
    """El orden importa: credenciales se comprueba ANTES que la forma."""
    c = MuvClient("https://localhost:9443",
                  TransporteFalso({RUTA_LOGIN: (401, json.dumps(RECHAZO_400))}))
    with pytest.raises(MuvCredencialesInvalidas):
        c.login(CREDS)
