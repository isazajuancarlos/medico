-- El registro de lo que se envió al Mecanismo Único de Validación.
--
-- # Por qué existe esta tabla
--
-- Desde el 2026-08-10 `/panel/rips` sube el FEV-RIPS y recibe el CUV, y hasta
-- ahora lo ENSEÑABA y se perdía al recargar. El CUV es el comprobante de que un
-- periodo se radicó: es lo que el prestador cita al responder una glosa y lo que
-- prueba, ante la EPS y la Supersalud, que reportó. Un comprobante que solo vive
-- en una pantalla no es un comprobante.
--
-- # Se guarda CADA envío, no el último
--
-- Un periodo se reenvía: se manda, el validador señala doce campos, se corrigen y
-- se vuelve a mandar. Guardar solo el último borraría la historia de esas
-- correcciones, que es justo lo que hay que poder enseñar si alguien pregunta por
-- qué se radicó tarde. Son filas, no un UPDATE.
--
-- # Solo entra lo que SALIÓ
--
-- Si el contenedor no responde, no hay fila: el archivo no llegó a MinSalud, y
-- una tabla que registra lo que no ocurrió miente igual que una bitácora que lo
-- hace. Es la misma regla que ya sigue el eslabón de la Ley 1581.
--
-- # El invariante lo sostiene la BASE, no el código
--
-- `cuv` no nulo si y solo si `aceptado`. Un CUV en una fila rechazada sería un
-- comprobante de algo que no pasó, y un aceptado sin CUV no sirve para nada; las
-- dos formas se leerían como datos válidos. Va como CHECK porque esta tabla la
-- escribirá algún día algo que no es el manejador de hoy (directiva 6).
CREATE TABLE IF NOT EXISTS envio_muv (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id   INTEGER NOT NULL,
    -- Quién pulsó. Un envío tiene efectos ante la EPS: no es anónimo.
    usuario_id       INTEGER NOT NULL,
    periodo_inicio   TEXT    NOT NULL,
    periodo_fin      TEXT    NOT NULL,
    -- Consecutivo, y no se puede repetir entre envíos ACEPTADOS: ver
    -- `nota_ya_aceptada` en `envio_muv.rs`.
    num_nota         TEXT    NOT NULL,
    destino          TEXT    NOT NULL CHECK (destino IN ('sin_factura', 'fev')),
    aceptado         INTEGER NOT NULL CHECK (aceptado IN (0, 1)),
    cuv              TEXT,
    proceso_id       INTEGER,
    fecha_radicacion TEXT,
    -- «DockerStage» o el que sea. Se guarda porque un CUV de stage NO vale, y
    -- sin este campo los dos se ven idénticos en la tabla.
    ambiente         TEXT,
    -- Los hallazgos tal como los devolvió el validador, en JSON. Es lo que
    -- permite volver a mirar QUÉ se rechazó sin reenviar nada.
    hallazgos        TEXT    NOT NULL DEFAULT '[]',
    -- Cuántos registros llevaba el archivo. Para cuadrar contra lo reportado.
    registros        INTEGER NOT NULL,
    -- RFC3339 en UTC, como el resto del sistema. Se muestra en la zona del
    -- prestador (`medico_dominio::zona`), que no es la del reloj de la máquina.
    enviado          TEXT    NOT NULL,

    CHECK ((aceptado = 1 AND cuv IS NOT NULL) OR (aceptado = 0 AND cuv IS NULL))
);

-- La consulta de la pantalla: los envíos de un periodo, de esta institución.
CREATE INDEX IF NOT EXISTS ix_envio_muv_periodo
    ON envio_muv (institucion_id, periodo_inicio, periodo_fin);

-- La puerta del número de nota: «¿este consecutivo ya obtuvo un CUV?».
CREATE INDEX IF NOT EXISTS ix_envio_muv_nota
    ON envio_muv (institucion_id, num_nota);
