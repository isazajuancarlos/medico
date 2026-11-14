// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza
//! Los TRES recursos que los cuatro RDA comparten: `Patient`, `Organization` y
//! `Practitioner`.
//!
//! # Por qué van juntos y por qué van primero
//!
//! Son exactamente los tres a los que apuntaban las referencias colgantes que
//! medía `hoy_el_bundle_esta_incompleto_y_se_sabe`: `subject`, `author`,
//! `attester.party` y `custodian` de **los cuatro** documentos. Construirlos una
//! vez los completa a los cuatro; construirlos dentro de cada uno habría sido
//! copiarlos cuatro veces y que divergieran a la tercera.
//!
//! # De dónde sale cada forma
//!
//! De `datos/ihce/ejemplos/enviar-rda-paciente.json`, recurso a recurso. No se
//! deduce ni una: los `system` de MinSalud no son adivinables y una errata en
//! ellos **no da error** — el servidor guarda el dato con el significado que no
//! era.
//!
//! # LO QUE MEDICO NO CAPTURA HOY, y por eso va `Option`
//!
//! El perfil `PatientRDA` lleva cuatro extensiones demográficas —nacionalidad,
//! pertenencia étnica, discapacidad e identidad de género— y una de zona de
//! residencia, y **la tabla `paciente` de medico no tiene ninguna de las cuatro
//! primeras**. Van `Option` y se OMITEN cuando faltan: no se rellenan con «otras
//! etnias» ni con «sin discapacidad», que es lo que trae el ejemplo oficial y
//! sería un dato inventado con aspecto de dato (directiva 20).
//!
//! Que se omitan no significa que sobren: significa que **falta capturarlas**, y
//! eso es trabajo de pantalla que pertenece a #84. Un RDA sin ellas puede ser
//! rechazado por el perfil el día que se valide contra la `StructureDefinition`
//! de verdad; hoy no hay forma de saberlo, y decirlo aquí es más honesto que
//! enviarlas inventadas.

use super::{Faltas, Meta};
use serde_json::{json, Value};

/// Los `CodeSystem` propios de MinSalud.
pub const CS: &str = "https://fhir.minsalud.gov.co/rda/CodeSystem";
/// Los `NamingSystem` propios de MinSalud: RNEC para personas, REPS para sedes.
pub const NS: &str = "https://fhir.minsalud.gov.co/rda/NamingSystem";
/// La tabla v2-0203 de HL7, que clasifica el TIPO de identificador. Va SIEMPRE
/// acompañada del `CodeSystem` colombiano: el ejemplo trae los dos `coding` en
/// el mismo `type`, uno internacional y uno nacional.
pub const V2: &str = "http://terminology.hl7.org/CodeSystem/v2-0203";

/// Un valor codificado que puede faltar.
///
/// `display` es `Option` porque el ejemplo lo omite en algunos `valueCoding`
/// —DIVIPOLA, por ejemplo— y ponerlo donde el ejemplo no lo pone es inventar.
#[derive(Debug, Clone, Copy)]
pub struct Codigo<'a> {
    pub code: &'a str,
    pub display: Option<&'a str>,
}

impl<'a> Codigo<'a> {
    pub fn nuevo(code: &'a str, display: &'a str) -> Self {
        Self { code, display: Some(display) }
    }
    pub fn desnudo(code: &'a str) -> Self {
        Self { code, display: None }
    }
    /// Con `code` y `display` recortados. Ver `Paciente::recortado`.
    pub(super) fn recortado(&self) -> Self {
        Self { code: self.code.trim(), display: self.display.map(str::trim) }
    }
    fn coding(&self, sistema: &str) -> Value {
        let mut v = json!({ "system": sistema, "code": self.code });
        if let Some(d) = self.display {
            v["display"] = json!(d);
        }
        v
    }
    /// Una extensión de las que cuelgan de un campo primitivo (`_gender`,
    /// `_city`) o del recurso.
    fn extension(&self, url: &str, sistema: &str) -> Value {
        json!({ "url": format!("{}/{url}", super::PERFIL), "valueCoding": self.coding(sistema) })
    }
}

/// El nombre de una persona, partido como lo parte el RDA.
///
/// **`family` va compuesto Y desglosado a la vez**, y no es redundancia: el
/// ejemplo pone los dos apellidos juntos en `family` y cada uno por separado en
/// las extensiones `ExtensionFathersFamilyName` / `ExtensionMothersFamilyName`.
/// Un lector que solo entienda FHIR estándar ve el apellido completo; el visor
/// nacional los distingue.
#[derive(Debug, Clone, Copy)]
pub struct Nombre<'a> {
    pub nombre1: &'a str,
    pub nombre2: Option<&'a str>,
    pub apellido1: &'a str,
    pub apellido2: Option<&'a str>,
}

impl<'a> Nombre<'a> {
    /// Con los cuatro trozos recortados. Ver `Paciente::recortado`.
    ///
    /// Aquí el defecto era visible en una sola línea: `a_json` decidía si el
    /// segundo nombre existe con `filter(|s| !s.trim().is_empty())` y luego lo
    /// emitía SIN recortar.
    pub(super) fn recortado(&self) -> Self {
        Self {
            nombre1: self.nombre1.trim(),
            nombre2: self.nombre2.map(str::trim),
            apellido1: self.apellido1.trim(),
            apellido2: self.apellido2.map(str::trim),
        }
    }

    fn a_json(&self) -> Value {
        let mut given = vec![json!(self.nombre1)];
        if let Some(n) = self.nombre2.filter(|s| !s.trim().is_empty()) {
            given.push(json!(n));
        }
        let mut ext = vec![json!({
            "url": format!("{}/ExtensionFathersFamilyName", super::PERFIL),
            "valueString": self.apellido1,
        })];
        let compuesto = match self.apellido2.filter(|s| !s.trim().is_empty()) {
            Some(a2) => {
                ext.push(json!({
                    "url": format!("{}/ExtensionMothersFamilyName", super::PERFIL),
                    "valueString": a2,
                }));
                format!("{} {a2}", self.apellido1)
            }
            None => self.apellido1.to_string(),
        };
        json!({
            "use": "official",
            "family": compuesto,
            "_family": { "extension": ext },
            "given": given,
        })
    }
}

