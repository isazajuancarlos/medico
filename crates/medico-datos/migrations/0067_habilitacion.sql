-- El bloque de HABILITACIÓN (#249): equipos, comités e infecciones.
--
-- Son tres de los cuatro que el estándar de habilitación exige poder ENSEÑAR en
-- una visita. El cuarto, `INDICADORES`, no lleva tabla: se deriva (ver
-- `medico_datos::indicadores`).
--
-- # `item_inventario` NO sirve para los equipos, y hay que decir por qué
--
-- La 0025 tiene un tipo `DISPOSITIVO`, así que la tentación era meter ahí los
-- equipos biomédicos y ahorrarse una tabla. No sirve: `item_inventario` modela
-- CONSUMIBLES —cantidad, lote, vencimiento, mínimo de reposición—, y un
-- ecógrafo no es «cantidad_actual = 1». Lo que un equipo biomédico necesita es
-- lo que un consumible no tiene: número de serie, hoja de vida, periodicidad de
-- mantenimiento y la fecha del próximo. Meterlo en la tabla de la farmacia
-- dejaría seis columnas nulas y ninguna de las que se revisan en la visita.
--
-- Es el mismo juicio que en `terceros` (que SÍ era `entidad`) pero con el signo
-- contrario, y por eso se escribe: buscar por parecido acierta unas veces y
-- otras no, y lo que decide es qué preguntas tiene que responder la tabla.

CREATE TABLE equipo_biomedico (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id      INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    nombre              TEXT    NOT NULL,
    marca               TEXT,
    modelo              TEXT,
    -- La serie identifica al aparato. Es única POR INSTITUCIÓN y no global: dos
    -- prestadores distintos pueden tener equipos con numeraciones que choquen.
    serie               TEXT,
    ubicacion           TEXT,
    registro_invima     TEXT,
    fecha_adquisicion   TEXT,

    estado              TEXT    NOT NULL DEFAULT 'ACTIVO'
        CHECK (estado IN ('ACTIVO','EN_MANTENIMIENTO','FUERA_DE_SERVICIO','DADO_DE_BAJA')),

    -- Cada cuántos meses toca el preventivo. NULL = no se le ha definido
    -- periodicidad, que NO es lo mismo que «no necesita»: sin ella, el equipo
    -- sale en el informe como «sin cronograma», que es justo lo que la visita
    -- de habilitación anota (directiva 20).
    periodicidad_meses  INTEGER CHECK (periodicidad_meses IS NULL OR periodicidad_meses > 0),
    -- Cuándo toca el próximo. Se escribe al registrar un mantenimiento; que esté
    -- VENCIDO no se guarda, se compara con hoy al leer — un booleano guardado
    -- estaría mintiendo desde el día siguiente.
    proximo_mantenimiento TEXT,

    motivo_baja         TEXT,
    fecha_baja          TEXT,
    creado_en           TEXT    NOT NULL,

    UNIQUE (institucion_id, serie),
    -- Dar de baja un equipo exige decir cuándo y por qué: la hoja de vida tiene
    -- que explicar por qué el aparato ya no está.
    CHECK (estado <> 'DADO_DE_BAJA'
           OR (fecha_baja IS NOT NULL AND motivo_baja IS NOT NULL AND trim(motivo_baja) <> ''))
);

CREATE INDEX ix_equipo_institucion ON equipo_biomedico (institucion_id, estado);
-- La consulta de la visita: qué equipos tienen el mantenimiento vencido.
CREATE INDEX ix_equipo_proximo ON equipo_biomedico (proximo_mantenimiento);

CREATE TABLE mantenimiento_equipo (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    equipo_id       INTEGER NOT NULL REFERENCES equipo_biomedico(id),
    tipo            TEXT    NOT NULL
        CHECK (tipo IN ('PREVENTIVO','CORRECTIVO','CALIBRACION')),
    fecha           TEXT    NOT NULL,
    -- Quién lo hizo. Puede ser externo, así que es texto y no una FK a usuario:
    -- el proveedor que calibra no tiene cuenta en el sistema.
    responsable     TEXT    NOT NULL,
    hallazgos       TEXT,
    -- ¿Quedó conforme? Un mantenimiento que NO deja el equipo conforme y no dice
    -- qué encontró es una firma sin contenido.
    conforme        INTEGER NOT NULL CHECK (conforme IN (0,1)),
    costo           REAL    CHECK (costo IS NULL OR costo >= 0),
    creado_en       TEXT    NOT NULL,

    CHECK (conforme = 1 OR (hallazgos IS NOT NULL AND trim(hallazgos) <> ''))
);

CREATE INDEX ix_mantenimiento_equipo ON mantenimiento_equipo (equipo_id, fecha);

CREATE TABLE comite (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id     INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    -- Los cinco que una IPS suele tener que acreditar, más `OTRO` para los que
    -- cada prestador añada. La lista no pretende ser la norma: pretende que los
    -- cinco habituales no se escriban de cinco formas distintas y luego no se
    -- puedan contar.
    tipo               TEXT    NOT NULL
        CHECK (tipo IN ('HISTORIAS_CLINICAS','INFECCIONES','FARMACIA_Y_TERAPEUTICA',
                        'ETICA_HOSPITALARIA','SEGURIDAD_DEL_PACIENTE','OTRO')),
    nombre             TEXT    NOT NULL,
    -- Cada cuántos meses debe sesionar. De aquí sale «este comité está en mora»,
    -- que es lo que se revisa.
    periodicidad_meses INTEGER CHECK (periodicidad_meses IS NULL OR periodicidad_meses > 0),
    activo             INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0,1)),
    creado_en          TEXT    NOT NULL
);

