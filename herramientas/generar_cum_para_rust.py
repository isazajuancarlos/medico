#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
"""Extrae los códigos del CatalogoCUMs para empotrarlos en el binario Rust (#67).

    python3 herramientas/generar_cum_para_rust.py

Lee `backend/datos/cum.json.gz` —el catálogo del INVIMA que ya usa el Python— y
escribe `crates/medico-dominio/datos/cum_codigos.txt` con **solo las claves**,
una por línea y ordenadas.

# Por qué solo las claves

El AM valida `codTecnologiaSalud` contra el catálogo, y para eso basta la
pertenencia: el nombre comercial, el laboratorio y los principios activos no
entran en el RIPS. 66 mil claves son 727 KB de texto; el catálogo entero son 2,6
MB comprimidos y bastante más en memoria. Se empotra lo que se usa.

# Por qué no se lee el `.gz` desde Rust

Descomprimir en el binario obligaría a una dependencia nueva (`flate2`) para leer
un archivo que cambia una vez al mes en el mejor caso. El texto plano se empotra
con `include_str!` y no añade nada al árbol de dependencias.

# Los dos archivos no pueden divergir

Este guion se corre A MANO cuando se regenera `cum.json.gz`, igual que
`generar_mapeo_cie11.py`. Que nadie se acuerde es exactamente lo que pasa, así
que la vigilancia no queda en esta nota: `backend/tests/test_cum_paridad_con_rust.py`
compara el archivo generado contra el `.gz` y se pone rojo si uno de los dos se
movió sin el otro.
"""
from __future__ import annotations

import gzip
import json
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
ORIGEN = RAIZ / "backend" / "datos" / "cum.json.gz"
DESTINO = RAIZ / "crates" / "medico-dominio" / "datos" / "cum_codigos.txt"


def codigos() -> list[str]:
    """Las claves del catálogo, ordenadas y sin repetir."""
    with gzip.open(ORIGEN, "rt", encoding="utf-8") as f:
        catalogo = json.load(f)
    productos = catalogo.get("productos")
    if not isinstance(productos, dict) or not productos:
        raise SystemExit(
            f"{ORIGEN}: no trae un mapa «productos» con datos. No se escribe un "
            f"archivo vacío: un catálogo vacío haría que TODO CUM se rechazara, "
            f"o —peor— que la validación pareciera pasar sin comprobar nada."
        )
    return sorted(productos)


def main() -> None:
    lista = codigos()
    DESTINO.write_text("\n".join(lista) + "\n", encoding="utf-8")
    print(
        f"{len(lista)} códigos CUM -> {DESTINO.relative_to(RAIZ)} "
        f"({DESTINO.stat().st_size // 1024} KB)"
    )
    if len(lista) < 10_000:
        print(
            "AVISO: el catálogo trae menos de 10 000 códigos, que no es lo que "
            "publica el INVIMA. Revise el origen antes de commitear.",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
