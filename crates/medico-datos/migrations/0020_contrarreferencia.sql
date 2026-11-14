-- Contrarreferencia (#65/#126, port a Rust de backend/routers/contrarreferencia.py).
-- La REMISIÓN no es tabla nueva: es una `orden` de tipo ESPECIALISTA con su
-- máquina de estados (PENDIENTE→EN_PROCESO→COMPLETADA/CANCELADA). La
-- contrarreferencia es la respuesta desde la institución de destino; al crearla
-- se CIERRA la remisión (orden→COMPLETADA) si seguía abierta.
--
-- El acuse de recibo (recibida) exige responsable (recibida_por_id): un acuse sin
-- quién no sirve para demostrar la continuidad si el paciente reclama.
CREATE TABLE contrarreferencia (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    orden_id                  INTEGER REFERENCES orden(id),
    paciente_id               INTEGER NOT NULL REFERENCES paciente(id),
    institucion_origen_id     INTEGER REFERENCES institucion(id),
    medico_contrarrefiere_id  INTEGER REFERENCES usuario(id),
    fecha_creacion            TEXT NOT NULL,   -- RFC 3339 UTC
    diagnostico_final         TEXT NOT NULL,
    procedimientos_realizados TEXT,
    tratamiento_instaurado    TEXT,
    recomendaciones           TEXT,
    requiere_seguimiento      INTEGER NOT NULL DEFAULT 0,
    fecha_control_sugerida    TEXT,            -- fecha (YYYY-MM-DD)
    recibida                  INTEGER NOT NULL DEFAULT 0,
    fecha_recepcion           TEXT,
    recibida_por_id           INTEGER REFERENCES usuario(id)
);

CREATE INDEX idx_contrarref_paciente ON contrarreferencia (paciente_id, fecha_creacion);
