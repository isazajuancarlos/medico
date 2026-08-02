-- Los campos que el RIPS exige y el esquema no tenía dónde guardar (#250).
--
-- Salieron al portar el AC: son los tres que el documento pide, que nadie puede
-- inventar, y que hasta hoy no existían en ninguna tabla. Sin ellos el archivo
-- no se puede emitir — y emitirlo con relleno es lo que este port existe para
-- no repetir.
--
-- TODAS VAN NULLABLE Y SIN DEFECTO, y eso es la decisión, no un descuido. Un
-- `NOT NULL DEFAULT '...'` convertiría el hueco en un dato con la forma
-- correcta que nadie afirmó, y el recolector dejaría de verlo: es exactamente
-- lo que pasó con `historia.finalidad DEFAULT '10'`, que pone en toda consulta
-- un código que el MUV no reconoce sin que nada falle. Vacío, el informe de
-- cierre lo dice y alguien lo corrige.

-- El `codPrestador` de CADA línea de servicio: el código de habilitación del
-- prestador en el REPS. No es el NIT —que ya está en `institucion.nit` y es el
-- `numDocumentoIdObligado` del documento—: son dos identificadores distintos de
-- la misma institución, y confundirlos hace rechazar el archivo entero.
ALTER TABLE institucion ADD COLUMN codigo_habilitacion TEXT;

-- P11/P12: el documento del PROFESIONAL que atendió.
--
-- Es el campo que ya causó el fallo en el Python: iba el documento del PACIENTE.
-- El MUV lo destapó rechazando las atenciones de menores («valores permitidos
-- CC,CE,CD,PA,SC,PE,DE,PT, valor enviado: RC»), pero lo grave era lo otro: las
-- de adultos PASABAN, atribuyendo el acto médico a un documento que no era el
-- del médico. En Rust el campo no existía, así que la única salida honesta era
-- crearlo — no reutilizar el del paciente «mientras tanto».
ALTER TABLE usuario ADD COLUMN tipo_documento   TEXT;
ALTER TABLE usuario ADD COLUMN numero_documento TEXT;

-- `codPaisOrigen` del bloque de identificación del usuario. Distinto de
-- `cod_pais` (residencia), que ya existe desde 0003: un migrante residente en
-- Colombia tiene 170 en residencia y otro en origen, y esa diferencia es
-- justamente lo que el campo reporta.
ALTER TABLE paciente ADD COLUMN cod_pais_origen TEXT;
