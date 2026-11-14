// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza
//! **RDA de paciente**: los antecedentes que el paciente manifiesta.
//!
//! Es el más pequeño de los cuatro —8 recursos frente a los 38 perfiles de
//! urgencias— y por eso se construye primero: fija el patrón que los otros tres
//! reutilizan.
//!
//! # Cuándo se emite
//!
//! Tras cualquier evento clínico, con lo que el paciente declara: diagnósticos
//! conocidos, alergias, medicamentos que toma y antecedentes familiares. **No es
//! la historia clínica**: es lo manifestado, y el perfil lo dice en su nombre —
//! `CompositionPatientStatementRDA`.
//!
//! # La estructura, leída del ejemplo oficial y no supuesta
//!
//! ```text
//! Bundle (type: document)
//!  ├─ Composition   perfil CompositionPatientStatementRDA · LOINC 102089-0
//!  │   ├─ subject   -> #<tipoDoc>-<numero>      el paciente
//!  │   ├─ author    -> #<tipoDoc>-<numero>      el profesional
//!  │   ├─ attester  -> #<código de habilitación>   modo «legal»
//!  │   ├─ custodian -> #<código de habilitación>
//!  │   └─ section[4]  diagnósticos · alergias · medicamentos · familiares
//!  ├─ Patient · Organization · Practitioner
//!  └─ Condition · AllergyIntolerance · MedicationStatement · FamilyMemberHistory
//! ```
//!
//! Los cuatro códigos LOINC de sección salen del ejemplo, byte a byte. No se
//! deducen: una sección con el código equivocado la acepta el servidor y la
//! guarda en el sitio que no es.

use super::{Concepto, Faltas, Meta, Ref};
use serde::Serialize;

/// Los cuatro códigos de sección, tal como los trae el ejemplo oficial.
///
/// Van como constantes con su display exacto porque **son el contrato**: el
/// servidor los usa para saber qué es cada sección, y una errata aquí no da
/// error — guarda las alergias donde van los diagnósticos.
pub const SEC_DIAGNOSTICOS: (&str, &str) = ("11450-4", "Problem list - Reported");
pub const SEC_ALERGIAS: (&str, &str) = ("48765-2", "Allergies and adverse reactions Document");
pub const SEC_MEDICAMENTOS: (&str, &str) = ("10160-0", "History of Medication use Narrative");
pub const SEC_FAMILIARES: (&str, &str) = ("10157-6", "History of family member diseases Narrative");

/// Las cuatro secciones del RDA de paciente AVALADAS por un clínico, para las
/// fixtures (F4.5). Cada una nombra usuario y sello, como exige [`super::Aval`];
/// el orden es el de las `SEC_*`, así que `&CONFIRMACIONES[..3]` modela «revisó
/// tres y le faltó una».
///
/// **`pub(crate)` y `#[cfg(test)]` a propósito** (observación de la revisión de
/// seguridad del 2026-08-23): NO puede ser un asa pública que un llamante externo
/// —medico-api al cablear la captura— use para «confirmar las cuatro de golpe» y
/// desactivar la puerta del `nilknown`. En producción el aval se construye desde
/// el acto real del clínico, sección a sección con su usuario y su sello; esta
/// lista es solo para las FIXTURES internas que modelan «un clínico las revisó
/// todas», y no debe existir en el binario.
#[cfg(test)]
pub(crate) const CONFIRMACIONES: [super::Confirmacion<'static>; 4] = [
    super::Confirmacion { seccion: SEC_DIAGNOSTICOS.0, usuario_id: 7, sello: "2026-08-14T18:42:00-05:00" },
    super::Confirmacion { seccion: SEC_ALERGIAS.0, usuario_id: 7, sello: "2026-08-14T18:42:00-05:00" },
    super::Confirmacion { seccion: SEC_MEDICAMENTOS.0, usuario_id: 7, sello: "2026-08-14T18:42:00-05:00" },
    super::Confirmacion { seccion: SEC_FAMILIARES.0, usuario_id: 7, sello: "2026-08-14T18:42:00-05:00" },
];

/// El tipo de documento del RDA de paciente.
///
/// El `display` es **literal del ejemplo**, sin el sufijo `Document` que tenía
/// hasta el 2026-08-15: se lo había añadido yo y no lo vio nadie porque la
/// prueba comparaba `system` y `code` y **nunca el `display`**. Lo cazó la
/// revisión de seguridad; ahora se compara entero.
pub const TIPO_DOCUMENTO: (&str, &str) =
    ("102089-0", "FHIR resource patient medical record");

