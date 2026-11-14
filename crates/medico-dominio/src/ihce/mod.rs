// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza
//! Ensamblado de los **Resumen Digital de Atención (RDA)** en HL7 FHIR R4, para
//! la Interoperabilidad de la Historia Clínica Electrónica (IHCE).
//!
//! Obligatorio para todo prestador con REPS activo desde el **2026-04-15**
//! (Resolución 1888 de 2025, que adopta el RDA bajo la Ley 2015 de 2020).
//!
//! # Qué desbloqueó esto, y cuándo
//!
//! Hasta el 2026-08-14 este módulo no existía y no podía existir: el paquete de
//! perfiles `minsalud.fhir.co.rda` no era traíble —vulcano geobloqueaba— y
//! ensamblar un Bundle sin él habría sido **inventarlo**, que es exactamente lo
//! que la casa no hace (fallar ruidosamente antes que rellenar).
//!
//! Ese día apareció la vía: la **colección Postman del sandbox de prestadores**
//! que MinSalud publica sin registro, con los CUATRO RDA en ejemplos completos.
//! Están guardados en `datos/ihce/ejemplos/` y son el ORÁCULO de este módulo:
//! no se copia su contenido, se comprueba contra su forma.
//!
//! # Lo que un ejemplo NO es
//!
//! Un ejemplo no es una `StructureDefinition`. Sirve para construir y para
//! probar; **no valida formalmente**. Para eso sigue haciendo falta el paquete,
//! y hasta que llegue este módulo puede producir un Bundle bien formado que el
//! servidor rechace por una regla que el ejemplo no ejercita. Se dice aquí para
//! que nadie lea el verde de sus pruebas como conformidad.
//!
//! # Referencias internas: `#`, no URN
//!
//! El ejemplo oficial referencia con `#<id>` —`#CC-000000000`, `#Condition-0`,
//! `#Códigohabilitaciónprestador`— y no con `urn:uuid:`, que es lo que uno
//! esperaría de un Bundle `document`. Se sigue el ejemplo, porque el servidor es
//! el que manda y es lo único que tenemos medido.
//!
//! # LO QUE ESTE MÓDULO PRODUCE HOY, y lo que TODAVÍA NO
//!
//! **Los tres actores están dentro** desde el 2026-08-15: `Patient`,
//! `Organization` y `Practitioner`, en `actores`. Con ellos, `subject`,
//! `author`, `attester.party` y `custodian` —de los CUATRO documentos— apuntan
//! a recursos que viajan en el Bundle.
//!
//! **Los RECURSOS CLÍNICOS del ENCUENTRO están dentro desde FASE 4 (#603,
//! 2026-08-29)**: `Encounter` (`Composition.encounter`), `Condition` (diagnósticos
//! de ingreso/egreso), el pagador `Organization` EAPB (`sectionPayers`) y el
//! `DocumentReference` de la epicrisis (`sectionAddendumDocuments`), en `recursos`
//! y `clinico`. Con ellos los TRES RDA clínicos son **estructuralmente conformes a
//! nivel de la `Composition` (0/0/0 incumplidas)**, y cada recurso se contrasta
//! contra SU perfil, no solo la referencia (`los_recursos_clinicos_contra_su_perfil`
//! en `tests/conformidad_rda.rs`): emitir la referencia a un `Encounter` no conforme
//! es un verde falso que MinSalud rechaza (D28/D33). Ninguna referencia cuelga ni se
//! duplica (`los_actores_y_recursos_clinicos_estan_dentro_y_nada_cuelga`).
//!
//! **Lo que fase 4 NO cierra, y está MEDIDO** (D20): en PRODUCCIÓN medico aún no
//! emite estos RDA clínicos. El ENTORNO de atención YA está cableado (config del
//! prestador `MEDICO_ENTORNO_ATENCION`, default `05` Institucional). La MODALIDAD de
//! urgencias/hospitalización ya NO bloquea: se DERIVA (`Encuentro::modalidad_derivada`
//! → intramural `01`) SOLO cuando ese entorno es `05` Institucional —ahí la cama/la
//! urgencia presencial la fijan—; con un entorno extramural NO se deriva (sería
//! afirmar «Hogar + Intramural», dato falso), vuelve a Falta. Consulta la captura
//! porque varía. El TIPO de
//! diagnóstico de ingreso/egreso —que el perfil RDA exige aunque la Resolución 948 no
//! lo pida para AU/AH— YA se captura desde la migración 0075 (columnas
//! `tipo_diagnostico_ingreso`/`_egreso` de `hospitalizacion` y `atencion_urgencias`).
//! Lo que sigue frenando la emisión son los avales del `nilknown` (y en urgencias el
//! diagnóstico de egreso, write-only). El ensamblador exige los que falten (`Faltas`);
//! las fixtures los traen del ejemplo, así que la conformidad medida es ESTRUCTURAL. Y siguen fuera los recursos
//! clínicos OPCIONALES del ejemplo (`AllergyIntolerance`, `MedicationStatement`,
//! `Procedure`, `Observation`…), cuyas secciones van con `emptyReason=nilknown`.
//!
//! El RDA de PACIENTE sigue con sus 4 secciones vacías confirmadas (aún sin los
//! recursos `Condition`/`AllergyIntolerance`/… manifestados).
//!
//! Esto se escribe aquí porque la revisión de seguridad del 2026-08-14 encontró que
//! el módulo **aparentaba lo contrario**: sus pruebas fijaban 8/16/24/23 recursos…
//! **del EJEMPLO**, no de lo generado. El banco de conformidad y los tests por
//! recurso miden ahora el PRODUCTO.
//!
//! **`ensamblar` sigue SIN cablearse** a `medico_ihce::correr_cola`: encolar de
//! verdad manda datos clínicos a la plataforma nacional, y eso pasa por la
//! revisión de seguridad y por que exista dónde enviarlos (faltan las
//! credenciales, que se emiten al PRESTADOR y no al proveedor de software).

