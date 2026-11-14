// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza
//! Los **tres RDA de encuentro clínico**: consulta externa, urgencias y
//! hospitalización.
//!
//! Van juntos y no en tres módulos porque son el MISMO documento con distinto
//! alcance: comparten las siete primeras secciones en el mismo orden, el mismo
//! esqueleto de `Composition` y las mismas referencias. Lo único que los separa
//! es el perfil, el código LOINC del tipo y qué secciones añaden al final.
//!
//! Separarlos habría triplicado el sitio donde arreglar un fallo de forma, que
//! es exactamente lo que `quipu_nucleo::container` evita en la familia Quipu.
//!
//! # De dónde salen estas tablas
//!
//! **De los ejemplos oficiales, leídos con un programa, no transcritos a ojo.**
//! Están en `datos/ihce/ejemplos/` y cada constante de abajo se compara contra
//! el suyo en las pruebas: si MinSalud publica otra versión con una sección
//! distinta, la prueba se pone roja y dice cuál.
//!
//! Importa porque una sección con el código equivocado **no da error**: el
//! servidor la acepta y guarda las alergias donde van los procedimientos.
//!
//! # El mapeo desde medico
//!
//! Ya estaba escrito en `datos/ihce/TRANSPORTE.md` y no cambia:
//!
//! | de medico | RDA |
//! |---|---|
//! | `historia` | consulta externa |
//! | `atencion_urgencias` | urgencias |
//! | `hospitalizacion` (al egreso) | hospitalización |
//! | evento clínico cualquiera | paciente (ver [`super::paciente`]) |

use super::{Concepto, Faltas, Meta, Ref};

/// Una sección: código LOINC, su `display` y el título en español. Los tres van
/// juntos porque los tres viajan en el documento y los tres los fija el ejemplo.
pub type Seccion = (&'static str, &'static str, &'static str);

/// Las SIETE que comparten los tres. Idénticas y en el mismo orden.
///
/// La primera lleva un sufijo distinto en cada documento —«(consulta)»,
/// «(urgencias)», «(Hospitalización / Internación)»—, así que su título se
/// completa en cada variante y aquí va sin él.
const COMUNES: [Seccion; 7] = [
    ("48768-6", "Payment sources Document",
     "Entidad(es) responsable(s) por el plan de beneficios en salud"),
    ("74208-0", "Demographic information + History of occupation Document",
     "Otros datos demográficos"),
    ("105583-9", "Worker Sick leave form",
     "Datos incapacidad (SIPE – Sistema de Incapacidades y Prestaciones Economicas)"),
    ("11450-4", "Problem list - Reported",
     "Historial de diagnósticos de problemas de salud"),
    ("48765-2", "Allergies and adverse reactions Document",
     "Historial de alergias, intolerancias y reacciones adversas"),
    ("75492-9", "Risk assessment and screening note", "Factores de riesgo"),
    ("10160-0", "History of Medication use Narrative", "Historial de medicamentos"),
];

const PROCEDIMIENTOS: Seccion =
    ("47519-4", "History of Procedures Document", "Historial de procedimientos");
const RESULTADOS: Seccion = ("30954-2", "Relevant diagnostic tests/laboratory data note",
                             "Resultados del uso de las tecnologías en salud");
const ORDENES: Seccion = ("61146-1", "Orders for services Document",
                          "Órdenes, prescripciones o solicitudes de servicio");
const SOPORTES: Seccion = ("55107-7", "Addendum Document", "Documentos de soporte");
const TRIAJE: Seccion = ("54094-8", "Emergency department Triage note", "Clasificación de triaje");

/// Las 12 secciones del RDA clínico (las 7 comunes + las 5 específicas) AVALADAS
/// por un clínico, para las fixtures (F4.5). Cada una nombra usuario y sello, como
/// exige [`super::Aval`]; una fixture que las confirma todas modela «el clínico
/// revisó el encuentro».
///
/// **`#[cfg(test)]` a propósito, como el de `paciente`**: producción no confirma
/// secciones desde una lista; el aval se construye desde el acto real del clínico.
/// Una lista de «todas las secciones» que sirve para saltarse la puerta del
/// `nilknown` no debe existir en el binario ni ser un asa de bypass entre crates
/// (revisión de seguridad #603).
#[cfg(test)]
const fn conf(seccion: &str) -> super::Confirmacion<'_> {
    super::Confirmacion { seccion, usuario_id: 7, sello: "2026-08-14T18:42:00-05:00" }
}

#[cfg(test)]
pub(crate) const CONFIRMACIONES: [super::Confirmacion<'static>; 12] = [
    conf(COMUNES[0].0), conf(COMUNES[1].0), conf(COMUNES[2].0), conf(COMUNES[3].0),
    conf(COMUNES[4].0), conf(COMUNES[5].0), conf(COMUNES[6].0),
    conf(PROCEDIMIENTOS.0), conf(RESULTADOS.0), conf(ORDENES.0), conf(SOPORTES.0), conf(TRIAJE.0),
];

/// Cuál de los tres encuentros se está resumiendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encuentro {
    ConsultaExterna,
    Urgencias,
    Hospitalizacion,
}

