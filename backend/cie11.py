"""CIE-11: derivar del CIE-10 lo que se pueda, proponer el resto, inventar nada.

La Resolución 948 de 2026 abre el periodo en que un mismo episodio lleva los dos
códigos. Pedirle al médico que codifique dos veces es multiplicar por dos el
problema que este sistema existe para no repetir, así que:

    equivalencia exacta y única  → se DERIVA sin preguntar
    equivalencia por categoría   → se PROPONE, menos específica
    varios destinos              → se PROPONEN todos, no se elige
    sin equivalencia             → se dice, y el campo queda vacío

Ver [[procedencia]]: cada una de esas cuatro deja una marca distinta, y por eso
la auditoría puede después separar «lo derivó el sistema» de «lo afirmó alguien».

=== POR QUÉ NO SE ELIGE ENTRE VARIOS ===

Medido sobre las tablas oficiales: **1.402 códigos (11,3 %) tienen más de un
destino en CIE-11**, hasta 42. La OMS lo advierte en su readme: «ICD-10 concepts
may no longer have a unique code in ICD-11».

Poner el primero de una lista de 42 tiene exactamente la misma naturaleza que
`cod_consulta_cups="890201"`: un dato con la forma correcta que nadie afirmó.
Que lo elija un algoritmo en vez de una constante no lo mejora.

=== LA URI, NO SOLO EL CÓDIGO ===

Se guarda también la `FoundationURI` de la OMS, que es el identificador que NO
cambia entre versiones de la clasificación. El código sí cambia. Un registro que
solo guarde el código deja de ser interpretable cuando la OMS publique la
siguiente revisión; con la URI se puede volver a resolver. Es lo que pide la
guía de transición de la OMS en su §2.4.8.
"""

from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path
from typing import Any, NamedTuple, Optional

import procedencia as proc

RUTA = Path(__file__).resolve().parent / "datos" / "mapeo_cie10_cie11.json"


class Destino(NamedTuple):
    """Un candidato CIE-11."""

    codigo: str
    uri: str
    titulo: str
    capitulo: str


class Equivalencia(NamedTuple):
    cie10: str
    destinos: tuple[Destino, ...]
    #: `"exacto"` si la tabla de la OMS trae ese código; `"categoria"` si lo que
    #: se encontró fue su categoría de tres caracteres.
    nivel: str
    #: La categoría por la que se llegó, cuando `nivel == "categoria"`.
    via: Optional[str] = None

    @property
    def ambigua(self) -> bool:
        return len(self.destinos) > 1

    @property
    def derivable(self) -> bool:
        """Si se puede poner sin preguntar.

        Exige las dos cosas: destino único Y equivalencia exacta. Una categoría
        con destino único sigue siendo menos específica que el diagnóstico que
        el médico escribió, y rebajar la especificidad en silencio es perder un
        dato clínico sin que nadie se entere.
        """
        return self.nivel == "exacto" and len(self.destinos) == 1


@lru_cache(maxsize=1)
def _cargar() -> dict:
    with open(RUTA, encoding="utf-8") as f:
        return json.load(f)


def release() -> str:
    """La versión de la clasificación contra la que se generó el mapeo."""
    return _cargar()["release"]


def normalizar(cie10: str | None) -> str:
    """Deja el código en la forma del RIPS: cuatro caracteres, sin punto.

    Acepta lo que llega de fuera —`a00.0`, `A00.0`, `A09`— porque un código
    copiado de una remisión en papel o de otro sistema viene como venga. Lo que
    NO hace es completar a cuatro con `X` por su cuenta: eso solo es correcto
    para las categorías sin subdivisión, y decidirlo aquí, sin la tabla delante,
    fabricaría códigos inexistentes.
    """
    if not cie10:
        return ""
    return cie10.strip().upper().replace(".", "")


def buscar(cie10: str | None) -> Optional[Equivalencia]:
    """La equivalencia, o `None` si el código no está mapeado."""
    codigo = normalizar(cie10)
    if not codigo:
        return None
    mapa = _cargar()["mapa"]
    entrada = mapa.get(codigo)
    if entrada is None and len(codigo) == 3:
        # Una categoría sin subdivisión se escribe `A09` fuera del RIPS y `A09X`
        # dentro. Se acepta la forma corta al consultar.
        entrada = mapa.get(f"{codigo}X")
        codigo = f"{codigo}X" if entrada else codigo
    if entrada is None:
        return None
    return Equivalencia(
        cie10=codigo,
        destinos=tuple(Destino(d["codigo"], d["uri"], d["titulo"], d["capitulo"])
                       for d in entrada["destinos"]),
        nivel=entrada["nivel"],
        via=entrada.get("via"),
    )


def candidatos(cie10: str | None) -> list[Destino]:
    """Los CIE-11 posibles, para que el formulario los ofrezca."""
    eq = buscar(cie10)
    return list(eq.destinos) if eq else []


def derivar(cie10: str | None) -> Optional[Destino]:
    """El CIE-11 cuando la equivalencia es exacta y única. Si no, `None`."""
    eq = buscar(cie10)
    return eq.destinos[0] if eq and eq.derivable else None


def inverso(cie11: str | None) -> list[str]:
    """Los CIE-10 del RIPS que llevan a ese CIE-11.

    Hace falta para leer lo que venga codificado en CIE-11 de otra institución y
    poder seguir facturando en CIE-10 durante el periodo paralelo.
    """
    codigo = (cie11 or "").strip().upper()
    return list(_cargar()["inverso"].get(codigo, []))


# --- Escritura sobre el registro -----------------------------------------

def completar(
    obj: Any,
    *,
    campo_cie10: str = "diagnostico_principal",
    campo_cie11: str = "diagnostico_principal_cie11",
    campo_uri: str = "diagnostico_principal_uri",
) -> Optional[Equivalencia]:
    """Rellena el CIE-11 desde el CIE-10 y deja constancia de cómo.

    Solo escribe cuando la equivalencia es derivable, y **nunca pisa un valor
    que alguien haya confirmado**: si el médico eligió un CIE-11 distinto del
    que sale de la tabla, el que manda es el suyo. La tabla es una ayuda, no
    una autoridad sobre el criterio clínico.

    Devuelve la equivalencia encontrada —incluso cuando no escribió nada— para
    que quien llama pueda ofrecer los candidatos.
    """
    if not hasattr(obj, campo_cie11):
        # Sin la columna, `setattr` crearía un atributo suelto que nunca llega a
        # la base de datos: el sistema PARECERÍA estar derivando el CIE-11 y no
        # estaría guardando nada. Un fallo silencioso aquí se descubre cuando el
        # RIPS rebota, meses después.
        raise AttributeError(
            f"{type(obj).__name__} no tiene el campo «{campo_cie11}». "
            f"Añádelo al modelo antes de derivar el CIE-11."
        )
    eq = buscar(getattr(obj, campo_cie10, None))
    if eq is None:
        return None
    if proc.de(obj, campo_cie11) is proc.Procedencia.CONFIRMADO:
        return eq
    if not eq.derivable:
        return eq
    destino = eq.destinos[0]
    setattr(obj, campo_cie11, destino.codigo)
    if hasattr(obj, campo_uri):
        setattr(obj, campo_uri, destino.uri)
        proc.marcar(obj, campo_uri, proc.Procedencia.DERIVADO)
    proc.marcar(obj, campo_cie11, proc.Procedencia.DERIVADO)
    return eq


def total_mapeados() -> int:
    return len(_cargar()["mapa"])