pub mod actores;
pub mod terminologia;
pub mod clinico;
pub mod paciente;
mod recursos;

/// El sexo del paciente, traducido del `M`/`F` que guarda medico (el `cod_sexo`
/// del RIPS, ver `rips::documentos`) a los DOS vocabularios que el `Patient` del
/// RDA necesita: el `gender` de FHIR (`male`/`female`) y el código
/// `ColombianGenderGroup` de `_gender` (`01`/`02`).
///
/// Devuelve `(genero_fhir, código de ColombianGenderGroup)`; el `display` del
/// código lo deriva luego [`terminologia::display`]. Son dos vocabularios para el
/// mismo hecho y por eso van los dos (directiva 32).
///
/// **Un valor que no sea `M` ni `F` da `None`**, y el productor lo anota como
/// falta: medico solo captura esos dos, y traducir un dato roto a «intersexual»
/// (`03`) sería afirmar un dato clínico que nadie registró (directiva 20). El día
/// que medico capture el intersexual, se añade aquí con su código `03`.
pub fn sexo_rda(sexo: &str) -> Option<(&'static str, &'static str)> {
    match sexo.trim() {
        "M" => Some(("male", "01")),
        "F" => Some(("female", "02")),
        _ => None,
    }
}

/// El arnés que leen las pruebas de los tres submódulos: convierte el ejemplo
/// oficial —que es una petición de Postman, no JSON estricto— en un valor
/// comparable.
///
/// Vive AQUÍ y no en cada submódulo porque ya estaba escrito dos veces con dos
/// grados de cuidado distintos, y la tercera copia habría sido la que se
/// quedara atrás (directiva 9). Su propio positivo de control es
/// `el_limpiador_no_destroza_las_url`, en `clinico`.
#[cfg(test)]
pub(crate) mod arnes {
    /// Quita las variables `{{…}}` de Postman y los comentarios `//`.
    ///
    /// **La trampa está en los comentarios**: quitarlos a ciegas destroza cada
    /// `"https://fhir.minsalud.gov.co/…"`, que es justo lo que se compara. Se
    /// respetan las cadenas.
    ///
    /// SE RECORRE POR CARACTERES, NO POR BYTES, y costó una prueba roja: hacer
    /// `b[i] as char` sobre `as_bytes()` parte cada carácter UTF-8 en dos y
    /// convierte «demográficos» en «demogrÃ¡ficos». El oráculo salía con la
    /// mojibake dentro, así que la comparación medía basura.
    pub fn limpiar(t: &str) -> String {
        let sin_var: String = t
            .split("{{")
            .map(|p| p.split_once("}}").map(|(_, r)| r).unwrap_or(p))
            .collect();
        let mut out = String::new();
        let (mut en_cadena, mut escape, mut saltando) = (false, false, false);
        let mut it = sin_var.chars().peekable();
        while let Some(c) = it.next() {
            if saltando {
                if c == '\n' { saltando = false; out.push(c) }
                continue;
            }
            if en_cadena {
                out.push(c);
                if escape { escape = false }
                else if c == '\\' { escape = true }
                else if c == '"' { en_cadena = false }
            } else if c == '"' {
                en_cadena = true; out.push(c);
            } else if c == '/' && it.peek() == Some(&'/') {
                saltando = true;
            } else {
                out.push(c);
            }
        }
        out
    }

    /// El ejemplo oficial entero, ya parseado.
    pub fn oficial(bruto: &str) -> serde_json::Value {
        serde_json::from_str(&limpiar(bruto)).expect("el ejemplo oficial tiene que parsear")
    }

    /// **EL POSITIVO DE CONTROL DEL PROPIO ARNÉS.**
    ///
    /// Si `limpiar` se comiera los `//` de dentro de las cadenas, cada
    /// `"https://fhir.minsalud.gov.co/…"` quedaría destrozado — y eso es
    /// justo lo que comparan las pruebas de los tres submódulos. Sin esta, el
    /// arnés podría fabricar un oráculo de basura.
    #[test]
    fn el_limpiador_de_arnes_no_destroza_las_url() {
        let t = r#"{ // comentario
            "a": "https://fhir.minsalud.gov.co/rda/StructureDefinition/PatientRDA",
            "b": "{{var}}texto" }"#;
        let v: serde_json::Value = serde_json::from_str(&limpiar(t)).unwrap();
        assert_eq!(v["a"], "https://fhir.minsalud.gov.co/rda/StructureDefinition/PatientRDA");
        assert_eq!(v["b"], "texto");
    }

    /// UN recurso del ejemplo, por su `resourceType`.
    pub fn recurso(bruto: &str, tipo: &str) -> serde_json::Value {
        oficial(bruto)["entry"].as_array().unwrap().iter()
            .map(|e| e["resource"].clone())
            .find(|r| r["resourceType"] == tipo)
            .unwrap_or_else(|| panic!("el ejemplo no trae ningún {tipo}"))
    }
}

use serde::Serialize;

/// El sistema de codificación LOINC, que nombra el tipo de documento y cada
/// sección. Va como constante porque aparece en todas partes y una errata aquí
/// se propaga a los cuatro RDA.
pub const LOINC: &str = "http://loinc.org";

/// La base canónica de los perfiles del RDA. Sale de los ejemplos oficiales.
pub const PERFIL: &str = "https://fhir.minsalud.gov.co/rda/StructureDefinition";

