-- Completa el paciente a la forma del producto: campos que el registro clínico
-- y RIPS necesitan (contacto, tipo de usuario, país, zona, entidad).
--
-- Todos ANULABLES a propósito: una fila anterior no los traía, y NULL es «no
-- capturado», no un valor por defecto inventado. El formulario los llena al
-- crear/editar; una fila vieja los muestra en blanco hasta que se editen.

ALTER TABLE paciente ADD COLUMN celular      TEXT;
ALTER TABLE paciente ADD COLUMN email        TEXT;
ALTER TABLE paciente ADD COLUMN direccion    TEXT;
-- Régimen/tipo de usuario RIPS (01 contributivo, 02 subsidiado, …).
ALTER TABLE paciente ADD COLUMN tipo_usuario TEXT;
-- País de residencia (170 = Colombia). Zona: 01 urbana, 02 rural.
ALTER TABLE paciente ADD COLUMN cod_pais     TEXT;
ALTER TABLE paciente ADD COLUMN zona         TEXT;
ALTER TABLE paciente ADD COLUMN entidad_id   INTEGER;
