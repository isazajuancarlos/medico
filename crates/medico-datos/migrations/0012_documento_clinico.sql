-- Documento clínico: la espina de casi todas las notas que cuelgan del paciente
-- (evolución, epicrisis, interconsulta, nota operatoria, consentimiento,
-- incapacidad, contrarreferencia, nota de enfermería, valoraciones…).
--
-- En el sistema VFP original cada una era una tabla y una pantalla distintas,
-- pero todas comparten la MISMA forma: cuelgan de un paciente, tienen autor,
-- fecha y firma, y se aíslan por la cadena paciente→institución. Lo común vive
-- aquí una sola vez; lo específico de cada tipo va en `contenido` (JSON abierto),
-- igual que los antecedentes del paciente y `datos_dinamicos` de la historia.
--
-- `tipo` discrimina (EVOLUCION, EPICRISIS, INTERCONSULTA, …). `historia_id` es
-- opcional: una nota puede colgar de una atención concreta o solo del paciente.
-- `firmada` la cierra: un documento firmado es inmutable (Res. 1995/1999), la
-- misma regla que la historia.

CREATE TABLE documento_clinico (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    tipo         TEXT    NOT NULL,
    paciente_id  INTEGER NOT NULL REFERENCES paciente(id) ON DELETE CASCADE,
    historia_id  INTEGER REFERENCES historia(id) ON DELETE SET NULL,
    medico_id    INTEGER NOT NULL REFERENCES usuario(id),
    fecha        TEXT    NOT NULL DEFAULT (datetime('now')),
    titulo       TEXT    NOT NULL DEFAULT '',
    contenido    TEXT    NOT NULL DEFAULT '{}',
    firmada      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX ix_documento_paciente ON documento_clinico (paciente_id, tipo, fecha);
