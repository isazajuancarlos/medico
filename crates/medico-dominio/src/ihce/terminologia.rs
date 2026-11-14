// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza

//! **Los `display` del RDA salen del CodeSystem, no de la memoria de nadie.**
//!
//! Un `Coding` de FHIR lleva `system`, `code` y `display`. El servidor nacional
//! valida el par `system`+`code`; el `display` lo acepta **como venga** — y por
//! eso es exactamente el sitio donde un dato inventado pasa sin que nadie se
//! entere, igual que el `tableta = 1` de las presentaciones del CUM, que el MUV
//! aceptó porque validaba ese campo por rango.
//!
//! Aquí el `display` se **deriva** del CodeSystem vendorizado
//! (`datos/ihce/ig/CodeSystem-*.json`, bajado del sitio del Ministerio con su
//! `sha256` en `MANIFIESTO.txt`), y un código que no esté en el catálogo **no se
//! emite**: se anota como falta. Es la misma disciplina que
//! `unidad_dispensacion` y por la misma lección.
//!
//! Van EMPOTRADOS y no leídos de disco: el producto es un binario que corre en
//! el consultorio, sin la carpeta de datos al lado. Son 4 KB cada uno.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// `ColombianTechModality`: la modalidad de la atención (`01` intramural, …).
pub const SISTEMA_MODALIDAD: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianTechModality";

/// `GrupoServicios`: el grupo de servicios (`01` consulta externa, …).
pub const SISTEMA_GRUPO_SERVICIOS: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/GrupoServicios";

/// `ColombianEthnicGroup`: la pertenencia étnica (`1` indígena, …, `99` ninguna).
pub const SISTEMA_ETNIA: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianEthnicGroup";

/// `ColombianDisabilityClassification`: la discapacidad (`01` física, …, `08` sin).
pub const SISTEMA_DISCAPACIDAD: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianDisabilityClassification";

/// `ColombianGenderIdentity`: la identidad de género (`01` masculino, …).
pub const SISTEMA_IDENTIDAD_GENERO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianGenderIdentity";

// LOS SEIS DEL ENCUENTRO CLÍNICO (#603 fase 4). El `Encounter`, el `Condition` y
// el pagador nombran códigos de estos catálogos, y su `display` sale del artefacto
// por la misma razón que los de arriba: el servidor valida `system`+`code` y se
// traga el `display` que le manden.

/// `EntornoAtencion`: el entorno de la atención (`01` hogar, …, `05` institucional).
pub const SISTEMA_ENTORNO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/EntornoAtencion";

/// `RIPSCausaExternaVersion2`: la causa externa del encuentro (`Encounter.reasonCode`).
pub const SISTEMA_CAUSA_EXTERNA: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/RIPSCausaExternaVersion2";

/// `ViaIngreso`: la vía de ingreso (`Encounter.hospitalization.admitSource`).
pub const SISTEMA_VIA_INGRESO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ViaIngreso";

/// `CondicionyDestinoUsuarioEgreso`: condición y destino al egreso
/// (`Encounter.hospitalization.dischargeDisposition`).
pub const SISTEMA_DESTINO_EGRESO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/CondicionyDestinoUsuarioEgreso";

/// `RIPSTipoDiagnosticoPrincipalVersion2`: el tipo de diagnóstico (`01` impresión,
/// `02` confirmado nuevo, `03` confirmado repetido), en `ExtensionDiagnosisType`.
pub const SISTEMA_TIPO_DIAGNOSTICO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/RIPSTipoDiagnosticoPrincipalVersion2";

// LOS DOS DEL PRODUCTOR DE PRODUCCIÓN (#603 fase 4). El `display` del tipo de
// documento y del sexo salen del catálogo, no de una lista escrita a mano: es la
// misma lección del `tableta = 1`.

/// `ColombianPersonIdentifier`: el tipo de documento de una persona (`CC` cédula,
/// `TI` tarjeta de identidad, …). Es el `system` que `actores` emite en el
/// `type.coding` nacional del identificador; su `display` sale de aquí.
///
/// **El catálogo es JERÁRQUICO**: los tipos cuelgan de la autoridad que los emite
/// (`RNEC → CC/TI/RC…`, `CANCILLERIA → PA/CE…`), así que se cargan las HOJAS, no
/// el primer nivel —`display("CC")` tiene que dar «Cédula ciudadanía», no vacío—.
pub const SISTEMA_TIPO_DOCUMENTO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianPersonIdentifier";

