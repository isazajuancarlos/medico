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


# ---------------------------------------------------------------------------
# Tipos de documento
#
# `REFs_TipoDoc` marca cada tipo con dos banderas: si vale para el USUARIO del
# RIPS y si vale para quien REALIZA la atención. Es la misma distinción que la
# Resolución 948 hace en los campos P11/P12 y que estaba escrita a mano tras
# leerla del PDF —correctamente, resultó—, pero aquí viene como dato.
# ---------------------------------------------------------------------------

def _nombre_de_miembro(nombre: str) -> str:
    """El texto del catálogo convertido en un identificador de Python.

    Es mecánico a propósito —mayúsculas sin tildes, lo que no sea letra o
    dígito pasa a `_`— porque el nombre lo pone el catálogo y no nosotros. Un
    nombre «mejor» escrito a mano es una segunda lista que mantener, y el día
    que el maestro cambie una fila, las dos dirán cosas distintas.
    """
    plano = _normalizar(nombre)
    ident = "".join(c if c.isalnum() else "_" for c in plano)
    ident = "_".join(p for p in ident.split("_") if p)
    return ident if ident[:1].isalpha() else f"_{ident}"


def finalidades() -> list[dict]:
    """Las finalidades de la tecnología en salud (11 a 44), de la 948."""
    return [f for f in cargar("finalidad_tec")["filas"]]


def causas_motivo() -> list[dict]:
    """Las causas o motivos de atención (21 a 49), de la 948."""
    return [f for f in cargar("causa_motivo")["filas"]]


def vias_ingreso() -> list[dict]:
    """Vía de ingreso al servicio de salud (AP y AH).

    Trae la columna `AplicaAH`, y la columna importa: el `04` («derivado de
    hospitalización») vale para procedimientos y **no** para hospitalización, y
    el generador del AH lo emitía ahí.
    """
    return [f for f in cargar("via_ingreso")["filas"]]


def condiciones_destino() -> list[dict]:
    """Condición y destino del usuario al egreso (AU, AH y AN).

    La Resolución 948 **no trae esta tabla**: su ficha remite a
    `web.sispro.gov.co`. Mientras no estuvo extraída, el generador del AU llevaba
    un mapa escrito a mano con `FALLECIDO = "04"` — y el 04 es «referido a otra
    institución»; el muerto es el 02. Un fallecimiento se reportaba como un
    traslado, y con eso se caían las dos validaciones que la norma cuelga de ese
    campo.
    """
    return [f for f in cargar("condicion_destino")["filas"]]


@lru_cache(maxsize=1)
def _tipos_documento() -> list[dict]:
    return cargar("tipo_doc")["filas"]


def tipos_documento_paciente() -> dict[str, str]:
    """Los que el MUV admite para el usuario atendido."""
    return {f["TipoDocumentoIdentificacion"]: f["DocumentoIdentificacion"]
            for f in _tipos_documento() if f["UsuarioRips"]}


def tipos_documento_profesional() -> dict[str, str]:
    """Los que el MUV admite para quien realiza la atención.

    Ninguno es de menor ni de «sin identificar»: quien atiende es un adulto
    identificado. Es la regla que el MUV aplicó al rechazarnos las consultas de
    niños, cuando el documento del paciente iba en el campo del médico.
    """
    return {f["TipoDocumentoIdentificacion"]: f["DocumentoIdentificacion"]
            for f in _tipos_documento() if f["RealizaAtencionRips"]}