impl Encuentro {
    /// El perfil, sin la base canónica.
    pub fn perfil(self) -> &'static str {
        match self {
            Encuentro::ConsultaExterna => "CompositionAmbulatoryRDA",
            Encuentro::Urgencias => "CompositionEmergencyRDA",
            Encuentro::Hospitalizacion => "CompositionHospitalizationRDA",
        }
    }

    /// El LOINC del tipo de documento.
    pub fn tipo(self) -> &'static str {
        match self {
            Encuentro::ConsultaExterna => "51845-6",
            Encuentro::Urgencias => "59258-4",
            Encuentro::Hospitalizacion => "34105-7",
        }
    }

    /// El sufijo de la primera sección, que es lo único que la diferencia.
    fn sufijo(self) -> &'static str {
        match self {
            Encuentro::ConsultaExterna => " (consulta)",
            Encuentro::Urgencias => " (urgencias)",
            Encuentro::Hospitalizacion => " (Hospitalización / Internación)",
        }
    }

    /// `Composition.title`, **literal del ejemplo oficial**.
    ///
    /// Es 1..1 en FHIR R4, o sea OBLIGATORIO: sin él el documento es rechazable
    /// por estructura, y nada local lo diría. Faltaba hasta el 2026-08-14, y la
    /// asimetría es lo que lo delata — `paciente.rs` sí lo emitía y además tenía
    /// su prueba (`el_titulo_es_literal_el_del_ejemplo`), así que el repositorio
    /// ya trataba este campo como contrato y esta variante se quedó sin las dos
    /// cosas a la vez. Lo cazó la revisión de seguridad de este mismo cambio.
    ///
    /// No se compone ni se traduce: se copia. El del RDA de paciente es una
    /// frase larga y estos tres son cortos, y esa diferencia es del ejemplo, no
    /// nuestra.
    fn titulo(self) -> &'static str {
        match self {
            Encuentro::ConsultaExterna => "RDA Consulta",
            Encuentro::Urgencias => "RDA Urgencias",
            Encuentro::Hospitalizacion => "RDA Hospitalización",
        }
    }

    /// Las secciones del documento, en ORDEN. El orden es parte del contrato:
    /// el ejemplo las trae así y no hay motivo para adivinar otro.
    pub fn secciones(self) -> Vec<Seccion> {
        let mut v: Vec<Seccion> = COMUNES.to_vec();
        match self {
            // La consulta externa NO lleva procedimientos ni resultados: es el
            // encuentro más liviano de los tres.
            Encuentro::ConsultaExterna => v.extend([ORDENES, SOPORTES]),
            Encuentro::Urgencias => v.extend([PROCEDIMIENTOS, RESULTADOS, ORDENES, SOPORTES, TRIAJE]),
            Encuentro::Hospitalizacion => v.extend([PROCEDIMIENTOS, RESULTADOS, ORDENES, SOPORTES]),
        }
        v
    }

    // ── Lo que el `Encounter` necesita de cada encuentro (#603 fase 4). Todo
    // FIJO por el perfil `EncounterXxxRDA` (contrato, no dato).

    /// `Encounter.class`: `(code, display)`, fijado por el perfil.
    fn class(self) -> (&'static str, &'static str) {
        match self {
            Encuentro::ConsultaExterna => ("AMB", "ambulatory"),
            Encuentro::Urgencias => ("EMER", "emergency"),
            Encuentro::Hospitalizacion => ("IMP", "inpatient encounter"),
        }
    }

    /// `Encounter.type:encounterServiceGroup`: `(code, display)`, FIJO por el
    /// perfil del `Encounter` (`01`/`05`/`03`) — no es el `grupo_servicios` del
    /// insumo, que alimenta `Composition.event`.
    fn grupo_encounter(self) -> (&'static str, &'static str) {
        match self {
            Encuentro::ConsultaExterna => ("01", "Consulta externa"),
            Encuentro::Urgencias => ("05", "Atención inmediata"),
            Encuentro::Hospitalizacion => ("03", "Internación"),
        }
    }

    /// `Encounter.type:encounterModality` (`ColombianTechModality`) DERIVADO, cuando
    /// el encuentro Y el ENTORNO del prestador juntos lo determinan.
    ///
    /// Se deriva `01` (intramural) para hospitalización y urgencias **SOLO cuando el
    /// entorno de atención es `05` (Institucional)**. Ahí una internación (cama
    /// física, `cama_id NOT NULL`) o una urgencia presencial es intramural por
    /// estructura, no por elección clínica —igual que `grupo_encounter`—, y derivarla
    /// no es «afirmar lo que nadie eligió» (D20/D1).
    ///
    /// **La condición del entorno la exige la coherencia del propio `Encounter.type`,
    /// y la cazó el concilio (2026-09-17):** `encounterEnvironment` sale del entorno
    /// CONFIGURADO (`MEDICO_ENTORNO_ATENCION`, que medico hace configurable justo para
    /// soportar despliegues extramurales sin emitirlo en silencio). Si ese entorno es
    /// `01` Hogar / `02` Comunitario / … , derivar `01 Intramural` emitiría un
    /// `Encounter` que se contradice a sí mismo —«en el hogar, intramural»— y manda un
    /// dato clínico FALSO al visor nacional. A diferencia de `encounterServiceGroup`
    /// (que el perfil fija con `pattern`), `encounterModality` tiene binding abierto:
    /// el perfil NO lo determina, así que solo el entorno institucional lo hace.
    /// Fuera de `05`, no se deriva: vuelve a Falta hasta que exista captura de la
    /// modalidad. Consulta externa SÍ varía (intramural / telemedicina): `None`
    /// siempre —su modalidad se CAPTURA (`historia.modalidad_grupo_servicio`)—.
    fn modalidad_derivada(self, entorno: Option<&str>) -> Option<&'static str> {
        let institucional = entorno.map(str::trim) == Some("05");
        match self {
            Encuentro::Hospitalizacion | Encuentro::Urgencias if institucional => Some("01"),
            _ => None,
        }
    }

    /// `Encounter.participant`: `(id, code, display)`. En consulta el profesional
    /// ATIENDE (`ATND`); en urgencias y egreso, quien DA DE ALTA (`DIS`).
    fn participante(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Encuentro::ConsultaExterna => ("AttenderPhysician", "ATND", "attender"),
            Encuentro::Urgencias | Encuentro::Hospitalizacion => {
                ("DischargePhysician", "DIS", "discharger")
            }
        }
    }

    /// ¿El perfil de este encuentro admite el bloque `hospitalization`
    /// (`admitSource`/`dischargeDisposition`)? La consulta externa no lo tiene.
    fn admite_hospitalizacion(self) -> bool {
        matches!(self, Encuentro::Urgencias | Encuentro::Hospitalizacion)
    }

    /// ¿Es un encuentro con dos diagnósticos (ingreso + egreso)? La consulta
    /// externa lleva UNO (el principal); urgencias y hospitalización, dos.
    fn tiene_egreso(self) -> bool {
        matches!(self, Encuentro::Urgencias | Encuentro::Hospitalizacion)
    }
}