/// El paciente, tal como el RDA lo necesita.
#[derive(Debug, Clone, Copy)]
pub struct Paciente<'a> {
    /// `CC`, `TI`, `RC`… Es el código colombiano, no el de v2-0203.
    pub tipo_doc: &'a str,
    pub numero_doc: &'a str,
    /// Cómo se llama ese tipo de documento (`Cédula ciudadanía`). Va al
    /// `display` del `coding` nacional.
    pub tipo_doc_display: &'a str,
    pub nombre: Nombre<'a>,
    /// `male` · `female` · `other` · `unknown`. Es el vocabulario de FHIR, NO
    /// el sexo del RIPS: la traducción la hace quien construye este struct.
    pub genero_fhir: &'a str,
    /// El sexo en el vocabulario colombiano (`ColombianGenderGroup`), que viaja
    /// aparte en `_gender`. Los dos dicen lo mismo con distinto diccionario y
    /// por eso son dos campos, no uno (directiva 32).
    pub sexo_colombiano: Option<Codigo<'a>>,
    /// `YYYY-MM-DD`.
    pub nacimiento: &'a str,
    /// La HORA de nacimiento, si se conoce. Importa en neonatología y el perfil
    /// la admite como extensión de `birthDate`.
    pub hora_nacimiento: Option<&'a str>,
    /// Municipio de residencia: código DIVIPOLA y su nombre.
    pub municipio: Option<Codigo<'a>>,
    /// País de residencia: código ISO 3166-1 numérico y su nombre.
    pub pais: Option<Codigo<'a>>,
    /// Zona de residencia (`ColombianResidenceZone`): urbana, rural…
    pub zona: Option<Codigo<'a>>,
    /// Si el paciente MURIÓ, cuando el evento lo sabe.
    ///
    /// `None` = **no hay declaración**, que no es lo mismo que «vivo». Y la
    /// distinción importa más de lo que parece: la primera versión de este
    /// módulo OMITÍA siempre el campo, con un comentario que decía que medico
    /// no registra defunción. **Era falso** —`hospitalizacion.estado` admite
    /// `DEFUNCION` y `atencion_urgencias.destino_salida` admite `FALLECIDO`,
    /// y RIPS ya los reporta—, y FHIR R4 advierte en `Patient.deceased[x]` que
    /// «la mayoría de los sistemas interpretan la ausencia de valor como señal
    /// de que la persona está viva». O sea que omitirlo en el RDA de egreso
    /// presentaba como VIVO en el visor nacional a un paciente fallecido, sin
    /// que nada diera error. Lo cazó la revisión de seguridad del 2026-08-15.
    ///
    /// Quien construye este struct decide: en el RDA de paciente y en el de
    /// consulta externa nadie lo sabe y va `None`; en egreso y urgencias sale
    /// del evento.
    pub fallecido: Option<bool>,
    /// Las cuatro que medico NO captura hoy. Ver la nota del encabezado.
    pub nacionalidad: Option<Codigo<'a>>,
    pub etnia: Option<Codigo<'a>>,
    pub discapacidad: Option<Codigo<'a>>,
    pub identidad_genero: Option<Codigo<'a>>,
}

/// El profesional que responde por el documento.
///
/// Lleva MENOS que el paciente a propósito: en el ejemplo oficial el
/// `Practitioner` trae solo identificador y nombre. No se le añade nada que el
/// ejemplo no traiga.
#[derive(Debug, Clone, Copy)]
pub struct Profesional<'a> {
    pub tipo_doc: &'a str,
    pub numero_doc: &'a str,
    pub tipo_doc_display: &'a str,
    pub nombre: Nombre<'a>,
}

/// La sede prestadora: quien custodia y atesta.
#[derive(Debug, Clone, Copy)]
pub struct Institucion<'a> {
    /// Código de habilitación en REPS. **No es el NIT**, y es el `id` del
    /// recurso y el destino de `custodian`.
    pub codigo_habilitacion: &'a str,
    /// NIT de la persona jurídica.
    ///
    /// Obligatorio, y aquí está la única cosa que este módulo hace DISTINTO del
    /// ejemplo oficial a propósito: el ejemplo trae `"value": "Desconocido"`.
    /// Eso es un relleno, y un relleno viaja al índice nacional como si fuera un
    /// NIT. El prestador conoce el suyo siempre; si algún día no lo conoce,
    /// que falle ruidosamente (directiva 20).
    pub nit: &'a str,
}

/// DE DÓNDE SALE `Paciente.fallecido`, POR VOCABULARIO Y NO POR MEMORIA.
///
/// Existe porque la guarda de `clinico::ensamblar` EXIGE que el desenlace se
/// declare en egreso y urgencias, y sin esto no decía de dónde sacarlo. Una
/// guarda que pide una respuesta sin señalar la fuente empuja al que cablea
/// hacia la más barata —`Some(false)` incondicional—, que es la peor: afirma
/// «vivo» sobre episodios que ni siquiera han terminado.
///
/// # Es una LISTA y no un `match`, y eso es el arreglo
///
/// La primera versión era un `match` con los valores tecleados, y su prueba
/// comprobaba `archivo.contains("DEFUNCION")`. Eso no vigila nada en ninguna de
/// las dos direcciones: un estado NUEVO en la columna no rompe ningún
/// `contains`, y un estado RETIRADO tampoco, porque `REMISION` aparece además
/// en el `CHECK` de `via_ingreso` —otra columna, otro significado— y el
/// `contains` mira el archivo entero. Con la lista, el conjunto de claves es un
/// dato que la prueba puede comparar contra el `CHECK` de ESA columna en los
/// dos sentidos.
///
/// # Y el significado sale del CATÁLOGO, no de los tokens
///
/// Éste fue el error de la primera versión y conviene que quede escrito: se
/// ancló a las cadenas legales de la migración —que dicen qué se puede
/// escribir— y no a `muv.rs`, que dice qué SIGNIFICAN. Dos valores se mapearon
/// mal por eso, y los dos ya estaban decididos por escrito en esta casa.
const EGRESO: &[(&str, Option<bool>)] = &[
    // `ACTIVA` no declara nada: la hospitalización sigue abierta, no hay
    // desenlace, y la guarda lo rechazará — que es lo correcto, porque no se
    // emite el RDA de EGRESO de quien no ha egresado.
    ("ACTIVA", None),
    ("ALTA_MEDICA", Some(false)),
    ("ALTA_VOLUNTARIA", Some(false)),
    ("REMISION", Some(false)),
    ("DEFUNCION", Some(true)),
];

/// El vocabulario es el del `CHECK (estado …)` de `0017_hospitalizacion.sql`.
pub fn fallecido_de_egreso(estado: &str) -> Option<bool> {
    EGRESO.iter().find(|(k, _)| *k == estado.trim()).and_then(|(_, v)| *v)
}

const URGENCIAS: &[(&str, Option<bool>)] = &[
    // `OBSERVACION` NO declara nada, y esto se corrigió el 2026-08-15: decía
    // `Some(false)`. El catálogo (`muv.rs`) lo traduce al código 08, «PACIENTE
    // CONTINUA EN EL SERVICIO (CORTE FACTURACION)» — o sea que el episodio no
    // ha terminado, exactamente igual que `ACTIVA` en hospitalización. Decir
    // `Some(false)` abría la compuerta que `ACTIVA` existe para cerrar, y
    // dejaba salir un RDA de urgencias de una atención en curso.
    ("OBSERVACION", None),
    // `FUGA` TAMPOCO, y aquí la casa ya había decidido: `muv.rs` devuelve
    // `None` a propósito para este mismo valor, con su motivo escrito —no
    // convertir un abandono del servicio, que tiene consecuencias clínicas y
    // legales, en una atención terminada con normalidad—. La versión anterior
    // razonaba «se fue por su pie, luego estaba vivo», y eso NO es un dato
    // registrado: es una inferencia sobre una ausencia. La fuga se anota al
    // DESCUBRIR que el paciente no está; nadie vio su estado al salir.
    // Afirmar «vivo» en el índice nacional sobre eso es la simétrica exacta del
    // hallazgo de la primera revisión, que era omitir el campo.
    ("FUGA", None),
    ("DOMICILIO", Some(false)),
    ("HOSPITALIZACION", Some(false)),
    ("REMISION", Some(false)),
    ("FALLECIDO", Some(true)),
];

/// El vocabulario es el del `CHECK (destino_salida …)` de `0016_urgencias.sql`.
pub fn fallecido_de_urgencias(destino: &str) -> Option<bool> {
    URGENCIAS.iter().find(|(k, _)| *k == destino.trim()).and_then(|(_, v)| *v)
}

