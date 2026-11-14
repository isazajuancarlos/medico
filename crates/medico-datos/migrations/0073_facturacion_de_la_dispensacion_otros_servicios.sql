-- El arreglo `otrosServicios` (AT) de la 948, séptimo y último (#67). medico
-- emitía seis; el Python lo llevaba cableado a cero (`rips_completo.py:1417`),
-- así que esto no es portar: es construir el dato que no existía en ninguna de
-- las dos implementaciones.
--
-- ALCANCE (decisión de Juan, 2026-09-08): solo insumos/dispositivos, sin SISPRO.
-- `tipoOS = "01"` (Dispositivos médicos e insumos); la transición de la 948
-- admite el código propio del facturador como `codTecnologiaSalud`, así que se
-- usa `item_inventario.registro_invima` mientras no haya IUM (columna `ium`
-- abajo, opcional, para cuando se tenga).
--
-- # Por qué se RECONSTRUYE `dispensacion` y no se le hace ALTER
--
-- Un `otrosServicios` es una tecnología SUMINISTRADA a un paciente, valorada. La
-- fuente natural es `dispensacion` (0025), pero hoy toda dispensación cuelga de
-- una `formula` de MEDICAMENTO (`formula_id NOT NULL`, y `formula.medicamento`
-- es texto obligatorio). Un insumo/dispositivo entregado en una atención no es
-- una fórmula de medicamento, así que hoy NO HAY forma de capturarlo.
--
-- Opción elegida (Juan, «opción 2»): la dispensación de un insumo cuelga de la
-- ATENCIÓN (`historia_id`), no de una fórmula. Un medicamento sigue colgando de
-- `formula_id`, exactamente como hoy. La regla la fija el CHECK: exactamente uno
-- de los dos. SQLite no tiene `ALTER COLUMN` para quitar el `NOT NULL` ni para
-- añadir el CHECK, así que toca el procedimiento de reconstrucción de tabla que
-- SQLite documenta. Las migraciones corren con `foreign_keys=OFF` y el arranque
-- valida con `PRAGMA foreign_key_check` (ver `lib.rs`), así que el `DROP` no
-- dispara cascadas y ninguna fila se pierde. Nada referencia `dispensacion` por
-- clave foránea (verificado), y sus dos índices se recrean abajo.
--
-- # Todas las columnas nuevas NULL y sin DEFAULT
--
-- Es la lección de la 0041/0038: un `DEFAULT` dejaría al recolector —que solo ve
-- AUSENCIAS— sin nada que anotar, y el archivo afirmaría un valor que nadie
-- registró. De `vrUnitOS` depende lo que se le cobra a la EPS, así que un valor
-- inventado es un cobro sin sustento. NULL = «no se registró»; el generador se
-- niega a suponerlo y lo saca en el informe de cierre con el id de la fila.

CREATE TABLE dispensacion_nueva (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Un medicamento cuelga de su fórmula (como hasta hoy); un insumo/
    -- dispositivo de `otrosServicios` cuelga de la atención. Exactamente uno.
    formula_id          INTEGER REFERENCES formula(id),
    historia_id         INTEGER REFERENCES historia(id),
    item_inventario_id  INTEGER NOT NULL REFERENCES item_inventario(id),
    -- Denormalizado desde la fórmula → historia → paciente, o desde la atención
    -- directamente: quien dispensa no elige a quién se le apunta la entrega.
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    regente_farmacia_id INTEGER NOT NULL REFERENCES usuario(id),
    fecha_dispensacion  TEXT    NOT NULL DEFAULT (datetime('now')),
    cantidad_dispensada INTEGER NOT NULL CHECK (cantidad_dispensada > 0),
    lote_dispensado     TEXT    NOT NULL,
    observaciones       TEXT,
    -- ── Facturación del `otrosServicios` (18 campos de la 948) ──
    -- Los clínico-logísticos ya están arriba (fecha, paciente, cantidad, ítem).
    -- Estos son los de FACTURACIÓN, que medico no tenía para una dispensación.
    -- `numAutorizacion` e `idMIPRES`: autorización de la EPS y prescripción
    -- MIPRES del suministro. `null` cuando no aplica (el emisor manda null, no "").
    num_autorizacion       TEXT,
    id_mipres              TEXT,
    -- Los tres valores. `vrUnitOS` es la TARIFA unitaria pactada, NO el
    -- `item_inventario.costo_unitario` (que es costo, no tarifa). Enteros en
    -- pesos, como el resto del RIPS.
    valor_unitario         INTEGER,
    valor_dispensacion     INTEGER,
    valor_servicio         INTEGER,
    concepto_recaudo       TEXT,
    valor_pago_moderador   INTEGER,
    num_fev_pago_moderador TEXT,
    -- Exactamente uno de {formula_id, historia_id}: medicamento vs. otro
    -- servicio. Las filas de hoy (formula_id puesto, historia_id nulo) pasan.
    CHECK ((formula_id IS NULL) <> (historia_id IS NULL))
);

-- Las filas existentes son todas dispensación de medicamento: formula_id puesto,
-- historia_id e todo lo de facturación en NULL. Copia 1:1 de lo que había.
INSERT INTO dispensacion_nueva
    (id, formula_id, item_inventario_id, paciente_id, regente_farmacia_id,
     fecha_dispensacion, cantidad_dispensada, lote_dispensado, observaciones)
SELECT
    id, formula_id, item_inventario_id, paciente_id, regente_farmacia_id,
    fecha_dispensacion, cantidad_dispensada, lote_dispensado, observaciones
FROM dispensacion;

DROP TABLE dispensacion;
ALTER TABLE dispensacion_nueva RENAME TO dispensacion;

CREATE INDEX ix_dispensacion_formula ON dispensacion (formula_id);
CREATE INDEX ix_dispensacion_paciente ON dispensacion (paciente_id, fecha_dispensacion);
CREATE INDEX ix_dispensacion_historia ON dispensacion (historia_id);

-- IUM (Identificador Único de Medicamento/dispositivo) para `codTecnologiaSalud`
-- de la 948. Opcional: hoy se usa `registro_invima` como código propio durante
-- la transición; esta columna se llena cuando el prestador tenga el IUM. NULL =
-- «no se tiene», no se inventa.
ALTER TABLE item_inventario ADD COLUMN ium TEXT;