/// Ensambla el RDA de un encuentro clínico, o devuelve TODAS las faltas juntas.
pub fn ensamblar(
    encuentro: Encuentro,
    i: &super::paciente::Insumo<'_>,
) -> Result<super::paciente::Bundle, Faltas> {
    // EL BORDE. Aquí entra el dato al módulo, así que aquí se normaliza —y
    // no dentro de `actores::ensamblar`, que solo normalizaba su copia y
    // dejaba las referencias de este archivo leyendo el crudo (ver
    // `Insumo::recortado`).
    let i = &i.recortado();
    let mut f = Faltas::default();
    f.exigir(Some(i.fecha), "Composition", "date",
             "sin fecha con huso, el momento de la atención se guarda mal");
    // EN EGRESO Y EN URGENCIAS, «NO SÉ SI MURIÓ» NO ES UNA RESPUESTA.
    //
    // `Paciente.fallecido` es `Option<bool>` y `None` significa «no hay
    // declaración» — pero FHIR R4 advierte que la mayoría de los sistemas leen
    // la AUSENCIA de `Patient.deceased[x]` como que la persona vive. O sea que
    // en el cable `None` es indistinguible de «vivo», y el visor nacional
    // enseñaría vivo a un paciente que murió, sin que nada diera error.
    //
    // Estos son justamente los dos documentos donde medico SÍ conoce el dato:
    // `hospitalizacion.estado` admite `DEFUNCION` y
    // `atencion_urgencias.destino_salida` admite `FALLECIDO`, y RIPS ya los
    // reporta desde `rips/ah.rs` y `rips/au.rs`. Que el ensamblador tenga las
    // dos mitades —el encuentro y el paciente— y no las cruce era dejar el
    // agujero abierto con un tipo que parece haberlo cerrado.
    //
    // No se rellena con `Some(false)`, que sería inventar la respuesta más
    // peligrosa de las dos (directiva 20): se exige que el llamante declare.
    // El RDA de paciente y el de consulta externa NO pasan por aquí, y ahí
    // `None` sigue siendo correcto porque nadie lo sabe.
    if matches!(encuentro, Encuentro::Urgencias | Encuentro::Hospitalizacion)
        && i.paciente.fallecido.is_none()
    {
        f.0.push(super::Falta {
            recurso: "Patient",
            campo: "deceasedBoolean",
            porque: "en egreso y en urgencias el desenlace se conoce, y omitir el campo \
                     presenta como VIVO a un paciente fallecido: la ausencia de valor es \
                     lo que los lectores de FHIR interpretan como que la persona vive",
        });
    }
    // Las exigencias de los tres actores viven con ellos (`ihce::actores`) y no
    // repetidas aquí: eran las mismas seis líneas copiadas en los dos
    // ensambladores, y copiadas es como divergen (directiva 9).
    let actores = match super::actores::ensamblar(&i.paciente, &i.institucion, &i.profesional) {
        Ok(v) => {
            if f.hay() { return Err(f); }
            v
        }
        Err(suyas) => {
            f.0.extend(suyas.0);
            return Err(f);
        }
    };

    let paciente_id = i.id_paciente();
    let profesional_id = i.id_profesional();

    // ── LOS RECURSOS CLÍNICOS (#603 fase 4): Encounter, Condition(es), pagador
    // EAPB y epicrisis. Se construyen ANTES de las secciones porque las secciones
    // los referencian, y acumulan sus propias faltas —un encuentro sin el dato NO
    // emite el recurso (directiva 20)—. `entradas` dice qué sección puebla qué.
    let (recursos_clinicos, entradas) =
        recursos_del_encuentro(encuentro, i, &paciente_id, &profesional_id, &mut f);

    // LA PUERTA CLÍNICA del `nilknown`, la MISMA que el RDA de paciente (#603,
    // directiva 9): cada sección vacía necesita aval humano para emitir
    // `emptyReason=nilknown`; sin él es una FALTA y el documento no se emite. Las
    // secciones que el encuentro POBLÓ (`entradas`) llevan `entry` y NO pasan por
    // la puerta —una sección con contenido no afirma «Nil Known»—.
    let c = i.aval;
    let secciones: Vec<super::SeccionRda> = encuentro
        .secciones()
        .into_iter()
        .enumerate()
        .map(|(n, (code, display, title))| {
            // Solo la PRIMERA lleva sufijo, que es como viene en el ejemplo.
            let titulo =
                if n == 0 { format!("{title}{}", encuentro.sufijo()) } else { title.to_string() };
            match entradas.get(code) {
                Some(refs) => super::seccion_poblada(&titulo, (code, display), refs.clone()),
                None => super::seccion(&titulo, (code, display), c, &mut f),
            }
        })
        .collect();
    // Si alguna sección quedó vacía sin confirmación, o a un recurso le faltó un
    // dato, no se emite un RDA a medias — se devuelven TODAS las faltas juntas.
    if f.hay() {
        return Err(f);
    }

    let mut composicion = serde_json::json!({
        "resourceType": "Composition",
        "meta": Meta::perfil(encuentro.perfil()),
        "status": "final",
        "type": Concepto { coding: vec![super::Coding {
            system: super::LOINC.into(), code: encuentro.tipo().into(), display: None }] },
        "subject": Ref::a(&paciente_id),
        // El encuentro al que pertenece el documento (`min=1` en el perfil): apunta
        // al recurso Encounter que viaja en este mismo Bundle.
        "encounter": Ref::a(super::recursos::ID_ENCOUNTER),
        "date": i.fecha,
        "title": encuentro.titulo(),

        // OJO AL ORDEN, QUE VA AL REVÉS QUE EN `paciente.rs` Y NO ES UN DESCUIDO.
        //
        // Aquí el AUTOR es la INSTITUCIÓN y quien atesta legalmente es el
        // PROFESIONAL. En el RDA de paciente es exactamente al contrario. Los
        // cuatro documentos NO comparten esta convención, y suponer que sí es el
        // defecto que tuvo este archivo hasta el 2026-08-14: se copió el patrón
        // de `paciente.rs` y quedaron los dos campos intercambiados.
        //
        // Medido contra los cuatro ejemplos oficiales, que son unánimes:
        //
        //   paciente          author=#CC-…            attester.party=#Código…
        //   consulta externa  author=#Código…         attester.party=#CC-…
        //   urgencias         author=#Código…         attester.party=#CC-…
        //   hospitalización   author=#Código…         attester.party=#CC-…
        //
        // Y NO DABA ERROR, que es lo que lo hacía peligroso: es la misma clase
        // que la sección con el código equivocado —el servidor lo acepta—. Solo
        // que aquí `attester.mode = "legal"` nombra a QUIEN RESPONDE JURÍDICAMENTE
        // por el documento en el visor nacional. Invertirlo atribuye la autoría a
        // la institución y la responsabilidad legal al médico, al revés de lo que
        // publica MinSalud, y en silencio.
        //
        // Lo cazó la revisión de seguridad de este mismo cambio. Lo sujeta ahora
        // `los_cuatro_ejes_del_documento_van_donde_dice_el_ejemplo`, que compara
        // contra el ARTEFACTO y no contra literales.
        "author": [Ref::a(i.id_institucion())],
        "confidentiality": "N",
        "attester": [{ "mode": "legal", "party": Ref::a(&profesional_id) }],
        "custodian": Ref::a(i.id_institucion()),
        "section": secciones,
    });

    // EL MISMO `event` QUE EL RDA DE PACIENTE, Y POR LA MISMA FUNCIÓN. Aquí
    // había una copia del bloque —un solo código, sin `period` y con la URL del
    // CodeSystem escrita a mano—, así que el defecto que el banco de
    // conformidad encontró en `paciente.rs` estaba DOS veces. Arreglar una copia
    // y dejar la otra es exactamente cómo un defecto sobrevive a su arreglo
    // (directiva 1: un solo sitio donde vive la regla).
    if let Some(ev) = super::paciente::evento(i) {
        composicion["event"] = serde_json::json!([ev]);
    }

    let mut entry = vec![super::paciente::Entrada { resource: composicion }];
    entry.extend(actores.into_iter().map(|resource| super::paciente::Entrada { resource }));
    // Y los recursos clínicos, tras los actores (#603 fase 4).
    entry.extend(
        recursos_clinicos.into_iter().map(|resource| super::paciente::Entrada { resource }),
    );

    Ok(super::paciente::Bundle {
        resource_type: "Bundle",
        tipo: "document",
        language: super::paciente::IDIOMA,
        entry,
    })
}