/// `ColombianGenderGroup`: el sexo biológico (`01` Hombre, `02` Mujer, `03`
/// Indeterminado o Intersexual). Viaja en `_gender` del `Patient`; su `display`
/// sale de aquí y el código lo traduce quien construye el insumo desde el `M`/`F`
/// del RIPS (ver [`super::sexo_rda`]).
pub const SISTEMA_GENERO_GRUPO: &str =
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianGenderGroup";

const MODALIDAD_JSON: &str = include_str!("../../datos/ihce/ig/CodeSystem-ColombianTechModality.json");
const GRUPO_JSON: &str = include_str!("../../datos/ihce/ig/CodeSystem-GrupoServicios.json");
const ETNIA_JSON: &str = include_str!("../../datos/ihce/ig/CodeSystem-ColombianEthnicGroup.json");
const DISCAPACIDAD_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-ColombianDisabilityClassification.json");
const IDENTIDAD_GENERO_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-ColombianGenderIdentity.json");
const ENTORNO_JSON: &str = include_str!("../../datos/ihce/ig/CodeSystem-EntornoAtencion.json");
const CAUSA_EXTERNA_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-RIPSCausaExternaVersion2.json");
const VIA_INGRESO_JSON: &str = include_str!("../../datos/ihce/ig/CodeSystem-ViaIngreso.json");
const DESTINO_EGRESO_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-CondicionyDestinoUsuarioEgreso.json");
const TIPO_DIAGNOSTICO_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-RIPSTipoDiagnosticoPrincipalVersion2.json");
const TIPO_DOCUMENTO_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-ColombianPersonIdentifier.json");
const GENERO_GRUPO_JSON: &str =
    include_str!("../../datos/ihce/ig/CodeSystem-ColombianGenderGroup.json");

/// `código → display`, tal como lo publica el CodeSystem (primer nivel).
fn catalogo(json: &'static str, cache: &'static OnceLock<BTreeMap<String, String>>) -> &'static BTreeMap<String, String> {
    cache.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(json).expect("el CodeSystem vendorizado es JSON válido");
        v["concept"]
            .as_array()
            .map(|cs| {
                cs.iter()
                    .filter_map(|c| {
                        Some((c["code"].as_str()?.to_string(), c["display"].as_str()?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// `código → display` de las HOJAS de un CodeSystem JERÁRQUICO (`concept[].concept[]`).
///
/// Un nodo con hijos es una AUTORIDAD (`RNEC`, `CANCILLERIA`), no un tipo de
/// documento; el tipo real es la hoja (`CC`, `TI`). Se recogen solo las hojas —un
/// selector de tipos de documento con «Registraduría Nacional» dentro sería
/// inventar una opción que no es un tipo—, y por eso `catalogo` (primer nivel) no
/// sirve aquí: dejaría fuera todos los códigos que de verdad se usan.
fn catalogo_hojas(json: &'static str, cache: &'static OnceLock<BTreeMap<String, String>>) -> &'static BTreeMap<String, String> {
    cache.get_or_init(|| {
        fn hojas(concepts: &[serde_json::Value], acc: &mut BTreeMap<String, String>) {
            for c in concepts {
                match c["concept"].as_array() {
                    Some(hijos) if !hijos.is_empty() => hojas(hijos, acc),
                    _ => {
                        if let (Some(code), Some(display)) =
                            (c["code"].as_str(), c["display"].as_str())
                        {
                            acc.insert(code.to_string(), display.to_string());
                        }
                    }
                }
            }
        }
        let v: serde_json::Value =
            serde_json::from_str(json).expect("el CodeSystem vendorizado es JSON válido");
        let mut m = BTreeMap::new();
        if let Some(cs) = v["concept"].as_array() {
            hojas(cs, &mut m);
        }
        m
    })
}

fn modalidades() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(MODALIDAD_JSON, &C)
}

fn grupos() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(GRUPO_JSON, &C)
}

fn etnias() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(ETNIA_JSON, &C)
}

fn discapacidades() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(DISCAPACIDAD_JSON, &C)
}

fn identidades_genero() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(IDENTIDAD_GENERO_JSON, &C)
}

fn entornos() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(ENTORNO_JSON, &C)
}

fn causas_externas() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(CAUSA_EXTERNA_JSON, &C)
}

fn vias_ingreso() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(VIA_INGRESO_JSON, &C)
}

fn destinos_egreso() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(DESTINO_EGRESO_JSON, &C)
}

