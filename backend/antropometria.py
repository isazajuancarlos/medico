"""Peso y talla ideales, tal como los calcula el sistema original.

Migrado literal de `calcularpesoideal` y `calculartallaideal` del formulario
`exa_fisico_gen` del sistema en Visual FoxPro. No se elige convención clínica ni
se "mejora" nada: son las fórmulas que el prestador viene usando con sus
pacientes, y cambiarlas cambiaría números que él ya conoce.

=== PESO IDEAL ===

    mayores de 12 años:   talla² × 0.23   hombres
                          talla² × 0.22   mujeres

Equivale a fijar un IMC objetivo de 23 en hombres y 22 en mujeres, que está
dentro del rango normal de la OMS (18,5–24,9).

    12 años o menos:      NO CALCULA

En el original, la rama que resolvía la edad pediátrica contra la tabla
`pesotalla` está COMENTADA en el código activo, así que devuelve cero. Se migra
así por fidelidad. ES CANDIDATO CLARO PARA LA ETAPA DE MEJORAS: un consultorio
que atiende niños se queda sin referencia justo donde más se usa. No se toca
ahora para no mezclar migración con criterio clínico nuevo.

=== TALLA IDEAL ===

No es una fórmula: se busca en la tabla de referencia `pesotalla`, indexada por
edad + unidad de medida + sexo. Hace falta esa tabla para migrarlo; mientras no
esté cargada, esta función lo dice en vez de inventar un número.
"""

from __future__ import annotations

from typing import Optional

#: Factores del original. IMC objetivo por sexo.
FACTOR_HOMBRE = 0.23
FACTOR_MUJER = 0.22

#: Por debajo de esta edad el original no calcula peso ideal.
EDAD_MINIMA_PESO_IDEAL = 12


def peso_ideal(talla_metros: float, sexo: str, edad_años: int) -> Optional[float]:
    """Peso ideal en kilogramos, o None si el original no lo calcularía.

    Args:
        talla_metros: talla en METROS (1.70), no en centímetros.
        sexo: 'M' o 'F'. Cualquier otro valor se trata como femenino, igual que
            el original, que usaba `IF Lsexo='M' ... ELSE`.
        edad_años: años cumplidos.

    Devuelve None —no 0— cuando no aplica. El original devolvía 0, que en un
    campo de peso se lee como un dato real y equivocado; None obliga a quien
    llame a decidir qué mostrar.
    """
    if edad_años <= EDAD_MINIMA_PESO_IDEAL:
        return None
    if talla_metros is None or talla_metros <= 0:
        return None

    factor = FACTOR_HOMBRE if (sexo or "").strip().upper().startswith("M") else FACTOR_MUJER
    return round(talla_metros ** 2 * factor * 100, 1)


def talla_ideal(edad_años: int, sexo: str, tabla_pesotalla=None) -> Optional[float]:
    """Talla ideal desde la tabla de referencia del original.

    Mientras la tabla `pesotalla` no esté cargada devuelve None. Es a propósito:
    inventar una fórmula aquí produciría un número plausible y falso, que es
    peor que no dar ninguno.
    """
    if not tabla_pesotalla:
        return None
    clave = (edad_años, (sexo or "").strip().upper())
    return tabla_pesotalla.get(clave)


def imc(peso_kg: float, talla_metros: float) -> Optional[float]:
    """Índice de masa corporal. Lo trae el modelo Historia como campo propio."""
    if not peso_kg or not talla_metros or talla_metros <= 0:
        return None
    return round(peso_kg / (talla_metros ** 2), 1)
