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
    ]

    with Session(engine) as session:
        for sql in sql_migrations:
            try:
                session.exec(text(sql))
                session.commit()
            except Exception as e:
                session.rollback()
                logger.debug(f"Migración omitida (ya aplicada): {str(e)[:80]}")

    logger.info("Migraciones de esquema aplicadas")


def init_db():
    SQLModel.metadata.create_all(engine)
    _run_migrations()


def get_session():
    with Session(engine) as session:
        yield session
