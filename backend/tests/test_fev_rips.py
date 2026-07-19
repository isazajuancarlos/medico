"""El paquete FEV-RIPS: consolidación, sobres y numeración de consecutivos.

Las reglas que se fijan aquí están verificadas contra el MANUAL DE USUARIO
CONVERTIDOR A JSON, que declara seguir la Resolución 948 de 2026:

  - El consecutivo de USUARIO va de 1 a N sin repetirse, y representa la cantidad
    total de usuarios del reporte.
  - El consecutivo de ATENCIÓN va por usuario y por tipo de servicio, empezando
    en 1 cada vez.
  - `numNota` es obligatorio y no se puede repetir: repetirlo genera RECHAZO.

Se prueba con documentos sintéticos, sin tocar la base ni datos de pacientes.
"""
import os
import sys

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_fev.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fev_rips import (  # noqa: E402
    TIPOS_SERVICIO, consolidar, contar_atenciones, sobre_fev,
    sobre_rips_sin_factura,
)


def _usuario(documento, servicios, incapacidad="NO", consecutivo=1):
    return {
        "tipoDocumentoIdentificacion": "CC",
        "numDocumentoIdentificacion": documento,
        "tipoUsuario": "12",
        "fechaNacimiento": "1991-06-25",
        "codSexo": "F",
        "codPaisResidencia": "170",
        "codMunicipioResidencia": "05001",
        "codZonaTerritorialResidencia": "02",
        "codPaisOrigen": "170",
        "incapacidad": incapacidad,
        "consecutivo": consecutivo,
        "servicios": servicios,
    }


def _consulta(codigo="890201", fecha="2026-06-26 11:28:00", **extra):
    base = {
        "codPrestador": "192030405001",
        "fechaInicioAtencion": fecha,
        "numAutorizacion": "",
        "codConsulta": codigo,
        "codDiagnosticoPrincipal": "J00",
        "codDiagnosticoRelacionado1": "",
        "codDiagnosticoRelacionado2": "",
        "codDiagnosticoRelacionado3": "",
        "numFEVPagoModerador": "",
        "vrServicio": 50000,
        "consecutivo": 1,
    }
    base.update(extra)
    return base


def _doc(usuarios, obligado="530520110"):
    return {"numDocumentoIdObligado": obligado, "usuarios": usuarios}


# --- Consolidación -----------------------------------------------------------

def test_un_paciente_en_dos_documentos_queda_UNA_vez():
    """El motivo de existir del módulo: rips_completo devuelve seis documentos
    separados y un paciente con consulta y medicamento aparece en dos."""
    ac = _doc([_usuario("111", {"consultas": [_consulta()]})])
    am = _doc([_usuario("111", {"medicamentos": [{"codigo": "M1", "consecutivo": 1}]})])

    usuarios = consolidar([ac, am])

    assert len(usuarios) == 1, "es la misma persona, no dos"
    assert set(usuarios[0]["servicios"]) == {"consultas", "medicamentos"}


def test_pacientes_distintos_no_se_fusionan():
    ac = _doc([_usuario("111", {"consultas": [_consulta()]}),
               _usuario("222", {"consultas": [_consulta()]}, consecutivo=2)])
    usuarios = consolidar([ac])
    assert len(usuarios) == 2


def test_NO_se_fusiona_por_consecutivo_sino_por_documento():
    """La trampa que evitaría un dato clínico falso.

    Cada documento numera sus usuarios desde 1, así que el consecutivo 1 del AC
    y el 1 del AM son personas DISTINTAS. Fusionar por consecutivo le pegaría a
    un paciente los medicamentos de otro.
    """
    ac = _doc([_usuario("AAA", {"consultas": [_consulta()]}, consecutivo=1)])
    am = _doc([_usuario("BBB", {"medicamentos": [{"consecutivo": 1}]}, consecutivo=1)])

    usuarios = consolidar([ac, am])

    assert len(usuarios) == 2, "consecutivo 1 en dos documentos no es la misma persona"
    por_doc = {u["numDocumentoIdentificacion"]: u["servicios"] for u in usuarios}
    assert "medicamentos" not in por_doc["AAA"]
    assert "consultas" not in por_doc["BBB"]


def test_la_incapacidad_SI_gana_sobre_NO():
    """Si en cualquiera de sus atenciones hubo incapacidad, el usuario la tiene."""
    ac = _doc([_usuario("111", {"consultas": [_consulta()]}, incapacidad="NO")])
    au = _doc([_usuario("111", {"urgencias": [{"consecutivo": 1}]}, incapacidad="SI")])

    assert consolidar([ac, au])[0]["incapacidad"] == "SI"
    assert consolidar([au, ac])[0]["incapacidad"] == "SI", "y no depende del orden"


# --- Numeración de consecutivos ----------------------------------------------

def test_los_usuarios_se_renumeran_de_1_a_N_sin_repetir():
    ac = _doc([_usuario(str(i), {"consultas": [_consulta()]}, consecutivo=99)
               for i in range(4)])
    usuarios = consolidar([ac])
    assert [u["consecutivo"] for u in usuarios] == [1, 2, 3, 4]


def test_las_atenciones_se_numeran_por_usuario_y_por_tipo():
    """Manual del convertidor: «si un usuario fue atendido 3 veces, el
    consecutivo de ese usuario inicia en 1 y secuencialmente va 1, 2, 3»."""
    ac = _doc([
        _usuario("111", {"consultas": [_consulta("A"), _consulta("B"), _consulta("C")]}),
        _usuario("222", {"consultas": [_consulta("D")]}, consecutivo=2),
    ])
    usuarios = {u["numDocumentoIdentificacion"]: u for u in consolidar([ac])}

    assert [c["consecutivo"] for c in usuarios["111"]["servicios"]["consultas"]] == [1, 2, 3]
    assert [c["consecutivo"] for c in usuarios["222"]["servicios"]["consultas"]] == [1]


