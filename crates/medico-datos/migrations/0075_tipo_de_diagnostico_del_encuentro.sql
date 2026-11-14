-- Capturar el TIPO de diagnóstico (impresión / confirmado nuevo / confirmado
-- repetido) del encuentro, para HOSPITALIZACIÓN y URGENCIAS (#603, pieza «tipo de
-- diagnóstico de ingreso/egreso»).
--
-- POR QUÉ, medido contra el artefacto y no la memoria: aunque la Resolución 948
-- (RIPS/FEV) NO pide el tipo de diagnóstico para AU/AH, el PERFIL del RDA con el
-- que MinSalud valida el Bundle SÍ lo exige. En los IG oficiales
-- `StructureDefinition-EncounterEmergencyRDA` y `-EncounterHospitalizationRDA`,
-- las rebanadas `AdmissionDiagnosis` y `DischargeDiagnosis` traen:
--     Encounter.diagnosis.extension:ExtensionDiagnosisType  min=1 max=1
-- o sea OBLIGATORIO. Hoy el productor pasa `tipo: None` y el dominio lo reporta
-- como Falta (D20), así que estos RDA no emiten. Estas columnas dan de dónde
-- sacarlo. Las comorbilidades NO exigen el tipo, por eso solo se captura el del
-- diagnóstico PRINCIPAL de ingreso y de egreso.
--
-- NULLABLE Y SIN DEFAULT, y esto es la lección de la 0038, no un descuido: el
-- generador VFP llevaba el tipo de diagnóstico QUEMADO en '01' y la 0038 tuvo que
-- BORRARLO (`UPDATE historia SET tipo_diagnostico='' WHERE ='01'`) porque «el
-- sistema afirmaba en nombre del médico» (D20). Un DEFAULT aquí repetiría ese
-- error: afirmaría «impresión diagnóstica» sin que nadie lo eligiera. NULL = no
-- capturado, y el RDA no sale hasta que un humano lo registre.
--
-- El CÓDIGO es el de `RIPSTipoDiagnosticoPrincipalVersion2` (mismo value-set que
-- valida el productor para la consulta externa, `historia.tipo_diagnostico`); la
-- validación vive en el dominio al emitir, como en consulta — no se duplica aquí
-- (D19).

-- Hospitalización: ingreso y egreso, los dos diagnósticos principales que el
-- productor ya arma (`rda_encuentro::preparar_hosp_interno`).
ALTER TABLE hospitalizacion ADD COLUMN tipo_diagnostico_ingreso TEXT;
ALTER TABLE hospitalizacion ADD COLUMN tipo_diagnostico_egreso  TEXT;

-- Urgencias: SOLO el de ingreso. El productor de urgencias hoy arma únicamente el
-- diagnóstico de ingreso (`diagnostico_egreso` de urgencias es write-only: la
-- tabla lo guarda desde la 0042 pero ni `Atencion`/`COLS` lo leen ni el productor
-- lo emite — hueco aparte, fuera de esta pieza). Añadir aquí un
-- `tipo_diagnostico_egreso` sería andamiaje sin consumidor (D20/D22).
ALTER TABLE atencion_urgencias ADD COLUMN tipo_diagnostico_ingreso TEXT;
