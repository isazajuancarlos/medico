-- Imágenes diagnósticas (radiografías, ecografías, fotos clínicas) de una
-- historia. El archivo vive en disco; aquí solo va la referencia. Cuelgan de la
-- historia → paciente; el aislamiento se sigue por esa cadena.
--
-- `ruta_archivo` la construye SIEMPRE el servidor desde el id de la fila; nada
-- que venga del cliente entra en ella. `nombre_original` es solo etiqueta para
-- mostrar y descargar, nunca ruta.

CREATE TABLE imagen_diagnostica (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    historia_id      INTEGER NOT NULL REFERENCES historia(id) ON DELETE CASCADE,
    ruta_archivo     TEXT    NOT NULL,
    nombre_original  TEXT    NOT NULL,
    tipo_mime        TEXT    NOT NULL,
    tamano_bytes     INTEGER NOT NULL,
    descripcion      TEXT    NOT NULL DEFAULT '',
    fecha_subida     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX ix_imagen_historia ON imagen_diagnostica (historia_id);
