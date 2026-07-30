-- Freno al ensayo de contraseñas (#194 → prerrequisito del 2FA).
--
-- Hasta aquí `/api/acceso` admitía intentos sin límite: con Argon2 cada uno
-- cuesta ~100 ms, lo que ya frena algo, pero no impide dejar corriendo un
-- diccionario toda la noche contra la cuenta de un médico.
--
-- # Por qué la clave es (email, origen) y no solo el email
--
-- Un freno POR CUENTA se puede volver contra el cliente: basta con fallar cinco
-- veces contra el correo de la doctora de turno para dejarla fuera cuando más
-- falta hace. En un consultorio esa espera no es una molestia, es un riesgo
-- clínico. Con el origen en la clave, quien falla se frena a SÍ MISMO: el
-- atacante remoto no puede bloquear la estación de la doctora.
--
-- Si no hay proxy configurado todos los intentos comparten origen y esto
-- degrada exactamente al freno por cuenta, que es la forma segura de degradar.
--
-- # Por qué se guarda también el intento con éxito
--
-- Porque la cuenta de fallos se hace DESDE EL ÚLTIMO ÉXITO. Sin la fila del
-- acierto habría que borrar el historial al entrar, y borrar es peor: la serie
-- de fallos previos a un acceso correcto es justo lo que se quiere poder mirar
-- cuando alguien pregunta si a esa cuenta la estuvieron probando.
CREATE TABLE IF NOT EXISTS intento_acceso (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    -- El correo TAL COMO SE ESCRIBIÓ en el intento, exista o no ese usuario. No
    -- es un `usuario_id` a propósito: si solo se contaran los intentos contra
    -- cuentas reales, recibir o no el freno diría qué correos existen, y eso es
    -- justo lo que el login evita al dar el mismo error para todo.
    email    TEXT    NOT NULL,
    origen   TEXT    NOT NULL,
    momento  TEXT    NOT NULL,
    exito    INTEGER NOT NULL CHECK (exito IN (0, 1))
);

CREATE INDEX IF NOT EXISTS ix_intento_acceso
    ON intento_acceso (email, origen, momento);
