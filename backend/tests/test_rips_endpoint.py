"""El endpoint del RIPS: qué contesta cuando faltan datos, y qué deja escrito.

La decisión que se fija aquí no es técnica. Un RIPS incompleto no se genera
solo, pero **se puede forzar**, porque lo que se migre del sistema viejo no va
a traer los campos de facturación y un cierre de mes bloqueado sin alternativa
se resuelve editando la base a mano. Eso sí sería peor.

Lo que hace aceptable la salida es que no sea silenciosa:

  - hay que pedirla (`permitir_supuestos=true`, y por defecto es `false`);
  - queda en auditoría con quién la usó y cuántos valores se supusieron;
  - **el resumen viaja con el archivo**, no en un log aparte, para que quien lo
    firme vea qué lleva supuesto ANTES de firmarlo.

Sin ese último punto la salida sería lo mismo de antes con un parámetro nuevo.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

RUTA_BD = "./test_rips_endpoint.db"

COMPLETO = dict(
    modalidad_grupo_servicio="01", grupo_servicios="01", cod_servicio=328,
    via_ingreso_servicio="01", causa_motivo="13",
    valor_servicio=45000, concepto_recaudo="04", valor_pago_moderador=0,
)


@pytest.fixture
def cliente(monkeypatch):
    from fastapi.testclient import TestClient
    from sqlmodel import Session, SQLModel, create_engine

    import auditoria
    import database
    import main
    import rips_completo
    from core.deps import get_current_active_user, get_db
    from models import (
        EstadoLicencia, Historia, Institucion, Licencia, Paciente, RolUsuario,
        TipoLicencia, Usuario,
    )

    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)
    for modulo in (database, rips_completo, auditoria):
        monkeypatch.setattr(modulo, "engine", motor)

    with Session(motor) as db:
        db.add(Institucion(
            id=1, nit="900000001", razon_social="IPS", nombre_comercial="IPS",
            direccion="c", ciudad="Medellín", departamento="Antioquia",
            telefono="6", email="a@x.co", representante_legal="R",
            identificacion_representante="1", codigo_prestador="05001"))
        db.add(Licencia(
            institucion_id=1, tipo_licencia=TipoLicencia.BASICA,
            estado=EstadoLicencia.ACTIVA, fecha_inicio=date(2026, 1, 1),
            fecha_vencimiento=date(2027, 1, 1), modulos_habilitados=["RIPS"],
            clave_licencia="k1", hash_validacion="h"))
        db.add(Usuario(id=1, username="admin", hashed_password="h",
                       nombre_completo="Admin", email="a@x.co",
                       rol=RolUsuario.ADMIN, institucion_id=1, activo=True))
        db.add(Paciente(id=10, institucion_id=1, identificacion="CC10",
                        nombre1="N", apellido1="A",
                        fecha_nacimiento=date(1990, 1, 1), sexo="M",
                        celular="3", cod_municipio="05360"))
        db.add(Historia(id=1, paciente_id=10, medico_id=1, codigo_historia="H1",
                        motivo_consulta="x", enfermedad_actual="x",
                        diagnostico_principal="J449", plan_tratamiento="x",
                        fecha=datetime(2026, 6, 15, 9, 0),
                        cod_consulta_cups="890301", **COMPLETO))
        db.commit()

    admin = Usuario(id=1, username="admin", hashed_password="h",
                    nombre_completo="Admin", email="a@x.co",
                    rol=RolUsuario.ADMIN, institucion_id=1, activo=True)

    def sesion():
        with Session(motor) as db:
            yield db

    main.app.dependency_overrides[get_current_active_user] = lambda: admin
    main.app.dependency_overrides[get_db] = sesion
    with TestClient(main.app, raise_server_exceptions=False) as c:
        c.motor = motor
        yield c
    main.app.dependency_overrides.clear()
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


def _vaciar(motor, *campos):
    from sqlmodel import Session
    from models import Historia
    with Session(motor) as db:
        h = db.get(Historia, 1)
        for c in campos:
            setattr(h, c, None)
        db.add(h)
        db.commit()


PERIODO = {"inicio": "2026-06-01", "fin": "2026-06-30"}


class TestConLosDatosCompletos:
    def test_genera_sin_pedir_nada(self, cliente):
        r = cliente.get("/api/rips/", params=PERIODO)
        assert r.status_code == 200, r.text
        assert r.json()["archivos"]["AC"]["usuarios"]

    def test_y_declara_que_no_supuso_nada(self, cliente):
        """El campo va siempre, vacío o no: si solo apareciera cuando hay
        supuestos, su ausencia sería ambigua."""
        assert cliente.get("/api/rips/", params=PERIODO).json()["supuestos"] == {}


class TestSinLosDatosYSinPermiso:
    def test_no_genera_y_contesta_422(self, cliente):
        """422 y no 500: la petición es válida y el servidor está bien. Lo que
        falta son datos en las historias."""
        _vaciar(cliente.motor, "valor_servicio")
        r = cliente.get("/api/rips/", params=PERIODO)
        assert r.status_code == 422

    def test_dice_QUE_falta_y_EN_QUE_registro(self, cliente):
        """Un «faltan datos» sin la lista no permite corregir nada."""
        _vaciar(cliente.motor, "valor_servicio", "cod_servicio")
        detalle = cliente.get("/api/rips/", params=PERIODO).json()["detail"]
        campos = {f["campo"] for f in detalle["faltantes"]}
        assert campos == {"valor_servicio", "cod_servicio"}
        assert all(f["registro"] == 1 for f in detalle["faltantes"])
        assert all(f["archivo"] == "AC" for f in detalle["faltantes"])

    def test_no_deja_rastro_en_auditoria(self, cliente):
        """No se generó nada: no hay nada que auditar."""
        _vaciar(cliente.motor, "valor_servicio")
        cliente.get("/api/rips/", params=PERIODO)
        assert _auditorias(cliente.motor) == []


class TestForzandoLaGeneracion:
    def test_con_permiso_SI_genera(self, cliente):
        _vaciar(cliente.motor, "valor_servicio")
        r = cliente.get("/api/rips/", params={**PERIODO, "permitir_supuestos": "true"})
        assert r.status_code == 200, r.text
        assert r.json()["archivos"]["AC"]["usuarios"]

    def test_el_resumen_VIAJA_CON_EL_ARCHIVO(self, cliente):
        """Quien firma tiene que ver qué lleva supuesto antes de firmarlo. Si
        esto solo estuviera en un log, la salida sería lo de antes con un
        parámetro de más."""
        _vaciar(cliente.motor, "valor_servicio", "cod_consulta_cups")
        supuestos = cliente.get(
            "/api/rips/", params={**PERIODO, "permitir_supuestos": "true"}
        ).json()["supuestos"]
        assert supuestos == {"valor del servicio": 1, "CUPS de la consulta": 1}

    def test_queda_en_auditoria_con_quien_y_cuanto(self, cliente):
        _vaciar(cliente.motor, "valor_servicio")
        cliente.get("/api/rips/", params={**PERIODO, "permitir_supuestos": "true"})
        registros = _auditorias(cliente.motor)
        assert len(registros) == 1
        a = registros[0]
        assert a.accion == "GENERAR_RIPS_CON_SUPUESTOS"
        assert a.usuario_id == 1
        import json
        datos = json.loads(a.datos_despues) if isinstance(a.datos_despues, str) else a.datos_despues
        assert datos["total_supuestos"] == 1
        assert datos["periodo"] == "2026-06-01/2026-06-30"

    def test_forzar_SIN_que_falte_nada_no_ensucia_la_auditoria(self, cliente):
        """Si registrara siempre que se pasa el parámetro, el log se llenaría de
        entradas que no dicen nada y dejaría de mirarse."""
        cliente.get("/api/rips/", params={**PERIODO, "permitir_supuestos": "true"})
        assert _auditorias(cliente.motor) == []


class TestElPermisoNoSeActivaSolo:
    def test_por_defecto_el_parametro_es_false(self, cliente):
        """La prueba de que no se coló como valor por defecto en la ruta."""
        esquema = cliente.get("/openapi.json").json()
        params = esquema["paths"]["/api/rips/"]["get"]["parameters"]
        p = next(x for x in params if x["name"] == "permitir_supuestos")
        assert p["schema"]["default"] is False

    def test_el_RIPS_es_el_de_LA_INSTITUCION_DEL_USUARIO(self, cliente):
        """Con un solo usuario en la institución 1, quemar `institucion_id=1`
        en el router daba el mismo resultado y la prueba no lo notaba. Hace
        falta un segundo administrador, de otra institución, para que el
        endpoint tenga que elegir.
        """
        from sqlmodel import Session
        from models import (
            EstadoLicencia, Institucion, Licencia, Paciente, RolUsuario,
            TipoLicencia, Usuario,
        )
        import main
        from core.deps import get_current_active_user

        # id alto a propósito: `_auto_seed()` corre al arrancar la aplicación y
        # ya ocupa los primeros. Chocar con él daba un IntegrityError que no
        # tiene nada que ver con lo que aquí se prueba.
        with Session(cliente.motor) as db:
            db.add(Institucion(
                id=99, nit="900000099", razon_social="Otra", nombre_comercial="Otra",
                direccion="c", ciudad="Cali", departamento="Valle", telefono="6",
                email="b99@x.co", representante_legal="R",
                identificacion_representante="2", codigo_prestador="76001"))
            db.add(Licencia(
                institucion_id=99, tipo_licencia=TipoLicencia.BASICA,
                estado=EstadoLicencia.ACTIVA, fecha_inicio=date(2026, 1, 1),
                fecha_vencimiento=date(2027, 1, 1), modulos_habilitados=["RIPS"],
                clave_licencia="k99", hash_validacion="h"))
            db.add(Paciente(id=99, institucion_id=99, identificacion="CC99",
                            nombre1="O", apellido1="B",
                            fecha_nacimiento=date(1985, 5, 5), sexo="F",
                            celular="3", cod_municipio="76001"))
            db.commit()

        otro = Usuario(id=99, username="admin99", hashed_password="h",
                       nombre_completo="Admin 99", email="b99@x.co",
                       rol=RolUsuario.ADMIN, institucion_id=99, activo=True)
        main.app.dependency_overrides[get_current_active_user] = lambda: otro

        r = cliente.get("/api/rips/", params=PERIODO)
        assert r.status_code == 200, r.text
        cuerpo = r.json()
        assert cuerpo["archivos"]["AC"]["numDocumentoIdObligado"] == "900000099"
        # Y sobre todo: NO trae al paciente de la institución 1.
        documentos = {u["numDocumentoIdentificacion"]
                      for u in cuerpo["archivos"]["AC"]["usuarios"]}
        assert "CC10" not in documentos

    def test_la_institucion_NO_se_puede_pedir_por_parametro(self, cliente):
        """Si viniera de la petición, un administrador podría pedir el RIPS de
        otra institución. Sale del usuario autenticado y de ningún otro sitio."""
        esquema = cliente.get("/openapi.json").json()
        nombres = {p["name"] for p in esquema["paths"]["/api/rips/"]["get"]["parameters"]}
        assert "institucion_id" not in nombres


def _auditorias(motor):
    from sqlmodel import Session, select
    from models import AuditoriaAcceso
    with Session(motor) as db:
        return list(db.exec(select(AuditoriaAcceso).where(
            AuditoriaAcceso.accion == "GENERAR_RIPS_CON_SUPUESTOS")).all())
