"""El inicio de sesión y la migración de contraseñas.

Antes había tres copias de bcrypt con parámetros propios. Ahora hay una cadena
de esquemas: se escribe con Argon2id y bcrypt solo verifica, para que ningún
usuario existente se quede fuera el día del despliegue.

Un hash es de una sola vía: la contraseña solo está en claro cuando el usuario
la escribe. Si no se recifra en ese instante, no se recifra nunca.
"""
import os
import sys

import bcrypt
import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, select  # noqa: E402

import main  # noqa: E402
from database import engine  # noqa: E402
from core.deps import get_db  # noqa: E402
from core.passwords import (  # noqa: E402
    CADENA, avance_migracion, hash_password, verificar_y_migrar, verify_password,
)
from models import RolUsuario, Usuario  # noqa: E402


def _bcrypt_heredado(clave: str) -> str:
    """Un hash tal como lo escribía el sistema antes de este cambio."""
    return bcrypt.hashpw(clave.encode(), bcrypt.gensalt(rounds=12)).decode()


@pytest.fixture
def db():
    """Sesión sobre el MOTOR REAL de la aplicación.

    Con un motor propio en memoria las pruebas fallaban con «no such table»:
    parte del camino de la petición usa el motor global de `database`, así que
    dos motores distintos dejan la mitad de la aplicación mirando una base
    vacía. Se crean y se borran las tablas del de verdad.
    """
    SQLModel.metadata.create_all(engine)
    with Session(engine) as s:
        yield s
    SQLModel.metadata.drop_all(engine)


@pytest.fixture
def cliente(db):
    main.app.dependency_overrides[get_db] = lambda: db
    yield TestClient(main.app)
    main.app.dependency_overrides.clear()


def _crear(db, username, hashed, activo=True, bloqueada=False):
    u = Usuario(nombre_completo="Prueba", username=username, email=f"{username}@x.co",
                hashed_password=hashed, rol=RolUsuario.MEDICO, institucion_id=1,
                activo=activo, cuenta_bloqueada=bloqueada)
    db.add(u)
    db.commit()
    db.refresh(u)
    return u


# --- Lo nuevo se escribe con el esquema fuerte -------------------------------

def test_las_contrasenas_nuevas_son_argon2id():
    assert hash_password("secreta").startswith("$argon2id$")


def test_no_queda_ninguna_copia_suelta_de_bcrypt():
    """El motivo de core/passwords.py: tres copias con parámetros propios
    significan que endurecer el algoritmo exige acordarse de tres sitios."""
    import crear_usuarios_multi_inst
    import routers.auth
    import seed

    for modulo in (routers.auth, seed, crear_usuarios_multi_inst):
        assert not hasattr(modulo, "bcrypt"), \
            f"{modulo.__name__} volvió a importar bcrypt por su cuenta"

    assert seed.get_password_hash("x").startswith("$argon2id$")
    assert crear_usuarios_multi_inst.hash_password("x").startswith("$argon2id$")


# --- Nadie se queda fuera ----------------------------------------------------

def test_el_usuario_heredado_en_bcrypt_todavia_entra(cliente, db):
    _crear(db, "viejo", _bcrypt_heredado("secreta"))
    r = cliente.post("/api/auth/login", json={"username": "viejo", "password": "secreta"})
    assert r.status_code == 200


def test_al_entrar_su_hash_QUEDA_MIGRADO_EN_LA_BASE(cliente, db):
    """La prueba central: devolver el hash nuevo no basta, hay que guardarlo."""
    _crear(db, "viejo", _bcrypt_heredado("secreta"))
    cliente.post("/api/auth/login", json={"username": "viejo", "password": "secreta"})

    db.expire_all()
    guardado = db.exec(select(Usuario).where(Usuario.username == "viejo")).one().hashed_password
    assert guardado.startswith("$argon2id$"), "el hash migrado debe persistirse"


def test_despues_de_migrar_la_misma_clave_sigue_sirviendo(cliente, db):
    _crear(db, "viejo", _bcrypt_heredado("secreta"))
    cliente.post("/api/auth/login", json={"username": "viejo", "password": "secreta"})
    db.expire_all()
    r = cliente.post("/api/auth/login", json={"username": "viejo", "password": "secreta"})
    assert r.status_code == 200


# --- Y lo que debe seguir fallando -------------------------------------------

