-- CERTIFICADO_DEFUNCION (#249), el gemelo del registro de nacimiento.
--
-- # Lo que medico YA sabía, y por qué no bastaba
--
-- Medido antes de escribir: el sistema **sí** registra que alguien murió, en dos
-- sitios y con dos vocabularios distintos —`hospitalizacion.estado = 'DEFUNCION'`
-- y `atencion_urgencias.destino_salida = 'FALLECIDO'`—, y el RIPS ya lo reporta
-- (condición de destino `02`). Lo cazó la revisión del 2026-08-15, cuando el RDA
-- omitía `Patient.deceased` con un comentario que decía que medico no registraba
-- defunción: era falso, y presentaba como VIVO en el visor nacional a un
-- fallecido.
--
-- Lo que NO existe es el CERTIFICADO: el documento con la cadena de causas, la
-- manera de muerte y quién la certifica. Un egreso que dice «murió» no dice de
-- qué, y de eso dependen la estadística vital del DANE y el registro civil de
-- defunción.
--
-- # `estado = 'DEFUNCION'` y esta tabla son DOS datos, no uno (directiva 32)
--
-- El egreso mide la ATENCIÓN —terminó, y así—; el certificado mide el HECHO
-- VITAL. Derivar el segundo del primero sería inventar las causas; derivar el
-- primero del segundo dejaría hospitalizaciones abiertas de pacientes muertos.
-- Se capturan los dos y se COMPRUEBA que no se contradigan, que es distinto de
-- copiarlos.
--
-- # La cadena de causas es del certificado internacional, y el orden importa
--
-- `causa_directa` es lo que mató; `antecedente_a/b/c` son las que la produjeron,
-- en orden ascendente; `causa_basica` es la que el certificador declara como
-- origen de todo, y es LA QUE VA A LA ESTADÍSTICA. No se deduce de las otras:
-- decidirla es un acto médico, y un programa que eligiera «la última de la
-- cadena» produciría estadística vital falsa a escala nacional.
--
-- Por eso `causa_basica` es NOT NULL y las demás no. Un certificado sin causa
-- básica no es un certificado.
--
-- # Los tres sitios y las cuatro maneras salen del formato del certificado
--
-- No se inventan aquí y no se amplían sin motivo: un valor de más en una tabla
-- que alimenta estadística nacional es un valor que nadie sabe interpretar.
CREATE TABLE defuncion (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    paciente_id           INTEGER NOT NULL REFERENCES paciente(id),
    -- El evento durante el que ocurrió, si ocurrió dentro de uno. Nulables las
    -- dos: hay defunciones que llegan ya ocurridas (cadáver, muerte en casa).
    hospitalizacion_id    INTEGER REFERENCES hospitalizacion(id),
    atencion_urgencias_id INTEGER REFERENCES atencion_urgencias(id),

    fecha_defuncion       TEXT NOT NULL,   -- RFC 3339 UTC, con hora
    sitio                 TEXT NOT NULL
        CHECK (sitio IN ('INSTITUCION','DOMICILIO','VIA_PUBLICA','OTRO')),
    manera                TEXT NOT NULL
        CHECK (manera IN ('NATURAL','VIOLENTA','EN_ESTUDIO','NO_DETERMINADA')),

    -- La cadena. CIE-10, `varchar(6)` como en el resto del producto.
    causa_directa         TEXT,
    antecedente_a         TEXT,
    antecedente_b         TEXT,
    antecedente_c         TEXT,
    -- La que va a la estadística. Obligatoria: sin ella no hay certificado.
    causa_basica          TEXT NOT NULL,

    -- El número del certificado del DANE. Nulable porque el talonario a veces
    -- se diligencia después; cuando está, es único por institución.
    numero_certificado    TEXT,
    -- Quién certifica. Es un acto médico con nombre y responsabilidad: no puede
    -- ser nulo, igual que la firma de la historia.
    certificado_por       INTEGER NOT NULL REFERENCES usuario(id),

    observaciones         TEXT,

    -- La anulación, en vez del borrado, por lo mismo que el MAR: un certificado
    -- es prueba y su corrección deja rastro.
    anulada               INTEGER NOT NULL DEFAULT 0 CHECK (anulada IN (0,1)),
    anulada_por           INTEGER REFERENCES usuario(id),
    anulada_en            TEXT,
    motivo_anulacion      TEXT,

    creado_en             TEXT NOT NULL,

    -- Una muerte VIOLENTA sin observaciones deja al forense sin nada. No se
    -- exige un texto concreto —eso sería fingir que el sistema entiende el
    -- caso— pero sí que haya alguno.
    CHECK (manera <> 'VIOLENTA' OR (observaciones IS NOT NULL AND trim(observaciones) <> '')),
    CHECK (anulada = 0 OR (anulada_por IS NOT NULL AND motivo_anulacion IS NOT NULL
                           AND trim(motivo_anulacion) <> ''))
);

-- Un paciente no muere dos veces. El índice es parcial —solo sobre las NO
-- anuladas— para que corregir un certificado (anular y reexpedir) siga siendo
-- posible: sin el `WHERE`, la reexpedición chocaría con el certificado anulado.
CREATE UNIQUE INDEX ux_defuncion_paciente ON defuncion (paciente_id) WHERE anulada = 0;
CREATE INDEX ix_defuncion_fecha ON defuncion (fecha_defuncion);
