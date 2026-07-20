"""Corregir un paciente ya creado.

`PacienteUpdate` dejaba fuera `identificacion`, `tipo_id`, `fecha_nacimiento`,
`sexo` y `cod_municipio`. No era una salvaguarda: son precisamente los campos de
los que depende el RIPS —la edad y el sexo mandan en la clasificación, el
documento es la llave del usuario— y un error de digitación en ellos quedaba
para siempre. En el FoxPro se corregían sin más.

Y usaba `exclude_none=True`, así que mandar `null` no borraba: no había forma de
quitar un correo mal escrito.
"""
import os
import sys
from datetime import date

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_aislamiento.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import main  # noqa: E402
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    EstadoLicencia, Institucion, Licencia, Paciente, RolUsuario, TipoLicencia,
    Usuario,
)

RUTA_BD = "./test_pacientes_edicion.db"


@pytest.fixture
def cliente():
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)

    with Session(motor) as db:
        db.add(Institucion(
            id=1, nit="900000001", razon_social="Consultorio",
            nombre_comercial="Consultorio", direccion="calle 1",
            ciudad="Medellín", departamento="Antioquia", telefono="600",
            email="c@x.co", representante_legal="R",
            identificacion_representante="1", codigo_prestador="050011",
        ))
        db.add(Licencia(
            institucion_id=1, tipo_licencia=TipoLicencia.BASICA,
            estado=EstadoLicencia.ACTIVA, fecha_inicio=date(2026, 1, 1),
            fecha_vencimiento=date(2027, 1, 1),
            modulos_habilitados=["PACIENTES"], clave_licencia="k1",
            hash_validacion="h",
        ))
        db.add(Paciente(
            id=10, institucion_id=1, identificacion="1111", nombre1="Ana",
            apellido1="Restrepo", fecha_nacimiento=date(1990, 3, 14), sexo="M",
            celular="300", email="viejo@x.co",
        ))
        db.add(Paciente(
            id=11, institucion_id=1, identificacion="2222", nombre1="Otro",
            apellido1="Paciente", fecha_nacimiento=date(1980, 1, 1), sexo="F",
            celular="301",
        ))
        db.commit()

    usuario = Usuario(
        id=1, username="medico1", hashed_password="h",
        nombre_completo="Médico", email="m@x.co", rol=RolUsuario.MEDICO,
        institucion_id=1, activo=True,
    )

    def sesion():
        with Session(motor) as db:
            yield db

    main.app.dependency_overrides[get_current_active_user] = lambda: usuario
    main.app.dependency_overrides[get_db] = sesion
    with TestClient(main.app, raise_server_exceptions=False) as c:
        yield c
    main.app.dependency_overrides.clear()
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


def test_se_corrige_la_fecha_de_nacimiento(cliente):
    """De ella dependen la edad reportada y toda la clasificación etaria."""
    r = cliente.put("/api/pacientes/10", json={"fecha_nacimiento": "1990-03-15"})
    assert r.status_code == 200
    assert r.json()["fecha_nacimiento"] == "1990-03-15"


def test_se_corrige_el_sexo(cliente):
    r = cliente.put("/api/pacientes/10", json={"sexo": "F"})
    assert r.status_code == 200
    assert r.json()["sexo"] == "F"


def test_se_corrige_el_documento(cliente):
    r = cliente.put("/api/pacientes/10", json={"identificacion": "9999", "tipo_id": "TI"})
    assert r.status_code == 200
    assert r.json()["identificacion"] == "9999"
    assert r.json()["tipo_id"] == "TI"


def test_NO_se_puede_duplicar_el_documento_de_otro(cliente):
    """Corregir no puede convertirse en pisar a otro paciente."""
    r = cliente.put("/api/pacientes/10", json={"identificacion": "2222"})
    assert r.status_code == 400
    assert "identificación" in r.json()["detail"].lower()


def test_dejar_el_mismo_documento_no_se_toma_por_duplicado(cliente):
    r = cliente.put("/api/pacientes/10", json={"identificacion": "1111", "celular": "302"})
    assert r.status_code == 200
    assert r.json()["celular"] == "302"


def test_mandar_null_BORRA_el_campo(cliente):
    """Antes era imposible: `exclude_none` confundía 'bórralo' con 'no lo mando'."""
    r = cliente.put("/api/pacientes/10", json={"email": None})
    assert r.status_code == 200
    assert r.json()["email"] is None


def test_lo_que_no_se_manda_no_se_toca(cliente):
    r = cliente.put("/api/pacientes/10", json={"celular": "305"})
    assert r.status_code == 200
    cuerpo = r.json()
    assert cuerpo["celular"] == "305"
    assert cuerpo["email"] == "viejo@x.co", "un campo ausente no debe borrarse"
    assert cuerpo["nombre1"] == "Ana"


# --- Municipio: la validación contra DIVIPOLA en el camino real ------------

def test_crear_con_municipio_sin_el_cero_lo_normaliza(cliente):
    """Es lo que llega de una hoja de cálculo o de un sistema viejo."""
    r = cliente.post("/api/pacientes/", json={
        "identificacion": "3333", "nombre1": "Nuevo", "apellido1": "Paciente",
        "fecha_nacimiento": "1990-01-01", "sexo": "M", "celular": "300",
        "cod_municipio": "5001",
    })
    assert r.status_code == 201, r.text
    assert r.json()["cod_municipio"] == "05001"


def test_crear_con_un_municipio_inventado_se_rechaza(cliente):
    r = cliente.post("/api/pacientes/", json={
        "identificacion": "4444", "nombre1": "Nuevo", "apellido1": "Paciente",
        "fecha_nacimiento": "1990-01-01", "sexo": "M", "celular": "300",
        "cod_municipio": "99999",
    })
    assert r.status_code == 422
    assert "DIVIPOLA" in r.text


def test_crear_sin_municipio_se_rechaza(cliente):
    """Antes se colaba con el defecto "05001" y salía en el RIPS como Medellín."""
    r = cliente.post("/api/pacientes/", json={
        "identificacion": "5555", "nombre1": "Nuevo", "apellido1": "Paciente",
        "fecha_nacimiento": "1990-01-01", "sexo": "M", "celular": "300",
    })
    assert r.status_code == 422


def test_leer_un_paciente_viejo_con_municipio_malo_NO_falla(cliente):
    """Si leerlo fallara, no habría forma de abrirlo para corregirlo."""
    r = cliente.get("/api/pacientes/10")
    assert r.status_code == 200