/// El `id` interno de una persona dentro del Bundle: `<tipoDoc>-<numero>`.
///
/// **Recorta los dos trozos**, y no es cosmética: `Faltas::exigir` recortaba
/// para DECIDIR si el dato estaba y luego se emitía sin recortar, así que un
/// `"123456 "` pasaba la puerta y viajaba con el espacio a tres sitios a la vez
/// —`Patient.id`, la referencia `#…` y `identifier.value`—.
pub fn id_persona(tipo_doc: &str, numero_doc: &str) -> String {
    format!("{}-{}", tipo_doc.trim(), numero_doc.trim())
}

/// El tipo `id` de FHIR R4 está fijado por regex: `[A-Za-z0-9\-\.]{1,64}`.
///
/// Se comprueba AQUÍ porque este módulo es el único que construye ese `id` y
/// el único que lo convierte en referencia. No lo comprueba nadie más: la
/// columna `paciente.numero_documento` es `TEXT NOT NULL` **sin `CHECK`**, y la
/// ruta de alta solo valida el municipio.
///
/// Y el fallo es SILENCIOSO cuando el servidor es permisivo —lo es: su propio
/// ejemplo publica `"id": "Códigohabilitaciónprestador"`, con tildes, que viola
/// esta misma regex—. Si lo acepta, el índice nacional indexa al paciente por
/// un identificador con espacio y **sus atenciones en otros prestadores no se
/// unen a este RDA**: el médico que consulte el visor ve una historia
/// incompleta sin ninguna señal de que falta algo.
fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// El bloque `identifier` de una persona: el par (tipo internacional, tipo
/// nacional) y el número.
fn identificador_persona(tipo_doc: &str, display: &str, numero: &str, sistema: Option<&str>) -> Value {
    let mut v = json!({
        "id": "NationalPersonIdentifier-0",
        "use": "official",
        "type": { "coding": [
            { "system": V2, "code": "PN", "display": "Person number" },
            { "system": format!("{CS}/ColombianPersonIdentifier"), "code": tipo_doc, "display": display },
        ]},
        "value": numero.trim(),
    });
    if let Some(s) = sistema {
        v["system"] = json!(s);
    }
    v
}

impl Paciente<'_> {
    /// Una copia con TODO campo de texto recortado.
    ///
    /// # Por qué existe, y por qué en el borde y no en cada emisión
    ///
    /// La revisión de seguridad del 2026-08-15 encontró que `exigir` recortaba
    /// para DECIDIR si el dato estaba y luego se emitía SIN recortar. Se
    /// arreglaron los TRES sitios que el informe nombraba —`Patient.id`, la
    /// referencia `#…` y `identifier.value`— y la segunda revisión encontró que
    /// quedaban cuatro más: `identifier.type.coding.code`, `gender`,
    /// `Organization.id` y el segundo nombre. Arreglar la LISTA en vez de la
    /// REGLA es lo que produjo esa segunda tanda (directiva 23).
    ///
    /// Así que no se recorta en cada emisión: se recorta UNA vez al entrar en
    /// `ensamblar`, y a partir de ahí **el valor crudo no existe**. Lo que
    /// `faltas()` juzga y lo que `a_json()` escribe son el mismo objeto, no dos
    /// lecturas del mismo campo — que es la propiedad que hay que sostener, y
    /// la única forma de sostenerla sin una lista que ampliar.
    pub(super) fn recortado(&self) -> Self {
        Self {
            tipo_doc: self.tipo_doc.trim(),
            numero_doc: self.numero_doc.trim(),
            tipo_doc_display: self.tipo_doc_display.trim(),
            nombre: self.nombre.recortado(),
            genero_fhir: self.genero_fhir.trim(),
            sexo_colombiano: self.sexo_colombiano.map(|c| c.recortado()),
            nacimiento: self.nacimiento.trim(),
            hora_nacimiento: self.hora_nacimiento.map(str::trim),
            municipio: self.municipio.map(|c| c.recortado()),
            pais: self.pais.map(|c| c.recortado()),
            zona: self.zona.map(|c| c.recortado()),
            fallecido: self.fallecido,
            nacionalidad: self.nacionalidad.map(|c| c.recortado()),
            etnia: self.etnia.map(|c| c.recortado()),
            discapacidad: self.discapacidad.map(|c| c.recortado()),
            identidad_genero: self.identidad_genero.map(|c| c.recortado()),
        }
    }

    fn faltas(&self, f: &mut Faltas) {
        f.exigir(Some(self.tipo_doc), "Patient", "identifier.type",
                 "sin tipo de documento no se identifica al paciente en el índice nacional");
        f.exigir(Some(self.numero_doc), "Patient", "identifier.value",
                 "sin número de documento el RDA no se vincula a nadie");
        f.exigir(Some(self.nombre.nombre1), "Patient", "name.given",
                 "sin nombre, quien lea la historia en otra institución no sabe de quién es");
        f.exigir(Some(self.nombre.apellido1), "Patient", "name.family",
                 "sin apellido no hay nombre oficial que casar con el índice nacional");
        f.exigir(Some(self.nacimiento), "Patient", "birthDate",
                 "la edad decide dosis y tamizajes; sin fecha de nacimiento el resumen engaña");
        f.exigir(Some(self.genero_fhir), "Patient", "gender",
                 "el perfil lo exige y condiciona qué antecedentes tienen sentido");
        if !id_valido(&id_persona(self.tipo_doc, self.numero_doc)) {
            f.0.push(super::Falta {
                recurso: "Patient",
                campo: "id",
                porque: "un documento con espacios, símbolos o más de 64 caracteres no \
                         se puede indexar en el índice nacional, y las atenciones del \
                         paciente en otros prestadores no se unirían a este resumen",
            });
        }
        // El vocabulario de `Patient.gender` es CERRADO y se conoce aquí. Es el
        // único de todo el módulo que no depende de un catálogo colombiano, así
        // que se comprueba en vez de delegarlo: un «M» produciría un `gender`
        // fuera del value set que el servidor puede aceptar igual.
        if !matches!(self.genero_fhir.trim(), "male" | "female" | "other" | "unknown") {
            f.0.push(super::Falta {
                recurso: "Patient",
                campo: "gender",
                porque: "FHIR solo admite male, female, other o unknown; el sexo del \
                         diccionario propio se traduce antes de llegar aquí",
            });
        }
    }

    /// El recurso `Patient`, con `id` = `<tipoDoc>-<numero>`.
    pub fn a_json(&self) -> Value {
        let mut extensiones = Vec::new();
        for (codigo, url, sistema) in [
            (self.nacionalidad, "ExtensionPatientNationality", "ISO31661"),
            (self.etnia, "ExtensionPatientEthnicity", "ColombianEthnicGroup"),
            (self.discapacidad, "ExtensionPatientDisability", "ColombianDisabilityClassification"),
            (self.identidad_genero, "ExtensionPatientGenderIdentity", "ColombianGenderIdentity"),
        ] {
            if let Some(c) = codigo {
                extensiones.push(c.extension(url, &format!("{CS}/{sistema}")));
            }
        }

        let mut r = json!({
            "resourceType": "Patient",
            "id": id_persona(self.tipo_doc, self.numero_doc),
            "meta": Meta::perfil("PatientRDA"),
            "identifier": [identificador_persona(
                self.tipo_doc, self.tipo_doc_display, self.numero_doc,
                Some(&format!("{NS}/RNEC")))],
            "name": [self.nombre.a_json()],
            // `active` es del REGISTRO, no de la persona: dice que esta ficha
            // está vigente. Un paciente que medico está resumiendo lo está.
            "active": true,
            "gender": self.genero_fhir,
            "birthDate": self.nacimiento,
        });
        if !extensiones.is_empty() {
            r["extension"] = json!(extensiones);
        }
        if let Some(s) = self.sexo_colombiano {
            r["_gender"] = json!({ "extension": [
                s.extension("ExtensionBiologicalGender", &format!("{CS}/ColombianGenderGroup"))
            ]});
        }
        if let Some(h) = self.hora_nacimiento.filter(|s| !s.trim().is_empty()) {
            r["_birthDate"] = json!({ "extension": [{
                "url": format!("{}/ExtensionBirthTime", super::PERFIL),
                "valueTime": h,
            }]});
        }
        // `deceasedBoolean` solo si el evento lo sabe. Ausente = «sin
        // declaración»; ver la nota del campo, que corrige una premisa falsa.
        if let Some(m) = self.fallecido {
            r["deceasedBoolean"] = json!(m);
        }
        if let Some(dir) = self.direccion() {
            r["address"] = json!([dir]);
        }
        r
    }

    /// La dirección, solo si hay algo real que poner.
    ///
    /// Devuelve `None` **en cuanto falta el municipio**, aunque se conozcan
    /// país y zona. Es deliberado y tiene precio: un paciente migrante del que
    /// se sabe el país pero no el municipio sale sin `address` ninguno. Se
    /// prefiere a emitir una dirección que no localiza nada, pero está escrito
    /// aquí porque el comentario anterior decía otra cosa —«ni municipio ni
    /// país ni zona»— y describir mal lo que el código hace es cómo una
    /// decisión se convierte en un descuido.
    fn direccion(&self) -> Option<Value> {
        let m = self.municipio?;
        let mut d = json!({ "id": "HomeAddress-0", "use": "home", "type": "physical" });
        if let Some(n) = m.display {
            d["city"] = json!(n);
        }
        d["_city"] = json!({ "extension": [
            Codigo::desnudo(m.code).extension("ExtensionDivipolaMunicipality", &format!("{CS}/DIVIPOLA"))
        ]});
        if let Some(p) = self.pais {
            if let Some(n) = p.display {
                d["country"] = json!(n);
            }
            d["_country"] = json!({ "extension": [
                Codigo::desnudo(p.code).extension("ExtensionCountryCode", &format!("{CS}/ISO31661"))
            ]});
        }
        if let Some(z) = self.zona {
            d["extension"] = json!([
                z.extension("ExtensionResidenceZone", &format!("{CS}/ColombianResidenceZone"))
            ]);
        }
        Some(d)
    }
}

