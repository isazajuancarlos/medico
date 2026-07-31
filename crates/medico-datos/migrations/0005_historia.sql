-- Historia clínica, con el modelo COMPLETO del sistema (Historia + CamposRips):
-- anamnesis, signos vitales, examen físico por sistemas, RIPS, diagnósticos,
-- incapacidad y plan. NO se recorta ni un campo: un formulario que manda 50 y
-- una tabla que guarda 15 borraría datos clínicos al editar.
--
-- No lleva institucion_id a propósito: la historia CUELGA del paciente, y el
-- aislamiento se sigue por esa cadena (como en el sistema original). Quien no ve
-- al paciente no ve su historia.

CREATE TABLE historia (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    fecha              TEXT    NOT NULL DEFAULT (datetime('now')),
    codigo_historia    TEXT    NOT NULL,
    paciente_id        INTEGER NOT NULL REFERENCES paciente(id),
    medico_id          INTEGER NOT NULL REFERENCES usuario(id),
    cita_id            INTEGER REFERENCES cita(id),

    -- Acompañante
    acomp_nombre       TEXT,
    acomp_parentesco   TEXT,
    acomp_telefono     TEXT,
    acomp_documento    TEXT,

    -- Anamnesis
    motivo_consulta    TEXT    NOT NULL,
    enfermedad_actual  TEXT    NOT NULL,

    -- Signos vitales
    tension_arterial   TEXT,
    pam                TEXT,
    frecuencia_cardiaca      INTEGER,
    frecuencia_respiratoria  INTEGER,
    pulso              INTEGER,
    temperatura        REAL,
    pabd               REAL,
    pc                 REAL,
    peso               REAL,
    talla              REAL,
    imc                REAL,
    peso_ideal         REAL,
    estado_nutricional TEXT,

    -- Examen físico por sistemas
    ef_cabeza          TEXT,
    ef_ojos            TEXT,
    ef_nariz           TEXT,
    ef_oidos           TEXT,
    ef_boca            TEXT,
    ef_faringe         TEXT,
    ef_cuello          TEXT,
    ef_pulmones        TEXT,
    ef_corazon         TEXT,
    ef_abdomen         TEXT,
    ef_gu              TEXT,
    ef_neurologico     TEXT,
    ef_extremidad      TEXT,
    ef_osteomuscular   TEXT,
    ef_piel            TEXT,
    ef_hallazgos       TEXT,
    ef_orl             TEXT,
    ef_cardiopulmonar  TEXT,

    -- Finalidad / causa / tipo de diagnóstico (RIPS). Defectos = los valores del
    -- sistema original (NO_APLICA=10, ENFERMEDAD_GENERAL=13, IMPRESION_DX=01).
    finalidad          TEXT    NOT NULL DEFAULT '10',
    causa_externa      TEXT    NOT NULL DEFAULT '13',
    tipo_diagnostico   TEXT    NOT NULL DEFAULT '01',
    cod_consulta_cups  TEXT,

    -- CamposRips: lo que el RIPS exige de cualquier servicio.
    modalidad_grupo_servicio TEXT,
    grupo_servicios    TEXT,
    cod_servicio       INTEGER,
    via_ingreso_servicio     TEXT,
    causa_motivo       TEXT,
    valor_servicio     INTEGER,
    concepto_recaudo   TEXT,
    valor_pago_moderador     INTEGER NOT NULL DEFAULT 0,
    num_fev_pago_moderador   TEXT,

    -- Campos dinámicos configurables (JSON).
    datos_dinamicos    TEXT    NOT NULL DEFAULT '{}',

    -- Diagnósticos
    diagnostico_principal       TEXT NOT NULL,
    diagnostico_relacionado1    TEXT,
    diagnostico_relacionado2    TEXT,
    diagnostico_relacionado3    TEXT,
    impresion_diagnostica       TEXT,
    diagnostico_principal_cie11 TEXT,
    diagnostico_principal_uri   TEXT,

    -- Procedencia de cada dato (JSON): confirmado/derivado/propuesto/no_aplica.
    procedencias       TEXT,

    -- Incapacidad
    genera_incapacidad INTEGER NOT NULL DEFAULT 0,
    dias_incapacidad   INTEGER NOT NULL DEFAULT 0,

    -- Plan
    plan_tratamiento   TEXT    NOT NULL
);

CREATE INDEX ix_historia_paciente ON historia (paciente_id, fecha);
