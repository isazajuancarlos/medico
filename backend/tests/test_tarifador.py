"""El tarifador: que nadie tenga que escribir el valor a mano en el JSON.

Hoy el prestador genera el archivo y después lo EDITA para poner el valor de
cada servicio. Estas pruebas fijan que el sistema lo resuelva solo, y —lo que
más importa— que lo CONGELE al momento de la atención.
"""
import os
import sys
from datetime import date, datetime

import pytest

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


#: El prestador de estas pruebas. Un contrato SIN dueño ya no resuelve nada
#: (2026-08-07): la búsqueda por contrato filtra por institución, porque antes
#: barría los de todos los prestadores de la base y aplicaba el precio del
#: vecino. Ver la cabecera de `tarifador._contratos_vigentes`.
INSTITUCION = 1


def _contrato(db, entidad, *, numero="C-1", desde=date(2026, 1, 1),
              hasta=date(2026, 12, 31), incremento=0, activo=True,
              institucion_id=INSTITUCION):
    c = Contrato(
        entidad_id=entidad.id, institucion_id=institucion_id,
        nombre="Contrato de prueba", numero_contrato=numero,
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
    def __init__(self, entidad_id, institucion_id=INSTITUCION):
        self.entidad_id = entidad_id
        # `aplicar_a_historia` saca la institución de aquí, y sin ella el
        # contrato del prestador no se le aplica a su propia atención.
        self.institucion_id = institucion_id


# --- Resolución --------------------------------------------------------------

def test_resuelve_el_valor_desde_el_contrato(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    r = resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION, fecha=date(2026, 6, 15))

    assert r.resuelto
    assert r.valor == 45000
    assert r.contrato == "C-1", "se dice de qué contrato salió, para explicar una glosa"


def test_aplica_el_incremento_pactado(db):
    """Un contrato «ISS + 30 %» guarda la tarifa base y el 30 aparte."""
    e = _entidad(db)
    c = _contrato(db, e, incremento=30)
    _tarifa(db, c, "890301", 40000)

    assert resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION,
                    fecha=date(2026, 6, 15)).valor == 52000


def test_calcula_el_copago_sobre_el_valor_final(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 50000, copago=10)

    r = resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION, fecha=date(2026, 6, 15))
    assert r.valor == 50000
    assert r.pago_moderador == 5000


# --- Lo que NO se inventa ----------------------------------------------------

def test_sin_contrato_vigente_devuelve_None_y_dice_por_que(db):
    """Poner un cero produce un archivo que pasa la validación y no le paga a
    nadie. Es lo que hacía el sistema."""
    e = _entidad(db)
    r = resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION, fecha=date(2026, 6, 15))

    assert not r.resuelto
    assert r.valor is None
    assert "ni contrato de la entidad" in r.motivo


def test_un_contrato_vencido_no_sirve(db):
    e = _entidad(db)
    c = _contrato(db, e, desde=date(2025, 1, 1), hasta=date(2025, 12, 31))
    _tarifa(db, c, "890301", 45000)

    assert not resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION,
                        fecha=date(2026, 6, 15)).resuelto


def test_un_contrato_inactivo_no_sirve(db):
    e = _entidad(db)
    c = _contrato(db, e, activo=False)
    _tarifa(db, c, "890301", 45000)

    assert not resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION,
                        fecha=date(2026, 6, 15)).resuelto


def test_un_CUPS_que_no_esta_en_el_contrato_se_dice(db):
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)

    r = resolver(db, cups="999999", entidad_id=e.id, institucion_id=INSTITUCION, fecha=date(2026, 6, 15))
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

    en_2025 = resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION, fecha=date(2025, 6, 15))
    en_2026 = resolver(db, cups="890301", entidad_id=e.id, institucion_id=INSTITUCION, fecha=date(2026, 6, 15))

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


# --- La universalidad: cada tipo de prestador usa lo que tiene ---------------
#
# La cascada es contrato -> tarifa propia -> nada. Una IPS que negocia con EPS
# usa la primera; un consultorio pequeño, que no tiene contratos, la segunda; un
# hospital con contratos que atiende a un particular, las dos. Ninguno queda
# fuera y ninguno tiene que configurar lo que no usa.

from models import Institucion, TarifaInstitucion  # noqa: E402


def _institucion(db, nombre="Consultorio Prueba"):
    i = Institucion(
        razon_social=nombre, nombre_comercial=nombre, nit="800123456",
        codigo_habilitacion="700010000101", direccion="Calle 1", ciudad="Sincelejo",
        departamento="Sucre", telefono="6000000", email="p@x.co",
        representante_legal="Prueba", identificacion_representante="1",
        codigo_prestador="700010000101",
    )
    db.add(i)
    db.commit()
    db.refresh(i)
    return i


def _tarifa_propia(db, institucion, cups="890301", valor=40000, copago=0,
                   desde=date(2020, 1, 1), hasta=None):
    t = TarifaInstitucion(
        institucion_id=institucion.id, codigo_servicio=cups, valor_tarifa=valor,
        copago_porcentaje=copago, vigente_desde=desde, vigente_hasta=hasta,
    )
    db.add(t)
    db.commit()
    return t


