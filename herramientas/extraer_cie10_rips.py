"""Extrae la tabla CIE-10 del RIPS del PDF oficial de MinSalud.

Se ejecuta a mano y deja `backend/datos/cie10_rips.json`. No se llama en
caliente: el PDF es una publicación que cambia cada varios años, no un servicio.

Por qué del PDF y no del CIE10.xlsx que también tenemos: son tablas distintas.
El xlsx trae la CIE-10 tal cual (códigos de 3 y 4 caracteres, `A09` sin más);
el PDF trae **la tabla del RIPS**, que es la que valida el MUV y que además
lleva las restricciones de sexo, los límites de edad y la marca de «no es
afección principal». Y sobre todo: en el RIPS todo código va a CUATRO
caracteres, así que las categorías sin subdivisión se rellenan con `X` —
`A09X`, 254 casos. Cargar del xlsx dejaría `A09`, que el validador rechaza.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
PDF = RAIZ / "backend" / "datos" / "normativa" / "cie10-rips-minsalud.pdf"
SALIDA = RAIZ / "backend" / "datos" / "cie10_rips.json"

#: código, [símbolo], descripción, sexo, límite inferior, límite superior, cola.
#: La cola trae la columna «No son afección principal» —que el PDF imprime como
#: un `0`— y las observaciones («Ver RN»).
FILA = re.compile(
    r"^\s*([A-Z]\d{2}[0-9A-Z])\s+(.*?)\s+([HMA])\s+(\d{1,3})\s+(\d{1,3})(.*?)\s*$"
)
#: El símbolo daga/asterisco va en su propia columna y no forma parte del texto.
SIMBOLOS = "†*‡"


def _limpiar(texto: str) -> str:
    return re.sub(r"\s+", " ", texto.strip(" " + SIMBOLOS)).strip()


def _es_continuacion(linea: str) -> bool:
    """Si la línea es el resto de una descripción que no cupo.

    Son líneas sin código, todo en mayúsculas y sin las columnas numéricas. Una
    descripción puede ocupar TRES o más renglones: mirar solo el anterior deja
    la descripción truncada por delante —«DE CADENA RAMIFICADA, NO
    ESPECIFICADOS» en vez de «OTROS TRASTORNOS DEL METABOLISMO DE LOS
    AMINOACIDOS DE CADENA RAMIFICADA, NO ESPECIFICADOS»— y eso es peor que no
    tenerla, porque parece un diagnóstico distinto.
    """
    t = linea.strip()
    if not t or FILA.match(linea):
        return False
    if re.fullmatch(r"[\d\s]+", t):          # número de página
        return False
    if re.search(r"\s[HMA]\s+\d{1,3}\s", linea):  # arrastra columnas numéricas
        return False
    # Los dígitos SÍ aparecen dentro de la descripción —«VITAMINA B12»,
    # «GLUCOSA-6-FOSFATO»—, así que descartar toda línea con cifras truncaba 66
    # descripciones. Lo que descalifica es la forma de las columnas, no el dígito.
    return t == t.upper()


def extraer(pdf: Path = PDF) -> list[dict]:
    crudo = subprocess.run(
        ["pdftotext", "-layout", str(pdf), "-"],
        capture_output=True, text=True, check=True,
    ).stdout
    lineas = crudo.split("\n")

    filas: list[dict] = []
    for n, linea in enumerate(lineas):
        m = FILA.match(linea)
        if not m:
            continue
        codigo, desc, sexo, inf, sup, cola = m.groups()
        # La descripción se parte cuando no cabe: los renglones de arriba van
        # solos, sin código. Se recogen todos, no solo el inmediato.
        previas: list[str] = []
        k = n - 1
        while k >= 0 and _es_continuacion(lineas[k]):
            previas.append(_limpiar(lineas[k]))
            k -= 1
        # `previas` se llenó de abajo hacia arriba; el fragmento que acompaña al
        # código es el ÚLTIMO renglón, no el primero.
        desc = " ".join(list(reversed(previas)) + [_limpiar(desc)])
        filas.append({
            "codigo": codigo,
            "descripcion": _limpiar(desc),
            "sexo": sexo,
            "edad_min": int(inf),
            "edad_max": int(sup),
            # V, W, X e Y —las causas externas— están marcadas al 100 %, junto
            # con las secuelas y buena parte de los códigos Z. Un RIPS que
            # ponga uno de estos como diagnóstico principal se rechaza.
            "no_principal": cola.strip().startswith("0"),
            "observaciones": "Ver RN" if "Ver RN" in cola else None,
        })
    return filas


def main() -> int:
    filas = extraer()
    codigos = {f["codigo"] for f in filas}
    if len(codigos) != len(filas):
        print(f"AVISO: {len(filas) - len(codigos)} códigos repetidos", file=sys.stderr)
    # Nos negamos a escribir una tabla obviamente incompleta: el PDF tiene del
    # orden de 12.000 códigos y un fallo de extracción se nota en el conteo.
    if len(codigos) < 11_000:
        print(f"ERROR: solo se extrajeron {len(codigos)} códigos", file=sys.stderr)
        return 1
    SALIDA.write_text(
        json.dumps({"fuente": PDF.name, "codigos": sorted(filas, key=lambda f: f["codigo"])},
                   ensure_ascii=False),
        encoding="utf-8",
    )
    con_x = sum(1 for c in codigos if c.endswith("X"))
    print(f"{len(codigos)} códigos ({con_x} rellenados con X) → {SALIDA}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
