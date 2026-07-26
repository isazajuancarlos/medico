"""CIE-11: que derive lo cierto y se niegue a inventar lo demás.

Lo que estas pruebas fijan no es «el mapeo funciona», sino la frontera: dónde
el sistema puede escribir solo y dónde tiene que callarse y preguntar. Es la
misma frontera de [[procedencia]], aplicada al dato que la Resolución 948 de
2026 vuelve obligatorio.
"""
import os
import sys

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import cie11  # noqa: E402
import procedencia as proc  # noqa: E402


class Historia:
    """Lo mínimo que toca `completar`."""

    def __init__(self, dx="A09X", **k):
        self.diagnostico_principal = dx
        self.diagnostico_principal_cie11 = None
        self.diagnostico_principal_uri = None
        self.procedencias = None
        for a, v in k.items():
            setattr(self, a, v)


class TestElCatalogoEstaCompleto:
    def test_cubre_casi_todo_el_catalogo_del_RIPS(self):
        """Si una regeneración deja el mapeo a medias, se nota aquí y no seis
        meses después en un RIPS rechazado."""
        assert cie11.total_mapeados() > 12_000

    def test_declara_contra_que_version_se_genero(self):
        """Sin esto no se puede saber si el mapeo envejeció."""
        assert "release/11/" in cie11.release()


class TestNormalizar:
    @pytest.mark.parametrize("entrada,esperado", [
        ("A00.0", "A000"),      # la forma de la OMS
        ("a00.0", "A000"),      # minúsculas de un copiar-pegar
        ("  A000  ", "A000"),
        ("A09", "A09"),         # NO se rellena con X a ciegas
        ("", ""),
        (None, ""),
    ])
    def test_deja_la_forma_del_RIPS(self, entrada, esperado):
        assert cie11.normalizar(entrada) == esperado

    def test_no_completa_con_X_por_su_cuenta(self):
        """`C83` no es `C83X`: C83 tiene subdivisiones y `C83X` no existe.
        Rellenar aquí fabricaría un código que el validador rechaza."""
        assert cie11.normalizar("C83") == "C83"


class TestBuscar:
    def test_encuentra_un_codigo_del_RIPS(self):
        eq = cie11.buscar("A09X")
        assert eq is not None
        assert eq.destinos
        assert all(d.codigo and d.uri for d in eq.destinos)

    def test_acepta_la_forma_corta_de_una_categoria(self):
        """Fuera del RIPS se escribe `A09`; dentro, `A09X`. Las dos valen al
        consultar, y llevan a lo mismo."""
        assert cie11.buscar("A09") == cie11.buscar("A09X")

    def test_un_codigo_inexistente_da_None_y_no_algo_parecido(self):
        assert cie11.buscar("ZZ99") is None

    @pytest.mark.parametrize("vacio", [None, "", "   "])
    def test_vacio_da_None(self, vacio):
        assert cie11.buscar(vacio) is None

    def test_toda_equivalencia_trae_la_URI_de_la_OMS(self):
        """El código cambia entre versiones; la FoundationURI no. Un registro
        con solo el código deja de ser interpretable en la próxima revisión."""
        for codigo in ("A000", "A09X", "E119", "I10X"):
            eq = cie11.buscar(codigo)
            if eq:
                assert all(d.uri.startswith("http://id.who.int/icd/entity/")
                           for d in eq.destinos), codigo


