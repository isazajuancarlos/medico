-- Órdenes médicas (laboratorio, procedimientos, remisiones) de una historia.
-- Cuelgan de la historia → paciente; el aislamiento se sigue por esa cadena.

CREATE TABLE orden (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    historia_id            INTEGER NOT NULL REFERENCES historia(id) ON DELETE CASCADE,
    fecha_orden            TEXT    NOT NULL DEFAULT (datetime('now')),
    estado                 TEXT    NOT NULL DEFAULT 'PENDIENTE',
    tipo                   TEXT    NOT NULL,
    codigo_cups            TEXT,
    descripcion            TEXT    NOT NULL,
    justificacion          TEXT    NOT NULL,
    especialidad_destino_id  INTEGER,
    institucion_destino_id   INTEGER
);

CREATE INDEX ix_orden_historia ON orden (historia_id);
