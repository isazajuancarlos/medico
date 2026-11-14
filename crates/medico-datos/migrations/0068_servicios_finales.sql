-- Los últimos módulos del catálogo (#249): banco de sangre, turnos y
-- teleconsulta.
--
-- `TRABAJO_SOCIAL` no aparece aquí y no es un olvido: es una nota clínica, y vive
-- en la espina `documento_clinico` con su tipo, como la de enfermería. Lo que
-- necesita es una CAPACIDAD propia, no una tabla.
--
-- # `TELEMEDICINA` ya estaba a medias, y hay que decirlo
--
-- Medido antes de escribir: `historia.modalidad_grupo_servicio` existe desde la
-- 0005, y el CodeSystem `ColombianTechModality` **ya está vendorizado** con las
-- cuatro modalidades de telemedicina —`06` interactiva, `07` no interactiva,
-- `08` telexperticia, `09` telemonitoreo—. O sea que una teleconsulta ya se
-- puede reportar al RIPS y al RDA.
--
-- Lo que NO existía es lo que la telemedicina tiene y una consulta presencial no:
-- **quién está en el otro extremo**. Sin eso, el registro no distingue una
-- telexperticia con un especialista de otra institución de una consulta normal
-- mal etiquetada. Ésa es la tabla, y solo ésa.

CREATE TABLE teleconsulta (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    -- La atención. UNIQUE: una consulta es teleconsulta o no lo es.
    historia_id         INTEGER NOT NULL UNIQUE REFERENCES historia(id) ON DELETE CASCADE,
    -- La modalidad, de las CUATRO de telemedicina del CodeSystem vendorizado.
    -- Se repite aquí y no se lee de `historia` a propósito: `historia` la tiene
    -- como texto libre para todo el catálogo, y aquí el `CHECK` garantiza que lo
    -- que hay en esta tabla es telemedicina y no un `01` intramural.
    modalidad           TEXT    NOT NULL CHECK (modalidad IN ('06','07','08','09')),

    -- El otro extremo. Al menos uno de los dos tiene que estar: sin ninguno, la
    -- fila no dice nada que `historia` no dijera ya.
    institucion_remota  TEXT,
    profesional_remoto  TEXT,

    plataforma          TEXT,
    -- El consentimiento del paciente para la modalidad. Nulable en el esquema
    -- porque puede firmarse en otro momento; cuando viene, la capa de datos
    -- comprueba que sea del MISMO paciente, igual que en quirófano.
    consentimiento_id   INTEGER REFERENCES consentimiento_informado(id),
    -- Las incidencias de conexión, que en telemedicina son parte del acto: una
    -- consulta que se cayó a los tres minutos no es la misma consulta.
    incidencias         TEXT,
    creado_en           TEXT    NOT NULL,

    CHECK ((institucion_remota IS NOT NULL AND trim(institucion_remota) <> '')
           OR (profesional_remoto IS NOT NULL AND trim(profesional_remoto) <> ''))
);

-- # TURNOS
--
-- `agenda_medico` y `bloque_horario` (0035) son la disponibilidad para AGENDAR
-- citas. Un turno es otra cosa: quién está de servicio en la planta a las tres de
-- la mañana, que no atiende citas y aun así responde. Usar la agenda para esto
-- pondría al médico de turno como disponible para consulta externa.

CREATE TABLE turno_personal (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    usuario_id     INTEGER NOT NULL REFERENCES usuario(id),
    servicio       TEXT    NOT NULL,
    tipo           TEXT    NOT NULL DEFAULT 'PRESENCIAL'
        CHECK (tipo IN ('PRESENCIAL','DISPONIBILIDAD')),
    inicio         TEXT    NOT NULL,
    fin            TEXT    NOT NULL,
    notas          TEXT,
    creado_en      TEXT    NOT NULL,

    -- Un turno que termina antes de empezar es imposible, no inusual.
    CHECK (fin > inicio)
);

CREATE INDEX ix_turno_institucion ON turno_personal (institucion_id, inicio);
CREATE INDEX ix_turno_usuario ON turno_personal (usuario_id, inicio);

-- # BANCO DE SANGRE
--
-- La regla de compatibilidad NO está aquí: vive en `medico_dominio::sangre`,
-- porque es una regla y se prueba sin base. Aquí está lo que la base sí puede
-- garantizar: que una unidad no se transfunda dos veces y que la que no pasó el
-- tamizaje no se transfunda nunca.

CREATE TABLE unidad_sangre (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    institucion_id    INTEGER NOT NULL REFERENCES institucion(id) ON DELETE CASCADE,
    -- El número de la unidad. Único por institución: es su identidad, y dos
    -- unidades con el mismo número hacen imposible la trazabilidad.
    codigo            TEXT    NOT NULL,
    hemocomponente    TEXT    NOT NULL
        CHECK (hemocomponente IN ('GLOBULOS_ROJOS','PLASMA','PLAQUETAS','CRIOPRECIPITADO')),
    grupo             TEXT    NOT NULL CHECK (grupo IN ('O','A','B','AB')),
    rh                TEXT    NOT NULL CHECK (rh IN ('+','-')),

    fecha_obtencion   TEXT,
    fecha_vencimiento TEXT NOT NULL,

    -- El tamizaje infeccioso. **`NULL` = todavía no se hizo**, que NO es lo mismo
    -- que «no conforme» ni, sobre todo, que «conforme». Sin él la unidad no se
    -- puede transfundir, y la diferencia entre los tres estados es la que impide
    -- que una unidad sin tamizar pase por tamizada.
    tamizaje_conforme INTEGER CHECK (tamizaje_conforme IS NULL OR tamizaje_conforme IN (0,1)),

    estado            TEXT    NOT NULL DEFAULT 'EN_CUARENTENA'
        CHECK (estado IN ('EN_CUARENTENA','DISPONIBLE','RESERVADA','TRANSFUNDIDA','DESCARTADA')),
    motivo_descarte   TEXT,
    creado_en         TEXT    NOT NULL,

    UNIQUE (institucion_id, codigo),
    -- Descartar una unidad exige decir por qué: es sangre que alguien donó.
    CHECK (estado <> 'DESCARTADA'
           OR (motivo_descarte IS NOT NULL AND trim(motivo_descarte) <> '')),
    -- Y una unidad DISPONIBLE tiene que haber pasado el tamizaje. No basta con
    -- comprobarlo en el código: un importador que no pase por ahí tampoco puede
    -- poner disponible lo que no se tamizó.
    --
    -- # `COALESCE` y no `= 1` a secas, y esto casi se me va
    --
    -- La primera versión decía `OR tamizaje_conforme = 1`. **No dispara con
    -- `NULL`**, que es justo el caso peligroso —«todavía no se tamizó»—: en SQL,
    -- `NULL = 1` no es falso, es NULL, y un `CHECK` que evalúa a NULL SE
    -- CONSIDERA SATISFECHO. O sea que la barandilla escrita para impedir que una
    -- bolsa sin tamizar se marque disponible dejaba pasar exactamente eso, y sin
    -- dar error.
    --
    -- Lo cazó `el_check_impide_disponible_sin_tamizaje`, que existe porque la
    -- casa exige probar los CHECK por la puerta de atrás. Es el único CHECK de
    -- las siete migraciones de #249 que compara una columna NULABLE con `=`;
    -- los demás comparan columnas `NOT NULL` o usan `IS NOT NULL`, y por eso no
    -- tienen el problema. Revisados uno a uno.
    CHECK (estado NOT IN ('DISPONIBLE','RESERVADA','TRANSFUNDIDA')
           OR COALESCE(tamizaje_conforme, 0) = 1)
);

CREATE INDEX ix_unidad_institucion ON unidad_sangre (institucion_id, estado);
CREATE INDEX ix_unidad_vencimiento ON unidad_sangre (fecha_vencimiento);

CREATE TABLE transfusion (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    -- UNIQUE: una unidad se transfunde UNA vez. Es la garantía que la base sí
    -- puede dar, y la que impide que dos servicios crean tener la misma bolsa.
    unidad_id        INTEGER NOT NULL UNIQUE REFERENCES unidad_sangre(id),
    paciente_id      INTEGER NOT NULL REFERENCES paciente(id),
    responsable_id   INTEGER NOT NULL REFERENCES usuario(id),
    fecha            TEXT    NOT NULL,

    -- EL GRUPO DEL RECEPTOR SE REGISTRA AQUÍ, en cada transfusión, y no se lee
    -- de una columna de `paciente` —que no existe, y es correcto que no exista—.
    -- El grupo se confirma con pruebas cruzadas antes de cada transfusión;
    -- transfundir contra un dato guardado hace meses es el error que mata.
    grupo_receptor   TEXT    NOT NULL CHECK (grupo_receptor IN ('O','A','B','AB')),
    rh_receptor      TEXT    NOT NULL CHECK (rh_receptor IN ('+','-')),

    reaccion_adversa INTEGER NOT NULL DEFAULT 0 CHECK (reaccion_adversa IN (0,1)),
    descripcion_reaccion TEXT,
    observaciones    TEXT,
    creado_en        TEXT    NOT NULL,

    -- Una reacción adversa sin describir no sirve para nada: es justo lo que se
    -- investiga después.
    CHECK (reaccion_adversa = 0
           OR (descripcion_reaccion IS NOT NULL AND trim(descripcion_reaccion) <> ''))
);

CREATE INDEX ix_transfusion_paciente ON transfusion (paciente_id, fecha);
