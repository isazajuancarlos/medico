-- La licencia comercial de cada institución: cuántos usuarios admite y hasta
-- cuándo. Es DISTINTA de cualquier licencia offline de visibilidad: esta SÍ
-- limita la expansión (crear usuarios), pero nunca la lectura —eso se decide en
-- el código, no aquí—.
--
-- Una fila por institución. El cupo (`max_usuarios`) NO se compara contra un
-- contador guardado —que se queda desactualizado— sino contra las filas reales
-- de usuarios activos, al momento de crear.

CREATE TABLE licencia (
    institucion_id INTEGER PRIMARY KEY REFERENCES institucion(id) ON DELETE CASCADE,
    max_usuarios   INTEGER NOT NULL,
    -- Fecha de vencimiento, YYYY-MM-DD. NULL = perpetua (no vence).
    vence          TEXT
);
