#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
"""Convierte los maestros CIE-10 y CUPS de `.xlsx` a JSON compacto (#77).

Se ejecuta A MANO, no en el arranque ni en el CI: las fuentes son archivos
oficiales que cambian cada varios años, y regenerar es un paso deliberado. El
JSON resultante se versiona con el código y se empotra en el binario, por la
misma razón que DIVIPOLA — un consultorio rural no puede depender de que haya
internet para saber si un diagnóstico existe.

Lee el `.xlsx` con la biblioteca estándar (un xlsx es un zip de XML) para no
añadir una dependencia por una conversión que se hace una vez al lustro.

    python3 herramientas/generar_catalogos.py /ruta/a/docs

=== LA DAGA Y EL ASTERISCO NO SON BASURA ===

378 códigos del CIE-10 acaban en `†` o `*`. No es corrupción: es la notación
daga-asterisco de la CIE, donde `†` marca la etiología y `*` la manifestación.
Pero el MUV valida contra su tabla y `A065†` no está en ninguna, así que el
código se guarda LIMPIO y la notación aparte.

Comprobado antes de decidirlo: los 12 423 códigos son distintos, y siguen siendo
distintos tras quitar la notación. Cero colisiones, así que limpiar no funde dos
diagnósticos en uno.
"""
from __future__ import annotations

import json
import re
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

NS = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"


def _sin_doctype(crudo: bytes, de_donde: str) -> bytes:
    """Rechaza un XML que declare entidades, antes de parsearlo.

    `xml.etree.ElementTree` **sí expande entidades internas**, así que un
    `.xlsx` malicioso puede tumbar el proceso con «billion laughs». Comprobado
    aquí mismo: la entidad EXTERNA (XXE, lectura de archivos) sí la bloquea
    Python —`undefined entity`— pero la interna se expande sin quejarse.

    Importa más de lo que parece porque **la procedencia de una de las fuentes es
    dudosa**: la propia hoja del CIE-10 declara venir de un blog de WordPress.

    Un `.xlsx` producido por Excel o LibreOffice NUNCA lleva `DOCTYPE`, así que
    rechazarlo cierra el vector y no estorba a ningún archivo legítimo. Se
    prefiere a `defusedxml` porque no añade una dependencia a una herramienta
    que se corre a mano una vez al lustro.
    """
    if b"<!DOCTYPE" in crudo or b"<!ENTITY" in crudo:
        raise SystemExit(
            f"{de_donde}: el XML declara entidades. Un .xlsx legítimo no lo hace; "
            "esto se rechaza en vez de parsearlo."
        )
    return crudo


def filas(ruta: Path) -> list[list[str]]:
    """Las filas de la primera hoja, como listas de cadenas."""
    z = zipfile.ZipFile(ruta)
    compartidas: list[str] = []
    if "xl/sharedStrings.xml" in z.namelist():
        raiz = ET.fromstring(_sin_doctype(z.read("xl/sharedStrings.xml"), f"{ruta.name}:sharedStrings"))
        compartidas = [
            "".join(t.text or "" for t in si.iter(NS + "t"))
            for si in raiz.findall(NS + "si")
        ]
    hoja = next(n for n in z.namelist() if n.startswith("xl/worksheets/sheet"))
    raiz = ET.fromstring(_sin_doctype(z.read(hoja), f"{ruta.name}:{hoja}"))
    out = []
    for fila in raiz.findall(f".//{NS}row"):
        celdas = []
        for c in fila.findall(NS + "c"):
            v = c.find(NS + "v")
            if v is None:
                celdas.append("")
            elif c.get("t") == "s" and compartidas:
                celdas.append(compartidas[int(v.text)])
            else:
                celdas.append(v.text or "")
        out.append(celdas)
    return out


