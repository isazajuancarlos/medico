"""Facturación: un prestador no ve las facturas, glosas ni contratos de otro.

El sistema es multi-institucional y facturación era el módulo donde el
aislamiento fallaba ABIERTO, en cuatro sitios y por tres causas distintas:

1. `Factura` no tenía `institucion_id`, así que `filtrar_por_institucion` —que
   solo añadía el `WHERE` `if hasattr(model, "institucion_id")`— devolvía la
   consulta SIN ACOTAR. La llamada estaba escrita y parecía filtrar.
   Afectaba a `/facturacion/facturas` y a `/facturacion/stats`.
2. La consulta de glosas pendientes de `/stats` y el listado `/glosas` **no
   pasaban por ningún filtro**: `select(Glosa)` a secas.
3. `/contratos` tampoco filtraba, y `Contrato` estaba declarado en
   `COMPARTIDOS` como si fuera un catálogo nacional.

Lo que se expone no son números: `DetalleFactura` lleva `paciente_id`,
`historia_id` y el CUPS del procedimiento. Es qué se le facturó a qué paciente
en la clínica de al lado.

**Por qué SIEMPRE hay dos instituciones aquí**: con una sola, la consulta sin
filtro devuelve exactamente lo mismo que la filtrada, así que ninguna prueba
puede distinguirlas. Ese es el motivo de que el defecto viviera meses con 700
pruebas en verde. Cada caso mira las DOS mitades: que cada quien vea lo suyo, y
que NO vea lo del otro.
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
    Contrato, DetalleFactura, Entidad, EstadoLicencia, Factura, Glosa,
    Institucion, Licencia, RolUsuario, TipoLicencia, Usuario,
)

RUTA_BD = "./test_facturacion_aislamiento.db"

MODULOS = ["PACIENTES", "CITAS", "HISTORIA", "FACTURACION", "HOSPITALIZACION"]


def _sembrar(motor):
    """Dos prestadores con su contrato, su factura y su glosa cada uno.

    La ENTIDAD es una sola y compartida a propósito: es el catálogo nacional, y
    que las dos facturen a la misma EPS es justo el caso en que un filtro flojo
    se nota.
    """
    with Session(motor) as db:
        db.add(Entidad(id=1, nit="800", nombre="EPS Común", codigo_eps="EPS001"))
        for id_ in (1, 2):
            db.add(Institucion(
                id=id_, nit=f"90000000{id_}", razon_social=f"Prestador {id_}",
                nombre_comercial=f"Prestador {id_}", direccion="calle 1",
                ciudad="Medellín", departamento="Antioquia", telefono="600",
                email=f"{id_}@x.co", representante_legal="R",
                identificacion_representante="1", codigo_prestador=f"05001{id_}",
            ))
            db.add(Licencia(
                institucion_id=id_, tipo_licencia=TipoLicencia.BASICA,
                estado=EstadoLicencia.ACTIVA,
                fecha_inicio=date(2026, 1, 1), fecha_vencimiento=date(2027, 1, 1),
                modulos_habilitados=MODULOS,
                clave_licencia=f"clave-{id_}", hash_validacion="h",
            ))
            marca = "UNO" if id_ == 1 else "DOS"
            db.add(Contrato(
                id=id_, institucion_id=id_, entidad_id=1,
                nombre=f"Capitación{marca}", numero_contrato="001",
                fecha_inicio=date(2026, 1, 1), fecha_fin=date(2026, 12, 31),
                tipo_tarifa="ISS",
            ))
            db.add(Factura(
                # LAS DOS con el MISMO número, que es precisamente lo que el
                # `unique=True` global impedía. Que la siembra funcione ya es
                # media prueba.
                id=id_, institucion_id=id_, numero_factura="FAC-2026-00001",
                entidad_id=1, fecha_factura=date(2026, 1, 31),
                periodo_inicio=date(2026, 1, 1), periodo_fin=date(2026, 1, 31),
                valor_bruto=1000 * id_, valor_neto=1000 * id_, estado="RADICADA",
                observaciones=f"Observacion{marca}",
            ))
            db.add(DetalleFactura(
                factura_id=id_, servicio=f"Consulta{marca}", cantidad=1,
                valor_unitario=1000 * id_, valor_total=1000 * id_,
            ))
            db.add(Glosa(
                factura_id=id_, item_glosado=f"Item{marca}", motivo="tarifa",
                valor_glosado=100 * id_, fecha_glosa=date(2026, 2, 1),
                estado="PENDIENTE",
            ))
        db.commit()


def _cliente(motor, institucion_id: int) -> TestClient:
    usuario = Usuario(
        id=institucion_id, username=f"admin{institucion_id}", hashed_password="h",
        nombre_completo=f"Admin {institucion_id}", email=f"a{institucion_id}@x.co",
        rol=RolUsuario.ADMIN, institucion_id=institucion_id, activo=True,
    )

    def sesion():
        with Session(motor) as db:
            yield db

    main.app.dependency_overrides[get_current_active_user] = lambda: usuario
    main.app.dependency_overrides[get_db] = sesion
    return TestClient(main.app, raise_server_exceptions=False)


@pytest.fixture
def motor():
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    m = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(m)
    _sembrar(m)
    yield m
    main.app.dependency_overrides.clear()
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


# --- Los cuatro sitios que fallaban abierto ------------------------------------

@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_las_facturas_del_vecino_no_salen_en_el_listado(motor, mio, ajeno):
    """La mitad de arriba la pasa cualquier filtro; la de abajo es la que mide."""
    c = _cliente(motor, mio)
    r = c.get("/api/facturacion/facturas")
    assert r.status_code == 200, r.text
    cuerpo = r.text
    marca_mia = "UNO" if mio == 1 else "DOS"
    marca_ajena = "UNO" if ajeno == 1 else "DOS"
    assert f"Observacion{marca_mia}" in cuerpo, "no vio ni lo suyo"
    assert f"Observacion{marca_ajena}" not in cuerpo, "FUGA: vio la factura ajena"
    assert len(r.json()) == 1


@pytest.mark.parametrize("mio", [1, 2])
def test_el_panel_no_cuenta_ni_suma_lo_del_vecino(motor, mio):
    c = _cliente(motor, mio)
    r = c.get("/api/facturacion/stats")
    assert r.status_code == 200, r.text
    d = r.json()
    assert d["total_facturas"] == 1, d
    # Y el dinero, que es la parte que menos parece un dato personal y más dice
    # del negocio ajeno.
    assert d["glosas_pendientes"] == 1, d
    assert d["valor_glosado_pendiente"] == 100 * mio, d


@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_las_glosas_del_vecino_no_salen(motor, mio, ajeno):
    c = _cliente(motor, mio)
    r = c.get("/api/facturacion/glosas")
    assert r.status_code == 200, r.text
    marca_mia = "UNO" if mio == 1 else "DOS"
    marca_ajena = "UNO" if ajeno == 1 else "DOS"
    assert f"Item{marca_mia}" in r.text
    assert f"Item{marca_ajena}" not in r.text, "FUGA: vio la glosa ajena"


@pytest.mark.parametrize("mio, ajeno", [(1, 2), (2, 1)])
def test_los_contratos_del_vecino_no_salen(motor, mio, ajeno):
    c = _cliente(motor, mio)
    r = c.get("/api/facturacion/contratos")
    assert r.status_code == 200, r.text
    marca_mia = "UNO" if mio == 1 else "DOS"
    marca_ajena = "UNO" if ajeno == 1 else "DOS"
    assert f"Capitación{marca_mia}" in r.text
    assert f"Capitación{marca_ajena}" not in r.text, "FUGA: vio el contrato ajeno"


# --- El acceso por id, que fallaba CERRADO (el otro lado de la moneda) ---------

def test_la_factura_ajena_da_404_y_la_propia_200(motor):
    """El camino por id fallaba cerrado: TODAS las facturas daban 404, incluida
    la propia, porque `institucion_de` no sabía deducir su dueño. Es el mismo
    defecto por el otro lado — el ruidoso se quejaba y el ciego no (directiva
    23)—, así que arreglarlo tenía que arreglar los dos.
    """
    c = _cliente(motor, 1)
    assert c.get("/api/facturacion/facturas/1").status_code == 200, "la propia no se ve"
    assert c.get("/api/facturacion/facturas/2").status_code == 404, "FUGA: vio la ajena"


# --- La numeración, que revelaba el volumen del vecino -------------------------

def test_la_numeracion_no_se_comparte_entre_prestadores(motor):
    """Con la secuencia global, la primera factura del segundo prestador salía
    con el número siguiente al del primero — y ese número le dice cuántas lleva
    emitidas el vecino.
    """
    numeros = {}
    for inst in (1, 2):
        c = _cliente(motor, inst)
        r = c.post("/api/facturacion/facturas", json={
            "entidad_id": 1,
            "periodo_inicio": "2026-02-01",
            "periodo_fin": "2026-02-28",
            "detalles": [],
        })
        assert r.status_code == 201, r.text
        numeros[inst] = r.json()["numero_factura"]
    assert numeros[1] == numeros[2] == "FAC-2026-00002", (
        f"cada prestador sigue SU serie desde su propia última factura: {numeros}"
    )


def test_el_numero_de_contrato_es_unico_por_institucion(motor):
    """Los dos prestadores YA tienen su contrato «001» sembrado, y eso es lo que
    la unicidad global impedía. Repetirlo dentro de la misma institución sí
    choca.
    """
    c = _cliente(motor, 1)
    r = c.post("/api/facturacion/contratos", json={
        "entidad_id": 1, "nombre": "Otro", "numero_contrato": "001",
        "fecha_inicio": "2026-01-01", "fecha_fin": "2026-12-31",
        "tipo_tarifa": "ISS",
    })
    assert r.status_code == 400, "repetir el número DENTRO de la institución sí choca"

    r2 = c.post("/api/facturacion/contratos", json={
        "entidad_id": 1, "nombre": "Nuevo", "numero_contrato": "002",
        "fecha_inicio": "2026-01-01", "fecha_fin": "2026-12-31",
        "tipo_tarifa": "ISS",
    })
    assert r2.status_code == 201, r2.text


# --- La REGLA, que las pruebas de arriba NO miden ------------------------------
#
# Lo dijo un mutante: al restaurar el `filtrar_por_institucion` viejo —el del
# `hasattr` que falla abierto— las once pruebas de arriba SEGUÍAN EN VERDE.
# Porque el arreglo tuvo dos mitades y ellas solo ejercen una: al darle
# `institucion_id` a `Factura` y a `Contrato`, el `hasattr` viejo ya acierta.
#
# O sea que la regla nueva —fallar ruidosamente ante lo no declarado, y acotar
# por la cadena de ENLACES— no la sujetaba nada. Estas dos sí.

def test_hospitalizacion_se_acota_por_su_paciente(motor):
    """`Hospitalizacion` NO tiene `institucion_id` y aun así se acota.

    Es el caso que solo la regla nueva resuelve: cuelga de `paciente_id`, y
    `Paciente` sí tiene la columna. La cadena existía —`obtener` la recorría
    para el acceso por id— y esta capa no la usaba, así que el listado de
    hospitalizaciones activas salía SIN FILTRAR.

    Con la regla vieja esta prueba se pone roja: `hasattr` es falso, no se añade
    el `WHERE`, y el de la institución 1 ve la hospitalización de la 2.
    """
    from datetime import datetime

    from models import Cama, Hospitalizacion, Paciente

    with Session(motor) as db:
        for id_ in (1, 2):
            # `Cama` es de las que están declaradas en COMPARTIDOS: hoy el
            # inventario de camas no está segmentado. Va aquí porque
            # `hospitalizacion.cama_id` es NOT NULL, no porque la prueba mida
            # nada de ella.
            db.add(Cama(id=id_, numero=f"C{id_}", servicio="General",
                        piso=1, estado="OCUPADA"))
            marca = "UNO" if id_ == 1 else "DOS"
            db.add(Paciente(
                id=100 + id_, institucion_id=id_, identificacion=f"pac{id_}",
                nombre1=f"Paciente{marca}", apellido1="X",
                fecha_nacimiento=date(1990, 1, 1), sexo="M", celular="300",
            ))
            db.add(Hospitalizacion(
                id=id_, paciente_id=100 + id_, cama_id=id_, medico_tratante_id=id_,
                fecha_ingreso=datetime(2026, 1, 1, 8, 0),
                motivo_ingreso=f"Motivo{marca}", estado="ACTIVA",
                diagnostico_ingreso="J449",
            ))
        db.commit()

    for mio, ajeno in ((1, 2), (2, 1)):
        c = _cliente(motor, mio)
        r = c.get("/api/hospitalizacion/")
        assert r.status_code == 200, r.text
        marca_mia = "UNO" if mio == 1 else "DOS"
        marca_ajena = "UNO" if ajeno == 1 else "DOS"
        assert f"Motivo{marca_mia}" in r.text, "no vio ni la suya"
        assert f"Motivo{marca_ajena}" not in r.text, "FUGA: vio la hospitalización ajena"


def test_un_modelo_sin_dueno_declarado_se_cae_ruidosamente():
    """Un modelo que no dice de quién es NO devuelve la consulta sin acotar.

    Es la mitad que convierte el defecto en imposible en vez de en arreglado:
    el día que alguien añada una tabla clínica y olvide decir a quién pertenece,
    tiene que romperse en la primera prueba que la liste — no servir los datos
    de todos con un 200.

    Con la regla vieja esto NO levantaba nada: devolvía la consulta entera.
    """
    from sqlmodel import select as _select

    from core.deps import filtrar_por_institucion

    class ModeloHuerfano:
        """Sin `institucion_id`, sin enlaces conocidos, sin declarar."""

        __name__ = "ModeloHuerfano"

    usuario = Usuario(
        id=1, username="a", hashed_password="h", nombre_completo="A",
        email="a@x.co", rol=RolUsuario.ADMIN, institucion_id=1, activo=True,
    )
    with pytest.raises(RuntimeError, match="COMPARTIDOS"):
        filtrar_por_institucion(_select(Entidad), ModeloHuerfano, usuario)


def test_el_superadmin_sigue_viendo_todo(motor):
    """El control del anterior: la regla nueva no cierra de más.

    El SUPERADMIN no pertenece a ninguna institución —es el dueño del sistema,
    no un cliente— y su caso va primero, antes de cualquier comprobación. Sin
    esta prueba, un `raise` demasiado ansioso pasaría inadvertido.
    """
    usuario = Usuario(
        id=99, username="root", hashed_password="h", nombre_completo="Root",
        email="root@x.co", rol=RolUsuario.SUPERADMIN, institucion_id=None,
        activo=True,
    )

    def sesion():
        with Session(motor) as db:
            yield db

    main.app.dependency_overrides[get_current_active_user] = lambda: usuario
    main.app.dependency_overrides[get_db] = sesion
    c = TestClient(main.app, raise_server_exceptions=False)
    r = c.get("/api/facturacion/facturas")
    assert r.status_code == 200, r.text
    assert len(r.json()) == 2, "el superadmin tiene que ver las de las dos"


# --- Lo que el cliente NOMBRA en el cuerpo, no solo lo que lee -----------------
#
# Las pruebas de arriba miden la LECTURA. Estas miden la ESCRITURA, que es por
# donde quedaba abierto: `filtrar_por_institucion` no toca un `db.get` sobre un
# id que viene en el payload, y `_enrich_factura` devolvía el nombre de lo que
# ese id apuntara. Las dos las cazó la revisión de seguridad de la rama.

def test_una_linea_no_puede_nombrar_al_paciente_del_vecino(motor):
    """Con el `paciente_id` ajeno, `_enrich_factura` devolvía su NOMBRE COMPLETO.

    Enumerando identificadores se sacaba el censo de pacientes de la clínica de
    al lado, y «ser paciente de X» es dato de salud — el mismo argumento con el
    que este proyecto responde 404 y nunca 403.

    El comentario que había en esa línea decía «ya autorizado por el padre», y
    era falso justo ahí: el padre autorizado es la FACTURA, no el paciente que
    eligió el cliente en el cuerpo de la petición.
    """
    from models import Paciente

    with Session(motor) as db:
        for pid, inst, marca in ((701, 1, "PROPIO"), (702, 2, "AJENO")):
            db.add(Paciente(
                id=pid, institucion_id=inst, identificacion=f"doc{pid}",
                nombre1=f"Paciente{marca}", apellido1="Apellido",
                fecha_nacimiento=date(1990, 1, 1), sexo="M", celular="300",
            ))
        db.commit()

    c = _cliente(motor, 1)
    base = {"entidad_id": 1, "periodo_inicio": "2026-03-01", "periodo_fin": "2026-03-31"}

    # CONTROL: el paciente propio entra, y su nombre sale.
    r = c.post("/api/facturacion/facturas", json={
        **base, "detalles": [{"servicio": "X", "cantidad": 1, "valor_unitario": 10,
                              "paciente_id": 701}]})
    assert r.status_code == 201, r.text
    assert "PacientePROPIO" in r.text, "control: el propio sí sale"

    # LA AGUJA: el paciente ajeno se rechaza, y su nombre NO sale.
    r = c.post("/api/facturacion/facturas", json={
        **base, "detalles": [{"servicio": "X", "cantidad": 1, "valor_unitario": 10,
                              "paciente_id": 702}]})
    assert r.status_code == 404, f"FUGA: se aceptó el paciente ajeno ({r.status_code})"
    assert "PacienteAJENO" not in r.text, "FUGA: salió el nombre ajeno"

    # Y por la otra vía, añadiendo la línea después.
    fid = c.post("/api/facturacion/facturas", json={**base, "detalles": []}).json()["id"]
    r = c.post(f"/api/facturacion/facturas/{fid}/detalles",
               json={"servicio": "X", "cantidad": 1, "valor_unitario": 10,
                     "paciente_id": 702})
    assert r.status_code == 404, "FUGA por la vía de /detalles"


def test_una_factura_no_puede_colgar_del_contrato_del_vecino(motor):
    """Esta fuga la CREÓ el propio cambio que le dio dueño a `Contrato`.

    Hasta entonces estaba en `COMPARTIDOS`, así que leer su nombre era legítimo
    por declaración escrita. Al hacerlo privado se cerraron `/contratos` y
    `obtener`, y quedó abierto este camino: el `contrato_id` del cuerpo llegaba a
    `_enrich_factura`, que devolvía `contrato_nombre`. Además dejaba una factura
    de un prestador apuntando al contrato de otro — una referencia cruzada entre
    inquilinos que se queda escrita.
    """
    c = _cliente(motor, 1)
    base = {"entidad_id": 1, "periodo_inicio": "2026-04-01",
            "periodo_fin": "2026-04-30", "detalles": []}

    # CONTROL: el contrato propio (id 1) entra y su nombre sale.
    r = c.post("/api/facturacion/facturas", json={**base, "contrato_id": 1})
    assert r.status_code == 201, r.text
    assert "CapitaciónUNO" in r.text, "control: el contrato propio sí sale"

    # LA AGUJA: el contrato de la institución 2.
    r = c.post("/api/facturacion/facturas", json={**base, "contrato_id": 2})
    assert r.status_code == 404, f"FUGA: se aceptó el contrato ajeno ({r.status_code})"
    assert "CapitaciónDOS" not in r.text, "FUGA: salió el nombre del contrato ajeno"

    # Y por el PUT, que es el otro camino que la revisión midió.
    fid = c.post("/api/facturacion/facturas", json=base).json()["id"]
    r = c.put(f"/api/facturacion/facturas/{fid}", json={"contrato_id": 2})
    assert r.status_code == 404, "FUGA por la vía del PUT"
    assert "CapitaciónDOS" not in r.text


# =========================================================================
# EL ESQUEMA TIENE QUE SOSTENER TODO LO DE ARRIBA, O EL SERVIDOR NO ARRANCA
# =========================================================================
#
# Todo lo anterior se apoya en dos cosas del esquema: la columna
# `institucion_id` en `factura` y `contrato`, y el índice único POR institución
# sobre el número. Hasta el 2026-08-05 el bucle de `_run_migrations` envolvía las
# 52 sentencias en un `except` que las daba por «ya aplicadas» en DEBUG, y a
# continuación registraba «Migraciones de esquema aplicadas». O sea: si el índice
# único no entraba —porque la base ya tenía números repetidos entre
# instituciones—, el servidor arrancaba igual, la numeración volvía a ser global
# de hecho, y el registro afirmaba lo contrario. Directiva 20: el relleno no se
# cae, y por eso no se nota.
#
# `_exigir_el_aislamiento_de_facturacion` mira el ESQUEMA y no las sentencias, y
# el porqué está escrito en su docstring: son dos motores que llegan por caminos
# distintos, y exigir el camino deja sin arrancar a uno de los dos.

import database as _database  # noqa: E402
from sqlalchemy import create_engine as _create_engine, text as _text  # noqa: E402
from sqlmodel import SQLModel as _SQLModel  # noqa: E402


def _base_con_el_esquema_real(tmp_path):
    """Una base de verdad, construida como la construye el producto."""
    motor = _create_engine(f"sqlite:///{tmp_path}/aislamiento.db")
    _SQLModel.metadata.create_all(motor)
    with motor.begin() as cx:
        for tabla in ("factura", "contrato"):
            cx.execute(_text(
                f"CREATE UNIQUE INDEX IF NOT EXISTS ux_{tabla}_institucion_numero "
                f"ON {tabla}(institucion_id, numero_{tabla})"
            ))
    return motor


def test_con_el_esquema_completo_el_arranque_pasa(tmp_path):
    """CONTROL. Sin esta mitad, una comprobación que dijera «falta» a todo
    pasaría por buena y el caso rojo de abajo no probaría nada (directiva 33)."""
    motor = _base_con_el_esquema_real(tmp_path)
    _database._exigir_el_aislamiento_de_facturacion(motor)  # no levanta


@pytest.mark.parametrize("tabla,indice", _database._GARANTIAS_DE_AISLAMIENTO)
def test_sin_el_indice_unico_por_institucion_el_servidor_no_arranca(
    tmp_path, tabla, indice
):
    """LA AGUJA. Se rompe la salvaguarda EN EL ESQUEMA, que es por donde se
    rompería de verdad —una base con duplicados rechaza el índice—, no en el
    arnés."""
    motor = _base_con_el_esquema_real(tmp_path)
    with motor.begin() as cx:
        cx.execute(_text(f"DROP INDEX {indice}"))

    with pytest.raises(RuntimeError, match="NO sostiene el aislamiento"):
        _database._exigir_el_aislamiento_de_facturacion(motor)


def test_sin_la_columna_institucion_id_tampoco(tmp_path):
    """La otra mitad de la garantía: sin la columna, `filtrar_por_institucion`
    devuelve la consulta SIN ACOTAR — que es el defecto original de esta rama.

    Se reconstruye `factura` sin ella, porque SQLite no sabe quitar columnas.
    """
    motor = _base_con_el_esquema_real(tmp_path)
    with motor.begin() as cx:
        cx.execute(_text("DROP TABLE factura"))
        cx.execute(_text(
            "CREATE TABLE factura (id INTEGER PRIMARY KEY, numero_factura VARCHAR)"
        ))

    with pytest.raises(RuntimeError, match="columna factura.institucion_id"):
        _database._exigir_el_aislamiento_de_facturacion(motor)


def test_un_indice_que_existe_pero_NO_es_unico_no_cuenta(tmp_path):
    """El caso que un `name in [...]` daría por bueno. Un índice con el mismo
    nombre y sin UNIQUE acelera la consulta y no impide un solo duplicado: es
    exactamente la forma de fallo silencioso que esto existe para ver."""
    motor = _base_con_el_esquema_real(tmp_path)
    with motor.begin() as cx:
        cx.execute(_text("DROP INDEX ux_factura_institucion_numero"))
        cx.execute(_text(
            "CREATE INDEX ux_factura_institucion_numero "
            "ON factura(institucion_id, numero_factura)"
        ))

    with pytest.raises(RuntimeError, match="índice único ux_factura"):
        _database._exigir_el_aislamiento_de_facturacion(motor)
