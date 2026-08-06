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


def test_un_token_que_dice_ES256_se_rechaza_sin_tocar_ecdsa():
    """La pareja del anterior, y la que de verdad mide.

    Sin lista blanca, un atacante manda un token cuya CABECERA dice `ES256` y
    fuerza el camino de curva elíptica del lado del servidor. `decode_token` pasa
    `algorithms=[JWT_ALGORITHM]`, así que se rechaza antes de tocar cripto.
    """
    from core.security import decode_token
    from fastapi import HTTPException

    cabecera = base64.urlsafe_b64encode(
        json.dumps({"alg": "ES256", "typ": "JWT"}).encode()
    ).rstrip(b"=")
    cuerpo = base64.urlsafe_b64encode(json.dumps({"sub": "1"}).encode()).rstrip(b"=")
    token = (cabecera + b"." + cuerpo + b"." + b"AAAA").decode()

    with pytest.raises(HTTPException):
        decode_token(token)

    # Y el control que hace que esto signifique algo: `ecdsa` no llegó a cargarse.
    assert "ecdsa" not in sys.modules, (
        "se cargó `ecdsa` al procesar el token: el camino vulnerable SÍ se recorre"
    )


def test_un_token_bueno_sigue_entrando():
    """Control positivo: sin esto, un `decode_token` que rechazara TODO pasaría
    las dos pruebas de arriba y el banco no diría nada.
    """
    from core.security import create_access_token, decode_token

    t = create_access_token({"sub": "42"})
    assert decode_token(t)["sub"] == "42"
