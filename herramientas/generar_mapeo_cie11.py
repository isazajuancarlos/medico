"""Genera el mapeo CIE-10 ↔ CIE-11 desde las tablas oficiales de la OMS.

Se ejecuta a mano y deja `backend/datos/mapeo_cie10_cie11.json`.

=== POR QUÉ HACE FALTA ===

La Resolución 948 de 2026 abre un periodo en el que un mismo episodio lleva los
DOS códigos: el CIE-10 de siempre y el CIE-11 en los campos nuevos. Pedirle al
médico que codifique dos veces es exactamente el problema que este sistema
existe para no repetir, así que el CIE-11 se DERIVA del CIE-10 cuando la
equivalencia es única.

=== POR QUÉ NO ES UN DICCIONARIO ===

La OMS lo dice en su propio readme: «ICD-10 concepts may no longer have a unique
code in ICD-11». Por eso publica dos tablas, una de destino único y otra de
destinos múltiples. Medido sobre las tablas: **1.285 códigos CIE-10 (el 10,2 %)
se abren en varios CIE-11**, hasta 42 destinos.

En esos el mapeo NO elige. Deja los candidatos y que un humano confirme. Elegir
el primero sería fabricar un diagnóstico —el mismo `"890201"` que quitamos de
los valores por defecto, con mejor coartada.

=== POR QUÉ SE FILTRA CONTRA LA TABLA DEL RIPS ===

La OMS escribe los códigos con punto (`A00.0`) y deja las categorías en tres
caracteres (`A09`). El RIPS los quiere en cuatro sin punto, rellenando con `X`
las categorías sin subdivisión (`A09X`). Convertir a ciegas produciría códigos
con la forma correcta que el validador rechaza, o peor, códigos válidos que no
existen en la tabla colombiana.

Así que se convierte y se COMPRUEBA contra `cie10_rips.json`. Lo que no está,
no se inventa: se cuenta aparte y se informa.
"""

from __future__ import annotations

import csv
import json
import sys
from collections import defaultdict
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
#: Tablas de mapeo CIE-10 ↔ CIE-11 de la OMS. Apuntaban a
#: `~/Documentos/mapping-cie10-cie-11`, que el 2026-08-04 YA NO EXISTE: la única
#: copia viva quedó en `_ajenos-a-quipu/docs`. Se admite ruta por argumento para
#: no volver a cablear un sitio que puede desaparecer, y si no está se dice
#: dónde buscarla en vez de reventar con un rastro de pila.
#: Procedencia y riesgo: `/mnt/data/conocimiento/insumos-normativos.md`.
TABLAS = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
    "/mnt/data/_ajenos-a-quipu/docs/mapping-cie10-cie-11")
if not TABLAS.is_dir():
    raise SystemExit(
        f"No encuentro las tablas de la OMS en {TABLAS}.\n"
        "Pásalas como argumento: generar_mapeo_cie11.py <directorio>.\n"
        "Qué son y de dónde salen: /mnt/data/conocimiento/insumos-normativos.md"
    )
CIE10_RIPS = RAIZ / "backend" / "datos" / "cie10_rips.json"
SALIDA = RAIZ / "backend" / "datos" / "mapeo_cie10_cie11.json"
#: La copia que se empotra en el binario de Rust. La escribe ESTA corrida y no
#: un paso aparte: dos archivos con el mismo contenido mantenidos por separado
#: divergen, y el día que divergen el Python y el Rust derivan CIE-11 distintos
#: para el mismo diagnóstico sin que nada falle.
SALIDA_RUST = RAIZ / "crates" / "medico-dominio" / "datos" / "mapeo_cie10_cie11.json"

UNICO = "10To11MapToOneCategory.txt"
MULTIPLE = "10To11MapToMultipleCategories.txt"


def codigos_rips() -> set[str]:
    datos = json.loads(CIE10_RIPS.read_text(encoding="utf-8"))
    return {c["codigo"] for c in datos["codigos"]}


def a_rips(codigo_oms: str, validos: set[str]) -> str | None:
    """Pasa un código de la OMS a la forma del RIPS, o `None` si no existe.

    `None` no es un fallo del proceso: la CIE-10 de la OMS y la tabla del RIPS
    no son el mismo conjunto. Devolver un código inventado sí sería un fallo.
    """
    limpio = codigo_oms.strip().upper().replace(".", "")
    if not limpio:
        return None
    if limpio in validos:
        return limpio
    if len(limpio) == 3 and f"{limpio}X" in validos:
        return f"{limpio}X"
    return None


def _leer(nombre: str) -> list[dict]:
    with open(TABLAS / nombre, encoding="utf-8-sig") as fh:
        return list(csv.DictReader(fh, delimiter="\t"))


def _columna(fila: dict, *candidatas: str) -> str:
    """Las dos tablas nombran las columnas distinto («Foundation URI» con y sin
    espacio, `Depth` repetido). Se busca por varios nombres en vez de fijar uno.
    """
    for c in candidatas:
        if c in fila:
            return (fila[c] or "").strip()
    raise KeyError(f"ninguna de {candidatas} está en {list(fila)}")


