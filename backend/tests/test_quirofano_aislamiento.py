"""Quirófano: un prestador no ve las cirugías programadas de otro.

MEDIDO ANTES DE ARREGLARLO, que es lo que hace que este archivo signifique algo.
`GET /api/quirofano/programaciones` hacía `select(ProgramacionQuirurgica)` sin
filtro alguno, y `_enrich` le añade `paciente_nombre` y `paciente_identificacion`:

    [institución 1] CirugiaUNO paciente='PacienteUNO Secreto' doc='doc1'
                    CirugiaDOS paciente='PacienteDOS Secreto' doc='doc2'  <- ajena
    [institución 2] CirugiaUNO paciente='PacienteUNO Secreto' doc='doc1'  <- ajena
                    CirugiaDOS paciente='PacienteDOS Secreto' doc='doc2'

Expone más que la fuga de facturación. Allí eran importes y nombres; aquí es qué
cirugía se le va a practicar a quién, con su documento de identidad — categoría
especial de la Ley 1581.

**Por qué SIEMPRE hay dos instituciones**: con una sola, la consulta sin filtro
devuelve exactamente lo mismo que la filtrada, así que ninguna prueba puede
distinguirlas. Es el motivo de que el defecto viviera sin verse. Cada caso mira
las DOS mitades: que cada quien vea lo suyo, y que NO vea lo del otro.
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
    EstadoLicencia, Institucion, Licencia, Paciente, ProgramacionQuirurgica,
    Quirofano, RolUsuario, TipoLicencia, Usuario,
)

RUTA_BD = "./test_quirofano_aislamiento.db"
MODULOS = ["PACIENTES", "QUIROFANO"]


def _sembrar(motor):
    """Dos prestadores, con su sala, su cirujano, su paciente y su cirugía."""
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
                id=50 + i, username=f"cir{i}", nombre_completo=f"Cirujano{marca}",
                email=f"c{i}@x.co", hashed_password="x", rol=RolUsuario.MEDICO,
                institucion_id=i, activo=True,
            ))
            db.add(Quirofano(id=i, nombre=f"Sala{marca}", institucion_id=i))
            db.add(Usuario(
                id=70 + i, username=f"anes{i}", nombre_completo=f"Anestesiologo{marca}",
                email=f"an{i}@x.co", hashed_password="x", rol=RolUsuario.ANESTESIOLOGO,
                institucion_id=i, activo=True,
            ))
        # Y una sala SIN DUEÑO, que es lo que crea `seed.py` en toda instalación
        # real: `list_salas` la enseña a todo el mundo desde siempre.
        db.add(Quirofano(id=9, nombre="SalaHuerfana", institucion_id=None))
        db.commit()
        for i in (1, 2):
            db.add(ProgramacionQuirurgica(
                id=i, quirofano_id=i, paciente_id=100 * i, cirujano_id=50 + i,
                fecha_programada=datetime(2026, 3, 1, 8, 0),
                procedimiento=f"Cirugia{'UNO' if i == 1 else 'DOS'}",
            ))
        db.commit()


def _cliente(motor, institucion_id: int) -> TestClient:
    usuario = Usuario(
        id=900 + institucion_id, username=f"adm{institucion_id}",
        nombre_completo="Admin", email=f"a{institucion_id}@x.co",
        hashed_password="x", rol=RolUsuario.ADMIN,
        institucion_id=institucion_id, activo=True,
    )

    def _db():
        with Session(motor) as s:
            yield s

    main.app.dependency_overrides[get_db] = _db
    main.app.dependency_overrides[get_current_active_user] = lambda: usuario
    return TestClient(main.app)


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


@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_las_cirugias_del_vecino_no_salen_en_el_listado(motor, mio, ajeno):
    c = _cliente(motor, mio)
    r = c.get("/api/quirofano/programaciones")
    assert r.status_code == 200, r.text
    procedimientos = [p["procedimiento"] for p in r.json()]

    marca_mia = "UNO" if mio == 1 else "DOS"
    marca_ajena = "UNO" if ajeno == 1 else "DOS"
    assert f"Cirugia{marca_mia}" in procedimientos, (
        f"el prestador {mio} no ve ni su propia cirugía: {procedimientos}"
    )
    assert f"Cirugia{marca_ajena}" not in procedimientos, (
        f"FUGA: el prestador {mio} ve la cirugía del {ajeno}: {procedimientos}"
    )


@pytest.mark.parametrize("mio", [1, 2])
def test_el_documento_del_paciente_ajeno_no_sale(motor, mio):
    """La parte que de verdad duele. `_enrich` añade el documento de identidad,
    así que la fuga del listado no era «una fila de más»: era el padrón."""
    c = _cliente(motor, mio)
    texto = c.get("/api/quirofano/programaciones").text
    ajeno = "doc2" if mio == 1 else "doc1"
    propio = "doc1" if mio == 1 else "doc2"
    assert propio in texto, "control: el documento del paciente PROPIO sí sale"
    assert ajeno not in texto, (
        f"FUGA: el prestador {mio} recibió el documento de identidad «{ajeno}» "
        "de un paciente de la otra institución"
    )


@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_no_se_programa_una_cirugia_al_paciente_del_vecino(motor, mio, ajeno):
    """El segundo agujero, de la misma familia que `insertar_detalle`.

    Sin la comprobación, se crea una programación PROPIA nombrando al paciente de
    la otra institución, y la respuesta devuelve su nombre y su documento — la
    clave foránea no protege, es global.
    """
    c = _cliente(motor, mio)
    cuerpo = {
        "quirofano_id": mio,
        "paciente_id": 100 * ajeno,
        "cirujano_id": 50 + mio,
        "fecha_programada": "2026-04-01T08:00:00",
        "procedimiento": "Colada",
    }
    r = c.post("/api/quirofano/programaciones", json=cuerpo)
    assert r.status_code == 404, (
        f"FUGA: se programó una cirugía al paciente del prestador {ajeno} "
        f"({r.status_code}): {r.text[:200]}"
    )
    assert "Secreto" not in r.text, "FUGA: salió el nombre del paciente ajeno"

    # CONTROL: con el paciente propio sí entra. Sin esto, un endpoint que
    # rechazara TODO pasaría esta prueba.
    cuerpo["paciente_id"] = 100 * mio
    r = c.post("/api/quirofano/programaciones", json=cuerpo)
    assert r.status_code == 201, f"control: el paciente propio sí debe entrar: {r.text[:200]}"


@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_no_se_programa_en_la_sala_ni_con_el_cirujano_del_vecino(motor, mio, ajeno):
    """Los otros dos identificadores del cuerpo. Una cirugía en la sala de otro
    prestador, o firmada por su cirujano, es un registro clínico inventado en su
    institución."""
    c = _cliente(motor, mio)
    base = {
        "quirofano_id": mio,
        "paciente_id": 100 * mio,
        "cirujano_id": 50 + mio,
        "fecha_programada": "2026-04-01T09:00:00",
        "procedimiento": "Colada",
    }

    ajena_sala = dict(base, quirofano_id=ajeno)
    assert c.post("/api/quirofano/programaciones", json=ajena_sala).status_code == 404, (
        "FUGA: se programó en la sala de quirófano del vecino"
    )

    ajeno_cirujano = dict(base, cirujano_id=50 + ajeno)
    assert c.post("/api/quirofano/programaciones", json=ajeno_cirujano).status_code == 404, (
        "FUGA: se programó una cirugía a nombre del cirujano del vecino"
    )

    # Y un id que no existe responde LO MISMO que uno ajeno: si distinguiera, se
    # enumeraría la plantilla del vecino probando números.
    inexistente = dict(base, cirujano_id=999999)
    assert c.post("/api/quirofano/programaciones", json=inexistente).status_code == 404


def test_el_superadmin_sigue_viendo_las_dos(motor):
    """El CONTROL del conjunto. Sin él, un filtro que devolviera SIEMPRE vacío
    pasaría todas las pruebas de arriba."""
    dueno = Usuario(
        id=999, username="dueno", nombre_completo="Dueño", email="s@x.co",
        hashed_password="x", rol=RolUsuario.SUPERADMIN, institucion_id=None, activo=True,
    )

    def _db():
        with Session(motor) as s:
            yield s

    main.app.dependency_overrides[get_db] = _db
    main.app.dependency_overrides[get_current_active_user] = lambda: dueno
    r = TestClient(main.app).get("/api/quirofano/programaciones")
    assert r.status_code == 200, r.text
    procedimientos = sorted(p["procedimiento"] for p in r.json())
    assert procedimientos == ["CirugiaDOS", "CirugiaUNO"], (
        f"el superadmin es el dueño del sistema y debe ver las dos: {procedimientos}"
    )


@pytest.mark.parametrize("mio", [1, 2])
def test_se_puede_programar_en_una_sala_SIN_DUENO(motor, mio):
    """La regresión que introdujo la primera versión de este arreglo.

    Usar `obtener()` para la sala la leía como `SIN_DUENIO` y la negaba. Suena
    bien y rompía el producto: `seed.py:706` crea las seis salas sin institución,
    así que en toda instalación sembrada —la demo incluida— NINGUNA programación
    de cirugía se podía crear. Falla cerrado, así que no era una fuga: era el
    sesgo a cerrar de la directiva 21, una regla probada contra el atacante y no
    contra la operación normal del cliente.

    `list_salas` enseña las huérfanas a todo el mundo desde antes de este PR.
    Crear contra lo que se enseña tiene que funcionar, y esta prueba es lo que
    ata las dos capas.
    """
    c = _cliente(motor, mio)
    # Primero, el control: la sala huérfana SÍ sale en el listado.
    salas = [s["nombre"] for s in c.get("/api/quirofano/salas").json()]
    assert "SalaHuerfana" in salas, f"el listado dejó de enseñarla: {salas}"

    r = c.post("/api/quirofano/programaciones", json={
        "quirofano_id": 9,
        "paciente_id": 100 * mio,
        "cirujano_id": 50 + mio,
        "fecha_programada": "2026-05-01T08:00:00",
        "procedimiento": "EnSalaSinDueno",
    })
    assert r.status_code == 201, (
        "no se puede programar en la sala que el propio listado enseña: "
        f"{r.status_code} {r.text[:200]}. Es la regresión de `seed.py`."
    )


@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_no_se_programa_con_el_anestesiologo_del_vecino(motor, mio, ajeno):
    """La rama que ninguna prueba cubría.

    La revisión lo midió: borrando `_exigir_del_ambito(... anestesiologo_id ...)`
    las nueve pruebas seguían verdes. La comprobación estaba bien puesta y
    funcionaba; lo que faltaba era que algo impidiera al próximo refactor
    llevársela.
    """
    c = _cliente(motor, mio)
    base = {
        "quirofano_id": mio,
        "paciente_id": 100 * mio,
        "cirujano_id": 50 + mio,
        "fecha_programada": "2026-05-02T08:00:00",
        "procedimiento": "ConAnestesiologo",
    }

    r = c.post("/api/quirofano/programaciones", json=dict(base, anestesiologo_id=70 + ajeno))
    assert r.status_code == 404, (
        f"FUGA: se programó con el anestesiólogo del prestador {ajeno}: "
        f"{r.status_code} {r.text[:200]}"
    )

    # CONTROL: el propio sí entra. Sin esto, negar todo anestesiólogo pasaría.
    r = c.post("/api/quirofano/programaciones", json=dict(base, anestesiologo_id=70 + mio))
    assert r.status_code == 201, f"control: el anestesiólogo propio sí debe entrar: {r.text[:200]}"
