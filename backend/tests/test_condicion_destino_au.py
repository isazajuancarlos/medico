# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas
"""El destino del egreso de urgencias, contra el maestro del validador (#67).

Esta prueba existe por un fallo que estuvo vivo y no dio ni un error: el mapa de
`generar_archivo_au` traducía el destino de salida a un código del RIPS, y

1. sus claves no eran las de `DestinoSalidaUrgencias` —eran `ALTA`,
   `HOSPITALIZADO`, `REMITIDO`, `OBSERVACION`—, así que **cuatro de los seis
   destinos no casaban con nada** y salían como falta;
2. los dos que sí casaban emitían el código **equivocado**: `FALLECIDO = "04"`,
   y el 04 del catálogo es REFERIDO A OTRA INSTITUCION.

O sea que una muerte en urgencias se reportaba como un traslado. Se corrigió al
portar el AU a Rust, y estas comprobaciones son lo que impide que vuelva: miden
el mapa contra el ENUM por un lado y contra el MAESTRO por el otro, que son las
dos mitades por las que se rompió.
"""
from __future__ import annotations

import inspect
import re

import pytest

import maestros_muv
import rips_completo
from models import DestinoSalidaUrgencias


def _mapa_del_generador() -> dict[str, str]:
    """El mapa tal y como está escrito en el generador.

    Se lee del código fuente a propósito: importar una copia declarada aparte
    probaría la copia, no lo que el generador usa.
    """
    fuente = inspect.getsource(rips_completo.generar_archivo_au)
    bloque = re.search(r"destino_map = \{(.*?)\n        \}", fuente, re.S)
    assert bloque, "no se encontró el mapa en generar_archivo_au"
    return dict(re.findall(r'"([^"]+)":\s*"(\d\d)"', bloque.group(1)))


@pytest.fixture(scope="module")
def catalogo() -> dict[str, dict]:
    return {f["Codigo"]: f for f in maestros_muv.condiciones_destino()}


def test_el_catalogo_esta_extraido_y_no_vacio(catalogo):
    """Positivo de control: sin él, todo lo de abajo pasaría por vacuidad."""
    assert len(catalogo) == 8
    assert catalogo["02"]["Nombre"] == "PACIENTE MUERTO"


def test_todo_codigo_del_mapa_existe_y_aplica_a_urgencias(catalogo):
    for destino, codigo in _mapa_del_generador().items():
        assert codigo in catalogo, f"«{destino}» apunta al {codigo}, que no existe"
        assert catalogo[codigo]["AplicaAU"] == "SI", (
            f"«{destino}» apunta al {codigo} ({catalogo[codigo]['Nombre']}), "
            f"que no aplica al archivo de urgencias"
        )


@pytest.mark.parametrize("destino", list(DestinoSalidaUrgencias))
def test_cada_destino_que_el_sistema_guarda_se_traduce(destino, catalogo):
    """La mitad que se rompió en silencio: las CLAVES.

    Con las claves viejas (`ALTA`, `HOSPITALIZADO`…) esta prueba se pone roja
    para cuatro de los seis destinos, que es exactamente lo que pasaba en
    producción sin que nadie lo viera.
    """
    mapa = _mapa_del_generador()
    clave = str(destino.value).upper()
    if destino is DestinoSalidaUrgencias.FUGA:
        # A propósito: el catálogo no tiene código para «se fue sin alta», y el
        # 01 afirmaría que salió con destino a su domicilio.
        assert clave not in mapa
        return
    assert clave in mapa, (
        f"«{destino.value}» no casa con ninguna clave del mapa: saldría como "
        f"falta y, con supuestos permitidos, como «01» (a su casa)"
    )


def test_el_muerto_es_el_02_y_el_04_es_un_traslado(catalogo):
    """El caso concreto que estuvo mal, escrito para que no vuelva."""
    mapa = _mapa_del_generador()
    assert mapa["FALLECIDO"] == "02"
    assert "MUERTO" in catalogo["02"]["Nombre"]
    assert "REFERIDO A OTRA" in catalogo["04"]["Nombre"]
    assert mapa["FALLECIDO"] != "04"


def test_los_dos_sistemas_dicen_lo_mismo():
    """El mapa del Python y el de Rust no pueden divergir.

    `medico_dominio::muv::condicion_destino_de_urgencias` traduce el vocabulario
    de SQLite (`DOMICILIO`, `HOSPITALIZACION`…) y este el de Postgres
    (`Domicilio`, `Hospitalización`…). Son dos escrituras del mismo concepto, y
    si un día uno cambia sin el otro, los dos sistemas emitirían archivos
    distintos para la misma atención.
    """
    import pathlib

    rust = (pathlib.Path(__file__).resolve().parents[2]
            / "crates" / "medico-dominio" / "src" / "muv.rs").read_text(encoding="utf-8")
    bloque = re.search(
        r"pub fn condicion_destino_de_urgencias\(.*?\n\}", rust, re.S
    )
    assert bloque, "no se encontró la función en muv.rs"
    de_rust = dict(re.findall(r'"([A-Z]+)" => Some\("(\d\d)"\)', bloque.group(0)))
    # Se comparan por CONCEPTO, quitando tildes y unificando a la forma de Rust.
    equivalencias = {
        "DOMICILIO": "DOMICILIO",
        "HOSPITALIZACIÓN": "HOSPITALIZACION",
        "REMISIÓN": "REMISION",
        "OBSERVACIÓN": "OBSERVACION",
        "FALLECIDO": "FALLECIDO",
    }
    for clave_py, codigo in _mapa_del_generador().items():
        clave_rs = equivalencias[clave_py]
        assert de_rust.get(clave_rs) == codigo, (
            f"el Python traduce «{clave_py}» al {codigo} y el Rust al "
            f"{de_rust.get(clave_rs)}"
        )
    assert "FUGA" not in de_rust, "la fuga tampoco se traduce en Rust"
