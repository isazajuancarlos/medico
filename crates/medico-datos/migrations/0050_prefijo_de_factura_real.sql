-- El prefijo de factura empieza a APLICARSE, y por eso su defecto tiene que
-- decir la verdad (2026-08-08).
--
-- La 0048 creó `configuracion_institucion.prefijo_factura` con
-- `DEFAULT 'FE'`, y `facturacion::siguiente_numero` numeraba con `FAC-`
-- escrito a mano. O sea que esa columna NUNCA se aplicó: el administrador la
-- editaba desde el panel y el binario seguía numerando igual. Se midió el
-- 2026-08-08 —ninguna de las 17 columnas de esa tabla tenía lector— y ésta es
-- una de las dos que se enchufan.
--
-- # Por qué el defecto pasa a 'FAC' en vez de dejar 'FE'
--
-- Porque enchufarla con 'FE' rompería la SERIE de quien ya factura: todas sus
-- facturas dicen `FAC-2026-000NN`, el contador busca el último con ese prefijo,
-- y al cambiar a `FE-` empezaría una serie nueva desde el 1. Una serie de
-- numeración no es un detalle estético: es lo que la resolución de la DIAN
-- autoriza, y partirla sin que nadie lo pida es peor que el defecto que se está
-- arreglando.
--
-- 'FAC' es además lo ÚNICO que este producto ha escrito nunca en una factura,
-- así que poner ahí el defecto es hacer que la columna diga lo que ya ocurría.
--
-- # Y las filas que ya existen
--
-- Se pasan a 'FAC' **solo si siguen en 'FE'**, que es el valor que nadie pudo
-- elegir con efecto: hasta hoy escribirlo no hacía nada, así que no hay
-- intención que respetar y sí una serie que conservar. La que tenga cualquier
-- otro valor se deja intacta: ésa sí es una elección, y desde hoy se cumple.
--
-- Quien quiera 'FE' lo escribe en la pantalla y ahora sí numera con él.

UPDATE configuracion_institucion SET prefijo_factura = 'FAC' WHERE prefijo_factura = 'FE';

-- SQLite no deja cambiar un DEFAULT con ALTER; se rehace la tabla. Es la misma
-- receta que la 0023 con las camas: crear, copiar, borrar, renombrar.
CREATE TABLE configuracion_institucion_nueva (
    institucion_id INTEGER PRIMARY KEY REFERENCES institucion(id) ON DELETE CASCADE,

    horario_apertura TEXT NOT NULL DEFAULT '07:00',
    horario_cierre   TEXT NOT NULL DEFAULT '18:00',
    dias_atencion    TEXT NOT NULL DEFAULT '["lunes","martes","miercoles","jueves","viernes"]',

    duracion_cita_default INTEGER NOT NULL DEFAULT 20 CHECK (duracion_cita_default > 0),
    tiempo_buffer         INTEGER NOT NULL DEFAULT 0  CHECK (tiempo_buffer >= 0),

    enviar_recordatorios INTEGER NOT NULL DEFAULT 0,
    horas_anticipacion   INTEGER NOT NULL DEFAULT 24 CHECK (horas_anticipacion >= 0),

    smtp_host    TEXT,
    smtp_port    INTEGER CHECK (smtp_port IS NULL OR (smtp_port > 0 AND smtp_port <= 65535)),
    smtp_usuario TEXT,

    -- LO ÚNICO QUE CAMBIA: 'FE' -> 'FAC'.
    prefijo_factura        TEXT NOT NULL DEFAULT 'FAC',
    resolucion_facturacion TEXT,
    rango_desde            INTEGER CHECK (rango_desde IS NULL OR rango_desde >= 0),
    rango_hasta            INTEGER CHECK (rango_hasta IS NULL OR rango_hasta >= 0),

    permitir_citas_online     INTEGER NOT NULL DEFAULT 0,
    requiere_autorizacion     INTEGER NOT NULL DEFAULT 0,
    dias_maximos_agendamiento INTEGER NOT NULL DEFAULT 90 CHECK (dias_maximos_agendamiento > 0),

    CHECK (rango_desde IS NULL OR rango_hasta IS NULL OR rango_desde <= rango_hasta)
);

INSERT INTO configuracion_institucion_nueva SELECT * FROM configuracion_institucion;

DROP TABLE configuracion_institucion;
ALTER TABLE configuracion_institucion_nueva RENAME TO configuracion_institucion;
