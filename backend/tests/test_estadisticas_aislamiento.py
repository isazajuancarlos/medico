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
                enfermera_triage_id=50 + i, diagnostico_ingreso="J00",
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

@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_el_panel_de_urgencias_no_cuenta_lo_del_vecino(motor, mio, ajeno):
    """Respondía **500**, y cuando dejó de responderlo contaba CERO.

    Dos defectos encadenados, y el segundo tapaba al primero:

    1. `TipoTriage.ROJO` no existe —los miembros son `TRIAGE_1..TRIAGE_5`— así
       que la ruta daba `AttributeError` a quien la llamara.
    2. Y con eso arreglado seguía contando cero, porque en SQLite
       `CAST(datetime AS DATE)` NO es un cast de fecha: aplica afinidad numérica
       y devuelve `2026`, así que la comparación es siempre falsa.

    LA PRIMERA VERSIÓN DE ESTA PRUEBA NO MEDÍA NADA, y conviene que quede
    escrito. Afirmaba las etiquetas con `for x in d["by_triage"]: x["triage"]`,
    y `by_triage` es un **diccionario**, no una lista: iterar da claves `str` y
    `"Rojo (I)"["triage"]` sería `TypeError`. No reventaba solo porque el
    diccionario **siempre llegaba vacío**, y `all(...)` sobre vacío es `True`.
    Lo midió la revisión con un mutante: cambiando una etiqueta a basura, las
    cinco pruebas seguían verdes. Una aserción que no puede ponerse roja dentro
    de un banco de aislamiento es justo lo que enseña a no leerlos.

    Con el `cast` fuera, el panel cuenta de verdad y esto puede afirmar las DOS
    mitades: lo propio se ve y lo ajeno no.
    """
    d = _cliente(motor, _admin(mio)).get("/api/estadisticas/urgencias").json()

    assert d["total"] == 1, (
        f"FUGA o ceguera: el prestador {mio} tiene 1 atención y el panel dice "
        f"{d['total']}. Si dice 2, ve la del vecino; si dice 0, no ve nada."
    )
    # `by_triage` es `{etiqueta: conteo}` — así lo tipa el frontal
    # (`estadisticas.service.ts:71`, `Record<string, number>`).
    assert d["by_triage"] == {"Amarillo (III)": 1}, (
        f"el reparto por triage no es el esperado: {d['by_triage']}. Las claves "
        "son las etiquetas legibles; si salen `TRIAGE_3` o `TipoTriage.TRIAGE_3`, "
        "la pantalla enseña el identificador interno."
    )
    # Y el motivo del vecino no aparece por ningún lado de la respuesta.
    texto = str(d)
    assert f"Motivo{ajeno}" not in texto, f"FUGA: apareció algo del prestador {ajeno}"


def test_el_panel_de_pacientes_no_revienta(motor):
    """El tercer 500 del mismo archivo, y la MISMA clase que el de `/urgencias`.

    `db.exec()` sobre un `select` de una sola columna devuelve escalares, no
    tuplas, así que `for (fn,) in fechas:` daba `TypeError`. Respondía 500
    siempre que hubiera un paciente con fecha de nacimiento — o sea, siempre.

    Entra aquí porque el docstring de este archivo abre diciendo «y ninguno
    revienta», y sin esto esa frase era falsa. Lo levantó la revisión de
    seguridad; es preexistente, no lo introduce este PR.
    """
    r = _cliente(motor, _admin(1)).get("/api/estadisticas/pacientes")
    assert r.status_code == 200, f"el panel de pacientes revienta: {r.text[:300]}"
    d = r.json()
    # Y de paso su aislamiento, que sí se puede medir aquí: este panel NO usa
    # `cast(..., Date)` para acotar por institución, así que cuenta de verdad.
    assert d["total"] == 1, (
        f"FUGA: el prestador 1 tiene 1 paciente y el panel cuenta {d['total']}"
    )
    assert d["by_grupo_edad"]["30-44"] == 1, d["by_grupo_edad"]


# =========================================================================
# 2. EL FILTRO NO PUEDE FALLAR ABIERTO
# =========================================================================

def test_un_usuario_sin_institucion_no_recibe_los_pacientes_de_todos(motor):
    """LA AGUJA. Un no-superadmin sin institución tenía `sub = None`, y las cinco
    consultas que usan `_pac_sub` hacen `if sub is not None`: le devolvían el
    agregado de TODOS los prestadores.

    Se afirma la PROPIEDAD y no el mecanismo: la respuesta no puede ser un 200
    con datos de más. Hoy quien lo para primero es el guardián del módulo
    —`modulos_de` devuelve el conjunto vacío sin institución, así que 403— y
    `_pac_sub` es la SEGUNDA línea, la que muerde si algún día ese guardián
    cambia. Afirmar el 403 ataría la prueba a la capa de arriba y dejaría de
    medir esta.

    Que `_pac_sub` levanta de verdad se mide aparte, sin pasar por HTTP, en
    `test_pac_sub_distingue_los_dos_casos_sin_institucion`. Las dos hacen falta:
    ésta dice que el sistema no sirve, aquélla dice que esta capa concreta no
    falla abierta.
    """
    huerfano = Usuario(
        id=777, username="huerfano", nombre_completo="Sin institución",
        email="h@x.co", hashed_password="x", rol=RolUsuario.MEDICO,
        institucion_id=None, activo=True,
    )
    c = _cliente(motor, huerfano)
    for ruta in ("/api/estadisticas/urgencias", "/api/estadisticas/citas",
                 "/api/estadisticas/produccion"):
        r = c.get(ruta)
        assert r.status_code != 200, (
            f"{ruta} le sirvió datos a un usuario sin institución ({r.status_code}). "
            "Da igual QUÉ lo pare —hoy es el guardián del módulo, con un 403— "
            "pero servir no puede."
        )


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
    assert r.status_code == 200, (
        "al superadmin no se le puede cerrar el panel: no tiene institución "
        f"porque es el dueño del sistema, no un cliente. {r.text[:200]}"
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
