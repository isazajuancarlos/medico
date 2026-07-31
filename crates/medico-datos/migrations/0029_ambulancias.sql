-- Ambulancias y traslados asistenciales.
--
-- Módulo NUEVO: no existe en el Visual FoxPro del prestador, ni en el sistema
-- Python, ni en la interfaz. No hay original al que ser fiel, así que lo que
-- manda es el marco en el que opera un prestador colombiano: el transporte
-- asistencial es un servicio que se habilita, y sus dos modalidades son TAB
-- (básico) y TAM (medicalizado). Un traslado es PRIMARIO cuando lleva al
-- paciente desde donde ocurrió el evento hasta el prestador, y SECUNDARIO cuando
-- va de un prestador a otro — que es el que se engancha con una remisión.
--
-- # El parque es de la institución
--
-- Igual que las camas (0023) y el inventario (0025). Una ambulancia que se puede
-- despachar desde otra institución es la misma avería que la cama ajena
-- ocupada: la que la ve como «En Traslado» no puede usarla ni liberarla.
--
-- # Sobre el SOAT y por qué la regla NO es «sin fecha no sale»
--
-- Despachar con SOAT VENCIDO se rechaza: es un hecho conocido, la póliza es
-- obligatoria y el vehículo circula sin amparo. Pero una fecha AUSENTE no
-- demuestra nada, y bloquear un despacho por un campo que nadie llenó convierte
-- un problema administrativo en una demora asistencial — que es exactamente
-- contra lo que hay que probar una restricción antes de escribirla. Así que la
-- ausencia no bloquea: se marca, y se ve. Lo que el sistema no hace, ni aquí ni
-- en ningún otro sitio, es inventarse una fecha para poder decir que está al día.

CREATE TABLE ambulancia (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id   INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    placa            TEXT    NOT NULL,
    -- TAB: transporte asistencial básico. TAM: medicalizado (con médico a bordo
    -- y equipamiento de soporte avanzado). No hay una tercera: lo que no es una
    -- de las dos no es una ambulancia habilitada.
    tipo             TEXT    NOT NULL DEFAULT 'TAB' CHECK (tipo IN ('TAB','TAM')),
    numero_interno   TEXT,
    marca            TEXT,
    modelo           TEXT,
    estado           TEXT    NOT NULL DEFAULT 'Disponible'
        CHECK (estado IN ('Disponible','En Traslado','En Mantenimiento','Fuera de Servicio')),
    -- YYYY-MM-DD. NULL = no registrado, que NO es lo mismo que vigente.
    vence_soat       TEXT,
    vence_tecnomecanica TEXT,
    observaciones    TEXT,
    activa           INTEGER NOT NULL DEFAULT 1,
    UNIQUE (institucion_id, placa)
);

CREATE INDEX ix_ambulancia_institucion ON ambulancia (institucion_id, activa, estado);

CREATE TABLE traslado (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    ambulancia_id     INTEGER NOT NULL REFERENCES ambulancia(id),
    paciente_id       INTEGER NOT NULL REFERENCES paciente(id),
    -- La remisión que lo origina, cuando la hay. Es una `orden` de tipo
    -- «Remisión Especialista»; opcional porque un traslado primario —recoger a
    -- alguien en la vía— no nace de ninguna remisión.
    remision_id       INTEGER REFERENCES orden(id),
    tipo_traslado     TEXT    NOT NULL DEFAULT 'Primario'
        CHECK (tipo_traslado IN ('Primario','Secundario')),
    origen            TEXT    NOT NULL,
    destino           TEXT    NOT NULL,
    motivo            TEXT    NOT NULL,
    estado            TEXT    NOT NULL DEFAULT 'En Curso'
        CHECK (estado IN ('En Curso','Finalizado','Cancelado')),
    fecha_salida      TEXT    NOT NULL DEFAULT (datetime('now')),
    -- Se llena al cerrar. Nula mientras el traslado está en curso: poner la de
    -- salida «provisionalmente» daría traslados de cero minutos en el informe.
    fecha_llegada     TEXT,
    conductor         TEXT,
    personal_asistencial TEXT,
    registrado_por_id INTEGER NOT NULL REFERENCES usuario(id),
    observaciones     TEXT
);

CREATE INDEX ix_traslado_ambulancia ON traslado (ambulancia_id, estado);
CREATE INDEX ix_traslado_paciente ON traslado (paciente_id, fecha_salida);

-- UNA ambulancia, UN traslado en curso.
--
-- El código lo comprueba antes de despachar, pero esa comprobación se puede
-- olvidar en la ruta siguiente; el índice no se olvida. Es parcial —solo mira
-- las filas «En Curso»— para que el historial pueda tener cien traslados
-- cerrados de la misma ambulancia.
CREATE UNIQUE INDEX ux_traslado_en_curso
    ON traslado (ambulancia_id) WHERE estado = 'En Curso';
