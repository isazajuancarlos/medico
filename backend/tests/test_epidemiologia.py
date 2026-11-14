"""Los informes epidemiológicos, contra el comportamiento del sistema original.

Lo que más importa aquí es la FIDELIDAD: si los números no coinciden con los que
el prestador viene reportando a la Secretaría de Salud, el informe nuevo no
sirve. Por eso hay pruebas que fijan comportamientos del original que a primera
vista parecen defectos y que se conservan a propósito. La excepción es el
cálculo de la edad, que SÍ se corrigió: ver la sección correspondiente.

Todo con datos sintéticos.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from epidemiologia import (  # noqa: E402
    CONJUNTO_COBERTURA, CONJUNTO_CVE, CONJUNTO_IRA, EVENTO_IRA, catalogos,
    clasificar_edad, cobertura, edad, edad_calendario, edad_vfp, es_its,
    eventos_vigilancia, evento_de, ira, its, morbilidad, rangos, resumen,
)


class Paciente:
    def __init__(self, ident, nacimiento, sexo="F"):
        self.identificacion = ident
        self.fecha_nacimiento = nacimiento
        self.sexo = sexo
        self.tipo_id = "CC"
        self.apellido1 = "Pérez"
        self.apellido2 = ""
        self.nombre1 = "Ana"
        self.nombre2 = ""
        self.zona = "URBANO"
        self.tipo_usuario = "01"
        self.cod_municipio = "70001"


class Atencion:
    def __init__(self, paciente, fecha, diagnostico):
        self.paciente = paciente
        self.fecha = fecha
        self.diagnostico_principal = diagnostico


HOY = date(2026, 7, 19)


# --- Catálogos ---------------------------------------------------------------

def test_los_catalogos_se_cargaron_del_sistema_original():
    c = catalogos()
    assert len(c["its"]) == 13, "el catálogo ITS del original tiene 13 códigos"
    assert len(c["cve"]) == 229
    assert len(c["eventos"]) == 78


def test_los_tres_conjuntos_de_rangos_son_los_del_original():
    assert len(rangos(CONJUNTO_IRA)) == 7
    assert len(rangos(CONJUNTO_COBERTURA)) == 6
    assert len(rangos(CONJUNTO_CVE)) == 18


def test_un_conjunto_inexistente_no_pasa_en_silencio():
    with pytest.raises(ValueError, match="desconocido"):
        rangos("rango99")


def test_el_evento_se_deriva_del_diagnostico():
    """Sustituye al campo cve1 que llenaba el médico en el original."""
    assert evento_de("J00X") == EVENTO_IRA
    assert evento_de("j00x") == EVENTO_IRA, "sin importar mayúsculas"
    assert evento_de("X209") == "100", "accidente ofídico"
    assert evento_de("Z999") is None
    assert evento_de(None) is None


def test_el_catalogo_de_ITS_reconoce_sus_codigos():
    assert es_its("A509")
    assert es_its("a64x")
    assert not es_its("J00X")
    assert not es_its(None)


# --- El cálculo del original, conservado solo para comparar ----------------------------------------------------

def test_la_edad_se_calcula_como_el_original_en_anos_de_365_dias():
    """Fidelidad deliberada: el VFP hacía INT(días/365) e ignoraba bisiestos."""
    nacimiento = date(2000, 1, 1)
    evento = date(2026, 1, 1)
    dias = (evento - nacimiento).days
    assert edad_vfp(evento, nacimiento) == int(dias / 365) == 26


def test_la_edad_VFP_ENVEJECE_a_la_gente_frente_a_la_calendario():
    """El caso concreto que justifica conservar el cálculo, MEDIDO, no supuesto.

    Nacido el 1-ene-1950, el 14-dic-2020 tiene 70 años cumplidos, pero el
    original lo cuenta como 71: los bisiestos acumulados adelantan el cambio de
    año unas dos semanas, y el desfase crece con la edad.
    """
    nacimiento = date(1950, 1, 1)
    evento = date(2020, 12, 14)

    assert edad_calendario(evento, nacimiento) == 70, "todavía no cumple 71"
    assert edad_vfp(evento, nacimiento) == 71, "pero el original ya lo cuenta como 71"


def test_el_desfase_de_edad_afecta_POCAS_clasificaciones_de_rango():
    """Lo que importa no es cuántas edades difieren, sino cuántas cambian de
    RANGO: es lo único que mueve un número en el informe.

    Medido sobre nacimientos de 1930 a 2024 y eventos de todo 2026: la edad
    difiere en ~5 % de los casos, pero el rango solo cambia en menos del 1 %.
    Esta medición fue la que permitió decidir con datos: al ser tan pequeña, se
    adoptó el cálculo CORRECTO. La prueba se conserva para poder explicarle al
    prestador el tamaño de la diferencia si compara contra sus reportes viejos.
    """
    from datetime import timedelta

    def rango_de(edad, conjunto):
        for r in rangos(conjunto):
            if r["inicial"] <= edad <= r["final"]:
                return r["nombre"]
        return None

    for conjunto in (CONJUNTO_IRA, CONJUNTO_COBERTURA, CONJUNTO_CVE):
        cambia = total = 0
        for anio in range(1930, 2026, 2):
            nacimiento = date(anio, 6, 15)
            d = date(2026, 1, 1)
            while d < date(2027, 1, 1):
                total += 1
                if rango_de(edad_vfp(d, nacimiento), conjunto) != rango_de(
                    edad_calendario(d, nacimiento), conjunto
                ):
                    cambia += 1
                d += timedelta(days=7)

        proporcion = cambia / total
        assert proporcion < 0.01, (
            f"{conjunto}: {proporcion:.2%} de las clasificaciones cambian de rango; "
            f"si esto sube, revisar si el cálculo de edad sigue siendo el del original"
        )


def test_los_limites_de_cada_rango_son_inclusivos():
    """rango1: 0 | 1 | 2-4 | 5-19 | 20-39 | 40-59 | 60+"""
    def con_edad(años):
        nac = date(HOY.year - años, HOY.month, HOY.day)
        return clasificar_edad(HOY, nac, CONJUNTO_IRA)

    assert con_edad(0) == "rango1"
    assert con_edad(1) == "rango2"
    assert con_edad(3) == "rango3"
    assert con_edad(19) == "rango4"
    assert con_edad(20) == "rango5"
    assert con_edad(59) == "rango6"
    assert con_edad(60) == "rango7"


def test_nacer_despues_del_evento_es_un_error_de_datos_y_se_dice():
    """Antes producía una edad negativa que caía callada fuera de todo rango.
    Es un dato imposible: mejor que reviente donde se puede corregir."""
    futuro = date(2030, 1, 1)
    with pytest.raises(ValueError, match="anterior al nacimiento"):
        clasificar_edad(HOY, futuro, CONJUNTO_IRA)


# --- El cálculo correcto de edad: años, meses y días -------------------------

def test_la_edad_trae_los_tres_componentes():
    e = edad(date(2026, 7, 19), date(1980, 3, 15))
    assert (e.años, e.meses, e.días) == (46, 4, 4)
    assert e.total_días == (date(2026, 7, 19) - date(1980, 3, 15)).days


def test_el_aniversario_mensual_se_RECORTA_al_mes_corto():
    """Un mes después del 31 de enero es el 29 de febrero en año bisiesto.

    Restar campo a campo y pedir prestado al mes anterior producía días
    NEGATIVOS en este caso, porque febrero es más corto que el día 31.
    """
    nacimiento = date(2000, 1, 31)
    assert str(edad(date(2000, 2, 29), nacimiento)) == "0a 1m 0d"
    assert str(edad(date(2000, 3, 1), nacimiento)) == "0a 1m 1d"


def test_quien_nace_un_29_de_febrero_cumple_el_28():
    """En año no bisiesto el aniversario se recorta al último día existente."""
    assert str(edad(date(2025, 2, 28), date(2024, 2, 29))) == "1a 0m 0d"


def test_ningun_componente_puede_ser_negativo_jamas():
    """Barrido: cuatro fechas de nacimiento difíciles —día 31, 29 de febrero,
    fin de año— por 4.000 días cada una."""
    from datetime import timedelta

    for base in (date(2000, 1, 31), date(2024, 2, 29), date(1999, 12, 31), date(2001, 3, 15)):
        d = base
        for _ in range(4000):
            d += timedelta(days=1)
            e = edad(d, base)
            assert e.años >= 0 and 0 <= e.meses <= 11 and e.días >= 0, f"{base} -> {d}: {e}"


def test_la_unidad_RIPS_cambia_con_la_edad():
    """RIPS y las fichas de notificación piden la unidad de medida: un menor de
    un año se reporta en meses, y un recién nacido en días."""
    nacimiento = date(2026, 7, 10)
    assert edad(date(2026, 7, 19), nacimiento).unidad_rips == "días"
    assert edad(date(2026, 7, 19), nacimiento).valor_rips == 9

    assert edad(date(2026, 9, 19), nacimiento).unidad_rips == "meses"
    assert edad(date(2026, 9, 19), nacimiento).valor_rips == 2

    assert edad(date(2028, 9, 19), nacimiento).unidad_rips == "años"
    assert edad(date(2028, 9, 19), nacimiento).valor_rips == 2


def test_el_mismo_dia_es_cero_dias_no_un_error():
    e = edad(date(2026, 7, 19), date(2026, 7, 19))
    assert (e.años, e.meses, e.días, e.total_días) == (0, 0, 0, 0)
    assert e.unidad_rips == "días"


def test_total_meses_acumula_los_anios():
    e = edad(date(2028, 9, 19), date(2026, 7, 10))
    assert e.años == 2 and e.meses == 2
    assert e.total_meses == 26


def test_los_informes_ya_usan_la_edad_correcta():
    """`clasificar_edad` dejó de usar el cálculo del original."""
    nacimiento = date(1950, 1, 1)
    evento = date(2020, 12, 14)
    assert edad_vfp(evento, nacimiento) == 71, "el original lo envejecía"
    assert edad(evento, nacimiento).años == 70

    # rango1: ...40-59 | 60+ — aquí ambos caen en el mismo tramo, pero la
    # clasificación debe seguir la edad REAL.
    assert clasificar_edad(evento, nacimiento, CONJUNTO_IRA) == "rango7"


# --- Morbilidad --------------------------------------------------------------

def test_morbilidad_cuenta_y_ordena_de_mayor_a_menor():
    p = Paciente("111", date(1990, 1, 1))
    atenciones = [
        Atencion(p, HOY, "J00X"), Atencion(p, HOY, "J00X"),
        Atencion(p, HOY, "A509"),
    ]
    r = morbilidad(atenciones, {"J00X": "Rinofaringitis", "A509": "Sífilis"})

    assert r[0] == {"codigo": "J00X", "nombre": "Rinofaringitis", "cantidad": 2}
    assert r[1]["codigo"] == "A509"


def test_morbilidad_descarta_diagnosticos_vacios():
    """El original filtraba con !EMPTY(diag1)."""
    p = Paciente("111", date(1990, 1, 1))
    r = morbilidad([Atencion(p, HOY, ""), Atencion(p, HOY, None), Atencion(p, HOY, "J00X")])
    assert len(r) == 1
    assert r[0]["codigo"] == "J00X"


# --- Informes cruzados -------------------------------------------------------

def test_eventos_de_vigilancia_cruza_por_rango_y_sexo():
    hombre = Paciente("111", date(1996, 7, 19), sexo="M")
    mujer = Paciente("222", date(1996, 7, 19), sexo="F")
    r = eventos_vigilancia([
        Atencion(hombre, HOY, "J00X"),
        Atencion(mujer, HOY, "J00X"),
        Atencion(mujer, HOY, "X209"),
    ])

    por_evento = {f["codigo"]: f for f in r}
    assert por_evento[EVENTO_IRA]["totalM"] == 1
    assert por_evento[EVENTO_IRA]["totalF"] == 1
    assert por_evento[EVENTO_IRA]["total"] == 2
    assert por_evento["100"]["totalF"] == 1


def test_un_diagnostico_sin_evento_no_entra_al_informe():
    p = Paciente("111", date(1990, 1, 1))
    assert eventos_vigilancia([Atencion(p, HOY, "Z999")]) == []


def test_ira_solo_toma_el_evento_995_y_usa_siete_tramos():
    p = Paciente("111", date(1990, 1, 1))
    r = ira([Atencion(p, HOY, "J00X"), Atencion(p, HOY, "X209")])

    assert len(r) == 1
    assert r[0]["codigo"] == EVENTO_IRA
    assert r[0]["total"] == 1, "el accidente ofídico no es IRA"
    for tramo in ("rango1", "rango7"):
        assert tramo in r[0]
    assert "rango8" not in r[0], "rango1 tiene 7 tramos, no 18"


def test_its_cruza_contra_su_propio_catalogo():
    p = Paciente("111", date(1996, 7, 19), sexo="F")
    r = its([Atencion(p, HOY, "A509"), Atencion(p, HOY, "J00X")])

    assert len(r) == 1
    assert r[0]["codigo"] == "A509"
    assert r[0]["totalF"] == 1


# --- Cobertura: personas, no atenciones --------------------------------------

def test_cobertura_cuenta_PERSONAS_no_atenciones():
    """La diferencia con los demás informes: quien consultó tres veces cuenta
    una sola vez. Viene del original, que indexaba por cédula antes de contar."""
    p = Paciente("111", date(1990, 1, 1))
    otro = Paciente("222", date(1990, 1, 1))
    r = cobertura([
        Atencion(p, HOY, "J00X"), Atencion(p, HOY, "J00X"), Atencion(p, HOY, "A509"),
        Atencion(otro, HOY, "J00X"),
    ])

    assert len(r) == 1
    assert r[0]["total"] == 2, "dos personas, aunque hubo cuatro atenciones"


def test_cobertura_usa_los_seis_tramos():
    p = Paciente("111", date(1990, 1, 1))
    r = cobertura([Atencion(p, HOY, "J00X")])
    assert "rango6" in r[0]
    assert "rango7" not in r[0]


# --- Robustez ----------------------------------------------------------------

def test_una_atencion_sin_paciente_no_tumba_el_informe():
    class SinPaciente:
        paciente = None
        fecha = HOY
        diagnostico_principal = "J00X"

    assert eventos_vigilancia([SinPaciente()]) == []


def test_un_paciente_sin_fecha_de_nacimiento_se_omite():
    p = Paciente("111", None)
    assert eventos_vigilancia([Atencion(p, HOY, "J00X")]) == []


def test_acepta_datetime_ademas_de_date():
    p = Paciente("111", date(1990, 1, 1))
    r = eventos_vigilancia([Atencion(p, datetime(2026, 7, 19, 10, 30), "J00X")])
    assert r[0]["total"] == 1


def test_un_periodo_vacio_produce_informes_vacios():
    assert resumen([]) == {
        "morbilidad": 0, "eventos_vigilancia": 0, "ira": 0,
        "its": 0, "cobertura": 0, "procedimientos": 0,
    }
