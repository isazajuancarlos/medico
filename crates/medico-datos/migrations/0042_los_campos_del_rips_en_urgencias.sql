-- Los tres campos que el AU exige y `atencion_urgencias` no tenía (#67).
--
-- La 0016 creó la tabla con lo clínico —triage, motivo, diagnóstico de ingreso,
-- destino de salida— y sin lo que el RIPS pide del EGRESO. El generador del
-- Python los leía con `getattr(au, …, None)` sobre campos que en su modelo sí
-- existen; aquí no había ni columna.
--
-- Todas NULL y sin DEFAULT, por lo mismo que la 0041: un defecto dejaría al
-- recolector sin nada que anotar y el archivo afirmaría un dato que nadie
-- registró.

-- Causa o motivo de la atención (tabla del MUV, 21 a 49). Iba QUEMADA en «17»
-- en el Python, y el 17 ni siquiera está en la tabla de la 948. No es un
-- detalle: un accidente de trabajo, uno de tránsito y una lesión por agresión
-- llegan los tres por urgencias, son causas distintas y tienen responsables de
-- pago distintos.
ALTER TABLE atencion_urgencias ADD COLUMN causa_motivo TEXT;

-- Diagnóstico principal DE EGRESO (campo R05). La tabla guardaba el de ingreso
-- y las observaciones de egreso, pero no el diagnóstico con el que el paciente
-- sale, que es el que la 948 exige.
ALTER TABLE atencion_urgencias ADD COLUMN diagnostico_egreso TEXT;

-- Causa básica de muerte (campo R10). La 948: «se debe registrar si la
-- condición y destino del usuario al egreso es Paciente muerto». Va aparte del
-- diagnóstico de egreso a propósito — son dos datos distintos, y el certificado
-- de defunción es el que manda sobre este.
ALTER TABLE atencion_urgencias ADD COLUMN diagnostico_causa_muerte TEXT;
