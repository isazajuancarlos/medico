-- Proveedores, y el eslabón que le faltaba a la entrada de stock.
--
-- Sin esto, una entrada dice cuánto entró y cuándo, pero no DE QUIÉN. Es la
-- mitad que convierte el inventario en trazabilidad: ante un lote defectuoso hay
-- que poder ir hacia atrás —a quién se le compró, con qué factura— y hacia
-- delante —a qué pacientes se les entregó—. Lo segundo ya lo daba `dispensacion`;
-- lo primero es esto.
--
-- # El proveedor va en el MOVIMIENTO, no en el item
--
-- La tentación es ponerlo en `item_inventario`, que es una columna menos. Sería
-- falso: el mismo medicamento se compra a un distribuidor este mes y a otro el
-- siguiente, y la columna en el item solo podría guardar uno de los dos —el
-- último—, reescribiendo la procedencia de todas las remesas anteriores cada vez
-- que se compra. La procedencia es de la REMESA, y la remesa es el movimiento.
--
-- # Qué se exige y qué no
--
-- Solo la razón social. **El NIT no es obligatorio y no se valida su forma**: un
-- proveedor extranjero no tiene NIT, y una persona natural que factura tampoco
-- se identifica igual. Exigirlo dejaría fuera compras legítimas para ganar un
-- formato. Lo que sí se impide es que dos proveedores de la misma institución
-- compartan NIT cuando ambos lo declaran, porque entonces uno de los dos está
-- mal escrito.

CREATE TABLE proveedor (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    razon_social   TEXT    NOT NULL,
    nit            TEXT,
    contacto       TEXT,
    telefono       TEXT,
    email          TEXT,
    direccion      TEXT,
    ciudad         TEXT,
    -- Qué suministra, para filtrar el selector de la entrada. Texto libre a
    -- propósito: la lista de lo que una IPS compra no la cierra este esquema.
    suministra     TEXT,
    observaciones  TEXT,
    -- Un proveedor con el que ya no se trabaja NO se borra: sus entradas
    -- anteriores tienen que seguir diciendo de quién vinieron. Se desactiva, y
    -- deja de salir en el selector.
    activo         INTEGER NOT NULL DEFAULT 1
);

-- El NIT es único DENTRO de la institución, y solo cuando se declara: el índice
-- parcial deja convivir a varios proveedores sin NIT (que es NULL, y en SQL dos
-- NULL no son iguales) sin dejar pasar dos veces el mismo.
CREATE UNIQUE INDEX ux_proveedor_nit
    ON proveedor (institucion_id, nit) WHERE nit IS NOT NULL AND nit <> '';

CREATE INDEX ix_proveedor_institucion ON proveedor (institucion_id, activo, razon_social);

-- El eslabón. Nulo porque hay entradas que no vienen de una compra —una
-- donación, un traslado entre sedes, un ajuste de conteo— y obligar a inventar
-- un proveedor para esas es exactamente el defecto que este proyecto rechaza.
ALTER TABLE movimiento_inventario
    ADD COLUMN proveedor_id INTEGER REFERENCES proveedor(id);

-- Número de la factura o remisión con la que llegó la remesa. Es el documento
-- que un auditor pide primero y el que cierra el rastro hacia atrás.
ALTER TABLE movimiento_inventario ADD COLUMN documento_soporte TEXT;
