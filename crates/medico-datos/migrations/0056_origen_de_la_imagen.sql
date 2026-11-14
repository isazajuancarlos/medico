-- De dónde viene un archivo de `imagen_diagnostica`. Son DOS cosas distintas
-- guardadas en la misma tabla, y confundirlas costó dos defectos el 2026-08-10.
--
-- # Los dos orígenes
--
-- `adjunto_historia` — el archivo que el médico añade a la historia mientras la
--   escribe: una radiografía que trae el paciente, un informe externo. **Es
--   contenido del documento**: entra en lo que se firma, y no se puede añadir ni
--   quitar de una historia ya firmada.
--
-- `resultado_imagenologia` — la placa que el servicio de imagenología sube
--   DESPUÉS, contra una orden. **No es contenido del documento firmado**: llega
--   por el flujo operativo posterior, exactamente igual que cerrar una orden de
--   laboratorio, y lo normal es que la consulta esté firmada cuando llega.
--
-- # Los dos defectos que esta columna arregla
--
-- Al cerrar «una historia firmada no admite que se le cuelgue nada» (mismo día),
-- `imagen::registrar` empezó a responder 403 a las DOS, y con eso el radiólogo
-- dejaba de poder adjuntar la placa a su informe. Es el mismo caso que
-- `orden::cambiar_estado`, que sí se dejó exento con su argumento (directiva 21);
-- aquí no se razonó y se cerró de más.
--
-- Y en la otra dirección: si la placa posterior entrara en el resumen firmado,
-- subirla ROMPERÍA la firma de una historia firmada legítimamente. Una
-- verificación que falla por una operación normal es ruido, y el ruido enseña a
-- ignorar la verificación — que es justo lo que la firma no se puede permitir.
--
-- # El defecto es `adjunto_historia`, y las filas VIEJAS se reetiquetan
--
-- CORREGIDO EL MISMO DÍA, y la corrección es la lección: la primera versión de
-- este archivo decía que las filas existentes «quedan descritas por lo que de
-- verdad son» porque el adjunto de la historia era «el único camino
-- documentado». **Era falso, y bastaba mirarlo**: la placa del radiólogo ya
-- escribía en esta misma tabla antes de esta rama, por el mismo
-- `guardar_imagen_de` (`691e003:crates/medico-api/src/rutas.rs:3404`). Con el
-- defecto a secas, toda placa ya subida quedaba etiquetada como contenido del
-- documento firmado — justo al revés de lo que esta columna existe para
-- distinguir, y en las únicas filas que después no se pueden reclasificar desde
-- el producto.
--
-- El relleno es posible porque las placas dejan rastro: `informe_imagen`
-- guarda en `imagenes_paths` un array JSON con `/api/imagenes/{id}/archivo` por
-- cada una, así que toda imagen citada ahí es un `resultado_imagenologia`. Se
-- reetiquetan con un `LIKE` sobre esa lista — no es elegante, pero es exacto: la
-- ruta lleva el id entre dos separadores fijos, así que `%/api/imagenes/7/%` no
-- puede casar con la 17 ni con la 71.
--
-- Medido antes de escribirlo: la base de la demostración tiene CERO filas en
-- esta tabla, así que hoy este `UPDATE` no toca nada. Va igual, porque una
-- migración que solo funciona sobre una base vacía no es una migración.
ALTER TABLE imagen_diagnostica
    ADD COLUMN origen TEXT NOT NULL DEFAULT 'adjunto_historia';

UPDATE imagen_diagnostica
   SET origen = 'resultado_imagenologia'
 WHERE EXISTS (
     SELECT 1 FROM informe_imagen i
      WHERE i.imagenes_paths LIKE '%/api/imagenes/' || imagen_diagnostica.id || '/archivo%'
 );

-- SQLite no admite añadir un CHECK con ALTER TABLE, así que el vocabulario lo
-- sostiene el tipo `imagen::Origen` en Rust —que es por donde pasan las dos
-- escrituras— y este índice deja constancia de los valores esperados.
CREATE INDEX IF NOT EXISTS ix_imagen_origen ON imagen_diagnostica (historia_id, origen);
