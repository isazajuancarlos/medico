"""Nota operatoria: el tiempo se calcula y el estado de salida se afirma.

Dos cosas del original:

  - `ProtocoloQuirurgico` guarda `hora_inicio`, `hora_fin` Y
    `tiempo_quirurgico_min`: tres campos para dos datos, escritos por separado.
  - `estado_paciente_salida` traía `default="ESTABLE"`. Es una afirmación
    clínica sobre cómo salió el paciente de quirófano, puesta de antemano. Si
    nadie la registraba, la nota decía que salió estable. Nadie firmó eso.
"""
import os
import sys
from datetime import datetime

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_aislamiento.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from notas_operatorias import (  # noqa: E402
    ESTADOS_SALIDA,
    EstadoNoValido,
    TiemposNoValidos,
    duracion,
    revisar,
    validar_estado_salida,
)


class Nota:
    def __init__(self, **k):
        self.hora_inicio = datetime(2026, 3, 2, 8, 0)
        self.hora_fin = datetime(2026, 3, 2, 9, 30)
        self.tiempo_quirurgico_min = 90
        self.sangrado_estimado_ml = 150
        self.complicaciones = None
        self.estado_paciente_salida = "ESTABLE"
        self.firma_cirujano_path = "f.png"
        for a, v in k.items():
            setattr(self, a, v)


class TestDuracion:
    def test_se_calcula_de_las_horas(self):
        assert duracion(datetime(2026, 3, 2, 8, 0),
                        datetime(2026, 3, 2, 9, 30)).minutos == 90

    def test_una_cirugia_puede_cruzar_la_MEDIANOCHE(self):
        """Empieza a las 23:40 y termina a la 1:20: son 100 minutos, no menos
        veintidós horas. Es la razón de que las horas sean datetime."""
        t = duracion(datetime(2026, 3, 2, 23, 40), datetime(2026, 3, 3, 1, 20))
        assert t.minutos == 100

    def test_terminar_antes_de_empezar_es_un_dato_malo(self):
        with pytest.raises(TiemposNoValidos) as e:
            duracion(datetime(2026, 3, 2, 10, 0), datetime(2026, 3, 2, 8, 0))
        assert "cruzó la medianoche" in str(e.value), "el mensaje debe orientar"

    def test_cero_minutos_no_es_una_cirugia(self):
        mismo = datetime(2026, 3, 2, 8, 0)
        with pytest.raises(TiemposNoValidos):
            duracion(mismo, mismo)

    def test_las_horas_se_devuelven_redondeadas(self):
        assert duracion(datetime(2026, 3, 2, 8, 0),
                        datetime(2026, 3, 2, 11, 15)).horas == 3.25


class TestDiscrepancia:
    def test_si_lo_escrito_coincide_no_hay_nada_que_decir(self):
        assert duracion(datetime(2026, 3, 2, 8, 0),
                        datetime(2026, 3, 2, 9, 30), 90).discrepancia is None

    def test_un_minuto_de_diferencia_se_tolera(self):
        """Redondeos al capturar. No merece un aviso."""
        assert duracion(datetime(2026, 3, 2, 8, 0),
                        datetime(2026, 3, 2, 9, 30), 91).discrepancia is None

    def test_una_diferencia_grande_se_señala_pero_NO_se_pisa(self):
        """Puede ser que las horas estén mal y no el tiempo: quien revise
        decide, el sistema no."""
        t = duracion(datetime(2026, 3, 2, 8, 0), datetime(2026, 3, 2, 9, 30), 150)
        assert t.discrepancia == 150
        assert t.minutos == 90


class TestEstadoDeSalida:
    @pytest.mark.parametrize("estado", ESTADOS_SALIDA)
    def test_los_admitidos_pasan(self, estado):
        assert validar_estado_salida(estado) == estado

    def test_se_normaliza_a_mayusculas(self):
        assert validar_estado_salida("  estable ") == "ESTABLE"

    @pytest.mark.parametrize("vacio", [None, "", "   "])
    def test_NO_registrarlo_es_un_error_no_un_ESTABLE(self, vacio):
        """El fallo del original."""
        with pytest.raises(EstadoNoValido) as e:
            validar_estado_salida(vacio)
        assert "que no ocurrió" in str(e.value)

    def test_uno_inventado_se_rechaza_y_dice_cuales_hay(self):
        with pytest.raises(EstadoNoValido) as e:
            validar_estado_salida("REGULAR")
        assert "ESTABLE" in str(e.value)

    def test_fallecido_es_un_estado_admitido(self):
        """Registrar lo que ocurrió, aunque sea lo peor."""
        assert "FALLECIDO" in ESTADOS_SALIDA


class TestAvisos:
    def test_una_nota_normal_no_produce_avisos(self):
        assert not revisar(Nota())
        assert "Sin observaciones" in str(revisar(Nota()))

    def test_avisa_del_sangrado_alto_sin_impedirlo(self):
        """En cirugía lo infrecuente pasa. Un sistema que no deja registrarlo
        obliga a mentirle."""
        a = revisar(Nota(sangrado_estimado_ml=2000))
        assert a
        assert "2000 ml" in str(a)

    def test_distingue_no_medido_de_no_sangro(self):
        assert "No se registró el sangrado" in str(revisar(Nota(sangrado_estimado_ml=None)))
        assert not revisar(Nota(sangrado_estimado_ml=0))

    def test_avisa_de_una_cirugia_larguisima(self):
        a = revisar(Nota(hora_fin=datetime(2026, 3, 2, 22, 0)))
        assert "horas de duración" in str(a)

    def test_avisa_de_las_complicaciones(self):
        assert "complicaciones" in str(revisar(Nota(complicaciones="Lesión de vía biliar")))

    def test_avisa_si_el_paciente_no_salio_estable(self):
        assert "INESTABLE" in str(revisar(Nota(estado_paciente_salida="INESTABLE")))

    def test_avisa_de_la_firma_que_falta(self):
        assert "firma del cirujano" in str(revisar(Nota(firma_cirujano_path=None)))

    def test_avisa_de_la_discrepancia_de_tiempos(self):
        a = revisar(Nota(tiempo_quirurgico_min=200))
        assert "no coincide" in str(a)

    def test_acumula_varios_avisos(self):
        a = revisar(Nota(sangrado_estimado_ml=1500, complicaciones="x",
                         firma_cirujano_path=None))
        assert len(a.mensajes) == 3
