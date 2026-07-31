"""Configuración, y la comprobación de que existe de verdad.

=== POR QUÉ ESTO NO SON TRES `os.getenv` Y YA ===

Hasta hoy este archivo decía:

    SECRET_KEY: str = os.getenv("SECRET_KEY", "changeme")
    JWT_SECRET_KEY: str = os.getenv("JWT_SECRET_KEY", SECRET_KEY)

Con eso, un despliegue sin `.env` —un contenedor nuevo, una variable mal
escrita, un `.env` borrado por creerlo un sobrante— **arrancaba igual y sin
decir nada**, firmando los tokens de sesión con la cadena `changeme`. Que es
pública, está en este repositorio y la sabe cualquiera que lo lea.

Un token firmado con una clave conocida se falsifica. Se puede fabricar uno de
ADMIN de cualquier institución, y a partir de ahí el aislamiento por institución
—todo `core/aislamiento.py`— no protege nada: el intruso no está saltándose el
filtro, está pasando por la puerta con una credencial que él mismo emitió.

`seguridad.py` tenía el mismo patrón para la clave de cifrado de datos
sensibles, con el valor `default_insecure_key_change_in_production`.

=== LA REGLA: FALLAR AL ARRANCAR, NO DEGRADARSE ===

No hay valor por defecto para un secreto. Si falta, el proceso no arranca y
dice exactamente qué hacer. No hay «modo desarrollo» que lo permita, porque ese
atajo es justamente el camino por el que `changeme` llega a producción: alguien
lo activa una vez para probar y nadie lo desactiva.

Es preferible un servicio que no levanta —se nota en un minuto— a uno que
levanta con una credencial falsificable, que no se nota nunca.
"""

import os
import secrets
from pathlib import Path

from dotenv import load_dotenv

_BASE_DIR = Path(__file__).resolve().parent.parent
load_dotenv(_BASE_DIR.parent / ".env")


class ConfiguracionInsegura(RuntimeError):
    """Falta un secreto, o el que hay es uno de los conocidos."""


#: Valores que alguna vez estuvieron como defecto en este código, o que son el
#: relleno habitual. Si aparecen, es que nadie configuró nada.
#:
#: Los dos últimos son los que `docker-compose.yml` traía incrustados hasta el
#: 2026-07-31: pasaban esta comprobación —ni se parecen a «changeme» ni son
#: cortos—, así que `docker compose up -d` levantaba el backend firmando JWT con
#: una clave del repositorio. Ya no vienen en el compose, pero se prohíben
#: igualmente porque siguen en el historial de git: quien los copiara antes tiene
#: que enterarse al reiniciar, no dentro de un año.
CONOCIDOS = frozenset({
    "changeme", "change-me", "secret", "supersecret", "clave", "password",
    "default_insecure_key_change_in_production",
    "your-secret-key-here-change-in-production",
    "change-me-in-production-use-64-random-characters",
    "medsys_docker_secret_key_cambie_en_produccion_2026",
    "medsys_jwt_secret_cambie_en_produccion_2026_muy_largo",
})

#: Un HS256 con menos de esto se ataca por fuerza bruta fuera de línea a partir
#: de un solo token capturado.
LARGO_MINIMO = 32


def _exigir(nombre: str, *, minimo: int = LARGO_MINIMO) -> str:
    valor = os.getenv(nombre, "").strip()

    if not valor:
        raise ConfiguracionInsegura(
            f"Falta la variable de entorno {nombre}.\n"
            f"\n"
            f"  Genere una y póngala en el archivo .env de la raíz:\n"
            f"      echo '{nombre}={secrets.token_urlsafe(48)}' >> .env\n"
            f"\n"
            f"  No hay valor por defecto a propósito: uno inseguro dejaría el "
            f"servicio en pie firmando sesiones con una clave que está en el "
            f"código, y eso no se nota hasta que alguien lo aprovecha."
        )

    if valor.lower() in CONOCIDOS:
        raise ConfiguracionInsegura(
            f"{nombre} tiene el valor «{valor}», que es uno de los de relleno y "
            f"está publicado en este repositorio. Cualquiera puede firmar un "
            f"token de administrador con él. Genere uno propio:\n"
            f"      {secrets.token_urlsafe(48)}"
        )

    if len(valor) < minimo:
        raise ConfiguracionInsegura(
            f"{nombre} tiene {len(valor)} caracteres y hacen falta al menos "
            f"{minimo}. Con menos, quien capture un solo token puede sacar la "
            f"clave por fuerza bruta sin conexión y firmar los suyos."
        )

    return valor


DATABASE_URL: str = os.getenv("DATABASE_URL", "")
SECRET_KEY: str = _exigir("SECRET_KEY")
#: Si no se da aparte, se usa la misma. Es aceptable: ambas son secretos del
#: servidor y comparten ciclo de vida.
JWT_SECRET_KEY: str = os.getenv("JWT_SECRET_KEY", "").strip() or SECRET_KEY
if JWT_SECRET_KEY is not SECRET_KEY:
    JWT_SECRET_KEY = _exigir("JWT_SECRET_KEY")

#: Cifrado de datos sensibles en reposo. Si no se define, se deriva de
#: SECRET_KEY, que ya está comprobada; lo que NO puede es tener un defecto
#: propio y público como tenía.
ENCRYPTION_KEY: str = os.getenv("ENCRYPTION_KEY", "").strip() or SECRET_KEY

JWT_ALGORITHM: str = "HS256"
ACCESS_TOKEN_EXPIRE_MINUTES: int = 15
REFRESH_TOKEN_EXPIRE_DAYS: int = 7
UPLOAD_DIR: str = str(_BASE_DIR.parent / "uploads")
