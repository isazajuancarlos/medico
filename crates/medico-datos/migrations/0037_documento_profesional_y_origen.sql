-- Lo que le faltaba al esquema para poder emitir el RIPS (#67/#250).
--
-- =========================================================================
-- EL DOCUMENTO DEL PROFESIONAL QUE ATENDIÓ (campos P11/P12)
-- =========================================================================
--
-- La tabla `usuario` no tiene documento de identidad. Ninguno de los cinco
-- documentos del RIPS se puede emitir bien sin él: la Resolución 948 (Documento
-- Técnico 1) define P11/P12 como «tipo y número de documento de identificación
-- del profesional de la salud que ordenó o realizó el procedimiento».
--
-- ES EL CAMPO QUE YA SE EQUIVOCÓ UNA VEZ, y por eso se porta con su historia
-- pegada. Hasta el 2026-07-26 el backend Python mandaba ahí el documento del
-- PACIENTE. Lo destapó el MUV rechazando las atenciones de menores con RVG01
-- («valores permitidos CC,CE,CD,PA,SC,PE,DE,PT, valor enviado: RC»), y lo grave
-- no fue el rechazo: **las atenciones de adultos PASABAN**, llevando el
-- documento del paciente como si fuera el del médico. Un dato falso que afirma
-- quién realizó un acto médico, y que no se habría notado nunca.
--
-- VAN NULLABLE, y no es descuido: los usuarios que ya existen no lo traen, y
-- ponerles un valor sería exactamente lo que este campo enseñó a no hacer. Sin
-- él, el recolector anota la falta y el cierre del mes la muestra. **No hay
-- supuesto posible**: inventar el documento de un profesional es atribuirle un
-- acto médico a alguien.
--
-- El CHECK enumera los ocho tipos que la 948 admite, y solo esos ocho. Se
-- rechaza lo IMPOSIBLE: `RC` (registro civil) es el documento de un menor de
-- edad, y un menor de edad no firma una atención. No entra `NULL` en la lista
-- porque `CHECK` con NULL no se evalúa — la columna sigue admitiendo ausencia.

ALTER TABLE usuario ADD COLUMN tipo_documento TEXT
    CHECK (tipo_documento IS NULL OR tipo_documento IN
        ('CC','CE','CD','PA','SC','PE','DE','PT'));

ALTER TABLE usuario ADD COLUMN numero_documento TEXT;

-- Dos profesionales no comparten documento. Parcial, porque la mayoría de las
-- filas lo tendrán en NULL durante mucho tiempo y NULL no colisiona.
CREATE UNIQUE INDEX ux_usuario_documento
    ON usuario (tipo_documento, numero_documento)
    WHERE tipo_documento IS NOT NULL AND numero_documento IS NOT NULL;

-- =========================================================================
-- EL PAÍS DE ORIGEN DEL USUARIO (campo U08 del bloque de usuarios)
-- =========================================================================
--
-- `paciente` ya tiene `tipo_usuario`, `cod_pais`, `zona` y `cod_municipio`; le
-- falta este. Es el único del bloque de identificación que no estaba.
--
-- NULLABLE por lo mismo que los demás: el Python declaraba el supuesto «170»
-- (Colombia) y lo emitía sin decirlo, con lo que todo paciente sin dato quedaba
-- reportado como colombiano de origen. En una IPS de frontera eso no es un
-- detalle. Aquí el supuesto se declara y solo sale si el llamante lo pide.
ALTER TABLE paciente ADD COLUMN cod_pais_origen TEXT;
