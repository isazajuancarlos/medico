"""Un usuario de una institución no puede leer ni tocar los datos de otra.

El sistema es multi-institucional: el mismo binario sirve a un consultorio, una
IPS y un hospital, y cada uno ve lo suyo. El aislamiento se hacía con
`filtrar_por_institucion`, que añade un `WHERE institucion_id = …` a un
`select()`.

El problema es que solo protege las CONSULTAS POR LISTA. Todo lo que se busca
por identificador usa `db.get(Modelo, id)`, que no pasa por ahí: basta con
adivinar o incrementar un número en la URL. En historias clínicas eso es acceso
a datos de categoría especial (Ley 1581, art. 5) de un responsable distinto.

Estas pruebas recorren el camino real —HTTP, con un usuario de la institución 1
pidiendo un registro de la institución 2— y no leen el código: comprueban el
código de respuesta.
"""
import os
import sys
from datetime import date

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import main  # noqa: E402
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    EstadoLicencia, Historia, Institucion, Licencia, Paciente, RolUsuario,
    TipoLicencia, Usuario,
)

#: Base propia de estas pruebas. NO es la de `DATABASE_URL`: el `setdefault` de
#: arriba lo gana el primer módulo que se importe, así que ese archivo lo
#: comparten todos los módulos de prueba del proceso. Borrar ESE en el teardown
#: dejaba a los demás con "attempt to write a readonly database" —30 fallos y 32
#: errores que no tenían nada que ver con el aislamiento—. Aquí se crea y se
#: borra solo el archivo propio.
RUTA_BD = "./test_aislamiento_datos.db"


@pytest.fixture
def entorno():
    """Dos instituciones, un paciente en cada una, y sesión iniciada en la 1."""
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)

    with Session(motor) as db:
        for id_, nombre, nit in [(1, "Consultorio Uno", "900000001"),
                                 (2, "Clínica Dos", "900000002")]:
            db.add(Institucion(
                id=id_, nit=nit, razon_social=nombre, nombre_comercial=nombre,
                direccion="calle 1", ciudad="Medellín", departamento="Antioquia",
                telefono="600", email=f"{id_}@x.co",
                representante_legal="R", identificacion_representante="1",
                codigo_prestador=f"05001{id_}",
            ))
            db.add(Licencia(
                institucion_id=id_, tipo_licencia=TipoLicencia.BASICA,
                estado=EstadoLicencia.ACTIVA,
                fecha_inicio=date(2026, 1, 1), fecha_vencimiento=date(2027, 1, 1),
                modulos_habilitados=["PACIENTES", "CITAS", "HISTORIA"],
                clave_licencia=f"clave-{id_}", hash_validacion="h",
            ))
        db.add(Paciente(
            id=10, institucion_id=1, identificacion="1111", nombre1="Propio",
            apellido1="Mio", fecha_nacimiento=date(1990, 1, 1), sexo="M",
            celular="300",
        ))
        db.add(Paciente(
            id=20, institucion_id=2, identificacion="2222", nombre1="Ajeno",
            apellido1="Suyo", fecha_nacimiento=date(1985, 6, 15), sexo="F",
            celular="301",
        ))
        # El médico de la institución 1, para que las historias tengan autor.
        db.add(Usuario(id=1, username="medico1", hashed_password="h",
                       nombre_completo="Médico de la Uno", email="uno@x.co",
                       rol=RolUsuario.MEDICO, institucion_id=1, activo=True))
        # Una historia en cada institución. `Historia` no lleva institucion_id:
        # su dueño se deduce del paciente, que es justo lo que se prueba.
        db.add(Historia(id=400, paciente_id=10, medico_id=1,
                        codigo_historia="H-400", motivo_consulta="propia",
                        enfermedad_actual="—", diagnostico_principal="J449",
                        plan_tratamiento="—"))
        db.add(Historia(id=500, paciente_id=20, medico_id=1,
                        codigo_historia="H-500", motivo_consulta="ajena",
                        enfermedad_actual="—", diagnostico_principal="J449",
                        plan_tratamiento="—"))
        db.commit()

    usuario = Usuario(
        id=1, username="medico1", hashed_password="h",
        nombre_completo="Médico de la Uno", email="uno@x.co",
        rol=RolUsuario.MEDICO, institucion_id=1, activo=True,
    )

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


def test_ve_su_propio_paciente(entorno):
    """Control: si esto fallara, las demás pruebas no probarían nada."""
    r = entorno.get("/api/pacientes/10")
    assert r.status_code == 200
    assert r.json()["nombre1"] == "Propio"


def test_NO_ve_el_paciente_de_otra_institucion(entorno):
    r = entorno.get("/api/pacientes/20")
    assert r.status_code == 404, (
        f"devolvió {r.status_code}: un usuario de la institución 1 leyó la "
        f"historia de un paciente de la institución 2 cambiando el id en la URL"
    )


def test_NO_puede_modificar_el_paciente_de_otra_institucion(entorno):
    r = entorno.put("/api/pacientes/20", json={"celular": "3009999999"})
    assert r.status_code == 404, (
        f"devolvió {r.status_code}: escritura sobre el paciente de otra institución"
    )


def test_NO_ve_la_historia_clinica_de_otra_institucion(entorno):
    """`Historia` no lleva `institucion_id`: cuelga del paciente. El aislamiento
    tiene que seguir esa cadena, o la historia queda desprotegida aunque el
    paciente no lo esté."""
    r = entorno.get("/api/historias/500")
    assert r.status_code == 404, (
        f"devolvió {r.status_code}: la historia clínica de un paciente ajeno"
    )


def test_si_ve_la_historia_de_su_propio_paciente(entorno):
    """Control de la prueba anterior: la cadena no puede cerrarlo todo."""
    r = entorno.get("/api/historias/400")
    assert r.status_code == 200, f"devolvió {r.status_code}: cerró de más"


def test_las_historias_de_un_paciente_ajeno_no_se_listan(entorno):
    r = entorno.get("/api/pacientes/20/historias")
    assert r.status_code == 404


def test_la_lista_si_estaba_protegida(entorno):
    """Para situar el fallo: el filtro de lista sí funcionaba. Lo que faltaba
    era el acceso por identificador."""
    r = entorno.get("/api/pacientes/")
    identificaciones = [p["identificacion"] for p in r.json()["items"]]
    assert identificaciones == ["1111"]