def construir() -> dict:
    validos = codigos_rips()
    destinos: dict[str, dict[str, dict]] = defaultdict(dict)
    #: Lo mismo pero indexado por la categoría de tres caracteres TAL CUAL la
    #: escribe la OMS, sin pasar por el filtro del RIPS. Alimenta la cascada:
    #: `C83` no es un código válido del RIPS —tiene subdivisiones— pero sí es la
    #: categoría de `C832`, que sí lo es y no tiene fila propia en las tablas.
    por_categoria: dict[str, dict[str, dict]] = defaultdict(dict)
    release = ""
    sin_equivalente_rips: set[str] = set()

    for nombre in (UNICO, MULTIPLE):
        for fila in _leer(nombre):
            if _columna(fila, "10ClassKind").lower() != "category":
                continue  # capítulos y bloques no llevan código
            oms = _columna(fila, "icd10Code")
            if not oms:
                continue
            c11 = _columna(fila, "icd11Code")
            if not c11 or c11 == "No Mapping":
                continue
            uri = _columna(fila, "ICD-11 FoundationURI", "ICD-11 Foundation URI")
            release = release or _columna(
                fila, "Linearization (releaseURI)", "Linearization (release) URI")
            destino = {
                "codigo": c11,
                "uri": uri,
                "titulo": _columna(fila, "icd11Title"),
                "capitulo": _columna(fila, "icd11Chapter"),
            }
            if len(oms.replace(".", "")) == 3:
                por_categoria[oms.replace(".", "").upper()][c11] = destino

            rips = a_rips(oms, validos)
            if rips is None:
                sin_equivalente_rips.add(oms)
                continue
            # Clave por código CIE-11: las dos tablas repiten el destino único.
            destinos[rips][c11] = destino

    mapa = {
        rips: {"nivel": "exacto",
               "destinos": sorted(d.values(), key=lambda x: x["codigo"])}
        for rips, d in sorted(destinos.items())
    }

    # --- Cascada: la categoría cuando no hay equivalencia exacta -----------
    #
    # La CIE-10 de la OMS y la tabla del RIPS no subdividen igual: 3.237 códigos
    # colombianos de cuatro caracteres no tienen fila propia en las tablas. Casi
    # todos SÍ tienen categoría (`C832` → `C83`), y esa categoría sí está
    # mapeada.
    #
    # Se guarda como `nivel: "categoria"` y nunca se confirma solo: es un
    # candidato menos específico, no una equivalencia. La alternativa —dejar el
    # 26 % del catálogo sin ninguna vía— empuja a codificar a mano bajo presión
    # de tiempo, que es como se llega otra vez al dato inventado.
    for codigo in sorted(validos):
        if codigo in mapa:
            continue
        padre = por_categoria.get(codigo[:3])
        if not padre:
            continue
        mapa[codigo] = {
            "nivel": "categoria",
            "via": codigo[:3],
            "destinos": sorted(padre.values(), key=lambda x: x["codigo"]),
        }
    inverso: dict[str, list[str]] = defaultdict(list)
    for rips, entrada in mapa.items():
        for d in entrada["destinos"]:
            inverso[d["codigo"]].append(rips)

    return {
        "release": "/".join(release.split("/")[:7]) if release else "",
        "version_tabla": "2025-01-24",
        "fuente": "OMS, ICD-10/ICD-11 mapping tables",
        "mapa": mapa,
        "inverso": {k: sorted(v) for k, v in sorted(inverso.items())},
        "_sin_equivalente_rips": sorted(sin_equivalente_rips),
    }


def para_rust(datos: dict) -> dict:
    """La misma tabla, sin lo que el binario no usa.

    Se quitan tres cosas y ninguna es una decisión de contenido:

    - `uri` y `capitulo` de cada destino — el RIPS pide el código y su título,
      y la pantalla de captura ofrecerá lo mismo. La URI de la OMS no entra en
      ningún campo de la 948.
    - `inverso` — el índice CIE-11 → CIE-10 solo lo usa el análisis en Python.
    - `_sin_equivalente_rips` — es un informe de la generación, no un dato.

    Lo que NO se toca es el conjunto de códigos ni el `nivel`: quitar entradas
    haría que el binario derivara distinto que el Python, que es exactamente lo
    que esta función existe para impedir. De 3,3 MB baja a 2,0 MB.
    """
    return {
        "release": datos["release"],
        "version_tabla": datos["version_tabla"],
        "fuente": datos["fuente"],
        "mapa": {
            codigo: {
                "nivel": e["nivel"],
                **({"via": e["via"]} if e.get("via") else {}),
                "destinos": [{"codigo": d["codigo"], "titulo": d["titulo"]}
                             for d in e["destinos"]],
            }
            for codigo, e in datos["mapa"].items()
        },
    }


def main() -> int:
    datos = construir()
    mapa = datos["mapa"]
    if len(mapa) < 11_500:
        print(f"ERROR: solo se mapearon {len(mapa)} códigos", file=sys.stderr)
        return 1
    SALIDA.write_text(json.dumps(datos, ensure_ascii=False), encoding="utf-8")
    SALIDA_RUST.parent.mkdir(parents=True, exist_ok=True)
    SALIDA_RUST.write_text(json.dumps(para_rust(datos), ensure_ascii=False),
                           encoding="utf-8")

    total_rips = len(codigos_rips())
    exactos = [e for e in mapa.values() if e["nivel"] == "exacto"]
    ambiguos = sum(1 for e in mapa.values() if len(e["destinos"]) > 1)
    print(f"{len(mapa)} de {total_rips} códigos del RIPS mapeados "
          f"({len(mapa) * 100 / total_rips:.1f} %)")
    print(f"  exactos     : {len(exactos)}")
    print(f"  por categoría: {len(mapa) - len(exactos)}")
    print(f"  ambiguos    : {ambiguos} ({ambiguos * 100 / len(mapa):.1f} %) "
          f"— estos NO se autocompletan")
    print(f"  CIE-10 de la OMS sin equivalente en la tabla RIPS: "
          f"{len(datos['_sin_equivalente_rips'])}")
    print(f"  release  : {datos['release']}")
    print(f"→ {SALIDA} ({SALIDA.stat().st_size / 1e6:.1f} MB)")
    print(f"→ {SALIDA_RUST} ({SALIDA_RUST.stat().st_size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
