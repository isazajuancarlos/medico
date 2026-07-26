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
#: `medicamentos` salió de aquí el 2026-07-26: era el único de los tres
#: verificado por DOS fuentes independientes —el ejemplo JSON y la tabla
#: numerada M## del Documento Técnico 1—, así que se podía corregir sin
#: adivinar. Para `urgencias` la única fuente fue el ejemplo JSON, y en
#: `hospitalizacion` la tabla dice 30 campos y el ejemplo da 28: hay dos sin
#: identificar. Ninguno de los dos se toca sin leer esas secciones del PDF.
SIN_CORREGIR = {"urgencias", "hospitalizacion"}


def emitidos(arreglo: str) -> list[str]:
    """Los nombres que `rips_completo.py` pone en ese diccionario."""
    fuente = (RAIZ / "rips_completo.py").read_text(encoding="utf-8")
    i = fuente.index(BLOQUES[arreglo])
    j = fuente.index("\n            }", i)
    return re.findall(r'"([A-Za-z0-9]+)":', fuente[i:j])


@pytest.mark.parametrize("arreglo", [
    pytest.param(a, marks=pytest.mark.xfail(
        strict=True,
        reason=f"#67: {a} usa nombres que la 948 no tiene. No se corrige a "
               f"ciegas: en urgencias la única fuente fue el ejemplo JSON, y en "
               f"hospitalización la tabla dice 30 campos y el ejemplo da 28. "
               f"Hay que leer esas secciones del PDF antes de tocarlo."))
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


#: Campos válidos que HOY se emiten en cada arreglo. Es una foto, no un ideal.
#:
#: Existe por un error real: el 2026-07-26, corrigiendo los nombres del AM, se
#: quitó `numAutorizacion` del bloque de PROCEDIMIENTOS —un `replace` con
#: `count=1` sobre una cadena que aparece en cuatro bloques— y ninguna prueba lo
#: notó. Las de arriba miran lo que SOBRA, que es lo que hace rechazar el
#: archivo; nada miraba lo que se pierde.
#:
#: Perder un campo exigido no rechaza el archivo: lo deja incompleto, que es
#: peor, porque pasa el validador y falta información en la factura.
#:
#: Cuando se implemente un campo nuevo, esta lista CRECE. Lo que no puede es
#: encoger sin que alguien lo decida a propósito.
#: MEDIDO, no recordado. La primera versión de esta tabla copió las cifras del
#: apunte de la tarea («urgencias emite 14, pide 25»), pero aquel 14 contaba
#: TODOS los campos emitidos, y aquí solo cuentan los que la norma reconoce. En
#: urgencias son 6 y en hospitalización 9: por eso siguen marcados como fallo
#: esperado. Copiar una cifra de un documento y llamarlo medición es el error
#: que estas pruebas existen para no repetir.
EMITIDOS_HOY = {
    "consultas": 21, "procedimientos": 16, "medicamentos": 11,
    "urgencias": 6, "hospitalizacion": 9,
}


@pytest.mark.parametrize("arreglo", sorted(BLOQUES))
def test_no_se_pierde_un_campo_que_la_norma_exige(arreglo):
    """Los campos válidos no desaparecen sin que se note."""
    validos = set(emitidos(arreglo)) & set(NORMA[arreglo])
    assert len(validos) >= EMITIDOS_HOY[arreglo], (
        f"{arreglo} emite {len(validos)} campos de la norma y antes emitía "
        f"{EMITIDOS_HOY[arreglo]}: se perdió alguno. Si la reducción es "
        f"deliberada, baja el número aquí y di por qué."
    )
