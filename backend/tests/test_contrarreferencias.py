"""Contrarreferencia: que el circuito se cierre.

Una remisión y su contrarreferencia son las dos mitades de lo mismo. El módulo
NiceGUI las guardaba sin relación operativa: se registraba la contrarreferencia
y la remisión seguía PENDIENTE para siempre.

El efecto es el de siempre en este sistema: la lista de pendientes deja de
significar lo que dice. Y cuando una lista miente lo suficiente se deja de
mirar, así que las que sí están pendientes tampoco se ven.
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
from contrarreferencias import (  # noqa: E402
    YaRecibida,
    cerrar_remision,
    marcar_recibida,
    pendiente_de_recibir,
    requiere_control,
)
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    Contrarreferencia, EstadoLicencia, EstadoOrden, Historia, Institucion,
    Licencia, Orden, Paciente, RolUsuario, TipoLicencia, TipoOrden, Usuario,
)


class Remision:
    def __init__(self, estado=EstadoOrden.PENDIENTE):
        self.estado = estado


class CR:
    def __init__(self, **k):
        self.recibida = False
        self.fecha_recepcion = None
        self.recibida_por_id = None
        self.requiere_seguimiento = False
        for a, v in k.items():
            setattr(self, a, v)


class TestCierreDelCircuito:
    def test_una_remision_pendiente_queda_completada(self):
        r = Remision(EstadoOrden.PENDIENTE)
        c = cerrar_remision(r)
        assert c.cerrada
        assert r.estado == EstadoOrden.COMPLETADA

    def test_una_en_proceso_tambien(self):
        r = Remision(EstadoOrden.EN_PROCESO)
        assert cerrar_remision(r).cerrada
        assert r.estado == EstadoOrden.COMPLETADA

    def test_una_ya_completada_no_se_toca_pero_NO_revienta(self):
        """La contrarreferencia es un documento clínico: tiene que poder
        guardarse aunque el trámite esté en un estado raro."""
        r = Remision(EstadoOrden.COMPLETADA)
        c = cerrar_remision(r)
        assert not c.cerrada
        assert r.estado == EstadoOrden.COMPLETADA
        assert "se guardó igual" in c.motivo

    def test_una_cancelada_no_se_resucita(self):
        r = Remision(EstadoOrden.CANCELADA)
        assert not cerrar_remision(r).cerrada
        assert r.estado == EstadoOrden.CANCELADA

    def test_sin_remision_asociada_lo_dice_y_ya(self):
        c = cerrar_remision(None)
        assert not c.cerrada
        assert "no se asoció" in c.motivo

    def test_el_cierre_usa_la_MISMA_maquina_que_remisiones(self):
        """Si tuviera reglas propias, acabarían discrepando."""
        import remisiones
        for estado in EstadoOrden:
            r = Remision(estado)
            esperado = remisiones.puede_pasar(estado, EstadoOrden.COMPLETADA)
            assert cerrar_remision(r).cerrada == esperado


class TestAcuseDeRecibo:
    def test_marcar_recibida_deja_quien_y_cuando(self):
        cr = CR()
        marcar_recibida(cr, recibida_por_id=7)
        assert cr.recibida
        assert cr.recibida_por_id == 7
        assert cr.fecha_recepcion is not None

    def test_no_se_sobrescribe_el_primer_acuse(self):
        """El primero es el que cuenta: es el que prueba cuándo se supo."""
        cr = CR()
        marcar_recibida(cr, 7, datetime(2026, 3, 1, 10, 0))
        with pytest.raises(YaRecibida):
            marcar_recibida(cr, 9)
        assert cr.recibida_por_id == 7
        assert cr.fecha_recepcion == datetime(2026, 3, 1, 10, 0)

    def test_pendiente_hasta_que_alguien_acuse(self):
        cr = CR()
        assert pendiente_de_recibir(cr)
        marcar_recibida(cr, 1)
        assert not pendiente_de_recibir(cr)

    def test_el_seguimiento_se_señala(self):
        assert requiere_control(CR(requiere_seguimiento=True))
        assert not requiere_control(CR())


# --- Por HTTP -------------------------------------------------------------

RUTA_BD = "./test_contrarreferencia.db"


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
                modulos_habilitados=["CONTRARREFERENCIA"],
                clave_licencia=f"k{i}", hash_validacion="h"))
        db.add(Usuario(id=1, username="m", hashed_password="h",
                       nombre_completo="Dra. Ruiz", email="m@x.co",
                       rol=RolUsuario.MEDICO, institucion_id=1, activo=True))
        for pid, inst in [(10, 1), (20, 2)]:
            db.add(Paciente(id=pid, institucion_id=inst, identificacion=str(pid),
                            nombre1="N", apellido1="A",
                            fecha_nacimiento=date(1990, 1, 1), sexo="M",
                            celular="3"))
            db.add(Historia(id=pid * 10, paciente_id=pid, medico_id=1,
                            codigo_historia=f"H{pid}", motivo_consulta="x",
                            enfermedad_actual="x", diagnostico_principal="J449",
                            plan_tratamiento="x"))
        db.add(Orden(id=1, historia_id=100, paciente_id=10, medico_ordena_id=1,
                     tipo=TipoOrden.ESPECIALISTA, descripcion="Neumología",
                     justificacion="EPOC", estado=EstadoOrden.PENDIENTE,
                     fecha_orden=datetime(2026, 2, 2, 9, 0)))
        db.add(Orden(id=2, historia_id=100, paciente_id=10, medico_ordena_id=1,
                     tipo=TipoOrden.ESPECIALISTA, descripcion="Cerrada",
                     justificacion="x", estado=EstadoOrden.CANCELADA,
                     fecha_orden=datetime(2026, 2, 1, 9, 0)))
        db.add(Orden(id=9, historia_id=200, paciente_id=20, medico_ordena_id=1,
                     tipo=TipoOrden.ESPECIALISTA, descripcion="Ajena",
                     justificacion="x", estado=EstadoOrden.PENDIENTE,
                     fecha_orden=datetime(2026, 2, 3, 9, 0)))
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


def _nueva(orden_id=1, paciente_id=10):
    return {"paciente_id": paciente_id, "orden_id": orden_id,
            "diagnostico_final": "EPOC GOLD II",
            "recomendaciones": "Control en 3 meses",
            "requiere_seguimiento": True}


def test_solo_ofrece_remisiones_ABIERTAS(cliente):
    """El original listaba las últimas 30 sin mirar el estado."""
    ids = [r["id"] for r in cliente.get("/api/contrarreferencia/remisiones-abiertas").json()]
    assert ids == [1], f"ofreció cerradas o ajenas: {ids}"


def test_registrarla_CIERRA_la_remision(cliente):
    r = cliente.post("/api/contrarreferencia/", json=_nueva())
    assert r.status_code == 201, r.text
    assert "completada" in r.json()["cierre"].lower()
    # y ya no aparece entre las abiertas
    assert cliente.get("/api/contrarreferencia/remisiones-abiertas").json() == []


def test_contra_una_remision_ajena_da_404(cliente):
    r = cliente.post("/api/contrarreferencia/", json=_nueva(orden_id=9, paciente_id=10))
    assert r.status_code == 404


def test_la_remision_tiene_que_ser_del_mismo_paciente(cliente):
    """Cerrar la remisión de otro paciente sería peor que no cerrar ninguna."""
    r = cliente.post("/api/contrarreferencia/",
                     json={"paciente_id": 10, "orden_id": 2,
                           "diagnostico_final": "x"})
    assert r.status_code == 201  # misma persona, remisión cancelada
    assert "no se modificó" in r.json()["cierre"]


def test_se_puede_registrar_sin_remision(cliente):
    """Un paciente puede volver de otra institución sin remisión formal."""
    r = cliente.post("/api/contrarreferencia/",
                     json={"paciente_id": 10, "diagnostico_final": "x"})
    assert r.status_code == 201
    assert "no se asoció" in r.json()["cierre"]


def test_el_acuse_de_recibo(cliente):
    cr_id = cliente.post("/api/contrarreferencia/", json=_nueva()).json()["id"]
    assert cliente.get("/api/contrarreferencia/",
                       params={"solo_pendientes": True}).json()

    r = cliente.post(f"/api/contrarreferencia/{cr_id}/recibir")
    assert r.status_code == 200
    assert r.json()["recibida"] is True

    assert cliente.post(f"/api/contrarreferencia/{cr_id}/recibir").status_code == 409
