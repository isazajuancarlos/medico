"""El catálogo de presentaciones que alimenta el formulario de prescripción.

Existe porque la lista tiene que vivir en UN sitio. El código de la unidad
mínima de dispensación (campo M13 del RIPS) determina cómo se leen el valor
unitario y la cantidad que van a su lado, así que una lista copiada al frontend
que se quede vieja no da un error: da una factura que dice otra cosa.

Es el mismo fallo que ya se cometió dos veces el 2026-07-26 en el guardián de
campos, con las claves de un ayudante copiadas a mano.

=== POR QUÉ NO SE PRUEBA POR HTTP ===

El endpoint cuelga del router de historias, que exige licencia activa y módulo
habilitado, y montar eso arrastra media base de datos para comprobar un
diccionario. Lo que hay que fijar es que el catálogo SE DERIVE del mapa, y eso
se ve mejor llamando a la función. El cableado HTTP del router ya lo cubren las
pruebas de historias.
"""
import os
import sys

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from routers.historias import unidades_dispensacion  # noqa: E402
import maestros_muv  # noqa: E402
from rips_completo import _unidad_min_dispensa  # noqa: E402


def catalogo():
    """El endpoint no usa el usuario para nada: solo lo exige la autenticación."""
    return unidades_dispensacion(current_user=None)


def test_devuelve_las_presentaciones_con_su_codigo():
    datos = catalogo()
    assert datos, "el catálogo no puede venir vacío"
    assert all({"codigo", "nombre"} == set(d) for d in datos)
    assert all(isinstance(d["codigo"], int) for d in datos)


def test_el_catalogo_SALE_del_mapa_y_no_de_una_lista_aparte():
    """Si alguien escribe las opciones a mano en el endpoint, esto lo caza.

    Es la prueba que justifica que el endpoint exista: servir la lista desde el
    backend solo aporta algo si de verdad se deriva de la misma fuente que usa
    el generador de RIPS.
    """
    maestro = {int(f["Codigo"]): f["Nombre"] for f in maestros_muv.cargar("upr")["filas"]}
    for d in catalogo():
        assert maestro[d["codigo"]] == d["nombre"], (
            f"«{d['nombre']}» no lleva el código {d['codigo']} en el maestro")
    assert {d["codigo"] for d in catalogo()} == set(maestro)


def test_no_repite_un_codigo_aunque_el_mapa_traiga_singular_y_plural():
    """El mapa tiene «tableta» y «tabletas» apuntando al mismo código. En una
    lista para elegir, dos entradas idénticas son un error de interfaz."""
    codigos = [d["codigo"] for d in catalogo()]
    assert len(codigos) == len(set(codigos))


def test_lo_que_ofrece_el_formulario_se_puede_traducir():
    """Cada opción tiene que sobrevivir el viaje de vuelta.

    Ofrecer en el desplegable algo que luego `_unidad_min_dispensa` no sepa
    traducir sería peor que no ofrecerlo: el médico creería haberlo registrado.
    """
    for d in catalogo():
        assert _unidad_min_dispensa(d["nombre"]) == d["codigo"]


def test_el_valor_por_defecto_viejo_NO_se_traduce():
    """`«unidad»` era el defecto del formulario y no es una presentación: es la
    ausencia de una. Por eso se quitó, y por eso no puede colarse de vuelta."""
    assert _unidad_min_dispensa("unidad") is None
    assert _unidad_min_dispensa(None) is None
    assert _unidad_min_dispensa("") is None
    assert "unidad" not in {d["nombre"] for d in catalogo()}


# ---------------------------------------------------------------------------
# La prueba que habría impedido el defecto
#
# El mapa anterior estaba escrito a mano y NINGUNO de sus cinco códigos
# coincidía con el catálogo. Pasó la validación del MUV porque ese campo se
# comprueba como rango (1 a 999), no contra la tabla.
# ---------------------------------------------------------------------------

CASOS_REALES = {
    "AMPOLLA": 1,
    "FRASCO": 13,
    "CAPSULA": 14,
    "TABLETA": 66,
    "VIAL": 75,
}


def test_los_codigos_son_los_del_catalogo_y_no_unos_inventados():
    """Los cinco que estuvieron mal, fijados contra el maestro."""
    for nombre, codigo in CASOS_REALES.items():
        assert _unidad_min_dispensa(nombre) == codigo, (
            f"{nombre} debe ser {codigo}. Si esto falla tras regenerar el "
            f"maestro, comprobar el cambio contra el validador antes de "
            f"actualizar la cifra aquí.")


def test_el_plural_y_las_tildes_no_rompen_la_traduccion():
    """El médico escribe «tabletas» y «dilución»; el catálogo, sin tilde."""
    assert _unidad_min_dispensa("tabletas") == 66
    assert _unidad_min_dispensa("  Tableta  ") == 66
    assert _unidad_min_dispensa("dilucion homeopatica") == \
        _unidad_min_dispensa("DILUCIÓN HOMEOPÁTICA")


def test_lo_que_no_esta_en_el_catalogo_no_se_parece_a_nada():
    """No hay traducción por semejanza: «gotero» no es «gotas»."""
    assert _unidad_min_dispensa("gotero") is None
    assert _unidad_min_dispensa("cucharadita") is None


def test_el_frasco_deshabilitado_no_entra():
    """`REFs_UPR` trae FRASCO dos veces: 13 habilitado y 40 no. Quedarse con
    el primero de la tabla habría dado el que ya no vale."""
    assert _unidad_min_dispensa("frasco") == 13
    assert 40 not in {d["codigo"] for d in catalogo()}


# ---------------------------------------------------------------------------
# Los tipos de documento salen del mismo maestro
#
# `REFs_TipoDoc` marca cada tipo con `UsuarioRips` y `RealizaAtencionRips`. La
# lista de profesionales estaba escrita a mano tras leer el PDF —y resultó
# correcta—, pero la de pacientes traía `NU` (NUIP), que el MUV no admite, y no
# traía `CN`, sin el cual un recién nacido no se puede registrar.
# ---------------------------------------------------------------------------

def test_el_profesional_no_admite_documentos_de_menor():
    """La regla que el MUV aplicó al rechazarnos las consultas de niños."""
    from models import TIPOS_DOCUMENTO_PROFESIONAL

    for imposible in ("RC", "TI", "CN", "AS", "MS"):
        assert imposible not in TIPOS_DOCUMENTO_PROFESIONAL


def test_las_dos_listas_salen_del_maestro_y_no_de_una_copia():
    from models import TIPOS_DOCUMENTO, TIPOS_DOCUMENTO_PROFESIONAL

    filas = maestros_muv.cargar("tipo_doc")["filas"]
    assert set(TIPOS_DOCUMENTO) == {
        f["TipoDocumentoIdentificacion"] for f in filas if f["UsuarioRips"]}
    assert set(TIPOS_DOCUMENTO_PROFESIONAL) == {
        f["TipoDocumentoIdentificacion"] for f in filas
        if f["RealizaAtencionRips"]}


def test_el_recien_nacido_puede_registrarse():
    """`CN` faltaba, y el RIPS tiene un archivo entero para recién nacidos."""
    from models import TIPOS_DOCUMENTO

    assert "CN" in TIPOS_DOCUMENTO


def test_el_NUIP_que_el_MUV_no_admite_ya_no_se_ofrece():
    from models import TIPOS_DOCUMENTO

    assert "NU" not in TIPOS_DOCUMENTO
