"""La auditoría de completitud: qué se reportaría supuesto.

El fallo que evita: un generador que rellena huecos con constantes SIEMPRE
produce archivo, y el archivo pasa la validación de estructura. El problema
aparece un mes después, como glosa o como un pago que no llega.
"""
import os
import sys
from datetime import date

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from auditoria_rips import CAMPOS, auditar  # noqa: E402
from procedencia import Procedencia, marcar  # noqa: E402


class Paciente:
    def __init__(self, ident="111"):
        self.identificacion = ident


class Atencion:
    """Atención completa por defecto; las pruebas le quitan lo que quieren probar.

    Los campos se marcan como CONFIRMADOS porque eso es lo que significa
    «completa»: alguien los puso. Una atención con los mismos valores pero sin
    procedencia NO está completa, y esa es justamente la distinción nueva.
    """

    def __init__(self, sin_procedencia=False, **faltantes):
        self.id = 1
        self.fecha = date(2026, 7, 19)
        self.paciente = Paciente()
        self.valor_servicio = 45000
        self.cod_servicio = 328
        self.cod_consulta_cups = "890301"
        self.modalidad_grupo_servicio = "01"
        self.grupo_servicios = "01"
        self.concepto_recaudo = "05"
        self.procedencias = None
        for k, v in faltantes.items():
            setattr(self, k, v)
        if not sin_procedencia:
            for atributo, _, _, _ in CAMPOS:
                if getattr(self, atributo, None) not in (None, "", 0):
                    marcar(self, atributo, Procedencia.CONFIRMADO)


def test_una_atencion_completa_no_produce_huecos():
    a = auditar([Atencion()])
    assert a.total == 1 and a.completas == 1
    assert a.huecos == []
    assert a.reportable_sin_supuestos


def test_detecta_el_valor_del_servicio_en_cero():
    """El hueco más grave: iba QUEMADO en 0 para todo paciente, y de ese valor
    depende que le paguen."""
    a = auditar([Atencion(valor_servicio=0)])

    assert a.completas == 0
    assert len(a.huecos) == 1
    assert a.huecos[0].campo == "valor_servicio"
    assert a.huecos[0].critico
    assert not a.reportable_sin_supuestos


def test_None_y_cero_y_vacio_cuentan_igual_como_faltante():
    for vacio in (None, 0, ""):
        assert auditar([Atencion(valor_servicio=vacio)]).completas == 0


def test_separa_los_criticos_de_los_demas():
    """El valor y el CUPS afectan pago y clasificación; la modalidad, no tanto."""
    a = auditar([Atencion(valor_servicio=0, modalidad_grupo_servicio=None)])

    assert len(a.huecos) == 2
    assert len(a.criticos) == 1
    assert a.criticos[0].campo == "valor_servicio"


def test_cuenta_por_campo_para_saber_que_corregir_primero():
    atenciones = [Atencion(valor_servicio=0) for _ in range(5)]
    atenciones += [Atencion(cod_servicio=None) for _ in range(2)]

    por_campo = auditar(atenciones).por_campo()
    assert por_campo["valor del servicio"] == 5
    assert por_campo["código de servicio"] == 2
    assert list(por_campo)[0] == "valor del servicio", "el más frecuente va primero"


def test_una_atencion_puede_tener_varios_huecos():
    a = auditar([Atencion(valor_servicio=0, cod_servicio=None, concepto_recaudo=None)])
    assert a.total == 1
    assert a.completas == 0
    assert len(a.huecos) == 3


def test_no_bloquea_nada_solo_informa():
    """Un consultorio que siempre cobra igual puede aceptar el valor por defecto
    a sabiendas. Lo que no debe pasar es que no lo sepa."""
    a = auditar([Atencion(valor_servicio=0)])
    assert a.total == 1, "la atención sigue contándose"
    assert a.huecos[0].supuesto == 0, "y se dice qué se enviaría en su lugar"


def test_el_informe_se_lee():
    atenciones = [Atencion() for _ in range(8)] + [Atencion(valor_servicio=0)] * 2
    texto = str(auditar(atenciones))

    assert "8 de 10" in texto
    assert "valor del servicio: 2" in texto
    assert "afectan el pago" in texto


def test_un_periodo_vacio_no_divide_por_cero():
    a = auditar([])
    assert a.total == 0
    assert not a.reportable_sin_supuestos
    assert "No hay atenciones" in str(a)


def test_una_atencion_sin_paciente_no_tumba_la_auditoria():
    at = Atencion(valor_servicio=0)
    at.paciente = None
    assert auditar([at]).incompletas == 1


def test_todos_los_campos_auditados_tienen_su_supuesto_declarado():
    """Si alguien añade un campo sin decir qué se supone en su lugar, el informe
    mentiría por omisión."""
    for atributo, etiqueta, supuesto, critico in CAMPOS:
        assert atributo and etiqueta
        assert supuesto is not None
        assert isinstance(critico, bool)


# --- Lo que la procedencia añade -----------------------------------------

def test_un_valor_SIN_procedencia_ya_no_pasa_por_bueno():
    """El fallo original: `cod_consulta_cups="890201"` en toda consulta. La
    columna nunca estaba vacía, así que la auditoría no lo veía."""
    a = auditar([Atencion(sin_procedencia=True)])

    assert a.completas == 0
    assert len(a.sin_respaldo) == len(CAMPOS)
    assert a.huecos == [], "no faltan datos: falta saber quién los puso"
    assert not a.reportable_sin_supuestos


def test_los_mismos_valores_CON_procedencia_si_pasan():
    """El control: lo que se rechaza es la falta de respaldo, no el valor."""
    a = auditar([Atencion()])
    assert a.completas == 1
    assert a.sin_respaldo == []
    assert a.reportable_sin_supuestos


def test_derivado_y_propuesto_tambien_cuentan_como_respaldo():
    """Un tiempo calculado de las horas es cierto y no cuesta tiempo al
    clínico; un CUPS sugerido y aceptado cuesta una tecla. Los dos son mejores
    que un defecto, y los dos dejan rastro."""
    for como in (Procedencia.DERIVADO, Procedencia.PROPUESTO):
        at = Atencion(sin_procedencia=True)
        for atributo, _, _, _ in CAMPOS:
            marcar(at, atributo, como)
        assert auditar([at]).sin_respaldo == []


def test_falta_de_dato_y_falta_de_respaldo_se_distinguen():
    """Son problemas distintos y se corrigen distinto: uno se captura, el otro
    se verifica."""
    a = auditar([Atencion(sin_procedencia=True, valor_servicio=0)])

    campos_huecos = {h.campo for h in a.huecos}
    campos_sin_respaldo = {h.campo for h in a.sin_respaldo}

    assert campos_huecos == {"valor_servicio"}
    assert "valor_servicio" not in campos_sin_respaldo
    assert "cod_consulta_cups" in campos_sin_respaldo


def test_el_informe_menciona_los_que_nadie_afirmo():
    texto = str(auditar([Atencion(sin_procedencia=True)]))
    assert "nadie los afirmó" in texto
    assert "por defecto" in texto
