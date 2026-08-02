-- La finalidad de la atención vuelve a ser un dato y deja de ser un relleno.
--
-- MEDIDO EL 2026-08-02, y son tres cosas distintas:
--
-- 1. La tabla del sistema es la ANTERIOR a la Resolución 948. `FinalidadConsulta`
--    tiene nueve valores (02..10) y la tabla del MUV
--    —`REFs_FinalidadTecnologiaSalud`, 34 filas— va del **11 al 44**. Ninguno de
--    los nueve está en ella.
--
-- 2. El `10` («NO APLICA») no lo eligió nadie: lo pone el defecto, aquí con
--    `finalidad TEXT NOT NULL DEFAULT '10'` y en el código con dos
--    `unwrap_or("10")`.
--
-- 3. **Ninguna pantalla ofrece el campo.** `finalidad` no aparece en un solo
--    formulario del frontend, así que ningún médico ha elegido una nunca. Toda
--    historia existente lleva el defecto, sin excepción.
--
-- El 3 es lo que hace segura esta migración: no hay ninguna elección clínica que
-- respetar. `'10'` no significa «no aplica según el médico»; significa «aquí no
-- había nada y el sistema lo tapó».

-- El relleno pasa a ser lo que siempre fue: un hueco. Cadena vacía y no `NULL`
-- porque la columna es NOT NULL y cambiarlo en SQLite exige reconstruir una
-- tabla de ochenta columnas — y para el recolector del RIPS son lo mismo: el
-- espacio en blanco YA cuenta como ausente (`Recolector::vacio`), así que la
-- consulta sale en el informe de cierre pidiendo la finalidad, que es
-- exactamente lo que tiene que pasar.
UPDATE historia SET finalidad = '' WHERE finalidad = '10';

-- Y NO ES SOLO LA FINALIDAD. Al medirla salió que `causa_externa` tiene el mismo
-- problema y de la misma forma: el enum del sistema (`CausaExterna`, 13 valores
-- del 01 al 15) es la tabla anterior, y la del MUV
-- —`REFs_CausaMotivoAtencion`— va del **21 al 49**. Ninguno de los trece está.
-- El defecto era `'13'` (ENFERMEDAD GENERAL), que en la tabla nueva es el 38.
--
-- No se traduce 13 → 38, aunque el nombre coincida EXACTAMENTE. Traducir un
-- valor que nadie eligió no lo convierte en una afirmación: lo convierte en un
-- código válido que el MUV acepta y que sigue sin haber dicho nadie, que es peor
-- que el actual porque ya no lo detecta nada.
UPDATE historia SET causa_externa = '' WHERE causa_externa = '13';

-- `tipo_diagnostico` va igual, y aquí con una diferencia que hay que decir: **no
-- se pudo comprobar** si el `'01'` es válido, porque entre los ocho maestros
-- extraídos del validador no hay tabla de tipo de diagnóstico. Lo que sí consta
-- es que tampoco lo eligió nadie —el campo no está en ninguna pantalla— y eso
-- basta: el problema de estos tres campos no es que el código sea inválido, es
-- que el sistema afirma en nombre del médico. Que además dos de ellos sean
-- inválidos es la consecuencia que lo destapó.
UPDATE historia SET tipo_diagnostico = '' WHERE tipo_diagnostico = '01';

-- LOS OTROS OCHO NO SE CONVIERTEN, y no por pereza.
--
-- Hoy no existe ninguna fila con ellos (nadie ha podido elegirlos), pero la
-- regla se escribe igual porque el día que se importen las historias del Visual
-- FoxPro van a llegar. Solo dos tendrían equivalencia limpia —06 «atención del
-- parto» → 25 ATENCION DEL PARTO Y PUERPERIO, y 08 «planificación familiar» →
-- 19 PLANIFICACION FAMILIAR Y ANTICONCEPCION—; las otras seis no:
--
--   07 atención del recién nacido → ¿26 «cuidado del recién nacido» (que NO
--      aplica a consultas) o 27 «seguimiento del recién nacido»?
--   05 enfermedad profesional     → ¿13 «detección temprana de enfermedad
--      laboral» o 16 «tratamiento»? Detectar y tratar no son lo mismo.
--   02, 03, 09 detección de alteraciones del joven / del adulto / del
--      crecimiento → la 948 rehizo esos programas; ¿11 «valoración integral» o
--      12 «detección temprana de enfermedad general»?
--
-- Elegir por el médico en un `UPDATE` masivo es el mismo error que el `'10'`,
-- solo que más difícil de descubrir: quedaría un código válido que el MUV acepta
-- y que nadie afirmó. Se dejan como están, y el generador del RIPS los saca en
-- el informe de cierre diciendo que son de la tabla anterior y hay que
-- convertirlos (`medico-dominio/src/rips/ac.rs`).

-- El `DEFAULT '10'` del DDL sigue ahí y no se puede quitar sin reconstruir la
-- tabla, pero deja de alcanzarse: `historia::crear` y `historia::actualizar`
-- son los únicos que insertan, los dos escriben la columna siempre, y desde
-- este cambio escriben `''` en vez de `'10'` cuando no viene ninguna. Lo ata la
-- prueba `sin_finalidad_se_guarda_el_hueco_y_no_el_diez`, que falla si alguien
-- vuelve a poner el relleno.
