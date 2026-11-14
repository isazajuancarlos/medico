-- Generalizar la BITÁCORA del aval del `nilknown` a CUALQUIER encuentro (#603,
-- piezas 2 y 3: RDA de HOSPITALIZACIÓN y URGENCIAS). Hasta ahora la tabla se
-- llaveaba por `historia_id` con FK a `historia`, porque el único productor era
-- el de CONSULTA EXTERNA, que cuelga de una historia. Hospitalización
-- (`hospitalizacion`) y urgencias (`atencion_urgencias`) son tablas de encuentro
-- PROPIAS —no son `historia`—, así que su RDA no podía avalar sus secciones
-- vacías sin esta generalización.
--
-- La llave pasa a ser POLIMÓRFICA: (tipo_encuentro, encuentro_id, seccion_codigo).
-- `tipo_encuentro` es el nombre corto de la operación RDA, EXACTAMENTE como lo
-- fija `medico_ihce::OperacionRda::nombre()` y como lo repiten las constantes
-- `OPERACION_*` de `rda_encuentro` ('consulta-externa'|'hospitalizacion'|
-- 'urgencias'); `encuentro_id` es el id de la fila de ESE encuentro (historia.id,
-- hospitalizacion.id, atencion_urgencias.id).
--
-- El INVARIANTE del `nilknown` (regla de Juan, 2026-08-23) NO cambia: el aval
-- sigue siendo un acto humano ATRIBUIDO (usuario_id + sello), y sin fila una
-- sección vacía es FALTA y el RDA no se emite. Solo se extiende a tres tipos de
-- encuentro; un aval de un tipo NO vale para otro, porque entran en la llave.
--
-- SE PIERDE la FK `ON DELETE CASCADE` a `historia`: no hay FK polimórfica en
-- SQLite. No es una regresión real —medico no borra historias (registro clínico
-- append-only)— y la existencia + institución del encuentro la valida la guarda
-- de ámbito de `rda_aval` antes de escribir. Se deja dicho aquí para que, el día
-- que exista un borrado de encuentro, barra también su aval.
--
-- Reconstrucción SEGURA (mismo patrón que 0073): tabla nueva, copiar las filas
-- vigentes marcándolas 'consulta-externa' (el único productor hasta hoy), soltar
-- la vieja, renombrar. Las filas existentes conservan su atribución y su sello.
CREATE TABLE aval_seccion_rda_nueva (
    tipo_encuentro TEXT    NOT NULL,
    encuentro_id   INTEGER NOT NULL,
    seccion_codigo TEXT    NOT NULL,
    usuario_id     INTEGER NOT NULL REFERENCES usuario(id),
    sello          TEXT    NOT NULL,
    PRIMARY KEY (tipo_encuentro, encuentro_id, seccion_codigo)
);

INSERT INTO aval_seccion_rda_nueva (tipo_encuentro, encuentro_id, seccion_codigo, usuario_id, sello)
    SELECT 'consulta-externa', historia_id, seccion_codigo, usuario_id, sello
    FROM aval_seccion_rda;

DROP TABLE aval_seccion_rda;
ALTER TABLE aval_seccion_rda_nueva RENAME TO aval_seccion_rda;
