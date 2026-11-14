-- La búsqueda de paciente plegaba tildes y caja con ~42 REPLACE por fila en CADA
-- consulta, así que no podía usar índice y costaba ~145 ms con 20 000 fichas
-- (#596). Se materializa el texto de búsqueda ya plegado en una columna que la
-- BASE mantiene por trigger —así el invariante vale para el escritor Rust, para
-- el SQL de las pruebas y para cualquier otra escritura, sin que nadie tenga que
-- acordarse—, y la búsqueda pasa a un LIKE plano sobre ella (~5,6 ms medidos).
--
-- El plegado es la GEMELA SQL de `paciente::plegar` (Rust) y su forma es
-- inmutable a partir de aquí (una migración no se reedita). Lo ata la prueba
-- `la_columna_plegado_dice_lo_mismo_que_plegar`, que compara la columna que ESTE
-- trigger escribe contra `plegar`: si las dos reglas se separan, salta.
--
-- El texto plegado es el nombre entero + el documento, unidos por espacios, para
-- que una aguja case contra el nombre O el documento con un solo LIKE. El orden
-- del plegado importa: primero las tildes en las DOS cajas —`LOWER` de SQLite
-- solo baja el ASCII y dejaría `Á` intacta— y `LOWER` al final para el resto.

ALTER TABLE paciente ADD COLUMN plegado TEXT NOT NULL DEFAULT '';

UPDATE paciente SET plegado =
    LOWER(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(nombre1 || COALESCE(' ' || nombre2, '') || ' ' || apellido1 || COALESCE(' ' || apellido2, '') || ' ' || numero_documento, 'á', 'a'), 'Á', 'a'), 'à', 'a'), 'À', 'a'), 'ä', 'a'), 'Ä', 'a'), 'â', 'a'), 'Â', 'a'), 'é', 'e'), 'É', 'e'), 'è', 'e'), 'È', 'e'), 'ë', 'e'), 'Ë', 'e'), 'ê', 'e'), 'Ê', 'e'), 'í', 'i'), 'Í', 'i'), 'ì', 'i'), 'Ì', 'i'), 'ï', 'i'), 'Ï', 'i'), 'î', 'i'), 'Î', 'i'), 'ó', 'o'), 'Ó', 'o'), 'ò', 'o'), 'Ò', 'o'), 'ö', 'o'), 'Ö', 'o'), 'ô', 'o'), 'Ô', 'o'), 'ú', 'u'), 'Ú', 'u'), 'ù', 'u'), 'Ù', 'u'), 'ü', 'u'), 'Ü', 'u'), 'û', 'u'), 'Û', 'u'), 'ñ', 'n'), 'Ñ', 'n'));

CREATE TRIGGER paciente_plegado_al_insertar
AFTER INSERT ON paciente
BEGIN
    UPDATE paciente SET plegado =
        LOWER(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(nombre1 || COALESCE(' ' || nombre2, '') || ' ' || apellido1 || COALESCE(' ' || apellido2, '') || ' ' || numero_documento, 'á', 'a'), 'Á', 'a'), 'à', 'a'), 'À', 'a'), 'ä', 'a'), 'Ä', 'a'), 'â', 'a'), 'Â', 'a'), 'é', 'e'), 'É', 'e'), 'è', 'e'), 'È', 'e'), 'ë', 'e'), 'Ë', 'e'), 'ê', 'e'), 'Ê', 'e'), 'í', 'i'), 'Í', 'i'), 'ì', 'i'), 'Ì', 'i'), 'ï', 'i'), 'Ï', 'i'), 'î', 'i'), 'Î', 'i'), 'ó', 'o'), 'Ó', 'o'), 'ò', 'o'), 'Ò', 'o'), 'ö', 'o'), 'Ö', 'o'), 'ô', 'o'), 'Ô', 'o'), 'ú', 'u'), 'Ú', 'u'), 'ù', 'u'), 'Ù', 'u'), 'ü', 'u'), 'Ü', 'u'), 'û', 'u'), 'Û', 'u'), 'ñ', 'n'), 'Ñ', 'n'))
    WHERE id = NEW.id;
END;

-- `OF` las columnas de nombre y documento: solo se vuelve a plegar cuando cambia
-- lo que se busca, y como el trigger escribe `plegado` —que NO está en la lista—
-- no se dispara a sí mismo aunque los triggers recursivos estuvieran activos.
CREATE TRIGGER paciente_plegado_al_actualizar
AFTER UPDATE OF nombre1, nombre2, apellido1, apellido2, numero_documento ON paciente
BEGIN
    UPDATE paciente SET plegado =
        LOWER(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(nombre1 || COALESCE(' ' || nombre2, '') || ' ' || apellido1 || COALESCE(' ' || apellido2, '') || ' ' || numero_documento, 'á', 'a'), 'Á', 'a'), 'à', 'a'), 'À', 'a'), 'ä', 'a'), 'Ä', 'a'), 'â', 'a'), 'Â', 'a'), 'é', 'e'), 'É', 'e'), 'è', 'e'), 'È', 'e'), 'ë', 'e'), 'Ë', 'e'), 'ê', 'e'), 'Ê', 'e'), 'í', 'i'), 'Í', 'i'), 'ì', 'i'), 'Ì', 'i'), 'ï', 'i'), 'Ï', 'i'), 'î', 'i'), 'Î', 'i'), 'ó', 'o'), 'Ó', 'o'), 'ò', 'o'), 'Ò', 'o'), 'ö', 'o'), 'Ö', 'o'), 'ô', 'o'), 'Ô', 'o'), 'ú', 'u'), 'Ú', 'u'), 'ù', 'u'), 'Ù', 'u'), 'ü', 'u'), 'Ü', 'u'), 'û', 'u'), 'Û', 'u'), 'ñ', 'n'), 'Ñ', 'n'))
    WHERE id = NEW.id;
END;
