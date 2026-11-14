"""El único sitio del sistema que sabe hashear contraseñas.

Antes había TRES copias de bcrypt —`routers/auth.py`, `seed.py` y
`crear_usuarios_multi_inst.py`—, cada una con sus propios parámetros. Tres
copias significan que endurecer el algoritmo exige acordarse de tres sitios, y
tarde o temprano uno se queda atrás sin que nadie lo note.

Se escribe con Argon2id. bcrypt sigue en la cadena SOLO para verificar, para no
dejar fuera a los usuarios creados antes de este cambio; se retira cuando
`avance_migracion()` diga que ya no queda ninguno.

Un hash no se puede revertir: no existe forma de migrar las contraseñas con una
tarea en segundo plano. El único instante en que la contraseña está en claro es
cuando el usuario la escribe para entrar, así que la migración ocurre ahí o no
ocurre.
"""

from __future__ import annotations

import logging

from tumbaga import Argon2id, Bcrypt, EsquemaDesconocido, Tumbaga

logger = logging.getLogger(__name__)

CADENA = Tumbaga([
    Argon2id(),
    Bcrypt(rondas=12),  # solo verifica
])


def hash_password(password: str) -> str:
    """Hash nuevo, siempre con el esquema más fuerte de la cadena."""
    return CADENA.cifrar(password)


def verify_password(plain: str, hashed: str) -> bool:
    """Verifica sin actualizar.

    Para el inicio de sesión úsese `verificar_y_migrar`, que además sube el hash
    de esquema. Este queda para comprobaciones sueltas —confirmar la contraseña
    actual antes de cambiarla— donde no hay nada que persistir.
    """
    try:
        return CADENA.verificar(plain, hashed).valida
    except (EsquemaDesconocido, ValueError):
        return False


def verificar_y_migrar(plain: str, usuario) -> bool:
    """Verifica y deja el hash del usuario al día, sin hacer commit.

    Modifica `usuario.hashed_password` en memoria; el commit lo hace quien tiene
    la sesión, que es quien sabe si la petición terminó bien. Si nadie persiste,
    el sistema sigue funcionando y no migra nada — por eso existe
    `avance_migracion()`, que es lo único capaz de delatarlo.
    """
    try:
        resultado = CADENA.verificar(plain, usuario.hashed_password)
    except (EsquemaDesconocido, ValueError):
        # Hash corrupto o de un esquema ya retirado: no se deja entrar, pero se
        # registra, porque significa que un usuario legítimo quedó fuera.
        logger.error(
            "El hash del usuario %s no lo reconoce ningún esquema de la cadena",
            getattr(usuario, "id", "?"),
        )
        return False

    if not resultado.valida:
        return False

    if resultado.hash_nuevo:
        usuario.hashed_password = resultado.hash_nuevo
        logger.info(
            "Contraseña del usuario %s migrada de %s a %s",
            getattr(usuario, "id", "?"), resultado.esquema, CADENA.escritor.nombre,
        )
    return True


def avance_migracion(db):
    """Cuántas contraseñas siguen en el esquema antiguo.

    Es lo que autoriza a retirar bcrypt de la cadena: mientras no esté completo,
    retirarlo deja fuera a quien todavía no ha vuelto a iniciar sesión.
    """
    from sqlmodel import select

    from models import Usuario

    hashes = [u.hashed_password for u in db.exec(select(Usuario)).all() if u.hashed_password]
    return CADENA.avance(hashes)
