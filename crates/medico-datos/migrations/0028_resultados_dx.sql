-- Laboratorio e imágenes: lo que responde a una orden.
--
-- Las dos tablas cuelgan de `orden` → `historia` → `paciente` → `institucion`,
-- que es la cadena por la que se aísla todo lo clínico. No llevan
-- `institucion_id` propio, y es a propósito: un resultado pertenece a la orden
-- que lo pidió, y darle un dueño aparte permitiría que discrepen.
--
-- # Una orden, un resultado
--
-- `orden_id UNIQUE` en las dos. Un segundo resultado para la misma orden no es
-- «otra medición»: es una corrección, y una corrección que se guarda al lado de
-- la anterior deja dos respuestas a la misma pregunta sin decir cuál manda. Lo
-- que se repite es la ORDEN —el médico pide otro hemograma la semana siguiente—
-- y eso ya son dos filas de `orden`.
--
-- # Inmutable en cuanto se valida o se firma
--
-- Misma regla que la historia y el documento clínico: mientras es borrador se
-- edita; validado o firmado, no. Un resultado sobre el que un médico ya decidió
-- no se reescribe — se emite otro y se dice que corrige al primero. La regla la
-- aplica el código (`Resultado::Firmada`); aquí quedan las columnas que la
-- sostienen, incluida la de QUIÉN validó, que es la mitad que da valor a la
-- firma bajo la Ley 527 de 1999.
--
-- Nota honesta: `validado` y `firmado` son banderas, no firma criptográfica. La
-- historia clínica sí la tiene (0013); estos no. Ponerlo aquí a medias sería
-- peor que no ponerlo — ver la tarea de firma real.

CREATE TABLE resultado_laboratorio (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    orden_id            INTEGER NOT NULL UNIQUE REFERENCES orden(id) ON DELETE CASCADE,
    -- Denormalizado desde la orden, para buscar por paciente sin tres joins.
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    bioanalista_id      INTEGER NOT NULL REFERENCES usuario(id),
    fecha_resultado     TEXT    NOT NULL DEFAULT (datetime('now')),
    -- `{"hemoglobina": {"valor":"13.5","unidad":"g/dL","ref_min":"12",
    --   "ref_max":"16","critico":false}, …}`. JSON como TEXT: la forma la fija
    -- el perfil del examen, no este esquema.
    resultados_json     TEXT    NOT NULL DEFAULT '{}',
    observaciones       TEXT,
    -- DERIVADO al escribir: verdadero si lo declara quien procesa **o** si algún
    -- parámetro viene marcado crítico. Ver `laboratorio::valores_criticos_de`.
    valores_criticos    INTEGER NOT NULL DEFAULT 0,
    validado            INTEGER NOT NULL DEFAULT 0,
    fecha_validacion    TEXT,
    validado_por_id     INTEGER REFERENCES usuario(id)
);

CREATE INDEX ix_resultado_lab_paciente ON resultado_laboratorio (paciente_id, fecha_resultado);

-- Plantillas de examen: qué parámetros lleva un hemograma y con qué referencias.
-- Son de la institución: los valores de referencia de un laboratorio no son los
-- de otro, porque dependen del método y del equipo.
CREATE TABLE perfil_laboratorio (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    nombre         TEXT    NOT NULL,
    descripcion    TEXT,
    examenes       TEXT    NOT NULL DEFAULT '[]',   -- JSON
    activo         INTEGER NOT NULL DEFAULT 1,
    UNIQUE (institucion_id, nombre)
);

CREATE TABLE informe_imagen (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    orden_id               INTEGER NOT NULL UNIQUE REFERENCES orden(id) ON DELETE CASCADE,
    paciente_id            INTEGER NOT NULL REFERENCES paciente(id),
    radiologo_id           INTEGER NOT NULL REFERENCES usuario(id),
    fecha_resultado        TEXT    NOT NULL DEFAULT (datetime('now')),
    tecnica                TEXT    NOT NULL,
    hallazgos              TEXT    NOT NULL,
    impresion_diagnostica  TEXT    NOT NULL,
    recomendaciones        TEXT,
    -- BI-RADS, PI-RADS, LI-RADS… El sistema no la interpreta: la transcribe.
    clasificacion          TEXT,
    -- Identificadores de las imágenes adjuntas, JSON. Los bytes viven donde ya
    -- viven los de `imagen` (cifrados en reposo si hay clave); aquí solo el
    -- enlace, para que el informe no dependa del sistema de archivos.
    imagenes_paths         TEXT    NOT NULL DEFAULT '[]',
    firmado                INTEGER NOT NULL DEFAULT 0,
    fecha_firma            TEXT,
    firmado_por_id         INTEGER REFERENCES usuario(id)
);

CREATE INDEX ix_informe_imagen_paciente ON informe_imagen (paciente_id, fecha_resultado);