/// El título, literal del ejemplo. No se traduce ni se abrevia: viaja al visor
/// nacional y es lo que ve un médico en otra institución.
pub const TITULO: &str =
    "Resumen Digital de Atención en Salud - RDA de antecedentes manifestados por el paciente";

/// Un diagnóstico del encuentro (#603 fase 4): el CIE-10 con su nombre y el tipo.
///
/// El **CIE-11 no viaja aquí**: se DERIVA del CIE-10 con [`crate::cie11::par`], la
/// misma regla y desde la misma tabla que el RIPS, para no tener dos códigos
/// distintos para el mismo diagnóstico. El `display` es el nombre oficial del
/// CIE-10 (`cie10.descripcion` en `medico-datos`), que el `Condition` exige
/// (`code.coding:ICD10.display`, `min=1`) y que el RIPS no emite.
///
/// **`tipo` es `Option`** (`RIPSTipoDiagnosticoPrincipalVersion2`): la consulta
/// externa lo tiene (`historia.tipo_diagnostico`), pero urgencias y
/// hospitalización NO —ni la Resolución 948 lo pide para AU/AH—. `None` significa
/// «no capturado», y el ensamblador lo trata como FALTA en los encuentros cuyo
/// `Encounter` lo exige (`ExtensionDiagnosisType`, `min=1`): no se inventa un tipo
/// de diagnóstico, que es un dato clínico (directiva 20).
#[derive(Debug, Clone, Copy)]
pub struct Diagnostico<'a> {
    pub cie10: &'a str,
    pub display: &'a str,
    pub tipo: Option<&'a str>,
}

impl Diagnostico<'_> {
    fn recortado(&self) -> Self {
        Self { cie10: self.cie10.trim(), display: self.display.trim(), tipo: self.tipo.map(str::trim) }
    }
}

/// La entidad responsable del pago (EAPB), para el pagador del RDA clínico
/// (`sectionPayers` → `HealthBenefitPlanAdminOrganizationRDA`). Sale de la tabla
/// `entidad` de `medico-datos` (`codigo_eps` + `nombre`).
#[derive(Debug, Clone, Copy)]
pub struct Eps<'a> {
    pub codigo: &'a str,
    pub nombre: &'a str,
}

impl Eps<'_> {
    fn recortado(&self) -> Self {
        Self { codigo: self.codigo.trim(), nombre: self.nombre.trim() }
    }
}

/// El servicio prestado (CUPS), para `Encounter.serviceType`. Es OBLIGATORIO en
/// consulta externa (`min=1` en `EncounterAmbulatoryRDA`) y opcional en los otros
/// dos. Sale del CUPS de la consulta (`historia`, `cod_consulta_cups`); el `display`
/// es el nombre del CUPS (tabla `cups` de `medico-datos`), que el perfil exige y el
/// RIPS no emite —el `system` es `CodeSystem/CUPS`, un catálogo grande no
/// vendorizado, así que el `display` no se puede derivar y viaja con el dato—.
#[derive(Debug, Clone, Copy)]
pub struct Servicio<'a> {
    pub cups: &'a str,
    pub display: &'a str,
}

impl Servicio<'_> {
    fn recortado(&self) -> Self {
        Self { cups: self.cups.trim(), display: self.display.trim() }
    }
}

