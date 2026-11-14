-- #758 — Borrado demostrable de la historia clínica por defunción (Ley 1581).
--
-- Al cumplirse el periodo legal de conservación contado DESDE LA FECHA DE
-- DEFUNCIÓN (20 años, Res. 1995/1999, más conservador que el mínimo de la
-- Res. 839/2017; y la defunción es ≥ la última atención, así que anclar en ella
-- nunca borra antes del mínimo legal), el sistema:
--   1. archiva el expediente completo del paciente en una copia COMPRIMIDA +
--      CIFRADA (guaca::reposo) bajo custodia, con un MANIFIESTO FIRMADO
--      (guaca::firma, la identidad de firma de medico) que ata el hash del
--      archivo a lo que contenía;
--   2. purga esos datos de la base viva (borrado explícito tabla por tabla);
--   3. deja este registro como CERTIFICADO DE SUPRESIÓN: prueba que el
--      expediente existió y se suprimió, SIN conservar el dato clínico.
--
-- El «demostrable» de la Ley 1581 vive aquí: el manifiesto firmado + el hash del
-- archivo permiten acreditar qué se borró y cuándo, sin retener la historia.
--
-- La fila `paciente` NO se borra (la referencian la cadena `acceso`, que se
-- conserva como prueba de tratamiento lícito, y los `recien_nacido` de hijos que
-- pueden seguir vivos); se ANONIMIZA su PII. El dato completo queda solo en el
-- archivo cifrado.

CREATE TABLE supresion (
    id                INTEGER PRIMARY KEY,
    -- El ancla referencial que sobrevive a la purga (fila `paciente` anonimizada).
    paciente_id       INTEGER NOT NULL REFERENCES paciente(id),
    institucion_id    INTEGER NOT NULL REFERENCES institucion(id),
    -- La fecha que dispara el reloj (RFC 3339, copiada del certificado de
    -- defunción vigente al momento de programar).
    fecha_defuncion   TEXT    NOT NULL,
    -- fecha_defuncion + periodo legal. Congelada al certificar la supresión.
    fecha_vencimiento TEXT    NOT NULL,
    -- El acto en sí.
    fecha_supresion   TEXT    NOT NULL,
    -- La copia bajo custodia: ruta del archivo cifrado y su hash SHA-256 (hex).
    -- El hash ata este certificado al blob: si el archivo se altera, no casa.
    ruta_archivo      TEXT    NOT NULL,
    sha256_archivo    TEXT    NOT NULL,
    -- El manifiesto canónico (JSON) y su firma híbrida (Ed25519+ML-DSA-87).
    -- El manifiesto enumera las tablas archivadas con su recuento: es lo que se
    -- verifica contra el contenido del archivo.
    manifiesto        TEXT    NOT NULL,
    firma             TEXT    NOT NULL,
    -- Recuento de filas purgadas por tabla (JSON), para acreditar el alcance.
    recuentos         TEXT    NOT NULL,
    creado_en         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

-- Un paciente se suprime UNA sola vez. El barrido salta a quien ya tenga fila
-- aquí: es la marca de «ya hecho» que hace idempotente la tarea de fondo.
CREATE UNIQUE INDEX ux_supresion_paciente ON supresion(paciente_id);

CREATE INDEX supresion_por_institucion ON supresion(institucion_id);
