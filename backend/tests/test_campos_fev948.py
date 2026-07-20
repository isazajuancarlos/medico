"""Lo que emitimos tiene que llamarse como la Resolución 948 dice.

Esta prueba compara los diccionarios que arma `rips_completo.py` contra
`datos/normativa/campos_fev948.json`, extraído del Documento Técnico 1 por
`herramientas/extraer_campos_fev948.py`. La norma deja de ser algo que alguien
recuerda y pasa a ser un dato contra el que se falla.

=== POR QUÉ IMPORTA MÁS LO QUE SOBRA QUE LO QUE FALTA ===

Un campo que falta puede ser opcional. Un campo con un nombre que la norma no
tiene hace RECHAZAR el archivo entero, y con él la factura. Al medirlo por
primera vez:

    medicamentos    emitía `codMedicamento`, `nombreMedicamento`,
                    `fechaDispensacion` — la norma dice `codTecnologiaSalud`,
                    `nomTecnologiaSalud`, `fechaDispensAdmon`
    urgencias       emitía `causaMotivo` y `fechaEgresoAtencion` — la norma
                    dice `causaMotivoAtencion` y `fechaEgreso`
    hospitalización lo mismo, más `diasEstancia`, que no existe en la norma

Ninguno de esos errores se ve mirando el archivo: tiene la forma de un RIPS.

Se prueba sobre la ESTRUCTURA, sin base de datos: lo que se fija aquí son los
nombres, y para eso no hacen falta datos de pacientes.
"""
import json
import os
import re
import sys
from pathlib import Path

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_campos948.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

RAIZ = Path(__file__).resolve().parent.parent
NORMA = json.loads(
    (RAIZ / "datos" / "normativa" / "campos_fev948.json").read_text(encoding="utf-8")
)["arreglos"]

#: Dónde arma cada archivo su diccionario de servicio, en `rips_completo.py`.
BLOQUES = {
    "consultas": "consulta = {",
    "procedimientos": "procedimiento = {",
    "medicamentos": "medicamento = {",
    "urgencias": "urgencia = {",
    "hospitalizacion": "hospitalizacion_rec = {",
}

#: Campos que medico añade y la norma no nombra, con la razón de cada uno.
#: Vacío no es la meta: la meta es que nada esté aquí sin justificación.
TOLERADOS = {
    "consultas": {"consecutivo"},
    "procedimientos": {"consecutivo"},
    "medicamentos": {"consecutivo"},
    # `tipoDocumentoIdentificacion` y `numDocumentoIdentificacion` NO aparecen
    # en la lista extraída de urgencias ni de hospitalización, pero sí en la de
    # consultas y procedimientos. No me creo la ausencia: las tablas R y H son
    # las dos que `pdftotext` peor descompone, y el resto de archivos sí los
    # llevan. Se toleran hasta comprobarlo A OJO contra el PDF.
    "urgencias": {"consecutivo", "tipoDocumentoIdentificacion",
                  "numDocumentoIdentificacion"},
    "hospitalizacion": {"consecutivo", "tipoDocumentoIdentificacion",
                        "numDocumentoIdentificacion"},
}

#: Archivos con el defecto todavía sin corregir (#67). Se marcan como fallo
#: ESPERADO en vez de borrar la prueba: así el defecto queda escrito en el
#: código y, en cuanto se arregle, `strict=True` obliga a quitar la marca.
#: Ver [[principio-registrar-cerca-de-la-violacion]].
SIN_CORREGIR = {"medicamentos", "urgencias", "hospitalizacion"}


def emitidos(arreglo: str) -> list[str]:
    """Los nombres que `rips_completo.py` pone en ese diccionario."""
    fuente = (RAIZ / "rips_completo.py").read_text(encoding="utf-8")
    i = fuente.index(BLOQUES[arreglo])
    j = fuente.index("\n            }", i)
    return re.findall(r'"([A-Za-z0-9]+)":', fuente[i:j])


@pytest.mark.parametrize("arreglo", [
    pytest.param(a, marks=pytest.mark.xfail(
        strict=True,
        reason=f"#67: {a} usa nombres que la 948 no tiene (el AM dice "
               f"codMedicamento donde la norma dice codTecnologiaSalud)"))
    if a in SIN_CORREGIR else a
    for a in sorted(BLOQUES)
])
def test_no_se_emite_ningun_campo_que_la_norma_no_tenga(arreglo):
    """El que sobra es el que hace rechazar la factura."""
    sobran = set(emitidos(arreglo)) - set(NORMA[arreglo]) - TOLERADOS[arreglo]
    assert not sobran, (
        f"{arreglo} emite campos que la Resolución 948 no nombra: "
        f"{sorted(sobran)}. El MUV rechaza el archivo completo."
    )


@pytest.mark.parametrize("arreglo", sorted(BLOQUES))
def test_no_se_repite_ningun_campo(arreglo):
    campos = emitidos(arreglo)
    assert len(campos) == len(set(campos)), f"{arreglo} repite claves"


class TestLaNormaCargada:
    def test_estan_los_siete_arreglos(self):
        assert set(NORMA) == {
            "consultas", "procedimientos", "urgencias", "hospitalizacion",
            "recienNacidos", "medicamentos", "otrosServicios",
        }

    def test_las_cuentas_son_las_del_documento(self):
        """Si alguien regenera el JSON y sale distinto, se entera aquí."""
        assert {a: len(c) for a, c in NORMA.items()} == {
            "consultas": 30, "procedimientos": 27, "urgencias": 25,
            "hospitalizacion": 30, "recienNacidos": 18, "medicamentos": 28,
            "otrosServicios": 18,
        }

    def test_los_campos_CIE11_son_veintidos(self):
        """Lo nuevo de la 948. Estaban anotados como 35 sin haberlos contado."""
        cie11 = {c for v in NORMA.values() for c in v if c.endswith("CIE11")}
        assert len(cie11) == 22
        assert sum(1 for c in cie11 if c.startswith("nomCod")) == 11

    def test_cada_codigo_CIE11_tiene_su_nombre(self):
        """La norma los pide en pareja: el código y su descripción."""
        cie11 = {c for v in NORMA.values() for c in v if c.endswith("CIE11")}
        for c in cie11:
            if c.startswith("nomCod"):
                continue
            assert f"nomCod{c[3:]}" in cie11, f"{c} va sin su nomCod"

    def test_codigoVIDA_esta_en_los_siete(self):
        """No estaba en ninguna lista de pendientes anterior."""
        for arreglo, campos in NORMA.items():
            assert "codigoVIDA" in campos, arreglo
