"""Remisiones a especialista: qué cambios de estado tienen sentido.

Migrado de `reemision.scx` (FoxPro) y `modules/remisiones.py` (NiceGUI). Una
remisión no es una entidad propia: es una `Orden` de tipo ESPECIALISTA.

=== EL ESTADO SE PODÍA PONER EN CUALQUIER COSA ===

El módulo NiceGUI hacía esto:

    orden = session.get(Orden, orden_id)
    if orden:
        orden.estado = nuevo_estado
        session.commit()

Sin comprobar nada. Una remisión COMPLETADA volvía a PENDIENTE, y una CANCELADA
pasaba a EN PROCESO, con solo elegirlo en una lista desplegable.

En una remisión eso importa: la lista de pendientes es la que dice a quién falta
atender. Una completada que reaparece como pendiente hace que alguien la
gestione dos veces; una cancelada que revive manda a un paciente a una cita que
ya no existe. Y el registro de auditoría, que sí se guardaba, dejaba constancia
del disparate sin impedirlo.

Aquí los estados finales —COMPLETADA y CANCELADA— son finales de verdad. Si hay
que reabrir una remisión, se emite otra: queda el rastro de que hubo dos, que es
justo lo que un auditor necesita ver.

=== POR QUÉ UNA MÁQUINA DE ESTADOS Y NO UNOS `if` ===

Es la misma decisión que en `shared/formulario/estado-formulario.ts` del
frontend: las transiciones válidas en un solo sitio, en una tabla que se lee de
un vistazo. Repartidas en condicionales por el router, se desincronizan con lo
que la pantalla ofrece y acaban permitiendo lo que la interfaz decía impedir.
"""

from __future__ import annotations

from typing import Optional

from models import EstadoOrden

#: Adónde puede ir cada estado. Lo que no está aquí, no se puede.
TRANSICIONES: dict[EstadoOrden, frozenset[EstadoOrden]] = {
    EstadoOrden.PENDIENTE: frozenset({
        EstadoOrden.EN_PROCESO,
        EstadoOrden.COMPLETADA,   # se atendió sin pasar por «en proceso»
        EstadoOrden.CANCELADA,
    }),
    EstadoOrden.EN_PROCESO: frozenset({
        EstadoOrden.COMPLETADA,
        EstadoOrden.CANCELADA,
    }),
    #: Finales. Para reabrir se emite una remisión nueva, y así queda el rastro
    #: de que hubo dos.
    EstadoOrden.COMPLETADA: frozenset(),
    EstadoOrden.CANCELADA: frozenset(),
}

#: Los que ya no admiten cambios.
FINALES: frozenset[EstadoOrden] = frozenset(
    e for e, destinos in TRANSICIONES.items() if not destinos
)

#: Las que siguen abiertas y cuentan como carga pendiente.
ABIERTAS: frozenset[EstadoOrden] = frozenset(
    e for e in EstadoOrden if e not in FINALES
)


class TransicionNoValida(ValueError):
    """El cambio de estado pedido no tiene sentido para esa remisión."""


def puede_pasar(desde: EstadoOrden, hasta: EstadoOrden) -> bool:
    return hasta in TRANSICIONES.get(desde, frozenset())


def destinos(desde: EstadoOrden) -> list[EstadoOrden]:
    """Los estados a los que se puede ir, para que la pantalla no ofrezca más.

    Ordenados como en la máquina, no alfabéticamente: la interfaz debe
    presentar primero el camino normal.
    """
    orden = [EstadoOrden.EN_PROCESO, EstadoOrden.COMPLETADA, EstadoOrden.CANCELADA]
    permitidos = TRANSICIONES.get(desde, frozenset())
    return [e for e in orden if e in permitidos]


def validar_cambio(desde: EstadoOrden, hasta: EstadoOrden) -> EstadoOrden:
    """Devuelve el estado nuevo, o levanta explicando por qué no."""
    if desde == hasta:
        raise TransicionNoValida(
            f"La remisión ya está en «{desde.value}»."
        )
    if desde in FINALES:
        raise TransicionNoValida(
            f"La remisión está {desde.value.lower()} y eso no se deshace. "
            f"Si hay que retomarla, emita una remisión nueva: así queda "
            f"constancia de que fueron dos y no de que una cambió de opinión."
        )
    if not puede_pasar(desde, hasta):
        posibles = ", ".join(e.value for e in destinos(desde)) or "ninguno"
        raise TransicionNoValida(
            f"No se puede pasar de «{desde.value}» a «{hasta.value}». "
            f"Desde «{desde.value}» solo cabe: {posibles}."
        )
    return hasta


def esta_abierta(estado: Optional[EstadoOrden]) -> bool:
    """Si cuenta como carga pendiente para el prestador."""
    return estado in ABIERTAS if estado is not None else False
