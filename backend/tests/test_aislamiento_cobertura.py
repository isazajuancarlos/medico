"""Que TODO modelo tenga un dueño averiguable, no solo los que ya se usan.

La guardia de `test_guardia_aislamiento.py` mide una cosa: que en el código de
los routers no quede un `db.get` suelto. Eso no es lo mismo que «el acceso
cruzado está bloqueado». Un modelo nuevo, sin `institucion_id` y sin enlace,
pasaría la guardia el día que alguien escriba su router usando `obtener()` —y
`obtener()` reventaría en producción, o peor, lo dejaría pasar.

Esta prueba recorre los ochenta y tantos modelos con tabla y comprueba que cada
uno se pueda resolver: o lleva `institucion_id`, o enlaza con algo que lo lleve,
o está declarado como catálogo compartido. Si mañana se añade `RegistroVacuna`
sin decir de quién es, falla aquí y no en la clínica.
"""
import os
import sys
from datetime import date

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import models as m  # noqa: E402
from core.aislamiento import COMPARTIDOS, ENLACES, institucion_de, puede_ver  # noqa: E402
from models import Paciente, RolUsuario, Usuario  # noqa: E402

RUTA_BD = "./test_cobertura_aislamiento.db"


def modelos_con_tabla():
    for nombre in dir(m):
        objeto = getattr(m, nombre)
        # `__table__` y no `__tablename__`: SQLModel le pone nombre de tabla a
        # TODA subclase, tenga tabla o no, así que un mixin como `CamposRips`
        # —que solo declara campos compartidos— pasaba por modelo y se le exigía
        # decir de quién es. Lo que distingue a una tabla real es `__table__`,
        # que solo aparece con `table=True`.
        if (isinstance(objeto, type) and issubclass(objeto, SQLModel)
                and getattr(objeto, "__table__", None) is not None
                and objeto is not SQLModel):
            yield nombre, objeto


@pytest.mark.parametrize("nombre", sorted(n for n, _ in modelos_con_tabla()))
def test_todo_modelo_declara_de_quien_es(nombre):
    modelo = getattr(m, nombre)
    campos = modelo.model_fields

    if "institucion_id" in campos:
        return                                  # lo dice él mismo
    if nombre in COMPARTIDOS:
        return                                  # declarado sin dueño, a propósito
    if any(campo in campos for campo, _ in ENLACES):
        return                                  # cuelga de algo que sí lo dice

    pytest.fail(
        f"{nombre} no tiene `institucion_id`, no enlaza con ningún modelo que lo "
        f"tenga y no está en COMPARTIDOS. Decida de quién es antes de exponerlo: "
        f"añádale institucion_id, enlácelo con su paciente, o inclúyalo en "
        f"COMPARTIDOS si de verdad es de todas las instituciones."
    )


def test_los_compartidos_existen_de_verdad():
    """Un nombre mal escrito en COMPARTIDOS abre un hueco sin que se note: el
    modelo real deja de estar declarado y el nombre fantasma no protege nada."""
    reales = {n for n, _ in modelos_con_tabla()}
    fantasmas = COMPARTIDOS - reales
    assert not fantasmas, f"COMPARTIDOS nombra modelos inexistentes: {fantasmas}"


def test_ningun_compartido_lleva_institucion_id():
    """Si lo lleva, no es compartido: estaría declarándose público un dato que
    sí tiene dueño."""
    con_duenno = {
        n for n in COMPARTIDOS
        if "institucion_id" in getattr(m, n).model_fields
    }
    assert not con_duenno, (
        f"están en COMPARTIDOS pero tienen institucion_id: {con_duenno}"
    )


@pytest.fixture
def db():
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)
    with Session(motor) as s:
        yield s
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


def test_la_cadena_llega_al_paciente(db):
    """`Historia` no lleva institucion_id: su dueño sale del paciente."""
    db.add(Paciente(id=1, institucion_id=7, identificacion="1", nombre1="A",
                    apellido1="B", fecha_nacimiento=date(1990, 1, 1), sexo="M",
                    celular="3"))
    db.add(m.Historia(id=1, paciente_id=1, medico_id=1, codigo_historia="H",
                      motivo_consulta="x", enfermedad_actual="x",
                      diagnostico_principal="J449", plan_tratamiento="x"))
    db.commit()

    assert institucion_de(db, db.get(m.Historia, 1)) == 7


def test_dos_saltos_tambien(db):
    """`Formula` cuelga de la historia, que cuelga del paciente."""
    db.add(Paciente(id=1, institucion_id=7, identificacion="1", nombre1="A",
                    apellido1="B", fecha_nacimiento=date(1990, 1, 1), sexo="M",
                    celular="3"))
    db.add(m.Historia(id=1, paciente_id=1, medico_id=1, codigo_historia="H",
                      motivo_consulta="x", enfermedad_actual="x",
                      diagnostico_principal="J449", plan_tratamiento="x"))
    db.add(m.Formula(id=1, historia_id=1, medicamento="Salbutamol", cantidad=1,
                     dosis="2 inh", frecuencia="c/8h", duracion="5 días"))
    db.commit()

    assert institucion_de(db, db.get(m.Formula, 1)) == 7


def test_un_catalogo_no_tiene_dueño(db):
    """Los códigos CIE-10 son los mismos para todo el país: negarlos por
    institución sería cerrar de más."""
    cie = m.Cie10(codigo="J449", descripcion="EPOC no especificada")
    db.add(cie)
    db.commit()
    db.refresh(cie)
    assert institucion_de(db, cie) is None


def _usuario(institucion_id, rol=RolUsuario.MEDICO):
    return Usuario(id=1, username="u", hashed_password="h", nombre_completo="U",
                   email="u@x.co", rol=rol, institucion_id=institucion_id,
                   activo=True)


def test_puede_ver_discrimina(db):
    db.add(Paciente(id=1, institucion_id=7, identificacion="1", nombre1="A",
                    apellido1="B", fecha_nacimiento=date(1990, 1, 1), sexo="M",
                    celular="3"))
    db.commit()
    paciente = db.get(Paciente, 1)

    assert puede_ver(db, paciente, _usuario(7)), "el de su propia institución sí"
    assert not puede_ver(db, paciente, _usuario(8)), "el de otra institución no"


def test_el_superadmin_ve_todo(db):
    """Es el dueño del sistema multi-institucional, no un cliente."""
    db.add(Paciente(id=1, institucion_id=7, identificacion="1", nombre1="A",
                    apellido1="B", fecha_nacimiento=date(1990, 1, 1), sexo="M",
                    celular="3"))
    db.commit()
    assert puede_ver(db, db.get(Paciente, 1),
                     _usuario(None, RolUsuario.SUPERADMIN))
