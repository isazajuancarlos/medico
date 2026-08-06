-- Facturación: las cinco tablas del módulo, y NACEN CON DUEÑO.
--
-- =========================================================================
-- POR QUÉ ESTA MIGRACIÓN NO ES UNA COPIA DEL PYTHON
-- =========================================================================
--
-- En el sistema vivo, facturación **no está aislada entre instituciones**.
-- Medido el 2026-08-05 llamando a `core.aislamiento.institucion_de` con su
-- pareja de control:
--
--     Factura CON contrato_id     -> None = «de TODOS»    -> la ve cualquiera
--     Factura SIN contrato_id     -> RuntimeError          -> 500
--     Paciente(institucion_id=42) -> 42          (control: el mecanismo SÍ sabe)
--     Paciente sin institución    -> SIN_DUENIO  (control: y sí niega)
--
-- Tres piezas que se tapan entre sí lo producen: (1) ninguna de las cinco
-- tablas declara `institucion_id`; (2) la cadena de enlaces de `Factura` acaba
-- en `Contrato`, que está en `COMPARTIDOS`, así que termina en «de todos»; y
-- (3) `filtrar_por_institucion` de los listados es un no-op, porque solo añade
-- el WHERE `if hasattr(model, "institucion_id")` — la llamada está escrita y
-- parece que filtra.
--
-- Lo que se expone no son números: `detalle_factura` lleva `paciente_id` e
-- `historia_id` y el CUPS del procedimiento. Es qué se le facturó a qué paciente
-- en la clínica de al lado.
--
-- Es la misma clase que `ItemInventario`, y se cierra igual: **en Rust nace con
-- dueño**. Queda registrado como tarea aparte para el Python, que es el que hoy
-- corre en casa del cliente y necesita decidir a quién se le asigna cada fila
-- existente — eso no se puede adivinar y no se rellena con un defecto.

-- =========================================================================
-- ENTIDAD — la única que SÍ es catálogo compartido, y por un motivo
-- =========================================================================
--
-- Una EPS/ARL/aseguradora es la misma persona jurídica para todos los
-- prestadores del país: su NIT y su código ante el ADRES no cambian según quién
-- la mire. Por eso NO lleva `institucion_id`, igual que `cie10` y `cups`. Es la
-- decisión contraria a la de `contrato`, que está justo debajo, y las dos van
-- juntas a propósito para que se lean como el par que son.
CREATE TABLE entidad (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    nit         TEXT NOT NULL UNIQUE,
    nombre      TEXT NOT NULL,
    codigo_eps  TEXT NOT NULL
);

-- =========================================================================
-- CONTRATO — sale de «compartido» y gana dueño
-- =========================================================================
--
-- El Python lo declara en `COMPARTIDOS` junto a los catálogos de códigos, y es
-- justamente lo que hace que una factura acabe siendo «de todos». Un contrato es
-- el acuerdo entre UN prestador y UNA entidad, con SUS tarifas: no tiene nada de
-- catálogo nacional. Aquí lleva institución y es NOT NULL.
--
-- `numero_contrato` es único POR INSTITUCIÓN, no globalmente. En el Python es
-- `unique=True` a secas, y con dos prestadores en la misma base el segundo que
-- numere sus contratos «001» no puede guardarlo — un choque entre dos clientes
-- que no se conocen. Lo mismo pasa con `numero_factura`, abajo.
CREATE TABLE contrato (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id         INTEGER NOT NULL REFERENCES institucion(id),
    entidad_id             INTEGER NOT NULL REFERENCES entidad(id),
    nombre                 TEXT NOT NULL,
    numero_contrato        TEXT NOT NULL,
    fecha_inicio           TEXT NOT NULL,   -- fecha, AAAA-MM-DD
    fecha_fin              TEXT NOT NULL,
    activo                 INTEGER NOT NULL DEFAULT 1,
    -- Vocabulario cerrado desde el principio (lección de 0027: tres formas de
    -- escribir lo mismo dejaron dos pantallas muertas sin que nada fallara).
    tipo_tarifa            TEXT NOT NULL
        CHECK (tipo_tarifa IN ('ISS','SOAT','CUPS','PERSONALIZADA')),
    porcentaje_incremento  REAL NOT NULL DEFAULT 0,
    observaciones          TEXT,
    UNIQUE (institucion_id, numero_contrato)
);
CREATE INDEX ix_contrato_institucion ON contrato(institucion_id);

