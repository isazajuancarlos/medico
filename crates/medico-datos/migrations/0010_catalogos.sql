-- Catálogos de referencia, compartidos por todas las instituciones (no llevan
-- institucion_id: una especialidad o un medicamento son los mismos para todas).
--
-- Tres catálogos con distinta procedencia:
--  * especialidad: lista corta y estable de la práctica clínica; se siembra aquí.
--  * medicamento (CUM del INVIMA) y unidad_dispensacion (M13 del MUV): son
--    maestros oficiales grandes que carga la tarea de maestros (#77). Se crean
--    VACÍOS a propósito: un catálogo vacío da un selector vacío —honesto— y no
--    un valor inventado. No se siembran con datos de muestra que luego se
--    confundan con los reales.

CREATE TABLE especialidad (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    nombre TEXT NOT NULL UNIQUE
);

INSERT INTO especialidad (nombre) VALUES
    ('MEDICINA GENERAL'), ('MEDICINA INTERNA'), ('CARDIOLOGIA'), ('DERMATOLOGIA'),
    ('PEDIATRIA'), ('GINECOLOGIA Y OBSTETRICIA'), ('CIRUGIA GENERAL'),
    ('ORTOPEDIA Y TRAUMATOLOGIA'), ('ANESTESIOLOGIA'), ('UROLOGIA'),
    ('NEUROLOGIA'), ('NEUROCIRUGIA'), ('OTORRINOLARINGOLOGIA'), ('OFTALMOLOGIA'),
    ('PSIQUIATRIA'), ('GASTROENTEROLOGIA'), ('NEUMOLOGIA'), ('NEFROLOGIA'),
    ('ENDOCRINOLOGIA'), ('HEMATOLOGIA'), ('ONCOLOGIA'), ('REUMATOLOGIA'),
    ('CIRUGIA PLASTICA'), ('CIRUGIA CARDIOVASCULAR'), ('MEDICINA FISICA Y REHABILITACION'),
    ('PATOLOGIA'), ('RADIOLOGIA E IMAGENES'), ('MEDICINA NUCLEAR'),
    ('MEDICINA DE URGENCIAS'), ('CUIDADO INTENSIVO'), ('NEONATOLOGIA'),
    ('INFECTOLOGIA'), ('NUTRICION CLINICA'), ('PSICOLOGIA CLINICA'),
    ('TRABAJO SOCIAL'), ('TERAPIA FISICA'), ('TERAPIA OCUPACIONAL'),
    ('TERAPIA RESPIRATORIA'), ('FONOAUDIOLOGIA');

-- Medicamentos: el maestro CUM del INVIMA. Se llena en #77.
CREATE TABLE medicamento (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    codigo_cum        TEXT NOT NULL UNIQUE,
    nombre            TEXT NOT NULL,
    concentracion     TEXT NOT NULL DEFAULT '',
    forma_farmaceutica TEXT NOT NULL DEFAULT '',
    titular           TEXT
);

CREATE INDEX ix_medicamento_nombre ON medicamento (nombre);

-- Unidades mínimas de dispensación (campo M13 del RIPS), del catálogo UPR del
-- MUV. Se llena en #77.
CREATE TABLE unidad_dispensacion (
    codigo INTEGER PRIMARY KEY,
    nombre TEXT NOT NULL
);
