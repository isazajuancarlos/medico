-- Las camas dejan de ser globales (hallazgo de la security-review, 2026-07-30).
--
-- La 0017 las creó globales «como en el original», y dejó su alcance por
-- institución «como mejora anotada». La revisión midió lo que esa nota costaba, y
-- no era solo lectura:
--
--   LECTURA: `GET /api/hospitalizacion/camas` devolvía a cualquier usuario
--   autenticado el inventario de TODAS las instituciones — número, piso,
--   servicio (UCI, PEDIATRIA, MATERNIDAD) y ocupación en vivo.
--
--   ESCRITURA, que es lo peor: `ingresar` validaba el paciente contra el ámbito
--   pero tomaba el `cama_id` del cliente. La institución A ingresaba su paciente
--   en una cama de B; B la veía OCUPADA en su listado, sin ocupante en el listado
--   de hospitalizaciones —ese sí filtrado por institución— y sin poder liberarla,
--   porque el egreso también filtra por ámbito. Una cama que B no puede usar ni
--   soltar, creada por un tercero que B no ve.
--
-- Una revisión anterior, el mismo día, había marcado esto como falso positivo
-- mirando solo la lectura: una cama no es dato personal. Cierto, y no era el
-- punto.
--
-- # Sobre las filas que ya existan
--
-- SQLite no admite añadir una columna NOT NULL con clave foránea a una tabla
-- viva, así que se rehace. Las filas se copian asignándolas a la institución
-- **si y solo si hay exactamente una** — el caso de una instalación de un solo
-- prestador, que es el único en el que la respuesta es evidente. Con cero o con
-- varias instituciones, el SELECT da NULL, la restricción NOT NULL rompe y la
-- migración FALLA: repartir camas entre instituciones es una decisión de quien
-- opera, no un reparto que invente el instalador (directiva 20).
--
-- Hoy esto no borra nada de nadie: `cama` solo se llena en una prueba.

CREATE TABLE cama_nueva (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    -- El número deja de ser único en todo el despliegue y pasa a serlo DENTRO de
    -- la institución: dos prestadores pueden tener cada uno su «101-A», y con el
    -- UNIQUE global el segundo no podía registrarla.
    numero         TEXT NOT NULL,
    piso           TEXT NOT NULL,
    servicio       TEXT NOT NULL,
    estado         TEXT NOT NULL DEFAULT 'DISPONIBLE'
        CHECK (estado IN ('DISPONIBLE','OCUPADA','EN_ASEO','BLOQUEADA','MANTENIMIENTO')),
    tiene_oxigeno  INTEGER NOT NULL DEFAULT 0,
    tiene_monitor  INTEGER NOT NULL DEFAULT 0,
    es_aislamiento INTEGER NOT NULL DEFAULT 0,
    UNIQUE (institucion_id, numero)
);

INSERT INTO cama_nueva
    (id, institucion_id, numero, piso, servicio, estado,
     tiene_oxigeno, tiene_monitor, es_aislamiento)
SELECT
    c.id,
    (SELECT CASE WHEN (SELECT COUNT(*) FROM institucion) = 1
                 THEN (SELECT id FROM institucion) END),
    c.numero, c.piso, c.servicio, c.estado,
    c.tiene_oxigeno, c.tiene_monitor, c.es_aislamiento
FROM cama c;

DROP TABLE cama;
ALTER TABLE cama_nueva RENAME TO cama;

CREATE INDEX IF NOT EXISTS ix_cama_institucion ON cama (institucion_id, estado);
