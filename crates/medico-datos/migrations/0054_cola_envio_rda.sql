-- Cola de envío de los RDA al mecanismo IHCE (#603 fase 4b).
--
-- # Por qué una cola y no un POST síncrono
--
-- El Manual de Operaciones IHCE (§5.7) pide envío ASÍNCRONO: el RDA se transmite
-- a la plataforma nacional, que puede tardar o estar caída, y eso NO puede
-- bloquear la operación clínica —guardar una historia no espera a que MinSalud
-- conteste—. Así que el guardado ENCOLA una fila aquí, y una tarea de fondo la
-- drena con reintentos y backoff. Es el mismo patrón que el freno: la app trae
-- sus filas, la regla decide.
--
-- # Los estados, y por qué CUATRO
--
-- `pendiente` → esperando su turno de envío (o su reintento tras un fallo).
-- `aceptado`  → el mecanismo lo recibió (200), `rda_id` trae su identificador.
-- `duplicado` → 409: ya existía un RDA con el mismo Encounter. Es TERMINAL y NO
--               es una alarma: el dato está en la plataforma, no se reintenta.
--               Colapsarlo con `fallido` haría sonar una alerta por algo correcto.
-- `fallido`   → error permanente (400 de estructura) o se agotaron los reintentos.
--               `ultimo_error` dice por qué; esta fila necesita mirada humana.
--
-- `proximo_intento` (RFC3339) es cuándo puede volver a intentarse: la tarea de
-- fondo solo toma filas `pendiente` con `proximo_intento <= ahora`. Con cada
-- fallo transitorio se empuja hacia adelante con backoff exponencial, de modo
-- que un mecanismo caído no se martillea.
CREATE TABLE IF NOT EXISTS envio_rda (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id  INTEGER NOT NULL,
    -- El nombre CORTO de la operación (`OperacionRda::nombre`), no la ruta HTTP:
    -- la ruta es contrato de transporte y podría cambiar; esto es clave persistida.
    operacion       TEXT    NOT NULL,
    -- El Bundle FHIR ya ensamblado, tal como se enviará. Se guarda entero a
    -- propósito: reconstruirlo al reintentar podría producir uno distinto del que
    -- se validó, y entonces el reintento mediría otra cosa.
    bundle          TEXT    NOT NULL,
    estado          TEXT    NOT NULL DEFAULT 'pendiente'
                    CHECK (estado IN ('pendiente', 'aceptado', 'duplicado', 'fallido')),
    intentos        INTEGER NOT NULL DEFAULT 0,
    proximo_intento TEXT    NOT NULL,
    ultimo_error    TEXT,
    rda_id          TEXT,
    creado          TEXT    NOT NULL,
    actualizado     TEXT    NOT NULL
);

-- El índice que sirve a la consulta de la tarea de fondo: las pendientes cuyo
-- turno llegó, y en orden de llegada.
CREATE INDEX IF NOT EXISTS ix_envio_rda_por_drenar
    ON envio_rda (estado, proximo_intento);
