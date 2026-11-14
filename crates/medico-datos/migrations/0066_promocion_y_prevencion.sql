-- Promoción y prevención (#249): los cuatro programas del bloque de P&P.
--
-- `CRECIMIENTO_DESARROLLO`, `PRENATAL`, `SALUD_REPRODUCTIVA` y `VACUNACION`.
--
-- # El vocabulario sale del CATÁLOGO DEL MUV, no de la memoria
--
-- Iba a escribir los códigos de programa citando la norma de cabeza. No hizo
-- falta: `medico-dominio/datos/muv_finalidad.json` **ya está vendorizado** en el
-- repositorio y es la tabla de finalidades del validador, con su código, su
-- nombre y su vigencia. Los cuatro programas se anclan ahí:
--
-- | módulo | finalidad del MUV |
-- |---|---|
-- | `CRECIMIENTO_DESARROLLO` | `11` VALORACION INTEGRAL PARA LA PROMOCION Y MANTENIMIENTO |
-- | `VACUNACION` | `14` PROTECCION ESPECIFICA |
-- | `SALUD_REPRODUCTIVA` | `19` PLANIFICACION FAMILIAR Y ANTICONCEPCION |
-- | `PRENATAL` | `23` ATENCION PARA EL CUIDADO PRENATAL |
--
-- Es la misma disciplina que con `recienNacidos`: la forma la fija el artefacto
-- que el validador usa, no lo que uno recuerde de la resolución. Y tiene premio
-- inmediato — la consulta de un control de P&P ya puede salir en el RIPS con la
-- finalidad correcta, en vez de con el `12` genérico.
--
-- # DOS formas, no cuatro
--
-- Tres de los programas son CONTROLES PERIÓDICOS —se inscribe al paciente y se
-- le ven cada tanto— y el cuarto es un REGISTRO DE DOSIS. Meter la vacuna en la
-- forma del control obligaría a que «hallazgos» y «conducta» fueran nulos
-- siempre, y a que el lote viviera en un campo libre. Son dos hechos distintos.
--
-- # El LOTE de la vacuna es obligatorio, y es la razón de ser del registro
--
-- Sin lote, el día que un biológico se retire del mercado **no se puede saber a
-- quién se le puso**. Un registro de vacunación sin lote es una lista de nombres.
-- Por eso va `NOT NULL` con `trim() <> ''` y no como campo opcional «que casi
-- siempre se llena».

CREATE TABLE inscripcion_pyp (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id     INTEGER NOT NULL REFERENCES paciente(id),
    programa        TEXT    NOT NULL
        CHECK (programa IN ('CRECIMIENTO_DESARROLLO','PRENATAL','SALUD_REPRODUCTIVA','VACUNACION')),
    -- El código de finalidad del MUV que le corresponde. Se guarda EN LA FILA y
    -- no se deduce del programa al leer: si mañana la 948 renumera, las
    -- inscripciones viejas conservan con qué código se reportaron (directiva 32
    -- — el nominal y el derivado son dos datos).
    finalidad_muv   TEXT    NOT NULL,

    fecha_inscripcion TEXT  NOT NULL,
    estado          TEXT    NOT NULL DEFAULT 'ACTIVA'
        CHECK (estado IN ('ACTIVA','EGRESADA')),
    fecha_egreso    TEXT,
    motivo_egreso   TEXT,

    creado_por      INTEGER REFERENCES usuario(id),
    creado_en       TEXT    NOT NULL,

    -- Egresar exige decir cuándo y por qué. Un egreso mudo deja sin saber si el
    -- paciente terminó el programa, se trasladó o abandonó — y de eso vive el
    -- indicador de adherencia.
    CHECK (estado = 'ACTIVA'
           OR (fecha_egreso IS NOT NULL AND motivo_egreso IS NOT NULL
               AND trim(motivo_egreso) <> ''))
);

-- Una inscripción ACTIVA por paciente y programa. Parcial, como en `defuncion`:
-- reinscribir a quien egresó es normal —un segundo embarazo— y un índice total
-- lo impediría.
CREATE UNIQUE INDEX ux_pyp_activa ON inscripcion_pyp (paciente_id, programa)
    WHERE estado = 'ACTIVA';
CREATE INDEX ix_pyp_paciente ON inscripcion_pyp (paciente_id, programa);

CREATE TABLE control_pyp (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    inscripcion_id    INTEGER NOT NULL REFERENCES inscripcion_pyp(id),
    -- Denormalizado desde la inscripción, no recibido del cliente.
    paciente_id       INTEGER NOT NULL REFERENCES paciente(id),
    profesional_id    INTEGER NOT NULL REFERENCES usuario(id),
    fecha             TEXT    NOT NULL,

    -- El número dentro del programa. Es lo que el reporte cuenta, y lo que
    -- permite ver si alguien lleva uno de tres o tres de tres.
    numero_control    INTEGER NOT NULL CHECK (numero_control >= 1),

    -- Solo del PRENATAL, y por eso nulable aquí: la regla de que en el prenatal
    -- es obligatoria vive en `revisar()`, donde se sabe de qué programa es la
    -- inscripción. Un `CHECK` que lo exigiera aquí obligaría a repetir el
    -- programa en cada control.
    semanas_gestacion INTEGER
        CHECK (semanas_gestacion IS NULL OR semanas_gestacion BETWEEN 1 AND 45),

    riesgo            TEXT    CHECK (riesgo IS NULL OR riesgo IN ('BAJO','ALTO')),
    hallazgos         TEXT,
    conducta          TEXT,
    -- Cuándo toca el siguiente. Nulable: en el último control no hay siguiente.
    proximo_control   TEXT,
    -- Lo propio de cada programa, como objeto JSON. Misma decisión que
    -- `documento_clinico.contenido`: lo que varía por programa no se modela en
    -- columnas que serían nulas tres de cada cuatro veces.
    datos             TEXT    NOT NULL DEFAULT '{}',

    creado_en         TEXT    NOT NULL
);

CREATE INDEX ix_control_pyp_inscripcion ON control_pyp (inscripcion_id, fecha);
CREATE INDEX ix_control_pyp_paciente ON control_pyp (paciente_id, fecha);

CREATE TABLE vacuna_aplicada (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id     INTEGER NOT NULL REFERENCES paciente(id),
    -- La inscripción al programa, cuando la hay. Nulable: una jornada de
    -- vacunación atiende a quien llega, sin inscribirlo a nada.
    inscripcion_id  INTEGER REFERENCES inscripcion_pyp(id),
    aplicado_por    INTEGER NOT NULL REFERENCES usuario(id),

    fecha           TEXT    NOT NULL,
    biologico       TEXT    NOT NULL,
    dosis_numero    INTEGER NOT NULL CHECK (dosis_numero >= 1),
    -- OBLIGATORIO. Sin lote, un retiro de mercado no puede encontrar a quién se
    -- le puso: el registro deja de servir para lo único para lo que existe.
    lote            TEXT    NOT NULL CHECK (trim(lote) <> ''),
    vencimiento_lote TEXT,
    via             TEXT,
    sitio_aplicacion TEXT,
    observaciones   TEXT,

    creado_en       TEXT    NOT NULL
);

CREATE INDEX ix_vacuna_paciente ON vacuna_aplicada (paciente_id, fecha);
-- Por LOTE, que es la consulta del día que hay un retiro: «¿a quién le pusimos
-- este lote?». Sin este índice esa pregunta recorre la tabla entera.
CREATE INDEX ix_vacuna_lote ON vacuna_aplicada (lote);
