"""El RIPS no se genera con datos inventados.

Lo que había en `rips_completo.py` no era un descuido suelto: era un patrón
repetido en los seis archivos. La lista completa, medida sobre el código:

    codConsulta            → "890201"    consulta de PRIMERA VEZ, siempre
    codProcedimiento       → "999999"    un CUPS que no existe
    codMedicamento         → "00000000"  un CUM que no existe
    codDiagnosticoPrincipal→ "R69"       «morbilidad desconocida» (urgencias)
    codDiagnosticoPrincipal→ "Z00"       «examen general» (hospitalización)
    codMunicipioResidencia → "05001"     Medellín, para todo el país
    causaMotivo            → "17" / "13" urgencia / enfermedad general
    vrServicio             → 0           en los CINCO archivos
    conceptoRecaudo        → "05"        en los CINCO archivos
    fecha_egreso           → now()       para quien sigue internado

Todos tienen la forma correcta. Ninguno lo rechaza el validador de estructura.
Por eso el problema aparecía después: como glosa, como rechazo del MUV, o como
un pago que no llega porque el valor iba en cero.

=== POR QUÉ NO BASTABA CON AUDITAR ===

`auditoria_rips.py` ya sabía cuáles eran supuestos —lo dice su docstring— y el
generador los mandaba igual. Saber que un dato es inventado y enviarlo de todos
modos no es auditar: es documentar la mentira. Lo que faltaba no era otro
mecanismo, era conectar el que ya existía con la decisión de generar.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_rips_supuestos.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

RUTA_BD = "./test_rips_supuestos.db"
DESDE, HASTA = date(2026, 6, 1), date(2026, 6, 30)

#: Los siete campos que el RIPS pide de cualquier servicio, completos.
COMPLETO = dict(
    modalidad_grupo_servicio="01", grupo_servicios="01", cod_servicio=328,
    via_ingreso_servicio="01", causa_motivo="13",
    valor_servicio=45000, concepto_recaudo="04", valor_pago_moderador=0,
)


@pytest.fixture
def bd(monkeypatch):
    from sqlmodel import Session, SQLModel, create_engine

    import database
    import rips_completo
    from models import (
        AtencionUrgencias, Formula, Historia, Hospitalizacion, Institucion,
        Orden, Paciente, RolUsuario, TipoOrden, Usuario,
    )

    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)
    monkeypatch.setattr(database, "engine", motor)
    monkeypatch.setattr(rips_completo, "engine", motor)

    with Session(motor) as db:
        db.add(Institucion(
            id=1, nit="900000001", razon_social="IPS", nombre_comercial="IPS",
            direccion="c", ciudad="Medellín", departamento="Antioquia",
            telefono="6", email="a@x.co", representante_legal="R",
            identificacion_representante="1", codigo_prestador="05001"))
        db.add(Usuario(id=1, username="m", hashed_password="h",
                       nombre_completo="Médica", email="m@x.co",
                       rol=RolUsuario.MEDICO, institucion_id=1, activo=True))
        db.add(Paciente(id=10, institucion_id=1, identificacion="CC10",
                        nombre1="N", apellido1="A",
                        fecha_nacimiento=date(1990, 1, 1), sexo="M",
                        celular="3", cod_municipio="05360"))
        # Una historia COMPLETA y otra a la que le falta el valor del servicio.
        db.add(Historia(id=1, paciente_id=10, medico_id=1, codigo_historia="H1",
                        motivo_consulta="x", enfermedad_actual="x",
                        diagnostico_principal="J449", plan_tratamiento="x",
                        fecha=datetime(2026, 6, 15, 9, 0),
                        cod_consulta_cups="890301", **COMPLETO))
        db.commit()
    yield motor
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


def _historia_incompleta(motor, **quitar):
    """Deja la historia 1 sin los campos indicados."""
    from sqlmodel import Session
    from models import Historia
    with Session(motor) as db:
        h = db.get(Historia, 1)
        for campo in quitar:
            setattr(h, campo, None)
        db.add(h)
        db.commit()


class TestLaHistoriaCompletaSaleSola:
    def test_una_atencion_con_todos_los_datos_genera_sin_permiso(self, bd):
        """La restricción no puede estorbar la operación normal: si el dato
        está, el archivo sale sin pedir nada. Ver
        [[directiva-verificar-el-encuadre]]."""
        import rips_completo
        ac = rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1)
        assert len(ac["usuarios"]) == 1

    def test_y_lleva_los_datos_REALES_no_los_supuestos(self, bd):
        import rips_completo
        c = rips_completo.generar_archivo_ac(
            DESDE, HASTA, institucion_id=1)["usuarios"][0]["servicios"]["consultas"][0]
        assert c["codConsulta"] == "890301", "volvió a poner el 890201 de siempre"
        assert c["vrServicio"] == 45000
        assert c["conceptoRecaudo"] == "04"

    def test_el_municipio_es_el_del_paciente_no_Medellin(self, bd):
        """`"05001"` reportaba como residente de Medellín a todo el país."""
        import rips_completo
        u = rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1)["usuarios"][0]
        assert u["codMunicipioResidencia"] == "05360"


class TestSinElDatoNoSeGenera:
    @pytest.mark.parametrize("campo,etiqueta", [
        ("valor_servicio", "valor del servicio"),
        ("cod_consulta_cups", "CUPS de la consulta"),
        ("concepto_recaudo", "concepto de recaudo"),
        ("cod_servicio", "código de servicio"),
    ])
    def test_falta_uno_y_revienta_diciendo_cual(self, bd, campo, etiqueta):
        import rips_completo
        _historia_incompleta(bd, **{campo: None})
        with pytest.raises(rips_completo.DatosIncompletos) as e:
            rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1)
        assert etiqueta in str(e.value), "el mensaje no dice qué falta"
        assert "1" in str(e.value)

    def test_el_diagnostico_principal_lo_protege_la_BASE_DE_DATOS(self, bd):
        """Este no puede faltar nunca: la columna es NOT NULL, así que el hueco
        se cierra una capa antes y el generador nunca llega a verlo.

        Se deja escrito porque es la excepción: en los demás campos la barrera
        está en el generador, y conviene saber cuál está dónde.
        """
        import sqlalchemy
        from sqlmodel import Session
        from models import Historia
        with Session(bd) as db:
            h = db.get(Historia, 1)
            h.diagnostico_principal = None
            db.add(h)
            with pytest.raises(sqlalchemy.exc.IntegrityError):
                db.commit()

    def test_CERO_no_es_lo_mismo_que_no_registrado(self, bd):
        """Un servicio puede valer cero de verdad —los hay gratuitos— y eso es
        un dato, no un hueco. Confundirlos es el error que este módulo existe
        para no cometer."""
        from sqlmodel import Session
        from models import Historia
        import rips_completo
        with Session(bd) as db:
            h = db.get(Historia, 1)
            h.valor_servicio = 0
            db.add(h)
            db.commit()
        ac = rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1)
        assert ac["usuarios"][0]["servicios"]["consultas"][0]["vrServicio"] == 0

    def test_reune_TODO_lo_que_falta_no_solo_lo_primero(self, bd):
        """Quien cierra el mes necesita la lista completa: reventar en el primer
        hueco de mil convierte el cierre en mil intentos."""
        import rips_completo
        _historia_incompleta(bd, valor_servicio=None, cod_servicio=None,
                             concepto_recaudo=None)
        with pytest.raises(rips_completo.DatosIncompletos) as e:
            rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1)
        assert len(e.value.faltantes) == 3
        campos = {f.campo for f in e.value.faltantes}
        assert campos == {"valor_servicio", "cod_servicio", "concepto_recaudo"}

    def test_la_falta_dice_en_que_registro_esta(self, bd):
        """Sin el id, la lista no sirve para corregir nada."""
        import rips_completo
        _historia_incompleta(bd, valor_servicio=None)
        with pytest.raises(rips_completo.DatosIncompletos) as e:
            rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1)
        falta = e.value.faltantes[0]
        assert falta.archivo == "AC"
        assert falta.registro_id == 1


class TestElCaminoPermisivoSePideExpresamente:
    def test_con_permiso_SI_genera_y_usa_el_supuesto(self, bd):
        """Existe, porque bloquear la facturación de un consultorio que lleva
        años sin capturar el valor sería una restricción que se acaba rodeando.
        Pero lo pide el llamante: nunca se activa solo."""
        import rips_completo
        _historia_incompleta(bd, valor_servicio=None, cod_consulta_cups=None)
        ac = rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1,
                                              permitir_supuestos=True)
        c = ac["usuarios"][0]["servicios"]["consultas"][0]
        assert c["vrServicio"] == 0
        assert c["codConsulta"] == "890201"

    def test_por_defecto_NO_esta_activo(self, bd):
        """La prueba de que el permiso no se coló como valor por defecto."""
        import inspect

        import rips_completo
        for nombre in ("generar_archivo_ac", "generar_archivo_ap",
                       "generar_archivo_am", "generar_archivo_au",
                       "generar_archivo_ah", "exportar_rips_completo"):
            firma = inspect.signature(getattr(rips_completo, nombre))
            assert firma.parameters["permitir_supuestos"].default is False, nombre


class TestLoQueNoHaTerminadoNoSeFactura:
    def test_una_urgencia_sin_egreso_no_entra(self, bd):
        """Antes se le ponía `fecha_egreso = datetime.now()`: el paciente
        seguía en urgencias y el archivo decía que salió en el instante en que
        alguien generó el RIPS."""
        from sqlmodel import Session
        from models import AtencionUrgencias, TipoTriage
        import rips_completo
        with Session(bd) as db:
            db.add(AtencionUrgencias(
                id=1, paciente_id=10, fecha_ingreso=datetime(2026, 6, 20, 3, 0),
                fecha_egreso=None, triage=TipoTriage.TRIAGE_2,
                enfermera_triage_id=1, motivo_consulta_triage="dolor",
                diagnostico_ingreso="I200", **COMPLETO))
            db.commit()
        au = rips_completo.generar_archivo_au(DESDE, HASTA, institucion_id=1)
        assert au["usuarios"] == [], "reportó una atención que no ha terminado"

    def test_una_hospitalizacion_ACTIVA_tampoco(self, bd):
        """El mapa de estados ya decía «no debería reportarse», y la reportaba
        igual con el código "00"."""
        from sqlmodel import Session
        from models import Cama, Hospitalizacion
        import rips_completo
        with Session(bd) as db:
            db.add(Cama(id=1, numero="101", piso="1", servicio="GENERAL"))
            db.add(Hospitalizacion(
                id=1, paciente_id=10, cama_id=1, medico_tratante_id=1,
                fecha_ingreso=datetime(2026, 6, 10, 8, 0), fecha_egreso=None,
                diagnostico_ingreso="J189", estado="ACTIVA", **COMPLETO))
            db.commit()
        ah = rips_completo.generar_archivo_ah(DESDE, HASTA, institucion_id=1)
        assert ah["usuarios"] == []
