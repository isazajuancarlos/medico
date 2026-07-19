"""Peso y talla ideales: las fórmulas del sistema original, sin cambiarlas.

Juan fue explícito: «las fórmulas son las que están en el sistema». Estas
pruebas fijan exactamente eso, incluido lo que el original NO calcula.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from antropometria import (  # noqa: E402
    EDAD_MINIMA_PESO_IDEAL, imc, peso_ideal, talla_ideal,
)


# --- Peso ideal, tal cual el original ----------------------------------------

def test_hombre_adulto_usa_el_factor_023():
    # talla² × 0.23 × 100  ->  1.75² × 0.23 × 100 = 70.4
    assert peso_ideal(1.75, "M", 40) == 70.4


def test_mujer_adulta_usa_el_factor_022():
    assert peso_ideal(1.60, "F", 40) == 56.3


def test_el_factor_equivale_a_un_IMC_objetivo():
    """23 en hombres y 22 en mujeres, dentro del rango normal de la OMS."""
    assert imc(peso_ideal(1.75, "M", 40), 1.75) == 23.0
    assert imc(peso_ideal(1.60, "F", 40), 1.60) == 22.0


def test_un_sexo_no_reconocido_cae_al_femenino():
    """El original hacía IF Lsexo='M' ... ELSE, sin tercer caso."""
    assert peso_ideal(1.60, "X", 40) == peso_ideal(1.60, "F", 40)
    assert peso_ideal(1.60, "", 40) == peso_ideal(1.60, "F", 40)


def test_minusculas_tambien_cuentan_como_hombre():
    assert peso_ideal(1.75, "m", 40) == peso_ideal(1.75, "M", 40)


# --- Lo que el original NO calcula -------------------------------------------

def test_a_los_12_anios_o_menos_NO_CALCULA():
    """Fidelidad: en el original la rama pediátrica está COMENTADA y devuelve 0.

    Se migra el comportamiento, no el 0: devolver None obliga a la interfaz a
    decidir qué mostrar, mientras que un 0 en un campo de peso se lee como un
    dato real y equivocado.

    ES CANDIDATO PARA LA ETAPA DE MEJORAS: un consultorio que atiende niños se
    queda sin referencia justo donde más se usa.
    """
    assert peso_ideal(1.40, "M", 12) is None
    assert peso_ideal(1.20, "F", 8) is None
    assert peso_ideal(0.50, "M", 0) is None

    assert peso_ideal(1.45, "M", EDAD_MINIMA_PESO_IDEAL + 1) is not None, \
        "a partir de los 13 sí calcula"


def test_una_talla_invalida_no_produce_un_numero():
    assert peso_ideal(0, "M", 40) is None
    assert peso_ideal(-1.7, "M", 40) is None
    assert peso_ideal(None, "M", 40) is None


# --- Talla ideal: tabla, no fórmula ------------------------------------------

def test_sin_la_tabla_de_referencia_no_se_inventa_un_numero():
    """Inventar una fórmula daría un valor plausible y falso, peor que ninguno."""
    assert talla_ideal(10, "M") is None
    assert talla_ideal(10, "M", tabla_pesotalla={}) is None


def test_con_la_tabla_cargada_la_busca():
    tabla = {(10, "M"): 1.38, (10, "F"): 1.39}
    assert talla_ideal(10, "M", tabla) == 1.38
    assert talla_ideal(10, "F", tabla) == 1.39
    assert talla_ideal(99, "M", tabla) is None


# --- IMC ---------------------------------------------------------------------

def test_imc_normal():
    assert imc(70, 1.75) == 22.9


def test_imc_no_divide_por_cero():
    assert imc(70, 0) is None
    assert imc(0, 1.75) is None
    assert imc(None, 1.75) is None
