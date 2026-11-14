-- La BITÁCORA del aval del `nilknown` (#603 F4.5, requisito #2). Una sección
-- vacía de un RDA solo emite `emptyReason=nilknown` —«revisado, no hay nada
-- conocido»— si un CLÍNICO lo confirmó (la puerta clínica de #603, decidida por
-- Juan el 2026-08-23). Hasta ahora ese aval era un `&[&str]` de códigos que no
-- dejaba rastro: el artefacto emitido no registraba quién lo afirmó ni cuándo.
--
-- Esta tabla es ese rastro, y es la mitad DATOS del newtype infalsificable del
-- dominio (`ihce::Aval`/`Confirmacion`): una fila por (historia, sección
-- avalada), con el usuario que la confirmó y el instante. El productor del RDA la
-- LEE para construir el aval; sin filas, toda sección vacía es una FALTA y el RDA
-- no se emite (directiva 20). La pantalla de captura (F4.5, pasada aparte) la
-- ESCRIBE, y la ruta registra además el acto en la cadena `acceso` (encadenada
-- por hash y firmada, Ley 1581) — no una segunda cadena, la que ya existe.
--
-- PK (historia_id, seccion_codigo): un solo aval VIGENTE por sección. Reavalar
-- —otro clínico, otra revisión— reemplaza la fila y su atribución; el histórico
-- del acto vive en la cadena `acceso`, que es append-only.
--
-- `sello` es ISO-8601 CON huso, en la zona del prestador (`MEDICO_ZONA`), como el
-- resto de instantes clínicos de medico: el momento de la confirmación se guarda
-- tal como ocurrió, no en el reloj de la máquina.
CREATE TABLE aval_seccion_rda (
    historia_id    INTEGER NOT NULL REFERENCES historia(id) ON DELETE CASCADE,
    seccion_codigo TEXT    NOT NULL,
    usuario_id     INTEGER NOT NULL REFERENCES usuario(id),
    sello          TEXT    NOT NULL,
    PRIMARY KEY (historia_id, seccion_codigo)
);