/// Lo que hace falta para ensamblar el RDA de paciente.
///
/// Todo `&str` y todo obligatorio salvo lo que diga lo contrario: si algo puede
/// faltar de verdad, se declara `Option` y se justifica. Un campo que se rellena
/// con `""` por comodidad es un dato inventado con otro nombre.
///
/// **Los campos clínicos (`entorno`…`epicrisis_id`) los usa SOLO el ensamblador
/// de encuentro** (`super::clinico`): el RDA de paciente los ignora. Van aquí y no
/// en un insumo aparte porque los cuatro documentos comparten actores, fecha y
/// secciones, y partir el tipo duplicaría esa mitad (directiva 9). Todos `Option`:
/// un encuentro que no traiga el dato NO emite el recurso que lo necesita —el
/// ensamblador devuelve `Faltas`, no inventa (directiva 20)—.
#[derive(Debug, Clone, Copy)]
pub struct Insumo<'a> {
    /// El paciente, entero. **Ya no son dos `&str` sueltos**, y el cambio es la
    /// razón de que el Bundle deje de estar incompleto: de aquí sale tanto la
    /// referencia `#CC-000000000` como el recurso `Patient` al que apunta. Con
    /// los identificadores sueltos se podía escribir la referencia sin tener el
    /// recurso —y eso es exactamente lo que pasó hasta hoy.
    pub paciente: super::actores::Paciente<'a>,
    /// El profesional que responde por el resumen.
    pub profesional: super::actores::Profesional<'a>,
    /// La sede prestadora: quien custodia y atesta.
    pub institucion: super::actores::Institucion<'a>,
    /// Momento del documento, en ISO 8601 **con huso**. Sin huso, un RDA de las
    /// 23:30 se guarda con la fecha del día siguiente en el servidor.
    pub fecha: &'a str,
    /// Modalidad de la atención (`ColombianTechModality`), solo el CÓDIGO.
    ///
    /// El `display` no se recibe: se **deriva** del CodeSystem vendorizado
    /// (`super::terminologia`). El servidor nacional valida `system`+`code` y se
    /// traga el `display` que le manden, así que recibirlo de fuera es abrirle
    /// la puerta a un dato falso que nadie detecta — la lección del
    /// `tableta = 1` del CUM.
    pub modalidad: Option<&'a str>,
    /// Grupo de servicios (`GrupoServicios`), solo el CÓDIGO. Igual que la
    /// modalidad: el `display` se deriva.
    ///
    /// **El perfil lo EXIGE**: `Composition.event.code` es `min=2 max=2` —
    /// modalidad Y grupo—, y hasta el 2026-08-16 medico emitía uno solo. Lo
    /// destapó el banco de conformidad contra el IG vendorizado, no una prueba
    /// propia: el ejemplo oficial sí trae los dos.
    pub grupo_servicios: Option<&'a str>,
    /// El periodo del evento, `(inicio, fin)` en ISO 8601 **con huso**.
    ///
    /// También lo exige el perfil (`Composition.event.period` con `start` y
    /// `end`, los tres `min=1`). No se deriva de `fecha`: el documento se emite
    /// en un instante y la atención DURA — inventar el fin a partir del sello del
    /// documento sería afirmar una duración que nadie midió (directiva 20).
    pub periodo: Option<(&'a str, &'a str)>,
    /// El aval de las secciones que un CLÍNICO confirmó vacías —«revisé, no hay
    /// nada conocido»—. **Solo las avaladas emiten `emptyReason=nilknown`**: una
    /// sección vacía que NADIE confirmó hace FALLAR el ensamblado, en vez de
    /// afirmar «Nil Known» por defecto de máquina (la puerta clínica de #603,
    /// decidida por Juan el 2026-08-23). El perfil de MinSalud fija la razón a
    /// `nilknown`, así que la única forma honesta de emitirla es el aval humano
    /// por sección — no una frase predeterminada que se rubrica sin leer.
    ///
    /// Es un [`super::Aval`] y no un `&[&str]` (F4.5, requisito #1): avalar exige
    /// nombrar quién confirmó y cuándo, por sección; pasar códigos sueltos no
    /// compila. `Aval::ninguno()` = nadie confirmó → toda sección vacía es falta.
    pub aval: super::Aval<'a>,

    // ── Campos del ENCUENTRO CLÍNICO (#603 fase 4). Solo `super::clinico` los lee.

    /// Entorno de la atención (`EntornoAtencion`), solo el CÓDIGO; el `display` se
    /// deriva. Es propiedad de la SEDE, no del paciente, así que en producción sale
    /// de la **configuración del prestador** (decisión de Juan, 2026-08-29), como la
    /// zona horaria. `None` = sin configurar → el `Encounter` no se emite.
    pub entorno: Option<&'a str>,
    /// Causa externa (`RIPSCausaExternaVersion2`), solo el código (`Encounter.reasonCode`).
    pub causa_externa: Option<&'a str>,
    /// Diagnóstico de ingreso: en consulta externa es el diagnóstico PRINCIPAL
    /// (`MainDiagnosis`); en urgencias y hospitalización, el `AdmissionDiagnosis`.
    pub diagnostico_ingreso: Option<Diagnostico<'a>>,
    /// Diagnóstico de egreso (`DischargeDiagnosis`), solo urgencias y hospitalización.
    pub diagnostico_egreso: Option<Diagnostico<'a>>,
    /// Vía de ingreso (`ViaIngreso`), solo el código (`hospitalization.admitSource`).
    /// Solo hospitalización lo tiene en medico; urgencias no.
    pub via_ingreso: Option<&'a str>,
    /// Condición y destino al egreso (`CondicionyDestinoUsuarioEgreso`), solo el
    /// código (`hospitalization.dischargeDisposition`).
    pub destino_egreso: Option<&'a str>,
    /// La EAPB responsable del pago, para el pagador (`sectionPayers`).
    pub eps: Option<Eps<'a>>,
    /// El identificador de la epicrisis del encuentro, para el `masterIdentifier`
    /// del `DocumentReference` de la sección de documentos de soporte.
    pub epicrisis_id: Option<&'a str>,
    /// El servicio prestado (CUPS), para `Encounter.serviceType` — obligatorio en
    /// consulta externa.
    pub servicio: Option<Servicio<'a>>,
}

