-- Lo que una institución APARTA del defecto de autorización (#235).
--
-- Solo las excepciones. Una institución que no configura nada no tiene ni una
-- fila y sigue el defecto entero; si mañana el defecto cambia, se mueve con él.
-- Guardar la matriz completa haría lo contrario: congelaría en cada cliente el
-- defecto del día de la instalación, sin que nadie lo hubiera decidido.
--
-- `permitido` es 0 o 1 y la ausencia de fila NO es un 0: es «sigue el defecto».
-- Por eso la tabla guarda un booleano y no es una lista de lo concedido — una
-- lista no sabría distinguir «se lo quitaron» de «nunca lo tuvo», y solo lo
-- primero debe sobrevivir a un cambio del defecto.
--
-- Qué NO puede aparecer aquí, comprobado en el dominio antes de escribir
-- (`medico_dominio::ajustes::Ajustes::validar`) y no por un CHECK, porque la
-- razón es de negocio y el mensaje tiene que llegar al administrador:
--   · el rol `superadmin`, que no pertenece a ninguna institución;
--   · el rol `paciente`, cuyo ámbito es Ninguno mientras no exista el portal;
--   · la capacidad `gestionar_instituciones`, que vive por encima de la
--     institución — concederla desde aquí sería una escalada, no una
--     configuración.
CREATE TABLE IF NOT EXISTS capacidad_rol (
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    capacidad      TEXT    NOT NULL,
    rol            TEXT    NOT NULL,
    permitido      INTEGER NOT NULL CHECK (permitido IN (0, 1)),
    -- Quién y cuándo. El rastro fuerte va a la bitácora firmada (`acceso`);
    -- esto es para que la propia fila diga de dónde salió al mirarla en la base.
    --
    -- `ON DELETE SET NULL` y no la acción por defecto: dar de baja a un usuario
    -- no puede quedar bloqueado porque una vez tocó esta matriz, ni debe
    -- arrastrarse la configuración de la institución con él. Se pierde el
    -- nombre, no la regla — y el rastro de quién la cambió sigue entero en la
    -- bitácora firmada, que es donde vale.
    cambiado_por   INTEGER REFERENCES usuario(id) ON DELETE SET NULL,
    cambiado_en    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (institucion_id, capacidad, rol)
);

-- Se lee entera por institución al calentar la caché, así que este índice cubre
-- la única consulta que existe.
CREATE INDEX IF NOT EXISTS idx_capacidad_rol_institucion
    ON capacidad_rol (institucion_id);