class TestLaFronteraDeLoQueSePuedeDerivar:
    def test_exacto_y_unico_se_deriva(self):
        eq = cie11.buscar("A000")
        assert eq.nivel == "exacto"
        assert not eq.ambigua
        assert eq.derivable

    def test_un_codigo_con_varios_destinos_NO_es_derivable(self):
        """Elegir el primero de la lista es fabricar un diagnóstico."""
        ambiguos = [c for c in ("Q922", "Y32X", "H57X", "R458")
                    if (e := cie11.buscar(c)) and e.ambigua]
        assert ambiguos, "no se encontró ningún caso ambiguo conocido"
        for c in ambiguos:
            assert not cie11.buscar(c).derivable
            assert cie11.derivar(c) is None

    def test_una_equivalencia_por_categoria_NO_es_derivable_aunque_sea_unica(self):
        """Rebajar la especificidad en silencio pierde un dato clínico sin que
        nadie se entere. Se propone, no se pone."""
        porcat = [c for c in cie11._cargar()["mapa"]
                  if cie11._cargar()["mapa"][c]["nivel"] == "categoria"
                  and len(cie11._cargar()["mapa"][c]["destinos"]) == 1]
        assert porcat, "el mapeo no tiene ninguna entrada por categoría"
        eq = cie11.buscar(porcat[0])
        assert eq.nivel == "categoria"
        assert eq.via == porcat[0][:3]
        assert not eq.derivable
        assert cie11.derivar(porcat[0]) is None

    def test_pero_SI_se_ofrecen_como_candidatos(self):
        """No derivable no es lo mismo que sin ayuda: la lista se ofrece igual,
        y confirmarla cuesta una tecla."""
        ambiguo = next(c for c in cie11._cargar()["mapa"]
                       if len(cie11._cargar()["mapa"][c]["destinos"]) > 1)
        assert len(cie11.candidatos(ambiguo)) > 1


class TestInverso:
    def test_de_un_CIE11_se_vuelve_a_los_CIE10(self):
        """Hace falta para leer lo que llegue codificado en CIE-11 de otra
        institución y poder seguir facturando en CIE-10."""
        destino = cie11.derivar("A000")
        assert "A000" in cie11.inverso(destino.codigo)

    def test_uno_desconocido_da_lista_vacia_no_error(self):
        assert cie11.inverso("NOEXISTE") == []


class TestCompletar:
    def test_deriva_y_deja_la_procedencia(self):
        h = Historia(dx="A000")
        cie11.completar(h)
        assert h.diagnostico_principal_cie11 == cie11.derivar("A000").codigo
        assert proc.de(h, "diagnostico_principal_cie11") is proc.Procedencia.DERIVADO

    def test_guarda_tambien_la_URI(self):
        h = Historia(dx="A000")
        cie11.completar(h)
        assert h.diagnostico_principal_uri.startswith("http://id.who.int/icd/entity/")
        assert proc.de(h, "diagnostico_principal_uri") is proc.Procedencia.DERIVADO

    def test_ante_ambiguedad_NO_escribe_pero_devuelve_los_candidatos(self):
        ambiguo = next(c for c in cie11._cargar()["mapa"]
                       if len(cie11._cargar()["mapa"][c]["destinos"]) > 1)
        h = Historia(dx=ambiguo)
        eq = cie11.completar(h)
        assert h.diagnostico_principal_cie11 is None, "eligió por su cuenta"
        assert eq is not None and eq.ambigua
        assert len(eq.destinos) > 1

    def test_NO_pisa_lo_que_alguien_confirmo(self):
        """La tabla ayuda; no manda sobre el criterio clínico."""
        h = Historia(dx="A000", diagnostico_principal_cie11="XX99")
        proc.marcar(h, "diagnostico_principal_cie11", proc.Procedencia.CONFIRMADO)
        cie11.completar(h)
        assert h.diagnostico_principal_cie11 == "XX99"
        assert proc.de(h, "diagnostico_principal_cie11") is proc.Procedencia.CONFIRMADO

    def test_SI_pisa_una_derivacion_anterior(self):
        """Si cambia el diagnóstico, el CIE-11 derivado del anterior es basura
        y tiene que actualizarse solo."""
        h = Historia(dx="A000")
        cie11.completar(h)
        h.diagnostico_principal = "A001"
        cie11.completar(h)
        assert h.diagnostico_principal_cie11 == cie11.derivar("A001").codigo

    def test_un_diagnostico_sin_mapeo_deja_el_campo_VACIO(self):
        """Vacío y señalado, nunca relleno. Ver [[directiva-fallar-en-vez-de-suponer]]."""
        h = Historia(dx="ZZ99")
        assert cie11.completar(h) is None
        assert h.diagnostico_principal_cie11 is None

    def test_sin_diagnostico_no_revienta(self):
        h = Historia(dx=None)
        assert cie11.completar(h) is None

    def test_el_campo_derivado_queda_sin_respaldo_si_nadie_lo_marco(self):
        """La prueba de que procedencia caza esto: un CIE-11 escrito por fuera
        de `completar` aparece como valor que nadie afirmó."""
        h = Historia(dx="A000")
        h.diagnostico_principal_cie11 = "1A00"   # a mano, sin marcar
        assert proc.sin_respaldo(h, ["diagnostico_principal_cie11"]) == [
            "diagnostico_principal_cie11"]
        cie11.completar(h)
        assert proc.sin_respaldo(h, ["diagnostico_principal_cie11"]) == []


