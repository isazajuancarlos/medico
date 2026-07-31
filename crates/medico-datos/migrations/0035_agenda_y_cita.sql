-- La agenda del médico, y las dos columnas que la cita no tenía.
--
-- =========================================================================
-- PARTE 1 — LA CITA DEJA DE INVENTARSE EL TIPO Y LA DURACIÓN (#238)
-- =========================================================================
--
-- `cita.service.ts:47-52` rellenaba `tipo: 'Primera Vez'` y
-- `duracion_minutos: 30` en el navegador «porque la tabla de Rust no los
-- tiene». Están comentados como valores de presentación, pero en la pantalla
-- son indistinguibles de un dato registrado: quien abre la agenda lee que ese
-- paciente viene por primera vez, y eso no lo dijo nadie. Es el mismo defecto
-- que el `cod_consulta_cups="890201"` que ya se retiró.
--
-- La interfaz YA los recoge —`citas.component.ts` tiene el selector de tipo y el
-- campo de duración—, así que el backend los estaba tirando a la basura. Se
-- añaden las columnas.
--
-- LAS DOS VAN NULLABLE, y esto es lo que importa: las citas que ya existen no
-- traen el dato, y ponerles un DEFAULT sería exactamente lo que se está
-- corrigiendo — cambiar «el navegador se lo inventa» por «la base se lo
-- inventa». NULL significa «no se sabe», la pantalla lo muestra vacío, y quien
-- las cree de ahora en adelante manda el valor.
--
-- EL VOCABULARIO ES EL DE `estado`, EN ESTA MISMA TABLA. Es decir
-- PRIMERA_VEZ / CONTROL / URGENCIA, no las cadenas «Primera Vez» y
-- «Control / Revisión» que usan el Python y la interfaz. Dos vocabularios
-- escribiendo una misma tabla es lo que dejó muerta la pantalla de laboratorio
-- (ver 0027), y aquí ya hay uno establecido: `estado` guarda PROGRAMADA y el
-- frontend lo traduce con `A_ANGULAR`. El texto que ve el usuario se traduce en
-- la capa que lo pinta; no se guarda en la columna.

ALTER TABLE cita ADD COLUMN tipo TEXT
    CHECK (tipo IS NULL OR tipo IN ('PRIMERA_VEZ','CONTROL','URGENCIA'));

-- Minutos. Se rechaza lo IMPOSIBLE y nada más: cero o negativo no es una
-- duración, y más de un día no cabe en una cita de una agenda diaria. Lo
-- inusual —una junta médica de tres horas, un procedimiento largo— pasa.
ALTER TABLE cita ADD COLUMN duracion_minutos INTEGER
    CHECK (duracion_minutos IS NULL OR (duracion_minutos > 0 AND duracion_minutos <= 1440));

-- Cuándo PIDIÓ el paciente la cita (YYYY-MM-DD). No es lo mismo que cuándo se
-- la dieron, y esa diferencia es justamente el indicador de OPORTUNIDAD de la
-- Resolución 256 de 2016: días entre la solicitud y la atención, con meta de 3
-- días en medicina general y 5 en especialista.
--
-- Se añade porque el extremo que el Python llama `/indicadores/oportunidad` NO
-- calcula oportunidad: cuenta atendidas, canceladas e inasistencias, y devuelve
-- las dos metas como constantes al lado. Son tres indicadores reales con un
-- nombre que promete un cuarto, y quien lea el tablero creerá que su IPS está
-- midiendo lo que la norma le exige. Sin esta columna el dato no se puede
-- calcular, así que portar el extremo tal cual habría sido mudar la mentira de
-- lenguaje.
--
-- NULLABLE, y las citas anteriores se quedan sin ella: el indicador dirá sobre
-- cuántas pudo medir. Rellenarla con la fecha de creación de la fila daría una
-- oportunidad de cero días para todo el histórico — el mejor número posible,
-- inventado.
ALTER TABLE cita ADD COLUMN solicitada_en TEXT;

-- =========================================================================
-- PARTE 2 — LA AGENDA (#237)
-- =========================================================================
--
-- Sin esto, «Agenda Médica» del menú no carga nada contra el binario y
-- `getSlots()` devuelve `of([])`: la hora de la cita se teclea a mano, sin que
-- nada compruebe que el médico atiende ese día ni que la hora está libre.
--
-- Tres tablas, que son las tres preguntas distintas:
--   agenda_medico   — qué meses tiene abiertos este médico
--   bloque_horario  — dentro de un mes, qué franjas atiende cada día de semana
--   bloqueo_agenda  — qué ratos concretos NO atiende (vacaciones, incapacidad)
--
-- EL DUEÑO ES LA INSTITUCIÓN, Y NO ES OPCIONAL. En el Python
-- (`models.py:2159`, `2189`) `institucion_id` es `Optional`, el mismo agujero
-- que tenía `ItemInventario`: agendas y bloqueos de todos los prestadores en la
-- misma bolsa. Aquí nacen con dueño y `NOT NULL`.

CREATE TABLE agenda_medico (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    medico_id      INTEGER NOT NULL REFERENCES usuario(id),
    anio           INTEGER NOT NULL CHECK (anio BETWEEN 2000 AND 2200),
    mes            INTEGER NOT NULL CHECK (mes BETWEEN 1 AND 12),
    -- Desactivar una agenda no la borra: el mes pasado sigue explicando por qué
    -- hubo citas a esas horas.
    activa         INTEGER NOT NULL DEFAULT 1,
    observaciones  TEXT,
    creada_por_id  INTEGER REFERENCES usuario(id),
    creada_en      TEXT NOT NULL DEFAULT (datetime('now')),
    -- Un médico, un mes, una agenda. Dos agendas del mismo mes darían dos
    -- respuestas distintas a «¿a qué hora atiende?» sin forma de elegir.
    UNIQUE (institucion_id, medico_id, anio, mes)
);

CREATE INDEX ix_agenda_medico ON agenda_medico (institucion_id, medico_id, anio, mes);

CREATE TABLE bloque_horario (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    agenda_id    INTEGER NOT NULL REFERENCES agenda_medico(id) ON DELETE CASCADE,
    -- 0 = lunes … 6 = domingo. Misma convención que `date.weekday()` del Python,
    -- para que el port no invierta la semana en silencio.
    dia_semana   INTEGER NOT NULL CHECK (dia_semana BETWEEN 0 AND 6),
    hora_inicio  TEXT NOT NULL,   -- HH:MM
    hora_fin     TEXT NOT NULL,   -- HH:MM
    -- Cuánto dura cada turno DE ESTE BLOQUE.
    --
    -- El Python lo tenía como constante global `DURACION_SLOT_MIN = 30`, y eso
    -- es una decisión de la IPS que el código tomaba por ella: el psiquiatra
    -- cita a 45 minutos y la jornada de vacunación a 10. Ojo con confundirlo con
    -- lo que corrige la parte 1 de esta migración: allí el defecto era mostrar
    -- como dato registrado algo que nadie registró; esto es una configuración
    -- declarada, con un valor convencional que el 30 recoge bien.
    duracion_slot_min INTEGER NOT NULL DEFAULT 30
        CHECK (duracion_slot_min > 0 AND duracion_slot_min <= 1440),
    activo       INTEGER NOT NULL DEFAULT 1,
    -- Que el bloque no vaya al revés. Comparar 'HH:MM' como texto ordena bien
    -- porque el formato es de ancho fijo con ceros a la izquierda. Va al final:
    -- SQLite solo admite restricciones de TABLA tras la última columna.
    CHECK (hora_inicio < hora_fin)
);

CREATE INDEX ix_bloque_agenda ON bloque_horario (agenda_id, dia_semana, activo);

CREATE TABLE bloqueo_agenda (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    medico_id      INTEGER NOT NULL REFERENCES usuario(id),
    -- YYYY-MM-DD HH:MM. Instante de inicio y de fin, no un día suelto: media
    -- mañana de reunión es el caso corriente, no la excepción.
    inicio         TEXT NOT NULL,
    fin            TEXT NOT NULL,
    motivo         TEXT NOT NULL,
    -- Los SEIS que la pantalla ya ofrece (`agenda-medica.component.ts:21`), ni
    -- uno menos: quitar «Día Libre» porque el vocabulario quedaba más limpio
    -- sería decidir por el prestador que ese caso no existe. El valor lo fija
    -- quien ya lo lee.
    tipo           TEXT NOT NULL DEFAULT 'OTRO'
        CHECK (tipo IN ('VACACIONES','INCAPACIDAD','REUNION','CAPACITACION','DIA_LIBRE','OTRO')),
    creado_por_id  INTEGER REFERENCES usuario(id),
    creado_en      TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (inicio < fin)
);

CREATE INDEX ix_bloqueo_medico ON bloqueo_agenda (institucion_id, medico_id, inicio, fin);
