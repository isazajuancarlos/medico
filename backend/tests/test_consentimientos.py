"""Consentimiento informado: firmar no es consentir, y revocar no se bloquea.

La versión NiceGUI guardaba el registro y lo daba por bueno. Nadie juzgaba si lo
que había ahí era un consentimiento informado o solo un papel con una firma.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from consentimientos import (  # noqa: E402
    EXPLICACIONES,
    NoSePuedeRevocar,
    evaluar,
    requiere_testigo,
    revocar,
)


class Consentimiento:
    """Uno completo por defecto; cada prueba le quita lo que quiere probar."""

    def __init__(self, **cambios):
        self.descripcion_procedimiento = "Colecistectomía laparoscópica"
        self.riesgos_explicados = "Sangrado, infección, conversión a abierta"
        self.beneficios_esperados = "Cese del dolor biliar"
        self.alternativas_explicadas = "Manejo médico, cirugía abierta"
        self.consecuencias_no_realizarlo = "Riesgo de colecistitis aguda"
        self.paciente_comprende = True
        self.acepta_procedimiento = True
        self.firma_paciente_path = "uploads/firmas/p.png"
        self.firma_medico_path = "uploads/firmas/m.png"
        self.testigo_nombre = None
        self.revocado = False
        self.fecha_revocacion = None
        self.motivo_revocacion = None
        for k, v in cambios.items():
            setattr(self, k, v)


class TestFirmarNoEsConsentir:
    def test_uno_completo_vale(self):
        v = evaluar(Consentimiento())
        assert v.valido
        assert v.faltantes == ()

    def test_con_las_firmas_pero_SIN_comprension_no_vale(self):
        """El caso que sostiene toda la figura. Hay un papel firmado; no hay
        consentimiento informado, y quien opere con eso no tiene defensa."""
        v = evaluar(Consentimiento(paciente_comprende=False))

        assert not v.valido
        assert "la constancia de que el paciente comprendió" in v.faltantes

    def test_comprender_no_es_aceptar(self):
        """Se puede entender perfectamente y decir que no."""
        v = evaluar(Consentimiento(acepta_procedimiento=False))
        assert not v.valido
        assert "la aceptación del procedimiento" in v.faltantes

    @pytest.mark.parametrize("campo,etiqueta", EXPLICACIONES)
    def test_falta_cualquiera_de_las_explicaciones_de_la_norma(self, campo, etiqueta):
        v = evaluar(Consentimiento(**{campo: ""}))
        assert not v.valido
        assert etiqueta in v.faltantes

    def test_un_campo_con_solo_espacios_no_cuenta_como_explicado(self):
        assert not evaluar(Consentimiento(riesgos_explicados="    ")).valido

    def test_sin_la_firma_del_paciente_no_vale(self):
        v = evaluar(Consentimiento(firma_paciente_path=None))
        assert "la firma del paciente" in v.faltantes

    def test_sin_la_firma_del_medico_tampoco(self):
        v = evaluar(Consentimiento(firma_medico_path=None))
        assert "la firma del médico" in v.faltantes

    def test_enumera_TODO_lo_que_falta_no_solo_lo_primero(self):
        v = evaluar(Consentimiento(paciente_comprende=False,
                                   firma_paciente_path=None,
                                   riesgos_explicados=""))
        assert len(v.faltantes) == 3

    def test_el_resumen_se_lee(self):
        assert "completo" in str(evaluar(Consentimiento()))
        assert "falta" in str(evaluar(Consentimiento(paciente_comprende=False)))


class TestRevocacion:
    def test_un_revocado_no_autoriza_nada(self):
        v = evaluar(Consentimiento(revocado=True))
        assert not v.valido
        assert v.revocado
        assert "No autoriza" in str(v)

    def test_el_revocado_manda_sobre_todo_lo_demas(self):
        """Aunque esté impecablemente firmado y comprendido."""
        c = Consentimiento(revocado=True)
        assert c.paciente_comprende and c.firma_paciente_path
        assert not evaluar(c).valido

    def test_se_puede_revocar_uno_completo(self):
        c = Consentimiento()
        revocar(c, "Prefiere consultar una segunda opinión")
        assert c.revocado
        assert c.motivo_revocacion == "Prefiere consultar una segunda opinión"
        assert c.fecha_revocacion is not None

    def test_se_puede_revocar_uno_INCOMPLETO(self):
        """El derecho a revocar no depende de en qué punto esté el trámite."""
        c = Consentimiento(paciente_comprende=False, firma_medico_path=None)
        revocar(c, "Ya no quiere")
        assert c.revocado

    def test_se_puede_revocar_uno_YA_REVOCADO(self):
        """No se bloquea: si el paciente lo manifiesta otra vez, se registra
        otra vez. Un 409 aquí no protege a nadie."""
        c = Consentimiento()
        revocar(c, "primera")
        revocar(c, "segunda")
        assert c.motivo_revocacion == "segunda"

    def test_lo_UNICO_que_impide_revocar_es_no_decir_qué_pasó(self):
        for vacio in ("", "   ", None):
            with pytest.raises(NoSePuedeRevocar):
                revocar(Consentimiento(), vacio)

    def test_el_mensaje_aclara_que_el_motivo_no_se_juzga(self):
        with pytest.raises(NoSePuedeRevocar) as e:
            revocar(Consentimiento(), "")
        assert "no tiene que darlo" in str(e.value)

    def test_revocar_conserva_el_registro_no_lo_borra(self):
        """Borrarlo dejaría a la institución sin la prueba de que sí lo hubo."""
        c = Consentimiento()
        revocar(c, "cambió de opinión")
        assert c.descripcion_procedimiento
        assert c.firma_paciente_path
        assert c.riesgos_explicados

    def test_se_puede_fijar_la_fecha(self):
        c = Consentimiento()
        cuando = datetime(2026, 3, 1, 10, 0)
        revocar(c, "x", cuando)
        assert c.fecha_revocacion == cuando


class TestTestigo:
    def test_hay_testigo_cuando_se_registró_uno(self):
        assert requiere_testigo(Consentimiento(testigo_nombre="María Gómez"))

    def test_no_se_deduce_de_la_edad_ni_de_nada_automatico(self):
        """Una regla automática se equivocaría con un menor adulto o con
        alguien bajo medida de apoyo. Lo decide quien atiende."""
        assert not requiere_testigo(Consentimiento())