def test_cada_tipo_de_servicio_reinicia_su_numeracion():
    ac = _doc([_usuario("111", {"consultas": [_consulta("A"), _consulta("B")]})])
    ap = _doc([_usuario("111", {"procedimientos": [{"c": 1}, {"c": 2}]})])
    u = consolidar([ac, ap])[0]

    assert [x["consecutivo"] for x in u["servicios"]["consultas"]] == [1, 2]
    assert [x["consecutivo"] for x in u["servicios"]["procedimientos"]] == [1, 2]


# --- Normalización de campos -------------------------------------------------

def test_las_cadenas_vacias_pasan_a_null():
    """El esquema declara estos campos nulables; enviar "" es causal de rechazo."""
    ac = _doc([_usuario("111", {"consultas": [_consulta()]})])
    consulta = consolidar([ac])[0]["servicios"]["consultas"][0]

    for campo in ("numAutorizacion", "codDiagnosticoRelacionado1",
                  "codDiagnosticoRelacionado2", "codDiagnosticoRelacionado3",
                  "numFEVPagoModerador"):
        assert consulta[campo] is None, f"{campo} debía quedar en null"


def test_un_valor_real_no_se_borra():
    ac = _doc([_usuario("111", {"consultas": [_consulta(numAutorizacion="306540078",
                                                        codDiagnosticoRelacionado1="C402")]})])
    consulta = consolidar([ac])[0]["servicios"]["consultas"][0]
    assert consulta["numAutorizacion"] == "306540078"
    assert consulta["codDiagnosticoRelacionado1"] == "C402"


def test_la_fecha_va_sin_segundos():
    """El manual muestra "2025-06-26 11:28"; la extracción produce segundos."""
    ac = _doc([_usuario("111", {"consultas": [_consulta(fecha="2026-06-26 11:28:47")]})])
    consulta = consolidar([ac])[0]["servicios"]["consultas"][0]
    assert consulta["fechaInicioAtencion"] == "2026-06-26 11:28"


def test_una_fecha_ya_corta_no_se_estropea():
    ac = _doc([_usuario("111", {"consultas": [_consulta(fecha="2026-06-26 11:28")]})])
    consulta = consolidar([ac])[0]["servicios"]["consultas"][0]
    assert consulta["fechaInicioAtencion"] == "2026-06-26 11:28"


# --- Sobres ------------------------------------------------------------------

def test_el_sobre_sin_factura_va_sin_xml_y_con_tipoNota_RS():
    p = sobre_rips_sin_factura("530520110", [], num_nota="NAF20")
    assert p["xmlFevFile"] == ""
    assert p["rips"]["tipoNota"] == "RS"
    assert p["rips"]["numFactura"] is None
    assert p["rips"]["numNota"] == "NAF20"


def test_sin_numNota_no_se_arma_el_paquete():
    """El manual advierte que repetirlo o dejarlo vacío genera RECHAZO. Mejor
    fallar aquí, donde todavía se puede corregir, que gastar un ciclo del MUV."""
    for vacio in ("", "   ", None):
        with pytest.raises(ValueError, match="numNota"):
            sobre_rips_sin_factura("530520110", [], num_nota=vacio)


def test_el_sobre_de_factura_exige_el_xml():
    """Una IPS que factura electrónicamente no puede enviar sin AttachedDocument."""
    with pytest.raises(ValueError, match="Base64|factura"):
        sobre_fev("530520110", [], num_factura="WCBR102030", xml_fev_base64="")


def test_el_sobre_de_factura_bien_armado():
    p = sobre_fev("530520110", [], num_factura="WCBR102030", xml_fev_base64="UEsDBBQ=")
    assert p["rips"]["numFactura"] == "WCBR102030"
    assert p["rips"]["tipoNota"] is None
    assert p["xmlFevFile"] == "UEsDBBQ="


# --- Cobertura de tipos y conteo ---------------------------------------------

def test_estan_los_siete_tipos_de_atencion():
    """Las nueve pestañas del convertidor menos transacción y usuarios.
    Un consultorio llena dos; un hospital, los siete."""
    assert set(TIPOS_SERVICIO) == {
        "consultas", "urgencias", "procedimientos", "hospitalizacion",
        "recienNacidos", "medicamentos", "otrosServicios",
    }


def test_el_conteo_cuadra_con_lo_consolidado():
    """Sirve para contrastar contra CantidadTotalAtenciones de ConsultarCUV."""
    ac = _doc([_usuario("111", {"consultas": [_consulta(), _consulta()]}),
               _usuario("222", {"consultas": [_consulta()]}, consecutivo=2)])
    am = _doc([_usuario("111", {"medicamentos": [{"x": 1}]})])

    paquete = sobre_rips_sin_factura("530520110", consolidar([ac, am]), num_nota="N1")
    conteo = contar_atenciones(paquete)

    assert conteo["consultas"] == 3
    assert conteo["medicamentos"] == 1
    assert conteo["urgencias"] == 0


def test_un_periodo_sin_atenciones_produce_un_paquete_vacio_valido():
    paquete = sobre_rips_sin_factura("530520110", consolidar([]), num_nota="N1")
    assert paquete["rips"]["usuarios"] == []
    assert contar_atenciones(paquete)["consultas"] == 0


def test_el_paquete_es_json_serializable():
    import json
    ac = _doc([_usuario("111", {"consultas": [_consulta()]})])
    paquete = sobre_rips_sin_factura("530520110", consolidar([ac]), num_nota="N1")
    texto = json.dumps(paquete, ensure_ascii=False)
    assert json.loads(texto) == paquete