impl<'a> Profesional<'a> {
    /// Ver `Paciente::recortado`.
    pub(super) fn recortado(&self) -> Self {
        Self {
            tipo_doc: self.tipo_doc.trim(),
            numero_doc: self.numero_doc.trim(),
            tipo_doc_display: self.tipo_doc_display.trim(),
            nombre: self.nombre.recortado(),
        }
    }

    fn faltas(&self, f: &mut Faltas) {
        f.exigir(Some(self.tipo_doc), "Practitioner", "identifier.type",
                 "sin tipo de documento del profesional no hay autor que responda");
        f.exigir(Some(self.numero_doc), "Practitioner", "identifier.value",
                 "sin número de documento del profesional no hay autor que responda");
        f.exigir(Some(self.nombre.nombre1), "Practitioner", "name.given",
                 "el visor nacional enseña quién atendió; un número solo no lo dice");
        f.exigir(Some(self.nombre.apellido1), "Practitioner", "name.family",
                 "el visor nacional enseña quién atendió; un número solo no lo dice");
        if !id_valido(&id_persona(self.tipo_doc, self.numero_doc)) {
            f.0.push(super::Falta {
                recurso: "Practitioner",
                campo: "id",
                porque: "un documento con espacios o símbolos rompe la referencia que \
                         nombra a quien responde por el documento",
            });
        }
    }

    pub fn a_json(&self) -> Value {
        json!({
            "resourceType": "Practitioner",
            "id": id_persona(self.tipo_doc, self.numero_doc),
            "meta": Meta::perfil("PractitionerRDA"),
            // Sin `system`: el ejemplo se lo pone al paciente (RNEC) y NO al
            // profesional. Se sigue el ejemplo, que es lo único medido.
            "identifier": [identificador_persona(
                self.tipo_doc, self.tipo_doc_display, self.numero_doc, None)],
            "name": [self.nombre.a_json()],
        })
    }
}

impl<'a> Institucion<'a> {
    /// Ver `Paciente::recortado`.
    pub(super) fn recortado(&self) -> Self {
        Self {
            codigo_habilitacion: self.codigo_habilitacion.trim(),
            nit: self.nit.trim(),
        }
    }

    fn faltas(&self, f: &mut Faltas) {
        f.exigir(Some(self.codigo_habilitacion), "Organization", "identifier",
                 "el código de habilitación REPS custodia y atesta el documento");
        f.exigir(Some(self.nit), "Organization", "identifier.TaxIdentifier",
                 "el NIT identifica a la persona jurídica responsable; no se manda «Desconocido»");
        // EL TERCER `id` DEL BUNDLE, que el primer arreglo se dejó fuera.
        //
        // `id_valido` decía en su propio docstring cubrir «el único módulo que
        // construye ese `id` y lo convierte en referencia». Construye TRES y
        // comprobaba dos: `Organization.id` sale de `codigo_habilitacion`, que
        // es `TEXT` anulable y SIN `CHECK` (migración 0037) y se llena desde un
        // campo de texto libre del panel de administración.
        //
        // Y con él viajan `custodian` y `attester.party`/`author` de los CUATRO
        // documentos. El fallo es silencioso por la misma razón que el del
        // paciente: el servidor es permisivo —su propio ejemplo publica
        // `"id": "Códigohabilitaciónprestador"`, con tildes—.
        // Solo la FORMA: la ausencia ya la reporta `exigir` de arriba, y dos
        // faltas por una sola causa es «cuatro intentos» en vez de «una
        // corrección» — lo cazó `las_faltas_salen_todas_juntas` al escribir
        // esto, que es exactamente para lo que está.
        if !self.codigo_habilitacion.is_empty() && !id_valido(self.codigo_habilitacion) {
            f.0.push(super::Falta {
                recurso: "Organization",
                campo: "id",
                porque: "un código de habilitación con espacios, símbolos o más de 64 \
                         caracteres rompe el `id` de la sede y las referencias que la \
                         nombran como custodio y como quien atesta",
            });
        }
    }

