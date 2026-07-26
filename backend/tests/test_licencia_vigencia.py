"""La licencia vence, y el sistema tiene que enterarse.

# Por qué existe este banco

Estos controles vivían en la versión NiceGUI (`middleware.py`,
`ValidadorLicencia`) y se perdieron al pasar a FastAPI. Medido antes de
reponerlos: `Licencia.fecha_vencimiento` no se leía en NINGUNA parte del
backend, y `max_usuarios`/`max_medicos` solo aparecían en el modelo y en la
semilla. Una licencia vencía y el sistema seguía funcionando indefinidamente,
que a efectos prácticos es no tener licencia.

# Las dos propiedades que se fijan aquí

1. **Vencida se lee, pero no se crece.** Bloquear la lectura dejaría a un
   hospital sin su historia clínica por un asunto comercial, y con ello
   incumpliendo ante el paciente y ante la Supersalud. Lo que se corta es el
   alta de usuarios y de médicos.

2. **El cupo se cuenta.** El modelo trae `uso_actual_usuarios`, y nadie lo
   actualiza: vale 0 desde la semilla. El control heredado comparaba contra ese
   contador, así que no podía saltar nunca. Aquí se cuentan filas reales, y hay
   una prueba dedicada a que así siga siendo.
"""
import os
import sys
from datetime import date, timedelta

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

from core.deps import hay_cupo, licencia_de, licencia_vigente  # noqa: E402
from models import (  # noqa: E402
    EstadoLicencia, Institucion, Licencia, RolUsuario, TipoLicencia, Usuario,
)

RUTA = "./test_licencia_vigencia.db"


@pytest.fixture
def db():
    if os.path.exists(RUTA):
        os.remove(RUTA)
    motor = create_engine(f"sqlite:///{RUTA}")
    SQLModel.metadata.create_all(motor)
    with Session(motor) as sesion:
        yield sesion
    motor.dispose()
    if os.path.exists(RUTA):
        os.remove(RUTA)


def _institucion(db, nombre="Clínica de prueba") -> Institucion:
    i = Institucion(
        nit=f"NIT-{nombre}", razon_social=nombre, nombre_comercial=nombre,
        direccion="Calle 1", ciudad="Medellín", departamento="Antioquia",
        telefono="6041234567", email="x@y.co", representante_legal="Quien sea",
        identificacion_representante="1", codigo_prestador="050010000001",
    )
    db.add(i)
    db.commit()
    db.refresh(i)
    return i


def _licencia(db, institucion, *, vence_en_dias=365, empieza_hace_dias=1,
              estado=EstadoLicencia.ACTIVA, max_usuarios=5, max_medicos=2):
    lic = Licencia(
        institucion_id=institucion.id,
        tipo_licencia=TipoLicencia.ENTERPRISE,
        estado=estado,
        fecha_inicio=date.today() - timedelta(days=empieza_hace_dias),
        fecha_vencimiento=date.today() + timedelta(days=vence_en_dias),
        max_usuarios=max_usuarios,
        max_medicos=max_medicos,
        modulos_habilitados=["CITAS"],
        clave_licencia=f"CLAVE-{institucion.id}-{estado.value}-{vence_en_dias}",
        hash_validacion="no-se-valida-en-estas-pruebas",
    )
    db.add(lic)
    db.commit()
    return lic


def _usuario(db, institucion, *, rol=RolUsuario.ADMIN, username=None):
    u = Usuario(
        username=username or f"u{id(object()) % 100000}",
        hashed_password="x",
        nombre_completo="Quien sea",
        rol=rol,
        institucion_id=institucion.id if institucion else None,
        activo=True,
    )
    db.add(u)
    db.commit()
    db.refresh(u)
    return u


# --------------------------------------------------------------- vigencia

def test_una_licencia_al_dia_es_vigente(db):
    inst = _institucion(db)
    _licencia(db, inst, vence_en_dias=30)
    assert licencia_vigente(_usuario(db, inst), db) == (True, "")


def test_una_licencia_vencida_ayer_NO_es_vigente(db):
    inst = _institucion(db)
    _licencia(db, inst, vence_en_dias=-1)
    vigente, motivo = licencia_vigente(_usuario(db, inst), db)
    assert not vigente
    assert "venció" in motivo, motivo


