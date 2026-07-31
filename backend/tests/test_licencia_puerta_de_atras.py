"""`POST /admin/usuarios` creaba usuarios sin mirar la licencia.

`POST /usuarios` sí comprobaba vigencia y cupo desde a4aeee6. La ruta de
administración hacía exactamente lo mismo —crear un Usuario con la institución
del que llama— y no comprobaba nada. No era un descuido menor: era una puerta
de atrás completa, porque quien puede llamar a una puede llamar a la otra.

El control se prueba por HTTP y no sobre la función, porque lo que faltaba
estaba en el endpoint. Una prueba sobre `hay_cupo` habría seguido en verde con
el agujero abierto — de hecho lo estuvo.
"""
import itertools
import os
import sys
from datetime import date, timedelta

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

RUTA_BD = os.path.join(os.path.dirname(os.path.abspath(__file__)), "test_puerta.db")
_n = itertools.count()


@pytest.fixture
def cliente():
    from fastapi.testclient import TestClient
    from sqlmodel import Session, SQLModel, create_engine

    import database
    import main
    from core.deps import get_current_active_user, get_db
    from models import (EstadoLicencia, Institucion, Licencia, RolUsuario,
                        TipoLicencia, Usuario)

    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)

    estado = {}

    def montar(*, vence_en_dias: int, max_medicos: int, medicos_ya: int):
        with Session(motor) as db:
            inst = Institucion(
                nit="900000123", razon_social="IPS", nombre_comercial="IPS",
                direccion="c", ciudad="Medellín", departamento="Antioquia",
                telefono="6", email="a@x.co", representante_legal="R",
                identificacion_representante="1", codigo_prestador="05001")
            db.add(inst); db.commit(); db.refresh(inst)
            db.add(Licencia(
                institucion_id=inst.id, tipo_licencia=TipoLicencia.ENTERPRISE,
                estado=EstadoLicencia.ACTIVA,
                fecha_inicio=date.today() - timedelta(days=1),
                fecha_vencimiento=date.today() + timedelta(days=vence_en_dias),
                max_usuarios=50, max_medicos=max_medicos,
                modulos_habilitados=["CITAS"], clave_licencia="k", hash_validacion="h"))
            for _ in range(medicos_ya):
                db.add(Usuario(username=f"m{next(_n)}", hashed_password="x",
                               nombre_completo="Médica", rol=RolUsuario.MEDICO,
                               institucion_id=inst.id, activo=True))
            admin = Usuario(username=f"a{next(_n)}", hashed_password="x",
                            nombre_completo="Admin", rol=RolUsuario.ADMIN,
                            institucion_id=inst.id, activo=True)
            db.add(admin); db.commit(); db.refresh(admin)
            estado["admin"] = admin

        def _db():
            with Session(motor) as s:
                yield s

        main.app.dependency_overrides[get_db] = _db
        main.app.dependency_overrides[get_current_active_user] = lambda: estado["admin"]
        return TestClient(main.app)

    yield montar
    main.app.dependency_overrides.clear()
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


def _alta(c, rol="medico"):
    return c.post("/api/admin/usuarios", json={
        "username": f"nuevo{next(_n)}", "password": "Contrasena-larga-1",
        "nombre_completo": "Quien sea", "rol": rol})


def test_con_licencia_vencida_la_puerta_de_atras_ya_no_abre(cliente):
    c = cliente(vence_en_dias=-1, max_medicos=10, medicos_ya=0)
    r = _alta(c)
    assert r.status_code == 403, r.text
    assert "venció" in r.json()["detail"]


def test_con_el_cupo_de_medicos_lleno_tampoco(cliente):
    c = cliente(vence_en_dias=365, max_medicos=2, medicos_ya=2)
    r = _alta(c)
    assert r.status_code == 403, r.text
    assert "2 médicos" in r.json()["detail"]


def test_el_cupo_de_medicos_no_estorba_a_quien_no_es_medico(cliente):
    """Con los médicos llenos, dar de alta una enfermera debe seguir pudiendo:
    el tope de médicos no es el tope de usuarios."""
    c = cliente(vence_en_dias=365, max_medicos=2, medicos_ya=2)
    r = _alta(c, rol="enfermeria")
    assert r.status_code == 201, r.text


def test_con_todo_en_regla_el_alta_pasa(cliente):
    """Que discrimine: si rechazara siempre, las de arriba no probarían nada."""
    c = cliente(vence_en_dias=365, max_medicos=5, medicos_ya=1)
    r = _alta(c)
    assert r.status_code == 201, r.text
