-- Los campos que el RIPS exige de un PROCEDIMIENTO y de un MEDICAMENTO (#67).
--
-- El AC ya los tenía en `historia` desde la 0005; el AP y el AM no tenían dónde
-- leerlos, así que el generador del Python emitía constantes: `vrServicio: 0`,
-- `conceptoRecaudo: "05"`, `viaIngresoServicioSalud: "01"`. Una constante en el
-- generador es indistinguible de un dato real cuando se mira el archivo — y de
-- `vrServicio` depende lo que se cobra.
--
-- Son los mismos que declara `CamposRips` en `backend/models.py`, y por eso se
-- añaden con los mismos nombres: los dos sistemas tienen que poder generar el
-- mismo archivo mientras convivan.
--
-- # Todos NULL, y ninguno con DEFAULT
--
-- Es la lección de la 0038, que costó dos campos del RIPS saliendo con un código
-- rechazable en toda atención. Un `DEFAULT '01'` aquí haría que el recolector
-- —que solo ve AUSENCIAS— no tuviera nada que anotar, y el archivo saldría
-- afirmando una vía de ingreso que nadie eligió. NULL significa «no se
-- registró», el generador se niega a inventarlo, y sale en el informe de cierre
-- con el id de la fila donde hay que ir a corregirlo.
--
-- # Lo que esto NO es
--
-- No es una característica nueva: hoy ninguna pantalla captura estos campos, y
-- por eso el AP y el AM se generan con faltas hasta que alguien las capture o
-- las llene el tarifador. La alternativa —no crear las columnas y devolver
-- siempre `None`— daría el mismo archivo hoy y dejaría el port sin sitio donde
-- guardar el dato el día que se capture, con una segunda migración por delante.

-- Procedimientos (AP). `modalidadGrupoServicioTecSal` NO va aquí: se deriva del
-- tipo de orden, que ya es una columna con CHECK desde la 0027.
ALTER TABLE orden ADD COLUMN via_ingreso_servicio   TEXT;
ALTER TABLE orden ADD COLUMN grupo_servicios        TEXT;
ALTER TABLE orden ADD COLUMN cod_servicio           INTEGER;
ALTER TABLE orden ADD COLUMN valor_servicio         INTEGER;
ALTER TABLE orden ADD COLUMN concepto_recaudo       TEXT;
ALTER TABLE orden ADD COLUMN valor_pago_moderador   INTEGER;
ALTER TABLE orden ADD COLUMN num_fev_pago_moderador TEXT;

-- Medicamentos (AM). `dias_tratamiento` ya existe desde la 0006, con esta misma
-- razón escrita al lado.
ALTER TABLE formula ADD COLUMN tipo_medicamento       TEXT;
ALTER TABLE formula ADD COLUMN valor_unitario         INTEGER;
ALTER TABLE formula ADD COLUMN valor_servicio         INTEGER;
ALTER TABLE formula ADD COLUMN concepto_recaudo       TEXT;
ALTER TABLE formula ADD COLUMN valor_pago_moderador   INTEGER;
ALTER TABLE formula ADD COLUMN num_fev_pago_moderador TEXT;