class TestCoherenciaDelMapeo:
    def test_ningun_destino_se_repite_dentro_de_un_codigo(self):
        mapa = cie11._cargar()["mapa"]
        for codigo, entrada in list(mapa.items())[:2000]:
            codigos = [d["codigo"] for d in entrada["destinos"]]
            assert len(codigos) == len(set(codigos)), codigo

    def test_el_inverso_es_coherente_con_el_directo(self):
        mapa = cie11._cargar()["mapa"]
        for codigo in list(mapa)[:500]:
            for destino in cie11.candidatos(codigo):
                assert codigo in cie11.inverso(destino.codigo)

    def test_todo_codigo_mapeado_esta_en_la_tabla_del_RIPS(self):
        """El mapeo no puede introducir códigos CIE-10 que Colombia no usa."""
        import json
        from pathlib import Path
        ruta = Path(cie11.RUTA).parent / "cie10_rips.json"
        validos = {c["codigo"] for c in json.loads(ruta.read_text(encoding="utf-8"))["codigos"]}
        assert set(cie11._cargar()["mapa"]) <= validos


class TestNoFallaEnSilencio:
    def test_un_modelo_sin_la_columna_CIE11_revienta(self):
        """Sin columna, `setattr` crea un atributo suelto y el sistema parece
        derivar sin guardar nada. Se descubriría con el RIPS rechazado."""
        class SinColumna:
            diagnostico_principal = "A000"
        with pytest.raises(AttributeError) as e:
            cie11.completar(SinColumna())
        assert "diagnostico_principal_cie11" in str(e.value)

    def test_el_modelo_Historia_SI_tiene_las_columnas(self):
        from models import Historia
        assert "diagnostico_principal_cie11" in Historia.model_fields
        assert "diagnostico_principal_uri" in Historia.model_fields


# ---------------------------------------------------------------------------
# El CIE-11 dentro del RIPS
#
# La 948 pide 22 campos CIE-11 y los admite vacíos: sus propios ejemplos traen
# "" y null, porque el periodo paralelo no obliga a tenerlos todavía. Eso hace
# que rellenarlos mal sea peor que dejarlos en blanco — nadie lo rechaza y
# entra como dato clínico falso.
# ---------------------------------------------------------------------------

class TestElParQueEmiteElRips:
    def _par(self, cie10):
        import sys, os
        sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
        from rips_completo import _par_cie11
        return _par_cie11(cie10)

    def test_una_equivalencia_exacta_y_unica_se_rellena_con_su_titulo(self):
        codigo, titulo = self._par("I10")
        assert codigo == "BA00.Z"
        assert titulo, "el nomCod*CIE11 sale del propio mapeo, no se pide aparte"

    def test_lo_ambiguo_va_VACIO_en_vez_de_elegir_el_primero(self):
        """Escoger un candidato de varios sería inventar un diagnóstico."""
        import cie11
        ambiguo = next(
            (c for c in ("E119", "Z00") if (e := cie11.buscar(c)) and not e.derivable),
            None,
        )
        if ambiguo is None:
            pytest.skip("no hay un CIE-10 ambiguo a mano en el mapeo")
        assert self._par(ambiguo) == ("", "")

    def test_un_codigo_desconocido_no_revienta_ni_inventa(self):
        assert self._par("XXXX") == ("", "")
        assert self._par(None) == ("", "")
        assert self._par("") == ("", "")

    def test_una_categoria_de_tres_no_se_da_por_buena(self):
        """`derivable` exige nivel exacto además de destino único: rebajar un
        diagnóstico de cuatro caracteres a su categoría pierde especificidad
        clínica sin que nadie se entere."""
        import cie11
        porCategoria = next(
            (c for c in cie11._cargar()["mapa"]
             if (e := cie11.buscar(c)) and e.nivel == "categoria" and len(e.destinos) == 1),
            None,
        )
        if porCategoria is None:
            pytest.skip("el mapeo no trae equivalencias por categoría")
        assert self._par(porCategoria) == ("", ""), (
            "una equivalencia hallada por categoría no es exacta: no se rellena"
        )
