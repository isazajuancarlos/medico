"""DIVIPOLA, y sobre todo el cero a la izquierda.

`codMunicipioResidencia` va de cinco dígitos. Antioquia es el 05 y Atlántico el
08: 148 municipios del país —Medellín incluida— tienen código que empieza por
cero, y el conjunto de datos de MinSalud los publica sin él porque pasaron por
una hoja de cálculo. Cargar de ahí sin normalizar deja mal reportado al 13 % de
los pacientes del país.
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from divipola import (  # noqa: E402
    LARGO_CODIGO,
    MunicipioNoValido,
    buscar,
    buscar_por_nombre,
    departamentos,
    municipios_de,
    normalizar,
    total,
    validar,
)


class TestElCeroALaIzquierda:
    """El fallo que motivó todo esto."""

    def test_medellin_es_05001_y_no_5001(self):
        assert normalizar("5001") == "05001"
        assert normalizar(5001) == "05001"
        assert buscar("5001").nombre == "MEDELLÍN"

    def test_todos_los_codigos_miden_cinco(self):
        from divipola import _cargar
        municipios, _ = _cargar()
        malos = [c for c in municipios if len(c) != LARGO_CODIGO]
        assert not malos, f"códigos que no miden 5: {malos[:10]}"

    def test_hay_148_que_empiezan_por_cero(self):
        """Si este número baja, alguien volvió a cargar sin normalizar."""
        from divipola import _cargar
        municipios, _ = _cargar()
        con_cero = [c for c in municipios if c.startswith("0")]
        assert len(con_cero) == 148

    def test_solo_antioquia_y_atlantico_los_tienen(self):
        from divipola import _cargar
        municipios, _ = _cargar()
        deptos = {m.nombre_departamento for m in municipios.values()
                  if m.codigo.startswith("0")}
        assert deptos == {"ANTIOQUIA", "ATLÁNTICO"}

    def test_un_codigo_que_ya_viene_bien_no_se_toca(self):
        assert normalizar("11001") == "11001"   # Bogotá


class TestLasDosFuentesOficiales:
    """DANE y MinSalud discrepan en dos municipios. Se aceptan los cuatro."""

    @pytest.mark.parametrize("codigo,nombre", [
        ("27086", "Belén De Bajirá"),          # MinSalud
        ("27493", "NUEVO BELÉN DE BAJIRÁ"),    # DANE
        ("94663", "Mapiripana"),               # MinSalud
        ("94885", "LA GUADALUPE"),             # DANE
    ])
    def test_los_cuatro_codigos_en_disputa_son_validos(self, codigo, nombre):
        m = buscar(codigo)
        assert m is not None, f"{codigo} debería aceptarse"
        assert m.nombre == nombre

    def test_se_sabe_cual_esta_en_la_lista_del_MUV(self):
        """El MUV valida contra la lista de MinSalud. Saber de dónde viene cada
        código permite avisar antes de enviar, en vez de comerse el rechazo."""
        assert buscar("27086").en_minsalud is True
        assert buscar("27493").en_minsalud is False


class TestValidar:
    def test_un_codigo_bueno_vuelve_normalizado(self):
        assert validar("5001") == "05001"

    def test_un_codigo_inexistente_levanta(self):
        with pytest.raises(MunicipioNoValido) as e:
            validar("99999")
        assert "99999" in str(e.value)

    def test_vacio_levanta_y_NO_pone_uno_por_defecto(self):
        """Es el fallo original: `cod_municipio` tenía "05001" de defecto."""
        for vacio in (None, "", "   "):
            with pytest.raises(MunicipioNoValido) as e:
                validar(vacio)
            assert "por defecto" in str(e.value)

    def test_el_mensaje_dice_por_que_importa(self):
        with pytest.raises(MunicipioNoValido) as e:
            validar(None)
        assert "no vive" in str(e.value)


class TestCobertura:
    def test_estan_los_33_departamentos(self):
        assert len(departamentos()) == 33

    def test_estan_los_1124_municipios(self):
        """1.122 del DANE más los 2 que solo publica MinSalud."""
        assert total() == 1124

    def test_antioquia_tiene_125_municipios(self):
        assert len(municipios_de("05")) == 125

    def test_el_departamento_tambien_se_normaliza(self):
        assert municipios_de(5) == municipios_de("05")

    def test_cada_municipio_sabe_su_departamento(self):
        from divipola import _cargar
        municipios, _ = _cargar()
        huerfanos = [m.codigo for m in municipios.values()
                     if m.nombre_departamento == "—"]
        assert not huerfanos, f"sin departamento: {huerfanos}"

    def test_el_codigo_empieza_por_el_de_su_departamento(self):
        from divipola import _cargar
        municipios, _ = _cargar()
        malos = [m.codigo for m in municipios.values()
                 if not m.codigo.startswith(m.departamento)]
        assert not malos, f"código y departamento no concuerdan: {malos[:5]}"


class TestBusqueda:
    """Quien captura escribe rápido y sin tildes."""

    def test_encuentra_sin_tildes(self):
        assert any(m.codigo == "05001" for m in buscar_por_nombre("medellin"))

    def test_encuentra_sin_mayusculas(self):
        assert any(m.codigo == "11001" for m in buscar_por_nombre("bogota"))

    def test_los_que_empiezan_por_el_texto_van_primero(self):
        resultado = buscar_por_nombre("bello")
        assert resultado[0].nombre.lower().startswith("bello")

    def test_una_busqueda_vacia_no_devuelve_el_pais_entero(self):
        assert buscar_por_nombre("") == []
        assert buscar_por_nombre("   ") == []

    def test_respeta_el_limite(self):
        assert len(buscar_por_nombre("san", limite=5)) == 5
