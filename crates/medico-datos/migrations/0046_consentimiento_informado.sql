-- Consentimiento informado. Port de `backend/models.py:ConsentimientoInformado`.
--
-- =========================================================================
-- POR QUÉ NO VA EN `documento_clinico`, QUE ES DONDE PARECÍA QUE IBA
-- =========================================================================
--
-- La espina de la 0012 lista el consentimiento entre los tipos que aloja, y
-- reutilizarla habría sido lo simple y profundo (directiva 1): mismo aislamiento
-- por paciente, misma firma, mismo rastro. No se hace, y el motivo es una
-- incompatibilidad, no una preferencia.
--
-- `documento_clinico.firmada` CIERRA el documento: `documento.rs` rechaza toda
-- escritura sobre uno firmado (Res. 1995/1999, la misma inmutabilidad de la
-- historia). Un consentimiento, en cambio, tiene una escritura que NO se puede
-- bloquear jamás: la revocación. El paciente retira su consentimiento cuando
-- quiera y sin motivo, y un sistema que se lo impida —porque «ya está firmado»,
-- porque «ya se programó»— se convierte en el instrumento de una intervención
-- no consentida.
--
-- O sea que meterlo en la espina obligaba a una de dos: romper la inmutabilidad
-- para los ocho tipos, o romper el derecho a revocar para éste. Tabla propia.
--
-- =========================================================================
-- LO QUE NO LLEVA, Y ES DELIBERADO
-- =========================================================================
--
-- **No lleva `institucion_id`.** Es un modelo clínico y cuelga del paciente,
-- como `historia`, `documento_clinico` y los demás; el aislamiento sigue la
-- cadena consentimiento -> paciente -> institución. Añadirle columna propia
-- crearía la segunda fuente de verdad que ya costó una fuga en facturación,
-- al revés: allí el dueño FALTABA, aquí sobraría y podría discrepar del
-- paciente.
--
-- **Ningún campo del juicio tiene DEFAULT que rellene.** Los booleanos nacen en
-- 0 —que es «no consta», y es lo que hace que el consentimiento salga INVÁLIDO
-- hasta que alguien afirme lo contrario—; los cuatro textos de la explicación
-- son NOT NULL sin defecto, así que hay que escribirlos. La versión anterior de
-- este módulo (NiceGUI) guardaba el registro y lo daba por bueno: la regla de
-- que firmar no es consentir vive en `medico_dominio::consentimiento`, y esta
-- tabla existe para darle campos que puedan estar vacíos de verdad.
--
-- `historia_id` es opcional: un consentimiento puede firmarse antes de que
-- exista la atención en que se ejecuta el procedimiento.

CREATE TABLE consentimiento_informado (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id                 INTEGER NOT NULL REFERENCES paciente(id) ON DELETE CASCADE,
    historia_id                 INTEGER REFERENCES historia(id) ON DELETE SET NULL,
    medico_id                   INTEGER NOT NULL REFERENCES usuario(id),

    fecha_firma                 TEXT    NOT NULL DEFAULT (datetime('now')),

    -- El procedimiento
    tipo_procedimiento          TEXT    NOT NULL,
    codigo_cups                 TEXT,

    -- Lo que la norma exige haber explicado (Ley 23/1981 art. 15)
    descripcion_procedimiento   TEXT    NOT NULL,
    riesgos_explicados          TEXT    NOT NULL,
    beneficios_esperados        TEXT    NOT NULL,
    alternativas_explicadas     TEXT    NOT NULL,
    consecuencias_no_realizarlo TEXT,

    -- Las dos afirmaciones que separan un consentimiento de una firma
    paciente_comprende          INTEGER NOT NULL DEFAULT 0,
    acepta_procedimiento        INTEGER NOT NULL DEFAULT 0,

    firma_paciente_path         TEXT,
    firma_medico_path           TEXT,

    -- Testigo: hace falta cuando el paciente no puede firmar por sí mismo. No se
    -- deduce de la edad ni de la capacidad — eso lo decide quien atiende.
    testigo_nombre              TEXT,
    testigo_documento           TEXT,
    testigo_firma_path          TEXT,

    -- Revocación. No se borra la fila: la institución necesita poder probar que
    -- en su momento SÍ hubo consentimiento, que es justo lo que le discutirán.
    revocado                    INTEGER NOT NULL DEFAULT 0,
    fecha_revocacion            TEXT,
    motivo_revocacion           TEXT,

    CHECK (paciente_comprende   IN (0, 1)),
    CHECK (acepta_procedimiento IN (0, 1)),
    CHECK (revocado             IN (0, 1)),
    -- Un revocado sin fecha ni motivo sería una revocación sin constancia, que
    -- es lo único que la norma sí exige del acto de revocar.
    CHECK (revocado = 0 OR (fecha_revocacion IS NOT NULL AND motivo_revocacion IS NOT NULL))
);

CREATE INDEX ix_consentimiento_paciente ON consentimiento_informado (paciente_id, fecha_firma);
CREATE INDEX ix_consentimiento_historia ON consentimiento_informado (historia_id);
