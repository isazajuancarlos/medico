"""La procedencia del dato, y la derivación que la hace soportable.

Quitar los defectos falsos, solo, hace la captura más lenta; y bajo presión de
tiempo lo lento se rodea. La contrapartida es saber de dónde vino cada dato y
derivar lo que ya se puede saber.
"""
import os
import sys

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_aislamiento.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from derivaciones import completar_municipio, municipio_desde_texto  # noqa: E402
from procedencia import (  # noqa: E402
    Procedencia,
    de,
    faltantes,
    marcar,
    marcar_varios,
    respaldados,
    sin_respaldo,
    tiene_valor,
)


class Registro:
    def __init__(self, **k):
        self.procedencias = None
        for a, v in k.items():
            setattr(self, a, v)


class TestMarcar:
    def test_marcar_no_toca_el_valor(self):
        r = Registro(cups="890201")
        marcar(r, "cups", Procedencia.CONFIRMADO)
        assert r.cups == "890201"
        assert de(r, "cups") is Procedencia.CONFIRMADO

    def test_sin_marcar_no_hay_procedencia(self):
        assert de(Registro(cups="890201"), "cups") is None

    def test_se_puede_cambiar_la_procedencia(self):
        """Un derivado que una persona revisa pasa a confirmado."""
        r = Registro(cups="890201")
        marcar(r, "cups", Procedencia.DERIVADO)
        marcar(r, "cups", Procedencia.CONFIRMADO)
        assert de(r, "cups") is Procedencia.CONFIRMADO

    def test_reasigna_el_dict_entero(self):
        """SQLAlchemy no detecta la mutación en sitio de una columna JSON: sin
        reasignar, el cambio se perdería en el commit sin decir nada."""
        r = Registro(a=1, b=2)
        marcar(r, "a", Procedencia.CONFIRMADO)
        primero = r.procedencias
        marcar(r, "b", Procedencia.DERIVADO)
        assert r.procedencias is not primero
        assert set(r.procedencias) == {"a", "b"}

    def test_marcar_varios(self):
        r = Registro(a=1, b=2, c=3)
        marcar_varios(r, ["a", "b"], Procedencia.PROPUESTO)
        assert de(r, "a") is de(r, "b") is Procedencia.PROPUESTO
        assert de(r, "c") is None


class TestTieneValor:
    @pytest.mark.parametrize("v", [None, "", "   ", 0])
    def test_lo_vacio_no_cuenta(self, v):
        assert not tiene_valor(Registro(x=v), "x")

    @pytest.mark.parametrize("v", ["890201", 1, 45000, "0"])
    def test_lo_que_hay_si_cuenta(self, v):
        assert tiene_valor(Registro(x=v), "x")


class TestSinRespaldo:
    def test_valor_sin_procedencia_es_lo_que_se_busca(self):
        """Un valor que nadie afirmó. La firma de un defecto."""
        r = Registro(cups="890201", valor=45000)
        marcar(r, "valor", Procedencia.CONFIRMADO)
        assert sin_respaldo(r, ["cups", "valor"]) == ["cups"]

    def test_un_campo_vacio_NO_es_sin_respaldo(self):
        """Son problemas distintos: uno se captura, el otro se verifica."""
        assert sin_respaldo(Registro(cups=None), ["cups"]) == []

    def test_generaliza_a_campos_que_nadie_listo(self):
        """Es la ventaja sobre la lista escrita a mano: sirve para cualquier
        campo, incluidos los que se añadan mañana."""
        r = Registro(campo_nuevo="lo que sea")
        assert sin_respaldo(r, ["campo_nuevo"]) == ["campo_nuevo"]


class TestFaltantes:
    def test_un_campo_vacio_falta(self):
        assert faltantes(Registro(x=None), ["x"]) == ["x"]

    def test_salvo_que_se_haya_dicho_que_no_aplica(self):
        """«No se midió el sangrado» y «no sangró» no son lo mismo."""
        r = Registro(sangrado=None)
        marcar(r, "sangrado", Procedencia.NO_APLICA)
        assert faltantes(r, ["sangrado"]) == []

    def test_respaldados_son_los_defendibles(self):
        r = Registro(a="x", b="y", c=None)
        marcar(r, "a", Procedencia.DERIVADO)
        assert respaldados(r, ["a", "b", "c"]) == ["a"]


class TestDerivarMunicipio:
    def test_una_ciudad_inequivoca_se_deriva(self):
        assert municipio_desde_texto("Medellín") == "05001"
        assert municipio_desde_texto("Bogotá D.C.") == "11001"

    def test_sin_tildes_ni_mayusculas_tambien(self):
        assert municipio_desde_texto("medellin") == "05001"

    def test_un_nombre_ambiguo_NO_se_adivina(self):
        """Hay varios «San Juan». Elegir el primero sería inventar, que es
        exactamente el problema que se está resolviendo."""
        assert municipio_desde_texto("San Juan") is None

    def test_el_departamento_desambigua(self):
        con = municipio_desde_texto("San Juan", "Cesar")
        sin = municipio_desde_texto("San Juan")
        assert sin is None
        assert con is None or con.startswith("20")

    def test_vacio_devuelve_None(self):
        for v in (None, "", "   "):
            assert municipio_desde_texto(v) is None

    def test_una_ciudad_inexistente_devuelve_None(self):
        assert municipio_desde_texto("Ciudad Gótica") is None


class TestCompletarMunicipio:
    def test_deriva_y_marca(self):
        p = Registro(cod_municipio=None, ciudad_residencia="Medellín",
                     departamento_residencia="Antioquia")
        assert completar_municipio(p) == "05001"
        assert p.cod_municipio == "05001"
        assert de(p, "cod_municipio") is Procedencia.DERIVADO

    def test_NO_pisa_lo_que_una_persona_ya_puso(self):
        """Un dato confirmado manda sobre uno derivado, siempre."""
        p = Registro(cod_municipio="05360", ciudad_residencia="Medellín")
        marcar(p, "cod_municipio", Procedencia.CONFIRMADO)

        assert completar_municipio(p) is None
        assert p.cod_municipio == "05360"
        assert de(p, "cod_municipio") is Procedencia.CONFIRMADO

    def test_si_no_puede_derivar_no_inventa(self):
        p = Registro(cod_municipio=None, ciudad_residencia="San Juan")
        assert completar_municipio(p) is None
        assert p.cod_municipio is None
        assert de(p, "cod_municipio") is None
