#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas
"""Extrae del mapeo OMS el subconjunto que el RIPS puede DERIVAR solo (#67).

Se ejecuta a mano, como los demás generadores de maestros, y su salida se
versiona con el código y se empotra en el binario.

=== POR QUÉ UN SUBCONJUNTO Y NO EL MAPEO ENTERO ===

`backend/datos/mapeo_cie10_cie11.json` son 3,3 MB: 12 406 códigos con TODOS sus
destinos, sus URIs y sus capítulos. Eso lo necesita el formulario, que propone
candidatos al médico y guarda la procedencia de lo que él elija.

El RIPS necesita otra cosa, mucho más estrecha: el par (código, título) CUANDO la
equivalencia es EXACTA Y ÚNICA. Son 8 070 códigos y 0,5 MB. Portar el resto al
binario sería portar la maquinaria de un formulario que en Rust todavía no
existe — y lo que no opera se borra, no se traduce.

=== LAS DOS CONDICIONES, Y POR QUÉ NO SE RELAJA NINGUNA ===

**Única**: 1 402 códigos (11,3 %) tienen más de un destino, hasta 42. Poner el
primero de una lista de 42 es exactamente `cod_consulta_cups="890201"` con otro
nombre; que lo elija un algoritmo en vez de una constante no lo mejora.

**Exacta**: una categoría de tres caracteres con destino único sigue siendo menos
específica que el código de cuatro que el médico escribió. Rebajar la
especificidad de un diagnóstico en silencio es perder dato clínico sin que nadie
se entere, y en un RIPS eso no lo rechaza nadie: entra y ahí se queda.

Lo que no se puede derivar sale VACÍO, y la Resolución 948 lo permite: sus
propios ejemplos traen "" y null en estos campos, porque el periodo paralelo no
obliga a tener el CIE-11 todavía.

    python3 herramientas/generar_cie11_derivable.py
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
ORIGEN = RAIZ / "backend" / "datos" / "mapeo_cie10_cie11.json"
DESTINO = RAIZ / "crates" / "medico-datos" / "datos" / "cie11.json"


def main() -> int:
    if not ORIGEN.exists():
        print(f"no está el mapeo de origen: {ORIGEN}", file=sys.stderr)
        return 1

    with ORIGEN.open(encoding="utf-8") as f:
        fuente = json.load(f)

    mapa = fuente["mapa"]
    derivables = {}
    ambiguos = por_categoria = 0
    for cie10, entrada in mapa.items():
        destinos = entrada["destinos"]
        if entrada["nivel"] != "exacto":
            por_categoria += 1
            continue
        if len(destinos) != 1:
            ambiguos += 1
            continue
        d = destinos[0]
        derivables[cie10] = [d["codigo"], d["titulo"]]

    salida = {
        # La procedencia viaja DENTRO del archivo, igual que la advertencia del
        # CIE-10: quien lo lea dentro de tres años tiene que poder saber contra
        # qué versión de la clasificación se derivó, sin buscar el commit.
        "release": fuente["release"],
        "version_tabla": fuente["version_tabla"],
        "fuente": fuente["fuente"],
        "criterio": (
            "Solo equivalencias EXACTAS y ÚNICAS. Lo ambiguo y lo derivado por "
            "categoría se deja vacío a propósito: la Resolución 948 lo permite, "
            "y rellenarlo rebajaría la especificidad del diagnóstico en silencio."
        ),
        "derivables": derivables,
    }

    DESTINO.parent.mkdir(parents=True, exist_ok=True)
    with DESTINO.open("w", encoding="utf-8") as f:
        json.dump(salida, f, ensure_ascii=False, separators=(",", ":"))

    kb = DESTINO.stat().st_size / 1024
    print(f"mapeados en origen : {len(mapa)}")
    print(f"  ambiguos         : {ambiguos}  (más de un destino; NO se derivan)")
    print(f"  por categoría    : {por_categoria}  (menos específicos; NO se derivan)")
    print(f"DERIVABLES         : {len(derivables)}")
    print(f"escrito            : {DESTINO}  ({kb:.0f} KB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
