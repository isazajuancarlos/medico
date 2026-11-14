-- Lo que el AH exige de una hospitalización y la tabla no tenía (#67).
--
-- La 0017 la creó con lo clínico —cama, médico tratante, vía de ingreso, estado,
-- diagnósticos de ingreso y egreso, resumen— y sin los dos campos que el RIPS
-- pide y de los que no se puede deducir nada.
--
-- NULL y sin DEFAULT, por lo mismo que la 0041 y la 0042.

-- Causa o motivo de la atención (tabla del MUV, 21 a 49). El Python la llevaba
-- QUEMADA en «13» —enfermedad general— y con eso **ninguna hospitalización por
-- accidente de tránsito se cobraba al SOAT**. El 13 además es de la tabla
-- anterior a la 948, que va del 21 al 49: ni siquiera existe.
ALTER TABLE hospitalizacion ADD COLUMN causa_motivo TEXT;

-- Causa básica de muerte. La 948 la exige cuando la condición de egreso es
-- «paciente muerto», y el estado `DEFUNCION` ya dice cuándo pasa — pero el
-- diagnóstico no se deduce del estado: se registra. El Python emitía `null` fijo
-- en este campo, así que una defunción hospitalaria salía sin su causa.
ALTER TABLE hospitalizacion ADD COLUMN diagnostico_causa_muerte TEXT;
