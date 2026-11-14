# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
"""El catálogo CUM del binario Rust no puede separarse del del Python (#67).

El AM valida `codTecnologiaSalud` contra el CatalogoCUMs, y **hay dos copias**:
`backend/datos/cum.json.gz`, que lee el Python, y
`crates/medico-dominio/datos/cum_codigos.txt`, que se empotra en el binario y
genera `herramientas/generar_cum_para_rust.py` desde el primero.

Que las dos se separen no da ningún error: el binario aceptaría un CUM que el
Python rechaza —o al revés— y los dos sistemas emitirían archivos distintos para
la misma fórmula. Esto se pone rojo en cuanto una de las dos se mueve sin la
otra.

La prueba vive en el lado Python porque aquí `gzip` es gratis; desde Rust habría
que añadir una dependencia para descomprimir un archivo que se lee una vez al
mes.
"""
from __future__ import annotations

import gzip
import json
from pathlib import Path

import pytest

RAIZ = Path(__file__).resolve().parents[2]
ORIGEN = RAIZ / "backend" / "datos" / "cum.json.gz"
EMPOTRADO = RAIZ / "crates" / "medico-dominio" / "datos" / "cum_codigos.txt"


@pytest.fixture(scope="module")
def del_python() -> set[str]:
    with gzip.open(ORIGEN, "rt", encoding="utf-8") as f:
        return set(json.load(f)["productos"])


@pytest.fixture(scope="module")
def del_rust() -> list[str]:
    return [l for l in EMPOTRADO.read_text(encoding="utf-8").splitlines() if l.strip()]


def test_el_archivo_empotrado_existe_y_no_esta_vacio(del_rust):
    """El positivo de control de todo lo demás.

    Un archivo vacío haría que el binario rechazara TODO CUM, y las
    comprobaciones de «rechaza el inventado» pasarían exactamente igual.
    """
    assert len(del_rust) > 60_000, f"solo {len(del_rust)} códigos"


def test_los_dos_catalogos_traen_los_mismos_codigos(del_python, del_rust):
    faltan = del_python - set(del_rust)
    sobran = set(del_rust) - del_python
    assert not faltan and not sobran, (
        f"el catálogo empotrado en el binario se separó del de Python: "
        f"{len(faltan)} códigos que el Python conoce y el Rust no, "
        f"{len(sobran)} al revés. Corra "
        f"`python3 herramientas/generar_cum_para_rust.py` y commitee el archivo."
    )


def test_el_empotrado_va_ordenado_y_sin_repetir(del_rust):
    """Lo ordenado no es estética: es lo que hace legible el diff cuando el
    INVIMA publica el catálogo nuevo."""
    assert del_rust == sorted(del_rust)
    assert len(del_rust) == len(set(del_rust))


def test_la_prueba_discrimina(del_python, del_rust):
    """El caso rojo: si al empotrado le faltara un código, se vería.

    Sin esto, las tres de arriba podrían estar comparando dos cosas que siempre
    coinciden por construcción —por ejemplo, si un error de lectura dejara los
    dos conjuntos vacíos—.
    """
    mutilado = set(del_rust) - {del_rust[0]}
    assert del_python - mutilado, "quitarle un código TIENE que notarse"