/// Construye los recursos clínicos del encuentro y dice qué sección puebla cada
/// uno (#603 fase 4). Acumula faltas: un dato que no está no se inventa (D20).
///
/// Qué recurso y qué sección por documento sale del PERFIL (medido con el banco de
/// conformidad), no de suponer:
///   · los tres → `Encounter` (`Composition.encounter`) y el diagnóstico de ingreso
///     (`Condition`), que en consulta es el principal;
///   · urgencias y hospitalización → además el diagnóstico de egreso (`Condition`)
///     y el pagador EAPB (`sectionPayers`);
///   · consulta y urgencias → la epicrisis (`sectionAddendumDocuments`), y pueblan
///     `sectionProblems` con sus diagnósticos.
fn recursos_del_encuentro(
    enc: Encuentro,
    i: &super::paciente::Insumo<'_>,
    paciente_id: &str,
    profesional_id: &str,
    f: &mut Faltas,
) -> (Vec<serde_json::Value>, std::collections::BTreeMap<&'static str, Vec<Ref>>) {
    use super::recursos;
    let mut recs: Vec<serde_json::Value> = Vec::new();
    let mut entradas: std::collections::BTreeMap<&'static str, Vec<Ref>> =
        std::collections::BTreeMap::new();

    // Los códigos LOINC de las secciones que se pueblan (los del ejemplo oficial).
    const SEC_PROBLEMAS: (&str, &str) = ("11450-4", "Problem list - Reported");
    const SEC_PAGADORES: (&str, &str) = ("48768-6", "Payment sources Document");
    const SEC_SOPORTES: (&str, &str) = ("55107-7", "Addendum Document");

    // 1) El/los Condition. El de INGRESO siempre (en consulta es el principal).
    let mut refs_problemas: Vec<Ref> = Vec::new();
    match &i.diagnostico_ingreso {
        Some(d) => {
            recs.push(recursos::condicion(recursos::ID_COND_INGRESO, d, paciente_id, f));
            refs_problemas.push(Ref::a(recursos::ID_COND_INGRESO));
        }
        None => f.0.push(super::Falta {
            recurso: "Encounter",
            campo: "diagnosis:AdmissionDiagnosis",
            porque: "sin diagnóstico de ingreso no hay encuentro clínico que transmitir",
        }),
    }
    if enc.tiene_egreso() {
        match &i.diagnostico_egreso {
            Some(d) => {
                recs.push(recursos::condicion(recursos::ID_COND_EGRESO, d, paciente_id, f));
                refs_problemas.push(Ref::a(recursos::ID_COND_EGRESO));
            }
            None => f.0.push(super::Falta {
                recurso: "Encounter",
                campo: "diagnosis:DischargeDiagnosis",
                porque: "egreso y urgencias exigen el diagnóstico de egreso (min=2 en el perfil)",
            }),
        }
    }

    // 2) El Encounter (referencia a los Condition ya construidos).
    recs.push(construir_encounter(enc, i, paciente_id, profesional_id, f));

    // 3) El pagador EAPB, para los documentos que exigen `sectionPayers`.
    if enc.tiene_egreso() {
        match &i.eps {
            Some(eps) => {
                recs.push(recursos::organizacion_eapb(eps, f));
                entradas.insert(SEC_PAGADORES.0, vec![Ref::a(&recursos::id_eapb(eps.codigo))]);
            }
            None => f.0.push(super::Falta {
                recurso: "Composition.section:sectionPayers",
                campo: "entry",
                porque: "urgencias y hospitalización exigen declarar la entidad responsable del pago",
            }),
        }
    }

    // 4) La epicrisis (`DocumentReference`), para consulta y urgencias.
    if matches!(enc, Encuentro::ConsultaExterna | Encuentro::Urgencias) {
        match i.epicrisis_id {
            Some(id) => {
                recs.push(recursos::documento_epi(id, paciente_id, i.id_institucion(), i.fecha, f));
                entradas.insert(SEC_SOPORTES.0, vec![Ref::a(recursos::ID_DOCREF)]);
            }
            None => f.0.push(super::Falta {
                recurso: "Composition.section:sectionAddendumDocuments",
                campo: "entry",
                porque: "consulta y urgencias exigen la epicrisis del encuentro como documento de soporte",
            }),
        }
    }

    // 5) `sectionProblems` la pueblan consulta y urgencias (hospitalización no la
    // exige poblada: sus diagnósticos viven en `Encounter.diagnosis`).
    if matches!(enc, Encuentro::ConsultaExterna | Encuentro::Urgencias) && !refs_problemas.is_empty()
    {
        entradas.insert(SEC_PROBLEMAS.0, refs_problemas);
    }

    let _ = (SEC_PROBLEMAS.1, SEC_PAGADORES.1, SEC_SOPORTES.1);
    (recs, entradas)
}

/// Las secciones de este encuentro que quedan VACÍAS con este insumo —las que el
/// encuentro NO puebla—, que son exactamente las elegibles para el aval del
/// `nilknown` (#603 F4.5-C). La pantalla de captura las ofrece como casillas.
///
/// **Reusa la MISMA lógica que `ensamblar`** (directiva 9): una sección es avalable
/// si y solo si `ensamblar` la mandaría por la puerta `seccion()` en vez de
/// `seccion_poblada()`, y eso lo decide `recursos_del_encuentro` (si su código está
/// en `entradas`, se pobló). No se reconstruye el criterio en la pantalla —hacerlo
/// sería una copia que envejece en cuanto cambie qué puebla un encuentro—. El
/// cómputo de recursos se descarta (solo interesa el conjunto de códigos poblados);
/// las faltas van a un acumulador de usar y tirar, porque aquí solo se pregunta qué
/// hay que avalar, no se ensambla nada.
pub fn secciones_avalables(enc: Encuentro, i: &super::paciente::Insumo<'_>) -> Vec<Seccion> {
    let paciente_id = i.id_paciente();
    let profesional_id = i.id_profesional();
    let mut descarte = Faltas::default();
    let (_recs, entradas) =
        recursos_del_encuentro(enc, i, &paciente_id, &profesional_id, &mut descarte);
    enc.secciones()
        .into_iter()
        .filter(|(code, _, _)| !entradas.contains_key(code))
        .collect()
}

/// La URL de la extensión que porta el tipo de diagnóstico en `Encounter.diagnosis`.
const EXT_DIAGNOSIS_TYPE: &str =
    "https://fhir.minsalud.gov.co/rda/StructureDefinition/ExtensionDiagnosisType";
/// El `system` del rol del diagnóstico (`diagnosis.use`).
const SISTEMA_DIAGNOSIS_ROLE: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianDiagnosisRole";

/// Una entrada de `Encounter.diagnosis`: la extensión con el tipo de diagnóstico,
/// la referencia al `Condition`, el ROL (fijo por rebanada) y el `rank`.
fn entrada_diagnosis(
    id: &str,
    tipo: Option<&str>,
    cond_id: &str,
    rol_code: &str,
    rol_display: &str,
    rank: u64,
    f: &mut Faltas,
) -> serde_json::Value {
    let tipo_coding = super::recursos::tipo_diagnostico(tipo, f);
    serde_json::json!({
        "id": id,
        "extension": [{ "url": EXT_DIAGNOSIS_TYPE, "valueCoding": tipo_coding }],
        "condition": Ref::a(cond_id),
        "use": { "coding": [{
            "system": SISTEMA_DIAGNOSIS_ROLE, "code": rol_code, "display": rol_display }] },
        "rank": rank,
    })
}

