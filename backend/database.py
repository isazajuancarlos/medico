# database.py
import os
import logging
from pathlib import Path
from sqlmodel import create_engine, SQLModel, Session
from sqlalchemy import text
from dotenv import load_dotenv

# Cargar .env desde el directorio padre (raíz del proyecto, no backend/)
_BASE_DIR = Path(__file__).resolve().parent
load_dotenv(_BASE_DIR.parent / ".env")

logger = logging.getLogger(__name__)

DATABASE_URL = os.getenv("DATABASE_URL")

# Pool size optimizado para sistemas multiusuario
engine = create_engine(DATABASE_URL, echo=False, pool_size=20, max_overflow=0)

def _run_migrations():
    """
    Aplica migraciones de esquema para columnas nuevas en tablas existentes.
    Usa IF NOT EXISTS / DO $$ para ser idempotente (seguro de ejecutar múltiples veces).
    """
    sql_migrations = [
        # --- Hospitalizacion: nuevos campos ---
        "ALTER TABLE hospitalizacion ADD COLUMN IF NOT EXISTS estado VARCHAR DEFAULT 'ACTIVA'",
        "ALTER TABLE hospitalizacion ADD COLUMN IF NOT EXISTS motivo_ingreso TEXT",
        "ALTER TABLE hospitalizacion ADD COLUMN IF NOT EXISTS diagnostico_egreso VARCHAR",
        "ALTER TABLE hospitalizacion ADD COLUMN IF NOT EXISTS resumen_clinico TEXT",

        # --- ItemInventario: campos opcionales ---
        "ALTER TABLE iteminventario ADD COLUMN IF NOT EXISTS categoria VARCHAR",
        "ALTER TABLE iteminventario ADD COLUMN IF NOT EXISTS presentacion VARCHAR",

        # --- OrdenMedicaIntrahospitalaria: nuevos flags ---
        "ALTER TABLE ordenmedicaintrahospitalaria ADD COLUMN IF NOT EXISTS activa BOOLEAN DEFAULT TRUE",
        "ALTER TABLE ordenmedicaintrahospitalaria ADD COLUMN IF NOT EXISTS ejecutada BOOLEAN DEFAULT FALSE",

        # --- Hacer nullable columnas que lo requieren ---
        """DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_name='resultado_laboratorio' AND column_name='paciente_id'
                         AND is_nullable='NO') THEN
                ALTER TABLE resultado_laboratorio ALTER COLUMN paciente_id DROP NOT NULL;
            END IF;
        END $$""",
        """DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_name='resultado_imagen' AND column_name='paciente_id'
                         AND is_nullable='NO') THEN
                ALTER TABLE resultado_imagen ALTER COLUMN paciente_id DROP NOT NULL;
            END IF;
        END $$""",
        """DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_name='iteminventario' AND column_name='fecha_vencimiento'
                         AND is_nullable='NO') THEN
                ALTER TABLE iteminventario ALTER COLUMN fecha_vencimiento DROP NOT NULL;
            END IF;
        END $$""",

        # --- Renombrar columnas (TarifaContrato) ---
        """DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_name='tarifacontrato' AND column_name='codigo_cups') THEN
                ALTER TABLE tarifacontrato RENAME COLUMN codigo_cups TO codigo_servicio;
            END IF;
        END $$""",
        """DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_name='tarifacontrato' AND column_name='valor') THEN
                ALTER TABLE tarifacontrato RENAME COLUMN valor TO valor_tarifa;
            END IF;
        END $$""",

        # --- Renombrar columna OrdenMedicaIntrahospitalaria.tipo -> tipo_orden ---
        """DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_name='ordenmedicaintrahospitalaria' AND column_name='tipo') THEN
                ALTER TABLE ordenmedicaintrahospitalaria RENAME COLUMN tipo TO tipo_orden;
            END IF;
        END $$""",

        # --- Asegurar DEFAULT en via_ingreso de hospitalizacion ---
        """DO $$ BEGIN
            ALTER TABLE hospitalizacion ALTER COLUMN via_ingreso SET DEFAULT 'PROGRAMADA';
        EXCEPTION WHEN OTHERS THEN NULL;
        END $$""",

        # --- Paciente: campos ampliados ---
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS codigo VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS telefono2 VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS acompanante VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS telefono_acompanante VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS estado_civil VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS oficio VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS pais_residencia VARCHAR DEFAULT 'Colombia'",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS departamento_residencia VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS ciudad_residencia VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS pais_nacimiento VARCHAR DEFAULT 'Colombia'",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS departamento_nacimiento VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS ciudad_nacimiento VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS nombre_padre VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS nombre_madre VARCHAR",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS direccion_padres VARCHAR",

        # --- Paciente: antecedentes estructurados (JSON) ---
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS ant_familiares_struct JSONB",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS ant_personales_struct JSONB",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS ant_ginecologicos_struct JSONB",
        "ALTER TABLE paciente ADD COLUMN IF NOT EXISTS ant_obstetricos_struct JSONB",
        # --- Historia: diagnóstico libre en anamnesis ---
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS impresion_diagnostica TEXT",

        # --- Historia: signos vitales ampliados ---
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS pabd FLOAT",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS pam VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS pulso INTEGER",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS pc FLOAT",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS peso_ideal FLOAT",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS estado_nutricional VARCHAR",

        # --- Historia: examen físico por sistemas (nuevos) ---
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_nariz VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_oidos VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_boca VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_faringe VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_pulmones VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_corazon VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_gu VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_extremidad VARCHAR",
        "ALTER TABLE historia ADD COLUMN IF NOT EXISTS ef_hallazgos TEXT",

        # --- Cita: duración + multi-tenancy ---
        "ALTER TABLE cita ADD COLUMN IF NOT EXISTS duracion_minutos INTEGER DEFAULT 30",
        "ALTER TABLE cita ADD COLUMN IF NOT EXISTS institucion_id INTEGER REFERENCES institucion(id)",

        # --- Facturación: dueño propio (2026-08-05) ---
        #
        # `Factura` no tenía forma de decir de quién era. Su única cadena de
        # enlaces acaba en `Contrato`, que está declarado en COMPARTIDOS, así que
        # `institucion_de` respondía «de TODOS» — y el filtro de los listados,
        # que solo miraba `hasattr(model, "institucion_id")`, devolvía la
        # consulta SIN ACOTAR. Las facturas de todos los prestadores de la base
        # salían por `/facturacion/facturas` y `/facturacion/stats` con HTTP 200.
        #
        # No es un número: `detallefactura` lleva `paciente_id`, `historia_id` y
        # el CUPS del procedimiento. Es qué se le facturó a qué paciente.
        #
        # `contrato` la lleva también, y esa es la mitad que arregla la causa:
        # un contrato es el acuerdo entre UN prestador y UNA entidad con SUS
        # tarifas, no un catálogo nacional. Tenerla es lo que permite sacarlo de
        # COMPARTIDOS sin dejar a `Factura` colgando.
        #
        # SIN `DEFAULT`, a propósito. Un defecto asignaría todas las filas
        # existentes a una institución inventada, que es exactamente el dato
        # falso que este proyecto lleva meses quitando. El relleno lo decide
        # `_asignar_duenio_a_facturacion`, abajo, y SOLO cuando no hay nada que
        # adivinar.
        "ALTER TABLE factura ADD COLUMN IF NOT EXISTS institucion_id INTEGER REFERENCES institucion(id)",
        "ALTER TABLE contrato ADD COLUMN IF NOT EXISTS institucion_id INTEGER REFERENCES institucion(id)",
        "CREATE INDEX IF NOT EXISTS ix_factura_institucion ON factura(institucion_id)",
        "CREATE INDEX IF NOT EXISTS ix_contrato_institucion ON contrato(institucion_id)",

        # La unicidad del número pasa de GLOBAL a POR INSTITUCIÓN.
        #
        # `numero_contrato` y `numero_factura` tenían `unique=True` a secas, o
        # sea una restricción de esquema: con dos prestadores en la misma base,
        # el segundo que numerara su contrato «001» NO PODÍA GUARDARLO. Un choque
        # entre dos clientes que no se conocen, y de paso la confirmación de que
        # el vecino ya usó ese número.
        #
        # Esto lo cazó la prueba de dos instituciones, no la lectura del código:
        # la restricción vivía en el esquema, así que arreglar solo la consulta
        # de `crear_contrato` habría dejado el `IntegrityError` intacto.
        #
        # Los nombres son los que genera PostgreSQL por defecto
        # (`<tabla>_<columna>_key`). En SQLite el DROP falla y lo traga el
        # `except` de abajo, que es correcto: allí el esquema se construye desde
        # el modelo, que ya lleva el `UniqueConstraint` del par.
        # `contrato` sí llevaba CONSTRAINT: `numero_contrato: str = Field(unique=True)`
        # emite `UNIQUE (numero_contrato)` dentro del CREATE TABLE, y PostgreSQL la
        # nombra `<tabla>_<columna>_key`. Este DROP acierta.
        "ALTER TABLE contrato DROP CONSTRAINT IF EXISTS contrato_numero_contrato_key",

        # `factura` NO, y esta es la trampa — cazada por la revisión de seguridad
        # de este mismo cambio. Su campo era
        #
        #     numero_factura: str = Field(unique=True, index=True)
        #
        # y con `unique` E `index` juntos SQLAlchemy NO emite una constraint:
        # emite un ÍNDICE ÚNICO, `ix_factura_numero_factura`. Compilado contra el
        # dialecto PostgreSQL sobre el modelo de `50b396e`:
        #
        #     contrato -> CONSTRAINT UNIQUE (numero_contrato)   <- el DROP servía
        #     factura  -> CREATE UNIQUE INDEX ix_factura_numero_factura
        #
        # Un `DROP CONSTRAINT IF EXISTS` sobre algo que es un índice **no falla y
        # no dice nada**. Así que en toda base PostgreSQL ya desplegada el único
        # GLOBAL de `factura` sobrevivía, y con la numeración ya por institución
        # el segundo prestador chocaba contra la factura del primero: 500 al
        # facturar, y un canal lateral sobre los números que el vecino ya emitió.
        #
        # NINGUNA PRUEBA PODÍA VERLO, y conviene saber por qué: en SQLite el
        # esquema se construye del modelo NUEVO, donde esa restricción ya no
        # existe. Verde en el banco y roto en producción — el modo de fallo que
        # nombra la directiva 30.
        "DROP INDEX IF EXISTS ix_factura_numero_factura",
        "CREATE INDEX IF NOT EXISTS ix_factura_numero_factura ON factura(numero_factura)",
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_contrato_institucion_numero "
        "ON contrato(institucion_id, numero_contrato)",
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_factura_institucion_numero "
        "ON factura(institucion_id, numero_factura)",
    ]

    with Session(engine) as session:
        for sql in sql_migrations:
            try:
                session.exec(text(sql))
                session.commit()
            except Exception as e:
                session.rollback()
                logger.debug(f"Migración omitida (ya aplicada): {str(e)[:80]}")

    _asignar_duenio_a_facturacion()
    logger.info("Migraciones de esquema aplicadas")