def test_la_clave_incorrecta_no_entra_ni_toca_el_hash(cliente, db):
    original = _bcrypt_heredado("secreta")
    _crear(db, "viejo", original)

    r = cliente.post("/api/auth/login", json={"username": "viejo", "password": "mala"})
    assert r.status_code == 401

    db.expire_all()
    guardado = db.exec(select(Usuario).where(Usuario.username == "viejo")).one().hashed_password
    assert guardado == original, "una clave errónea jamás debe reescribir el hash bueno"


def test_la_cuenta_inactiva_no_entra(cliente, db):
    _crear(db, "baja", hash_password("secreta"), activo=False)
    assert cliente.post("/api/auth/login",
                        json={"username": "baja", "password": "secreta"}).status_code == 403


def test_la_cuenta_bloqueada_no_entra(cliente, db):
    _crear(db, "bloq", hash_password("secreta"), bloqueada=True)
    assert cliente.post("/api/auth/login",
                        json={"username": "bloq", "password": "secreta"}).status_code == 403


def test_un_hash_irreconocible_niega_el_acceso_sin_reventar(cliente, db):
    _crear(db, "raro", "{SHA}rastroDeUnSistemaViejo")
    r = cliente.post("/api/auth/login", json={"username": "raro", "password": "lo-que-sea"})
    assert r.status_code == 401, "debe negar el paso, no propagar una excepción"


# --- El informe de avance ----------------------------------------------------

def test_el_avance_delata_la_base_sin_migrar(db):
    for i in range(3):
        _crear(db, f"viejo{i}", _bcrypt_heredado("secreta"))

    avance = avance_migracion(db)
    assert avance.total == 3
    assert avance.pendientes == 3
    assert not avance.completo


def test_el_avance_se_cierra_cuando_todos_han_entrado(cliente, db):
    for i in range(3):
        _crear(db, f"viejo{i}", _bcrypt_heredado("secreta"))
    for i in range(3):
        cliente.post("/api/auth/login", json={"username": f"viejo{i}", "password": "secreta"})

    db.expire_all()
    avance = avance_migracion(db)
    assert avance.pendientes == 0
    assert avance.completo, "con todos migrados ya se puede retirar bcrypt de la cadena"


def test_cien_inicios_de_sesion_migran_una_sola_vez(cliente, db):
    """Recifrar en cada entrada regalaría un Argon2 completo por inicio de
    sesión, sin ganar nada."""
    _crear(db, "viejo", _bcrypt_heredado("secreta"))

    cliente.post("/api/auth/login", json={"username": "viejo", "password": "secreta"})
    db.expire_all()
    tras_migrar = db.exec(select(Usuario).where(Usuario.username == "viejo")).one().hashed_password

    for _ in range(100):
        r = cliente.post("/api/auth/login", json={"username": "viejo", "password": "secreta"})
        assert r.status_code == 200

    db.expire_all()
    guardado = db.exec(select(Usuario).where(Usuario.username == "viejo")).one().hashed_password
    assert guardado == tras_migrar, "el hash no debe cambiar en cada entrada una vez migrado"


# --- Cambio de contraseña ----------------------------------------------------

def test_cambiar_la_contrasena_la_guarda_en_argon2id(db):
    u = _crear(db, "quien", _bcrypt_heredado("vieja"))
    assert verify_password("vieja", u.hashed_password), "la actual se verifica contra bcrypt"
    u.hashed_password = hash_password("nueva")
    db.add(u)
    db.commit()

    db.expire_all()
    guardado = db.exec(select(Usuario).where(Usuario.username == "quien")).one().hashed_password
    assert guardado.startswith("$argon2id$")


def test_verificar_y_migrar_no_hace_commit_por_su_cuenta(db):
    """La librería no decide cuándo escribir: eso lo sabe quien tiene la sesión
    y sabe si la petición terminó bien."""
    u = _crear(db, "viejo", _bcrypt_heredado("secreta"))
    assert verificar_y_migrar("secreta", u)
    assert u.hashed_password.startswith("$argon2id$"), "cambia el objeto en memoria"

    db.rollback()
    db.expire_all()
    guardado = db.exec(select(Usuario).where(Usuario.username == "viejo")).one().hashed_password
    assert guardado.startswith("$2b$"), "sin commit del llamante, la base no cambia"
