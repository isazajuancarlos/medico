"""Contrarreferencia: lo que devuelve el especialista, y el cierre del circuito.

Migrado de `modules/contrarreferencia.py` (NiceGUI).

=== EL CIRCUITO NUNCA SE CERRABA ===

Una remisión y su contrarreferencia son las dos mitades de lo mismo: se manda al
paciente a un especialista y el especialista devuelve qué encontró y qué hizo.
El módulo anterior las guardaba como dos registros sin relación operativa: se
podía crear la contrarreferencia y **la remisión seguía en PENDIENTE**.

El efecto es el de siempre en este sistema: la lista de pendientes deja de
significar lo que dice. Alguien la revisa para saber a quién falta gestionar y
encuentra remisiones que ya volvieron hace meses. Cuando una lista miente lo
suficiente, se deja de mirar, y entonces las que sí están pendientes tampoco se
ven.

Aquí, registrar la contrarreferencia CIERRA la remisión asociada, usando la
misma máquina de estados de `remisiones.py`. Si la remisión ya estaba cerrada,
no se fuerza nada: se informa y la contrarreferencia se guarda igual, porque el
documento clínico existe con independencia de en qué estado quedó el trámite.

=== TAMPOCO SE PODÍA ELEGIR BIEN CONTRA QUÉ ===

El original ofrecía las últimas treinta remisiones, sin filtrar por estado: se
podía contrarreferir contra una cancelada. Aquí solo se ofrecen las abiertas.

=== RECIBIR NO ES LO MISMO QUE EXISTIR ===

`recibida` es que la institución de origen acusó recibo. Es un acto distinto de
crear el documento, lo hace otra persona y en otro momento, y por eso tiene su
propia fecha y su propio responsable. No se marca solo.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Optional

import remisiones
from models import EstadoOrden


class YaRecibida(ValueError):
    """El acuse de recibo ya estaba registrado."""


@dataclass(frozen=True)
class Cierre:
    """Qué pasó con la remisión al registrar la contrarreferencia."""

    cerrada: bool
    estado_anterior: Optional[EstadoOrden]
    motivo: str

    def __str__(self) -> str:
        return self.motivo


def cerrar_remision(orden: Any) -> Cierre:
    """Marca la remisión como completada porque el paciente ya volvió.

    No levanta si no se puede: la contrarreferencia es un documento clínico y
    tiene que poder guardarse aunque el trámite de la remisión esté en un
    estado raro. Lo que se hace es DECIRLO, no silenciarlo.
    """
    if orden is None:
        return Cierre(False, None, "La contrarreferencia no se asoció a ninguna remisión.")

    anterior = orden.estado
    if not remisiones.puede_pasar(anterior, EstadoOrden.COMPLETADA):
        return Cierre(
            False, anterior,
            f"La remisión ya estaba en «{anterior.value}» y no se modificó. "
            f"La contrarreferencia se guardó igual.",
        )
    orden.estado = EstadoOrden.COMPLETADA
    return Cierre(True, anterior, "La remisión quedó completada.")


def marcar_recibida(cr: Any, recibida_por_id: int,
                    cuando: Optional[datetime] = None) -> None:
    """Acusa recibo en la institución de origen.

    Se exige quién la recibió: un acuse sin responsable no sirve para nada, que
    es justo lo que hay que poder demostrar si el paciente reclama que nadie le
    dio continuidad.
    """
    if getattr(cr, "recibida", False):
        raise YaRecibida(
            f"Esta contrarreferencia ya se registró como recibida el "
            f"{getattr(cr, 'fecha_recepcion', '—')}. No se sobrescribe: el "
            f"primer acuse es el que cuenta."
        )
    cr.recibida = True
    cr.fecha_recepcion = cuando or datetime.now()
    cr.recibida_por_id = recibida_por_id


def pendiente_de_recibir(cr: Any) -> bool:
    return not getattr(cr, "recibida", False)


def requiere_control(cr: Any) -> bool:
    """Si el especialista pidió seguimiento, alguien tiene que agendarlo."""
    return bool(getattr(cr, "requiere_seguimiento", False))