def test_un_CONSULTORIO_sin_contratos_cobra_su_tarifa(db):
    """El caso del primer cliente: no negocia con EPS, tiene tarifa única.
    Antes de esto el tarifador le devolvía vacío SIEMPRE."""
    inst = _institucion(db)
    _tarifa_propia(db, inst, "890301", 40000)

    r = resolver(db, cups="890301", institucion_id=inst.id, fecha=date(2026, 6, 15))

    assert r.resuelto
    assert r.valor == 40000
    assert r.contrato == "tarifa propia"


def test_una_IPS_con_contrato_usa_el_CONTRATO_aunque_tenga_tarifa_propia(db):
    """Lo pactado manda sobre la lista de precios interna."""
    inst = _institucion(db)
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)
    _tarifa_propia(db, inst, "890301", 40000)

    r = resolver(db, cups="890301", entidad_id=e.id, institucion_id=inst.id,
                 fecha=date(2026, 6, 15))

    assert r.valor == 45000
    assert r.contrato == "C-1"


def test_un_CUPS_fuera_del_contrato_cae_a_la_tarifa_propia(db):
    """Un hospital con contrato que presta un servicio no pactado: no se queda
    sin valor, usa su lista de precios."""
    inst = _institucion(db)
    e = _entidad(db)
    c = _contrato(db, e)
    _tarifa(db, c, "890301", 45000)
    _tarifa_propia(db, inst, "890201", 60000)

    r = resolver(db, cups="890201", entidad_id=e.id, institucion_id=inst.id,
                 fecha=date(2026, 6, 15))

    assert r.valor == 60000
    assert r.contrato == "tarifa propia"


def test_la_tarifa_propia_TAMBIEN_se_congela(db):
    """El principio no depende de que haya contrato: una consulta del año pasado
    valía lo que valía entonces."""
    inst = _institucion(db)
    _tarifa_propia(db, inst, "890301", 30000,
                   desde=date(2025, 1, 1), hasta=date(2025, 12, 31))
    _tarifa_propia(db, inst, "890301", 40000, desde=date(2026, 1, 1))

    assert resolver(db, cups="890301", institucion_id=inst.id,
                    fecha=date(2025, 6, 15)).valor == 30000
    assert resolver(db, cups="890301", institucion_id=inst.id,
                    fecha=date(2026, 6, 15)).valor == 40000


def test_una_tarifa_sin_fecha_de_fin_sigue_vigente(db):
    inst = _institucion(db)
    _tarifa_propia(db, inst, "890301", 40000, desde=date(2026, 1, 1), hasta=None)

    assert resolver(db, cups="890301", institucion_id=inst.id,
                    fecha=date(2030, 1, 1)).resuelto


def test_sin_contrato_NI_tarifa_propia_lo_dice_claro(db):
    inst = _institucion(db)
    r = resolver(db, cups="890301", institucion_id=inst.id, fecha=date(2026, 6, 15))

    assert not r.resuelto
    assert "ni contrato" in r.motivo and "ni lista de precios" in r.motivo


# --- Aislamiento entre prestadores (2026-08-07) ------------------------------
#
# Estas dos son la pareja de un defecto MEDIDO, no imaginado. `_contratos_vigentes`
# no filtraba por institución: barría los contratos de todos los prestadores de
# la base y se quedaba con el primero cuyo CUPS casara. Con dos IPS que facturan
# a la misma EPS y pactaron 50 000 y 99 000 por el mismo código, las dos recibían
# 50 000 — el precio del vecino.
#
# Van SIEMPRE dos instituciones, por la misma razón que en
# `test_facturacion_aislamiento.py`: con una sola, la consulta sin filtro
# devuelve exactamente lo mismo que la filtrada y ninguna prueba las distingue.
# Ése es el motivo de que el defecto no lo viera nadie.


def test_cada_prestador_cobra_SU_tarifa_y_no_la_del_vecino(db):
    """El caso rojo y su pareja, que es la prueba entera.

    Sin las DOS mitades esto lo pasaría igual un filtro que negara siempre.
    """
    e = _entidad(db)
    mio = _contrato(db, e, numero="C-MIO", institucion_id=1)
    ajeno = _contrato(db, e, numero="C-AJENO", institucion_id=2)
    _tarifa(db, mio, "890301", 50000)
    _tarifa(db, ajeno, "890301", 99000)

    r1 = resolver(db, cups="890301", entidad_id=e.id, institucion_id=1,
                  fecha=date(2026, 6, 15))
    r2 = resolver(db, cups="890301", entidad_id=e.id, institucion_id=2,
                  fecha=date(2026, 6, 15))

    assert r1.valor == 50000, "el prestador 1 recibió una tarifa que no pactó"
    assert r2.valor == 99000, "el prestador 2 recibió una tarifa que no pactó"
    assert r1.contrato == "C-MIO" and r2.contrato == "C-AJENO"


def test_sin_saber_de_quien_es_la_atencion_NO_se_aplica_un_contrato_cualquiera(db):
    """Ante la ausencia del dato se falla ruidosamente, no se rellena.

    Devolver la lista entera de contratos produciría un número con la forma
    correcta que nadie pactó, y el valor se CONGELA en la historia: ese número
    entra en la factura y ya no se distingue de uno negociado (directiva 20).
    """
    e = _entidad(db)
    c = _contrato(db, e, institucion_id=1)
    _tarifa(db, c, "890301", 50000)

    r = resolver(db, cups="890301", entidad_id=e.id, institucion_id=None,
                 fecha=date(2026, 6, 15))

    assert not r.resuelto, "se aplicó un contrato sin saber de qué prestador es la atención"
    assert "no hay tarifa" in r.motivo
