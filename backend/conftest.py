"""Configuración común de las pruebas. Lo primero que pytest importa.

# El problema que resuelve

Cada módulo de prueba hacía esto en su cabecera:

    os.environ.setdefault("DATABASE_URL", "sqlite:///./test_login.db")

Once nombres distintos, y `setdefault` no pisa lo que ya está puesto. Como
pytest importa los módulos en un solo proceso y por orden alfabético, **ganaba
el primero**: `test_aislamiento.db`. Los otros diez nombres no llegaban a
existir nunca, y sus módulos escribían en una base ajena creyendo tener la suya.

Medido antes de arreglarlo: **11 nombres declarados, 2 ficheros en disco.**

Que las pruebas de AISLAMIENTO POR INSTITUCIÓN fueran las que compartían base no
es una ironía menor: son las que respaldan que una institución no vea los datos
de otra. Verificar eso sobre una base en la que han escrito otros diez módulos
es un verde que no significa nada.

# Qué hace este fichero

Fija la base ANTES de que se importe nada —`conftest.py` es lo primero que
carga pytest— y la pone en un directorio temporal propio de cada corrida:

- Se asigna con `=`, no con `setdefault`: aquí mandamos nosotros, y los
  `setdefault` de los módulos quedan en no-ops.
- Al terminar la sesión se borra. No quedan `.db` sueltos en el repositorio
  guardando datos de la corrida anterior — que es lo que hacía que un fallo
  dependiera de si alguien había corrido las pruebas antes.
- Y cierra un riesgo que estaba abierto: `database.py` hace `load_dotenv()` del
  `.env` del proyecto. Si por orden de importación ninguna cabecera hubiera
  fijado `DATABASE_URL`, la suite habría corrido contra la base REAL del `.env`.
  Con esto es imposible.

# Lo que este arreglo NO resuelve, y hay que decirlo

Los módulos siguen compartiendo UNA base durante la corrida. Deja de ser un
accidente —es una sola, declarada y efímera— pero un módulo todavía puede ver
filas que dejó otro. El aislamiento de verdad es una transacción por prueba con
reversión al terminar, y exige tocar cómo cada módulo obtiene su sesión.

Nueve módulos ya se lo montan aparte con su propio `create_engine`; esos no
dependen de esto.
"""
import os
import shutil
import tempfile
from pathlib import Path

#: Directorio de esta corrida. Se crea al importar el conftest, o sea antes de
#: que cualquier módulo de prueba toque `database`.
_TEMPORAL = Path(tempfile.mkdtemp(prefix="medico_pruebas_"))

os.environ["DATABASE_URL"] = f"sqlite:///{_TEMPORAL / 'pruebas.db'}"

#: Con `setdefault` a propósito: si alguien exporta una clave para reproducir un
#: caso concreto, se respeta. La base NO admite esa cortesía; la clave sí.
os.environ.setdefault(
    "SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok"
)


def pytest_sessionfinish(session, exitstatus):  # noqa: ARG001
    """Se lleva la base de la corrida.

    Si el borrado falla no se interrumpe nada: perder un directorio temporal es
    ruido, y hacer fallar una suite verde por eso sería peor que el problema.
    """
    shutil.rmtree(_TEMPORAL, ignore_errors=True)
