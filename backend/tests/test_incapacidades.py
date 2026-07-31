"""Incapacidades: el cálculo de fechas y el aislamiento.

El día de más: las dos versiones anteriores —el FoxPro y la NiceGUI— calculaban
`fecha_fin = fecha_inicio + timedelta(days=dias)`. Una incapacidad de un día que
empieza el lunes terminaba el martes. La fecha de fin es la que dice cuándo se
reincorpora el trabajador y hasta cuándo reconoce la EPS, así que un día de más
en cada una es dinero y es una fecha equivocada en un documento que firma el
médico.
"""
import os
import sys
from datetime import date, datetime, timedelta

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from incapacidades import (  # noqa: E402
    DiasNoValidos,
    Resumen,
    desde_historia,
    periodo,
    resumir,
)


class Atencion:
    """Una consulta, sin tocar la base."""

    def __init__(self, dias=0, incapacita=None, fecha=None):
        self.genera_incapacidad = incapacita if incapacita is not None else dias > 0
        self.dias_incapacidad = dias
        self.fecha = fecha or datetime(2026, 2, 2, 9, 30)


class TestElDiaDeMas:
    def test_una_incapacidad_de_UN_dia_empieza_y_termina_el_mismo_dia(self):
        p = periodo(date(2026, 2, 2), 1)
        assert p.inicio == date(2026, 2, 2)
        assert p.fin == date(2026, 2, 2)

    def test_tres_dias_desde_el_lunes_terminan_el_miercoles(self):
        # lunes 2, martes 3, miércoles 4. NO el jueves.
        p = periodo(date(2026, 2, 2), 3)
        assert p.fin == date(2026, 2, 4)

    def test_la_formula_vieja_daba_uno_de_mas(self):
        """Fija la diferencia, para que nadie la reintroduzca «arreglando»."""
        inicio, dias = date(2026, 2, 2), 5
        vieja = inicio + timedelta(days=dias)
        assert periodo(inicio, dias).fin == vieja - timedelta(days=1)

    @pytest.mark.parametrize("dias", [1, 2, 3, 7, 15, 30, 90, 180])
    def test_los_dias_cubiertos_son_exactamente_los_declarados(self, dias):
        p = periodo(date(2026, 1, 15), dias)
        cubiertos = (p.fin - p.inicio).days + 1
        assert cubiertos == dias

    def test_cruza_bien_el_fin_de_mes(self):
        p = periodo(date(2026, 1, 30), 5)
        assert p.fin == date(2026, 2, 3)

    def test_cruza_bien_un_29_de_febrero(self):
        p = periodo(date(2028, 2, 27), 4)   # 2028 es bisiesto
        assert p.fin == date(2028, 3, 1)


class TestPeriodoValido:
    @pytest.mark.parametrize("dias", [0, -1, -30])
    def test_cero_o_menos_no_es_una_incapacidad(self, dias):
        with pytest.raises(DiasNoValidos):
            periodo(date(2026, 2, 2), dias)

    def test_el_mensaje_dice_qué_hacer(self):
        with pytest.raises(DiasNoValidos) as e:
            periodo(date(2026, 2, 2), 0)
        assert "no marque la casilla" in str(e.value)

    def test_cubre_dice_si_un_dia_esta_dentro(self):
        p = periodo(date(2026, 2, 2), 3)
        assert p.cubre(date(2026, 2, 2))
        assert p.cubre(date(2026, 2, 4))
        assert not p.cubre(date(2026, 2, 1))
        assert not p.cubre(date(2026, 2, 5))


class TestDesdeHistoria:
    def test_una_consulta_que_no_incapacita_devuelve_None(self):
        assert desde_historia(Atencion(dias=0, incapacita=False)) is None

    def test_marcada_pero_con_cero_dias_tambien_devuelve_None(self):
        """«No hubo incapacidad» y «hubo una de cero días» son cosas distintas
        y no deben verse igual en la lista."""
        assert desde_historia(Atencion(dias=0, incapacita=True)) is None

    def test_toma_la_fecha_de_la_consulta_como_inicio(self):
        p = desde_historia(Atencion(dias=3, fecha=datetime(2026, 5, 10, 14, 0)))
        assert p.inicio == date(2026, 5, 10)
        assert p.fin == date(2026, 5, 12)

    def test_sin_fecha_no_inventa_una(self):
        a = Atencion(dias=3)
        a.fecha = None
        assert desde_historia(a) is None


