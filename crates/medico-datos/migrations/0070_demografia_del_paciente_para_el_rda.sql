-- Los cuatro campos demográficos que el perfil PatientRDA (#603) exige y que
-- `paciente` no tenía: nacionalidad, pertenencia étnica (grupo + comunidad),
-- discapacidad e identidad de género. El ensamblador del RDA
-- (`ihce::actores::Paciente`) ya tiene los huecos para los cuatro y los emite
-- cuando llegan; esta migración es la CAPTURA que faltaba (#84).
--
-- Todos NULL para el paciente actual: son datos que hoy nadie captura, así que
-- nacen ausentes y el RDA los OMITE —no se rellenan con «otras etnias» ni «sin
-- discapacidad», que sería inventar un dato con aspecto de dato (directiva 20)—.
-- Se pueblan por la API de captura (`PacienteNuevo`), y su procedencia queda
-- registrada en `paciente.procedencias` como la del resto de la demografía.
--
-- Los códigos salen de catálogos del ARTEFACTO, no de memoria:
--   · pertenencia_etnica → CodeSystem ColombianEthnicGroup (1-6, 99)
--   · discapacidad       → CodeSystem ColombianDisabilityClassification (01-08)
--   · identidad_genero   → CodeSystem ColombianGenderIdentity (01-05)
-- los tres servidos por `ihce::terminologia` para derivar su `display`.
-- `comunidad_etnica` es TEXTO LIBRE por diseño del perfil (ExtensionPatientEthnicCommunity
-- no lleva binding): es el pueblo/comunidad concreto, no un código.
--
-- `nacionalidad` es un código ISO 3166-1 numérico (170 = Colombia), como
-- `cod_pais`, y por la misma razón no lleva catálogo: el IHCE vendoriza el
-- descriptor de formato ISO31661 pero NO la lista de países.
ALTER TABLE paciente ADD COLUMN nacionalidad       TEXT;
ALTER TABLE paciente ADD COLUMN pertenencia_etnica TEXT;
ALTER TABLE paciente ADD COLUMN comunidad_etnica   TEXT;
ALTER TABLE paciente ADD COLUMN discapacidad       TEXT;
ALTER TABLE paciente ADD COLUMN identidad_genero   TEXT;