impl<'a> Insumo<'a> {
    /// El `id` interno del paciente dentro del Bundle.
    pub fn id_paciente(&self) -> String {
        super::actores::id_persona(self.paciente.tipo_doc, self.paciente.numero_doc)
    }
    /// El `id` interno del profesional.
    pub fn id_profesional(&self) -> String {
        super::actores::id_persona(self.profesional.tipo_doc, self.profesional.numero_doc)
    }
    /// El `id` interno de la sede: el código de habilitación **recortado**.
    ///
    /// El `.trim()` NO es cosmético y no se puede quitar: este valor tiene que
    /// ser IDÉNTICO al `Organization.id` que emite `Institucion::a_json`, que
    /// sale recortado desde `Insumo::recortado`. Si los dos lados dejan de
    /// coincidir, `custodian` y `attester.party` apuntan a un recurso que no
    /// existe en el Bundle — y eso es justo lo que pasó al introducir el borde
    /// (ver la nota de `recortado`).
    pub fn id_institucion(&self) -> &'a str {
        self.institucion.codigo_habilitacion.trim()
    }

    /// TODO el insumo recortado. **Éste es el borde**, y está aquí y no en
    /// `actores::ensamblar` por una razón que costó una regresión aprender.
    ///
    /// La primera versión normalizaba dentro de `actores::ensamblar`. Eso
    /// funciona para lo que ese ensamblador emite, y **no alcanza a sus
    /// llamantes**: `recortado()` devuelve una COPIA que muere ahí dentro, así
    /// que `paciente::ensamblar` y `clinico::ensamblar` seguían leyendo el
    /// `Insumo` crudo para construir las referencias. El resultado fue peor que
    /// el defecto original: `Organization.id` salía recortado y `custodian`,
    /// `author` y `attester.party` sin recortar, o sea **cuatro referencias
    /// colgantes en los cuatro RDA** donde antes había un identificador feo
    /// pero consistente. Un arreglo que rompe lo que funcionaba.
    ///
    /// Y dos campos propios del insumo —`fecha` y `modalidad`— nunca estuvieron
    /// dentro de aquel borde: `modalidad` se decidía con `.trim()` y se emitía
    /// cruda, que es literalmente el defecto que el borde vino a erradicar,
    /// escrito dos veces más.
    ///
    /// El borde va donde el dato ENTRA al módulo. Aquí.
    pub fn recortado(&self) -> Self {
        Self {
            paciente: self.paciente.recortado(),
            profesional: self.profesional.recortado(),
            institucion: self.institucion.recortado(),
            fecha: self.fecha.trim(),
            modalidad: self.modalidad.map(str::trim),
            grupo_servicios: self.grupo_servicios.map(str::trim),
            periodo: self.periodo.map(|(i, f)| (i.trim(), f.trim())),
            aval: self.aval,
            entorno: self.entorno.map(str::trim),
            causa_externa: self.causa_externa.map(str::trim),
            diagnostico_ingreso: self.diagnostico_ingreso.map(|d| d.recortado()),
            diagnostico_egreso: self.diagnostico_egreso.map(|d| d.recortado()),
            via_ingreso: self.via_ingreso.map(str::trim),
            destino_egreso: self.destino_egreso.map(str::trim),
            eps: self.eps.map(|e| e.recortado()),
            epicrisis_id: self.epicrisis_id.map(str::trim),
            servicio: self.servicio.map(|s| s.recortado()),
        }
    }
}