CREATE INDEX ix_comite_institucion ON comite (institucion_id, activo);

CREATE TABLE sesion_comite (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    comite_id   INTEGER NOT NULL REFERENCES comite(id),
    fecha       TEXT    NOT NULL,
    -- Un acta sin asistentes no es un acta. Se guarda como texto porque los
    -- asistentes incluyen a gente sin cuenta en el sistema (un representante de
    -- la comunidad, un proveedor), y una FK dejaría fuera justo a esos.
    asistentes  TEXT    NOT NULL CHECK (trim(asistentes) <> ''),
    temas       TEXT,
    -- Lo que la sesión DECIDIÓ. Es lo que se le pide a un acta cuando alguien la
    -- audita: no qué se habló, sino a qué se comprometió la institución.
    decisiones  TEXT,
    creado_en   TEXT    NOT NULL
);

CREATE INDEX ix_sesion_comite ON sesion_comite (comite_id, fecha);

CREATE TABLE evento_iaas (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id        INTEGER NOT NULL REFERENCES paciente(id),
    -- La hospitalización durante la que se detectó. Nulable: hay infecciones de
    -- sitio operatorio que se detectan después del egreso.
    hospitalizacion_id INTEGER REFERENCES hospitalizacion(id),
    reportado_por      INTEGER NOT NULL REFERENCES usuario(id),

    tipo               TEXT    NOT NULL
        CHECK (tipo IN ('NEUMONIA_ASOCIADA_A_VM','INFECCION_SITIO_OPERATORIO',
                        'ITU_ASOCIADA_A_CATETER','BACTERIEMIA_ASOCIADA_A_CVC','OTRA')),
    fecha_deteccion    TEXT    NOT NULL,
    -- El dispositivo al que se asocia. Los tres tipos «asociados a» lo EXIGEN:
    -- una neumonía asociada a ventilación mecánica sin decir a qué ventilador
    -- —ni cuántos días llevaba— no es un evento de vigilancia, es una nota.
    dispositivo_asociado TEXT,
    dias_dispositivo   INTEGER CHECK (dias_dispositivo IS NULL OR dias_dispositivo >= 0),
    microorganismo     TEXT,

    -- La notificación al sistema nacional de vigilancia. `notificado` sin fecha
    -- es una afirmación que nadie puede comprobar.
    notificado         INTEGER NOT NULL DEFAULT 0 CHECK (notificado IN (0,1)),
    fecha_notificacion TEXT,

    desenlace          TEXT,
    observaciones      TEXT,
    creado_en          TEXT    NOT NULL,

    CHECK (tipo NOT IN ('NEUMONIA_ASOCIADA_A_VM','ITU_ASOCIADA_A_CATETER',
                        'BACTERIEMIA_ASOCIADA_A_CVC')
           OR (dispositivo_asociado IS NOT NULL AND trim(dispositivo_asociado) <> '')),
    CHECK (notificado = 0 OR fecha_notificacion IS NOT NULL)
);

CREATE INDEX ix_iaas_fecha ON evento_iaas (fecha_deteccion);
CREATE INDEX ix_iaas_paciente ON evento_iaas (paciente_id);

-- ── Y lo que `INDICADORES` necesitaba y no existía ──────────────────────────
--
-- La OPORTUNIDAD de la consulta —los días entre que el paciente la pide y el día
-- que lo atienden— es de los indicadores que más se reportan, y **no se podía
-- calcular**: `cita` (migración 0004) guarda la fecha de la CITA y no la de la
-- SOLICITUD. Son dos datos distintos (directiva 32) y sin el primero cualquier
-- número de oportunidad sería inventado.
--
-- Se añade nulable, y las citas anteriores se quedan sin él a propósito: el
-- indicador dirá sobre cuántas pudo calcularse y cuántas quedaron fuera, en vez
-- de rellenarlas con la fecha de la cita —que daría oportunidad CERO para todo
-- el histórico, que es el número más halagador y más falso posible—.
ALTER TABLE cita ADD COLUMN fecha_solicitud TEXT;
