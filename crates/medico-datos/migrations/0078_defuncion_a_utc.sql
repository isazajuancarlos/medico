-- 0078: fecha_defuncion a UTC (fail-safe #1 de #758, H2 del concilio 2026-09-26).
--
-- Hasta ahora `fecha_defuncion` se guardaba CRUDA del <input type="datetime-local">
-- (hora LOCAL naive, sin zona). El fail-safe #1 la compara —con SQLite datetime()—
-- contra los actos de vida, que se guardan en UTC real (Utc::now()). Una hora local
-- leída como UTC corre la muerte 5 h: un acto de vida hasta 5 h ANTES de morir parecía
-- posterior y bloqueaba PARA SIEMPRE la supresión de un fallecido REAL (Ley 1581). El
-- camino de escritura (defuncion::certificar) ya sella la fecha a UTC; esto arregla las
-- filas que se guardaron crudas antes del arreglo.
--
-- SUPUESTO EXPLÍCITO (D20/D32): el prestador está en America/Bogota, UTC-05 FIJO (sin
-- horario de verano; Colombia no cambia la hora desde 1993). Un despliegue en otra zona
-- necesitaría OTRA migración con su desfase — no se generaliza a ciegas.
--
-- Alcance ACOTADO (D22): SOLO las filas en formato naive, que se reconocen por casar
-- exactamente `YYYY-MM-DDTHH:MM` o `...:SS` SIN sufijo de zona. Las que ya traen `Z` u
-- offset (fixtures, API, o ya normalizadas) NO casan estos GLOB y se dejan intactas;
-- reescribirlas correría una fecha ya correcta. +5 horas convierte la hora local -05 a UTC.
UPDATE defuncion
SET fecha_defuncion = strftime('%Y-%m-%dT%H:%M:%SZ', datetime(fecha_defuncion, '+5 hours'))
WHERE fecha_defuncion GLOB '????-??-??T??:??'
   OR fecha_defuncion GLOB '????-??-??T??:??:??';
