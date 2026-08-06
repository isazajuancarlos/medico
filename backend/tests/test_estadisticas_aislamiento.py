"""Estadísticas: ningún panel agrega datos de otra institución, y ninguno revienta.

Este archivo nace de dos cosas que encontró la revisión de seguridad del
2026-08-06 y que llevaban tiempo sin verse por el mismo motivo: **ninguna prueba
llamaba a estos endpoints**.

1. `_pac_sub` devolvía `None` —«sin filtro»— cuando el usuario no tenía
   institución, metiendo en la misma rama al SUPERADMIN (que no la tiene porque
   es el dueño) y a una cuenta a la que le falta el dato. Las cinco consultas que
   lo usan hacen `if sub is not None`, así que a la segunda le enseñaban los
   pacientes de todos los prestadores. Es el patrón «filtro opcional» que ya
   causó la fuga de `rips_completo.py`.

2. `GET /api/estadisticas/urgencias` respondía **500** con
   `AttributeError: type object 'TipoTriage' has no attribute 'ROJO'`. Los
   miembros son `TRIAGE_1`..`TRIAGE_5`; el color va en el valor.

**Por qué SIEMPRE hay dos instituciones**: con una sola, la consulta sin filtro
devuelve exactamente lo mismo que la filtrada.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import main  # noqa: E402
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    AtencionUrgencias, EstadoLicencia, Institucion, Licencia, Paciente,
    RolUsuario, TipoLicencia, TipoTriage, Usuario,
)
from routers import estadisticas as _estadisticas  # noqa: E402

RUTA_BD = "./test_estadisticas_aislamiento.db"
MODULOS = ["PACIENTES", "ESTADISTICAS", "URGENCIAS"]


def _sembrar(motor):
    with Session(motor) as db:
        for i in (1, 2):
            marca = "UNO" if i == 1 else "DOS"
            db.add(Institucion(
                id=i, nit=f"90000000{i}", razon_social=f"Prestador {i}",
                nombre_comercial=f"Prestador {i}", direccion="calle 1",
                ciudad="Medellín", departamento="Antioquia", telefono="600",
                email=f"{i}@x.co", representante_legal="R",
                identificacion_representante="1", codigo_prestador=f"05001{i}",
            ))
            db.add(Licencia(
                institucion_id=i, tipo_licencia=TipoLicencia.BASICA,
                estado=EstadoLicencia.ACTIVA,
                fecha_inicio=date(2026, 1, 1), fecha_vencimiento=date(2027, 1, 1),
                modulos_habilitados=MODULOS,
                clave_licencia=f"clave-{i}", hash_validacion="h",
            ))
            db.add(Paciente(
                id=100 * i, identificacion=f"doc{i}", tipo_id="CC",
                nombre1=f"Paciente{marca}", apellido1="Secreto",
                fecha_nacimiento=date(1990, 1, 1), sexo="M", institucion_id=i,
                celular="300", email=f"p{i}@x.co", direccion="calle",
            ))
            db.add(Usuario(
                id=50 + i, username=f"enf{i}", nombre_completo=f"Enfermera{marca}",
                email=f"e{i}@x.co", hashed_password="x", rol=RolUsuario.ENFERMERIA,
                institucion_id=i, activo=True,
            ))
        db.commit()
        for i in (1, 2):
            db.add(AtencionUrgencias(
                paciente_id=100 * i, fecha_ingreso=datetime(2026, 8, 1, 10, 0),
                motivo_consulta_triage=f"Motivo{i}", triage=TipoTriage.TRIAGE_3,
                enfermera_triage_id=50 + i,
            ))
        db.commit()


def _cliente(motor, usuario: Usuario) -> TestClient:
    def _db():
        with Session(motor) as s:
            yield s

    main.app.dependency_overrides[get_db] = _db
    main.app.dependency_overrides[get_current_active_user] = lambda: usuario
    return TestClient(main.app)


def _admin(institucion_id: int) -> Usuario:
    return Usuario(
        id=900 + institucion_id, username=f"adm{institucion_id}",
        nombre_completo="Admin", email=f"a{institucion_id}@x.co",
        hashed_password="x", rol=RolUsuario.ADMIN,
        institucion_id=institucion_id, activo=True,
    )


@pytest.fixture
def motor():
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    m = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(m)
    _sembrar(m)
    yield m
    main.app.dependency_overrides.clear()
    m.dispose()
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


# =========================================================================
# 1. EL PANEL DE URGENCIAS EXISTE Y RESPONDE
# =========================================================================

@pytest.mark.parametrize("mio", [1, 2])
def test_el_panel_de_urgencias_responde_y_no_cuenta_lo_del_vecino(motor, mio):
    """Respondía 500 y nadie lo sabía porque ninguna prueba lo llamaba.

    Las dos mitades en una: que responda (era `AttributeError` sobre
    `TipoTriage.ROJO`, que no existe) y que cuente solo lo propio.
    """
    r = _cliente(motor, _admin(mio)).get("/api/estadisticas/urgencias")
    assert r.status_code == 200, f"el panel de urgencias sigue reventando: {r.text[:300]}"
    d = r.json()
    assert d["total"] == 1, (
        f"FUGA: el prestador {mio} tiene 1 atención y el panel cuenta {d['total']}"
    )
    # Y el reparto por triage usa las etiquetas legibles, no los nombres internos.
    amarillo = [x for x in d["by_triage"] if x["triage"] == "Amarillo (III)"]
    assert amarillo and amarillo[0]["cantidad"] == 1, d["by_triage"]


# =========================================================================
# 2. EL FILTRO NO PUEDE FALLAR ABIERTO
# =========================================================================

def test_un_usuario_sin_institucion_no_recibe_los_pacientes_de_todos(motor):
    """LA AGUJA. Un no-superadmin sin institución tenía `sub = None`, y las cinco
    consultas que usan `_pac_sub` hacen `if sub is not None`: le devolvían el
    agregado de TODOS los prestadores.

    Ahora falla ruidosamente (directiva 20). Se comprueba que la respuesta NO sea
    un 200 con datos de más — que es lo único que importa: da igual si el
    mecanismo lo para con un 500 o con un 4xx, lo que no puede es servir.
    """
    huerfano = Usuario(
        id=777, username="huerfano", nombre_completo="Sin institución",
        email="h@x.co", hashed_password="x", rol=RolUsuario.MEDICO,
        institucion_id=None, activo=True,
    )
    c = _cliente(motor, huerfano)
    for ruta in ("/api/estadisticas/urgencias", "/api/estadisticas/citas",
                 "/api/estadisticas/produccion"):
        with pytest.raises(RuntimeError, match="NINGUNA institución"):
            c.get(ruta)


def test_el_superadmin_si_ve_las_dos(motor):
    """EL CONTROL, y sin él lo de arriba no probaría nada: una función que
    levantara SIEMPRE pasaría igual.

    El superadmin tampoco tiene institución —es el dueño del sistema, no un
    cliente— y ésa es justamente la distinción que `_pac_sub` no hacía.
    """
    dueno = Usuario(
        id=999, username="dueno", nombre_completo="Dueño", email="s@x.co",
        hashed_password="x", rol=RolUsuario.SUPERADMIN, institucion_id=None,
        activo=True,
    )
    r = _cliente(motor, dueno).get("/api/estadisticas/urgencias")
    assert r.status_code == 200, r.text
    assert r.json()["total"] == 2, (
        "el superadmin debe ver las atenciones de los dos prestadores: "
        f"{r.json()['total']}"
    )


def test_pac_sub_distingue_los_dos_casos_sin_institucion(motor):
    """La regla, directamente y sin pasar por HTTP.

    Es la mitad que fija la DECISIÓN: `None` significa «sin filtro» y solo el
    superadmin lo recibe. Un `if user.institucion_id` a secas —lo que había—
    devuelve `None` a los dos.
    """
    with Session(motor) as db:
        dueno = Usuario(
            id=999, username="d", nombre_completo="D", email="d@x.co",
            hashed_password="x", rol=RolUsuario.SUPERADMIN, institucion_id=None,
            activo=True,
        )
        assert _estadisticas._pac_sub(db, dueno) is None, (
            "el superadmin debe seguir viendo todo"
        )

        huerfano = Usuario(
            id=777, username="h", nombre_completo="H", email="h@x.co",
            hashed_password="x", rol=RolUsuario.MEDICO, institucion_id=None,
            activo=True,
        )
        with pytest.raises(RuntimeError, match="NINGUNA institución"):
            _estadisticas._pac_sub(db, huerfano)

        assert _estadisticas._pac_sub(db, _admin(1)) is not None, (
            "un admin con institución tiene que recibir su subconsulta"
        )
