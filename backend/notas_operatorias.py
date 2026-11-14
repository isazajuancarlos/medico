"""Nota operatoria: el tiempo quirúrgico y el estado de salida.

Migrado de `modules/nota_operatoria.py` (NiceGUI). El modelo se conserva; lo que
se añade es que alguien compruebe lo que dice.

=== EL TIEMPO QUIRÚRGICO SE CALCULA, NO SE ESCRIBE ===

`ProtocoloQuirurgico` guarda `hora_inicio`, `hora_fin` **y**
`tiempo_quirurgico_min`. Tres campos para dos datos. Escritos a mano por
separado, acaban discrepando: la nota dice 90 minutos y las horas dicen 150, y
el indicador de tiempo quirúrgico promedio —que es de los que mira una auditoría
de calidad— sale de uno de los dos sin saber cuál.

Aquí se deriva de las horas. Si además llega escrito, se compara y se avisa: no
se pisa en silencio, porque puede ser que las horas estén mal y no el tiempo.

=== UNA CIRUGÍA PUEDE CRUZAR LA MEDIANOCHE ===

Es la razón de que las horas sean `datetime` y no `time`, y hay que no
estropearlo: una cirugía que empieza a las 23:40 y termina a las 01:20 dura 100
minutos, no menos veintidós horas. Con `time` no habría forma de saberlo.

=== «ESTABLE» NO PUEDE SER EL VALOR POR DEFECTO ===

`estado_paciente_salida` venía con `default="ESTABLE"`. Es una afirmación
clínica sobre cómo salió el paciente de quirófano, y estaba puesta de antemano:
si nadie la registraba, la nota decía que salió estable. Nadie firmó eso.

Es el mismo patrón que ya apareció con el CUPS de la consulta y con el municipio
de residencia, pero aquí lo que se inventa es un hecho clínico, y es el que se
mira primero cuando algo sale mal.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Optional

#: Estados de salida admitidos. No hay ninguno «por defecto» a propósito.
ESTADOS_SALIDA = ("ESTABLE", "INESTABLE", "CRITICO", "FALLECIDO")

#: Cuánto se tolera entre el tiempo escrito y el calculado antes de avisar.
TOLERANCIA_MIN = 1


class TiemposNoValidos(ValueError):
    """Las horas de la cirugía no describen un intervalo posible."""


class EstadoNoValido(ValueError):
    """El estado de salida no está registrado o no es uno de los admitidos."""


@dataclass(frozen=True)
class Tiempo:
    minutos: int
    #: Lo que venía escrito, si venía y no coincide.
    discrepancia: Optional[int] = None

    @property
    def horas(self) -> float:
        return round(self.minutos / 60, 2)


def duracion(inicio: datetime, fin: datetime,
             escrito: Optional[int] = None) -> Tiempo:
    """Minutos de cirugía, a partir de las horas.

    Aguanta que cruce la medianoche porque las horas son `datetime` completos.
    Lo que no aguanta es que el fin sea anterior al inicio: eso no es una
    cirugía larga, es un dato mal capturado.
    """
    if fin < inicio:
        raise TiemposNoValidos(
            f"La cirugía no puede terminar ({fin:%Y-%m-%d %H:%M}) antes de "
            f"empezar ({inicio:%Y-%m-%d %H:%M}). Revise la fecha, no solo la "
            f"hora: si cruzó la medianoche, el fin es del día siguiente."
        )
    minutos = int((fin - inicio).total_seconds() // 60)
    if minutos == 0:
        raise TiemposNoValidos(
            "El inicio y el fin son la misma hora. Una cirugía de cero minutos "
            "no se registra: si se canceló, no hay nota operatoria."
        )

    discrepancia = None
    if escrito is not None and abs(escrito - minutos) > TOLERANCIA_MIN:
        discrepancia = escrito
    return Tiempo(minutos=minutos, discrepancia=discrepancia)


def validar_estado_salida(estado: Optional[str]) -> str:
    """Exige que alguien haya dicho cómo salió el paciente."""
    if not (estado or "").strip():
        raise EstadoNoValido(
            "Registre cómo salió el paciente de quirófano. No hay valor por "
            "defecto: decir «estable» sin que nadie lo haya afirmado es poner "
            "en la historia clínica un hecho que no ocurrió."
        )
    normalizado = estado.strip().upper()
    if normalizado not in ESTADOS_SALIDA:
        raise EstadoNoValido(
            f"«{estado}» no es un estado de salida válido. "
            f"Use uno de: {', '.join(ESTADOS_SALIDA)}."
        )
    return normalizado


@dataclass(frozen=True)
class Avisos:
    """Lo que conviene mirar antes de dar la nota por buena. No bloquea."""

    mensajes: tuple[str, ...]

    def __bool__(self) -> bool:
        return bool(self.mensajes)

    def __str__(self) -> str:
        return " ".join(self.mensajes) if self.mensajes else "Sin observaciones."


def revisar(nota: Any) -> Avisos:
    """Señala lo que merece una segunda mirada, sin impedir guardar.

    Son cosas posibles pero infrecuentes. Bloquearlas sería peor: en cirugía lo
    infrecuente pasa, y un sistema que no deja registrar lo que de verdad
    ocurrió obliga a mentirle.
    """
    avisos: list[str] = []

    t = duracion(nota.hora_inicio, nota.hora_fin,
                 getattr(nota, "tiempo_quirurgico_min", None))
    if t.discrepancia is not None:
        avisos.append(
            f"El tiempo anotado ({t.discrepancia} min) no coincide con el que "
            f"resulta de las horas ({t.minutos} min)."
        )
    if t.minutos > 12 * 60:
        avisos.append(f"La cirugía figura con {t.horas} horas de duración.")

    sangrado = getattr(nota, "sangrado_estimado_ml", None)
    if sangrado is None:
        avisos.append("No se registró el sangrado estimado.")
    elif sangrado > 1000:
        avisos.append(f"Sangrado estimado de {sangrado} ml.")

    if getattr(nota, "complicaciones", None):
        avisos.append("La nota registra complicaciones.")

    if (getattr(nota, "estado_paciente_salida", "") or "").upper() != "ESTABLE":
        avisos.append(
            f"El paciente salió «{nota.estado_paciente_salida}» de quirófano."
        )

    if not getattr(nota, "firma_cirujano_path", None):
        avisos.append("Falta la firma del cirujano.")

    return Avisos(tuple(avisos))
