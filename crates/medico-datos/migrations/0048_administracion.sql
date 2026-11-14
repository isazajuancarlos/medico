-- Administración de la institución: su ficha, su configuración, su política de
-- seguridad, los campos dinámicos de la historia y los datos de talento humano
-- que le faltaban a `usuario`.
--
-- Es el último módulo del port. Va todo en una migración porque las cinco piezas
-- las escribe la MISMA pantalla y separarlas dejaría estados intermedios en los
-- que `/api/admin` responde a medias.
--
-- ─── Dos filas por institución, no una tabla de pares clave-valor ───
--
-- `configuracion_institucion` y `politica_seguridad` son 17 y 20 columnas con
-- tipo. Un `ajuste(clave, valor TEXT)` habría sido más corto de escribir y peor:
-- `max_intentos_login` sería texto, nadie garantizaría que existe, y el día que
-- alguien guarde `"cinco"` el fallo aparece al iniciar sesión y no al guardar.
-- Con columnas, el `CHECK` y el `NOT NULL DEFAULT` hacen ese trabajo una vez.
--
-- ─── Lo que NO se porta, y por qué ───
--
-- El modelo Python tiene `cuenta_bloqueada`, `intentos_fallidos` y
-- `fecha_bloqueo` en `usuario`, con su endpoint de desbloqueo. **Aquí no
-- entran.** Este binario ya decidió lo contrario y está escrito en
-- `medico-datos/src/freno.rs`: tras cinco fallos el siguiente intento ESPERA, la
-- espera se dobla y se detiene a los cinco minutos, y un acierto lo borra todo.
-- «No bloquea la cuenta: un bloqueo que exige a alguien desbloquear convierte
-- cinco peticiones de un desconocido en una historia clínica inaccesible a las
-- tres de la mañana».
--
-- Añadir las tres columnas sería un SEGUNDO mecanismo para lo mismo, y de los
-- dos el que gana en la práctica es el que bloquea (directiva 1). Lo que se
-- sirve como «desbloquear» borra los intentos de `intento_acceso` de ese correo,
-- que es exactamente lo que hace un acierto: quita la espera ahora.

-- ─────────────────────────────────────────────────────────────────────────────
-- La ficha de la institución
-- ─────────────────────────────────────────────────────────────────────────────
--
-- Todas nulas: una institución creada por la API o por `sembrar-demo` existe
-- antes de que nadie rellene su representante legal, y exigirlo dejaría sin dar
-- de alta a un cliente que todavía no ha mandado los papeles. `nombre` y `nit`,
-- que son los que identifican, ya eran obligatorios desde la 0001.
ALTER TABLE institucion ADD COLUMN razon_social                 TEXT;
ALTER TABLE institucion ADD COLUMN nombre_comercial             TEXT;
ALTER TABLE institucion ADD COLUMN direccion                    TEXT;
ALTER TABLE institucion ADD COLUMN ciudad                       TEXT;
ALTER TABLE institucion ADD COLUMN departamento                 TEXT;
ALTER TABLE institucion ADD COLUMN telefono                     TEXT;
ALTER TABLE institucion ADD COLUMN email                        TEXT;
ALTER TABLE institucion ADD COLUMN representante_legal          TEXT;
ALTER TABLE institucion ADD COLUMN identificacion_representante TEXT;
ALTER TABLE institucion ADD COLUMN codigo_prestador             TEXT;
ALTER TABLE institucion ADD COLUMN tipo_prestador               TEXT;
-- Los colores de la marca del prestador. La pantalla los pinta; si faltan, usa
-- los del producto.
ALTER TABLE institucion ADD COLUMN color_primario               TEXT;
ALTER TABLE institucion ADD COLUMN color_secundario             TEXT;

