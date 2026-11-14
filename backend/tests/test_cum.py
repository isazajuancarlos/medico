"""El catálogo de CUM, contra el que el MUV valida la factura.

Antes, `codTecnologiaSalud` salía del formulario sin comprobar nada y con
`"00000000"` de relleno cuando faltaba. Ese valor tiene forma de CUM, pasa
cualquier validación de estructura, y hace rechazar la factura completa.

Lo que se fija aquí: que el catálogo esté cargado de verdad, que distinga
«no existe» de «está inactivo», y que NUNCA devuelva un valor por defecto.
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import cum  # noqa: E402


def test_el_catalogo_trae_los_codigos_esperados():
    """Si alguien regenera el artefacto y sale otra cifra, se entera aquí."""
    assert cum.total() == 65997
    assert cum.total_activos() == 50951


def test_un_cum_real_se_encuentra_con_sus_principios():
    m = cum.buscar("20048021-41")
    assert m is not None
    assert "CARDIK" in m.nombre
    assert m.forma_farmaceutica == "TABLETA RECUBIERTA"
    # Es un combinado: tres principios activos, no uno. El catálogo trae una
    # fila por principio y agruparlas mal daría la concentración de uno solo.
    assert len(m.principios) >= 2, m.principios
    assert " + " in m.concentracion


def test_un_codigo_inventado_no_existe():
    assert cum.buscar("00000000") is None
    assert cum.buscar("99999999-99") is None


class TestNormalizar:
    def test_acepta_los_separadores_que_llegan_de_sistemas_viejos(self):
        assert cum.normalizar("20048021 41") == "20048021-41"
        assert cum.normalizar("20048021_41") == "20048021-41"
        assert cum.normalizar("  20048021-41  ") == "20048021-41"

    def test_no_completa_lo_que_falta(self):
        """Un CUM sin consecutivo no es un CUM incompleto: es otro producto."""
        assert cum.buscar("20048021") is None

    def test_vacio_sale_vacio(self):
        assert cum.normalizar(None) == ""
        assert cum.normalizar("   ") == ""


class TestValidar:
    def test_devuelve_el_codigo_normalizado(self):
        assert cum.validar("20048021 41") == "20048021-41"

    def test_sin_codigo_levanta_en_vez_de_rellenar(self):
        """La regla que este módulo existe para hacer cumplir."""
        with pytest.raises(cum.CumNoValido) as e:
            cum.validar(None)
        assert "no tiene valor por defecto" in str(e.value)

    def test_el_relleno_historico_ya_no_pasa(self):
        with pytest.raises(cum.CumNoValido) as e:
            cum.validar("00000000")
        assert "no está en el CatalogoCUMs" in str(e.value)


class TestActivoNoEsLoMismoQueExiste:
    """Dos diagnósticos distintos llevan a acciones distintas."""

    def _un_inactivo(self):
        for m in cum._cargar().values():
            if not m.activo:
                return m
        pytest.skip("el catálogo no trae inactivos")

    def test_un_inactivo_SI_existe(self):
        inactivo = self._un_inactivo()
        assert cum.validar(inactivo.codigo) == inactivo.codigo

    def test_pero_validar_activo_lo_rechaza_diciendo_por_que(self):
        inactivo = self._un_inactivo()
        with pytest.raises(cum.CumNoValido) as e:
            cum.validar_activo(inactivo.codigo)
        mensaje = str(e.value)
        assert "INACTIVO" in mensaje
        assert "Existe, pero" in mensaje, (
            "el mensaje tiene que distinguirlo de «no existe»: son problemas "
            "distintos y se corrigen de forma distinta"
        )

    def test_un_activo_pasa_las_dos(self):
        activo = next(m for m in cum._cargar().values() if m.activo)
        assert cum.validar(activo.codigo) == activo.codigo
        assert cum.validar_activo(activo.codigo) == activo.codigo


def test_se_busca_por_principio_activo_y_no_solo_por_marca():
    """El médico piensa en «losartán», no en el nombre comercial."""
    porNombre = cum.buscar_por_nombre("LOSARTAN", limite=5)
    assert porNombre, "no encontró nada por principio activo"
    assert all(
        "LOSARTAN" in m.nombre.upper()
        or any("LOSARTAN" in p.upper() for p in m.principios)
        for m in porNombre
    )


def test_buscar_por_nombre_vacio_no_devuelve_el_catalogo_entero():
    assert cum.buscar_por_nombre("") == []
    assert cum.buscar_por_nombre("   ") == []


# ---------------------------------------------------------------------------
# El catálogo conectado al generador de RIPS
#
# Cargar un maestro que nadie consulta no arregla nada. Esto comprueba que el
# generador lo usa de verdad.
# ---------------------------------------------------------------------------

class TestElRecolectorComprueba:
    """`Recolector.codigo` distingue lo que falta de lo que no vale."""

    def _recolector(self, permitir=False):
        from rips_completo import Recolector
        return Recolector(permitir_supuestos=permitir)

    def _llamar(self, rec, valor):
        return rec.codigo(valor, validador=cum.validar, archivo="medicamentos",
                          registro_id=1, campo="codigo_cum",
                          etiqueta="CUM del medicamento", supuesto="00000000")

    def test_un_cum_valido_pasa_sin_anotar_nada(self):
        rec = self._recolector()
        assert self._llamar(rec, "20048021-41") == "20048021-41"
        assert rec.faltantes == []

    def test_un_cum_que_no_existe_se_anota_diciendo_por_que(self):
        """Lo que antes pasaba intacto y hacía rechazar la factura entera."""
        rec = self._recolector(permitir=True)
        self._llamar(rec, "99999999-99")
        assert len(rec.faltantes) == 1
        assert "no está en el CatalogoCUMs" in rec.faltantes[0].motivo

    def test_un_cum_invalido_se_devuelve_TAL_CUAL_y_no_como_supuesto(self):
        """Sustituirlo por «00000000» escondería lo que el operador escribió,
        que es justo el dato que necesita ver para corregirlo."""
        rec = self._recolector(permitir=True)
        assert self._llamar(rec, "99999999-99") == "99999999-99"

    def test_la_ausencia_sigue_distinguiendose_de_la_invalidez(self):
        rec = self._recolector(permitir=True)
        self._llamar(rec, None)
        assert len(rec.faltantes) == 1
        assert rec.faltantes[0].motivo is None, (
            "un dato ausente no lleva motivo: el motivo es lo que separa "
            "«falta» de «está y no vale»"
        )

    def test_sin_permitir_supuestos_el_cierre_revienta_con_el_cum_invalido(self):
        from rips_completo import DatosIncompletos
        rec = self._recolector(permitir=False)
        self._llamar(rec, "99999999-99")
        with pytest.raises(DatosIncompletos):
            rec.cerrar()