-- =========================================================================
-- FACTURA
-- =========================================================================
--
-- `contrato_id` sigue siendo opcional —se factura sin contrato: particulares,
-- SOAT—, y ESO es lo que en el Python hacía que la cadena de enlaces se
-- rompiera y levantara `RuntimeError`. Con `institucion_id` propio, el dueño ya
-- no depende de que exista el contrato.
--
-- Los importes van en REAL porque así están en el Python y el port tiene que
-- poder compararse byte a byte con él (regla de fidelidad del proyecto). Es una
-- deuda conocida —dinero en coma flotante— y NO se arregla aquí: cambiarlo
-- ahora rompería la comparación que autoriza a apagar el Python. Queda dicho.
CREATE TABLE factura (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id   INTEGER NOT NULL REFERENCES institucion(id),
    numero_factura   TEXT NOT NULL,
    entidad_id       INTEGER NOT NULL REFERENCES entidad(id),
    contrato_id      INTEGER REFERENCES contrato(id),
    fecha_factura    TEXT NOT NULL,   -- fecha, AAAA-MM-DD
    fecha_radicado   TEXT,
    num_radicado     TEXT,
    periodo_inicio   TEXT NOT NULL,
    periodo_fin      TEXT NOT NULL,
    valor_bruto      REAL NOT NULL DEFAULT 0,
    descuentos       REAL NOT NULL DEFAULT 0,
    valor_neto       REAL NOT NULL DEFAULT 0,
    estado           TEXT NOT NULL DEFAULT 'BORRADOR'
        CHECK (estado IN ('BORRADOR','RADICADA','ACEPTADA','GLOSADA','PAGADA')),
    observaciones    TEXT,
    -- `glosada` es DERIVADO de que exista una glosa, y aquí es una columna
    -- porque así está en el Python y el port se compara contra él. Quien la
    -- escriba tiene que hacerlo en la misma transacción que crea la glosa; si
    -- alguna vez divergen, la verdad es la tabla `glosa` (directiva 32: el
    -- nominal y el derivado son dos campos, y éste es el derivado).
    glosada          INTEGER NOT NULL DEFAULT 0,
    UNIQUE (institucion_id, numero_factura)
);
CREATE INDEX ix_factura_institucion ON factura(institucion_id);
CREATE INDEX ix_factura_entidad     ON factura(entidad_id);

-- =========================================================================
-- DETALLE_FACTURA y GLOSA — cuelgan de la factura, y por eso NO llevan
-- institucion_id
-- =========================================================================
--
-- Es la misma regla que ya sigue el resto del sistema: los modelos clínicos no
-- llevan institución, cuelgan del paciente. Aquí cuelgan de la factura con un
-- `factura_id NOT NULL`, así que su dueño es demostrable en un salto y no puede
-- quedar huérfano. Añadirles la columna crearía la posibilidad de que un detalle
-- dijera una institución y su factura otra — dos verdades para un hecho.
--
-- `ON DELETE CASCADE` porque borrar una factura sin sus líneas dejaría filas con
-- `paciente_id` apuntando a personas, sin nada que las alcance ni las borre: un
-- dato personal que sobrevive a su motivo (Ley 1581).
CREATE TABLE detalle_factura (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    factura_id      INTEGER NOT NULL REFERENCES factura(id) ON DELETE CASCADE,
    servicio        TEXT NOT NULL,
    -- SIN DEFAULT, y es deliberado. El Python trae `default="890201"`, que es
    -- exactamente el dato falso que este proyecto lleva meses quitando: un CUPS
    -- con la forma correcta que nadie afirmó. Si no se sabe, va NULL y el
    -- recolector de faltantes lo anota.
    codigo_cups     TEXT,
    cantidad        INTEGER NOT NULL DEFAULT 1,
    valor_unitario  REAL NOT NULL DEFAULT 0,
    valor_total     REAL NOT NULL DEFAULT 0,
    paciente_id     INTEGER REFERENCES paciente(id),
    historia_id     INTEGER REFERENCES historia(id)
);
CREATE INDEX ix_detalle_factura ON detalle_factura(factura_id);

CREATE TABLE glosa (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    factura_id       INTEGER NOT NULL REFERENCES factura(id) ON DELETE CASCADE,
    item_glosado     TEXT NOT NULL,
    motivo           TEXT NOT NULL,
    valor_glosado    REAL NOT NULL,
    fecha_glosa      TEXT NOT NULL,
    respuesta        TEXT,
    fecha_respuesta  TEXT,
    estado           TEXT NOT NULL DEFAULT 'PENDIENTE'
        CHECK (estado IN ('PENDIENTE','ACEPTADA','RECHAZADA')),
    -- JSON de rutas, como en el Python. Va TEXT porque SQLite no tiene tipo
    -- JSON; lo valida el código, no la columna.
    documentos_adjuntos TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX ix_glosa_factura ON glosa(factura_id);
