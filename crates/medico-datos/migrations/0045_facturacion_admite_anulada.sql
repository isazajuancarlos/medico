-- `factura.estado` admite ANULADA, que el producto ya usa y el CHECK no dejaba.
--
-- =========================================================================
-- POR QUÉ ES UNA MIGRACIÓN NUEVA Y NO UNA CORRECCIÓN DE LA 0044
-- =========================================================================
--
-- La 0044 es de ayer y no ha salido de esta rama, así que editarla en el sitio
-- sería más limpio de leer. No se hace: `sqlx::migrate!` guarda el HASH de cada
-- migración aplicada, y cualquiera que haya arrancado el binario sobre esta
-- rama —una prueba, una demo, un `cargo run`— tiene ya la 0044 registrada. Al
-- editarla, su siguiente arranque no encontraría un error de facturación: se
-- negaría a abrir la base entera. La regla del proyecto es que son inmutables, y
-- lo es precisamente para el caso en que uno cree que su edición es inofensiva.
--
-- El coste real es cero: la tabla se creó ayer, no la escribe todavía ninguna
-- ruta, y por tanto el copiado de abajo mueve cero filas en toda base existente.
--
-- =========================================================================
-- QUÉ FALTABA, Y CÓMO SE VIO
-- =========================================================================
--
-- El CHECK de la 0044 admite BORRADOR, RADICADA, ACEPTADA, GLOSADA y PAGADA. La
-- máquina de estados del Python (`routers/facturacion.py:_TRANSICIONES`) tiene
-- SEIS, y la sexta es ANULADA:
--
--     BORRADOR -> RADICADA
--     RADICADA -> ACEPTADA, GLOSADA, ANULADA
--     ACEPTADA -> PAGADA, ANULADA
--     GLOSADA  -> RADICADA, ANULADA
--     PAGADA   -> (ninguna)
--     ANULADA  -> (ninguna)
--
-- Y no es un estado de papel que solo exista en el diccionario: la pantalla lo
-- USA. `frontend/.../facturacion.component.ts` pinta el botón «Anular» siempre
-- que el estado no sea PAGADA ni ANULADA (l.1000-1002), ofrece «Anulada» en el
-- filtro (l.1213) y le da su propio color de estado (l.1520).
--
-- O sea que sin esta migración el port habría quedado así: el botón existe, el
-- usuario lo pulsa, y SQLite rechaza el UPDATE por violación de CHECK. Un 500
-- en la única acción que un administrativo necesita cuando una factura salió
-- mal. No lo habría cazado ninguna prueba del backend, porque la prueba habría
-- usado los cinco estados que el esquema declara.
--
-- Se anula y no se borra, y eso es contabilidad, no capricho: una factura
-- radicada ante una EPS ya existe fuera de este sistema. Por eso ANULADA no
-- sale hacia ningún lado —es terminal, como PAGADA— y por eso el borrado real
-- sigue estando permitido solo en BORRADOR, que es lo que nunca salió.
--
-- SQLite no sabe alterar un CHECK: hay que reconstruir la tabla. Se sigue el
-- procedimiento de 12 pasos de la documentación oficial, con las claves foráneas
-- APAGADAS durante el intercambio — si no, el `DROP` de la tabla vieja se
-- llevaría por delante a `detalle_factura` y `glosa`, que la referencian con
-- ON DELETE CASCADE.

PRAGMA foreign_keys = OFF;

CREATE TABLE factura_nueva (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id   INTEGER NOT NULL REFERENCES institucion(id),
    numero_factura   TEXT NOT NULL,
    entidad_id       INTEGER NOT NULL REFERENCES entidad(id),
    contrato_id      INTEGER REFERENCES contrato(id),
    fecha_factura    TEXT NOT NULL,
    fecha_radicado   TEXT,
    num_radicado     TEXT,
    periodo_inicio   TEXT NOT NULL,
    periodo_fin      TEXT NOT NULL,
    valor_bruto      REAL NOT NULL DEFAULT 0,
    descuentos       REAL NOT NULL DEFAULT 0,
    valor_neto       REAL NOT NULL DEFAULT 0,
    estado           TEXT NOT NULL DEFAULT 'BORRADOR'
        CHECK (estado IN ('BORRADOR','RADICADA','ACEPTADA','GLOSADA','PAGADA','ANULADA')),
    observaciones    TEXT,
    glosada          INTEGER NOT NULL DEFAULT 0,
    UNIQUE (institucion_id, numero_factura)
);

INSERT INTO factura_nueva
SELECT id, institucion_id, numero_factura, entidad_id, contrato_id, fecha_factura,
       fecha_radicado, num_radicado, periodo_inicio, periodo_fin, valor_bruto,
       descuentos, valor_neto, estado, observaciones, glosada
FROM factura;

DROP TABLE factura;
ALTER TABLE factura_nueva RENAME TO factura;

CREATE INDEX ix_factura_institucion ON factura(institucion_id);
CREATE INDEX ix_factura_entidad     ON factura(entidad_id);

PRAGMA foreign_keys = ON;
