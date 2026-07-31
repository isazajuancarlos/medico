"""Un ADMIN no puede fabricar un SUPERADMIN, y con ello salirse del aislamiento.

Los dos extremos de actualización de usuario —`PUT /api/usuarios/{id}` y
`PUT /api/admin/usuarios/{id}`— hacían `setattr` de todos los campos del payload,
y los dos esquemas exponen `rol`. El aislamiento por institución estaba bien
hecho y no servía de nada aquí: el ascenso ocurre DENTRO de la institución y su
efecto ocurre FUERA, porque `puede_ver()` no acota al superadmin. Un admin de un
consultorio se ascendía a sí mismo y leía la historia clínica de todos los demás
clientes del sistema.

Las pruebas recorren el camino HTTP real y comprueban el código de respuesta; no
leen el código. Y la primera es el control: sin ella, unas pruebas que solo
esperan 403 pasarían igual si el extremo estuviera roto y devolviera 403 a todo.
"""
import os
import sys
from datetime import date

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import main  # noqa: E402
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    EstadoLicencia, Institucion, Licencia, RolUsuario, TipoLicencia, Usuario,
)

#: Archivo propio, por lo mismo que explica `test_aislamiento_institucion.py`:
#: la base de `DATABASE_URL` la comparten todos los módulos del proceso y
#: borrarla en el teardown deja a los demás sin poder escribir.
RUTA_BD = "./test_ascenso_datos.db"

#: Quién pide el cambio. Se sustituye por prueba con `dependency_overrides`.
_QUIEN = {}


@pytest.fixture
def entorno():
    """Una institución, un admin, un superadmin y una cuenta a la que ascender."""
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)

    with Session(motor) as db:
        db.add(Institucion(
            id=1, nit="900000001", razon_social="Consultorio Uno",
            nombre_comercial="Consultorio Uno", direccion="calle 1",
            ciudad="Medellín", departamento="Antioquia", telefono="600",
            email="1@x.co", representante_legal="R",
            identificacion_representante="1", codigo_prestador="050011",
        ))
        # Cupo holgado: lo que se prueba es el ascenso, no la licencia. Si el
        # tope estuviera lleno, un 403 del cupo se confundiría con el del rol.
        db.add(Licencia(
            institucion_id=1, tipo_licencia=TipoLicencia.BASICA,
            estado=EstadoLicencia.ACTIVA,
            fecha_inicio=date(2026, 1, 1), fecha_vencimiento=date(2027, 1, 1),
            modulos_habilitados=["PACIENTES", "CITAS", "HISTORIA"],
            clave_licencia="clave-1", hash_validacion="h",
            max_usuarios=50, max_medicos=50,
        ))
        db.add(Usuario(id=1, username="admin1", hashed_password="h",
                       nombre_completo="Admin de la Uno", email="admin@x.co",
                       rol=RolUsuario.ADMIN, institucion_id=1, activo=True))
        db.add(Usuario(id=2, username="aux1", hashed_password="h",
                       nombre_completo="Auxiliar", email="aux@x.co",
                       rol=RolUsuario.AUXILIAR_ENFERMERIA, institucion_id=1,
                       activo=True))
        db.add(Usuario(id=3, username="dueno", hashed_password="h",
                       nombre_completo="Dueño del sistema", email="root@x.co",
                       rol=RolUsuario.SUPERADMIN, institucion_id=None,
                       activo=True))
        db.commit()

    admin = Usuario(id=1, username="admin1", hashed_password="h",
                    nombre_completo="Admin de la Uno", email="admin@x.co",
                    rol=RolUsuario.ADMIN, institucion_id=1, activo=True)
    _QUIEN["usuario"] = admin

    def sesion():
        with Session(motor) as db:
            yield db

    main.app.dependency_overrides[get_current_active_user] = lambda: _QUIEN["usuario"]
    main.app.dependency_overrides[get_db] = sesion

    with TestClient(main.app, raise_server_exceptions=False) as c:
        yield c

    main.app.dependency_overrides.clear()
    _QUIEN.clear()
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


def _como_superadmin():
    _QUIEN["usuario"] = Usuario(
        id=3, username="dueno", hashed_password="h",
        nombre_completo="Dueño del sistema", email="root@x.co",
        rol=RolUsuario.SUPERADMIN, institucion_id=None, activo=True,
    )


def test_el_admin_SI_puede_cambiar_un_rol_corriente(entorno):
    """Control. Un admin sube a alguien de auxiliar a enfermería a diario:
    cerrar eso sería prohibir lo inusual en vez de lo imposible."""
    r = entorno.put("/api/usuarios/2", json={"rol": "enfermeria"})
    assert r.status_code == 200, (
        f"devolvió {r.status_code}: la comprobación del ascenso está cerrando "
        f"un cambio de rol legítimo, que es el mismo fallo con el signo cambiado"
    )
    assert r.json()["rol"] == "enfermeria"


def test_el_admin_NO_puede_ascender_a_superadmin(entorno):
    r = entorno.put("/api/usuarios/2", json={"rol": "superadmin"})
    assert r.status_code == 403, (
        f"devolvió {r.status_code}: un admin fabricó un superadmin, que ve la "
        f"historia clínica de TODAS las instituciones"
    )


def test_el_admin_NO_puede_ascenderse_a_si_mismo(entorno):
    """El mismo camino, y el que de verdad se usaría."""
    r = entorno.put("/api/usuarios/1", json={"rol": "superadmin"})
    assert r.status_code == 403, (
        f"devolvió {r.status_code}: el admin se ascendió a sí mismo"
    )


def test_el_admin_NO_puede_ascender_por_el_extremo_de_administracion(entorno):
    """`/api/admin/usuarios/{id}` es el segundo camino, con su propio esquema.
    Cerrar solo uno de los dos deja la puerta abierta al lado."""
    r = entorno.put("/api/admin/usuarios/2", json={"rol": "superadmin"})
    assert r.status_code == 403, (
        f"devolvió {r.status_code}: la escalada pasa por /api/admin/usuarios"
    )


def test_el_superadmin_SI_puede_conceder_el_rol(entorno):
    """Quien vende el sistema tiene que poder dar de alta a otro dueño. Si esto
    fallara, la regla estaría prohibiendo la operación legítima."""
    _como_superadmin()
    r = entorno.put("/api/admin/usuarios/2", json={"rol": "superadmin"})
    assert r.status_code == 200, (
        f"devolvió {r.status_code}: el superadmin no puede conceder su propio rol"
    )
    assert r.json()["rol"] == "superadmin"


def test_degradar_a_un_superadmin_sigue_siendo_posible(entorno):
    """Se comprueba el DESTINO del cambio, no el origen: quitar autoridad no
    crea autoridad, y prohibirlo dejaría al dueño sin poder retirar la suya."""
    _como_superadmin()
    r = entorno.put("/api/admin/usuarios/3", json={"rol": "admin"})
    assert r.status_code == 200, f"devolvió {r.status_code}"