-- ─────────────────────────────────────────────────────────────────────────────
-- Talento humano: lo que le faltaba a `usuario`
-- ─────────────────────────────────────────────────────────────────────────────
--
-- Estas tres cierran una ausencia declarada: `InfoMedico` del contexto de
-- historia sirve `especialidad: None` con el comentario «llegan con el maestro
-- de talento humano». Llegan aquí.
ALTER TABLE usuario ADD COLUMN especialidad_id INTEGER REFERENCES especialidad(id);
ALTER TABLE usuario ADD COLUMN registro_medico TEXT;
ALTER TABLE usuario ADD COLUMN registro_rethus TEXT;
-- Al restablecer la contraseña, el usuario tiene que cambiarla al entrar: quien
-- la fijó la conoce, y una contraseña que conoce el administrador no identifica
-- a nadie.
ALTER TABLE usuario ADD COLUMN requiere_cambio_password INTEGER NOT NULL DEFAULT 0;
ALTER TABLE usuario ADD COLUMN ultimo_cambio_password   TEXT;

-- ─────────────────────────────────────────────────────────────────────────────
-- Configuración de operación
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE configuracion_institucion (
    institucion_id INTEGER PRIMARY KEY REFERENCES institucion(id) ON DELETE CASCADE,

    horario_apertura TEXT NOT NULL DEFAULT '07:00',
    horario_cierre   TEXT NOT NULL DEFAULT '18:00',
    -- JSON con los días que se atiende: ["lunes","martes",…]. Va como texto
    -- porque es una lista corta que solo se lee entera.
    dias_atencion    TEXT NOT NULL DEFAULT '["lunes","martes","miercoles","jueves","viernes"]',

    duracion_cita_default INTEGER NOT NULL DEFAULT 20 CHECK (duracion_cita_default > 0),
    tiempo_buffer         INTEGER NOT NULL DEFAULT 0  CHECK (tiempo_buffer >= 0),

    enviar_recordatorios INTEGER NOT NULL DEFAULT 0,
    horas_anticipacion   INTEGER NOT NULL DEFAULT 24 CHECK (horas_anticipacion >= 0),

    -- El correo saliente. **La contraseña NO vive aquí**: un secreto en una
    -- tabla que edita el administrador del cliente por HTTP se acaba leyendo en
    -- una copia de seguridad. Va por entorno, como el resto de secretos.
    smtp_host    TEXT,
    smtp_port    INTEGER CHECK (smtp_port IS NULL OR (smtp_port > 0 AND smtp_port <= 65535)),
    smtp_usuario TEXT,

    prefijo_factura        TEXT NOT NULL DEFAULT 'FE',
    resolucion_facturacion TEXT,
    rango_desde            INTEGER CHECK (rango_desde IS NULL OR rango_desde >= 0),
    rango_hasta            INTEGER CHECK (rango_hasta IS NULL OR rango_hasta >= 0),

    permitir_citas_online     INTEGER NOT NULL DEFAULT 0,
    requiere_autorizacion     INTEGER NOT NULL DEFAULT 0,
    dias_maximos_agendamiento INTEGER NOT NULL DEFAULT 90 CHECK (dias_maximos_agendamiento > 0),

    -- El rango de la resolución de facturación va de menor a mayor o no es un
    -- rango. Se comprueba aquí y no en la aplicación porque una resolución al
    -- revés no rechaza nada: acepta cualquier número y la DIAN devuelve la
    -- factura semanas después.
    CHECK (rango_desde IS NULL OR rango_hasta IS NULL OR rango_desde <= rango_hasta)
);

