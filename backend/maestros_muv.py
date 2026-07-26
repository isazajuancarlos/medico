"""Los catálogos de referencia del MUV, tal como el validador los usa.

Se generan con `herramientas/extraer_maestros_muv.py` desde el contenedor del
validador —la misma fuente contra la que el MUV juzga— y se versionan con el
código, igual que DIVIPOLA y el CUM.

=== POR QUÉ EXISTE ESTE MÓDULO ===

Porque el 2026-07-26 escribí a mano un mapa de presentaciones con códigos
inventados: `tableta = 1`, cuando el catálogo dice 66 y el 1 es AMPOLLA. El MUV
lo ACEPTÓ y devolvió CUV, porque valida ese campo como rango —«valores
permitidos entre 1 y 999»— y no contra la tabla.

La diferencia está en cómo redacta la queja, y conviene tenerla presente al
leer un rechazo:

    «valores permitidos entre 1 y 999»            -> rango, no comprueba nada más
    «no es válido, verificar tabla de referencia» -> sí contrasta

Un campo del primer tipo acepta cualquier número. Eso no lo vuelve opcional: lo
vuelve el sitio más fácil para que se cuele un dato falso sin que nadie se
entere. Por eso aquí no se escribe ningún código a mano.
"""
from __future__ import annotations

import gzip
import json
import unicodedata
from functools import lru_cache
from pathlib import Path
from typing import Optional

RUTA = Path(__file__).resolve().parent / "datos" / "maestros_muv"


class MaestroAusente(RuntimeError):
    """Falta un catálogo. No se sustituye por uno escrito a mano."""


@lru_cache(maxsize=None)
def cargar(nombre: str) -> dict:
    plano = RUTA / f"{nombre}.json"
    comprimido = RUTA / f"{nombre}.json.gz"
    if plano.exists():
        return json.loads(plano.read_text(encoding="utf-8"))
    if comprimido.exists():
        with gzip.open(comprimido, "rt", encoding="utf-8") as f:
            return json.load(f)
    raise MaestroAusente(
        f"falta el maestro «{nombre}» en {RUTA}. Se genera con "
        f"`python herramientas/extraer_maestros_muv.py {nombre.upper()}` "
        f"con el contenedor del validador levantado.")


def _normalizar(texto) -> str:
    """Sin tildes, sin espacios de más, en mayúsculas.

    El catálogo escribe «DILUCION HOMEOPATICA» sin tilde y el médico escribirá
    «dilución». Comparar en crudo haría fallar la traducción por un acento.
    """
    if texto is None:
        return ""
    plano = unicodedata.normalize("NFKD", str(texto))
    plano = "".join(c for c in plano if not unicodedata.combining(c))
    return " ".join(plano.upper().split())


@lru_cache(maxsize=1)
def presentaciones() -> dict[str, int]:
    """Nombre normalizado -> código de unidad mínima de dispensación (M13).

    Incluye el plural de cada nombre porque el médico escribe «tabletas» y el
    catálogo dice «TABLETA». Es una tolerancia de escritura, no una traducción
    por parecido: «gotas» y «gotero» siguen siendo cosas distintas y solo entra
    la que el catálogo nombra.
    """
    mapa: dict[str, int] = {}
    for fila in cargar("upr")["filas"]:
        nombre = _normalizar(fila["Nombre"])
        codigo = int(fila["Codigo"])
        mapa[nombre] = codigo
        if not nombre.endswith("S"):
            mapa.setdefault(f"{nombre}S", codigo)
    return mapa


def codigo_presentacion(unidad) -> Optional[int]:
    """El código de la presentación, o `None` si el catálogo no la tiene.

    `None` no es un fallo del que llama: es la respuesta correcta cuando lo
    escrito no corresponde a ninguna presentación conocida. Quien llama lo
    anota como falta; nunca se devuelve un código «parecido».
    """
    return presentaciones().get(_normalizar(unidad))


def nombres_presentacion() -> list[dict]:
    """Las presentaciones para elegir en un formulario, sin los plurales."""
    return sorted(
        ({"codigo": int(f["Codigo"]), "nombre": f["Nombre"]}
         for f in cargar("upr")["filas"]),
        key=lambda d: d["nombre"],
    )
