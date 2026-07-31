-- Urgencias: cola de triage y atención (#126, port a Rust de backend/routers/urgencias.py).
--
-- `triage` se guarda como la CLAVE (TRIAGE_1..TRIAGE_5), no como la etiqueta, a
-- propósito: ordenar por la clave ASC da la prioridad correcta (1 = Rojo primero).
-- El original ordenaba por el valor del enum ("Rojo…", "Naranja…"), que alfabético
-- pone Amarillo antes que Rojo — un fallo de orden en una cola de urgencias.
--
-- `destino_salida` NULL = el paciente sigue en la cola (aún no egresa). No se
-- inventa un diagnóstico de ingreso de relleno (el original ponía "Z00.0"): queda
-- NULL hasta que se registre uno real.
CREATE TABLE atencion_urgencias (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id           INTEGER NOT NULL REFERENCES paciente(id),
    triage                TEXT    NOT NULL
        CHECK (triage IN ('TRIAGE_1','TRIAGE_2','TRIAGE_3','TRIAGE_4','TRIAGE_5')),
    motivo_consulta       TEXT    NOT NULL,
    signos_vitales        TEXT,          -- JSON, opcional
    enfermera_triage_id   INTEGER NOT NULL REFERENCES usuario(id),
    fecha_ingreso         TEXT    NOT NULL,   -- RFC 3339 UTC
    diagnostico_ingreso   TEXT,          -- sin relleno: NULL hasta que haya uno real
    destino_salida        TEXT
        CHECK (destino_salida IN ('DOMICILIO','HOSPITALIZACION','REMISION','OBSERVACION','FUGA','FALLECIDO')),
    observaciones_egreso  TEXT,
    fecha_egreso          TEXT
);

-- La cola consulta por «aún sin egreso» y ordena por prioridad: un índice sobre
-- las filas abiertas mantiene la consulta barata según crece el histórico.
CREATE INDEX idx_urgencias_cola ON atencion_urgencias (destino_salida, triage, fecha_ingreso);