def test_el_ultimo_dia_de_vigencia_TODAVIA_vale(db):
    """El día del vencimiento se trabaja. Cortar ese día le quitaría al cliente
    una jornada que pagó, y es el tipo de detalle que se descubre en la peor
    mañana posible."""
    inst = _institucion(db)
    _licencia(db, inst, vence_en_dias=0)
    assert licencia_vigente(_usuario(db, inst), db)[0]


def test_una_licencia_que_aun_no_empieza_no_vale(db):
    inst = _institucion(db)
    _licencia(db, inst, empieza_hace_dias=-5)
    vigente, motivo = licencia_vigente(_usuario(db, inst), db)
    assert not vigente
    assert "empieza a regir" in motivo, motivo


def test_una_licencia_suspendida_no_vale_aunque_no_haya_vencido(db):
    inst = _institucion(db)
    _licencia(db, inst, vence_en_dias=365, estado=EstadoLicencia.SUSPENDIDA)
    vigente, motivo = licencia_vigente(_usuario(db, inst), db)
    assert not vigente
    assert "estado" in motivo, motivo


def test_sin_licencia_no_se_supone_que_esta_bien(db):
    """Ante la ausencia del dato se responde que NO, no que sí."""
    inst = _institucion(db)
    vigente, motivo = licencia_vigente(_usuario(db, inst), db)
    assert not vigente
    assert "no tiene licencia" in motivo, motivo


def test_al_superadmin_no_se_le_vence_una_licencia_que_no_tiene(db):
    _licencia(db, _institucion(db), vence_en_dias=-500)
    dueño = _usuario(db, None, rol=RolUsuario.SUPERADMIN)
    assert licencia_vigente(dueño, db) == (True, "")


def test_licencia_de_devuelve_la_vencida_en_vez_de_ocultarla(db):
    """Quien pregunta por la vigencia necesita VER la licencia vencida: si se
    filtrara por estado, el diagnóstico sería «no hay licencia», que es otra
    cosa y lleva a otra decisión."""
    inst = _institucion(db)
    _licencia(db, inst, vence_en_dias=-10)
    assert licencia_de(_usuario(db, inst), db) is not None


# ------------------------------------------------------------------ cupos

def test_hay_cupo_mientras_no_se_llegue_al_tope(db):
    inst = _institucion(db)
    _licencia(db, inst, max_usuarios=3)
    admin = _usuario(db, inst, username="admin")
    assert hay_cupo(admin, db, para_medico=False)[0]


def test_al_llegar_al_tope_no_cabe_uno_mas(db):
    inst = _institucion(db)
    _licencia(db, inst, max_usuarios=2)
    admin = _usuario(db, inst, username="a1")
    _usuario(db, inst, username="a2")
    cabe, motivo = hay_cupo(admin, db, para_medico=False)
    assert not cabe
    assert "permite 2" in motivo and "ya hay 2" in motivo, motivo


def test_el_cupo_de_medicos_se_cuenta_aparte(db):
    """Cinco administrativos no gastan el cupo de médicos, ni al revés."""
    inst = _institucion(db)
    _licencia(db, inst, max_usuarios=10, max_medicos=1)
    admin = _usuario(db, inst, username="admin")
    _usuario(db, inst, rol=RolUsuario.MEDICO, username="med1")

    assert hay_cupo(admin, db, para_medico=False)[0], "quedan plazas de usuario"
    assert not hay_cupo(admin, db, para_medico=True)[0], "el cupo médico se agotó"


def test_el_cupo_NO_se_lee_del_contador_que_nadie_actualiza(db):
    """La prueba que impide volver al defecto original.

    `uso_actual_usuarios` está en el modelo y ningún punto del sistema lo
    mueve. El control heredado comparaba contra él y por eso jamás saltaba.
    Aquí se deja el contador MINTIENDO —a cero con la institución llena— y se
    exige que el cupo se agote igual, porque se cuenta lo que hay.
    """
    inst = _institucion(db)
    lic = _licencia(db, inst, max_usuarios=2)
    admin = _usuario(db, inst, username="a1")
    _usuario(db, inst, username="a2")

    lic.uso_actual_usuarios = 0
    db.add(lic)
    db.commit()

    cabe, _ = hay_cupo(admin, db, para_medico=False)
    assert not cabe, (
        "el cupo se calculó con `uso_actual_usuarios`, que nadie actualiza: "
        "hay que contar las filas reales"
    )


def test_sin_licencia_no_hay_cupo(db):
    inst = _institucion(db)
    cabe, motivo = hay_cupo(_usuario(db, inst), db, para_medico=False)
    assert not cabe
    assert "no tiene licencia" in motivo, motivo
