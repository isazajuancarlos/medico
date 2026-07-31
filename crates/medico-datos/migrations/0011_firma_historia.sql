-- Firma de la historia clínica. Una historia FIRMADA es inmutable: en Colombia
-- (Resolución 1995/1999) la historia es un documento legal, y firmarla la cierra.
-- Por eso `actualizar` y `eliminar` la respetan y responden distinto —no es un
-- «no existe»— cuando el intento cae sobre una firmada.
--
-- Default 0 (borrador): la historia se redacta abierta y solo se cierra cuando el
-- médico firma. Nada se firma solo.

ALTER TABLE historia ADD COLUMN firmada INTEGER NOT NULL DEFAULT 0;
