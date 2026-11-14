-- El segundo factor: TOTP y sus códigos de recuperación (#212).
--
-- El prerrequisito ya está puesto desde la 0053: sin el freno al ensayo de
-- contraseñas (`medico_datos::freno`, sobre `guaca::freno`), seis dígitos con
-- tolerancia de una ventana a cada lado se prueban hasta acertar y el segundo
-- factor no añade nada. Van juntos o no van.
--
-- La REGLA no vive aquí ni en `segundofactor.rs`: vive en `guaca::dosfactores`,
-- que subió desde informes el 2026-08-12 justo porque este es el segundo
-- proyecto que la necesita (directiva 9). Lo que la aplicación pone son estas
-- columnas; cuántas ventanas se toleran, qué cuenta como reutilización y qué se
-- hace con un secreto ilegible se audita una sola vez, allí.

-- El secreto compartido, CIFRADO. En claro es exactamente igual de útil para
-- quien copie el archivo que para su dueño: con él se generan los códigos, así
-- que un secreto en claro convierte el segundo factor en un adorno frente al
-- único atacante que importa aquí — el que ya se llevó la base. Y en medico esa
-- base es una historia clínica, categoría especial de la Ley 1581.
--
-- Se envuelve con `guaca::reposo` (Argon2id -> AEAD -> relleno Padmé), el mismo
-- camino que los adjuntos, y por tanto **solo hay 2FA si hay
-- MEDICO_STORAGE_KEY**. Eso no es una limitación que se descubra tarde: el alta
-- del segundo factor lo dice y se niega a empezar, en vez de guardar el secreto
-- en claro «mientras tanto» (directiva 20).
ALTER TABLE usuario ADD COLUMN totp_secreto_cifrado BLOB;

-- Cuándo se ACTIVÓ, en RFC 3339 como el resto de la base. NULL significa «no
-- tiene segundo factor», y es lo único que decide si el acceso pide código.
--
-- Es una columna aparte del secreto A PROPÓSITO, y es la directiva 32: el
-- secreto y la vigencia son dos hechos distintos. Entre generar el secreto y
-- comprobar que la persona sabe leerlo hay un paso —teclear un código— y en ese
-- intervalo el secreto EXISTE pero el factor NO está activo. Con una sola
-- columna, quien cerrara la pantalla del QR a medias se quedaría fuera de su
-- propia cuenta sin haber llegado a enrolar nada.
ALTER TABLE usuario ADD COLUMN totp_confirmado_en TEXT;

-- El último contador de 30 s que se dio por bueno. Es TODO el antirreplay: un
-- código capturado y reenviado cae en un contador que ya no avanza.
--
-- NULL = todavía no se usó ninguno, que no es lo mismo que 0: con 0 el primer
-- código válido de la historia entraría igual, pero la distinción importa el día
-- que alguien lea la columna para saber si la cuenta llegó a usarse.
--
-- SE QUEMA EN EL MISMO `UPDATE` QUE LO COMPRUEBA, mirando `rows_affected`. La
-- revisión de seguridad de informes encontró la forma obvia —leer, verificar,
-- escribir— y midió lo que cuesta: entre la lectura y la escritura pasan el
-- turno del semáforo y un Argon2id, o sea cientos de milisegundos, y tres
-- peticiones simultáneas con el mismo código abrían tres sesiones. Aquí no se
-- repite: `WHERE ... AND (totp_ultimo_contador IS NULL OR totp_ultimo_contador < ?)`.
ALTER TABLE usuario ADD COLUMN totp_ultimo_contador INTEGER;

-- Los códigos de recuperación, HASHEADOS con el mismo Argon2 que las
-- contraseñas (`guaca::claves`). En claro serían una contraseña maestra escrita
-- en la puerta: diez cadenas que saltan el segundo factor, guardadas junto a lo
-- que protegen.
--
-- Uno por fila y no una lista en una columna, porque cada uno se gasta por
-- separado: `usado_en` no nulo es un código consumido, y BORRAR la fila perdería
-- la única prueba de que alguien entró por ahí. Esa prueba es lo que un
-- responsable de tratamiento necesita para responder «quién accedió» bajo la
-- Ley 1581.
CREATE TABLE IF NOT EXISTS codigo_recuperacion (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    usuario_id INTEGER NOT NULL REFERENCES usuario(id) ON DELETE CASCADE,
    hash       TEXT    NOT NULL,
    creado_en  TEXT    NOT NULL,
    usado_en   TEXT
);

-- La consulta es siempre «los códigos SIN gastar de este usuario».
CREATE INDEX IF NOT EXISTS idx_codigo_recuperacion_usuario
    ON codigo_recuperacion (usuario_id, usado_en);
