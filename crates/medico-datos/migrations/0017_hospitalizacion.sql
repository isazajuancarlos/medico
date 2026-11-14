-- Hospitalización (#65/#126, port a Rust de backend/routers/hospitalizacion.py).
-- Dos tablas: camas y hospitalizaciones. Ingresar toma una cama DISPONIBLE y la
-- marca OCUPADA; egresar libera la cama. Ambas transiciones van en UNA
-- transacción, para que no quede una cama ocupada sin hospitalización ni al revés.
--
-- Las camas son GLOBALES, como en el original (una cama no es dato personal; su
-- alcance por institución queda como mejora anotada). Las hospitalizaciones SÍ se
-- filtran por la institución del paciente, igual que el resto.
--
-- diagnostico_ingreso NULL, sin el "Z00.0" de relleno del original (directiva 20).

CREATE TABLE cama (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    numero         TEXT NOT NULL UNIQUE,   -- "101-A"
    piso           TEXT NOT NULL,
    servicio       TEXT NOT NULL,          -- UCI, GENERAL, PEDIATRIA, MATERNIDAD…
    estado         TEXT NOT NULL DEFAULT 'DISPONIBLE'
        CHECK (estado IN ('DISPONIBLE','OCUPADA','EN_ASEO','BLOQUEADA','MANTENIMIENTO')),
    tiene_oxigeno  INTEGER NOT NULL DEFAULT 0,
    tiene_monitor  INTEGER NOT NULL DEFAULT 0,
    es_aislamiento INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE hospitalizacion (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id          INTEGER NOT NULL REFERENCES paciente(id),
    cama_id              INTEGER NOT NULL REFERENCES cama(id),
    medico_tratante_id   INTEGER NOT NULL REFERENCES usuario(id),
    fecha_ingreso        TEXT NOT NULL,   -- RFC 3339 UTC
    fecha_egreso         TEXT,
    via_ingreso          TEXT NOT NULL DEFAULT 'PROGRAMADA'
        CHECK (via_ingreso IN ('URGENCIAS','PROGRAMADA','REMISION')),
    estado               TEXT NOT NULL DEFAULT 'ACTIVA'
        CHECK (estado IN ('ACTIVA','ALTA_MEDICA','ALTA_VOLUNTARIA','REMISION','DEFUNCION')),
    diagnostico_ingreso  TEXT,
    motivo_ingreso       TEXT,
    observaciones_ingreso TEXT,
    diagnostico_egreso   TEXT,
    resumen_clinico      TEXT
);

CREATE INDEX idx_hosp_estado ON hospitalizacion (estado, fecha_ingreso);
