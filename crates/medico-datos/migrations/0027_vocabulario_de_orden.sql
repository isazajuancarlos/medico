-- Las órdenes hablaban un idioma que la interfaz no entiende.
--
-- La 0007 puso `estado TEXT NOT NULL DEFAULT 'PENDIENTE'` y dejó `tipo` como
-- texto libre. El producto usa otros valores: `TipoOrden` y `EstadoOrden` de
-- `backend/models.py:81` son «Laboratorio», «Imagenología», «Pendiente», «En
-- Proceso»… y eso es lo que manda `historias.component.ts:1226` al crear una
-- orden y lo que compara `laboratorio.component.ts:164`.
--
-- Con las dos escrituras conviviendo:
--
--   * Una orden creada por la API en Rust nacía «PENDIENTE», así que
--     `@if (orden.estado === 'Pendiente')` era falso SIEMPRE y el botón de
--     iniciar el procesamiento no aparecía nunca.
--   * El filtro por estado de la pantalla de laboratorio no devolvía nada.
--   * Y la pantalla de laboratorio, que lista `tipo = 'Laboratorio'`, no habría
--     visto una sola orden si alguien escribía «LABORATORIO».
--
-- No es un detalle de mayúsculas: son dos maneras de escribir la misma cosa, y
-- dentro de un año nadie sabría cuál es la buena. Se elige UNA —la del producto,
-- que ya está escrita en la interfaz y en el sistema Python— y se cierra con
-- CHECK para que la otra no pueda volver a entrar por una ruta nueva.
--
-- # Y había un TERCER vocabulario, este ya en producción
--
-- `referencia.rs` —remisiones y contrarreferencia, que sí están portadas y
-- funcionando— escribía sobre esta misma tabla `tipo = 'ESPECIALISTA'` y estados
-- 'PENDIENTE'/'EN_PROCESO'/'COMPLETADA'. Dos consecuencias que nadie había visto:
--
--   * Una remisión creada desde la historia clínica nace con tipo «Remisión
--     Especialista», así que **no aparecía nunca en la bandeja de remisiones**,
--     que buscaba «ESPECIALISTA».
--   * `remisiones.component.ts:169` ya colorea comparando con 'Completada',
--     'En Proceso' y 'Cancelada' —la interfaz se escribió para el vocabulario
--     del producto—, de modo que las etiquetas salían todas del color por
--     defecto.
--
-- Añadir 'ESPECIALISTA' al CHECK habría dejado las dos escrituras conviviendo,
-- que es justo lo que este archivo existe para terminar. Se traduce, y
-- `referencia.rs` pasa a escribir lo mismo que todo lo demás.
--
-- # Las filas que ya existan
--
-- Se traducen las escrituras conocidas (las mismas palabras en mayúsculas, que
-- es lo único que este esquema pudo haber producido). Lo que no case con ninguna
-- rompe el CHECK y la migración FALLA: una orden con un estado que nadie sabe
-- leer no se convierte en «Pendiente» por conveniencia — eso sería inventar en
-- qué punto del proceso está un examen.

CREATE TABLE orden_nueva (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    historia_id             INTEGER NOT NULL REFERENCES historia(id) ON DELETE CASCADE,
    fecha_orden             TEXT    NOT NULL DEFAULT (datetime('now')),
    estado                  TEXT    NOT NULL DEFAULT 'Pendiente'
        CHECK (estado IN ('Pendiente','En Proceso','Completada','Cancelada')),
    tipo                    TEXT    NOT NULL
        CHECK (tipo IN ('Laboratorio','Imagenología','Remisión Especialista',
                        'Cirugía','Terapia','Otro')),
    codigo_cups             TEXT,
    descripcion             TEXT    NOT NULL,
    justificacion           TEXT    NOT NULL,
    especialidad_destino_id INTEGER,
    institucion_destino_id  INTEGER
);

INSERT INTO orden_nueva
    (id, historia_id, fecha_orden, estado, tipo, codigo_cups, descripcion,
     justificacion, especialidad_destino_id, institucion_destino_id)
SELECT
    id, historia_id, fecha_orden,
    CASE UPPER(estado)
        WHEN 'PENDIENTE'  THEN 'Pendiente'
        WHEN 'EN PROCESO' THEN 'En Proceso'
        WHEN 'EN_PROCESO' THEN 'En Proceso'
        WHEN 'COMPLETADA' THEN 'Completada'
        WHEN 'REALIZADA'  THEN 'Completada'
        WHEN 'CANCELADA'  THEN 'Cancelada'
        ELSE estado
    END,
    CASE UPPER(tipo)
        WHEN 'LABORATORIO'           THEN 'Laboratorio'
        WHEN 'IMAGENOLOGIA'          THEN 'Imagenología'
        WHEN 'IMAGENOLOGÍA'          THEN 'Imagenología'
        WHEN 'REMISION ESPECIALISTA' THEN 'Remisión Especialista'
        WHEN 'REMISIÓN ESPECIALISTA' THEN 'Remisión Especialista'
        -- El que usaba `referencia.rs`. Ver la nota de abajo.
        WHEN 'ESPECIALISTA'          THEN 'Remisión Especialista'
        WHEN 'CIRUGIA'               THEN 'Cirugía'
        WHEN 'CIRUGÍA'               THEN 'Cirugía'
        WHEN 'TERAPIA'               THEN 'Terapia'
        WHEN 'OTRO'                  THEN 'Otro'
        ELSE tipo
    END,
    codigo_cups, descripcion, justificacion,
    especialidad_destino_id, institucion_destino_id
FROM orden;

DROP TABLE orden;
ALTER TABLE orden_nueva RENAME TO orden;

CREATE INDEX ix_orden_historia ON orden (historia_id);
-- Las dos pantallas nuevas listan por tipo y estado; sin este índice cada carga
-- recorre todas las órdenes de la institución.
CREATE INDEX ix_orden_tipo_estado ON orden (tipo, estado);