def _asignar_duenio_a_facturacion() -> None:
    """Rellena `institucion_id` en `factura` y `contrato` — SOLO si es deducible.

    La columna se añade vacía a propósito, y esto es lo que la llena. La regla es
    una sola frase: **si en la base hay exactamente UNA institución, todas las
    filas son suyas; si hay más de una, no se toca nada.**

    # Por qué esa condición y no un defecto

    Con dos prestadores dentro no hay forma de saber de quién es cada factura:
    la única pista sería el contrato, y el contrato tampoco lo declaraba. Poner
    la primera institución que salga sería inventarse a quién se le facturó qué,
    que es peor que dejarlo vacío — un dato falso no se nota nunca, y aquí
    decidiría qué prestador ve las facturas de cuál.

    Dejarlo en NULL falla CERRADO: `institucion_id = ?` no casa con NULL, así que
    esas filas no se le muestran a nadie. Se pierde acceso, no se filtra. Y se
    GRITA en el log con el recuento, porque un sistema que se queda callado
    escondiendo facturas es la otra mitad del mismo problema.

    # Por qué la condición se cumple hoy

    `medico` se despliega con UNA INSTANCIA POR CLIENTE (confirmado por Juan el
    2026-08-05), así que en toda base real hay una sola institución y el relleno
    es exacto, no una suposición. Esa es también la razón de que la fuga fuera
    latente y no un incidente: sin dos instituciones en la misma base no hay a
    quién filtrarle nada.
    """
    from sqlalchemy import text as _text

    with Session(engine) as session:
        try:
            instituciones = session.exec(_text("SELECT id FROM institucion")).all()
        except Exception as e:  # la tabla puede no existir todavía en un arranque limpio
            logger.debug(f"Sin tabla institucion todavía: {str(e)[:60]}")
            return

        ids = [fila[0] for fila in instituciones]
        if len(ids) != 1:
            # Se cuenta lo que queda sin dueño para poder decirlo. Un
            # `UPDATE … WHERE institucion_id IS NULL` que no se hace y no se
            # cuenta es indistinguible de uno que no hacía falta.
            for tabla in ("factura", "contrato"):
                try:
                    n = session.exec(
                        _text(f"SELECT COUNT(*) FROM {tabla} WHERE institucion_id IS NULL")
                    ).one()[0]
                except Exception:
                    continue
                if n:
                    logger.warning(
                        "%s: %d fila(s) SIN institución y hay %d instituciones en la base. "
                        "NO se asignan solas: no hay forma de saber de quién son, y ponerles "
                        "una sería inventarlo. Quedan invisibles para todos hasta que alguien "
                        "las asigne a mano. Filas afectadas: SELECT id FROM %s WHERE "
                        "institucion_id IS NULL",
                        tabla, n, len(ids), tabla,
                    )
            return

        unica = ids[0]
        for tabla in ("factura", "contrato"):
            try:
                r = session.exec(
                    _text(f"UPDATE {tabla} SET institucion_id = :i WHERE institucion_id IS NULL"),
                    params={"i": unica},
                )
                session.commit()
                if getattr(r, "rowcount", 0):
                    logger.info(
                        "%s: %d fila(s) asignadas a la única institución de la base (id=%d)",
                        tabla, r.rowcount, unica,
                    )
            except Exception as e:
                session.rollback()
                # WARNING y no debug: si el relleno no ocurre, el resultado
                # —facturas invisibles para todos— es indistinguible de «no
                # había nada que rellenar», y a nivel debug no lo vería nadie.
                # La otra rama de esta función ya grita; ésta merece lo mismo.
                logger.warning(
                    "%s: NO se pudo asignar la institución (%s). Las filas sin dueño "
                    "quedan invisibles para todos: SELECT id FROM %s WHERE institucion_id IS NULL",
                    tabla, str(e)[:120], tabla,
                )


def init_db():
    SQLModel.metadata.create_all(engine)
    _run_migrations()


def get_session():
    with Session(engine) as session:
        yield session