/// Un Bundle de tipo `document`, que es lo que la operación espera.
#[derive(Debug, Serialize)]
pub struct Bundle {
    #[serde(rename = "resourceType")]
    pub resource_type: &'static str,
    #[serde(rename = "type")]
    pub tipo: &'static str,
    /// `es-CO`, como el ejemplo oficial. Faltaba, y no lo veía ninguna prueba.
    pub language: &'static str,
    pub entry: Vec<Entrada>,
}

/// El idioma del documento, literal del ejemplo oficial.
pub const IDIOMA: &str = "es-CO";

#[derive(Debug, Serialize)]
pub struct Entrada {
    pub resource: serde_json::Value,
}

/// Ensambla el RDA de paciente, o devuelve TODAS las faltas juntas.
///
/// No emite nada a medias: o el documento está completo, o se sabe exactamente
/// qué le falta. Es la regla del recolector de RIPS aplicada aquí, y por el
/// mismo motivo — un RDA con un hueco relleno lo acepta el servidor.
pub fn ensamblar(i: &Insumo<'_>) -> Result<Bundle, Faltas> {
    // EL BORDE. Aquí entra el dato al módulo, así que aquí se normaliza —y
    // no dentro de `actores::ensamblar`, que solo normalizaba su copia y
    // dejaba las referencias de este archivo leyendo el crudo (ver
    // `Insumo::recortado`).
    let i = &i.recortado();
    let mut f = Faltas::default();
    f.exigir(Some(i.fecha), "Composition", "date",
             "sin fecha con huso, el momento de la atención se guarda mal en el servidor");
    // Los actores traen sus propias exigencias, y se acumulan con la de aquí:
    // quien corrige necesita la lista COMPLETA de una vez.
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

    let c = i.aval;
    let secciones = vec![
        super::seccion("Historial de diagnósticos de problemas de salud", SEC_DIAGNOSTICOS, c, &mut f),
        super::seccion("Historial de alergias, intolerancias y reacciones adversas", SEC_ALERGIAS, c, &mut f),
        super::seccion("Historial de medicamentos", SEC_MEDICAMENTOS, c, &mut f),
        super::seccion("Historial de antecedentes familiares", SEC_FAMILIARES, c, &mut f),
    ];
    // LA PUERTA CLÍNICA: si alguna sección quedó vacía sin confirmación, no se
    // emite un RDA que afirme «Nil Known» por defecto — se devuelven las faltas.
    if f.hay() {
        return Err(f);
    }

    let mut composicion = serde_json::json!({
        "resourceType": "Composition",
        "meta": Meta::perfil("CompositionPatientStatementRDA"),
        "status": "final",
        "type": Concepto::loinc(TIPO_DOCUMENTO.0, TIPO_DOCUMENTO.1),
        "subject": Ref::a(&paciente_id),
        "date": i.fecha,
        "author": [Ref::a(&profesional_id)],
        "title": TITULO,
        // «N» = normal. El RDA lleva datos de categoría especial (Ley 1581), y
        // la confidencialidad la fija el perfil, no cada prestador.
        "confidentiality": "N",
        "attester": [{ "mode": "legal", "party": Ref::a(i.id_institucion()) }],
        "custodian": Ref::a(i.id_institucion()),
        "section": secciones,
    });

    // EL `event`, ENTERO O NADA. El perfil lo declara `min=1 max=1` con DOS
    // códigos y un `period` con `start` y `end`; emitirlo a medias es peor que no
    // emitirlo, porque un documento con un `event` incompleto se acepta y se
    // guarda mal. Hasta el 2026-08-16 salía con UN código y sin periodo.
    //
    // Los `display` se DERIVAN del CodeSystem vendorizado: un código que el
    // catálogo no tenga no se emite con `display` vacío — se queda fuera y la
    // falta lo dice.
    if let Some(ev) = evento(i) {
        composicion["event"] = serde_json::json!([ev]);
    }

    // La `Composition` va PRIMERA —es lo que hace que un Bundle sea un
    // documento— y detrás los actores, en el orden del ejemplo oficial.
    let mut entry = vec![Entrada { resource: composicion }];
    entry.extend(actores.into_iter().map(|resource| Entrada { resource }));

    Ok(Bundle { resource_type: "Bundle", tipo: "document", language: IDIOMA, entry })
}