    pub fn a_json(&self) -> Value {
        json!({
            "resourceType": "Organization",
            "id": self.codigo_habilitacion,
            "meta": Meta::perfil("CareDeliveryOrganizationRDA"),
            "identifier": [
                {
                    "id": "TaxIdentifier-0",
                    "use": "official",
                    "type": { "coding": [
                        { "system": V2, "code": "TAX", "display": "Tax ID number" },
                        { "system": format!("{CS}/ColombianOrganizationIdentifiers"),
                          "code": "NIT",
                          "display": "Número de Identificación Tributaria" },
                    ]},
                    "value": self.nit,
                },
                {
                    "id": "HealthcareProviderIdentifier-0",
                    "use": "official",
                    "type": { "coding": [
                        { "system": V2, "code": "PRN", "display": "Provider number" },
                        { "system": format!("{CS}/ColombianOrganizationIdentifiers"),
                          "code": "CodigoPrestador",
                          "display": "Código de habilitación de prestador de servicios de salud" },
                    ]},
                    "system": format!("{NS}/REPS"),
                    "value": self.codigo_habilitacion,
                },
            ],
        })
    }
}

/// Los tres actores, en el ORDEN del ejemplo oficial: paciente, institución,
/// profesional.
///
/// El orden importa poco para el servidor y mucho para comparar contra el
/// ejemplo, así que se respeta y se fija con una prueba.
pub fn ensamblar(
    paciente: &Paciente<'_>,
    institucion: &Institucion<'_>,
    profesional: &Profesional<'_>,
) -> Result<Vec<Value>, Faltas> {
    // EL BORDE. A partir de aquí el valor crudo NO EXISTE: lo que se juzga y
    // lo que se emite son el mismo objeto (ver `Paciente::recortado`).
    let paciente = &paciente.recortado();
    let institucion = &institucion.recortado();
    let profesional = &profesional.recortado();

    let mut f = Faltas::default();
    paciente.faltas(&mut f);
    institucion.faltas(&mut f);
    profesional.faltas(&mut f);
    // EL PACIENTE Y EL PROFESIONAL NO PUEDEN SER EL MISMO RECURSO.
    //
    // Sus `id` salen los dos de `<tipoDoc>-<numero>`, así que un profesional
    // atendido en su propia institución con su propio usuario como autor
    // produce `Patient.id == Practitioner.id`: dos entradas con el mismo `id`,
    // y `#CC-X` resuelve por ORDEN. `author` y `attester.party` —el que nombra
    // a quien responde JURÍDICAMENTE— acabarían resolviendo a un `Patient`,
    // que no tiene identidad profesional.
    //
    // Se falla ruidosamente en vez de renombrar (directiva 20): renombrar el
    // `id` del profesional sería desviarse del ejemplo oficial, que lo usa
    // desnudo.
    if id_persona(paciente.tipo_doc, paciente.numero_doc)
        == id_persona(profesional.tipo_doc, profesional.numero_doc)
    {
        f.0.push(super::Falta {
            recurso: "Composition",
            campo: "author",
            porque: "el paciente y el profesional no pueden ser la misma persona en el \
                     mismo documento: sus dos recursos compartirían `id` y la referencia \
                     que atesta legalmente resolvería al paciente",
        });
    }
    if f.hay() {
        return Err(f);
    }
    Ok(vec![paciente.a_json(), institucion.a_json(), profesional.a_json()])
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ihce::arnes;

    const EJEMPLO: &str = include_str!("../../datos/ihce/ejemplos/enviar-rda-paciente.json");

    fn oficial(tipo: &str) -> Value {
        arnes::recurso(EJEMPLO, tipo)
    }

    fn nombre() -> Nombre<'static> {
        Nombre {
            nombre1: "Primernombre",
            nombre2: Some("Segundonombre"),
            apellido1: "Primerapellido",
            apellido2: Some("Segundoapellido"),
        }
    }

    fn paciente() -> Paciente<'static> {
        Paciente {
            tipo_doc: "CC",
            numero_doc: "000000000",
            tipo_doc_display: "Cédula ciudadanía",
            nombre: nombre(),
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
        }
    }

    /// El paciente con TODO poblado.
    ///
    /// Existe por la revisión del 2026-08-15: `no_inventamos_ni_un_sistema…`
    /// usaba el fixture normal, que deja en `None` la hora de nacimiento y las
    /// cuatro extensiones demográficas — o sea que **nueve** de las cadenas que
    /// el módulo puede emitir (`ExtensionBirthTime`, las cuatro
    /// `ExtensionPatient*` y sus cuatro `CodeSystem`) no las miraba el guardián
    /// que el módulo declara como su defensa contra la errata que no da error.
    /// Las nueve resultaron correctas; el agujero era del instrumento.
    fn paciente_completo() -> Paciente<'static> {
        let mut p = paciente();
        p.hora_nacimiento = Some("14:30:00");
        p.nacionalidad = Some(Codigo::nuevo("170", "Colombia"));
        p.etnia = Some(Codigo::nuevo("6", "Otras etnias"));
        p.discapacidad = Some(Codigo::nuevo("08", "Sin discapacidad"));
        p.identidad_genero = Some(Codigo::nuevo("01", "Masculino"));
        p.fallecido = Some(false);
        p
    }

    fn profesional() -> Profesional<'static> {
        Profesional {
            tipo_doc: "CC",
            numero_doc: "1111111111",
            tipo_doc_display: "Cédula ciudadanía",
            nombre: nombre(),
        }
    }

    fn institucion() -> Institucion<'static> {
        Institucion { codigo_habilitacion: "0500100001", nit: "900123456" }
    }

    /// Todos los `system` y `url` que un valor emite, en cualquier profundidad.
    fn vocabulario(v: &Value) -> std::collections::BTreeSet<String> {
        let mut s = std::collections::BTreeSet::new();
        fn hurgar(v: &Value, s: &mut std::collections::BTreeSet<String>) {
            match v {
                Value::Object(m) => {
                    for (k, x) in m {
                        if (k == "system" || k == "url") && x.is_string() {
                            s.insert(x.as_str().unwrap().to_string());
                        }
                        hurgar(x, s);
                    }
                }
                Value::Array(a) => a.iter().for_each(|x| hurgar(x, s)),
                _ => {}
            }
        }
        hurgar(v, &mut s);
        s
    }

    /// **LA PRUEBA QUE MÁS SUJETA, y la razón de que este módulo no invente.**
    ///
    /// Ningún `system` ni `url` que emitamos puede ser uno que el ejemplo
    /// oficial no traiga. Es el modo de fallo que no da error: una errata en
    /// `https://fhir.minsalud.gov.co/rda/CodeSystem/DIVIPOLA` la acepta el
    /// servidor y guarda el municipio con un significado que no es.
    #[test]
    fn no_inventamos_ni_un_sistema_ni_una_extension() {
        for (generado, tipo) in [
            (paciente_completo().a_json(), "Patient"),
            (institucion().a_json(), "Organization"),
            (profesional().a_json(), "Practitioner"),
        ] {
            let nuestro = vocabulario(&generado);
            let suyo = vocabulario(&oficial(tipo));
            let inventados: Vec<_> = nuestro.difference(&suyo).collect();
            assert!(inventados.is_empty(), "{tipo} emite vocabulario que el ejemplo no trae: {inventados:?}");
        }
    }

    /// La pareja de la de arriba (directiva 33): si el instrumento dijera que sí
    /// a todo, un sistema falseado pasaría. Se falsea uno y TIENE que salir.
    #[test]
    fn y_el_detector_de_vocabulario_discrimina() {
        let mut falseado = paciente().a_json();
        falseado["identifier"][0]["type"]["coding"][0]["system"] =
            json!("http://terminology.hl7.org/CodeSystem/v2-0203-CON-ERRATA");
        let inventados: Vec<_> = vocabulario(&falseado)
            .difference(&vocabulario(&oficial("Patient")))
            .cloned()
            .collect();
        assert_eq!(inventados.len(), 1, "el detector no vio la errata: {inventados:?}");
    }


    /// **LA PAREJA DE `no_se_afirma_que_el_paciente_este_vivo`**, y la que
    /// faltaba el 2026-08-15.
    ///
    /// Sin ella, omitir SIEMPRE el campo pasaba por «prudencia» — y era un
    /// defecto: FHIR R4 advierte que la ausencia se lee como que la persona
    /// vive, así que el RDA de egreso de un paciente fallecido lo presentaba
    /// como vivo en el visor nacional.
    #[test]
    fn cuando_el_evento_sabe_de_la_muerte_el_documento_lo_dice() {
        let mut p = paciente();
        p.fallecido = Some(true);
        assert_eq!(p.a_json()["deceasedBoolean"], true);

        p.fallecido = Some(false);
        assert_eq!(p.a_json()["deceasedBoolean"], false, "«vivo» SABIDO también se dice");

        p.fallecido = None;
        assert!(p.a_json().get("deceasedBoolean").is_none(), "sin declaración, nada");
    }

    /// El paciente y el profesional comparten la forma del `id`, así que la
    /// misma persona en los dos papeles produce dos recursos con el mismo `id`
    /// y una referencia que resuelve por orden.
    #[test]
    fn el_paciente_y_el_profesional_no_pueden_ser_la_misma_persona() {
        let mut pr = profesional();
        pr.numero_doc = "000000000"; // el mismo del paciente, y mismo tipo
        let f = ensamblar(&paciente(), &institucion(), &pr).unwrap_err();
        assert!(f.0.iter().any(|x| x.recurso == "Composition" && x.campo == "author"), "{f}");

        // Pareja: con documentos distintos NO se queja.
        assert!(ensamblar(&paciente(), &institucion(), &profesional()).is_ok());
    }

    /// El tipo `id` de FHIR es `[A-Za-z0-9\-\.]{1,64}`, y nadie lo comprueba
    /// aguas arriba: la columna es `TEXT NOT NULL` sin `CHECK`.
    #[test]
    fn un_documento_que_no_cabe_en_un_id_de_fhir_no_pasa() {
        for malo in ["1234 5678", "12/34", "12#34", "A".repeat(70).as_str()] {
            let mut p = paciente();
            p.numero_doc = malo;
            let f = ensamblar(&p, &institucion(), &profesional())
                .expect_err(&format!("«{malo}» tenía que ser rechazado"));
            assert!(f.0.iter().any(|x| x.campo == "id"), "«{malo}» debería fallar: {f}");

            // EL PROFESIONAL, que es la mitad que NADIE medía.
            //
            // La guarda del `Practitioner` es una copia de la del `Patient`, y
            // copiadas es como divergen: `sin-caso-rojo` la marcó SIN CASO ROJO
            // porque este bucle solo movía `p.numero_doc`. Revertir el bloque
            // del profesional dejaba las 55 pruebas verdes, y lo que se degrada
            // en silencio es `attester.party`, la referencia que nombra a quien
            // responde JURÍDICAMENTE por el documento.
            let mut pr = profesional();
            pr.numero_doc = malo;
            let f = ensamblar(&paciente(), &institucion(), &pr)
                .expect_err(&format!("profesional «{malo}» tenía que ser rechazado"));
            assert!(
                f.0.iter().any(|x| x.recurso == "Practitioner" && x.campo == "id"),
                "profesional «{malo}» debería fallar: {f}"
            );

            // LA INSTITUCIÓN: el tercer `id` del Bundle, que el primer arreglo
            // se dejó fuera y del que cuelgan `custodian` y `attester.party`
            // de los cuatro documentos.
            let mut i = institucion();
            i.codigo_habilitacion = malo;
            let f = ensamblar(&paciente(), &i, &profesional())
                .expect_err(&format!("institución «{malo}» tenía que ser rechazada"));
            assert!(
                f.0.iter().any(|x| x.recurso == "Organization" && x.campo == "id"),
                "institución «{malo}» debería fallar: {f}"
            );
        }
        // Pareja: los que SÍ caben pasan — puntos y guiones incluidos, que son
        // legales en una cédula escrita «1.234.567».
        for bueno in ["1234567", "1.234.567", "AB-123"] {
            let mut p = paciente();
            p.numero_doc = bueno;
            assert!(ensamblar(&p, &institucion(), &profesional()).is_ok(), "«{bueno}»");
        }
    }

    /// El espacio al final pasaba la puerta —`exigir` recortaba para DECIDIR—
    /// y viajaba sin recortar a tres sitios a la vez.
    #[test]
    fn lo_que_se_emite_va_recortado() {
        let mut p = paciente();
        p.numero_doc = " 1234567 ";
        let v = p.a_json();
        assert_eq!(v["id"], "CC-1234567");
        assert_eq!(v["identifier"][0]["value"], "1234567");
    }

    /// La segunda revisión encontró que el arreglo anterior tapó los TRES
    /// sitios del informe y dejó cuatro más con el mismo defecto. Esta prueba
    /// mide la REGLA —se normaliza en el borde— y no la lista, así que un
    /// campo nuevo que se emita crudo cae aquí sin tocar la prueba.
    #[test]
    fn nada_sale_crudo_del_ensamblador() {
        let mut p = paciente();
        p.tipo_doc = " CC ";
        p.numero_doc = " 1234567 ";
        p.tipo_doc_display = " Cédula ciudadanía ";
        p.genero_fhir = " male ";
        p.nombre.nombre2 = Some("  Segundonombre  ");
        p.municipio = Some(Codigo::nuevo(" 11001 ", " Bogotá D.C. "));
        let mut pr = profesional();
        pr.numero_doc = " 111111111 ";
        let mut i = institucion();
        i.codigo_habilitacion = " 0500100001 ";
        i.nit = " 900000000 ";

        let v = ensamblar(&p, &i, &pr).expect("todo esto es válido una vez recortado");
        let (pac, org, prof) = (&v[0], &v[1], &v[2]);

        // El `id` y las tres cosas que ya se medían.
        assert_eq!(pac["id"], "CC-1234567");
        assert_eq!(pac["identifier"][0]["value"], "1234567");
        // Lo que NO se medía: el `code` del coding nacional iba crudo en el
        // MISMO literal `json!` donde `value` ya se recortaba.
        assert_eq!(pac["identifier"][0]["type"]["coding"][1]["code"], "CC");
        assert_eq!(pac["identifier"][0]["type"]["coding"][1]["display"], "Cédula ciudadanía");
        assert_eq!(pac["gender"], "male");
        assert_eq!(pac["name"][0]["given"][1], "Segundonombre");
        assert_eq!(prof["id"], "CC-111111111");
        // El tercer `id` del Bundle y su referencia.
        assert_eq!(org["id"], "0500100001");
        assert_eq!(org["identifier"][0]["value"], "900000000");

        // Y el positivo de control de que esto NO es un `assert` que dice que
        // sí a todo: sin recortar, ninguna de estas igualdades se cumpliría.
        assert_ne!(pac["identifier"][0]["type"]["coding"][1]["code"], " CC ");
    }

    /// Pareja del anterior: un espacio INTERNO no es blanco de borde y tiene
    /// que seguir siendo rechazado. Si `recortado()` se convirtiera algún día
    /// en «quitar todos los espacios», esto se pone rojo.
    #[test]
    fn recortar_no_es_borrar_espacios() {
        let mut p = paciente();
        p.numero_doc = " 123 4567 ";
        let f = ensamblar(&p, &institucion(), &profesional())
            .expect_err("el espacio de dentro no lo arregla el recorte");
        assert!(f.0.iter().any(|x| x.campo == "id"), "{f}");
    }

    /// `Patient.gender` es el único vocabulario CERRADO que este módulo conoce
    /// estáticamente. El sexo del diccionario propio se traduce antes.
    #[test]
    fn el_genero_fuera_del_value_set_de_fhir_no_pasa() {
        let mut p = paciente();
        p.genero_fhir = "M";
        let f = ensamblar(&p, &institucion(), &profesional()).unwrap_err();
        assert!(f.0.iter().any(|x| x.campo == "gender"), "{f}");

        for bueno in ["male", "female", "other", "unknown"] {
            let mut p = paciente();
            p.genero_fhir = bueno;
            assert!(ensamblar(&p, &institucion(), &profesional()).is_ok(), "«{bueno}»");
        }
    }

    /// **Los valores de vocabulario CERRADO, contra el ejemplo.**
    ///
    /// `vocabulario()` solo mira `system` y `url`, así que estos cinco —donde
    /// una errata tampoco da error y el servidor guarda otra cosa— quedaban
    /// fuera del arnés. Lo señaló la revisión del 2026-08-15.
    #[test]
    fn los_valores_de_vocabulario_cerrado_son_los_del_ejemplo() {
        let nuestro = paciente_completo().a_json();
        let suyo = oficial("Patient");
        for campo in ["gender", "active"] {
            assert_eq!(nuestro[campo], suyo[campo], "Patient.{campo}");
        }
        for campo in ["use", "type"] {
            assert_eq!(nuestro["address"][0][campo], suyo["address"][0][campo], "address.{campo}");
        }
        assert_eq!(nuestro["identifier"][0]["use"], suyo["identifier"][0]["use"]);
        assert_eq!(nuestro["name"][0]["use"], suyo["name"][0]["use"]);
    }

    #[test]
    fn los_tres_actores_salen_en_el_orden_del_ejemplo() {
        let v = ensamblar(&paciente(), &institucion(), &profesional()).unwrap();
        let tipos: Vec<&str> = v.iter().map(|r| r["resourceType"].as_str().unwrap()).collect();
        // El ejemplo los pone así: Patient, Organization, Practitioner.
        assert_eq!(tipos, ["Patient", "Organization", "Practitioner"]);
    }

    #[test]
    fn los_id_son_los_que_las_referencias_de_la_composition_esperan() {
        let v = ensamblar(&paciente(), &institucion(), &profesional()).unwrap();
        assert_eq!(v[0]["id"], "CC-000000000");
        assert_eq!(v[1]["id"], "0500100001");
        assert_eq!(v[2]["id"], "CC-1111111111");
    }

    #[test]
    fn el_perfil_de_cada_actor_es_el_del_ejemplo() {
        for (generado, tipo) in [
            (paciente().a_json(), "Patient"),
            (institucion().a_json(), "Organization"),
            (profesional().a_json(), "Practitioner"),
        ] {
            assert_eq!(generado["meta"], oficial(tipo)["meta"], "perfil de {tipo}");
        }
    }

    #[test]
    fn el_apellido_va_compuesto_y_desglosado_a_la_vez() {
        let p = paciente().a_json();
        assert_eq!(p["name"][0]["family"], "Primerapellido Segundoapellido");
        let ext = p["name"][0]["_family"]["extension"].as_array().unwrap();
        assert_eq!(ext.len(), 2);
        assert_eq!(ext[0]["valueString"], "Primerapellido");
        assert_eq!(ext[1]["valueString"], "Segundoapellido");
    }

    /// Con un solo apellido NO se emite la extensión de la madre.
    ///
    /// Es la pareja de la de arriba: emitirla vacía diría «no tiene segundo
    /// apellido» donde el dato es «no lo sabemos», que no es lo mismo.
    #[test]
    fn con_un_solo_apellido_no_se_inventa_el_segundo() {
        let mut p = paciente();
        p.nombre.apellido2 = None;
        p.nombre.nombre2 = None;
        let v = p.a_json();
        assert_eq!(v["name"][0]["family"], "Primerapellido");
        assert_eq!(v["name"][0]["_family"]["extension"].as_array().unwrap().len(), 1);
        assert_eq!(v["name"][0]["given"].as_array().unwrap().len(), 1);
    }

    /// Las cuatro extensiones demográficas que medico NO captura se OMITEN.
    #[test]
    fn lo_que_medico_no_captura_se_omite_y_no_se_rellena() {
        let sin = paciente().a_json();
        assert!(sin.get("extension").is_none(), "no debe emitir `extension` vacía: {sin}");

        // Y la pareja: cuando SÍ hay dato, sale — o sea que la ausencia de
        // arriba es ausencia de dato, no un camino muerto.
        let mut con = paciente();
        con.etnia = Some(Codigo::nuevo("6", "Otras etnias"));
        let v = con.a_json();
        let ext = v["extension"].as_array().unwrap();
        assert_eq!(ext.len(), 1);
        assert!(ext[0]["url"].as_str().unwrap().ends_with("ExtensionPatientEthnicity"));
    }

    #[test]
    fn el_sexo_viaja_en_los_dos_diccionarios_a_la_vez() {
        let p = paciente().a_json();
        assert_eq!(p["gender"], "male", "el de FHIR");
        let e = &p["_gender"]["extension"][0];
        assert!(e["url"].as_str().unwrap().ends_with("ExtensionBiologicalGender"));
        assert_eq!(e["valueCoding"]["code"], "01", "el colombiano");
    }

    #[test]
    fn no_se_afirma_que_el_paciente_este_vivo() {
        // El ejemplo trae `deceasedBoolean: false`; con `fallecido: None` no se
        // emite, porque `None` significa «sin declaración» y no «vivo».
        //
        // EL COMENTARIO QUE HABÍA AQUÍ ERA FALSO y sobrevivió a su corrección:
        // decía «medico no registra defunción». Sí la registra —`0017` admite
        // DEFUNCION y `0016` admite FALLECIDO—, y esa premisa falsa es la que
        // dejó el campo sin emitir en el RDA de egreso. Se corrigió en el tipo
        // dos commits antes y este comentario se quedó contradiciendo al
        // módulo a tres archivos de distancia. Ver `fallecido_de_egreso`.
        assert!(paciente().a_json().get("deceasedBoolean").is_none());
    }

    #[test]
    fn la_direccion_se_omite_entera_cuando_no_hay_municipio() {
        let mut p = paciente();
        p.municipio = None;
        assert!(p.a_json().get("address").is_none());
        // Pareja: con municipio, sale con su DIVIPOLA.
        let v = paciente().a_json();
        assert_eq!(v["address"][0]["city"], "Bogotá D.C.");
        assert_eq!(v["address"][0]["_city"]["extension"][0]["valueCoding"]["code"], "11001");
    }

    #[test]
    fn el_practitioner_no_lleva_system_en_su_identificador() {
        // El ejemplo se lo pone al paciente (RNEC) y NO al profesional. Se
        // sigue el ejemplo: es lo único medido.
        assert!(profesional().a_json()["identifier"][0].get("system").is_none());
        assert!(paciente().a_json()["identifier"][0]["system"]
            .as_str().unwrap().ends_with("/RNEC"));
    }

    #[test]
    fn la_organizacion_lleva_los_dos_identificadores_y_el_reps_como_id() {
        let o = institucion().a_json();
        let ids = o["identifier"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0]["value"], "900123456", "el NIT");
        assert_eq!(ids[1]["value"], "0500100001", "el código de habilitación");
        assert_eq!(o["id"], "0500100001", "el `id` es el REPS, no el NIT");
    }

    #[test]
    fn el_nit_no_se_rellena_con_desconocido() {
        // El ejemplo OFICIAL trae "Desconocido". Aquí eso es una falta.
        let mut i = institucion();
        i.nit = "   ";
        let f = ensamblar(&paciente(), &i, &profesional()).unwrap_err();
        assert!(f.0.iter().any(|x| x.campo == "identifier.TaxIdentifier"), "{f}");
    }

    #[test]
    fn las_faltas_de_los_tres_actores_salen_todas_juntas() {
        let mut p = paciente();
        p.nacimiento = "";
        p.nombre.apellido1 = "";
        let mut pr = profesional();
        pr.numero_doc = "";
        let mut i = institucion();
        i.nit = "";
        let f = ensamblar(&p, &i, &pr).unwrap_err();
        assert_eq!(f.0.len(), 4, "una corrección, no cuatro intentos: {f}");
    }

    /// El mapeo, con sus DOS listas: la que TIENE que ver la muerte y la que
    /// TIENE que no verla. Con solo la primera, un mapeo cableado a
    /// `Some(true)` pasaría igual.
    #[test]
    fn el_desenlace_sale_del_vocabulario_y_no_se_inventa() {
        assert_eq!(fallecido_de_egreso("DEFUNCION"), Some(true));
        assert_eq!(fallecido_de_urgencias("FALLECIDO"), Some(true));

        for v in ["ALTA_MEDICA", "ALTA_VOLUNTARIA", "REMISION"] {
            assert_eq!(fallecido_de_egreso(v), Some(false), "{v}");
        }
        for v in ["DOMICILIO", "HOSPITALIZACION", "REMISION"] {
            assert_eq!(fallecido_de_urgencias(v), Some(false), "{v}");
        }

        // LA TERCERA LISTA, que es la que se equivocó: los que NO declaran
        // nada. No es lo mismo que declarar «vivo», y confundirlos deja salir
        // el RDA de un episodio que no ha terminado.
        assert_eq!(fallecido_de_egreso("ACTIVA"), None, "sigue hospitalizado");
        assert_eq!(fallecido_de_urgencias("OBSERVACION"), None, "continúa en el servicio");
        assert_eq!(fallecido_de_urgencias("FUGA"), None, "nadie le vio salir");
        assert_eq!(fallecido_de_egreso("LO_QUE_SEA"), None);
        assert_eq!(fallecido_de_urgencias(""), None);
    }

    /// EL DESENLACE NO PUEDE CONTRADECIR AL CATÁLOGO DE RIPS sobre el mismo
    /// dato. Es la prueba que habría impedido los dos errores: `muv.rs` ya
    /// decide qué significa cada valor, y el RDA se ancló a los TOKENS de la
    /// migración en vez de al catálogo que les da significado.
    #[test]
    fn el_desenlace_no_contradice_al_catalogo_de_rips() {
        use crate::muv::condicion_destino_de_urgencias as rips;
        for (valor, nuestro) in URGENCIAS {
            match (rips(Some(valor)), nuestro) {
                // RIPS dice «no puedo traducirlo» -> nosotros tampoco
                // afirmamos nada. Es el caso de FUGA.
                (None, n) => assert_eq!(*n, None,
                    "«{valor}»: RIPS rehúsa traducirlo y el RDA afirma {n:?}"),
                // RIPS dice PACIENTE MUERTO -> nosotros, muerto.
                (Some("02"), n) => assert_eq!(*n, Some(true), "«{valor}»"),
                // RIPS dice que CONTINÚA EN EL SERVICIO -> no hay desenlace.
                (Some("08"), n) => assert_eq!(*n, None,
                    "«{valor}»: continúa en el servicio y el RDA declara {n:?}"),
                // El resto son salidas de verdad con el paciente vivo.
                (Some(_), n) => assert_eq!(*n, Some(false), "«{valor}»"),
            }
        }
    }

    /// EL VOCABULARIO ES EL DE ESA COLUMNA, en las DOS direcciones.
    ///
    /// La versión anterior hacía `archivo.contains("DEFUNCION")` y no vigilaba
    /// ninguna de las dos: un estado NUEVO no rompe un `contains`, y uno
    /// RETIRADO tampoco, porque `REMISION` también está en el `CHECK` de
    /// `via_ingreso` —otra columna— y `contains` mira el archivo entero.
    #[test]
    fn el_vocabulario_del_desenlace_es_el_de_la_columna() {
        /// Saca el conjunto del `CHECK (<columna> IN ('A','B'…))` de UNA TABLA.
        ///
        /// La tabla hace falta y no sobra: la primera versión buscaba solo por
        /// nombre de columna y encontró el `CHECK (estado …)` de las CAMAS
        /// —DISPONIBLE, OCUPADA, EN_ASEO…— que vive en el mismo archivo. Es
        /// literalmente el defecto que esta prueba vino a cerrar, cometido por
        /// el instrumento que lo cierra. Se descubrió porque salió ROJA a la
        /// primera, que es para lo que sirve escribirla así.
        fn columna(sql: &str, tabla: &str, col: &str) -> std::collections::BTreeSet<String> {
            let t = sql
                .find(&format!("CREATE TABLE IF NOT EXISTS {tabla}"))
                .or_else(|| sql.find(&format!("CREATE TABLE {tabla}")))
                .unwrap_or_else(|| panic!("no está la tabla `{tabla}`"));
            let sql = &sql[t..];
            let aguja = format!("CHECK ({col} IN (");
            let i = sql.find(&aguja).unwrap_or_else(|| panic!("no está `{aguja}` en `{tabla}`"));
            let resto = &sql[i + aguja.len()..];
            let fin = resto.find(')').expect("el CHECK no cierra");
            resto[..fin]
                .split(',')
                .map(|t| t.trim().trim_matches('\'').to_string())
                .collect()
        }
        for (sql, tabla, col, nuestro) in [
            (include_str!("../../../medico-datos/migrations/0017_hospitalizacion.sql"),
             "hospitalizacion", "estado", EGRESO),
            (include_str!("../../../medico-datos/migrations/0016_urgencias.sql"),
             "atencion_urgencias", "destino_salida", URGENCIAS),
        ] {
            let de_la_columna = columna(sql, tabla, col);
            let nuestras: std::collections::BTreeSet<String> =
                nuestro.iter().map(|(k, _)| k.to_string()).collect();
            assert_eq!(
                de_la_columna, nuestras,
                "«{col}»: la columna y el mapeo dejaron de decir lo mismo.\n  \
                 solo en la columna: {:?}\n  solo en el mapeo: {:?}",
                de_la_columna.difference(&nuestras).collect::<Vec<_>>(),
                nuestras.difference(&de_la_columna).collect::<Vec<_>>()
            );
        }
    }
}
