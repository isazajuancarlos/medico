-- Rehabilitación (#65/#126, port a Rust de backend/routers/rehabilitacion.py).
-- Dos entidades: el plan de rehabilitación y sus sesiones de terapia. Crear una
-- sesión incrementa sesiones_realizadas del plan (en transacción). Mismo patrón
-- que nutrición: institucion_id propio por fila, sesión cuelga del plan con
-- ON DELETE CASCADE, y la institución se sella desde la del paciente al crear.

CREATE TABLE plan_rehabilitacion (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    terapeuta_id        INTEGER NOT NULL REFERENCES usuario(id),
    fecha_creacion      TEXT NOT NULL,   -- RFC 3339 UTC
    diagnostico_funcional TEXT NOT NULL,
    objetivos           TEXT NOT NULL,
    sesiones_autorizadas INTEGER NOT NULL DEFAULT 10,
    sesiones_realizadas  INTEGER NOT NULL DEFAULT 0,
    tipo_terapia        TEXT NOT NULL,   -- FISICA, OCUPACIONAL, LENGUAJE, RESPIRATORIA
    frecuencia          TEXT NOT NULL,
    duracion_sesion_min INTEGER NOT NULL DEFAULT 45,
    activo              INTEGER NOT NULL DEFAULT 1,
    fecha_cierre        TEXT,
    resultado_final     TEXT,
    institucion_id      INTEGER REFERENCES institucion(id)
);

CREATE TABLE sesion_terapia (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id             INTEGER NOT NULL REFERENCES plan_rehabilitacion(id) ON DELETE CASCADE,
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    terapeuta_id        INTEGER NOT NULL REFERENCES usuario(id),
    fecha_sesion        TEXT NOT NULL,
    numero_sesion       INTEGER NOT NULL DEFAULT 1,   -- nº secuencial dentro del plan
    evaluacion_funcional TEXT NOT NULL,
    dolor_eva           INTEGER,         -- 0-10
    rango_movimiento    TEXT,
    tecnicas_aplicadas  TEXT NOT NULL,
    ejercicios_domicilio TEXT,
    progreso            TEXT             -- MEJORIA, ESTABLE, DETERIORO
        CHECK (progreso IS NULL OR progreso IN ('MEJORIA','ESTABLE','DETERIORO')),
    observaciones       TEXT,
    institucion_id      INTEGER REFERENCES institucion(id)
);

CREATE INDEX idx_plan_rehab_paciente ON plan_rehabilitacion (paciente_id, fecha_creacion);
CREATE INDEX idx_sesion_plan ON sesion_terapia (plan_id, fecha_sesion);
