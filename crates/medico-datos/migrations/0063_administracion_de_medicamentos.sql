-- MAR — Registro de Administración de Medicamentos (#249, bloque hospitalario).
--
-- # De los cuatro módulos de enfermería, éste es el ÚNICO que necesita tabla
--
-- Medido antes de escribir, que es la lección de `terceros` y `soportecobertura`:
-- de los cuatro que faltaban, dos ya tienen dónde vivir y uno no es una tabla.
--
-- | módulo | dónde vive | por qué |
-- |---|---|---|
-- | `ENFERMERIA` | `documento_clinico`, tipo `NOTA_ENFERMERIA` | la espina genérica ya sirve a evolución, epicrisis, interconsulta y nota operatoria |
-- | `PLAN_CUIDADO` | `documento_clinico`, tipo `PLAN_CUIDADO` | igual: es una nota estructurada que vive el ingreso entero |
-- | `KARDEX` | **ninguna** — es una VISTA | lo que el kardex enseña son las fórmulas activas del ingreso y lo que se ha administrado de cada una: los dos datos ya existen. Una tabla propia sería una tercera copia que se desincroniza |
-- | `MAR` | **esta tabla** | es un HECHO nuevo: quién puso qué, a quién y cuándo. No existía en ninguna parte |
--
-- # `dispensacion` NO es esto, y confundirlas cuesta una muerte
--
-- `dispensacion` (migración 0025) es la farmacia ENTREGANDO el medicamento. Esto
-- es enfermería ADMINISTRÁNDOLO al paciente. Entre las dos hay un carro, un turno
-- y a veces un error. Un sistema que dé la dispensación por administración
-- afirma que el paciente recibió lo que en realidad está en un cajón.
--
-- # La OMISIÓN se registra, y por eso `administrado` no es implícito
--
-- El registro que solo guarda lo que SÍ se puso es el que no sirve: la pregunta
-- clínica del turno siguiente —y la del juez— es «¿por qué no se le puso la de
-- las 14:00?». Un hueco en la tabla no distingue «no se administró» de «se
-- administró y no se registró», que es exactamente la directiva 30: el negativo
-- solo vale si el camino que lo produciría llegó a ejecutarse.
--
-- Por eso hay tres estados explícitos y ninguno por defecto:
--   `ADMINISTRADO` · `OMITIDO` (con motivo obligatorio) · `APLAZADO`
--
-- # No se borra ni se edita: se ANULA con motivo
--
-- Un MAR es prueba. Corregir en su sitio destruye lo que decía antes, y el
-- historial de qué se creyó administrado en cada momento es justo lo que se mira
-- cuando algo sale mal. La corrección es una fila nueva; ésta se marca anulada,
-- con quién y por qué. La misma regla que la historia firmada.
--
-- # `dosis_administrada` es TEXTO, y es deliberado
--
-- La fórmula guarda la dosis como texto («500 mg cada 8 horas»), y aquí se anota
-- lo que de verdad se puso («500 mg»). Convertirlo a número exigiría un modelo de
-- unidades que este producto no tiene, y un número sin unidad es peor que el
-- texto: 500 puede ser mg, ml o UI. Cuando exista el modelo de unidades, migra.
CREATE TABLE administracion_medicamento (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    -- La orden que se cumple. Sin cascada, y por lo mismo que `dispensacion`:
    -- borrar la fórmula no puede borrar la constancia de que se administró.
    formula_id          INTEGER NOT NULL REFERENCES formula(id),
    -- El ingreso durante el que se administró. Nulable: hay administraciones en
    -- urgencias sin hospitalización abierta.
    hospitalizacion_id  INTEGER REFERENCES hospitalizacion(id),
    -- Denormalizado desde formula→historia→paciente, NO recibido del cliente:
    -- quien administra no elige a quién se le apunta. Misma regla que 0025.
    paciente_id         INTEGER NOT NULL REFERENCES paciente(id),
    -- Quién lo puso. Es la firma del registro y no puede ser nula.
    administrado_por    INTEGER NOT NULL REFERENCES usuario(id),

    -- Cuándo TOCABA y cuándo se hizo. Son dos datos distintos (directiva 32):
    -- el primero mide la orden, el segundo el cumplimiento, y la diferencia
    -- entre ambos es el indicador de oportunidad que un hospital reporta.
    programada_para     TEXT,
    fecha               TEXT NOT NULL,

    estado              TEXT NOT NULL
        CHECK (estado IN ('ADMINISTRADO','OMITIDO','APLAZADO')),
    -- Obligatorio cuando el estado no es ADMINISTRADO. Lo exige el CHECK de
    -- abajo, no una regla escrita en prosa.
    motivo              TEXT,

    dosis_administrada  TEXT,
    -- Vía real. Puede diferir de la ordenada (una VO que se pasa a IV porque el
    -- paciente vomita), y ese cambio es información clínica, no una errata.
    via                 TEXT,
    observaciones       TEXT,

    -- La anulación, que sustituye al borrado.
    anulada             INTEGER NOT NULL DEFAULT 0 CHECK (anulada IN (0,1)),
    anulada_por         INTEGER REFERENCES usuario(id),
    anulada_en          TEXT,
    motivo_anulacion    TEXT,

    -- Lo que NO se administró tiene que decir por qué. Se pone aquí y no solo en
    -- el código porque un importador tampoco puede meter una omisión muda.
    CHECK (estado = 'ADMINISTRADO' OR (motivo IS NOT NULL AND trim(motivo) <> '')),
    -- Y una fila anulada tiene que decir quién y por qué: una anulación anónima
    -- borra la prueba sin dejar otra.
    CHECK (anulada = 0 OR (anulada_por IS NOT NULL AND motivo_anulacion IS NOT NULL
                           AND trim(motivo_anulacion) <> ''))
);

-- El kardex se lee por ingreso y por franja horaria; la hoja de un paciente, por
-- paciente. Los dos son consultas de turno, o sea las que más se corren.
CREATE INDEX ix_mar_hospitalizacion ON administracion_medicamento (hospitalizacion_id, fecha);
CREATE INDEX ix_mar_paciente        ON administracion_medicamento (paciente_id, fecha);
CREATE INDEX ix_mar_formula         ON administracion_medicamento (formula_id);