def cie10(ruta: Path) -> dict:
    """CIE-10: código de cuatro caracteres, descripción, y su categoría de tres.

    La cabecera está en la fila 3 —las dos primeras son el título y la
    procedencia—, así que los datos empiezan en la 4.
    """
    fs = filas(ruta)
    codigos = []
    categorias: dict[str, str] = {}
    for f in fs[3:]:
        if len(f) < 4:
            continue
        cod3, desc3, cod4, desc4 = (f[0] or ""), (f[1] or ""), (f[2] or ""), (f[3] or "")
        if cod3 and desc3:
            categorias.setdefault(cod3.strip(), desc3.strip())
        if not cod4:
            continue
        limpio = cod4.strip().rstrip("†*")
        notacion = ""
        if cod4.strip().endswith("†"):
            notacion = "daga"
        elif cod4.strip().endswith("*"):
            notacion = "asterisco"
        if not re.fullmatch(r"[A-Z]\d{2,3}", limpio):
            raise SystemExit(f"CIE-10: código con forma inesperada: {cod4!r}")
        codigos.append(
            {
                "codigo": limpio,
                "descripcion": desc4.strip(),
                "categoria": limpio[:3],
                "notacion": notacion,
            }
        )
    vistos = {c["codigo"] for c in codigos}
    if len(vistos) != len(codigos):
        raise SystemExit("CIE-10: hay códigos repetidos tras limpiar la notación")
    return {
        "fuente": "CIE10.xlsx",
        "advertencia": (
            "La hoja declara en su segunda fila «Subido por Niccpaganini · "
            "medicblog.wordpress.com». NO es una publicación oficial de MinSalud "
            "ni de la OMS. La estructura se verificó (12 423 códigos, todos "
            "distintos, todos con forma [A-Z]dd(d) tras quitar daga y asterisco), "
            "pero antes de facturar con esto hay que contrastarlo contra la tabla "
            "de referencia vigente del MUV."
        ),
        "categorias": categorias,
        "codigos": codigos,
    }


def cups(ruta: Path) -> dict:
    """CUPS: código, nombre, sección, en qué archivo del RIPS va, y si es quirúrgico.

    === DOS TRAMPAS EN LA CABECERA, Y LA SEGUNDA VALE MÁS QUE LA PRIMERA ===

    1. Los datos van CORRIDOS UNO respecto de la cabecera: la primera columna de
       cada fila trae la etiqueta constante «CUPSRips», que la cabecera no nombra.

    2. **Los nombres de la cabecera no describen su contenido.** La columna que
       dice «Habilitado» no lleva un sí/no: lleva `AP`, `AC` o `AT` — el archivo
       del RIPS en el que se reporta ese procedimiento (13 121 en AP, 334 en AC,
       185 en AT). La que de verdad dice si es quirúrgico es la siguiente, con
       `S`/`N`/`Z`.

       Se descubrió leyendo los VALORES de cada columna en vez de fiarse de su
       nombre. Y no es un detalle: `ambito_rips` responde justo la pregunta que
       el generador de RIPS tiene que hacerse por cada procedimiento.
    """
    fs = filas(ruta)
    out = []
    secciones_malas = []
    for f in fs[1:]:
        if len(f) < 6 or not f[1]:
            continue
        codigo = f[1].strip()
        nombre = (f[2] or "").strip()
        seccion = (f[3] or "").strip()
        ambito = (f[4] or "").strip().upper()
        if "SECCION" not in seccion.upper():
            # Se conserva TAL CUAL y se cuenta: corregirlo a ojo sería inventar
            # a qué sección pertenece. Que salga en el informe.
            secciones_malas.append((codigo, seccion))
        if ambito not in {"AP", "AC", "AT"}:
            raise SystemExit(f"CUPS {codigo}: ámbito RIPS inesperado {ambito!r}")
        out.append(
            {
                "codigo": codigo,
                "nombre": nombre,
                "seccion": seccion,
                "ambito_rips": ambito,
                "quirurgico": (f[5] or "").strip().upper() == "S",
            }
        )
    vistos = {c["codigo"] for c in out}
    if len(vistos) != len(out):
        raise SystemExit("CUPS: hay códigos repetidos")
    return {
        "fuente": "cups.xlsx",
        "secciones_ilegibles": [
            {"codigo": c, "seccion": s} for c, s in secciones_malas
        ],
        "codigos": out,
    }


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("uso: generar_catalogos.py <carpeta con CIE10.xlsx y cups.xlsx>")
    origen = Path(sys.argv[1])
    destino = Path(__file__).resolve().parent.parent / "crates/medico-datos/datos"
    destino.mkdir(parents=True, exist_ok=True)

    for nombre, fn, archivo in [
        ("CIE-10", cie10, "CIE10.xlsx"),
        ("CUPS", cups, "cups.xlsx"),
    ]:
        datos = fn(origen / archivo)
        salida = destino / (archivo.split(".")[0].lower() + ".json")
        # Compacto a propósito: va empotrado en el binario.
        salida.write_text(
            json.dumps(datos, ensure_ascii=False, separators=(",", ":")),
            encoding="utf-8",
        )
        n = len(datos["codigos"])
        print(f"{nombre}: {n} códigos -> {salida.name} ({salida.stat().st_size // 1024} KB)")
        if nombre == "CUPS" and datos["secciones_ilegibles"]:
            print(f"  ⚠ {len(datos['secciones_ilegibles'])} secciones ilegibles, conservadas tal cual:")
            for x in datos["secciones_ilegibles"]:
                print(f"      {x['codigo']}  {x['seccion']!r}")


if __name__ == "__main__":
    main()
