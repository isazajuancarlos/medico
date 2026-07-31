-- Farmacia: inventario, movimientos y dispensación (#65, port a Rust de
-- backend/routers/farmacia.py).
--
-- # El inventario ES de una institución, y en el original no lo era
--
-- `ItemInventario` de `backend/models.py:1172` NO tiene `institucion_id`. En un
-- sistema multi-institucional eso significa que el stock, los lotes, los costos
-- y las alertas de vencimiento de TODOS los prestadores están en la misma bolsa:
-- cualquier regente ve el inventario de la clínica de al lado, y —peor— puede
-- descontarlo dispensando contra él. Es el mismo hallazgo que la 0023 corrigió
-- en las camas, en un módulo donde además hay dinero.
--
-- Aquí nace con dueño obligatorio. No es una mejora opcional del port: sin la
-- columna, `Ambito` no tiene contra qué comparar y la capa de datos no podría
-- conceder ni negar nada.
--
-- # Lo que NO se guarda: `valor_total`
--
-- El original tiene una columna `valor_total` que es `cantidad * costo`. Un dato
-- derivado guardado en otra columna se desincroniza en cuanto una de las dos
-- fuentes cambia por un camino que nadie recordó actualizar — y entonces hay dos
-- respuestas a la misma pregunta y ninguna manda. Se calcula en la consulta.
--
-- # `cantidad_actual >= 0` es la regla, no un adorno
--
-- El código comprueba existencias antes de dispensar, pero esa comprobación se
-- puede olvidar en el camino siguiente. El CHECK no se olvida: una salida que
-- deje el stock en negativo aborta la transacción entera. Es la diferencia entre
-- una regla que hay que recordar y una que no se puede romper.

CREATE TABLE item_inventario (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id         INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    -- Enlace opcional al maestro CUM. Opcional a propósito: un insumo (gasa,
    -- jeringa) no está en el CUM y exigirlo dejaría fuera media farmacia.
    medicamento_id         INTEGER REFERENCES medicamento(id),
    nombre                 TEXT    NOT NULL,
    tipo                   TEXT    NOT NULL DEFAULT 'MEDICAMENTO'
        CHECK (tipo IN ('MEDICAMENTO','INSUMO','DISPOSITIVO','REACTIVO')),
    categoria              TEXT,
    presentacion           TEXT,
    -- El lote es parte de la identidad de la fila: dos lotes del mismo
    -- medicamento son dos filas, porque vencen en fechas distintas y la
    -- trazabilidad de un retiro sanitario se hace POR LOTE.
    lote                   TEXT    NOT NULL DEFAULT '',
    -- YYYY-MM-DD. NULL = no vence (un dispositivo, un instrumento). NULL es
    -- «no vence», no «no sé»: si no se sabe, no se pone fecha y el sistema no
    -- puede afirmar que está vigente.
    fecha_vencimiento      TEXT,
    registro_invima        TEXT,
    cantidad_actual        INTEGER NOT NULL DEFAULT 0 CHECK (cantidad_actual >= 0),
    cantidad_minima        INTEGER NOT NULL DEFAULT 10 CHECK (cantidad_minima >= 0),
    unidad_medida          TEXT    NOT NULL DEFAULT 'UNIDAD',
    ubicacion              TEXT    NOT NULL DEFAULT 'FARMACIA',
    costo_unitario         REAL    NOT NULL DEFAULT 0 CHECK (costo_unitario >= 0),
    requiere_refrigeracion INTEGER NOT NULL DEFAULT 0,
    controlado             INTEGER NOT NULL DEFAULT 0,
    -- Dentro de una institución, un nombre y un lote son una sola fila. Sin
    -- esto, dos altas del mismo lote parten el stock en dos y el conteo real
    -- deja de coincidir con ninguna de las dos filas.
    UNIQUE (institucion_id, nombre, lote)
);

CREATE INDEX ix_item_institucion ON item_inventario (institucion_id, nombre);
CREATE INDEX ix_item_vencimiento ON item_inventario (institucion_id, fecha_vencimiento);

-- Toda variación de existencias deja rastro. No hay forma de mover stock sin
-- escribir aquí: `inventario.rs` no expone ningún UPDATE de `cantidad_actual`
-- que no venga acompañado de su movimiento en la misma transacción.
CREATE TABLE movimiento_inventario (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    -- SIN `ON DELETE CASCADE`, a diferencia de casi todo lo demás del esquema:
    -- borrar el item no puede llevarse su historia de movimientos por delante.
    -- Con la cascada, dar de baja un medicamento controlado borraba en silencio
    -- el rastro de a quién se le entregó. Así la clave foránea IMPIDE el borrado,
    -- y `eliminar()` lo dice con palabras antes de que SQLite lo diga con un
    -- código de error.
    item_inventario_id INTEGER NOT NULL REFERENCES item_inventario(id),
    usuario_id         INTEGER NOT NULL REFERENCES usuario(id),
    fecha              TEXT    NOT NULL DEFAULT (datetime('now')),
    tipo_movimiento    TEXT    NOT NULL
        CHECK (tipo_movimiento IN ('ENTRADA','SALIDA','AJUSTE','VENCIMIENTO')),
    -- Siempre positiva: el signo lo da `tipo_movimiento`. Guardar cantidades
    -- negativas y además un tipo es tener el signo escrito dos veces, y basta
    -- que discrepen una vez para que el histórico no cuadre con el stock.
    cantidad           INTEGER NOT NULL CHECK (cantidad > 0),
    motivo             TEXT    NOT NULL,
    paciente_id        INTEGER REFERENCES paciente(id),
    formula_id         INTEGER REFERENCES formula(id)
);

CREATE INDEX ix_movimiento_item ON movimiento_inventario (item_inventario_id, fecha);

-- La entrega al paciente contra una fórmula médica.
--
-- NO hay UNIQUE sobre `formula_id`, y es deliberado: la entrega parcial existe
-- —la farmacia entrega 20 de las 30 tabletas y deja el resto pendiente— y es
-- justo el caso que más reclama el paciente. Lo que sí se impide, en el código y
-- con la suma de lo ya entregado, es dispensar MÁS de lo prescrito.
CREATE TABLE dispensacion (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Tampoco cascada aquí, y por lo mismo: borrar la fórmula no puede borrar la
    -- constancia de que el medicamento se entregó. `formula::eliminar` comprueba
    -- antes y responde 409 en vez de dejar que reviente la clave foránea.
    formula_id          INTEGER NOT NULL REFERENCES formula(id),
    item_inventario_id  INTEGER NOT NULL REFERENCES item_inventario(id),
    -- Denormalizado desde la fórmula → historia → paciente, no recibido del
    -- cliente: quien dispensa no elige a quién se le apunta la entrega.
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    regente_farmacia_id INTEGER NOT NULL REFERENCES usuario(id),
    fecha_dispensacion  TEXT    NOT NULL DEFAULT (datetime('now')),
    cantidad_dispensada INTEGER NOT NULL CHECK (cantidad_dispensada > 0),
    -- El lote REAL del que salió, copiado del item. Un lote distinto del que
    -- tiene la fila de inventario haría inútil la trazabilidad, que es el único
    -- motivo por el que este campo existe.
    lote_dispensado     TEXT    NOT NULL,
    observaciones       TEXT
);

CREATE INDEX ix_dispensacion_formula ON dispensacion (formula_id);
CREATE INDEX ix_dispensacion_paciente ON dispensacion (paciente_id, fecha_dispensacion);