class TestResumen:
    def test_cuenta_solo_las_que_incapacitan(self):
        r = resumir([Atencion(dias=3), Atencion(dias=0, incapacita=False),
                     Atencion(dias=5)])
        assert r.cantidad == 2
        assert r.dias_totales == 8

    def test_el_promedio_se_redondea_a_un_decimal(self):
        r = resumir([Atencion(dias=3), Atencion(dias=4)])
        assert r.promedio_dias == 3.5

    def test_un_periodo_vacio_no_divide_por_cero(self):
        r = resumir([])
        assert r.cantidad == 0
        assert r.promedio_dias == 0.0
        assert "Sin incapacidades" in str(r)

    def test_aguanta_dias_en_None(self):
        """Viene de la base, donde el campo admite nulo."""
        a = Atencion(dias=3)
        a.dias_incapacidad = None
        assert resumir([a]).dias_totales == 0

    def test_el_resumen_se_lee(self):
        assert "2 incapacidades" in str(Resumen(cantidad=2, dias_totales=8))


# --- El camino real, por HTTP, con dos instituciones ----------------------

from fastapi.testclient import TestClient  # noqa: E402
from sqlmodel import Session, SQLModel, create_engine  # noqa: E402

import main  # noqa: E402
from core.deps import get_current_active_user, get_db  # noqa: E402
from models import (  # noqa: E402
    EstadoLicencia, Historia, Institucion, Licencia, Paciente, RolUsuario,
    TipoLicencia, Usuario,
)

RUTA_BD = "./test_incapacidades.db"


def _historia(id_, paciente_id, dias, fecha=datetime(2026, 2, 2, 9, 0)):
    return Historia(
        id=id_, paciente_id=paciente_id, medico_id=1, codigo_historia=f"H{id_}",
        motivo_consulta="x", enfermedad_actual="x", diagnostico_principal="J449",
        plan_tratamiento="reposo", fecha=fecha,
        genera_incapacidad=dias > 0, dias_incapacidad=dias,
    )


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
                modulos_habilitados=["INCAPACIDADES"], clave_licencia=f"k{i}",
                hash_validacion="h"))
        db.add(Usuario(id=1, username="m", hashed_password="h",
                       nombre_completo="Dra. Ruiz", email="m@x.co",
                       rol=RolUsuario.MEDICO, institucion_id=1, activo=True))
        db.add(Paciente(id=10, institucion_id=1, identificacion="1111",
                        nombre1="Propio", apellido1="Mio",
                        fecha_nacimiento=date(1990, 1, 1), sexo="M", celular="3"))
        db.add(Paciente(id=20, institucion_id=2, identificacion="2222",
                        nombre1="Ajeno", apellido1="Suyo",
                        fecha_nacimiento=date(1985, 1, 1), sexo="F", celular="3"))
        db.add(_historia(400, 10, 3))
        db.add(_historia(401, 10, 0))          # consulta sin incapacidad
        db.add(_historia(500, 20, 10))         # de la OTRA institución
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


def test_lista_solo_las_de_su_institucion(cliente):
    """El módulo NiceGUI consultaba Historia sin filtro: listaba las de todas."""
    r = cliente.get("/api/incapacidades/")
    assert r.status_code == 200, r.text
    ids = [x["historia_id"] for x in r.json()]
    assert ids == [400], f"se coló algo ajeno: {ids}"


def test_no_lista_las_consultas_sin_incapacidad(cliente):
    ids = [x["historia_id"] for x in cliente.get("/api/incapacidades/").json()]
    assert 401 not in ids


def test_la_fecha_de_fin_no_lleva_el_dia_de_mas(cliente):
    fila = cliente.get("/api/incapacidades/").json()[0]
    assert fila["fecha_inicio"] == "2026-02-02"
    assert fila["fecha_fin"] == "2026-02-04", "3 días desde el lunes 2 terminan el 4"


def test_el_detalle_de_otra_institucion_da_404(cliente):
    assert cliente.get("/api/incapacidades/500").status_code == 404


def test_el_detalle_propio_trae_lo_de_la_certificacion(cliente):
    d = cliente.get("/api/incapacidades/400").json()
    assert d["paciente"] == "Propio  Mio "
    assert d["medico"] == "Dra. Ruiz"
    assert d["dias"] == 3
    assert d["observaciones"] == "reposo"


def test_el_resumen_cuenta_lo_MISMO_que_la_lista(cliente):
    """En el original el resumen salía de otra consulta, sin los filtros de la
    pantalla: se veía «12» arriba y tres filas abajo."""
    lista = cliente.get("/api/incapacidades/").json()
    resumen = cliente.get("/api/incapacidades/resumen").json()
    assert resumen["cantidad"] == len(lista)
    assert resumen["dias_totales"] == sum(x["dias"] for x in lista)


def test_el_resumen_respeta_los_filtros(cliente):
    r = cliente.get("/api/incapacidades/resumen", params={"desde": "2026-03-01"})
    assert r.json()["cantidad"] == 0
