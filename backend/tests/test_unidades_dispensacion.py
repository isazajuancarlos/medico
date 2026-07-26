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
from rips_completo import UNIDAD_MIN_DISPENSA, _unidad_min_dispensa  # noqa: E402


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
    datos = catalogo()
    assert {d["codigo"] for d in datos} == set(UNIDAD_MIN_DISPENSA.values())
    for d in datos:
        assert UNIDAD_MIN_DISPENSA[d["nombre"]] == d["codigo"], (
            f"«{d['nombre']}» no está en el mapa con el código {d['codigo']}")


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
