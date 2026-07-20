"""Consentimiento informado: cuándo vale y cuándo no.

Migrado de `modules/consentimiento.py` (NiceGUI). El modelo ya distinguía las
piezas correctas; lo que faltaba era que alguien las juzgara. La versión
anterior guardaba el registro y lo daba por bueno.

=== FIRMAR NO ES CONSENTIR ===

La distinción que sostiene toda esta figura: un papel firmado no es un
consentimiento informado. Lo que la Ley 23 de 1981 (art. 15) y la Resolución
1995 de 1999 exigen es que al paciente se le hayan explicado el procedimiento,
sus riesgos, sus beneficios, las alternativas y qué pasa si no se hace, y que
lo haya COMPRENDIDO. La firma es la prueba de eso, no el requisito.

Por eso `paciente_comprende` y `acepta_procedimiento` son campos distintos, y
por eso aquí valen los dos. Un registro con la firma puesta y `comprende` en
falso no es un consentimiento a medias: **no es un consentimiento**, y quien
opere confiando en él no tiene con qué defenderse.

=== LA REVOCACIÓN NO SE PUEDE BLOQUEAR NUNCA ===

Es el punto donde una regla de negocio mal puesta se vuelve un problema serio.
El paciente puede retirar su consentimiento en cualquier momento y sin motivo:
esa es la esencia del derecho, no un trámite. Cualquier comprobación que impida
registrar una revocación —porque el consentimiento «ya está firmado», porque
«ya se programó el procedimiento»— convierte al sistema en el instrumento de
una intervención no consentida.

Así que `revocar()` no valida nada sobre el estado previo. Solo pide el motivo,
para que quede el rastro.

=== NO SE BORRA, SE REVOCA ===

Un consentimiento revocado se conserva, marcado. Borrarlo dejaría a la
institución sin la prueba de que en su momento sí lo hubo, que es exactamente
lo que necesitará si alguien discute lo que se hizo antes de la revocación.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Optional

#: Lo que hay que haber explicado. Son los elementos que la norma enumera.
EXPLICACIONES = (
    ("descripcion_procedimiento", "en qué consiste el procedimiento"),
    ("riesgos_explicados", "los riesgos"),
    ("beneficios_esperados", "los beneficios esperados"),
    ("alternativas_explicadas", "las alternativas"),
)


class NoSePuedeRevocar(ValueError):
    """Solo por falta del motivo. Nunca por el estado del consentimiento."""


@dataclass(frozen=True)
class Validez:
    """Si un consentimiento sirve para lo que tiene que servir, y por qué no."""

    valido: bool
    faltantes: tuple[str, ...]
    revocado: bool

    def __str__(self) -> str:
        if self.revocado:
            return "Revocado por el paciente. No autoriza el procedimiento."
        if self.valido:
            return "Consentimiento informado completo."
        return "No es un consentimiento válido: falta " + ", ".join(self.faltantes) + "."


def evaluar(c: Any) -> Validez:
    """Juzga un consentimiento. No lo modifica.

    Se separa del guardado a propósito: la pantalla necesita saber qué falta
    MIENTRAS se llena, y el informe necesita saberlo después. Si el juicio
    viviera en el endpoint que guarda, habría que duplicarlo.
    """
    if getattr(c, "revocado", False):
        return Validez(valido=False, faltantes=("la revocación del paciente",),
                       revocado=True)

    faltan: list[str] = []
    for campo, etiqueta in EXPLICACIONES:
        if not (getattr(c, campo, None) or "").strip():
            faltan.append(etiqueta)

    if not getattr(c, "paciente_comprende", False):
        # El más importante y el más fácil de pasar por alto: sin esto hay una
        # firma, no un consentimiento informado.
        faltan.append("la constancia de que el paciente comprendió")
    if not getattr(c, "acepta_procedimiento", False):
        faltan.append("la aceptación del procedimiento")
    if not getattr(c, "firma_paciente_path", None):
        faltan.append("la firma del paciente")
    if not getattr(c, "firma_medico_path", None):
        faltan.append("la firma del médico")

    return Validez(valido=not faltan, faltantes=tuple(faltan), revocado=False)


def revocar(c: Any, motivo: str, cuando: Optional[datetime] = None) -> None:
    """Registra la revocación. No comprueba el estado previo, a propósito.

    El paciente puede retirar su consentimiento en cualquier momento y sin dar
    razones. Se pide el motivo únicamente para que quede constancia de lo que
    dijo, y se acepta cualquier texto: no es una justificación que la
    institución deba aprobar.
    """
    if not (motivo or "").strip():
        raise NoSePuedeRevocar(
            "Escriba lo que el paciente manifestó al revocar. No hace falta que "
            "sea un motivo válido —no tiene que darlo—, pero sí que quede "
            "constancia de que revocó y de cuándo."
        )
    c.revocado = True
    c.fecha_revocacion = cuando or datetime.now()
    c.motivo_revocacion = motivo.strip()


def requiere_testigo(c: Any) -> bool:
    """Si hace falta testigo: cuando el paciente no puede firmar por sí mismo.

    Se deduce de que no haya firma del paciente pero sí datos de testigo. No se
    infiere de la edad ni de la capacidad: eso lo decide quien atiende y lo
    registra, no una regla automática que podría equivocarse con un menor
    adulto o con alguien bajo medida de apoyo.
    """
    return bool(getattr(c, "testigo_nombre", None))