fn tipos_diagnostico() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(TIPO_DIAGNOSTICO_JSON, &C)
}

fn tipos_documento() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo_hojas(TIPO_DOCUMENTO_JSON, &C)
}

fn generos_grupo() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    catalogo(GENERO_GRUPO_JSON, &C)
}

/// Las opciones `(código, display)` de un catálogo, en orden de código, para
/// pintar un `<select>` de captura.
///
/// **Salen del CodeSystem, no de una lista escrita a mano** (misma disciplina que
/// [`display`]): un selector que enumera lo que el servidor nacional NO reconoce
/// dejaría teclear un código que después se rechaza, y el `01` de un catálogo no
/// significa lo del otro. Un `sistema` desconocido devuelve **vacío**: un selector
/// vacío es honesto y no inventa opciones (directiva 20).
pub fn opciones(sistema: &str) -> Vec<(&'static str, &'static str)> {
    let c = match sistema {
        SISTEMA_MODALIDAD => modalidades(),
        SISTEMA_GRUPO_SERVICIOS => grupos(),
        SISTEMA_ETNIA => etnias(),
        SISTEMA_DISCAPACIDAD => discapacidades(),
        SISTEMA_IDENTIDAD_GENERO => identidades_genero(),
        SISTEMA_ENTORNO => entornos(),
        SISTEMA_CAUSA_EXTERNA => causas_externas(),
        SISTEMA_VIA_INGRESO => vias_ingreso(),
        SISTEMA_DESTINO_EGRESO => destinos_egreso(),
        SISTEMA_TIPO_DIAGNOSTICO => tipos_diagnostico(),
        SISTEMA_TIPO_DOCUMENTO => tipos_documento(),
        SISTEMA_GENERO_GRUPO => generos_grupo(),
        _ => return Vec::new(),
    };
    c.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

/// El `display` oficial de un código, o `None` si el catálogo no lo tiene.
///
/// `None` **no se rellena**: quien llama anota la falta y el campo no sale
/// (directiva 20). Emitirlo con el código y un `display` vacío sería afirmar que
/// el catálogo lo contiene.
pub fn display(sistema: &str, codigo: &str) -> Option<&'static str> {
    let c = match sistema {
        SISTEMA_MODALIDAD => modalidades(),
        SISTEMA_GRUPO_SERVICIOS => grupos(),
        SISTEMA_ETNIA => etnias(),
        SISTEMA_DISCAPACIDAD => discapacidades(),
        SISTEMA_IDENTIDAD_GENERO => identidades_genero(),
        SISTEMA_ENTORNO => entornos(),
        SISTEMA_CAUSA_EXTERNA => causas_externas(),
        SISTEMA_VIA_INGRESO => vias_ingreso(),
        SISTEMA_DESTINO_EGRESO => destinos_egreso(),
        SISTEMA_TIPO_DIAGNOSTICO => tipos_diagnostico(),
        SISTEMA_TIPO_DOCUMENTO => tipos_documento(),
        SISTEMA_GENERO_GRUPO => generos_grupo(),
        _ => return None,
    };
    c.get(codigo.trim()).map(String::as_str)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **El catálogo se lee del artefacto**, y su positivo de control: si el
    /// `include_str!` apuntara a otro archivo o el JSON cambiara de forma, el
    /// mapa saldría vacío y todo lo de abajo daría `None` — que se ve igual que
    /// «ese código no existe».
    #[test]
    fn los_catalogos_se_cargan_y_no_estan_vacios() {
        assert_eq!(modalidades().len(), 9, "ColombianTechModality trae nueve modalidades");
        assert!(!grupos().is_empty(), "GrupoServicios salió vacío: el include no cargó");
        // Los tres demográficos del paciente (#84): tamaños exactos del artefacto,
        // que es el positivo de control de que el `include_str!` cargó el bueno.
        assert_eq!(etnias().len(), 7, "ColombianEthnicGroup: 1-6 + 99");
        assert_eq!(discapacidades().len(), 8, "ColombianDisabilityClassification: 01-08");
        assert_eq!(identidades_genero().len(), 5, "ColombianGenderIdentity: 01-05");
        // Los cinco del encuentro clínico (#603 fase 4): tamaños exactos del
        // artefacto, positivo de control de que cada `include_str!` cargó el bueno.
        assert_eq!(entornos().len(), 5, "EntornoAtencion: 01-05");
        assert_eq!(causas_externas().len(), 29, "RIPSCausaExternaVersion2");
        assert_eq!(vias_ingreso().len(), 14, "ViaIngreso");
        assert_eq!(destinos_egreso().len(), 8, "CondicionyDestinoUsuarioEgreso");
        assert_eq!(tipos_diagnostico().len(), 3, "RIPSTipoDiagnosticoPrincipalVersion2: 01-03");
        // Los dos del productor (#603 fase 4). El de tipo de documento es
        // JERÁRQUICO: 16 HOJAS (4 RNEC + 8 Cancillería + 1 DIAN + 3 OTROS), no las
        // 4 autoridades del primer nivel — ese es el positivo de control de que
        // `catalogo_hojas` recorre y no se queda arriba.
        assert_eq!(tipos_documento().len(), 16, "ColombianPersonIdentifier: 16 tipos hoja");
        assert_eq!(generos_grupo().len(), 3, "ColombianGenderGroup: 01-03");
    }

    /// **El tipo de documento sale de las HOJAS del catálogo jerárquico, y las
    /// autoridades NO son tipos.** Sin la pareja (la autoridad que NO está),
    /// `catalogo_hojas` podría estar recogiendo el primer nivel y nadie lo vería.
    #[test]
    fn el_tipo_de_documento_son_las_hojas_no_las_autoridades() {
        assert_eq!(display(SISTEMA_TIPO_DOCUMENTO, "CC"), Some("Cédula ciudadanía"));
        assert_eq!(display(SISTEMA_TIPO_DOCUMENTO, "TI"), Some("Tarjeta de Identidad"));
        assert_eq!(display(SISTEMA_TIPO_DOCUMENTO, "CE"), Some("Cédula de extranjería"));
        // La autoridad emisora es un nodo padre, no un tipo: no se emite como si
        // lo fuera (sería teclear «Registraduría» donde va «Cédula»).
        assert_eq!(display(SISTEMA_TIPO_DOCUMENTO, "RNEC"), None, "la autoridad no es un tipo");
        assert_eq!(display(SISTEMA_TIPO_DOCUMENTO, "CANCILLERIA"), None);
        // Y el género, plano.
        assert_eq!(display(SISTEMA_GENERO_GRUPO, "01"), Some("Hombre"));
        assert_eq!(display(SISTEMA_GENERO_GRUPO, "02"), Some("Mujer"));
        assert_eq!(display(SISTEMA_GENERO_GRUPO, "03"), Some("Indeterminado o Intersexual"));
        assert_eq!(display(SISTEMA_GENERO_GRUPO, "99"), None);
    }

    /// **EL `display` SALE DEL CATÁLOGO, Y EL QUE NO ESTÁ NO SE INVENTA.**
    ///
    /// Es la lección del `tableta = 1`: el servidor valida `system`+`code` y se
    /// traga el `display` que le manden, así que ese campo es donde un dato
    /// falso sobrevive.
    #[test]
    fn el_display_se_deriva_y_lo_desconocido_dice_que_no() {
        assert_eq!(display(SISTEMA_MODALIDAD, "01"), Some("Intramural"));
        assert_eq!(display(SISTEMA_MODALIDAD, " 01 "), Some("Intramural"), "se recorta");
        assert_eq!(display(SISTEMA_GRUPO_SERVICIOS, "01"), Some("Consulta externa"));
        // Los tres demográficos del paciente (#84), display derivado del artefacto.
        assert_eq!(display(SISTEMA_ETNIA, "1"), Some("Indigena"));
        assert_eq!(display(SISTEMA_ETNIA, "99"), Some("Ninguna de las anteriores"));
        assert_eq!(display(SISTEMA_DISCAPACIDAD, "02"), Some("Discapacidad visual"));
        assert_eq!(display(SISTEMA_IDENTIDAD_GENERO, "03"), Some("Transgénero"));

        // Los cinco del encuentro clínico (#603 fase 4), display del artefacto.
        assert_eq!(display(SISTEMA_ENTORNO, "05"), Some("Institucional"));
        assert_eq!(display(SISTEMA_CAUSA_EXTERNA, "22"), Some("ACCIDENTE EN EL HOGAR"));
        assert_eq!(display(SISTEMA_VIA_INGRESO, "01"), Some("DEMANDA ESPONTANEA"));
        assert_eq!(display(SISTEMA_DESTINO_EGRESO, "01"), Some("PACIENTE CON DESTINO A SU DOMICILIO"));
        assert_eq!(display(SISTEMA_TIPO_DIAGNOSTICO, "02"), Some("Confirmado Nuevo"));

        // La pareja, que es la que impide un instrumento que diga que sí a todo.
        assert_eq!(display(SISTEMA_MODALIDAD, "99"), None, "un código que no existe no se rellena");
        assert_eq!(display(SISTEMA_CAUSA_EXTERNA, "00"), None, "una causa externa que no existe no se rellena");
        assert_eq!(display(SISTEMA_TIPO_DIAGNOSTICO, "04"), None);
        assert_eq!(display(SISTEMA_ETNIA, "7"), None, "un código de etnia que no existe no se rellena");
        assert_eq!(display(SISTEMA_DISCAPACIDAD, "09"), None);
        assert_eq!(display(SISTEMA_MODALIDAD, ""), None);
        assert_eq!(
            display("https://ejemplo.invalido/CodeSystem/Otro", "01"),
            None,
            "un sistema que no es de los dos tampoco"
        );
        // Y el cruce: el `01` existe en LOS DOS y significa cosas distintas.
        assert_ne!(
            display(SISTEMA_MODALIDAD, "01"),
            display(SISTEMA_GRUPO_SERVICIOS, "01"),
            "el sistema decide qué significa el código: confundirlos emite el display del otro"
        );
    }

    /// **Las opciones del selector salen del catálogo, y su pareja: un sistema
    /// desconocido da vacío, no una lista inventada.** El caso rojo (el sistema
    /// inválido) es lo que impide un accesor que devuelva algo pase lo que pase.
    #[test]
    fn las_opciones_salen_del_catalogo_y_lo_desconocido_da_vacio() {
        let etnias = opciones(SISTEMA_ETNIA);
        assert_eq!(etnias.len(), 7, "las siete etnias del artefacto");
        // El par (código, display) es el que el <select> necesita, derivado del
        // CodeSystem y no escrito a mano: el 99 tiene que traer su display real.
        assert!(
            etnias.iter().any(|(c, d)| *c == "99" && *d == "Ninguna de las anteriores"),
            "el par (código, display) sale del catálogo"
        );
        assert_eq!(opciones(SISTEMA_DISCAPACIDAD).len(), 8);
        assert_eq!(opciones(SISTEMA_IDENTIDAD_GENERO).len(), 5);
        // Los cinco del encuentro clínico (#603 fase 4): `opciones` desde el
        // catálogo, para los selectores de captura del día que se cablee. Sin esto
        // sus cinco brazos del `match` no los mide nadie (sin-caso-rojo, 2026-08-29).
        assert_eq!(opciones(SISTEMA_ENTORNO).len(), 5);
        assert_eq!(opciones(SISTEMA_CAUSA_EXTERNA).len(), 29);
        assert_eq!(opciones(SISTEMA_VIA_INGRESO).len(), 14);
        assert_eq!(opciones(SISTEMA_DESTINO_EGRESO).len(), 8);
        assert_eq!(opciones(SISTEMA_TIPO_DIAGNOSTICO).len(), 3);
        // Los dos del productor (#603 fase 4). Sin estas aserciones, los brazos de
        // `opciones()` para estos catálogos no los medía nadie —solo se probaba
        // `display()`—, y se podían revertir sin que el banco se enterara (lo marcó
        // `sin-caso-rojo` en el concilio del 2026-09-03).
        let docs = opciones(SISTEMA_TIPO_DOCUMENTO);
        assert_eq!(docs.len(), 16, "las 16 hojas del catálogo de tipo de documento");
        assert!(
            docs.iter().any(|(c, d)| *c == "CC" && *d == "Cédula ciudadanía"),
            "el par (código, display) del tipo de documento sale del catálogo"
        );
        let generos = opciones(SISTEMA_GENERO_GRUPO);
        assert_eq!(generos.len(), 3, "ColombianGenderGroup: 01-03");
        assert!(generos.iter().any(|(c, d)| *c == "01" && *d == "Hombre"));
        // Ordenadas por código (BTreeMap): el selector no baraja las opciones
        // entre recargas.
        let disc = opciones(SISTEMA_DISCAPACIDAD);
        let mut ordenado = disc.clone();
        ordenado.sort_by_key(|(c, _)| *c);
        assert_eq!(disc, ordenado, "las opciones vienen en orden de código");
        // LA PAREJA: un sistema que no es de los conocidos NO devuelve opciones.
        assert!(
            opciones("https://ejemplo.invalido/CodeSystem/Otro").is_empty(),
            "un sistema desconocido da un selector vacío, no una lista inventada"
        );
    }
}
