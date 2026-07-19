"""El tarifador: que nadie tenga que escribir el valor a mano en el JSON.

Hoy el prestador genera el archivo y después lo EDITA para poner el valor de
cada servicio. Estas pruebas fijan que el sistema lo resuelva solo, y —lo que
más importa— que lo CONGELE al momento de la atención.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_tarifa.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from sqlmodel import Session, SQLModel  # noqa: E402

from database import engine  # noqa: E402
from models import Contrato, Entidad, TarifaContrato  # noqa: E402
from tarifador import aplicar_a_historia, resolver  # noqa: E402


@pytest.fixture
def db():
    SQLModel.metadata.create_all(engine)
    with Session(engine) as s:
        yield s
    SQLModel.metadata.drop_all(engine)


def _entidad(db, nombre="EPS Prueba"):
    e = Entidad(nombre=nombre, nit="900123456", codigo_eps="EPS001")
    db.add(e)
    db.commit()
    db.refresh(e)
    return e


def _contrato(db, entidad, *, numero="C-1", desde=date(2026, 1, 1),
              hasta=date(2026, 12, 31), incremento=0, activo=True):
    c = Contrato(
        entidad_id=entidad.id, nombre="Contrato de prueba", numero_contrato=numero,
        fecha_inicio=desde, fecha_fin=hasta, activo=activo,
        tipo_tarifa="CUPS", porcentaje_incremento=incremento,
    )
    db.add(c)
    db.commit()
    db.refresh(c)
    return c


def _tarifa(db, contrato, cups="890301", valor=45000, copago=0):
    t = TarifaContrato(
        contrato_id=contrato.id, codigo_servicio=cups,
        valor_tarifa=valor, copago_porcentaje=copago,
    )
    db.add(t)
    db.commit()
    return t


class Historia:
    """Doble de la historia, con lo que el tarifador mira."""

    def __init__(self, paciente, cups="890301", fecha=datetime(2026, 6, 15, 10, 0),
                 valor=None):
        self.paciente = paciente
        self.cod_consulta_cups = cups
        self.fecha = fecha
        self.valor_servicio = valor
        self.valor_pago_moderador = 0


class PacienteFalso:
    def __init__(self, entidad_id):
        self.entidad_id = entidad_id


# --- Resolución --------------------------------------------------------------

def test_resuelve_el_valor_desde_el_contrato(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    r = resolver(db, cups="890301", entidad_id=e.id, fecha=date(2026, 6, 15))

    assert r.resuelto
    assert r.valor == 45000
    assert r.contrato == "C-1", "se dice de qué contrato salió, para explicar una glosa"


def test_aplica_el_incremento_pactado(db):
    """Un contrato «ISS + 30 %» guarda la tarifa base y el 30 aparte."""
    e = _entidad(db)
    c = _contrato(db, e, incremento=30)
    _tarifa(db, c, "890301", 40000)

    assert resolver(db, cups="890301", entidad_id=e.id,
                    fecha=date(2026, 6, 15)).valor == 52000


def test_calcula_el_copago_sobre_el_valor_final(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 50000, copago=10)

    r = resolver(db, cups="890301", entidad_id=e.id, fecha=date(2026, 6, 15))
    assert r.valor == 50000
    assert r.pago_moderador == 5000


# --- Lo que NO se inventa ----------------------------------------------------

def test_sin_contrato_vigente_devuelve_None_y_dice_por_que(db):
    """Poner un cero produce un archivo que pasa la validación y no le paga a
    nadie. Es lo que hacía el sistema."""
    e = _entidad(db)
    r = resolver(db, cups="890301", entidad_id=e.id, fecha=date(2026, 6, 15))

    assert not r.resuelto
    assert r.valor is None
    assert "no hay contrato vigente" in r.motivo


def test_un_contrato_vencido_no_sirve(db):
    e = _entidad(db)
    c = _contrato(db, e, desde=date(2025, 1, 1), hasta=date(2025, 12, 31))
    _tarifa(db, c, "890301", 45000)

    assert not resolver(db, cups="890301", entidad_id=e.id,
                        fecha=date(2026, 6, 15)).resuelto


def test_un_contrato_inactivo_no_sirve(db):
    e = _entidad(db)
    c = _contrato(db, e, activo=False)
    _tarifa(db, c, "890301", 45000)

    assert not resolver(db, cups="890301", entidad_id=e.id,
                        fecha=date(2026, 6, 15)).resuelto


def test_un_CUPS_que_no_esta_en_el_contrato_se_dice(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    r = resolver(db, cups="999999", entidad_id=e.id, fecha=date(2026, 6, 15))
    assert not r.resuelto
    assert "999999" in r.motivo


def test_sin_CUPS_ni_sin_entidad_no_revienta(db):
    assert not resolver(db, cups=None, entidad_id=1, fecha=date(2026, 6, 15)).resuelto
    assert not resolver(db, cups="890301", entidad_id=None,
                        fecha=date(2026, 6, 15)).resuelto


# --- La decisión que más importa: se congela ---------------------------------

def test_EL_VALOR_SE_CONGELA_EN_LA_FECHA_DE_LA_ATENCION(db):
    """Dos contratos consecutivos con precios distintos.

    Si el informe se generara consultando la tarifa de HOY, un periodo viejo
    daría cifras distintas en cada corrida, y ninguna correspondería a lo pactado
    cuando se atendió al paciente.
    """
    e = _entidad(db)
    viejo = _contrato(db, e, numero="C-2025", desde=date(2025, 1, 1),
                      hasta=date(2025, 12, 31))
    _tarifa(db, viejo, "890301", 30000)
    nuevo = _contrato(db, e, numero="C-2026", desde=date(2026, 1, 1),
                      hasta=date(2026, 12, 31))
    _tarifa(db, nuevo, "890301", 45000)

    en_2025 = resolver(db, cups="890301", entidad_id=e.id, fecha=date(2025, 6, 15))
    en_2026 = resolver(db, cups="890301", entidad_id=e.id, fecha=date(2026, 6, 15))

    assert en_2025.valor == 30000 and en_2025.contrato == "C-2025"
    assert en_2026.valor == 45000 and en_2026.contrato == "C-2026"


def test_aplicar_guarda_el_valor_en_la_historia(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000, copago=10)

    h = Historia(PacienteFalso(e.id))
    r = aplicar_a_historia(db, h)

    assert r.resuelto
    assert h.valor_servicio == 45000
    assert h.valor_pago_moderador == 4500


def test_NO_pisa_un_valor_puesto_a_mano(db):
    """Si alguien lo corrigió tras una glosa, esa decisión gana sobre la tarifa."""
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    h = Historia(PacienteFalso(e.id), valor=99000)
    aplicar_a_historia(db, h)

    assert h.valor_servicio == 99000


def test_forzar_si_recalcula(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    h = Historia(PacienteFalso(e.id), valor=99000)
    aplicar_a_historia(db, h, forzar=True)

    assert h.valor_servicio == 45000


def test_si_no_se_resuelve_la_historia_queda_intacta(db):
    """Sin tarifa, el campo queda vacío y auditoria_rips lo señalará antes de
    enviar. No se escribe un cero."""
    h = Historia(PacienteFalso(entidad_id=None))
    r = aplicar_a_historia(db, h)

    assert not r.resuelto
    assert h.valor_servicio is None


def test_acepta_datetime_ademas_de_date(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    h = Historia(PacienteFalso(e.id), fecha=datetime(2026, 6, 15, 14, 30))
    assert aplicar_a_historia(db, h).valor == 45000
