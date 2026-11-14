-- Citas: la agenda mínima del prestador. Una cita cuelga de un paciente y (si
-- se asignó) de un médico, dentro de una institución. La agenda completa
-- (bloques, bloqueos, indicadores de oportunidad) es un subsistema aparte que se
-- portará después; esto es el registro de la cita en sí.

CREATE TABLE cita (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    paciente_id    INTEGER NOT NULL REFERENCES paciente(id),
    -- El médico puede faltar cuando se agenda sin asignar aún.
    medico_id      INTEGER REFERENCES usuario(id),
    fecha          TEXT    NOT NULL,   -- YYYY-MM-DD
    hora           TEXT    NOT NULL,   -- HH:MM
    estado         TEXT    NOT NULL DEFAULT 'PROGRAMADA'
                   CHECK (estado IN ('PROGRAMADA','ATENDIDA','CANCELADA','NO_ASISTIO')),
    motivo         TEXT
);

CREATE INDEX ix_cita_institucion_fecha ON cita (institucion_id, fecha);
CREATE INDEX ix_cita_paciente ON cita (paciente_id);