/// Falta un dato que el RDA exige, y **no se sustituye por nada**.
///
/// Es la misma regla del recolector de RIPS y por el mismo motivo: un Resumen
/// que sale con un dato inventado no se nota nunca —lo acepta el servidor, y el
/// error aparece cuando alguien lea esa historia clínica en otra institución—.
/// Un envío que no sale se arregla en una tarde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Falta {
    /// Qué recurso lo pedía: `Composition`, `Patient`, …
    pub recurso: &'static str,
    /// El campo, con el nombre que usa el perfil.
    pub campo: &'static str,
    /// Por qué no se puede seguir sin él, en una frase que sirva a quien lo lea
    /// en un registro a las once de la noche.
    pub porque: &'static str,
}

impl std::fmt::Display for Falta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}: {}", self.recurso, self.campo, self.porque)
    }
}

/// El ensamblado **acumula** todas las faltas y las devuelve juntas.
///
/// A propósito, y es la misma decisión que el recolector de RIPS: quien tiene
/// que corregir necesita la lista COMPLETA para arreglarla de una vez. Reventar
/// en el primer hueco convierte un cierre en veinte intentos.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Faltas(pub Vec<Falta>);

impl Faltas {
    pub fn exigir(&mut self, valor: Option<&str>, recurso: &'static str,
                  campo: &'static str, porque: &'static str) {
        // Vacío es AUSENTE. Una cadena en blanco pasando por «presente» es la
        // forma más barata de colar un dato inventado.
        if valor.map(str::trim).unwrap_or("").is_empty() {
            self.0.push(Falta { recurso, campo, porque });
        }
    }

    pub fn hay(&self) -> bool {
        !self.0.is_empty()
    }
}

impl std::fmt::Display for Faltas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let l: Vec<String> = self.0.iter().map(ToString::to_string).collect();
        write!(f, "{}", l.join("; "))
    }
}

/// Un `Coding` de FHIR.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Coding {
    pub system: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

impl Coding {
    pub fn loinc(code: &str, display: &str) -> Self {
        Self { system: LOINC.into(), code: code.into(), display: Some(display.into()) }
    }
}

/// Un `CodeableConcept` de FHIR.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Concepto {
    pub coding: Vec<Coding>,
}

impl Concepto {
    pub fn loinc(code: &str, display: &str) -> Self {
        Self { coding: vec![Coding::loinc(code, display)] }
    }

    /// El `emptyReason` que los perfiles del RDA fijan para una sección vacía:
    /// `list-empty-reason` / `nilknown` / «Nil Known». Los tres valores salen de
    /// los `StructureDefinition` (fixedUri/fixedCode/fixedString), verificados
    /// idénticos en los CUATRO perfiles (paciente + los tres clínicos), no de la
    /// memoria. Compartido para no tener cuatro copias que diverjan (directiva 9).
    ///
    /// **OJO CLÍNICO (#603):** `nilknown` es, por definición FHIR, una afirmación
    /// POSITIVA de un clínico —«no hay nada conocido»—, NO el defecto de una
    /// máquina ante la falta de información. Hoy es correcto SOLO porque las
    /// secciones nacen vacías y NADA se transmite (`ensamblar` no está cableado al
    /// envío). Antes de cablear el transporte hay que garantizar que no se afirme
    /// «Nil Known» sin aval humano (puerta registrada en PENDIENTES).
    pub fn sin_contenido_conocido() -> Self {
        Self {
            coding: vec![Coding {
                system: "http://terminology.hl7.org/CodeSystem/list-empty-reason".into(),
                code: "nilknown".into(),
                display: Some("Nil Known".into()),
            }],
        }
    }
}

/// Una `Composition.section` serializada, con la puerta clínica del `nilknown`.
/// Compartida por los CUATRO documentos (paciente + los tres clínicos): una sola
/// definición, no una copia por archivo (directiva 9). Antes vivía en
/// `paciente.rs` y `clinico.rs` tenía la suya SIN `emptyReason` ni puerta, que era
/// el déficit de #603 en los RDA clínicos.
#[derive(Debug, Serialize)]
pub(super) struct SeccionRda {
    pub title: String,
    pub code: Concepto,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entry: Vec<Ref>,
    /// FHIR (invariante cmp-2): una sección SIN `entry` lleva `emptyReason`. El
    /// perfil del RDA fija su `coding` (system/code/display, `min=1` cada uno), así
    /// que una sección vacía sin él no valida. Va `Option` porque el día que la
    /// sección tenga entradas clínicas NO puede llevarlo.
    #[serde(rename = "emptyReason", skip_serializing_if = "Option::is_none")]
    pub empty_reason: Option<Concepto>,
}

/// Una confirmación clínica de que UNA sección vacía del RDA fue revisada y no
/// tiene nada conocido que reportar (#603 F4.5, requisito #1).
///
/// Es la pieza que vuelve infalsificable el aval del `nilknown`: no basta el
/// código de la sección, hay que nombrar QUIÉN la confirmó y CUÁNDO.
#[derive(Debug, Clone, Copy)]
pub struct Confirmacion<'a> {
    /// El código LOINC de la sección avalada (p. ej. `48765-2`, alergias).
    pub seccion: &'a str,
    /// El usuario que la confirmó. La mitad que no se puede omitir: no hay aval
    /// sin un usuario que lo firme.
    pub usuario_id: i64,
    /// El instante de la confirmación, ISO-8601 con huso (zona del prestador),
    /// tal como lo sella la captura.
    pub sello: &'a str,
}

