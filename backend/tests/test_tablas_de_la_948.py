"""Las tablas de finalidad y causa son las de la Resolución 948, no las de antes.

Hasta el 2026-08-02 `FinalidadConsulta` y `CausaExterna` estaban escritas a mano
con la tabla ANTERIOR, y **ninguno de sus valores existía** en la que el MUV usa
para juzgar lo que se le envía:

    FinalidadConsulta  02..10   ·  la de la 948 va del 11 al 44
    CausaExterna       01..15   ·  la de causa/motivo, del 21 al 49

O sea que `finalidadTecnologiaSalud` y `causaMotivoAtencion` salían con un código
rechazable en TODA atención. No se notó durante meses por dos motivos que se
tapaban entre sí:

1. los campos tenían defecto (`NO_APLICA` y `ENFERMEDAD_GENERAL`), así que nunca
   llegaban vacíos y el recolector de faltantes —que solo ve AUSENCIAS— no tenía
   nada que anotar;
2. ninguna pantalla ofrece esos campos, así que nadie eligió uno nunca.

Un dato inventado que además es inválido es la peor combinación posible: pasa
todas las comprobaciones internas y lo rechaza el validador tres semanas después,
sin decir que lo puso el sistema y no el médico.

Estas pruebas atan las tres cosas: que las enumeraciones salgan del maestro, que
los códigos viejos ya no sean valores válidos, y que el modelo no vuelva a
responder por el profesional.
"""
import pytest

import maestros_muv
from models import CausaExterna, FinalidadConsulta, Historia


class TestSalenDelMaestroYNoDeAqui:
    def test_la_finalidad_son_las_34_del_validador(self):
        del_maestro = {f["Codigo"] for f in maestros_muv.finalidades()}
        assert len(del_maestro) == 34
        assert {m.value for m in FinalidadConsulta} == del_maestro

    def test_la_causa_son_las_29_del_validador(self):
        del_maestro = {c["Codigo"] for c in maestros_muv.causas_motivo()}
        assert len(del_maestro) == 29
        assert {m.value for m in CausaExterna} == del_maestro

    def test_los_rangos_son_los_de_la_948(self):
        finalidades = sorted(m.value for m in FinalidadConsulta)
        assert finalidades[0] == "11" and finalidades[-1] == "44"
        causas = sorted(m.value for m in CausaExterna)
        assert causas[0] == "21" and causas[-1] == "49"


class TestLosCodigosViejosYaNoValen:
    @pytest.mark.parametrize("codigo", ["10", "06", "07", "08", "09", "02", "03", "04", "05"])
    def test_ninguna_finalidad_de_la_tabla_anterior_es_valida(self, codigo):
        with pytest.raises(ValueError):
            FinalidadConsulta(codigo)

    @pytest.mark.parametrize("codigo", ["13", "01", "02", "15", "11"])
    def test_ninguna_causa_de_la_tabla_anterior_es_valida(self, codigo):
        with pytest.raises(ValueError):
            CausaExterna(codigo)

    def test_y_los_nuevos_si(self):
        # El contraste: si `FinalidadConsulta(...)` levantara ValueError para
        # cualquier cosa, las pruebas de arriba pasarían con la enumeración vacía.
        assert FinalidadConsulta("11").value == "11"
        assert FinalidadConsulta("44").value == "44"
        assert CausaExterna("21").value == "21"
        assert CausaExterna("49").value == "49"

    def test_enfermedad_general_existe_pero_con_otro_codigo(self):
        """El concepto no desapareció: cambió de número, de «13» a «38».

        Es justo lo que hace tentador traducir los datos viejos en un UPDATE
        masivo. No se hace: un valor que nadie eligió, traducido, sigue sin
        haberlo dicho nadie — y ya no lo detecta nada.
        """
        assert CausaExterna("38").name == "ENFERMEDAD_GENERAL"
        with pytest.raises(ValueError):
            CausaExterna("13")


class TestElModeloNoRespondePorElMedico:
    @pytest.mark.parametrize(
        "campo", ["finalidad", "causa_externa", "tipo_diagnostico"]
    )
    def test_los_tres_campos_nacen_vacios(self, campo):
        """Sin defecto: son afirmaciones del profesional sobre para qué y por qué
        se atendió. Que estén vacíos es lo que permite al generador pedirlos."""
        h = Historia(
            paciente_id=1,
            medico_id=1,
            codigo_historia="H1",
            motivo_consulta="x",
            enfermedad_actual="x",
            diagnostico_principal="J449",
            plan_tratamiento="x",
        )
        assert getattr(h, campo) is None

    def test_lo_que_se_elige_se_conserva(self):
        h = Historia(
            paciente_id=1,
            medico_id=1,
            codigo_historia="H1",
            motivo_consulta="x",
            enfermedad_actual="x",
            diagnostico_principal="J449",
            plan_tratamiento="x",
            finalidad=FinalidadConsulta("16"),
            causa_externa=CausaExterna("38"),
        )
        assert h.finalidad.value == "16"
        assert h.causa_externa.value == "38"
