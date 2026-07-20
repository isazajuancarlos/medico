"""Remisiones: que el estado no se pueda poner en cualquier cosa.

El módulo NiceGUI hacía `orden.estado = nuevo_estado` sin comprobar nada. Una
remisión COMPLETADA volvía a PENDIENTE y una CANCELADA pasaba a EN PROCESO con
solo elegirlo en una lista.

Importa porque la lista de pendientes es la que dice a quién falta atender: una
completada que reaparece se gestiona dos veces, y una cancelada que revive manda
a un paciente a una cita que ya no existe.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_aislamiento.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import main  # noqa: E402
import remisiones as dominio  # noqa: E402
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    EstadoLicencia, EstadoOrden, Historia, Institucion, Licencia, Orden,
    Paciente, RolUsuario, TipoLicencia, TipoOrden, Usuario,
)
from remisiones import TransicionNoValida, destinos, esta_abierta, validar_cambio

P, EP = EstadoOrden.PENDIENTE, EstadoOrden.EN_PROCESO
C, X = EstadoOrden.COMPLETADA, EstadoOrden.CANCELADA


class TestMaquinaDeEstados:
    @pytest.mark.parametrize("desde,hasta", [(P, EP), (P, C), (P, X), (EP, C), (EP, X)])
    def test_las_transiciones_del_flujo_normal_valen(self, desde, hasta):
        assert validar_cambio(desde, hasta) == hasta

    @pytest.mark.parametrize("desde,hasta", [
        (C, P), (C, EP), (C, X),      # completada no se deshace
        (X, P), (X, EP), (X, C),      # cancelada tampoco
        (EP, P),                      # no se retrocede a pendiente
    ])
    def test_las_que_no_tienen_sentido_se_rechazan(self, desde, hasta):
        with pytest.raises(TransicionNoValida):
            validar_cambio(desde, hasta)

    def test_no_se_puede_pasar_al_estado_en_que_ya_esta(self):
        with pytest.raises(TransicionNoValida) as e:
            validar_cambio(P, P)
        assert "ya está" in str(e.value)

    def test_el_mensaje_de_un_final_dice_qué_hacer_en_su_lugar(self):
        """Emitir una remisión nueva deja rastro de que fueron dos; cambiarle
        el estado a la vieja lo borra."""
        with pytest.raises(TransicionNoValida) as e:
            validar_cambio(C, P)
        assert "emita una remisión nueva" in str(e.value)

    def test_el_mensaje_de_una_invalida_dice_qué_sí_cabe(self):
        with pytest.raises(TransicionNoValida) as e:
            validar_cambio(EP, P)
        assert "Completada" in str(e.value)

    def test_los_destinos_son_los_que_la_pantalla_debe_ofrecer(self):
        assert destinos(P) == [EP, C, X]
        assert destinos(EP) == [C, X]
        assert destinos(C) == []
        assert destinos(X) == []

    def test_destinos_y_validar_dicen_lo_MISMO(self):
        """Si discreparan, la pantalla ofrecería opciones que el servidor
        rechaza, que es el fallo clásico de tener la regla en dos sitios."""
        for desde in EstadoOrden:
            for hasta in EstadoOrden:
                ofrecido = hasta in destinos(desde)
                try:
                    validar_cambio(desde, hasta)
                    aceptado = True
                except TransicionNoValida:
                    aceptado = False
                assert ofrecido == aceptado, f"{desde} → {hasta}"

    def test_abiertas_y_finales_parten_el_universo(self):
        assert dominio.ABIERTAS | dominio.FINALES == set(EstadoOrden)
        assert not (dominio.ABIERTAS & dominio.FINALES)

    def test_solo_cuentan_como_carga_las_abiertas(self):
        assert esta_abierta(P) and esta_abierta(EP)
        assert not esta_abierta(C) and not esta_abierta(X)
        assert not esta_abierta(None)


# --- Por HTTP, con dos instituciones ---------------------------------------

RUTA_BD = "./test_remisiones.db"


@pytest.fixture
def cliente():
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)
    with Session(motor) as db:
        for i, nombre in [(1, "Consultorio"), (2, "Clínica")]:
            db.add(Institucion(
                id=i, nit=f"90000000{i}", razon_social=nombre,
                nombre_comercial=nombre, direccion="c", ciudad="Medellín",
                departamento="Antioquia", telefono="6", email=f"{i}@x.co",
                representante_legal="R", identificacion_representante="1",
                codigo_prestador=f"0500{i}"))
            db.add(Licencia(
                institucion_id=i, tipo_licencia=TipoLicencia.BASICA,
                estado=EstadoLicencia.ACTIVA, fecha_inicio=date(2026, 1, 1),
                fecha_vencimiento=date(2027, 1, 1),
                modulos_habilitados=["REMISIONES"], clave_licencia=f"k{i}",
                hash_validacion="h"))
        db.add(Usuario(id=1, username="m", hashed_password="h",
                       nombre_completo="Dra. Ruiz", email="m@x.co",
                       rol=RolUsuario.MEDICO, institucion_id=1, activo=True))
        for pid, inst, doc in [(10, 1, "1111"), (20, 2, "2222")]:
            db.add(Paciente(id=pid, institucion_id=inst, identificacion=doc,
                            nombre1="N", apellido1="A",
                            fecha_nacimiento=date(1990, 1, 1), sexo="M",
                            celular="3"))
            db.add(Historia(id=pid * 10, paciente_id=pid, medico_id=1,
                            codigo_historia=f"H{pid}", motivo_consulta="x",
                            enfermedad_actual="x", diagnostico_principal="J449",
                            plan_tratamiento="x"))
        db.add(Orden(id=1, historia_id=100, paciente_id=10, medico_ordena_id=1,
                     tipo=TipoOrden.ESPECIALISTA, descripcion="Neumología",
                     justificacion="EPOC", estado=P,
                     fecha_orden=datetime(2026, 2, 2, 9, 0)))
        db.add(Orden(id=2, historia_id=100, paciente_id=10, medico_ordena_id=1,
                     tipo=TipoOrden.ESPECIALISTA, descripcion="Cardiología",
                     justificacion="Soplo", estado=C,
                     fecha_orden=datetime(2026, 2, 3, 9, 0)))
        db.add(Orden(id=3, historia_id=100, paciente_id=10, medico_ordena_id=1,
                     tipo=TipoOrden.LABORATORIO, descripcion="Hemograma",
                     justificacion="control", estado=P,
                     fecha_orden=datetime(2026, 2, 4, 9, 0)))
        db.add(Orden(id=9, historia_id=200, paciente_id=20, medico_ordena_id=1,
                     tipo=TipoOrden.ESPECIALISTA, descripcion="Ajena",
                     justificacion="x", estado=P,
                     fecha_orden=datetime(2026, 2, 5, 9, 0)))
        db.commit()

    usuario = Usuario(id=1, username="m", hashed_password="h",
                      nombre_completo="Dra. Ruiz", email="m@x.co",
                      rol=RolUsuario.MEDICO, institucion_id=1, activo=True)

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


def test_lista_solo_remisiones_de_su_institucion(cliente):
    ids = [r["id"] for r in cliente.get("/api/remisiones/").json()]
    assert ids == [2, 1], f"se coló algo: {ids}"


def test_una_orden_de_laboratorio_no_es_una_remision(cliente):
    ids = [r["id"] for r in cliente.get("/api/remisiones/").json()]
    assert 3 not in ids
    assert cliente.get("/api/remisiones/3").status_code == 404


def test_la_de_otra_institucion_da_404(cliente):
    assert cliente.get("/api/remisiones/9").status_code == 404


def test_la_pantalla_recibe_los_estados_que_puede_ofrecer(cliente):
    porid = {r["id"]: r for r in cliente.get("/api/remisiones/").json()}
    assert porid[1]["estados_posibles"] == ["En Proceso", "Completada", "Cancelada"]
    assert porid[2]["estados_posibles"] == [], "una completada no ofrece nada"


def test_el_cambio_valido_se_guarda(cliente):
    r = cliente.put("/api/remisiones/1/estado", json={"estado": "En Proceso"})
    assert r.status_code == 200
    assert r.json()["estado"] == "En Proceso"
    assert cliente.get("/api/remisiones/1").json()["estado"] == "En Proceso"


def test_reabrir_una_completada_se_rechaza_con_409(cliente):
    """El fallo del original: esto se guardaba sin más."""
    r = cliente.put("/api/remisiones/2/estado", json={"estado": "Pendiente"})
    assert r.status_code == 409
    assert "no se deshace" in r.json()["detail"]
    assert cliente.get("/api/remisiones/2").json()["estado"] == "Completada"


def test_cambiar_la_de_otra_institucion_da_404_no_409(cliente):
    """404 y no 409: un 409 confirmaría que la remisión existe."""
    r = cliente.put("/api/remisiones/9/estado", json={"estado": "Completada"})
    assert r.status_code == 404


def test_el_resumen_cuenta_lo_mismo_que_la_lista(cliente):
    lista = cliente.get("/api/remisiones/").json()
    resumen = cliente.get("/api/remisiones/resumen").json()
    assert resumen["total"] == len(lista)
    assert resumen["abiertas"] == sum(1 for r in lista if r["abierta"])
    assert resumen["por_estado"]["Pendiente"] == 1
    assert resumen["por_estado"]["Completada"] == 1