-- ─────────────────────────────────────────────────────────────────────────────
-- Política de seguridad
-- ─────────────────────────────────────────────────────────────────────────────
--
-- Los defectos NO son los más laxos posibles: son los que un prestador
-- colombiano puede sostener. Una institución que nunca abre esta pantalla queda
-- con 8 caracteres, mayúscula, minúscula y número exigidos, y con el rastro de
-- accesos encendido — que es lo que la Ley 1581 espera de quien trata datos de
-- categoría especial.
CREATE TABLE politica_seguridad (
    institucion_id INTEGER PRIMARY KEY REFERENCES institucion(id) ON DELETE CASCADE,

    longitud_minima_password   INTEGER NOT NULL DEFAULT 8
        CHECK (longitud_minima_password BETWEEN 8 AND 128),
    requiere_mayuscula         INTEGER NOT NULL DEFAULT 1,
    requiere_minuscula         INTEGER NOT NULL DEFAULT 1,
    requiere_numero            INTEGER NOT NULL DEFAULT 1,
    requiere_caracter_especial INTEGER NOT NULL DEFAULT 0,
    -- 0 = no expira. Un vencimiento corto empuja a la contraseña anotada en un
    -- papel, así que el defecto no fuerza rotación.
    dias_expiracion_password   INTEGER NOT NULL DEFAULT 0 CHECK (dias_expiracion_password >= 0),
    historial_passwords        INTEGER NOT NULL DEFAULT 0 CHECK (historial_passwords >= 0),

    timeout_sesion_minutos          INTEGER NOT NULL DEFAULT 60
        CHECK (timeout_sesion_minutos BETWEEN 5 AND 1440),
    sesiones_simultaneas_permitidas INTEGER NOT NULL DEFAULT 0
        CHECK (sesiones_simultaneas_permitidas >= 0),
    cerrar_otras_sesiones_al_login  INTEGER NOT NULL DEFAULT 0,

    -- El freno al ensayo de contraseñas vive en `freno.rs` y es una ESPERA que
    -- se dobla, no un bloqueo. Estos dos son los mandos de esa espera; la
    -- columna se llama `max_intentos_login` porque así lo llama la pantalla.
    max_intentos_login    INTEGER NOT NULL DEFAULT 5 CHECK (max_intentos_login BETWEEN 3 AND 20),
    tiempo_bloqueo_minutos INTEGER NOT NULL DEFAULT 5 CHECK (tiempo_bloqueo_minutos BETWEEN 1 AND 120),

    requiere_2fa       INTEGER NOT NULL DEFAULT 0,
    requiere_2fa_admin INTEGER NOT NULL DEFAULT 0,
    metodo_2fa         TEXT    NOT NULL DEFAULT 'NINGUNO'
        CHECK (metodo_2fa IN ('NINGUNO','TOTP','SMS','EMAIL')),

    registrar_todos_los_accesos INTEGER NOT NULL DEFAULT 1,
    retener_logs_dias           INTEGER NOT NULL DEFAULT 365
        CHECK (retener_logs_dias >= 0),

    notificar_login_nuevo_dispositivo INTEGER NOT NULL DEFAULT 0,
    notificar_cambio_password         INTEGER NOT NULL DEFAULT 1,
    notificar_intentos_fallidos       INTEGER NOT NULL DEFAULT 0
);

-- ─────────────────────────────────────────────────────────────────────────────
-- Campos dinámicos de la historia clínica
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE config_campo (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    etiqueta       TEXT    NOT NULL,
    tipo           TEXT    NOT NULL
        CHECK (tipo IN ('TEXTO','NUMERO','FECHA','BOOLEANO','SELECCION','AREA_TEXTO')),
    -- Las opciones de un desplegable, separadas por coma. Solo significa algo
    -- para `SELECCION`, y el CHECK lo exige: un desplegable sin opciones es un
    -- campo que el usuario no puede rellenar.
    opciones       TEXT,
    activo         INTEGER NOT NULL DEFAULT 1,
    orden          INTEGER NOT NULL DEFAULT 0,
    UNIQUE (institucion_id, etiqueta),
    CHECK (tipo <> 'SELECCION' OR (opciones IS NOT NULL AND length(trim(opciones)) > 0))
);

CREATE INDEX ix_config_campo_institucion ON config_campo (institucion_id, orden);
