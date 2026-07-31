-- Fórmulas (prescripciones) de una historia. Cuelgan de la historia, que cuelga
-- del paciente: el aislamiento se sigue por esa cadena. Al borrar la historia se
-- borran sus fórmulas (ON DELETE CASCADE).

CREATE TABLE formula (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    historia_id      INTEGER NOT NULL REFERENCES historia(id) ON DELETE CASCADE,
    medicamento      TEXT    NOT NULL,
    codigo_cum       TEXT,
    dosis            TEXT    NOT NULL,
    cantidad         REAL    NOT NULL,
    unidad           TEXT,
    -- Campo M15 del RIPS: el MUV exige que sea mayor que cero cuando se reporta.
    dias_tratamiento INTEGER,
    observaciones    TEXT,
    fecha_formula    TEXT    NOT NULL DEFAULT (datetime('now')),
    dispensada       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX ix_formula_historia ON formula (historia_id);