/// El `Composition.event`, o `None` si le falta cualquiera de sus cuatro piezas.
///
/// Cuatro y no una: modalidad, grupo de servicios, inicio y fin. El perfil las
/// exige todas, así que «tengo la modalidad» no alcanza para emitir el evento —
/// y emitirlo cojo es lo que el servidor guarda mal sin protestar.
pub(super) fn evento(i: &Insumo<'_>) -> Option<serde_json::Value> {
    use super::terminologia::{display, SISTEMA_GRUPO_SERVICIOS, SISTEMA_MODALIDAD};

    let codigo = |sistema: &str, valor: Option<&str>| -> Option<serde_json::Value> {
        let c = valor.map(str::trim).filter(|s| !s.is_empty())?;
        // Sin `display` en el catálogo no se emite: el código sería válido y la
        // etiqueta, inventada.
        let d = display(sistema, c)?;
        Some(serde_json::json!({ "coding": [{ "system": sistema, "code": c, "display": d }] }))
    };

    let modalidad = codigo(SISTEMA_MODALIDAD, i.modalidad)?;
    let grupo = codigo(SISTEMA_GRUPO_SERVICIOS, i.grupo_servicios)?;
    let (inicio, fin) = i.periodo?;
    let (inicio, fin) = (inicio.trim(), fin.trim());
    if inicio.is_empty() || fin.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "code": [modalidad, grupo],
        "period": { "start": inicio, "end": fin },
    }))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El ejemplo OFICIAL, empotrado en el binario. Es el oráculo de todas las
    /// pruebas de forma de abajo: no se comparan campos contra lo que yo creo
    /// recordar, se comparan contra lo que MinSalud publica.
    const EJEMPLO: &str = include_str!("../../datos/ihce/ejemplos/enviar-rda-paciente.json");

    /// El ejemplo oficial, por el arnés COMÚN.
    ///
    /// Tenía copia propia del limpiador —y encima una que no quitaba los
    /// comentarios `//`—, o sea que la consolidación de `arnes` había dejado
    /// tres. Lo señaló la revisión de seguridad del 2026-08-15.
    fn ejemplo() -> serde_json::Value {
        crate::ihce::arnes::oficial(EJEMPLO)
    }

    fn insumo() -> Insumo<'static> {
        use super::super::actores::{Codigo, Institucion, Nombre, Paciente, Profesional};
        let nombre = Nombre {
            nombre1: "Primernombre",
            nombre2: Some("Segundonombre"),
            apellido1: "Primerapellido",
            apellido2: Some("Segundoapellido"),
        };
        Insumo {
            paciente: Paciente {
                tipo_doc: "CC",
                numero_doc: "000000000",
                tipo_doc_display: "Cédula ciudadanía",
                nombre,
                genero_fhir: "male",
                sexo_colombiano: Some(Codigo::nuevo("01", "Hombre")),
                nacimiento: "1999-01-01",
                hora_nacimiento: None,
                fallecido: None,
                municipio: Some(Codigo::nuevo("11001", "Bogotá D.C.")),
                pais: Some(Codigo::nuevo("170", "Colombia")),
                zona: Some(Codigo::nuevo("01", "Urbana")),
                nacionalidad: None,
                etnia: None,
                discapacidad: None,
                identidad_genero: None,
            },
            profesional: Profesional {
                tipo_doc: "CC",
                numero_doc: "111111111",
                tipo_doc_display: "Cédula ciudadanía",
                nombre,
            },
            institucion: Institucion { codigo_habilitacion: "0500100001", nit: "900123456" },
            fecha: "2026-08-14T18:42:00-05:00",
            modalidad: Some("01"),
            grupo_servicios: Some("01"),
            periodo: Some(("2026-08-14T18:00:00-05:00", "2026-08-14T18:42:00-05:00")),
            // Modela un clínico que revisó las cuatro secciones y confirma «sin
            // nada conocido» — el único camino que emite `nilknown` (puerta #603).
            aval: crate::ihce::Aval::nuevo(&CONFIRMACIONES),
            // El RDA de paciente NO usa los campos del encuentro clínico.
            entorno: None,
            causa_externa: None,
            diagnostico_ingreso: None,
            diagnostico_egreso: None,
            via_ingreso: None,
            destino_egreso: None,
            eps: None,
            epicrisis_id: None,
            servicio: None,
        }
    }

    fn composicion_generada() -> serde_json::Value {
        let b = ensamblar(&insumo()).expect("el insumo completo tiene que ensamblar");
        b.entry.into_iter().next().unwrap().resource
    }

    /// **LA PUERTA DEL `nilknown`** (#603, decidida por Juan el 2026-08-23): una
    /// sección vacía SIN confirmación humana hace FALLAR el ensamblado, en vez de
    /// afirmar «Nil Known» por defecto de máquina. Es la pareja del camino verde
    /// —el `insumo()` de estas pruebas confirma las cuatro—: sin este caso rojo, el
    /// gate podría no engancharse y el RDA volvería a inventar «sin hallazgos».
    #[test]
    fn una_seccion_vacia_sin_confirmar_no_afirma_nilknown() {
        // Verde: con las cuatro confirmadas, ensambla.
        assert!(
            ensamblar(&insumo()).is_ok(),
            "con las secciones confirmadas el RDA de paciente debe ensamblar"
        );
        // Rojo, por la vía REAL: sin confirmar ninguna, se niega —una falta de
        // `emptyReason` por cada sección vacía— y NO emite el documento.
        let mut i = insumo();
        i.aval = crate::ihce::Aval::ninguno();
        let err = ensamblar(&i)
            .expect_err("sin confirmación NO se emite un RDA que afirme «Nil Known»");
        assert_eq!(err.0.len(), 4, "una falta por cada sección vacía sin confirmar");
        assert!(
            err.0.iter().all(|x| x.campo == "emptyReason"),
            "cada falta es la del gate del emptyReason"
        );
        // Y el control fino: confirmar SOLO tres (todas menos familiares) deja
        // exactamente una falta.
        i.aval = crate::ihce::Aval::nuevo(&CONFIRMACIONES[..3]);
        let err = ensamblar(&i).expect_err("falta una sección por confirmar");
        assert_eq!(err.0.len(), 1, "solo la sección no confirmada (familiares) falta");
    }

    fn composicion_oficial() -> serde_json::Value {
        ejemplo()["entry"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["resource"].clone())
            .find(|r| r["resourceType"] == "Composition")
            .expect("el ejemplo tiene Composition")
    }

    /// El control positivo de todo lo demás: que el oráculo se pueda leer y sea
    /// lo que dice ser. Sin esto, un ejemplo corrupto haría pasar las
    /// comparaciones de abajo por vacuidad.
    #[test]
    fn el_ejemplo_oficial_es_un_bundle_document_de_ocho_recursos() {
        let e = ejemplo();
        assert_eq!(e["resourceType"], "Bundle");
        assert_eq!(e["type"], "document");
        assert_eq!(e["entry"].as_array().unwrap().len(), 8);
    }

    #[test]
    fn el_perfil_de_la_composition_es_el_del_ejemplo() {
        assert_eq!(
            composicion_generada()["meta"]["profile"][0],
            composicion_oficial()["meta"]["profile"][0]
        );
    }

    #[test]
    fn el_tipo_de_documento_es_el_loinc_del_ejemplo() {
        let g = &composicion_generada()["type"]["coding"][0];
        let o = &composicion_oficial()["type"]["coding"][0];
        assert_eq!(g["system"], o["system"]);
        assert_eq!(g["code"], o["code"], "el LOINC del tipo de documento no coincide");
        // EL `display` TAMBIÉN, y faltaba: llevaba un sufijo «Document» que le
        // había puesto yo y que el ejemplo no trae. Comparar solo `system` y
        // `code` lo hacía invisible — la revisión del 2026-08-15 lo cazó
        // leyendo el artefacto, no la prueba.
        assert_eq!(g["display"], o["display"], "el display del LOINC no es el del ejemplo");
    }

    /// **La prueba que más vale**: los cuatro códigos de sección, en orden.
    /// Una sección con el código equivocado NO da error — el servidor la acepta
    /// y guarda las alergias donde van los diagnósticos.
    #[test]
    fn las_cuatro_secciones_llevan_el_codigo_loinc_del_ejemplo() {
        let g = composicion_generada();
        let o = composicion_oficial();
        let gs = g["section"].as_array().unwrap();
        let os = o["section"].as_array().unwrap();
        assert_eq!(gs.len(), os.len(), "el ejemplo tiene {} secciones", os.len());
        for (n, (a, b)) in gs.iter().zip(os.iter()).enumerate() {
            assert_eq!(a["code"]["coding"][0]["code"], b["code"]["coding"][0]["code"],
                       "sección {n}: el código LOINC no es el del ejemplo");
            assert_eq!(a["title"], b["title"], "sección {n}: el título no es el del ejemplo");
        }
    }

    #[test]
    fn el_titulo_es_literal_el_del_ejemplo() {
        assert_eq!(composicion_generada()["title"], composicion_oficial()["title"]);
    }

    #[test]
    fn las_referencias_llevan_almohadilla_y_apuntan_a_quien_deben() {
        let c = composicion_generada();
        assert_eq!(c["subject"]["reference"], "#CC-000000000");
        assert_eq!(c["author"][0]["reference"], "#CC-111111111");
        assert_eq!(c["custodian"]["reference"], "#0500100001");
        assert_eq!(c["attester"][0]["party"]["reference"], "#0500100001");
        assert_eq!(c["attester"][0]["mode"], "legal");
    }

    /// La pareja de todo lo de arriba, y la razón de ser del módulo: **falta un
    /// dato y NO sale documento**. Sin esta, todas las anteriores pasarían igual
    /// con un ensamblador que rellena huecos con cadenas vacías.
    #[test]
    fn sin_codigo_de_habilitacion_no_hay_documento() {
        let mut i = insumo();
        i.institucion.codigo_habilitacion = "";
        let e = ensamblar(&i).expect_err("tiene que negarse");
        assert!(e.to_string().contains("Organization.identifier"), "{e}");
    }

    /// Y falla ACUMULANDO, no en el primer hueco: quien corrige necesita la
    /// lista completa.
    #[test]
    fn las_faltas_salen_todas_juntas() {
        let mut i = insumo();
        i.paciente.numero_doc = "";
        i.institucion.codigo_habilitacion = "";
        i.fecha = "   ";
        let e = ensamblar(&i).expect_err("tiene que negarse");
        assert_eq!(e.0.len(), 3, "salieron {} faltas y esperaba 3: {e}", e.0.len());
    }

    /// **Los tres actores viajan TAMBIÉN en el documento de paciente.**
    ///
    /// La escribió `sin-caso-rojo` el 2026-08-15, no yo: midió que el hunk que
    /// mete los actores en este Bundle se podía REVERTIR sin que el banco se
    /// enterara. Y tenía razón — la prueba que lo comprobaba
    /// (`los_actores_estan_dentro_y_nada_cuelga`, en `mod`) llama a
    /// `clinico::ensamblar`, o sea que medía el otro ensamblador. Los dos
    /// caminos hacen lo mismo y solo uno estaba vigilado; el de aquí habría
    /// podido volver a emitir un documento con tres referencias colgantes y
    /// las 44 pruebas del módulo habrían seguido verdes.
    ///
    /// Es la directiva 33 con instrumento: la sonda existía y no discriminaba
    /// en este camino.
    #[test]
    fn los_actores_viajan_en_el_documento_de_paciente() {
        let b = ensamblar(&insumo()).unwrap();
        let v = serde_json::to_value(&b).unwrap();
        let tipos: Vec<&str> = v["entry"].as_array().unwrap().iter()
            .map(|e| e["resource"]["resourceType"].as_str().unwrap())
            .collect();
        assert_eq!(tipos, ["Composition", "Patient", "Organization", "Practitioner"]);
        let colgantes = crate::ihce::referencias_colgantes(&v);
        assert!(colgantes.is_empty(), "no debería colgar ninguna: {colgantes:?}");
    }

    /// La modalidad es lo ÚNICO opcional, y se comprueba por los dos lados.
    #[test]
    fn sin_modalidad_no_se_emite_el_event_vacio() {
        let mut i = insumo();
        i.modalidad = None;
        let b = ensamblar(&i).unwrap();
        let c = &b.entry[0].resource;
        assert!(c.get("event").is_none(), "un `event` vacío es peor que ninguno");
    }

    #[test]
    fn con_modalidad_el_event_lleva_el_code_system_del_ejemplo() {
        let c = composicion_generada();
        assert_eq!(
            c["event"][0]["code"][0]["coding"][0]["system"],
            "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianTechModality"
        );
    }
}
