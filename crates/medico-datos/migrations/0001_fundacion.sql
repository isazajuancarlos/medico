-- Fundación del esquema.
--
-- NO es una traducción de las 52 sentencias de `backend/database.py`. Aquéllas
-- son el HISTORIAL de cómo el esquema de PostgreSQL llegó a su forma actual
-- —`ADD COLUMN IF NOT EXISTS`, `ALTER COLUMN DROP NOT NULL`, bloques `DO $$`
-- contra `information_schema`—, no la forma en sí. Traducirlas arrastraría a
-- SQLite decisiones que solo existieron para no romper una base en producción.
--
-- Al declarar de nuevo se recupera lo que el dominio EXIGE. Varias columnas
-- acabaron siendo anulables en Postgres por migraciones sucesivas; aquí vuelven
-- a ser NOT NULL donde el dato no puede faltar.

CREATE TABLE institucion (
    id      INTEGER PRIMARY KEY,
    nombre  TEXT    NOT NULL,
    nit     TEXT    NOT NULL UNIQUE,
    activa  INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE usuario (
    id             INTEGER PRIMARY KEY,
    nombre         TEXT    NOT NULL,
    email          TEXT    NOT NULL UNIQUE,
    password_hash  TEXT    NOT NULL,
    rol            TEXT    NOT NULL,
    -- NULL solo para el superadmin, que no pertenece a ninguna institución.
    -- Es la única fila del sistema a la que se le permite, y por eso esta
    -- columna es la única del esquema sin NOT NULL por decisión y no por
    -- descuido. `RolUsuario::ambito` es quien traduce ese NULL, en un solo
    -- sitio, para que «sin institución» no pueda confundirse con «superadmin».
    institucion_id INTEGER REFERENCES institucion(id),
    activo         INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE paciente (
    id               INTEGER PRIMARY KEY,
    tipo_documento   TEXT NOT NULL,
    numero_documento TEXT NOT NULL,
    nombre1          TEXT NOT NULL,
    nombre2          TEXT,
    apellido1        TEXT NOT NULL,
    apellido2        TEXT,
    fecha_nacimiento TEXT NOT NULL,
    sexo             TEXT NOT NULL,
    -- Cinco dígitos CON el cero a la izquierda. 148 municipios —el 13 % del
    -- país— tienen código que empieza por cero, Medellín (05001) entre ellos.
    -- Va TEXT y no INTEGER exactamente por eso: guardarlo como número lo
    -- perdería y reportaría al paciente a un municipio que no existe.
    cod_municipio    TEXT,
    institucion_id   INTEGER NOT NULL REFERENCES institucion(id)
);

-- El documento es único DENTRO de la institución, no en todo el sistema: el
-- mismo paciente puede estar atendido en dos IPS y son dos filas legítimas.
-- Un UNIQUE global convertiría eso en un error de captura que nadie sabría
-- explicar en el mostrador.
CREATE UNIQUE INDEX paciente_documento_por_institucion
    ON paciente (institucion_id, tipo_documento, numero_documento);

CREATE INDEX paciente_por_institucion ON paciente (institucion_id);
