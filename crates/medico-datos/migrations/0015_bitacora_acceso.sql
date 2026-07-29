-- Bitácora de acceso a dato personal (Ley 1581 / OWASP A09). Append-only,
-- encadenada por hash y firmada (guaca::auditoria): borrar o alterar una entrada
-- rompe la cadena. `secuencia` es la posición en la cadena (0 = génesis). El
-- `contenido` es el JSON canónico que se hashea y firma; las columnas hash/firma
-- son la envoltura criptográfica. `firma` vacía = evento encadenado pero sin
-- firmar (sin MEDICO_SIGNING_KEY): la integridad la da el hash, el no repudio la
-- firma.
CREATE TABLE acceso (
    secuencia     INTEGER PRIMARY KEY,
    hash_anterior TEXT NOT NULL,
    contenido     TEXT NOT NULL,
    hash          TEXT NOT NULL,
    firma         TEXT NOT NULL,
    creado_utc    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

-- Para consultar el rastro de un paciente sin escanear toda la tabla. El
-- paciente_id va DENTRO del contenido firmado; esta columna generada es solo un
-- índice de conveniencia, no una fuente de verdad.
CREATE INDEX idx_acceso_paciente ON acceso (json_extract(contenido, '$.paciente_id'));
