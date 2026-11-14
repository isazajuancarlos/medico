-- REGISTRO_NACIMIENTO (#249), y con él la mitad `recienNacidos` de #67.
--
-- Es el ÚNICO de los 22 módulos pendientes con motivo externo, y el motivo no es
-- que falte una pantalla: la Resolución 948 define SIETE arreglos en el FEV-RIPS
-- y medico emite CINCO. `recienNacidos` no se puede emitir porque **no hay dónde
-- capturarlo** — en todo el dominio no existía una sola fila que describiera a un
-- recién nacido. Emitirlo sin esta tabla sería inventar el dato, que es cobrarle
-- a la EPS una atención que no se puede sustentar.
--
-- # La forma la fija el VALIDADOR, no la memoria
--
-- Las columnas salen de `Rips.RecienNacidos` del validador del MUV, leídas del
-- contenedor `fevrips-db` el 2026-08-16 (21 columnas, con sus tipos y longitudes),
-- no del recuerdo de una sesión anterior. Lo que ahí es `varchar(6)` aquí es TEXT
-- —SQLite no tiene longitudes—, pero la longitud se conserva escrita porque es lo
-- que el validador rechaza.
--
-- # El recién nacido NO es un paciente todavía, y por eso no cuelga de `paciente`
--
-- La atención facturable es la de la MADRE: el arreglo `recienNacidos` viaja
-- DENTRO de su `usuarios[].servicios`, y el validador lo ata con
-- `FK_RecienNacidos_Usuarios_UsuarioId`. Un recién nacido que no ha sido dado de
-- alta no tiene documento propio, así que crearle una fila en `paciente` obligaría
-- a inventarle una identificación — exactamente el relleno que prohíbe la
-- directiva 20. Cuelga de la madre y de la hospitalización donde nació.
--
-- Cuando SÍ lo tenga (registro civil), el enlace se añade sin migrar nada: es la
-- columna `paciente_id`, nula mientras no exista.
--
-- # Los tres documentos que puede traer, y por qué es nulable
--
-- Del catálogo `Rips.REFs_TipoDoc` del propio validador: `CN` certificado de
-- nacido vivo, `RC` registro civil, `MS` menor sin identificar. Es NULABLE en el
-- validador y lo es aquí: un parto atendido a las 3 de la mañana no tiene
-- certificado hasta que alguien lo expide, y forzar un valor produciría el dato
-- falso que no se nota nunca.
--
-- # `peso_gramos` es un ENTERO en GRAMOS, y va nombrado en la columna
--
-- El validador lo declara `float` y la norma lo llama `peso` a secas. Un campo de
-- peso sin unidad es la trampa clásica: 3,4 se lee como kilos y 3400 como gramos,
-- y los dos «funcionan». El RIPS lo define en gramos desde la Resolución 3374, y
-- aquí la unidad va EN EL NOMBRE para que ninguna pantalla futura tenga que
-- adivinarla. Se emite como número, que es lo que el `float` del validador espera.
--
-- # `edad_gestacional` en SEMANAS, con su rango
--
-- El `CHECK` no rechaza lo inusual, rechaza lo IMPOSIBLE (directiva 21): hay
-- partos viables desde la semana 22 y embarazos que llegan a la 45, así que el
-- borde se pone donde ningún parto humano cabe, no donde empieza lo raro.
--
-- # `condicion_destino` con sus ocho valores, del validador
--
-- `Rips.REFs_CondicionDestino`: 01 domicilio · 02 muerto · 03 derivado a otro
-- servicio · 04 referido · 05 contrarreferido · 06 hospitalización domiciliaria ·
-- 07 servicio social · 08 continúa en el servicio. Se escriben aquí porque un
-- código que no está en la tabla del MUV es un rechazo del paquete entero.
--
-- # La causa de muerte no se deduce del destino
--
-- Misma lección que la 0041 en hospitalización: `condicion_destino = '02'` dice
-- que el recién nacido murió, y NO dice de qué. Son dos datos y se capturan los
-- dos; deducir el segundo del primero deja el RIPS sin causa básica de muerte.
CREATE TABLE recien_nacido (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    -- La madre, que es quien tiene la atención facturable.
    madre_paciente_id        INTEGER NOT NULL REFERENCES paciente(id),
    -- El parto. Nulable porque un nacimiento puede atenderse fuera de una
    -- hospitalización abierta (parto en urgencias que no llega a ingresar).
    hospitalizacion_id       INTEGER REFERENCES hospitalizacion(id),
    -- Cuando el recién nacido llegue a tener historia propia. Hoy siempre NULL.
    paciente_id              INTEGER REFERENCES paciente(id),

    -- Identificación PROPIA, varchar(2)/varchar(20) en el validador.
    tipo_documento           TEXT CHECK (tipo_documento IN ('CN','RC','MS')),
    numero_documento         TEXT,

    fecha_nacimiento         TEXT NOT NULL,   -- RFC 3339 UTC, con hora
    edad_gestacional         INTEGER NOT NULL CHECK (edad_gestacional BETWEEN 20 AND 45),
    num_consultas_prenatal   INTEGER NOT NULL CHECK (num_consultas_prenatal >= 0),
    -- `M`/`F`/`I`, el mismo vocabulario que `codSexo` del usuario ya emite.
    sexo_biologico           TEXT NOT NULL CHECK (sexo_biologico IN ('M','F','I')),
    peso_gramos              INTEGER NOT NULL CHECK (peso_gramos BETWEEN 200 AND 8000),

    -- CIE-10, varchar(6) en el validador.
    diagnostico_principal    TEXT NOT NULL,
    condicion_destino        TEXT CHECK (condicion_destino IN ('01','02','03','04','05','06','07','08')),
    diagnostico_causa_muerte TEXT,
    fecha_egreso             TEXT,
    -- Certificado de nacido vivo del RUAF-ND. varchar(256) en el validador.
    codigo_vida              TEXT,

    -- Auditoría, como el resto del dominio.
    creado_en                TEXT NOT NULL,
    creado_por               INTEGER REFERENCES usuario(id)
);

-- El FEV se arma por periodo y por madre: los dos filtros de la consulta.
CREATE INDEX idx_recien_nacido_madre ON recien_nacido (madre_paciente_id, fecha_nacimiento);
CREATE INDEX idx_recien_nacido_hosp  ON recien_nacido (hospitalizacion_id);
