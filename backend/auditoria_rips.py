"""Qué le falta a la historia clínica para poder reportarse.

Un generador de RIPS que rellena huecos con constantes siempre produce un
archivo. El archivo pasa la validación de estructura, y el problema aparece
después: como glosa de la EPS, como rechazo del MUV, o —peor— como un pago que
nunca llega porque el valor del servicio iba en cero.

Este módulo mira la historia ANTES de generar y dice qué atenciones van
completas y cuáles con un supuesto. La diferencia práctica: el usuario corrige
doce consultas hoy, en vez de descubrir dentro de un mes que reportó mil.

=== POR QUÉ CADA CAMPO IMPORTA ===

`valor_servicio` es el más grave: iba QUEMADO EN CERO para todo paciente. Es el
valor de lo prestado y de él depende el reconocimiento del pago.

`modalidad_grupo_servicio` y `grupo_servicios` iban en "01" —intramural,
consulta externa—, que para un consultorio es casi siempre cierto y por eso el
problema no se veía. Para una IPS con urgencias u hospitalización es falso, y el
sistema se está ampliando justamente a eso.

`cod_consulta_cups` cae por defecto en 890201, «consulta de PRIMERA VEZ». Si no
se distingue del control (890301), todas las atenciones de un paciente crónico
se reportan como primeras veces. Es un patrón que el validador detecta.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Sequence

import procedencia as proc

#: Campos exigidos por el RIPS, con el valor que el generador supone si faltan.
#: `critico` marca aquellos cuyo supuesto tiene consecuencia económica o
#: clínica, no solo de forma.
CAMPOS = (
    # atributo            etiqueta                       supuesto  crítico
    ("valor_servicio",    "valor del servicio",           0,        True),
    ("cod_servicio",      "código de servicio",           328,      True),
    ("cod_consulta_cups", "CUPS de la consulta",          "890201", True),
    ("modalidad_grupo_servicio", "modalidad del servicio", "01",    False),
    ("grupo_servicios",   "grupo de servicios",           "01",     False),
    ("concepto_recaudo",  "concepto de recaudo",          "05",     False),
)


@dataclass
class Hueco:
    """Una atención a la que le falta un dato que el RIPS exige."""

    historia_id: Any
    fecha: Any
    paciente: str
    campo: str
    etiqueta: str
    supuesto: Any
    critico: bool


@dataclass
class Auditoria:
    total: int = 0
    completas: int = 0
    huecos: List[Hueco] = field(default_factory=list)
    #: Campos que SÍ traen valor pero que nadie afirmó: no tienen procedencia
    #: registrada. Son los defectos heredados —el `"890201"` de toda consulta—
    #: y no se veían, porque la columna nunca estaba vacía.
    sin_respaldo: List[Hueco] = field(default_factory=list)

    @property
    def incompletas(self) -> int:
        return self.total - self.completas

    @property
    def criticos(self) -> List[Hueco]:
        return [h for h in self.huecos if h.critico]

    @property
    def reportable_sin_supuestos(self) -> bool:
        return self.total > 0 and not self.huecos and not self.sin_respaldo

    def por_campo(self) -> Dict[str, int]:
        conteo: Dict[str, int] = {}
        for h in self.huecos:
            conteo[h.etiqueta] = conteo.get(h.etiqueta, 0) + 1
        return dict(sorted(conteo.items(), key=lambda kv: -kv[1]))

    def __str__(self) -> str:
        if self.total == 0:
            return "No hay atenciones en el periodo."
        lineas = [
            f"{self.completas} de {self.total} atenciones completas "
            f"({100 * self.completas / self.total:.0f} %)"
        ]
        if self.huecos:
            lineas.append(f"{len(self.huecos)} dato(s) se reportarían por defecto:")
            for etiqueta, n in self.por_campo().items():
                critico = any(h.critico for h in self.huecos if h.etiqueta == etiqueta)
                lineas.append(f"  {'⚠ ' if critico else '  '}{etiqueta}: {n}")
        if self.sin_respaldo:
            lineas.append(
                f"{len(self.sin_respaldo)} dato(s) tienen valor pero nadie los "
                f"afirmó: vienen de un valor por defecto, no de la atención."
            )
        if self.criticos:
            lineas.append(
                "Los marcados afectan el pago o la clasificación clínica: "
                "conviene corregirlos antes de enviar."
            )
        return "\n".join(lineas)


def auditar(atenciones: Sequence[Any]) -> Auditoria:
    """Revisa las atenciones y reporta qué se enviaría supuesto.

    No bloquea nada: decide el usuario. Un consultorio que siempre cobra lo
    mismo puede aceptar el valor por defecto a sabiendas; lo que no debe pasar
    es que no lo sepa.
    """
    resultado = Auditoria()

    for a in atenciones:
        resultado.total += 1
        paciente = getattr(a, "paciente", None)
        identificacion = getattr(paciente, "identificacion", "") if paciente else ""
        completa = True

        for atributo, etiqueta, supuesto, critico in CAMPOS:
            def _hueco():
                return Hueco(
                    historia_id=getattr(a, "id", None),
                    fecha=getattr(a, "fecha", None),
                    paciente=identificacion,
                    campo=atributo,
                    etiqueta=etiqueta,
                    supuesto=supuesto,
                    critico=critico,
                )

            if atributo in proc.sin_respaldo(a, [atributo]):
                # Hay dato, pero nadie dice de dónde salió. Es la firma de un
                # valor por defecto, y antes era invisible: la columna estaba
                # llena y la auditoría solo buscaba columnas vacías.
                completa = False
                resultado.sin_respaldo.append(_hueco())
                continue

            valor = getattr(a, atributo, None)
            if valor in (None, "", 0):
                completa = False
                resultado.huecos.append(
                    Hueco(
                        historia_id=getattr(a, "id", None),
                        fecha=getattr(a, "fecha", None),
                        paciente=identificacion,
                        campo=atributo,
                        etiqueta=etiqueta,
                        supuesto=supuesto,
                        critico=critico,
                    )
                )

        if completa:
            resultado.completas += 1

    return resultado