/// El aval humano de las secciones vacías de un RDA (#603 F4.5, requisito #1).
///
/// Reemplaza el `&[&str]` de códigos que el contrato aceptaba hasta ahora —que
/// cualquier llamante podía fabricar—. La puerta del `nilknown` ([`seccion`]) ya
/// no consulta una lista de cadenas: consulta un `Aval`, y **un `Aval` con
/// contenido solo se construye nombrando, por cada sección, un usuario y un
/// instante** ([`Confirmacion`]). Pasar códigos «sueltos» al ensamblador deja de
/// compilar, que es justo lo que pedía el requisito #1 de #603.
///
/// **Qué garantiza y qué NO** (directiva 20, sin sobrevender): esto NO es una
/// firma criptográfica; un llamante decidido todavía podría escribir un
/// `usuario_id` inventado. Lo que impide es la emisión por COMODIDAD —el defecto
/// de máquina que la puerta del #603 existe para cerrar— y hace el aval
/// ATRIBUIBLE: el único camino de producción que lo construye es la captura del
/// clínico, que escribe la bitácora en el mismo acto (`medico_datos`, requisito
/// #2). La infalsificabilidad es de tipos + un solo sitio de construcción, como
/// `procedencia.rs`, no de criptografía.
#[derive(Debug, Clone, Copy)]
pub struct Aval<'a> {
    confirmaciones: &'a [Confirmacion<'a>],
}

impl<'a> Aval<'a> {
    /// El ÚNICO constructor con contenido: exige las `Confirmacion`es del acto,
    /// cada una con su usuario y su sello.
    ///
    /// **AVISO para el día que nazca un SEGUNDO productor de producción** (p. ej.
    /// al cablear los RDA clínicos de urgencias/hospitalización, fase C): la
    /// infalsificabilidad de este tipo es de DISCIPLINA —hoy el único llamante de
    /// producción, `medico_datos::rda_encuentro::preparar_consulta`, arma las
    /// `Confirmacion` LEYÉNDOLAS de la bitácora `aval_seccion_rda`, no fabricándolas—.
    /// Un productor nuevo que pase aquí una lista fabricada en vez de leer la
    /// bitácora reintroduciría el defecto de #603 (afirmar `nilknown` sin acto
    /// humano) **sin que nada compile en rojo**. Todo productor de RDA lee el aval
    /// de la bitácora; nunca lo inventa.
    pub fn nuevo(confirmaciones: &'a [Confirmacion<'a>]) -> Self {
        Self { confirmaciones }
    }

    /// El aval VACÍO: nadie confirmó nada. Explícito —no un `&[]` anónimo— para
    /// el llamante que aún no tiene captura: con él, toda sección vacía es una
    /// FALTA y el RDA no se emite, que es la puerta del #603 obrando.
    pub const fn ninguno() -> Self {
        Self { confirmaciones: &[] }
    }

    /// ¿Un clínico confirmó vacía esta sección?
    pub fn confirma(&self, seccion: &str) -> bool {
        self.confirmaciones.iter().any(|c| c.seccion == seccion)
    }

    /// Las confirmaciones que porta, para quien deba registrarlas o auditarlas.
    pub fn confirmaciones(&self) -> &[Confirmacion<'a>] {
        self.confirmaciones
    }

    /// ¿El aval está vacío (nadie confirmó nada)?
    pub fn vacio(&self) -> bool {
        self.confirmaciones.is_empty()
    }
}

/// Arma una sección aplicando LA PUERTA CLÍNICA del `nilknown` (#603, decidida por
/// Juan el 2026-08-23). Una sección vacía necesita `emptyReason`, y el perfil de
/// MinSalud lo fija a `nilknown` = «revisado, no hay nada conocido» —una AFIRMACIÓN
/// de un clínico, no el defecto de una máquina—. Por eso:
///   · si un humano confirmó esta sección vacía (está en el `aval`)
///     → se emite `emptyReason=nilknown`;
///   · si NADIE la confirmó → **es una FALTA**: el ensamblado se niega, en vez de
///     afirmar «Nil Known» por defecto (directiva 20).
///
/// El día que la sección traiga entradas clínicas, `entry` deja de estar vacía y
/// ni el `emptyReason` ni la puerta aplican (se emite el contenido).
///
/// Es la MISMA función para los cuatro documentos: si mañana cambia la regla del
/// `nilknown`, cambia en un solo sitio.
pub(super) fn seccion(
    titulo: &str,
    codigo: (&str, &str),
    aval: Aval,
    f: &mut Faltas,
) -> SeccionRda {
    let entry: Vec<Ref> = vec![];
    let confirmada = aval.confirma(codigo.0);
    if entry.is_empty() && !confirmada {
        f.0.push(Falta {
            recurso: "Composition.section",
            campo: "emptyReason",
            porque: "sección vacía sin confirmación de revisión: el perfil solo admite \
                     «Nil Known», que es una afirmación de un clínico, y emitirla por \
                     defecto presentaría como «sin hallazgos» algo que nadie revisó",
        });
    }
    let empty_reason =
        (entry.is_empty() && confirmada).then(Concepto::sin_contenido_conocido);
    SeccionRda {
        title: titulo.to_string(),
        code: Concepto::loinc(codigo.0, codigo.1),
        entry,
        empty_reason,
    }
}

