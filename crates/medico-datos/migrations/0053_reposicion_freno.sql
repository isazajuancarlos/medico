-- Reposición administrativa del freno (#539), sin borrar la evidencia.
--
-- # El defecto que cierra
--
-- «Desbloquear» hacía `DELETE FROM intento_acceso WHERE email=? AND exito=0`, y
-- `intento_acceso` es el ÚNICO registro que existe de un ensayo de contraseñas
-- —la bitácora `acceso` anota lo que OCURRIÓ, no lo que se INTENTÓ—. Así el botón
-- que un admin pulsa para desatascar a un médico borraba de paso la única prueba
-- de que a esa cuenta la estuvieron probando. La 0022 ya guardaba el acierto
-- justamente para no tener que borrar esa serie (ver su cabecera); el DELETE la
-- contradecía.
--
-- # Por qué una tabla aparte y no una fila en `intento_acceso`
--
-- Una reposición NO es un intento de acceso: no tiene origen, ni éxito, ni la
-- tecleó nadie contra el login. Meterla en `intento_acceso` obligaría a inventar
-- un origen y a darle un `exito` que no significa lo que significa en esa tabla
-- (directiva 32: un valor con dos sentidos son dos campos). Va en su sitio.
--
-- # Cómo la usa la regla
--
-- `guaca::freno::evaluar` cuenta los fallos POSTERIORES al último acierto. La
-- reposición se le entrega como un acierto sintético en ese instante: mueve el
-- punto de inicio del conteo hacia adelante —limpia la espera— sin tocar ninguna
-- fila de fallos. Y como es un acierto más para la regla, los fallos que lleguen
-- DESPUÉS vuelven a contar: una reposición no es un pase permanente, y una más
-- vieja que la retención (24 h) no tiene efecto, igual que un acierto viejo.
--
-- Una fila por correo: solo importa la reposición MÁS RECIENTE. `ON CONFLICT`
-- la reemplaza, así que la tabla queda acotada al número de cuentas, sin poda.
CREATE TABLE IF NOT EXISTS reposicion_freno (
    email   TEXT NOT NULL PRIMARY KEY,
    momento TEXT NOT NULL
);