/// Arma el `Encounter` (`EncounterXxxRDA`) del encuentro. Sigue el ejemplo oficial
/// campo por campo —`type[].coding` es un OBJETO, no un array, como lo serializa
/// MinSalud— y acumula una falta por cada dato ausente (D20).
fn construir_encounter(
    enc: Encuentro,
    i: &super::paciente::Insumo<'_>,
    paciente_id: &str,
    profesional_id: &str,
    f: &mut Faltas,
) -> serde_json::Value {
    use super::recursos;
    use super::terminologia::SISTEMA_MODALIDAD;
    use serde_json::json;

    let perfil = match enc {
        Encuentro::ConsultaExterna => "EncounterAmbulatoryRDA",
        Encuentro::Urgencias => "EncounterEmergencyRDA",
        Encuentro::Hospitalizacion => "EncounterHospitalizationRDA",
    };
    let (class_code, class_display) = enc.class();
    let (grupo_code, grupo_display) = enc.grupo_encounter();
    let (part_id, part_code, part_display) = enc.participante();

    // La modalidad: el dato CAPTURADO manda (consulta lo trae de la historia); si no
    // hay, se DERIVA del tipo de encuentro (hosp/urgencias → intramural), y solo si
    // tampoco se deriva es Falta (consulta sin capturarla). Vacío se trata como
    // ausente para no colar un `''` al catálogo.
    let modalidad_code = i
        .modalidad
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| enc.modalidad_derivada(i.entorno));
    let modality = match modalidad_code {
        Some(m) => recursos::coding(
            SISTEMA_MODALIDAD, m, "Encounter", "type:encounterModality",
            "modalidad fuera del catálogo ColombianTechModality", f),
        None => {
            f.0.push(super::Falta { recurso: "Encounter", campo: "type:encounterModality",
                porque: "sin modalidad de atención no se puede construir el encuentro" });
            None
        }
    };
    let environment = match i.entorno {
        Some(e) => recursos::entorno(e, f),
        None => {
            f.0.push(super::Falta { recurso: "Encounter", campo: "type:encounterEnvironment",
                porque: "entorno de atención sin configurar (es config del prestador)" });
            None
        }
    };
    let reason = match i.causa_externa {
        Some(cc) => recursos::causa_externa(cc, f),
        None => {
            f.0.push(super::Falta { recurso: "Encounter", campo: "reasonCode",
                porque: "sin causa externa el perfil no admite el encuentro" });
            None
        }
    };
    let period = match i.periodo {
        Some((s, e)) if !s.trim().is_empty() && !e.trim().is_empty() => {
            Some(json!({ "start": s, "end": e }))
        }
        _ => {
            f.0.push(super::Falta { recurso: "Encounter", campo: "period",
                porque: "el encuentro exige inicio y fin de la atención con huso" });
            None
        }
    };

    // Los diagnósticos, con su tipo. En consulta uno (principal); en los otros dos.
    let tipo_ingreso = i.diagnostico_ingreso.as_ref().and_then(|d| d.tipo);
    let diagnosis: Vec<serde_json::Value> = if enc.tiene_egreso() {
        let tipo_egreso = i.diagnostico_egreso.as_ref().and_then(|d| d.tipo);
        vec![
            entrada_diagnosis("AdmissionDiagnosis", tipo_ingreso, recursos::ID_COND_INGRESO,
                "52870002", "diagnóstico de ingreso", 1, f),
            entrada_diagnosis("DischargeDiagnosis", tipo_egreso, recursos::ID_COND_EGRESO,
                "89100005", "diagnóstico final (alta)", 2, f),
        ]
    } else {
        vec![entrada_diagnosis("MainDiagnosis", tipo_ingreso, recursos::ID_COND_INGRESO,
            "8319008", "diagnóstico primario", 1, f)]
    };

    let mut encounter = json!({
        "resourceType": "Encounter",
        "id": recursos::ID_ENCOUNTER,
        "meta": Meta::perfil(perfil),
        "status": "finished",
        "class": {
            "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
            "code": class_code, "display": class_display },
        "type": [
            { "coding": modality },
            { "coding": {
                "system": "https://fhir.minsalud.gov.co/rda/CodeSystem/GrupoServicios",
                "code": grupo_code, "display": grupo_display } },
            { "coding": environment },
        ],
        "subject": Ref::a(paciente_id),
        "participant": [{
            "id": part_id,
            "type": { "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/v3-ParticipationType",
                "code": part_code, "display": part_display }] },
            "individual": Ref::a(profesional_id),
        }],
        "period": period,
        "reasonCode": [{ "coding": [reason] }],
        "diagnosis": diagnosis,
        "serviceProvider": Ref::a(i.id_institucion()),
    });

    // `serviceType` (CUPS): OBLIGATORIO en consulta externa, opcional en los otros.
    // Se emite solo en consulta —donde el perfil lo destaca— con el CUPS de la
    // consulta y su nombre; sin el dato, es una falta (D20).
    if matches!(enc, Encuentro::ConsultaExterna) {
        match i.servicio {
            Some(s) if !s.cups.trim().is_empty() && !s.display.trim().is_empty() => {
                encounter["serviceType"] = json!({ "coding": {
                    "system": "https://fhir.minsalud.gov.co/rda/CodeSystem/CUPS",
                    "code": s.cups, "display": s.display } });
            }
            _ => f.0.push(super::Falta { recurso: "Encounter", campo: "serviceType",
                porque: "la consulta externa exige el servicio prestado (CUPS) con su nombre" }),
        }
    }

    // El bloque `hospitalization` (`admitSource`/`dischargeDisposition`) solo donde
    // el perfil lo admite Y medico tiene los dos datos: hospitalización los tiene;
    // urgencias no captura la vía de ingreso, así que su bloque —OPCIONAL— se omite.
    if enc.admite_hospitalizacion()
        && let (Some(v), Some(d)) = (i.via_ingreso, i.destino_egreso)
    {
        let admit = recursos::via_ingreso(v, f);
        let disch = recursos::destino_egreso(d, f);
        encounter["hospitalization"] = json!({
            "admitSource": { "coding": [admit] },
            "dischargeDisposition": { "coding": [disch] },
        });
    }

    encounter
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ihce::paciente::Insumo;

    const CONSULTA: &str = include_str!("../../datos/ihce/ejemplos/enviar-rda-consulta-externa.json");
    const URGENCIAS: &str = include_str!("../../datos/ihce/ejemplos/enviar-rda-urgencias.json");
    const HOSPITAL: &str = include_str!("../../datos/ihce/ejemplos/enviar-rda-hospitalizacion.json");


    use crate::ihce::arnes::limpiar;

    /// El ejemplo oficial, por el arnés COMÚN (antes tenía copia propia).
    fn oficial(bruto: &str) -> serde_json::Value {
        crate::ihce::arnes::oficial(bruto)
    }

    fn composicion_oficial(bruto: &str) -> serde_json::Value {
        oficial(bruto)["entry"].as_array().unwrap().iter()
            .map(|e| e["resource"].clone())
            .find(|r| r["resourceType"] == "Composition")
            .expect("hay Composition")
    }

    fn insumo() -> Insumo<'static> {
        use crate::ihce::actores::{Codigo, Institucion, Nombre, Paciente, Profesional};
        use crate::ihce::paciente::{Diagnostico, Eps, Servicio};
        let nombre = Nombre {
            nombre1: "Primernombre", nombre2: Some("Segundonombre"),
            apellido1: "Primerapellido", apellido2: Some("Segundoapellido"),
        };
        Insumo {
            paciente: Paciente {
                tipo_doc: "CC", numero_doc: "000000000",
                tipo_doc_display: "Cédula ciudadanía",
                nombre,
                genero_fhir: "male",
                sexo_colombiano: Some(Codigo::nuevo("01", "Hombre")),
                nacimiento: "1999-01-01", hora_nacimiento: None,
                // El ejemplo oficial trae `deceasedBoolean: false` en los
                // CUATRO documentos. En egreso y urgencias el ensamblador lo
                // exige declarado (ver `ensamblar`), y ésta es la fixture de
                // los tres encuentros: el paciente del ejemplo salió vivo.
                fallecido: Some(false),
                municipio: Some(Codigo::nuevo("11001", "Bogotá D.C.")),
                pais: Some(Codigo::nuevo("170", "Colombia")),
                zona: Some(Codigo::nuevo("01", "Urbana")),
                nacionalidad: None, etnia: None, discapacidad: None, identidad_genero: None,
            },
            profesional: Profesional {
                tipo_doc: "CC", numero_doc: "111111111",
                tipo_doc_display: "Cédula ciudadanía", nombre,
            },
            institucion: Institucion { codigo_habilitacion: "0500100001", nit: "900123456" },
            fecha: "2026-08-14T18:42:00-05:00",
            grupo_servicios: Some("01"),
            periodo: Some(("2026-08-14T18:00:00-05:00", "2026-08-14T18:42:00-05:00")),
            modalidad: Some("01"),
            // La puerta del nilknown YA está cableada en clinico (#603, directiva
            // 9): las secciones nacen vacías, así que la fixture las confirma —el
            // aval humano que en producción da un clínico— para que el ensamblado
            // proceda. Sin esto, `ensamblar` se niega (y así lo prueba el caso rojo).
            aval: crate::ihce::Aval::nuevo(&CONFIRMACIONES),
            // Los datos del ENCUENTRO CLÍNICO (#603 fase 4), tomados del ejemplo
            // oficial. `A000` deriva CIE-11 con `cie11::par`; el tipo de diagnóstico
            // lo trae la consulta (`historia.tipo_diagnostico`) y aquí se modela para
            // urgencias/hospitalización, que en producción NO lo tienen (queda Falta).
            entorno: Some("05"),
            causa_externa: Some("22"),
            diagnostico_ingreso: Some(Diagnostico {
                cie10: "A000",
                display: "Colera debido a Vibrio cholerae 01, biotipo cholerae",
                tipo: Some("02"),
            }),
            diagnostico_egreso: Some(Diagnostico {
                cie10: "A000",
                display: "Colera debido a Vibrio cholerae 01, biotipo cholerae",
                tipo: Some("01"),
            }),
            via_ingreso: Some("01"),
            destino_egreso: Some("01"),
            eps: Some(Eps { codigo: "EPS016", nombre: "SALUD TOTAL EPS S.A." }),
            epicrisis_id: Some("EPI-000000001"),
            servicio: Some(Servicio {
                cups: "890201",
                display: "CONSULTA DE PRIMERA VEZ POR MEDICINA GENERAL",
            }),
        }
    }

    fn generada(e: Encuentro) -> serde_json::Value {
        ensamblar(e, &insumo()).unwrap().entry.into_iter().next().unwrap().resource
    }

    /// **El positivo de control del ARNÉS**, y sin él lo de abajo no vale nada:
    /// si el limpiador se comiera los `//` de las URL, el oráculo saldría con
    /// `"https:"` truncado y las comparaciones medirían basura contra basura.
    #[test]
    fn el_limpiador_no_destroza_las_url() {
        let t = r#"{"a": "https://fhir.minsalud.gov.co/rda", // esto sí es comentario
                    "b": 1}"#;
        let v: serde_json::Value = serde_json::from_str(&limpiar(t)).unwrap();
        assert_eq!(v["a"], "https://fhir.minsalud.gov.co/rda");
        assert_eq!(v["b"], 1);
    }

    /// #603 (modalidad): hospitalización y urgencias DERIVAN la modalidad (intramural
    /// `01`) **solo con entorno Institucional (`05`)**; consulta NO —la suya se captura—.
    /// Tres casos, tres pares D33:
    ///   · verde: hosp/urg con entorno `05` → `encounterModality=01` en el JSON emitido;
    ///   · ROJO del entorno (lo cazó el concilio): hosp con entorno `01` (Hogar) NO
    ///     deriva → Falta, porque emitir «Hogar + Intramural» sería un dato falso;
    ///   · rojo de consulta: sin modalidad → Falta, no se deriva.
    /// Mutar `modalidad_derivada` para ignorar el entorno rompe el rojo del entorno;
    /// para derivar en consulta, el de consulta; para no derivar, los verdes. Cruza la
    /// frontera del programa (D28): mira el JSON emitido, no el struct.
    #[test]
    fn la_modalidad_se_deriva_para_hosp_y_urg_pero_no_para_consulta() {
        let modalidad_emitida = |bundle: &crate::ihce::paciente::Bundle| -> Option<String> {
            bundle.entry.iter().find_map(|e| {
                let r = &e.resource;
                if r["resourceType"] != "Encounter" {
                    return None;
                }
                r["type"].as_array()?.iter().find_map(|t| {
                    let c = &t["coding"];
                    let sys = c["system"].as_str().or_else(|| c[0]["system"].as_str())?;
                    if sys.ends_with("ColombianTechModality") {
                        c["code"].as_str().or_else(|| c[0]["code"].as_str()).map(str::to_string)
                    } else {
                        None
                    }
                })
            })
        };
        // Verde: hosp y urgencias sin modalidad, con entorno Institucional (05) → `01`.
        for e in [Encuentro::Hospitalizacion, Encuentro::Urgencias] {
            let mut i = insumo(); // insumo() trae entorno = Some("05")
            i.modalidad = None;
            let b = ensamblar(e, &i).expect("hosp/urg institucional deben ensamblar con modalidad derivada");
            assert_eq!(
                modalidad_emitida(&b).as_deref(),
                Some("01"),
                "{e:?}: con entorno institucional la modalidad debe derivarse intramural (01)"
            );
        }
        // ROJO del entorno (par D33, hallazgo del concilio): hosp con entorno `01`
        // (Hogar) NO deriva → Falta. Emitir «Hogar + Intramural» sería un dato falso.
        let mut i = insumo();
        i.modalidad = None;
        i.entorno = Some("01"); // Hogar: despliegue extramural que medico permite
        let err = ensamblar(Encuentro::Hospitalizacion, &i)
            .expect_err("con entorno no-institucional la modalidad NO se deriva: debe faltar");
        assert!(
            format!("{err}").contains("encounterModality"),
            "hosp con entorno Hogar debe faltar la modalidad (no derivar intramural): {err}"
        );
        // Rojo de consulta: sin modalidad NO deriva → Falta (aunque el entorno sea 05).
        let mut i = insumo();
        i.modalidad = None;
        let err = ensamblar(Encuentro::ConsultaExterna, &i).unwrap_err();
        assert!(
            format!("{err}").contains("encounterModality"),
            "consulta sin modalidad debe faltar (no se deriva): {err}"
        );
    }

    fn casos() -> [(Encuentro, &'static str, usize); 3] {
        [(Encuentro::ConsultaExterna, CONSULTA, 16),
         (Encuentro::Urgencias, URGENCIAS, 24),
         (Encuentro::Hospitalizacion, HOSPITAL, 23)]
    }

    #[test]
    fn los_tres_ejemplos_son_bundle_document() {
        for (_, bruto, recursos) in casos() {
            let e = oficial(bruto);
            assert_eq!(e["resourceType"], "Bundle");
            assert_eq!(e["type"], "document");
            assert_eq!(e["entry"].as_array().unwrap().len(), recursos);
        }
    }

    #[test]
    fn el_perfil_y_el_tipo_son_los_del_ejemplo() {
        for (enc, bruto, _) in casos() {
            let g = generada(enc);
            let o = composicion_oficial(bruto);
            assert_eq!(g["meta"]["profile"][0], o["meta"]["profile"][0], "{enc:?}");
            assert_eq!(g["type"]["coding"][0]["code"], o["type"]["coding"][0]["code"], "{enc:?}");
        }
    }

    /// LOS CUATRO EJES DEL DOCUMENTO, contra el ejemplo y no contra literales
    /// míos — que es lo que faltaba y por donde entró un defecto real.
    ///
    /// Hasta el 2026-08-14 este archivo tenía `author` y `attester.party`
    /// INTERCAMBIADOS: se copió el patrón de `paciente.rs`, donde el autor es la
    /// persona, sin comprobar que en los tres RDA de encuentro es al revés. Las
    /// pruebas de aquí miraban `meta.profile`, `type.coding[0].code` y las
    /// secciones, y **ninguna miraba estos cuatro campos**, así que el defecto
    /// pasaba con todo en verde.
    ///
    /// No dan error: `attester.mode = "legal"` nombra a quien responde
    /// jurídicamente por el documento en el visor nacional, y el servidor acepta
    /// cualquiera de los dos órdenes. Es la misma clase que la sección con el
    /// código equivocado.
    ///
    /// Se compara contra el ARTEFACTO. Si mañana MinSalud cambia la convención,
    /// esto se pone rojo en vez de seguir afirmando lo de hoy.
    #[test]
    fn los_cuatro_ejes_del_documento_van_donde_dice_el_ejemplo() {
        for (enc, bruto, _) in casos() {
            let g = generada(enc);
            let o = composicion_oficial(bruto);
            for eje in ["subject", "custodian"] {
                assert_eq!(
                    g[eje]["reference"].as_str().map(quien),
                    o[eje]["reference"].as_str().map(quien),
                    "{enc:?} · {eje}"
                );
            }
            assert_eq!(
                g["author"][0]["reference"].as_str().map(quien),
                o["author"][0]["reference"].as_str().map(quien),
                "{enc:?} · author — en el encuentro clínico el autor es la INSTITUCIÓN"
            );
            assert_eq!(
                g["attester"][0]["party"]["reference"].as_str().map(quien),
                o["attester"][0]["party"]["reference"].as_str().map(quien),
                "{enc:?} · attester.party — quien atesta legalmente es el PROFESIONAL"
            );
            assert_eq!(g["attester"][0]["mode"], o["attester"][0]["mode"], "{enc:?}");
        }
    }

    /// Los identificadores del ejemplo y los nuestros son distintos —`#CC-1111111`
    /// frente al documento real—, así que comparar la cadena entera mediría el
    /// dato de prueba. Lo que hay que comparar es **de qué CLASE es cada
    /// referencia**: persona o institución.
    fn quien(r: &str) -> &'static str {
        if r.starts_with("#CC-") || r.starts_with("#TI-") || r.starts_with("#CE-") {
            "persona"
        } else {
            "institucion"
        }
    }

    /// Y su PAREJA: sin esto pasaría igual un `quien` que devolviera lo mismo
    /// siempre, y la prueba de arriba no distinguiría nada.
    #[test]
    fn la_clase_de_referencia_discrimina() {
        assert_eq!(quien("#CC-111111111"), "persona");
        assert_eq!(quien("#Códigohabilitaciónprestador"), "institucion");
        assert_ne!(quien("#CC-1"), quien("#0500100001"));
    }

    /// `Composition.title` es **1..1 en FHIR R4**, o sea obligatorio. Faltaba
    /// entero en este archivo mientras `paciente.rs` sí lo emitía y además tenía
    /// su prueba — la asimetría es lo que lo delató.
    #[test]
    fn el_titulo_es_el_del_ejemplo() {
        for (enc, bruto, _) in casos() {
            let g = generada(enc);
            let o = composicion_oficial(bruto);
            assert!(!o["title"].as_str().unwrap_or("").is_empty(), "el ejemplo trae título");
            assert_eq!(g["title"], o["title"], "{enc:?}");
        }
    }

    /// La que más vale: **cada sección, en su orden, con su código y su título**.
    /// Una sección mal codificada no da error; el servidor la acepta y guarda el
    /// contenido donde no va.
    #[test]
    fn todas_las_secciones_coinciden_con_el_ejemplo() {
        for (enc, bruto, _) in casos() {
            let g = generada(enc);
            let o = composicion_oficial(bruto);
            let gs = g["section"].as_array().unwrap();
            let os = o["section"].as_array().unwrap();
            assert_eq!(gs.len(), os.len(), "{enc:?}: el ejemplo tiene {} secciones", os.len());
            for (n, (a, b)) in gs.iter().zip(os.iter()).enumerate() {
                assert_eq!(a["code"]["coding"][0]["code"], b["code"]["coding"][0]["code"],
                           "{enc:?} sección {n}: código LOINC distinto");
                assert_eq!(a["title"], b["title"], "{enc:?} sección {n}: título distinto");
            }
        }
    }

    /// Los tres comparten las SIETE primeras. Si alguna vez divergen, esto lo
    /// dice antes de que alguien lo descubra depurando un rechazo.
    #[test]
    fn los_tres_comparten_las_siete_primeras_secciones() {
        let cod = |e: Encuentro| -> Vec<&str> {
            e.secciones().into_iter().take(7).map(|(c, _, _)| c).collect()
        };
        assert_eq!(cod(Encuentro::ConsultaExterna), cod(Encuentro::Urgencias));
        assert_eq!(cod(Encuentro::Urgencias), cod(Encuentro::Hospitalizacion));
    }

    /// El triaje es de urgencias y **solo** de urgencias. La pareja va incluida:
    /// sin el `assert!(!…)` pasaría igual una función que se lo pone a todos.
    #[test]
    fn el_triaje_es_solo_de_urgencias() {
        let tiene = |e: Encuentro| e.secciones().iter().any(|(c, _, _)| *c == TRIAJE.0);
        assert!(tiene(Encuentro::Urgencias));
        assert!(!tiene(Encuentro::ConsultaExterna));
        assert!(!tiene(Encuentro::Hospitalizacion));
    }

    /// Y la consulta externa NO lleva procedimientos ni resultados: es lo que la
    /// hace más liviana, y confundirlo metería secciones vacías en cada consulta.
    #[test]
    fn la_consulta_externa_no_lleva_procedimientos_ni_resultados() {
        let s = Encuentro::ConsultaExterna.secciones();
        assert!(!s.iter().any(|(c, _, _)| *c == PROCEDIMIENTOS.0));
        assert!(!s.iter().any(|(c, _, _)| *c == RESULTADOS.0));
        // La pareja: los otros dos SÍ las llevan.
        for e in [Encuentro::Urgencias, Encuentro::Hospitalizacion] {
            assert!(e.secciones().iter().any(|(c, _, _)| *c == PROCEDIMIENTOS.0), "{e:?}");
        }
    }

    #[test]
    fn sin_fecha_no_hay_documento_en_ninguno_de_los_tres() {
        for (enc, _, _) in casos() {
            let mut i = insumo();
            i.fecha = "";
            let e = ensamblar(enc, &i).expect_err("tiene que negarse");
            assert!(e.to_string().contains("Composition.date"), "{enc:?}: {e}");
        }
    }

    /// EN EGRESO Y EN URGENCIAS, «NO SÉ SI MURIÓ» NO ES UNA RESPUESTA.
    ///
    /// El arreglo anterior cambió `deceasedBoolean` por `Option<bool>` y lo dio
    /// por cerrado. No lo estaba: `None` es lo que los lectores de FHIR leen
    /// como «vive», y nada exigía declararlo justo en los dos documentos donde
    /// medico SÍ conoce el desenlace (`hospitalizacion.estado` = `DEFUNCION`,
    /// `atencion_urgencias.destino_salida` = `FALLECIDO`).
    #[test]
    fn en_egreso_y_urgencias_el_desenlace_se_declara() {
        for e in [Encuentro::Hospitalizacion, Encuentro::Urgencias] {
            let mut i = insumo();
            i.paciente.fallecido = None;
            let f = ensamblar(e, &i).expect_err("sin declaración no puede pasar");
            assert!(
                f.0.iter().any(|x| x.campo == "deceasedBoolean"),
                "{e:?} debería exigir el desenlace: {f}"
            );
        }
    }

    /// LA PAREJA, y es la mitad que hace que la de arriba signifique algo: en
    /// consulta externa y en el RDA de paciente nadie conoce el desenlace, así
    /// que `None` es CORRECTO y exigirlo ahí sería inventar (directiva 21: se
    /// rechaza lo imposible, no lo desconocido).
    #[test]
    fn en_consulta_externa_no_se_exige_lo_que_nadie_sabe() {
        let mut i = insumo();
        i.paciente.fallecido = None;
        ensamblar(Encuentro::ConsultaExterna, &i)
            .expect("en consulta externa nadie sabe si el paciente murió después");
    }

    /// **LA PUERTA DEL `nilknown` EN EL RDA CLÍNICO** (#603, la misma que paciente).
    /// Sin confirmar las secciones vacías, el ensamblado se NIEGA en vez de afirmar
    /// «Nil Known» por defecto: cada sección vacía es una falta del `emptyReason`.
    #[test]
    fn sin_confirmar_ninguna_seccion_no_emite() {
        for (enc, _, _) in casos() {
            let mut i = insumo();
            i.aval = crate::ihce::Aval::ninguno();
            let f = ensamblar(enc, &i).expect_err("sin aval humano no puede emitir nilknown");
            assert!(
                f.0.iter().any(|x| x.campo == "emptyReason"),
                "{enc:?} debería fallar por la puerta del emptyReason: {f}"
            );
        }
    }

    /// **`secciones_avalables` son EXACTAMENTE las que el ensamblado pide avalar**
    /// (#603 F4.5-C, round-trip D33): las que el encuentro POBLA no aparecen, y
    /// avalar justo las avalables ensambla —ni sobra ni falta—; quitar una deja
    /// exactamente una falta de `emptyReason`. Sin esto, la pantalla podría ofrecer
    /// avalar secciones ya pobladas (inocuo pero confuso) o dejar fuera una que sí
    /// hace falta (el RDA nunca saldría).
    #[test]
    fn las_avalables_son_exactamente_las_que_el_ensamblado_pide() {
        let base = insumo();
        let avalables = secciones_avalables(Encuentro::ConsultaExterna, &base);
        // Las que el encuentro pobla (dx→problemas, epicrisis→soportes) NO aparecen.
        for poblada in ["11450-4", "55107-7"] {
            assert!(
                !avalables.iter().any(|(c, _, _)| *c == poblada),
                "{poblada} se pobla, no es avalable"
            );
        }
        assert!(!avalables.is_empty(), "una consulta deja secciones vacías por avalar");
        // Avalar EXACTAMENTE las avalables ensambla.
        let confs: Vec<crate::ihce::Confirmacion> = avalables.iter().map(|s| conf(s.0)).collect();
        let mut i = insumo();
        i.aval = crate::ihce::Aval::nuevo(&confs);
        assert!(
            ensamblar(Encuentro::ConsultaExterna, &i).is_ok(),
            "avalar las avalables debe ensamblar la consulta"
        );
        // Quitar una deja exactamente una sección sin avalar (la pareja roja).
        let confs_menos = &confs[..confs.len() - 1];
        let mut j = insumo();
        j.aval = crate::ihce::Aval::nuevo(confs_menos);
        let err = ensamblar(Encuentro::ConsultaExterna, &j).expect_err("falta una por avalar");
        assert_eq!(
            err.0.iter().filter(|x| x.campo == "emptyReason").count(),
            1,
            "exactamente una sección quedó sin avalar"
        );
    }

    /// Y su PAREJA: confirmando las secciones, el documento SÍ sale y cada sección
    /// VACÍA lleva su `emptyReason=nilknown`. Desde FASE 4 (#603) las secciones que
    /// el encuentro POBLA —`sectionProblems`, `sectionPayers`,
    /// `sectionAddendumDocuments`— llevan `entry` y NO `emptyReason`: una sección con
    /// contenido no afirma «Nil Known». Así que la comprobación es: toda sección o
    /// bien tiene `entry`, o bien su `emptyReason` es `nilknown` —nunca ninguno de
    /// los dos, nunca los dos (cmp-2)—.
    #[test]
    fn confirmadas_las_secciones_el_documento_emite_nilknown() {
        for (enc, _, _) in casos() {
            let g = generada(enc); // la fixture confirma TODAS_LAS_SECCIONES
            let secs = g["section"].as_array().unwrap();
            for s in secs {
                let tiene_entry = s.get("entry").is_some();
                let nilknown = s["emptyReason"]["coding"][0]["code"] == "nilknown";
                assert!(
                    tiene_entry ^ nilknown,
                    "{enc:?}: cada sección es o POBLADA (entry) o VACÍA confirmada \
                     (nilknown), nunca ambas ni ninguna: {s}"
                );
            }
            // Y al menos UNA poblada, si no esto pasaría con todo vacío (control).
            assert!(
                secs.iter().any(|s| s.get("entry").is_some()),
                "{enc:?}: fase 4 puebla al menos una sección"
            );
        }
    }

    /// Y que cuando SÍ se declara la muerte, el documento lo diga — no basta
    /// con exigir el campo si luego no viaja.
    #[test]
    fn cuando_el_egreso_es_por_defuncion_el_documento_lo_dice() {
        let mut i = insumo();
        i.paciente.fallecido = Some(true);
        let b = ensamblar(Encuentro::Hospitalizacion, &i).expect("válido");
        let j = serde_json::to_value(&b).expect("serializa");
        let s = j.to_string();
        assert!(s.contains("\"deceasedBoolean\":true"), "el desenlace no viajó: {s}");
    }
}
