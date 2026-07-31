-- Un profesional no puede estar en dos citas a la vez.
--
-- La migración 0035 trajo la agenda, y el PR que la introdujo afirmó que con
-- ella «nada podía agendar sobre una hora ocupada». No era cierto: `cita::crear`
-- no consulta `bloque_horario` ni `bloqueo_agenda`, y `cita` no tenía ningún
-- índice sobre `(medico_id, fecha, hora)`. `GET /api/agenda/turnos` pinta bien
-- la pantalla y nada impide llamar al API directamente — ni impide que dos
-- peticiones simultáneas pasen las dos por el hueco. Lo señaló una revisión de
-- seguridad independiente el 2026-07-31.
--
-- # Lo que se cierra, y lo que NO
--
-- Se cierra lo IMPOSIBLE: la misma persona atendiendo a dos pacientes en el
-- mismo instante. No hay prestador legítimo que necesite eso.
--
-- NO se cierra agendar FUERA de la agenda. La agenda es un plan, no una ley: la
-- IPS que encaja una urgencia a las 6 de la tarde, o al paciente que llegó de
-- otro municipio, está operando bien. Rechazarlo sería prohibir lo inusual, y
-- convertiría el módulo en el estorbo que la recepción aprende a saltarse. Lo
-- que el sistema hace ahí es INFORMAR —el turno sale con su motivo— y dejar
-- decidir a quien coordina.
--
-- # Por qué un índice y no una comprobación en el código
--
-- La comprobación que se escribe en `crear` es la que falta en la ruta nueva, y
-- además no sobrevive a la concurrencia: dos peticiones leen «libre» y las dos
-- insertan. El índice no se olvida y no tiene ventana.
--
-- # Las tres condiciones del índice parcial
--
-- `medico_id IS NOT NULL`: agendar sin profesional asignado es un estado
-- legítimo —se agenda y se asigna después—, y muchas citas sin asignar comparten
-- hora sin que eso signifique nada.
--
-- `estado <> 'CANCELADA'`: una cita cancelada libera su hora. Si contara, el
-- paciente que cancela dejaría el turno inutilizable para siempre.

CREATE UNIQUE INDEX ux_cita_profesional_hora
    ON cita (medico_id, fecha, hora)
    WHERE medico_id IS NOT NULL AND estado <> 'CANCELADA';
