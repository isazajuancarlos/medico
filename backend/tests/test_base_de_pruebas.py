"""La base de las pruebas la fija el conftest, y solo el conftest.

Existe porque durante meses cada módulo declaró la suya en su cabecera:

    os.environ.setdefault("DATABASE_URL", "sqlite:///./test_login.db")

Once nombres distintos, y `setdefault` no pisa lo ya puesto. Como pytest importa
todo en un solo proceso, ganaba el primero por orden alfabético y los otros diez
nombres no existían nunca: sus módulos escribían en una base ajena creyendo
tener la suya. **Once declarados, dos ficheros en disco.**

Lo peor no era el desorden: las pruebas de AISLAMIENTO POR INSTITUCIÓN eran de
las que compartían base, y son las que respaldan que una institución no vea los
datos de otra.

Esta prueba es lo que impide que vuelva. Un comentario en el conftest se lee una
vez; esto se ejecuta siempre.
"""
import ast
from pathlib import Path

AQUI = Path(__file__).resolve().parent


def _declara_variable(nodo, nombre: str) -> bool:
    """¿Es `os.environ.setdefault("<nombre>", …)` o `os.environ["<nombre>"] = …`?"""
    if isinstance(nodo, ast.Expr) and isinstance(nodo.value, ast.Call):
        f = nodo.value.func
        if (isinstance(f, ast.Attribute) and f.attr == "setdefault"
                and isinstance(f.value, ast.Attribute) and f.value.attr == "environ"
                and nodo.value.args
                and isinstance(nodo.value.args[0], ast.Constant)
                and nodo.value.args[0].value == nombre):
            return True
    if isinstance(nodo, ast.Assign):
        for destino in nodo.targets:
            if (isinstance(destino, ast.Subscript)
                    and isinstance(destino.value, ast.Attribute)
                    and destino.value.attr == "environ"
                    and isinstance(destino.slice, ast.Constant)
                    and destino.slice.value == nombre):
                return True
    return False


def test_ningun_modulo_declara_su_propia_DATABASE_URL():
    culpables = []
    for py in sorted(AQUI.glob("test_*.py")):
        arbol = ast.parse(py.read_text(encoding="utf-8"))
        for nodo in ast.walk(arbol):
            if _declara_variable(nodo, "DATABASE_URL"):
                culpables.append(f"{py.name}:{nodo.lineno}")

    assert not culpables, (
        "La base de las pruebas la fija `conftest.py`, que pytest carga antes que "
        "cualquier módulo. Declararla aquí no la cambia —el conftest ya la puso— "
        "pero hace creer que este módulo tiene base propia, y no la tiene. "
        f"Reintroducida en: {culpables}"
    )


def test_la_base_de_esta_corrida_es_temporal_y_no_del_repositorio():
    """La suite no puede escribir dentro del repositorio.

    Un `.db` que sobrevive a la corrida hace que un fallo dependa de si alguien
    ejecutó las pruebas antes — el tipo de intermitencia que se acaba culpando
    al azar.
    """
    import os

    url = os.environ["DATABASE_URL"]
    assert url.startswith("sqlite:///"), f"se esperaba SQLite de pruebas, hay: {url}"

    ruta = Path(url.removeprefix("sqlite:///"))
    assert ruta.is_absolute(), (
        f"la base de pruebas debe ser una ruta absoluta a un temporal, no {ruta}: "
        "una relativa depende del directorio desde el que se invoque pytest"
    )
    repo = AQUI.parent.parent
    assert repo not in ruta.parents, (
        f"la base de pruebas ({ruta}) está DENTRO del repositorio {repo}"
    )
