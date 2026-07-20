"""De dónde salió cada dato clínico.

=== EL PROBLEMA QUE RESUELVE ===

Un sistema de salud predetermina valores porque el clínico tiene doce minutos
por paciente. No es pereza de quien programó: es una respuesta racional a la
presión de tiempo. Mala, pero racional. El resultado es que la historia acaba
llena de datos que nadie afirmó —`cod_consulta_cups="890201"`,
`estado_paciente_salida="ESTABLE"`— y que son indistinguibles de los reales.

Quitar los defectos sin poner nada en su lugar solo hace la captura más lenta,
y lo lento se rodea: se escribe cualquier cosa con tal de cerrar la nota. Se
cambia un dato falso por uno basura, con más fricción.

La salida no es exigir más, es **saber de dónde vino cada cosa**. Un valor
derivado de otros que ya estaban no cuesta tiempo y es cierto. Uno propuesto y
confirmado con una tecla cuesta una tecla. Y uno que nadie puso ni confirmó
tiene que poder distinguirse de los dos anteriores.

=== LA PREGUNTA QUE VUELVE TRIVIAL LA AUDITORÍA ===

Hoy `auditoria_rips` busca campos vacíos contra una lista de seis nombres
escrita a mano. Por eso los defectos la dejaron ciega: el campo nunca estaba
vacío. Y por eso solo ve los seis que alguien se acordó de listar.

Con procedencia la pregunta cambia y se generaliza:

    ¿qué campos tienen valor pero NO tienen procedencia registrada?

Eso caza los defectos **de cualquier campo**, incluidos los que nadie pensó en
listar y los que se añadan mañana. Un valor sin procedencia es exactamente un
valor que nadie afirmó.

=== LO QUE NO ES ===

No es un registro de auditoría: eso ya existe en `auditoria.py` y guarda quién
tocó qué. Esto es una propiedad del DATO, no del acto. Sobrevive a la sesión, se
consulta al generar el informe y viaja con el registro.

Y no sustituye al criterio clínico. `DERIVADO` no significa correcto: significa
que se calculó de otros datos y que, si esos están mal, este también.
"""

from __future__ import annotations

from enum import Enum
from typing import Any, Iterable, Optional

#: Nombre del campo JSON donde cada modelo guarda `{campo: procedencia}`.
CAMPO = "procedencias"


class Procedencia(str, Enum):
    """Cómo llegó un valor a la historia."""

    #: Una persona lo escribió, o lo revisó y lo dio por bueno.
    CONFIRMADO = "confirmado"

    #: Se calculó de otros datos que ya estaban. No cuesta tiempo al clínico y
    #: es reproducible, pero hereda los errores de aquellos.
    DERIVADO = "derivado"

    #: El sistema lo sugirió y alguien lo aceptó. Vale más que un defecto y
    #: menos que un confirmado: hay que poder medir cuántos se aceptan sin
    #: mirar, porque si son todos esto es un defecto con pasos de más.
    PROPUESTO = "propuesto"

    #: Se preguntó y no procede. Distinto de que falte: «no se midió el
    #: sangrado» y «no sangró» no son lo mismo.
    NO_APLICA = "no_aplica"


def _mapa(obj: Any) -> dict[str, str]:
    actual = getattr(obj, CAMPO, None)
    return dict(actual) if actual else {}


def marcar(obj: Any, campo: str, como: Procedencia) -> None:
    """Deja constancia de cómo llegó el valor de `campo`.

    No toca el valor. Se llama justo donde se asigna, para que las dos cosas
    viajen juntas y no puedan desincronizarse.
    """
    mapa = _mapa(obj)
    mapa[campo] = como.value
    # Se reasigna el dict completo: SQLAlchemy no detecta la mutación en sitio
    # de una columna JSON, y el cambio se perdería en el commit sin decir nada.
    setattr(obj, CAMPO, mapa)


def marcar_varios(obj: Any, campos: Iterable[str], como: Procedencia) -> None:
    mapa = _mapa(obj)
    for c in campos:
        mapa[c] = como.value
    setattr(obj, CAMPO, mapa)


def de(obj: Any, campo: str) -> Optional[Procedencia]:
    """La procedencia registrada, o `None` si nadie la registró."""
    valor = _mapa(obj).get(campo)
    return Procedencia(valor) if valor else None


def tiene_valor(obj: Any, campo: str) -> bool:
    """Si el campo trae algo. Cero y cadena vacía cuentan como que no."""
    v = getattr(obj, campo, None)
    if v is None:
        return False
    if isinstance(v, str):
        return bool(v.strip())
    if isinstance(v, (int, float)):
        return v != 0
    return bool(v)


def sin_respaldo(obj: Any, campos: Iterable[str]) -> list[str]:
    """Campos que TIENEN valor pero NO tienen procedencia.

    Es la pregunta que caza los defectos de cualquier campo, incluidos los que
    nadie listó. Un valor sin procedencia es un valor que nadie afirmó.
    """
    return [c for c in campos if tiene_valor(obj, c) and de(obj, c) is None]


def faltantes(obj: Any, campos: Iterable[str]) -> list[str]:
    """Campos sin valor y sin un `NO_APLICA` que explique la ausencia."""
    return [
        c for c in campos
        if not tiene_valor(obj, c) and de(obj, c) is not Procedencia.NO_APLICA
    ]


def respaldados(obj: Any, campos: Iterable[str]) -> list[str]:
    """Los que tienen valor Y procedencia. Los que se pueden defender."""
    return [c for c in campos if tiene_valor(obj, c) and de(obj, c) is not None]
