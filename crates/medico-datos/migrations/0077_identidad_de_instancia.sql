-- 0077: identidad de instancia (fail-safe #2 de #758, Ley 1581).
--
-- Un identificador único POR BASE, sembrado al aplicar la migración con bytes
-- aleatorios de SQLite (`randomblob`), no en el código: cada base nace con el
-- suyo y lo conserva entre arranques; un respaldo restaurado trae el suyo (es la
-- MISMA instancia lógica), y una base DISTINTA nace con otro.
--
-- Sirve para SELLAR la carpeta de imágenes: la reconciliación de huérfanas
-- (`medico_api::retencion::reconciliar_imagenes_huerfanas`) escribe un marcador
-- `.instancia` = este uuid en `MEDICO_DIR_IMAGENES` y, en cada pasada, exige que
-- casen. El oráculo `sqlite_sequence` acota el RADIO de lo borrable (ids que esta
-- base pudo emitir) pero no prueba IDENTIDAD de base: una base distinta con `seq`
-- alto apuntada a una carpeta de imágenes ajena (`DATABASE_URL` + `MEDICO_DIR_IMAGENES`
-- cruzados) borraría ids ajenos `≤ su seq`. El sello lo impide: si no casa, esta
-- base no es la dueña de esas imágenes y NO se reconcilia.
--
-- Una sola fila, inmutable (`CHECK (id = 1)`). No es dato de paciente: nunca se
-- purga (no tiene FK a `paciente`, así que queda fuera de `EXPEDIENTE`).
CREATE TABLE instancia (
    id   INTEGER PRIMARY KEY CHECK (id = 1),
    uuid TEXT    NOT NULL
);

INSERT INTO instancia (id, uuid) VALUES (1, lower(hex(randomblob(16))));