/// Una sección POBLADA: lleva `entry` y NO `emptyReason` (#603 fase 4). Es la otra
/// mitad de [`seccion`]: cuando el encuentro construye los recursos clínicos de una
/// sección (`sectionProblems`=Condition, `sectionPayers`=EAPB,
/// `sectionAddendumDocuments`=DocumentReference), la sección deja de estar vacía y
/// la puerta del `nilknown` no aplica —una sección con contenido no afirma «Nil
/// Known»—. El invariante FHIR cmp-2 (`entry` XOR `emptyReason`) lo respeta el
/// llamante: solo se llama con `entradas` no vacías.
pub(super) fn seccion_poblada(titulo: &str, codigo: (&str, &str), entradas: Vec<Ref>) -> SeccionRda {
    debug_assert!(!entradas.is_empty(), "una sección poblada sin entradas viola cmp-2");
    SeccionRda {
        title: titulo.to_string(),
        code: Concepto::loinc(codigo.0, codigo.1),
        entry: entradas,
        empty_reason: None,
    }
}

/// Una referencia interna del Bundle. **Con `#`**, como el ejemplo oficial.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Ref {
    pub reference: String,
}

impl Ref {
    pub fn a(id: &str) -> Self {
        Self { reference: format!("#{id}") }
    }
}

/// `meta.profile`, que es como el servidor sabe contra qué perfil validar.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Meta {
    pub profile: Vec<String>,
}

impl Meta {
    pub fn perfil(nombre: &str) -> Self {
        Self { profile: vec![format!("{PERFIL}/{nombre}")] }
    }
}

/// Las referencias `#<id>` del Bundle que NO resuelven a un recurso de dentro.
///
/// Un Bundle `document` con referencias colgantes está incompleto: el servidor
/// no puede resolver a qué paciente, a qué profesional ni a qué institución se
/// refiere el documento. Hoy devuelve TODAS, porque solo se emite la
/// `Composition` (ver la cabecera del módulo).
///
/// Existe para que esa incompletitud sea un NÚMERO y no una frase. Mientras la
/// fase 4 no construya los recursos, `hoy_el_bundle_esta_incompleto_y_se_sabe`
/// fija cuáles faltan; el día que se construyan, esa prueba se pone roja sola y
/// se convierte en `assert!(referencias_colgantes(&b).is_empty())`.
///
/// Recorre el JSON entero, no una lista de campos conocidos: si mañana se añade
/// una referencia en un sitio nuevo, entra sola.
pub fn referencias_colgantes(bundle: &serde_json::Value) -> Vec<String> {
    fn ids(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
        if let Some(o) = v.as_object() {
            if let (Some(_), Some(id)) =
                (o.get("resourceType"), o.get("id").and_then(|x| x.as_str()))
            {
                out.insert(id.to_string());
            }
            for x in o.values() {
                ids(x, out);
            }
        } else if let Some(a) = v.as_array() {
            for x in a {
                ids(x, out);
            }
        }
    }
    fn refs(v: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(o) = v.as_object() {
            for (k, x) in o {
                if let (true, Some(id)) =
                    (k == "reference", x.as_str().and_then(|s| s.strip_prefix('#')))
                {
                    out.push(id.to_string());
                }
                refs(x, out);
            }
        } else if let Some(a) = v.as_array() {
            for x in a {
                refs(x, out);
            }
        }
    }
    let mut presentes = std::collections::BTreeSet::new();
    ids(bundle, &mut presentes);
    let mut apuntadas = vec![];
    refs(bundle, &mut apuntadas);
    let mut colgantes: Vec<String> =
        apuntadas.into_iter().filter(|r| !presentes.contains(r)).collect();
    colgantes.sort();
    colgantes.dedup();
    colgantes
}

