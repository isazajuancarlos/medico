-- Los datos de contacto que el VFP capturaba de sus TERCEROS (#752).
--
-- `terceros` NO era un módulo que faltara: es esta tabla. Leído el `TERCEROS.DBF`
-- vivo del prestador —solo la cabecera y una muestra de nombres, nunca datos de
-- paciente—, sus filas son EPS y entidades responsables de pago: `E.P.S. SUSALUD`
-- con `CODIGO_MIN = EPS010`, `E.S.S. COOPSOSALUD` con `ESS014`. O sea `entidad`,
-- que medico tiene portada desde la 0044 con su CRUD y su pantalla.
--
-- La búsqueda anterior no lo vio porque buscaba la palabra «terceros», y en
-- medico se llama `entidad`; el único acierto era `Tercero`, que es un ROL de
-- usuario. Buscar por nombre habría dado por portado lo que no lo estaba, y aquí
-- hizo lo contrario: dio por sin portar lo que ya estaba.
--
-- Lo que SÍ faltaba son los seis campos de contacto y el código interno, que el
-- prestador usa hoy y que una migración sin ellos le haría perder.
--
-- # Todos NULL y ninguno con DEFAULT
--
-- Misma lección que la 0041: un `DEFAULT ''` haría que un contacto vacío se
-- viera igual que uno no registrado, y no hay forma de distinguirlos después.
--
-- # `codigo_interno` no es la llave
--
-- El VFP indexaba por un CODIGO de seis dígitos rellenado con ceros a la
-- izquierda (`000611`), único por prestador. Aquí la identidad la da el `id`, y
-- el NIT ya es `UNIQUE`; el código entra para que la importación pueda casar sus
-- filas y para que el usuario reconozca la entidad por el número que lleva años
-- tecleando. No se le pone `UNIQUE`: dos prestadores distintos pueden haber
-- numerado sus terceros de forma distinta, y `entidad` es una tabla NACIONAL.
ALTER TABLE entidad ADD COLUMN codigo_interno TEXT;
ALTER TABLE entidad ADD COLUMN direccion      TEXT;
ALTER TABLE entidad ADD COLUMN telefono       TEXT;
ALTER TABLE entidad ADD COLUMN fax            TEXT;
ALTER TABLE entidad ADD COLUMN contacto       TEXT;
ALTER TABLE entidad ADD COLUMN email          TEXT;
-- El municipio va como código DIVIPOLA de cinco dígitos, no como texto libre:
-- `divipola.rs` ya existe y el cero a la izquierda es significativo (148
-- municipios empiezan por cero, Medellín entre ellos).
ALTER TABLE entidad ADD COLUMN cod_municipio  TEXT;
