-- PREANESTESIA y RECUPERACION (#249), los dos módulos del bloque quirúrgico.
--
-- # Lo que ya había, y por qué no bastaba
--
-- `programacion_quirurgica` tiene desde la 0047 una casilla
-- `valoracion_preanestesica` dentro de la lista de verificación de la OMS. Es un
-- BOOLEANO: dice que la valoración se hizo y no dice qué dijo. Es el mismo patrón
-- que `dispensacion` frente al MAR — una marca de que algo ocurrió, sin el
-- registro de lo ocurrido.
--
-- La casilla se queda donde está, y no se toca: la rellena quien recibe al
-- paciente en la puerta, y puede marcarse antes de que el documento se
-- transcriba. Lo que cambia es que ahora existe el documento, y la pantalla puede
-- enseñar si está — o sea que la casilla deja de ser la única evidencia.
--
-- # DOS tablas y no una con `momento`
--
-- La tentación era una sola tabla con una columna que dijera si la fila es la
-- valoración o la recuperación. Son dos hechos distintos, en dos momentos
-- distintos (días antes / horas después), con dos autores posibles y sin un solo
-- campo en común más allá de la cirugía. Meterlos en una fila obligaría a que
-- media tabla fuera nula en cada caso, y el `CHECK` que lo vigilara sería más
-- largo que las dos tablas juntas (directiva 32).
--
-- # ASA y Aldrete son escalas cerradas, y por eso van con su rango
--
-- La ASA va de I a VI y la Aldrete de 0 a 10. Un valor fuera de rango no es un
-- dato inusual: es un dato imposible, y de la Aldrete depende que el paciente
-- salga de recuperación (directiva 21).

CREATE TABLE valoracion_preanestesica (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    programacion_id    INTEGER NOT NULL REFERENCES programacion_quirurgica(id),
    -- Denormalizado desde la programación, no recibido: quien valora no elige a
    -- quién se le apunta. Misma regla que el MAR y que `dispensacion`.
    paciente_id        INTEGER NOT NULL REFERENCES paciente(id),
    -- Quien responde por la valoración. Es un acto médico con nombre.
    anestesiologo_id   INTEGER NOT NULL REFERENCES usuario(id),
    fecha              TEXT    NOT NULL,

    -- Clasificación ASA (I a VI). `asa_urgencia` es el sufijo «E» de la escala:
    -- va aparte y no pegado al número porque son dos datos —el estado físico y
    -- si la cirugía es de urgencia— y juntarlos impediría contar por estado.
    asa                INTEGER NOT NULL CHECK (asa BETWEEN 1 AND 6),
    asa_urgencia       INTEGER NOT NULL DEFAULT 0 CHECK (asa_urgencia IN (0,1)),
    -- Mallampati, 1 a 4. Nulable: no siempre se explora.
    mallampati         INTEGER CHECK (mallampati IS NULL OR mallampati BETWEEN 1 AND 4),
    -- Horas de ayuno declaradas. Nulable, y NO se rellena con un supuesto: el
    -- ayuno insuficiente es causa de suspensión, y un cero inventado es peor que
    -- un hueco.
    ayuno_horas        INTEGER CHECK (ayuno_horas IS NULL OR ayuno_horas >= 0),

    comorbilidades     TEXT,
    plan_anestesico    TEXT,
    riesgos_explicados INTEGER NOT NULL DEFAULT 0 CHECK (riesgos_explicados IN (0,1)),

    -- El veredicto. `apto = 0` sin motivo deja al cirujano sin saber qué corregir
    -- para reprogramar, así que el motivo se exige.
    apto               INTEGER NOT NULL CHECK (apto IN (0,1)),
    motivo_no_apto     TEXT,
    observaciones      TEXT,

    creado_en          TEXT NOT NULL,

    CHECK (apto = 1 OR (motivo_no_apto IS NOT NULL AND trim(motivo_no_apto) <> ''))
);

-- Una valoración vigente por cirugía. Sin `UNIQUE` se acumularían valoraciones
-- contradictorias y nadie sabría cuál rige.
CREATE UNIQUE INDEX ux_preanestesica_programacion ON valoracion_preanestesica (programacion_id);

CREATE TABLE recuperacion_postanestesica (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    programacion_id     INTEGER NOT NULL REFERENCES programacion_quirurgica(id),
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    responsable_id      INTEGER NOT NULL REFERENCES usuario(id),

    ingreso_recuperacion TEXT   NOT NULL,
    -- NULL = sigue en recuperación. No es «salió y no se anotó»: para salir hay
    -- que puntuar y decir a dónde, y el CHECK de abajo lo exige.
    salida_recuperacion  TEXT,

    -- Aldrete, 0 a 10, al llegar y al salir.
    aldrete_ingreso      INTEGER NOT NULL CHECK (aldrete_ingreso BETWEEN 0 AND 10),
    aldrete_salida       INTEGER CHECK (aldrete_salida IS NULL OR aldrete_salida BETWEEN 0 AND 10),

    complicaciones       TEXT,
    destino              TEXT
        CHECK (destino IS NULL
               OR destino IN ('HOSPITALIZACION','DOMICILIO','UCI','FALLECIDO')),
    observaciones        TEXT,

    creado_en            TEXT NOT NULL,

    -- Salir de recuperación exige puntuar y decir a dónde. Los tres datos van
    -- juntos o no va ninguno: una salida sin Aldrete es una salida sin criterio.
    CHECK ((salida_recuperacion IS NULL AND aldrete_salida IS NULL AND destino IS NULL)
           OR (salida_recuperacion IS NOT NULL AND aldrete_salida IS NOT NULL
               AND destino IS NOT NULL)),
    -- Y salir con Aldrete BAJO se permite —hay motivos legítimos: se traslada a
    -- UCI, o fallece— pero tiene que decir por qué. La regla es «lo inusual con
    -- su razón», no «lo inusual prohibido» (directiva 21).
    CHECK (salida_recuperacion IS NULL OR aldrete_salida >= 9
           OR (observaciones IS NOT NULL AND trim(observaciones) <> ''))
);

CREATE UNIQUE INDEX ux_recuperacion_programacion ON recuperacion_postanestesica (programacion_id);
CREATE INDEX ix_recuperacion_paciente ON recuperacion_postanestesica (paciente_id, ingreso_recuperacion);
