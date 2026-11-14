-- 0079: vacuna_aplicada.fecha y control_pyp.fecha a UTC (R1 del concilio 2026-09-26).
--
-- El fail-safe #1 de #758 (retención, Ley 1581) bloquea la supresión si el paciente
-- tiene un acto de vida POSTERIOR a la defunción. La 0078 selló `fecha_defuncion` a
-- UTC para que esa comparación fuera correcta contra los actos que el servidor sella
-- en UTC real (`Utc::now()`). Pero DOS tablas de `PRUEBA_DE_VIDA` guardaban su `fecha`
-- CRUDA del `<input type="datetime-local">` (hora local naive): `vacuna_aplicada` y
-- `control_pyp` (módulo P&P). Tras la 0078, comparar una vacuna/control naive contra
-- una muerte en UTC volvió a descuadrar los relojes AL REVÉS: un acto hasta 5 h DESPUÉS
-- de la muerte caía «antes» y NO bloqueaba → paciente VIVO elegible para purga
-- irreversible (fail-open; y la vacunación-sin-historia es justo la población que el
-- fail-safe protege). El camino de escritura (`pyp::aplicar`/`controlar`) ya sella la
-- fecha a UTC; esto arregla las filas guardadas crudas antes del arreglo.
--
-- SUPUESTO EXPLÍCITO (D20/D32), idéntico a la 0078: prestador en America/Bogota, UTC-05
-- FIJO (Colombia no cambia la hora). Otra zona necesitaría OTRA migración con su desfase.
--
-- Alcance ACOTADO (D22): SOLO las filas en formato naive (`YYYY-MM-DDTHH:MM[:SS]` sin
-- sufijo de zona). Las que ya traen `Z`/offset (fixtures, ya normalizadas) NO casan los
-- GLOB y se dejan intactas. +5 horas convierte la hora local -05 a UTC. Idempotente: una
-- fila ya con `Z` es más larga y no vuelve a casar en una segunda corrida.
UPDATE vacuna_aplicada
SET fecha = strftime('%Y-%m-%dT%H:%M:%SZ', datetime(fecha, '+5 hours'))
WHERE fecha GLOB '????-??-??T??:??'
   OR fecha GLOB '????-??-??T??:??:??';

UPDATE control_pyp
SET fecha = strftime('%Y-%m-%dT%H:%M:%SZ', datetime(fecha, '+5 hours'))
WHERE fecha GLOB '????-??-??T??:??'
   OR fecha GLOB '????-??-??T??:??:??';
