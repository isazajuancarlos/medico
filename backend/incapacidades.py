"""Incapacidades médicas: el cálculo, aparte de la pantalla.

Migrado de `incapacidad.scx` del FoxPro y de `modules/incapacidad.py` de la
versión NiceGUI. En las dos, la incapacidad no es una entidad propia: cuelga de
la consulta, con `Historia.genera_incapacidad` y `Historia.dias_incapacidad`.
Se conserva así —la migración va primero, las mejoras después—, pero el cálculo
sale de la interfaz para poder probarlo.

=== EL DÍA DE MÁS ===

Las dos versiones anteriores calculaban el final así:

    fecha_fin = fecha_inicio + timedelta(days=dias)

Está corrido un día. Una incapacidad de UN día que empieza el lunes termina el
lunes, no el martes: el día de inicio ya cuenta. La fórmula correcta es
`dias - 1`.

No es una molestia de presentación. La fecha de fin es la que dice cuándo se
reincorpora el trabajador y hasta cuándo reconoce la EPS: un día de más en cada
incapacidad es dinero y es una fecha de regreso equivocada en la certificación
que firma el médico.

=== LO QUE NO SE INVENTA AQUÍ ===

El régimen colombiano tiene más reglas —los dos primeros días los paga el
empleador y desde el tercero la EPS, a los 180 días pasa a la AFP, las
prórrogas se cuentan seguidas—. Nada de eso estaba en el sistema original y no
se añade sobre la marcha: se registra como mejora. Lo que sí se hace es no
arrastrar el error de fechas.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, timedelta
from typing import Any, Iterable, Optional


class DiasNoValidos(ValueError):
    """Una incapacidad de cero o menos días no es una incapacidad."""


@dataclass(frozen=True)
class Periodo:
    """Los días que cubre una incapacidad."""

    inicio: date
    fin: date
    dias: int

    def cubre(self, dia: date) -> bool:
        return self.inicio <= dia <= self.fin


def periodo(inicio: date, dias: int) -> Periodo:
    """De fecha de inicio y número de días, al periodo cubierto.

    El día de inicio CUENTA. Una incapacidad de un día empieza y termina el
    mismo día.
    """
    if dias < 1:
        raise DiasNoValidos(
            f"Una incapacidad es de al menos un día, no de {dias}. "
            f"Si no hay incapacidad, no marque la casilla en la consulta."
        )
    return Periodo(inicio=inicio, fin=inicio + timedelta(days=dias - 1), dias=dias)


@dataclass(frozen=True)
class Resumen:
    """Lo que se muestra arriba de la lista."""

    cantidad: int
    dias_totales: int

    @property
    def promedio_dias(self) -> float:
        if self.cantidad == 0:
            return 0.0
        return round(self.dias_totales / self.cantidad, 1)

    def __str__(self) -> str:
        if self.cantidad == 0:
            return "Sin incapacidades en el periodo."
        return (
            f"{self.cantidad} incapacidades, {self.dias_totales} días en total, "
            f"{self.promedio_dias} días de promedio."
        )


def resumir(atenciones: Iterable[Any]) -> Resumen:
    """Cuenta incapacidades y días sobre las atenciones que ya vienen filtradas.

    No consulta la base a propósito: quien llama ya aplicó el filtro por
    institución y por fechas, y así esto se puede probar con objetos sueltos.
    """
    cantidad = 0
    dias = 0
    for a in atenciones:
        if not getattr(a, "genera_incapacidad", False):
            continue
        cantidad += 1
        dias += int(getattr(a, "dias_incapacidad", 0) or 0)
    return Resumen(cantidad=cantidad, dias_totales=dias)


def desde_historia(historia: Any) -> Optional[Periodo]:
    """El periodo de una consulta, o `None` si esa consulta no incapacitó.

    Devuelve `None` en vez de un periodo de cero días: «no hubo incapacidad» y
    «hubo una de cero días» son cosas distintas y no deben verse igual.
    """
    if not getattr(historia, "genera_incapacidad", False):
        return None
    dias = int(getattr(historia, "dias_incapacidad", 0) or 0)
    if dias < 1:
        return None
    fecha = getattr(historia, "fecha", None)
    if fecha is None:
        return None
    if hasattr(fecha, "date"):
        fecha = fecha.date()
    return periodo(fecha, dias)
