"""Los tres estados de un módulo tienen que distinguirse desde afuera.

Antes todo respondía `200 []`, así que un consultorio no podía saber si
"Banco de Sangre" estaba vacío, no contratado o sin construir. En un sistema
clínico esa ambigüedad es un dato falso.

  403  el módulo existe pero la institución NO lo licenció
  501  está licenciado pero AÚN NO está implementado
  200  funciona
"""
import os
import sys

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_modulos.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402

import main  # noqa: E402
from core.deps import get_current_active_user, get_db, modulos_de  # noqa: E402
from models import RolUsuario, Usuario  # noqa: E402


def _como(rol=RolUsuario.ADMIN, institucion_id=1):
    u = Usuario(id=1, nombre="Prueba", email="p@x.co", password_hash="h",
                rol=rol, institucion_id=institucion_id, activo=True)
    return u


@pytest.fixture
def cliente(monkeypatch):
    """App con un usuario fijo y un juego de módulos controlado por la prueba."""
    habilitados = {"valor": {"PACIENTES", "CITAS", "VACUNACION"}}

    main.app.dependency_overrides[get_current_active_user] = lambda: _como()
    main.app.dependency_overrides[get_db] = lambda: None
    monkeypatch.setattr("core.deps.modulos_de", lambda u, d: habilitados["valor"])
    # los routers ya capturaron la función al importarse: parchear también allí
    import routers.modulos as rm
    monkeypatch.setattr(rm, "modulos_de", lambda u, d: habilitados["valor"])

    with TestClient(main.app, raise_server_exceptions=False) as c:
        c.habilitados = habilitados
        yield c

    main.app.dependency_overrides.clear()


# --- 501: licenciado pero sin construir -----------------------------------

def test_un_modulo_sin_implementar_responde_501_no_lista_vacia(cliente):
    r = cliente.get("/api/vacunacion/")
    assert r.status_code == 501, (
        f"devolvió {r.status_code}: un módulo sin implementar que responde 200 [] "
        f"se lee como 'el paciente no tiene vacunas'"
    )
    assert "no implementado" in r.json()["detail"].lower()


def test_el_501_cubre_subrutas_y_otros_metodos(cliente):
    assert cliente.get("/api/kardex/cualquier/cosa").status_code == 501
    assert cliente.post("/api/kardex/").status_code == 501


def test_ningun_stub_devuelve_lista_vacia(cliente):
    """La regresión concreta: 26 módulos fingiendo estar vacíos."""
    from routers.stubs import PENDIENTES

    mentirosos = []
    for prefijo in PENDIENTES:
        r = cliente.get(f"/api/{prefijo}/")
        if r.status_code == 200 and r.json() == []:
            mentirosos.append(prefijo)
    assert not mentirosos, f"siguen fingiendo estar vacíos: {mentirosos}"


# --- 403: no licenciado ----------------------------------------------------

def test_modulo_no_licenciado_responde_403(cliente):
    r = cliente.get("/api/quirofano/salas")   # ruta real del router
    assert r.status_code == 403
    assert "no está habilitado" in r.json()["detail"]


def test_licenciar_el_modulo_lo_abre(cliente):
    assert cliente.get("/api/quirofano/salas").status_code == 403
    cliente.habilitados["valor"] = cliente.habilitados["valor"] | {"QUIROFANO"}
    assert cliente.get("/api/quirofano/salas").status_code != 403


# --- El catálogo que arma el menú -----------------------------------------

def test_modulos_reporta_los_tres_estados(cliente):
    r = cliente.get("/api/modulos/")
    assert r.status_code == 200
    datos = r.json()

    estados = {m["modulo"]: m["estado"]
               for g in datos["grupos"] for m in g["modulos"]}

    assert estados["PACIENTES"] == "habilitado"      # licenciado e implementado
    assert estados["VACUNACION"] == "pendiente"      # licenciado, sin construir
    assert estados["QUIROFANO"] == "no_incluido"     # existe, no contratado


def test_el_catalogo_viene_agrupado_para_el_menu(cliente):
    grupos = {g["grupo"] for g in cliente.get("/api/modulos/").json()["grupos"]}
    assert {"Core", "Hospitalario", "Quirúrgico", "Administrativo"} <= grupos


def test_un_consultorio_ve_menos_que_un_hospital(cliente):
    """El mismo binario sirve a los dos: lo que cambia es la licencia."""
    cliente.habilitados["valor"] = {"PACIENTES", "CITAS", "HISTORIA"}
    consultorio = cliente.get("/api/modulos/").json()["habilitados"]

    cliente.habilitados["valor"] = {"PACIENTES", "CITAS", "HISTORIA",
                                    "HOSPITALIZACION", "QUIROFANO", "BANCO_SANGRE"}
    hospital = cliente.get("/api/modulos/").json()["habilitados"]

    assert len(consultorio) < len(hospital)
    assert set(consultorio) < set(hospital)
