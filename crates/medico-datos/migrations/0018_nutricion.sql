-- Nutrición (#65/#126, port a Rust de backend/routers/nutricion.py).
-- Dos entidades: la valoración nutricional y su plan de alimentación (1 activo
-- por valoración). Ambas llevan institucion_id propio —como el original— y se
-- filtran por él (no por join al paciente). El plan cuelga de la valoración con
-- borrado en cascada: borrar la valoración se lleva su plan.
-- Los campos de dieta (distribución calórica, menú) son JSON guardado como TEXT.

CREATE TABLE valoracion_nutricional (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id            INTEGER NOT NULL REFERENCES paciente(id),
    hospitalizacion_id     INTEGER REFERENCES hospitalizacion(id),
    nutricionista_id       INTEGER NOT NULL REFERENCES usuario(id),
    fecha_valoracion       TEXT NOT NULL,   -- RFC 3339 UTC
    peso                   REAL NOT NULL,
    talla                  REAL NOT NULL,
    imc                    REAL NOT NULL,
    circunferencia_brazo   REAL,
    pliegue_tricipital     REAL,
    screening_nutricional  TEXT NOT NULL DEFAULT 'NRS-2002',
    puntaje_screening      INTEGER,
    riesgo_nutricional     TEXT NOT NULL DEFAULT 'BAJO'
        CHECK (riesgo_nutricional IN ('BAJO','MODERADO','ALTO')),
    requerimiento_calorico REAL,
    requerimiento_proteico REAL,
    albumina               REAL,
    prealbumina            REAL,
    diagnostico_nutricional TEXT NOT NULL,
    observaciones          TEXT,
    institucion_id         INTEGER REFERENCES institucion(id)
);

CREATE TABLE plan_alimentacion (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    valoracion_id         INTEGER NOT NULL REFERENCES valoracion_nutricional(id) ON DELETE CASCADE,
    paciente_id           INTEGER NOT NULL REFERENCES paciente(id),
    nutricionista_id      INTEGER NOT NULL REFERENCES usuario(id),
    fecha_creacion        TEXT NOT NULL,
    tipo_dieta            TEXT NOT NULL,
    calorias_totales      REAL NOT NULL,
    distribucion_calorica TEXT NOT NULL DEFAULT '{}',   -- JSON
    menu_diario           TEXT NOT NULL DEFAULT '{}',   -- JSON
    restricciones         TEXT,
    suplementos           TEXT,
    via_alimentacion      TEXT NOT NULL DEFAULT 'ORAL'
        CHECK (via_alimentacion IN ('ORAL','ENTERAL','PARENTERAL','MIXTA')),
    activo                INTEGER NOT NULL DEFAULT 1,
    observaciones         TEXT,
    institucion_id        INTEGER REFERENCES institucion(id)
);

CREATE INDEX idx_valoracion_paciente ON valoracion_nutricional (paciente_id, fecha_valoracion);
CREATE INDEX idx_plan_valoracion ON plan_alimentacion (valoracion_id);
