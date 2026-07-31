-- Qué módulos ve cada institución (#65, punto 6).
--
-- Es la pieza que permite vender el MISMO binario a un consultorio, una IPS, una
-- clínica y un hospital: lo que cambia no es el código, es la licencia. Sin
-- esto, un consultorio de consulta general ve cuarenta opciones de las que
-- treinta y cinco no le aplican, y un hospital no tiene forma de saber cuáles de
-- las suyas están realmente implementadas.
--
-- El defecto son los tres que TODO prestador usa. No es una cifra de compromiso
-- ni «los que caben en la pantalla»: sin pacientes no hay historia, sin historia
-- no hay RIPS y sin RIPS no hay pago. Una institución nueva arranca pudiendo
-- trabajar, y el administrador añade lo demás — al revés (arrancar con todo) el
-- cliente no distingue lo que compró de lo que no.
--
-- Va como JSON y no como tabla de unión a propósito: se lee ENTERO en cada
-- petición de menú y no se consulta por partes nunca, así que una tabla añadiría
-- un join para responder siempre la misma pregunta. Si algún día hace falta
-- saber «qué instituciones tienen FARMACIA», entonces sí toca normalizarlo.
ALTER TABLE institucion
    ADD COLUMN modulos_habilitados TEXT NOT NULL
    DEFAULT '["CITAS","PACIENTES","HISTORIA"]';
