-- Quirófano: salas y programación quirúrgica — el módulo QUIROFANO del catálogo.
--
-- LA SALA NACE CON DUEÑO, Y ESO CIERRA UNA PREGUNTA ABIERTA.
--
-- En el esquema Python `Quirofano.institucion_id` es opcional, y `seed.py:706`
-- crea las seis salas de la demostración SIN institución. Eso obligó al router a
-- aceptar «la mía o la de nadie», porque negar las huérfanas rompía el producto:
-- en toda instalación sembrada NINGUNA programación de cirugía se podía crear.
-- La limpieza —darles dueño y adoptar las existentes— quedó como decisión aparte
-- porque tocaba filas ya desplegadas.
--
-- Aquí esa decisión no existe: **esta tabla nace vacía**. No hay una sola fila
-- que rehospedar, así que la columna es `NOT NULL` desde el primer día y no hace
-- falta el «o de nadie» ni la regla que lo sostenía. Es la misma forma que
-- `cama` adoptó en la 0023, y por lo mismo: el número deja de ser único en todo
-- el despliegue y pasa a serlo DENTRO de la institución, para que dos
-- prestadores puedan tener cada uno su «Sala 1».
--
-- LA PROGRAMACIÓN **NO** LLEVA INSTITUCIÓN, y también es deliberado: cuelga del
-- paciente, igual que la historia clínica y el consentimiento. Su dueño es
-- demostrable en un salto (`JOIN paciente`), y una columna repetida es una
-- columna que puede discrepar de la verdad.

CREATE TABLE quirofano (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    nombre         TEXT    NOT NULL,
    tipo           TEXT    NOT NULL DEFAULT 'GENERAL'
        CHECK (tipo IN ('GENERAL','AMBULATORIA','PARTOS','ESPECIALIZADA')),
    -- `EN_USO` lo pone y lo quita el propio flujo de la cirugía, no un
    -- formulario: iniciar ocupa la sala y completar o cancelar la libera.
    estado         TEXT    NOT NULL DEFAULT 'DISPONIBLE'
        CHECK (estado IN ('DISPONIBLE','EN_USO','MANTENIMIENTO','BLOQUEADO')),
    -- JSON libre: qué tiene la sala (torre de laparoscopia, arco en C…). Va como
    -- texto y no como columnas porque el inventario de una sala varía por
    -- prestador y no se consulta por sus campos.
    equipamiento   TEXT    NOT NULL DEFAULT '{}',
    UNIQUE (institucion_id, nombre)
);

CREATE TABLE programacion_quirurgica (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    quirofano_id      INTEGER NOT NULL REFERENCES quirofano(id),
    paciente_id       INTEGER NOT NULL REFERENCES paciente(id),
    cirujano_id       INTEGER NOT NULL REFERENCES usuario(id),
    -- Puede faltar: una cirugía con anestesia local no lleva anestesiólogo.
    anestesiologo_id  INTEGER REFERENCES usuario(id),
    -- El consentimiento del procedimiento. Opcional en el esquema porque se
    -- firma antes de la cirugía pero no siempre antes de agendarla; cuando
    -- viene, la capa de datos comprueba que sea del MISMO paciente.
    consentimiento_id INTEGER REFERENCES consentimiento_informado(id),

    fecha_programada  TEXT    NOT NULL,   -- RFC 3339 UTC
    duracion_estimada_min INTEGER NOT NULL DEFAULT 60
        CHECK (duracion_estimada_min > 0),
    procedimiento     TEXT    NOT NULL,
    codigo_cups       TEXT,
    tipo_anestesia    TEXT
        CHECK (tipo_anestesia IS NULL
               OR tipo_anestesia IN ('GENERAL','REGIONAL','LOCAL','SEDACION')),

    -- Lista de verificación prequirúrgica (la de la OMS, reducida a lo que el
    -- producto ya pedía). Se rellena junto al paciente, antes de entrar.
    ayuno_verificado         INTEGER NOT NULL DEFAULT 0,
    labs_prequirurgicos      INTEGER NOT NULL DEFAULT 0,
    valoracion_preanestesica INTEGER NOT NULL DEFAULT 0,
    hemoclasificacion        TEXT,
    reserva_sangre           INTEGER NOT NULL DEFAULT 0,

    -- Ejecución. Los cinco estados son los del modelo Python; el CHECK impide
    -- que un estado inventado entre por una ruta futura y deje la fila fuera de
    -- toda transición válida.
    estado            TEXT    NOT NULL DEFAULT 'PROGRAMADA'
        CHECK (estado IN ('PROGRAMADA','EN_CURSO','COMPLETADA','CANCELADA','SUSPENDIDA')),
    hora_inicio_real  TEXT,
    hora_fin_real     TEXT,
    complicaciones    TEXT,
    nota_quirurgica   TEXT,
    hallazgos_quirurgicos TEXT,
    sangrado_estimado_ml  INTEGER
        CHECK (sangrado_estimado_ml IS NULL OR sangrado_estimado_ml >= 0),
    especimenes_patologia TEXT
);

CREATE INDEX ix_prog_quirurgica_paciente ON programacion_quirurgica (paciente_id);
CREATE INDEX ix_prog_quirurgica_sala     ON programacion_quirurgica (quirofano_id, fecha_programada);
CREATE INDEX ix_prog_quirurgica_estado   ON programacion_quirurgica (estado, fecha_programada);
