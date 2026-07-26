"""Los campos P11/P12 son del MÉDICO, no del paciente.

Lo destapó el MUV real el 2026-07-26: rechazaba con RVG01 toda consulta cuyo
paciente fuera menor, porque `RC` y `TI` no pueden ser el documento de un
profesional. Ninguna de las 606 pruebas lo vio: comprueban estructura, y esto
es semántica — el campo tenía la forma correcta con el contenido equivocado.

Lo grave no era el rechazo. Cuando el paciente era un adulto con cédula, el
archivo PASABA llevando el documento del paciente como si fuera el del médico:
un dato falso que no se nota nunca.
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from datos_sinteticos import documento_sintetico  # noqa: E402
from models import TIPOS_DOCUMENTO, TIPOS_DOCUMENTO_PROFESIONAL  # noqa: E402
from rips_completo import Recolector, _documento_profesional  # noqa: E402


class _Medico:
    def __init__(self, tipo=None, numero=None):
        self.tipo_documento = tipo
        self.numero_documento = numero


def _llamar(medico, permitir=False):
    rec = Recolector(permitir_supuestos=permitir)
    return _documento_profesional(medico, rec, archivo="AC", registro_id=1), rec


class TestElCampoEsDelProfesional:
    def test_un_medico_con_documento_pasa_sin_anotar_nada(self):
        campos, rec = _llamar(_Medico("CC", "79123456"))
        assert campos == {
            "tipoDocumentoIdentificacion": "CC",
            "numDocumentoIdentificacion": "79123456",
        }
        assert rec.faltantes == []

    def test_sin_medico_se_anota_y_NO_se_inventa(self):
        """Inventar un documento de profesional atribuye un acto médico."""
        campos, rec = _llamar(None, permitir=True)
        assert campos["tipoDocumentoIdentificacion"] is None
        assert campos["numDocumentoIdentificacion"] is None
        assert len(rec.faltantes) == 2

    def test_sin_supuestos_el_cierre_revienta(self):
        from rips_completo import DatosIncompletos
        _, rec = _llamar(_Medico())
        with pytest.raises(DatosIncompletos):
            rec.cerrar()


class TestLosTiposDeMenorSonImposibles:
    """El caso exacto que el MUV rechazó."""

    @pytest.mark.parametrize("tipo", ["RC", "TI"])
    def test_un_documento_de_menor_se_anota_diciendo_por_que(self, tipo):
        campos, rec = _llamar(_Medico(tipo, "1090123456"), permitir=True)
        # Se devuelve TAL CUAL: es el dato que hay que corregir en la base.
        assert campos["tipoDocumentoIdentificacion"] == tipo
        motivos = [f.motivo for f in rec.faltantes if f.motivo]
        assert motivos, "un tipo imposible tiene que llevar motivo"
        assert tipo in motivos[0] and "profesional" in motivos[0]

    def test_los_ocho_de_la_norma_pasan(self):
        for tipo in TIPOS_DOCUMENTO_PROFESIONAL:
            _, rec = _llamar(_Medico(tipo, "79123456"))
            assert rec.faltantes == [], f"{tipo} debería ser válido"

    def test_las_dos_listas_no_son_la_misma(self):
        """Si alguien las unifica, el error vuelve sin que nada se ponga rojo."""
        assert "RC" in TIPOS_DOCUMENTO
        assert "RC" not in TIPOS_DOCUMENTO_PROFESIONAL
        assert "TI" not in TIPOS_DOCUMENTO_PROFESIONAL


class TestElArchivoSinteticoLosDistingue:
    """La prueba que habría cazado el fallo original."""

    def _servicios(self):
        for u in documento_sintetico(20)["usuarios"]:
            for tipo, atenciones in u["servicios"].items():
                for a in atenciones:
                    yield u, tipo, a

    def test_ningun_servicio_lleva_el_documento_del_paciente(self):
        for usuario, tipo, atencion in self._servicios():
            assert (atencion["numDocumentoIdentificacion"]
                    != usuario["numDocumentoIdentificacion"]), (
                f"en {tipo} se coló el documento del paciente")

    def test_ningun_servicio_lleva_un_tipo_imposible(self):
        for _, tipo, atencion in self._servicios():
            assert atencion["tipoDocumentoIdentificacion"] in TIPOS_DOCUMENTO_PROFESIONAL, (
                f"{tipo} lleva un tipo que la 948 no admite para un profesional")

    def test_el_archivo_ejercita_los_cinco_bloques(self):
        """Validar solo consultas responde por un quinto y se concluye del todo."""
        tipos = {t for _, t, _ in self._servicios()}
        assert tipos == {"consultas", "procedimientos", "medicamentos",
                         "urgencias", "hospitalizacion"}, tipos