/// Los `id` que aparecen MÁS DE UNA VEZ entre los recursos del Bundle.
///
/// Es la hermana de [`referencias_colgantes`], y existe porque aquélla **no
/// puede ver este defecto**: acumula los `id` en un conjunto, así que dos
/// recursos con el mismo `id` colapsan en uno y su filtro sale verde
/// exactamente en el caso en que las referencias están mal. Lo señalaron las
/// dos revisiones de seguridad del 2026-08-15, cada una por su lado.
///
/// Con un `id` repetido, `#CC-X` resuelve por ORDEN de aparición: la referencia
/// que atesta legalmente puede acabar apuntando al recurso equivocado sin que
/// nada dé error.
pub fn ids_duplicados(bundle: &serde_json::Value) -> Vec<String> {
    fn recoger(v: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(o) = v.as_object() {
            if let (Some(_), Some(id)) =
                (o.get("resourceType"), o.get("id").and_then(|x| x.as_str()))
            {
                out.push(id.to_string());
            }
            for x in o.values() {
                recoger(x, out);
            }
        } else if let Some(a) = v.as_array() {
            for x in a {
                recoger(x, out);
            }
        }
    }
    let mut todos = Vec::new();
    recoger(bundle, &mut todos);
    let mut repetidos: Vec<String> = todos
        .iter()
        .filter(|id| todos.iter().filter(|o| o == id).count() > 1)
        .cloned()
        .collect();
    repetidos.sort();
    repetidos.dedup();
    repetidos
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El sexo se traduce a los dos vocabularios, y lo que no es `M`/`F` NO se
    /// inventa. La pareja (el valor desconocido → `None`) es la que impide un
    /// mapeo que devuelva algo pase lo que pase.
    #[test]
    fn el_sexo_se_traduce_a_los_dos_vocabularios_y_lo_desconocido_da_none() {
        assert_eq!(sexo_rda("M"), Some(("male", "01")));
        assert_eq!(sexo_rda("F"), Some(("female", "02")));
        assert_eq!(sexo_rda(" M "), Some(("male", "01")), "se recorta");
        // Y el código que produce tiene display en el catálogo: los dos lados
        // dicen lo mismo.
        assert_eq!(terminologia::display(terminologia::SISTEMA_GENERO_GRUPO, "01"), Some("Hombre"));
        assert_eq!(terminologia::display(terminologia::SISTEMA_GENERO_GRUPO, "02"), Some("Mujer"));
        // La pareja: un dato roto o un sexo que medico no captura no se traduce.
        for malo in ["", "X", "male", "1", "I", "Intersexual"] {
            assert_eq!(sexo_rda(malo), None, "«{malo}» no se inventa");
        }
    }

    /// **LOS ACTORES Y LOS RECURSOS CLÍNICOS DENTRO: NINGUNA REFERENCIA CUELGA.**
    ///
    /// Esta prueba fue mutando con el módulo: primero fijaba la incompletitud
    /// (`hoy_el_bundle_esta_incompleto_y_se_sabe`, 1 entrada y 3 colgantes), luego
    /// los actores dentro (2026-08-15), y desde FASE 4 (#603) los recursos
    /// CLÍNICOS —`Encounter`, `Condition`, `DocumentReference`—. Cada cambio la puso
    /// roja sola, que es lo que sus comentarios mandaban. Se cuenta aquí porque una
    /// prueba que cambia de sentido sin decir por qué se lee como si siempre hubiera
    /// medido esto.
    ///
    /// La consulta externa trae hoy: `Composition` + los tres actores + el
    /// `Encounter`, su `Condition` (diagnóstico principal) y el `DocumentReference`
    /// de la epicrisis. Lo que garantiza es que **ninguna referencia cuelga** y que
    /// **ningún `id` se repite** —el Encounter referencia el Condition, la sección
    /// referencia el Encounter/Condition/DocumentReference, y todos viajan—.
    #[test]
    fn los_actores_y_recursos_clinicos_estan_dentro_y_nada_cuelga() {
        let b = clinico::ensamblar(clinico::Encuentro::ConsultaExterna, &insumo()).unwrap();
        let v = serde_json::to_value(&b).unwrap();

        let tipos: Vec<&str> = v["entry"].as_array().unwrap().iter()
            .map(|e| e["resource"]["resourceType"].as_str().unwrap())
            .collect();
        assert_eq!(tipos[0], "Composition", "la Composition va primera");
        for t in ["Patient", "Organization", "Practitioner", "Encounter", "Condition",
                  "DocumentReference"] {
            assert!(tipos.contains(&t), "falta el recurso {t}: {tipos:?}");
        }

        let colgantes = referencias_colgantes(&v);
        assert!(colgantes.is_empty(), "no debería colgar ninguna: {colgantes:?}");
        let dup = ids_duplicados(&v);
        assert!(dup.is_empty(), "ningún id se repite: {dup:?}");
    }

    /// **LAS SECCIONES QUE FASE 4 PUEBLA LLEVAN `entry`, NO `nilknown`** (#603).
    ///
    /// Sustituye a `hoy_las_secciones_van_vacias_y_se_sabe`, que afirmaba 0 secciones
    /// pobladas y estaba escrita para ponerse roja al construir los recursos. En
    /// consulta externa se pueblan `sectionProblems` (Condition) y
    /// `sectionAddendumDocuments` (DocumentReference); las demás siguen con su
    /// `emptyReason=nilknown` confirmado. Una sección con `entry` NO puede llevar
    /// `emptyReason` (cmp-2), y esta prueba lo sujeta por los dos lados.
    #[test]
    fn las_secciones_de_fase_4_se_pueblan_con_entry() {
        let b = clinico::ensamblar(clinico::Encuentro::ConsultaExterna, &insumo()).unwrap();
        let v = serde_json::to_value(&b).unwrap();
        let secciones = v["entry"][0]["resource"]["section"].as_array().unwrap();

        let pobladas: Vec<&str> = secciones.iter()
            .filter(|s| s.get("entry").is_some())
            .map(|s| s["code"]["coding"][0]["code"].as_str().unwrap())
            .collect();
        // `sectionProblems` (11450-4) y `sectionAddendumDocuments` (55107-7).
        assert!(pobladas.contains(&"11450-4"), "sectionProblems poblada: {pobladas:?}");
        assert!(pobladas.contains(&"55107-7"), "sectionAddendumDocuments poblada: {pobladas:?}");

        // cmp-2: ninguna sección lleva `entry` Y `emptyReason` a la vez.
        for s in secciones {
            assert!(
                !(s.get("entry").is_some() && s.get("emptyReason").is_some()),
                "una sección viola cmp-2 (entry y emptyReason): {s}"
            );
        }
    }

    /// Aval del fixture COMPARTIDO: la unión de las 4 confirmaciones del RDA de
    /// paciente y las 12 de los clínicos, porque este `insumo()` se usa para
    /// ensamblar los dos. Confirmar un código que un documento no emite es inocuo
    /// (la puerta solo mira las secciones que existen). Sin la unión, ensamblar un
    /// RDA clínico desde aquí falla la puerta del nilknown (#603).
    const CONFIRMADAS: [Confirmacion<'static>; 16] = [
        paciente::CONFIRMACIONES[0], paciente::CONFIRMACIONES[1],
        paciente::CONFIRMACIONES[2], paciente::CONFIRMACIONES[3],
        clinico::CONFIRMACIONES[0], clinico::CONFIRMACIONES[1],
        clinico::CONFIRMACIONES[2], clinico::CONFIRMACIONES[3],
        clinico::CONFIRMACIONES[4], clinico::CONFIRMACIONES[5],
        clinico::CONFIRMACIONES[6], clinico::CONFIRMACIONES[7],
        clinico::CONFIRMACIONES[8], clinico::CONFIRMACIONES[9],
        clinico::CONFIRMACIONES[10], clinico::CONFIRMACIONES[11],
    ];

    fn insumo() -> paciente::Insumo<'static> {
        use actores::{Institucion, Nombre, Paciente, Profesional};
        use paciente::{Diagnostico, Eps, Servicio};
        let nombre = Nombre {
            nombre1: "Primernombre", nombre2: None,
            apellido1: "Primerapellido", apellido2: None,
        };
        paciente::Insumo {
            paciente: Paciente {
                tipo_doc: "CC", numero_doc: "000000000",
                tipo_doc_display: "Cédula ciudadanía",
                nombre,
                genero_fhir: "male",
                sexo_colombiano: None,
                nacimiento: "1999-01-01", hora_nacimiento: None,
                fallecido: None,
                municipio: None, pais: None, zona: None,
                nacionalidad: None, etnia: None, discapacidad: None, identidad_genero: None,
            },
            profesional: Profesional {
                tipo_doc: "CC", numero_doc: "111111111",
                tipo_doc_display: "Cédula ciudadanía", nombre,
            },
            institucion: Institucion { codigo_habilitacion: "0500100001", nit: "900123456" },
            fecha: "2026-08-14T10:00:00-05:00",
            modalidad: Some("01"),
            grupo_servicios: Some("01"),
            periodo: Some(("2026-08-14T18:00:00-05:00", "2026-08-14T18:42:00-05:00")),
            aval: Aval::nuevo(&CONFIRMADAS),
            // Datos del encuentro clínico (#603 fase 4), del ejemplo oficial.
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

    /// Un `id` repetido lo ve `ids_duplicados` y NO lo ve
    /// `referencias_colgantes` — que es justo por lo que hacen falta las dos.
    #[test]
    fn un_id_repetido_se_ve_y_no_lo_veia_nadie() {
        let bundle = serde_json::json!({
            "resourceType": "Bundle",
            "entry": [
                { "resource": { "resourceType": "Composition",
                                "author": [{ "reference": "#CC-1" }] } },
                { "resource": { "resourceType": "Patient", "id": "CC-1" } },
                { "resource": { "resourceType": "Practitioner", "id": "CC-1" } },
            ]
        });
        assert_eq!(ids_duplicados(&bundle), vec!["CC-1".to_string()]);
        // Y LA MITAD QUE IMPORTA: el instrumento anterior sale VERDE aquí.
        assert!(referencias_colgantes(&bundle).is_empty(),
                "documentado, no deseado: por eso hace falta ids_duplicados");
    }

    /// La pareja: sin repetidos, no denuncia nada.
    #[test]
    fn sin_repetidos_no_denuncia() {
        let b = clinico::ensamblar(clinico::Encuentro::ConsultaExterna, &insumo()).unwrap();
        let v = serde_json::to_value(&b).unwrap();
        assert!(ids_duplicados(&v).is_empty(), "{:?}", ids_duplicados(&v));
    }

    /// El Bundle declara idioma, como el ejemplo oficial. Faltaba.
    #[test]
    fn el_bundle_declara_su_idioma() {
        let b = clinico::ensamblar(clinico::Encuentro::ConsultaExterna, &insumo()).unwrap();
        assert_eq!(serde_json::to_value(&b).unwrap()["language"], "es-CO");
    }

    /// LA PAREJA, sin la cual `referencias_colgantes` podría devolverlo todo
    /// siempre y la de arriba pasaría igual: una referencia que SÍ resuelve no
    /// puede salir como colgante.
    #[test]
    fn una_referencia_que_resuelve_no_cuelga() {
        let v = serde_json::json!({
            "resourceType": "Bundle",
            "entry": [
                { "resource": { "resourceType": "Composition",
                                "subject": { "reference": "#p1" },
                                "custodian": { "reference": "#NO-ESTA" } } },
                { "resource": { "resourceType": "Patient", "id": "p1" } }
            ]
        });
        assert_eq!(referencias_colgantes(&v), vec!["NO-ESTA".to_string()]);
    }

    #[test]
    fn una_referencia_lleva_almohadilla() {
        // El ejemplo oficial usa `#<id>`, no `urn:uuid:`. Si esto cambia, el
        // servidor deja de resolver las referencias del documento.
        assert_eq!(Ref::a("Condition-0").reference, "#Condition-0");
    }

    #[test]
    fn el_perfil_se_construye_sobre_la_base_canonica() {
        assert_eq!(
            Meta::perfil("CompositionPatientStatementRDA").profile[0],
            "https://fhir.minsalud.gov.co/rda/StructureDefinition/CompositionPatientStatementRDA"
        );
    }

    #[test]
    fn una_cadena_en_blanco_cuenta_como_ausente() {
        // La pareja del de abajo, y es la que importa: si «   » pasara por
        // presente, el Bundle saldría con un campo vacío y el servidor lo
        // aceptaría. Un dato inventado no se nota nunca.
        let mut f = Faltas::default();
        f.exigir(Some("   "), "Patient", "identifier", "sin documento no hay paciente");
        f.exigir(None, "Patient", "name", "sin nombre no hay paciente");
        assert_eq!(f.0.len(), 2, "las dos formas de ausencia tienen que contar");
        assert!(f.hay());
    }

    #[test]
    fn un_valor_de_verdad_no_es_falta() {
        // La pareja: sin esto pasaría igual un `exigir` que se queja de todo.
        let mut f = Faltas::default();
        f.exigir(Some("CC-1234"), "Patient", "identifier", "…");
        assert!(!f.hay(), "un valor presente no puede contar como falta");
    }

    #[test]
    fn las_faltas_se_acumulan_y_se_leen_juntas() {
        let mut f = Faltas::default();
        f.exigir(None, "Composition", "date", "sin fecha no hay documento");
        f.exigir(None, "Composition", "author", "sin autor no hay quién responda");
        let t = f.to_string();
        assert!(t.contains("Composition.date"), "{t}");
        assert!(t.contains("Composition.author"), "{t}");
    }

    /// LA PAREJA QUE FALTABA, y la que habría impedido una regresión entera.
    ///
    /// El instrumento —`referencias_colgantes`— llevaba dos commits escrito, y
    /// las pruebas que lo corrían usaban SOLO fixtures limpias. Un artefacto
    /// limpio devuelve vacío con la regla buena y con la regla ciega, así que el
    /// banco medía el ARTEFACTO y no el VIGILANTE (directiva 33).
    ///
    /// Cuando el borde se puso en `actores::ensamblar`, `Organization.id` empezó
    /// a salir recortado mientras `Insumo::id_institucion` seguía leyendo el
    /// crudo: `custodian`, `author` y `attester.party` quedaron colgando en los
    /// CUATRO documentos. Las 60 pruebas siguieron verdes.
    ///
    /// Ésta mete blanco de borde por los DOS ensambladores reales y exige que
    /// nada cuelgue. Su pareja verde es la fixture limpia de siempre.
    #[test]
    fn el_blanco_de_borde_no_descuelga_ninguna_referencia() {
        let sucio = |mut i: paciente::Insumo<'static>| {
            i.institucion.codigo_habilitacion = " 0500100001 ";
            i.fecha = " 2026-08-15T10:00:00-05:00 ";
            i.paciente.tipo_doc = " CC ";
            i.paciente.numero_doc = " 000000000 ";
            i
        };

        // 1 · El RDA de paciente.
        let b = paciente::ensamblar(&sucio(insumo())).expect("válido una vez recortado");
        let v = serde_json::to_value(&b).expect("serializa");
        assert!(
            referencias_colgantes(&v).is_empty(),
            "RDA de paciente: {:?}", referencias_colgantes(&v)
        );

        // 2 · Los tres encuentros clínicos.
        for e in [clinico::Encuentro::ConsultaExterna,
                  clinico::Encuentro::Urgencias,
                  clinico::Encuentro::Hospitalizacion] {
            let mut i = sucio(insumo());
            i.paciente.fallecido = Some(false);
            let b = clinico::ensamblar(e, &i).expect("válido una vez recortado");
            let v = serde_json::to_value(&b).expect("serializa");
            assert!(
                referencias_colgantes(&v).is_empty(),
                "{e:?}: {:?}", referencias_colgantes(&v)
            );
        }
    }

    /// Y el POSITIVO DE CONTROL del instrumento, que es lo que distingue «no
    /// cuelga nada» de «el detector no mira». Si `referencias_colgantes`
    /// devolviera siempre vacío, la prueba de arriba pasaría igual.
    #[test]
    fn y_el_detector_de_colgantes_si_ve_uno_de_verdad() {
        let b = paciente::ensamblar(&insumo()).expect("la fixture limpia es válida");
        let mut v = serde_json::to_value(&b).expect("serializa");
        // Se descuelga a mano la referencia del custodio, que es justo la que la
        // regresión rompió.
        v["entry"][0]["resource"]["custodian"]["reference"] = serde_json::json!("#no-existe");
        assert_eq!(
            referencias_colgantes(&v),
            vec!["no-existe".to_string()],
            "el detector no vio una referencia que SÍ cuelga"
        );
    }

    /// LOS DOS CAMPOS QUE EL BORDE TRAJO Y QUE NO VIGILABA NADIE.
    ///
    /// `Insumo::recortado` recorta `fecha` y `modalidad` además de los tres
    /// actores, y eso era la aportación propia de ese commit. La ronda 4 lo
    /// midió: quitando los dos `.trim()` las 64 pruebas seguían VERDES, porque
    /// `el_blanco_de_borde_no_descuelga_ninguna_referencia` ensucia la fecha
    /// pero solo mira las referencias, y ni `fecha` ni `modalidad` participan
    /// en ninguna.
    ///
    /// Ésta afirma sobre el VALOR EMITIDO, que es el eje que faltaba: el otro
    /// mecanismo del borde —el `recortado()` propio de `actores::ensamblar`—
    /// no puede taparlo, porque estos dos campos no son de ningún actor.
    #[test]
    fn la_fecha_y_la_modalidad_salen_recortadas() {
        let sucio = |mut i: paciente::Insumo<'static>| {
            i.fecha = " 2026-08-15T10:00:00-05:00 ";
            i.modalidad = Some(" 01 ");
            i
        };
        let esperada = "2026-08-15T10:00:00-05:00";

        // 1 · El RDA de paciente.
        let b = paciente::ensamblar(&sucio(insumo())).expect("válido");
        let v = serde_json::to_value(&b).expect("serializa");
        let comp = &v["entry"][0]["resource"];
        assert_eq!(comp["date"], esperada, "RDA de paciente: la fecha salió cruda");
        assert_eq!(comp["event"][0]["code"][0]["coding"][0]["code"], "01",
                   "RDA de paciente: la modalidad salió cruda");

        // 2 · Los tres encuentros clínicos.
        for e in [clinico::Encuentro::ConsultaExterna,
                  clinico::Encuentro::Urgencias,
                  clinico::Encuentro::Hospitalizacion] {
            let mut i = sucio(insumo());
            i.paciente.fallecido = Some(false);
            let b = clinico::ensamblar(e, &i).expect("válido");
            let v = serde_json::to_value(&b).expect("serializa");
            let comp = &v["entry"][0]["resource"];
            assert_eq!(comp["date"], esperada, "{e:?}: la fecha salió cruda");
            assert_eq!(comp["event"][0]["code"][0]["coding"][0]["code"], "01",
                       "{e:?}: la modalidad salió cruda");
        }
    }
}
