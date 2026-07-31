-- Un medicamento del maestro CUM puede estar INACTIVO (#77).
--
-- El export del INVIMA trae `estadocum` con dos valores, y el cargador los
-- ignoraba: de los 65 997 CUM del maestro, **15 046 están inactivos — el 22 %**.
-- Se cargaban todos por igual, así que el sistema aceptaba prescribir un
-- medicamento que ya no se puede facturar. El MUV lo rechaza, y el rechazo llega
-- semanas después como glosa.
--
-- NO SE BORRAN LOS INACTIVOS, SE MARCAN. Son dos preguntas distintas y hacen
-- falta las dos:
--   · «¿qué medicamento es este código?» — para leer una historia de hace dos
--     años donde se prescribió algo que entonces estaba vigente;
--   · «¿puedo prescribir esto HOY?» — que es la que decide si el RIPS pasa.
-- Borrarlos contestaría la segunda perdiendo la primera, y dejaría fórmulas
-- antiguas apuntando a un código sin nombre.
--
-- El defecto es 1 y no 0: esta columna se añade sobre un maestro ya cargado, y
-- lo que había cuando se escribió esto era el catálogo entero SIN distinguir. Un
-- defecto de 0 marcaría todo como inactivo hasta el siguiente arranque; el
-- sembrado corrige ambos casos en cuanto corre, pero entre la migración y el
-- sembrado la respuesta menos dañina es la permisiva —el estado real llega
-- inmediatamente después, en el mismo arranque.
ALTER TABLE medicamento
    ADD COLUMN activo INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0, 1));

CREATE INDEX IF NOT EXISTS ix_medicamento_activo ON medicamento (activo);
