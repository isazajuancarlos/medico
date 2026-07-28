"""El mismo proceso sirve el API y el Angular compilado — sin taparse entre sí.

Es la decisión de la tarea #73: «el costo de Angular es de COMPILACIÓN, no de
despliegue; `ng build` produce estáticos que sirve el mismo FastAPI». Quita nginx
del despliegue en sitio: un proceso y un directorio.

=== LO QUE ESTAS PRUEBAS VIGILAN, Y POR QUÉ NO BASTA CON RECORDARLO ===

El montaje del SPA es un COMODÍN en `/`. Va después de los routers, así que todo
lo que el backend reconoce se resuelve antes — pero lo que el backend NO reconoce
cae en el comodín, y el comodín responde `index.html`.

Sin la guarda de `PREFIJOS_DEL_BACKEND`, `GET /api/pacintes` —con la errata—
devolvería **200 con HTML** en vez de 404. El cliente recibiría una página donde
espera JSON, y el error aparecería mucho más tarde y en otro sitio: exactamente
la clase de fallo silencioso que el resto de este backend evita a propósito
(los stubs devuelven 501 y no `200 []`, los secretos no tienen valor por defecto).

La guarda es un atajo que EVITA una comprobación, así que se le exige la prueba
de que discrimina: aquí se comprueba que deja pasar el 404 del API **y** que
sigue devolviendo el index.html en las rutas del enrutador de Angular.
"""
import os
import sys

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402

import main  # noqa: E402


#: `lifespan` toca la base y siembra; aquí no hace falta ninguna de las dos cosas.
cliente = TestClient(main.app)

hay_build = main.FRONTEND_DIR.is_dir()
sin_build = pytest.mark.skipif(
    not hay_build,
    reason=f"no hay build de Angular en {main.FRONTEND_DIR} (cd frontend && npm run build)",
)


# ---------------------------------------------------------------------
# La guarda: el comodín NO responde por el backend
# ---------------------------------------------------------------------

@pytest.mark.parametrize("ruta", [
    "/api/pacintes",              # errata de un cliente
    "/api/modulo-que-no-existe",
    "/api/pacientes/no-es-un-id/sub",
    "/uploads/no-existe.png",
])
def test_una_ruta_del_backend_que_no_existe_da_404_y_no_html(ruta):
    r = cliente.get(ruta)
    assert r.status_code == 404, (
        f"{ruta} devolvió {r.status_code}. Si es 200, el comodín del SPA está "
        "tapando el backend y una errata en la URL del API se lee como éxito."
    )
    assert "text/html" not in r.headers.get("content-type", "")


def test_los_prefijos_del_backend_incluyen_lo_que_publica_la_app():
    """Si mañana se monta un prefijo nuevo, tiene que entrar en la lista.

    No se comprueba la lista contra sí misma —eso no mediría nada—: se compara
    contra los prefijos que la aplicación PUBLICA de verdad, leídos del esquema
    OpenAPI.

    El esquema y no `app.routes`: en esta versión de FastAPI los routers
    incluidos quedan como objetos `_IncludedRouter` **sin atributo `path`**, así
    que recorrer `app.routes` ve `/docs`, `/health` y los montajes, y NO ve
    ninguna de las 180 rutas bajo `/api` — justo las que esta prueba existe para
    vigilar. Es el modo exacto en que una prueba pasa sin medir nada.
    """
    publicadas = list(main.app.openapi()["paths"])
    assert len(publicadas) > 100, (
        f"solo se leyeron {len(publicadas)} rutas del esquema; si la lectura se "
        "queda corta, el resto de esta prueba no significa nada"
    )

    expuestos = {p.split("/")[1] for p in publicadas if p != "/" and p.count("/") >= 1}
    assert "api" in expuestos, "la lectura no vio el prefijo /api: revisar cómo se mide"

    protegidos = {p.strip("/").split("/")[0] for p in main.PREFIJOS_DEL_BACKEND}
    sin_proteger = {e for e in expuestos if e and e not in protegidos}
    assert not sin_proteger, (
        f"Estos prefijos los sirve el backend y no están en PREFIJOS_DEL_BACKEND: "
        f"{sorted(sin_proteger)}. Una URL equivocada bajo ellos devolvería el "
        "index.html con 200 en vez de 404."
    )


def test_la_guarda_discrimina_de_verdad(monkeypatch):
    """Sin la guarda, la errata devuelve 200 con HTML. Medido, no supuesto.

    Una salvaguarda que nunca se ha visto fallar no está probada: está sin
    estrenar. Aquí se le quita el criterio y se comprueba que el fallo VUELVE.
    """
    assert cliente.get("/api/pacintes").status_code == 404

    monkeypatch.setattr(main, "PREFIJOS_DEL_BACKEND", ())
    r = cliente.get("/api/pacintes")

    if hay_build:
        assert r.status_code == 200 and "text/html" in r.headers["content-type"], (
            "sin la guarda esto debería devolver el index.html; si no lo hace, "
            "la guarda no es lo que está impidiendo el fallo y sobra"
        )
    else:
        assert r.status_code == 404  # sin comodín montado no hay nada que tapar


# ---------------------------------------------------------------------
# Y aun así el SPA sigue funcionando
# ---------------------------------------------------------------------

@sin_build
def test_la_raiz_sirve_el_index():
    r = cliente.get("/")
    assert r.status_code == 200
    assert "text/html" in r.headers["content-type"]


@sin_build
@pytest.mark.parametrize("ruta", ["/pacientes", "/pacientes/12", "/rips/generar"])
def test_una_ruta_del_enrutador_de_angular_devuelve_el_index(ruta):
    """Recargar la página o abrir un enlace directo tiene que funcionar.

    Angular resuelve `/pacientes/12` en el navegador: no hay archivo con ese
    nombre. Un 404 aquí rompe justo al compartir la dirección, que es la forma
    más incómoda de fallar — navegando dentro de la aplicación no se nota.
    """
    r = cliente.get(ruta)
    assert r.status_code == 200
    assert "text/html" in r.headers["content-type"]


@sin_build
def test_un_estatico_real_se_sirve_como_estatico_y_no_como_index():
    r = cliente.get("/index.html")
    assert r.status_code == 200
    assert "text/html" in r.headers["content-type"]


def test_health_sigue_respondiendo_por_debajo_del_comodin():
    r = cliente.get("/health")
    assert r.status_code == 200
    assert r.json()["status"] in {"ok", "error"}
