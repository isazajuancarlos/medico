"""El JWT se firma con HS256, y la lista blanca del decodificador lo exige.

No es una prueba de estilo: es lo que hace INALCANZABLE la vulnerabilidad de
`ecdsa` (Minerva, ataque de temporización sobre P-256), que **no tiene parche**
—el autor documenta que Python puro no puede ser de tiempo constante— y que
llega al árbol como dependencia de `python-jose`.

El razonamiento con el que se descarta esa alerta es: «este sistema nunca ejecuta
una operación ECDSA». Eso es cierto HOY y depende de dos cosas que un cambio de
una línea podría deshacer sin que nadie lo note — cambiar `JWT_ALGORITHM` a
`ES256`, o abrir la lista de `algorithms` del `decode`. Este archivo convierte
ese razonamiento en algo que se cae solo si deja de ser verdad (directiva 6:
prueba que falla > directiva que recordar).
"""
import os
import sys

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import base64  # noqa: E402
import json  # noqa: E402

import pytest  # noqa: E402
from jose import JWTError, jwt  # noqa: E402

from core.config import JWT_ALGORITHM  # noqa: E402


def test_el_algoritmo_del_jwt_es_simetrico():
    """HS256: HMAC-SHA256. No toca curva elíptica en ningún momento."""
    assert JWT_ALGORITHM == "HS256", (
        f"El JWT pasó a firmarse con «{JWT_ALGORITHM}». Si es un algoritmo de curva "
        "elíptica (ES*), el sistema empieza a ejecutar `ecdsa`, que tiene una "
        "vulnerabilidad de temporización SIN PARCHE (Minerva, P-256) y cuya alerta "
        "se descartó precisamente porque ese camino no se recorría. Revísala antes "
        "de cambiar esto."
    )


ALGORITMOS_QUE_NO_SON_EL_NUESTRO = [
    "none", "NONE", "nOnE",          # el clásico: firma vacía, y sus disfraces
    "ES256", "ES384", "ES512",       # curva elíptica: el camino de `ecdsa`
    "RS256", "PS256", "EdDSA",       # asimétricos: HMAC contra clave pública
    "HS384", "HS512",                # de la misma familia, pero NO el declarado
]


@pytest.mark.parametrize("alg", ALGORITMOS_QUE_NO_SON_EL_NUESTRO)
def test_ningun_otro_algoritmo_entra(alg):
    """La pareja del anterior, y la que de verdad mide.

    RECORRE UNA LISTA Y NO UN VALOR, y esa es la corrección del 2026-08-06: la
    versión anterior fabricaba SOLO un token `ES256`, así que abrir la lista
    blanca a otra cosa le era invisible. Medido por la revisión de seguridad de
    esta rama: con `algorithms=["HS256", "RS256"]` el banco seguía **3 en
    verde**, y el mutante no era inerte —un token RS256 escapaba como `JWKError`,
    o sea un 500 donde antes había un 401 limpio—.
    """
    from core.security import decode_token
    from fastapi import HTTPException

    cabecera = base64.urlsafe_b64encode(
        json.dumps({"alg": alg, "typ": "JWT"}).encode()
    ).rstrip(b"=")
    cuerpo = base64.urlsafe_b64encode(json.dumps({"sub": "1"}).encode()).rstrip(b"=")
    # Sin firma y con firma basura: `none` se ataca de las dos formas.
    for firma in (b"", b"AAAA"):
        token = (cabecera + b"." + cuerpo + b"." + firma).decode()
        with pytest.raises(HTTPException) as e:
            decode_token(token)
        assert e.value.status_code == 401, (
            f"un token que dice «{alg}» no dio 401 sino {e.value.status_code}: "
            "una credencial inválida tiene que responder 401, no reventar"
        )


def test_la_lista_blanca_es_EXACTAMENTE_el_algoritmo_declarado():
    """El guardián del mutante que se escapó, y va a la causa y no al síntoma.

    La prueba de arriba mira la CONDUCTA (todo lo forjado da 401), y eso deja de
    discriminar en cuanto el manejo de errores mejora: con `except JOSEError`, un
    token RS256 responde 401 tanto si `RS256` está en la lista blanca como si no.
    O sea que arreglar el 500 habría vuelto INVISIBLE el ensanchamiento de la
    lista — el arreglo de un fallo tapando la señal de otro (directiva 23).

    Así que esto no mira la respuesta: captura el argumento `algorithms` con el
    que `decode_token` llama de verdad a `jose`, y exige que sea la lista de UN
    elemento. Añadir cualquier algoritmo la pone roja, salga lo que salga por la
    API.
    """
    import core.security as seguridad

    visto = {}
    original = seguridad.jwt.decode

    def espia(token, key, **kw):
        visto.update(kw)
        return original(token, key, **kw)

    seguridad.jwt.decode = espia
    try:
        t = seguridad.create_access_token({"sub": "42"})
        seguridad.decode_token(t)
    finally:
        seguridad.jwt.decode = original

    assert visto.get("algorithms") == [JWT_ALGORITHM], (
        f"`decode_token` acepta {visto.get('algorithms')} y no [{JWT_ALGORITHM!r}]. "
        "Cada algoritmo de más es un camino de verificación que nadie eligió: "
        "un ES* enciende `ecdsa` (Minerva, sin parche) y un asimétrico abre la "
        "confusión de algoritmo clásica —HMAC verificado contra clave pública—."
    )
    assert visto, "el espía no llegó a ejecutarse: la prueba no midió nada"


def test_el_respaldo_EC_de_python_jose_es_el_de_cryptography():
    """Lo que la aserción anterior CREÍA medir, medido de verdad.

    Aquí había `assert "ecdsa" not in sys.modules`, presentada como «el control
    que hace que esto signifique algo». La revisión de seguridad demostró que
    **no puede fallar**: `jose.backends` toma `CryptographyECKey` y solo cae al
    respaldo `ecdsa` si el `import cryptography` falla. Se comprobó por la vía
    fuerte —firmando Y verificando un ES256 real con una clave P-256, o sea el
    camino de curva elíptica recorrido entero— y `'ecdsa' in sys.modules` seguía
    siendo False. Una aserción que no puede ponerse roja no es un control: es
    ruido que da confianza (directiva 33).

    Lo que sí importa es CUÁL respaldo elegiría `python-jose` si algún día se
    ejecutara una operación EC. Con `cryptography` presente es OpenSSL, que es de
    tiempo constante; sin ella, el `ecdsa` en Python puro, que es el vulnerable a
    Minerva y NO tiene parche. Esta línea se pone roja el día que `cryptography`
    desaparezca del entorno — que es el único camino real hacia esa vulnerabilidad
    en este repositorio.
    """
    from jose.backends import ECKey

    assert ECKey.__module__.endswith("cryptography_backend"), (
        f"`python-jose` elegiría el respaldo EC de «{ECKey.__module__}». Si es "
        "`ecdsa_backend`, el sistema pasó a poder ejecutar ECDSA en Python puro, "
        "que es vulnerable a Minerva (temporización sobre P-256) y no tiene "
        "parche. Comprueba que `cryptography` sigue instalada."
    )


def test_un_token_bueno_sigue_entrando():
    """Control positivo: sin esto, un `decode_token` que rechazara TODO pasaría
    las dos pruebas de arriba y el banco no diría nada.
    """
    from core.security import create_access_token, decode_token

    t = create_access_token({"sub": "42"})
    assert decode_token(t)["sub"] == "42"
