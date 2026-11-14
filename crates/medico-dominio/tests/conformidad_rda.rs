// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza

//! **El RDA que medico ensambla, contra lo que el PERFIL exige** (#603).
//!
//! Es la mitad que faltaba de la ficha. Hasta el 2026-08-16 las pruebas del
//! ensamblador comparaban contra el **ejemplo oficial** —un documento concreto
//! que MinSalud publica—, y eso deja fuera todo lo que el ejemplo no ejercita:
//! un campo obligatorio que el ejemplo trae por casualidad se ve igual que uno
//! que el perfil exige. El contrato no es el ejemplo, es el
//! `StructureDefinition`.
//!
//! # De dónde sale la aguja
//!
//! De `datos/ihce/ig/`, los 207 artefactos de la Guía bajados VERBATIM del sitio
//! del Ministerio, con su `sha256` en `MANIFIESTO.txt`. **No se escribe aquí ni
//! una regla a mano**: se leen del perfil, que es la misma disciplina que
//! `campos_de_la_948.rs` y la misma razón —una lista copiada envejece en
//! silencio—.
//!
//! # Lo que este banco SÍ mide y lo que NO, dicho antes de los números
//!
//! FHIR permite «rebanar» un elemento (`Composition.section:sectionMedications`),
//! y evaluar una rebanada exige resolver su discriminador. **Desde el 2026-08-22
//! este banco SÍ lo resuelve** (`evaluar_rebanada`): lee el discriminador del
//! perfil (`slicing.discriminator`: `value` o `pattern`), casa la instancia de la
//! rebanada en el recurso y evalúa la exigencia dentro de ella. Fue lo que cazó
//! que las cuatro secciones vacías del RDA de paciente se emitían sin su
//! `emptyReason` obligatorio (#603). Por eso este banco:
//!
//!   · evalúa los elementos sin rebanada, por ruta directa;
//!   · evalúa las REBANADAS resolviendo su discriminador;
//!   · lo único que sigue en «sin evaluar» es una rebanada cuyo discriminador el
//!     perfil no deja resolver (varios discriminadores, o una rebanada que no fija
//!     el suyo) — y ahí se NOMBRA el motivo, en vez de callarlo.
//!
//! Un banco que midiera 40 reglas de 300 y dijera «conforme» sería peor que no
//! tenerlo (directiva 30): «no se pudo medir» no es «cumple». Por eso la prueba
//! que resume imprime siempre las dos cifras.

use std::collections::BTreeMap;
use std::path::PathBuf;

use medico_dominio::ihce::actores::{Codigo, Institucion, Nombre, Paciente, Profesional};
use medico_dominio::ihce::paciente::{ensamblar, Diagnostico, Eps, Insumo, Servicio};
use medico_dominio::ihce::{Aval, Confirmacion};

/// Una confirmación de sección para las fixtures: sección + un usuario y un sello
/// de prueba, como exige [`Aval`] (F4.5). Se construye aquí, código a código, y no
/// se importa una lista pública del módulo: no existe un asa de bypass de la puerta
/// del `nilknown` entre crates (revisión de seguridad #603). Un código mal escrito
/// no confirma (falla cerrado) y el ensamblado se pondría rojo.
const fn c(seccion: &str) -> Confirmacion<'_> {
    Confirmacion { seccion, usuario_id: 7, sello: "2026-08-14T18:42:00-05:00" }
}

/// Las 13 secciones que estas fixtures confirman vacías: las 4 del RDA de paciente
/// más las de los RDA clínicos que paciente no comparte (#603).
const SECCIONES_CONFIRMADAS: [Confirmacion<'static>; 13] = [
    // RDA de paciente (4).
    c("11450-4"), c("48765-2"), c("10160-0"), c("10157-6"),
    // Las de los RDA clínicos que paciente no comparte (#603).
    c("48768-6"), c("74208-0"), c("105583-9"), c("75492-9"),
    c("47519-4"), c("30954-2"), c("61146-1"), c("55107-7"), c("54094-8"),
];

/// La carpeta del IG vendorizado, anclada al crate y no al `cwd`.
fn carpeta_ig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("datos/ihce/ig")
}

fn perfil(nombre: &str) -> serde_json::Value {
    let ruta = carpeta_ig().join(format!("StructureDefinition-{nombre}.json"));
    let texto = std::fs::read_to_string(&ruta)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
    serde_json::from_str(&texto)
        .unwrap_or_else(|e| panic!("{} no es JSON: {e}", ruta.display()))
}

/// Una exigencia del perfil, ya extraída.
#[derive(Debug)]
struct Exigencia {
    /// El `id` del elemento, tal como lo escribe el perfil.
    id: String,
    /// Cardinalidad mínima declarada, si la declara.
    min: Option<u64>,
    /// Valor fijo, si lo hay: `("fixedCode", json)`.
    fijo: Option<(String, serde_json::Value)>,
}

impl Exigencia {
    /// La ruta que hay que recorrer en el recurso, sin el nombre del tipo.
    ///
    /// `Composition.type.coding.code` → `["type", "coding", "code"]`.
    fn ruta(&self) -> Vec<&str> {
        self.id.split('.').skip(1).collect()
    }

    /// ¿Lleva rebanada? `Patient.identifier:NationalPersonIdentifier.value`.
    fn rebanada(&self) -> bool {
        self.id.contains(':')
    }
}

/// Lee las exigencias del `differential` de un perfil.
///
/// El `differential` es lo que el perfil AÑADE sobre su base; el `snapshot` sería
/// todo, base incluida, y con eso el banco exigiría cosas que FHIR ya da por
/// buenas. Aquí interesa lo que la Guía impone de más.
fn exigencias(perfil: &serde_json::Value) -> Vec<Exigencia> {
    let mut v = Vec::new();
    let Some(els) = perfil["differential"]["element"].as_array() else {
        return v;
    };
    for e in els {
        let Some(id) = e["id"].as_str() else { continue };
        let min = e["min"].as_u64();
        let fijo = e
            .as_object()
            .and_then(|o| {
                o.iter()
                    .find(|(k, _)| k.starts_with("fixed"))
                    .map(|(k, val)| (k.clone(), val.clone()))
            });
        if min.is_some_and(|m| m >= 1) || fijo.is_some() {
            v.push(Exigencia { id: id.to_string(), min, fijo });
        }
    }
    v
}

/// Recorre una ruta dentro de un recurso, atravesando los arrays.
///
/// Devuelve TODOS los valores encontrados: `Composition.section.title` sobre
/// cuatro secciones devuelve cuatro títulos. Un array vacío por el camino
/// devuelve la lista vacía, que es lo que hace que `min >= 1` falle.
fn recorrer<'a>(v: &'a serde_json::Value, ruta: &[&str]) -> Vec<&'a serde_json::Value> {
    let Some((primero, resto)) = ruta.split_first() else {
        // EN LA HOJA, un array se DESENVUELVE. En FHIR un elemento con `max > 1`
        // se serializa como array, y tanto la cardinalidad como el valor fijo se
        // predican de CADA ocurrencia, no del array. Sin esto,
        // `Composition.meta.profile` —que es `["…"]`— nunca casaba con su
        // `fixedCanonical`, y un `min = 2` se veía satisfecho por un array de uno.
        return match v {
            serde_json::Value::Array(xs) => xs.iter().collect(),
            otro => vec![otro],
        };
    };
    match v {
        serde_json::Value::Array(xs) => xs.iter().flat_map(|x| recorrer(x, ruta)).collect(),
        serde_json::Value::Object(o) => match o.get(*primero) {
            Some(hijo) => recorrer(hijo, resto),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// El insumo del RDA de paciente, con los datos del ejemplo oficial.
///
/// Se construye aquí y no se importa del `mod pruebas` del ensamblador: ese
/// módulo es `#[cfg(test)]` del crate y no cruza a un banco de integración. Los
/// valores son los del ejemplo de MinSalud, que es el caso que el producto sabe
/// producir hoy.
fn insumo() -> Insumo<'static> {
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
        // Un clínico confirmó las cuatro secciones vacías (revisó, nada conocido):
        // el único camino que emite `nilknown` (puerta #603).
        aval: Aval::nuevo(&SECCIONES_CONFIRMADAS),
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

fn bundle_de_paciente() -> serde_json::Value {
    let i = insumo();
    let b = ensamblar(&i).expect("el RDA de paciente se ensambla con el insumo del ejemplo");
    serde_json::to_value(b).expect("el Bundle serializa")
}

/// Saca de un Bundle el primer recurso de un `resourceType`.
fn recurso<'a>(bundle: &'a serde_json::Value, tipo: &str) -> Option<&'a serde_json::Value> {
    bundle["entry"]
        .as_array()?
        .iter()
        .map(|e| &e["resource"])
        .find(|r| r["resourceType"] == tipo)
}

/// Saca el recurso cuyo `meta.profile[0]` TERMINA en `nombre` — para distinguir
/// dos recursos del mismo `resourceType` (el custodio y el pagador son ambos
/// `Organization`, con perfiles distintos).
fn recurso_por_perfil<'a>(bundle: &'a serde_json::Value, nombre: &str) -> Option<&'a serde_json::Value> {
    bundle["entry"].as_array()?.iter().map(|e| &e["resource"]).find(|r| {
        r["meta"]["profile"][0].as_str().is_some_and(|u| u.ends_with(nombre))
    })
}

/// El resultado de contrastar un recurso contra su perfil.
struct Veredicto {
    cumplidas: usize,
    incumplidas: Vec<String>,
    /// Rebanadas de FHIR: este banco no resuelve discriminadores.
    sin_evaluar: Vec<String>,
    /// Exigencias que NO aplican porque su padre opcional no está presente.
    no_aplica: Vec<String>,
}

/// El elemento BRUTO del `differential` por su `id`, sin filtrar por min/fijo.
/// Se usa para leer el discriminador y el valor discriminante de una rebanada,
/// que no siempre son exigencias por sí mismos.
fn elemento<'a>(perfil: &'a serde_json::Value, id: &str) -> Option<&'a serde_json::Value> {
    perfil["differential"]["element"]
        .as_array()?
        .iter()
        .find(|e| e["id"].as_str() == Some(id))
}

/// El discriminador de un elemento rebanado: `(tipo, path)`.
///
/// FHIR lo declara en `<elemento>.slicing.discriminator[]`. Aquí se resuelve el
/// PRIMERO —los perfiles del RDA rebanan por uno solo (`value` sobre
/// `coding.system`, `pattern` sobre `code`)—; si algún perfil trajera varios,
/// esto se quedaría corto y hay que ampliarlo, no callar.
fn discriminador(perfil: &serde_json::Value, parent_id: &str) -> Option<(String, String)> {
    let d = &elemento(perfil, parent_id)?["slicing"]["discriminator"][0];
    Some((d["type"].as_str()?.to_string(), d["path"].as_str()?.to_string()))
}

/// El valor FIJO o PATRÓN de un elemento del differential, si lo tiene.
/// Devuelve `(es_patron, valor)`: un `pattern*` casa por subconjunto; un `fixed*`,
/// por igualdad.
fn fijo_o_patron(e: &serde_json::Value) -> Option<(bool, serde_json::Value)> {
    let o = e.as_object()?;
    o.iter()
        .find(|(k, _)| k.starts_with("fixed") || k.starts_with("pattern"))
        .map(|(k, val)| (k.starts_with("pattern"), val.clone()))
}

/// ¿`valor` satisface el `patron`? Igualdad para escalares; subconjunto recursivo
/// para objetos (todo lo que el patrón nombra tiene que estar y coincidir en el
/// valor); igualdad de conjunto-por-elemento para arrays de un patrón. Es la
/// semántica de `pattern[x]` de FHIR, acotada a lo que los perfiles del RDA usan.
fn casa_patron(valor: &serde_json::Value, patron: &serde_json::Value) -> bool {
    match (valor, patron) {
        (serde_json::Value::Object(v), serde_json::Value::Object(p)) => p
            .iter()
            .all(|(k, pv)| v.get(k).is_some_and(|vv| casa_patron(vv, pv))),
        (serde_json::Value::Array(v), serde_json::Value::Array(p)) => p
            .iter()
            .all(|pe| v.iter().any(|ve| casa_patron(ve, pe))),
        _ => valor == patron,
    }
}


fn contrastar(recurso: &serde_json::Value, perfil: &serde_json::Value) -> Veredicto {
    let mut v = Veredicto {
        cumplidas: 0,
        incumplidas: Vec::new(),
        sin_evaluar: Vec::new(),
        no_aplica: Vec::new(),
    };
    for ex in exigencias(perfil) {
        if ex.rebanada() {
            evaluar_rebanada(recurso, perfil, &ex, &mut v);
            continue;
        }
        let ruta = ex.ruta();
        if ruta.is_empty() {
            // El elemento raíz (`Composition`) no impone nada recorrible.
            continue;
        }
        // UN HIJO DE UN PADRE AUSENTE NO SE EXIGE, y esto es la diferencia entre
        // medir y hacer ruido.
        //
        // `Composition.identifier` es `min=0` y sus hijos `system` y `value` son
        // `min=1`: en FHIR eso significa «si emites `identifier`, tiene que
        // llevarlos», no «emite `identifier`». La primera versión de este banco
        // los daba por incumplidos y con eso me habría mandado a inventar un
        // identificador de documento que ni el perfil ni el ejemplo oficial de
        // MinSalud piden — el ejemplo lo trae en `null`.
        //
        // Se arregla en la REGLA y no en el caso (directiva 23): si algún
        // antepasado del elemento no está en el recurso, la exigencia no aplica
        // y se cuenta aparte. La cadena sigue funcionando donde SÍ debe: `event`
        // es `min=1`, así que su ausencia se reporta una vez, en `event`, y no
        // tres veces en `period`, `period.start` y `period.end`.
        if ruta.len() > 1 && recorrer(recurso, &ruta[..ruta.len() - 1]).is_empty() {
            v.no_aplica.push(format!("{} (su padre no está, y es opcional)", ex.id));
            continue;
        }
        let hallados = recorrer(recurso, &ruta);
        // La cardinalidad se comprueba CONTANDO, no solo mirando si hay algo.
        // `Composition.event.code` es `min=2 max=2` —modalidad Y grupo de
        // servicios— y con «hay al menos uno» un solo código pasaba.
        if let Some(m) = ex.min.filter(|m| *m >= 1) {
            let hay = hallados.len() as u64;
            if hay < m {
                v.incumplidas.push(if hay == 0 {
                    format!("{} — obligatorio y NO se emite", ex.id)
                } else {
                    format!("{} — el perfil exige {m} y se emiten {hay}", ex.id)
                });
                continue;
            }
        }
        if let Some((clave, esperado)) = &ex.fijo {
            // Un valor fijo sobre un elemento ausente no es incumplimiento por
            // sí solo: si su `min` es 0, no emitirlo es legítimo. Lo que no vale
            // es emitirlo DISTINTO.
            for h in &hallados {
                if *h != esperado {
                    v.incumplidas.push(format!(
                        "{} — el perfil fija {clave}={esperado} y se emite {h}",
                        ex.id
                    ));
                }
            }
        }
        v.cumplidas += 1;
    }
    v
}

/// Los valores fijos que definen una rebanada, bajo el path de su discriminador.
///
/// El valor distintivo puede estar en el PROPIO `disc_path` (`code` con
/// `patternCodeableConcept`, `coding.system` con `fixedUri`), MÁS ABAJO
/// (`type.coding.code` con `fixedCode`, cuando el discriminador es un
/// CodeableConcept complejo como `participant.type`), o —para una EXTENSIÓN, que se
/// rebana por `url`— en el `type[0].profile[0]` del propio elemento de la rebanada
/// (convención FHIR: el `url` de una extensión ES la URL de su perfil). Cada uno se
/// devuelve con su subruta relativa al discriminador.
fn valores_discriminador(
    perfil: &serde_json::Value,
    slice_id: &str,
    disc_path: &str,
) -> Vec<(Vec<String>, serde_json::Value)> {
    let prefijo = format!("{slice_id}.{disc_path}");
    let mut fijos: Vec<(Vec<String>, serde_json::Value)> = Vec::new();
    if let Some(els) = perfil["differential"]["element"].as_array() {
        for e in els {
            let Some(eid) = e["id"].as_str() else { continue };
            let sub: Option<Vec<String>> = if eid == prefijo {
                Some(Vec::new())
            } else {
                eid.strip_prefix(&format!("{prefijo}."))
                    .map(|resto| resto.split('.').map(String::from).collect())
            };
            if let (Some(sub), Some((_, val))) = (sub, fijo_o_patron(e)) {
                fijos.push((sub, val));
            }
        }
    }
    // Extensión rebanada por `url`: el valor lo da el perfil de la extensión.
    if fijos.is_empty()
        && disc_path == "url"
        && let Some(url) =
            elemento(perfil, slice_id).and_then(|e| e["type"][0]["profile"][0].as_str())
    {
        fijos.push((Vec::new(), serde_json::Value::String(url.to_string())));
    }
    fijos
}

/// Resuelve una exigencia REBANADA —incluso ANIDADA
/// (`diagnosis:MainDiagnosis.extension:ExtensionDiagnosisType`)— navegando nivel a
/// nivel, y evalúa la exigencia dentro de la instancia final.
///
/// Es la mitad que el banco no hacía. Sin esto, decenas de exigencias salían «sin
/// evaluar», y «sin evaluar» se lee igual que «conforme» al de un vistazo
/// (directiva 30). Lo que NO se puede resolver —sin discriminador, o una rebanada
/// que no fija el suyo— sigue yendo a `sin_evaluar` con el motivo.
fn evaluar_rebanada(
    recurso: &serde_json::Value,
    perfil: &serde_json::Value,
    ex: &Exigencia,
    v: &mut Veredicto,
) {
    let id = &ex.id;
    let toks: Vec<&str> = id.split('.').collect();
    // El ÚLTIMO token rebanado: hasta ahí se navega por niveles; lo que sigue es la
    // subruta simple final.
    let Some(k) = toks.iter().rposition(|t| t.contains(':')) else {
        v.sin_evaluar.push(format!("{id} (no se pudo partir la rebanada)"));
        return;
    };
    let slice_final = toks[k].split_once(':').map_or("", |(_, s)| s);

    // Navegar toks[1..=k]. `nodos` son los candidatos vivos; los tokens simples
    // avanzan por el campo, los rebanados además FILTRAN por los fijos del
    // discriminador de ese nivel.
    let mut nodos: Vec<&serde_json::Value> = vec![recurso];
    for i in 1..=k {
        let t = toks[i];
        let Some((campo, slice)) = t.split_once(':') else {
            // Token simple entre rebanadas: avanzar por el campo.
            nodos = nodos.iter().flat_map(|n| recorrer(n, &[t])).collect();
            continue;
        };
        nodos = nodos.iter().flat_map(|n| recorrer(n, &[campo])).collect();
        let padre_nivel = format!("{}.{campo}", toks[..i].join("."));
        let Some((_t, disc_path)) = discriminador(perfil, &padre_nivel) else {
            v.sin_evaluar.push(format!("{id} (sin discriminador en {padre_nivel})"));
            return;
        };
        let slice_id = toks[..=i].join(".");
        let fijos = valores_discriminador(perfil, &slice_id, &disc_path);
        if fijos.is_empty() {
            v.sin_evaluar
                .push(format!("{id} (la rebanada «{slice}» no fija su discriminador {disc_path})"));
            return;
        }
        let disc_partes: Vec<&str> = disc_path.split('.').collect();
        nodos.retain(|c| {
            fijos.iter().all(|(sub, val)| {
                let mut ruta = disc_partes.clone();
                ruta.extend(sub.iter().map(String::as_str));
                recorrer(c, &ruta).iter().any(|h| casa_patron(h, val))
            })
        });
    }

    if nodos.is_empty() {
        // La rebanada (final) no está. Que sea incumplimiento depende del `min` de
        // la REBANADA misma, NO del de sus sub-elementos (fail-cerrado, directiva
        // 30): `sectionClarificationNotes` es `min=0` y sus `title`/`code` `min=1`.
        let slice_id = toks[..=k].join(".");
        let slice_min = elemento(perfil, &slice_id)
            .and_then(|e| e.get("min"))
            .and_then(serde_json::Value::as_u64);
        let obligatoria = match slice_min {
            Some(m) => m >= 1,
            None => ex.min.is_some_and(|m| m >= 1),
        };
        if obligatoria {
            v.incumplidas
                .push(format!("{id} — la rebanada «{slice_final}» es obligatoria y NO se emite"));
        } else {
            v.no_aplica.push(format!("{id} (rebanada «{slice_final}» opcional, ausente)"));
        }
        return;
    }
    let instancia = nodos[0];

    // La subruta simple tras la última rebanada. Sin subruta, la presencia ya quedó
    // probada por haber casado el discriminador.
    let sub_ref: Vec<&str> = toks[k + 1..].to_vec();
    // UN SUB-ELEMENTO CUYO ANCESTRO OPCIONAL NO ESTÁ NO SE EXIGE —la misma regla que
    // el camino sin rebanada—. `sectionProblems.emptyReason` es `min=0`, así que sus
    // hijos `emptyReason.coding.*` (`min=1`) solo aplican SI la sección lleva
    // `emptyReason`; una sección POBLADA (con `entry`) no lo lleva (cmp-2). No ciega
    // el caso real: si `emptyReason` SÍ está pero le falta el `coding`, resuelve.
    if sub_ref.len() > 1 && recorrer(instancia, &sub_ref[..sub_ref.len() - 1]).is_empty() {
        v.no_aplica
            .push(format!("{id} (ancestro opcional ausente en la rebanada «{slice_final}»)"));
        return;
    }
    let hallados = if sub_ref.is_empty() {
        vec![instancia]
    } else {
        recorrer(instancia, &sub_ref)
    };
    if let Some(m) = ex.min.filter(|m| *m >= 1)
        && (hallados.len() as u64) < m
    {
        v.incumplidas
            .push(format!("{id} — «{slice_final}»: exige {m}, se emiten {}", hallados.len()));
        return;
    }
    if let Some((clave, esperado)) = &ex.fijo {
        for h in &hallados {
            if *h != esperado {
                v.incumplidas
                    .push(format!("{id} — «{slice_final}»: fija {clave}={esperado}, se emite {h}"));
                return;
            }
        }
    }
    v.cumplidas += 1;
}

/// **LOS CANÓNICOS QUE LA GUÍA REFERENCIA Y NO TRAE.** Once, y están declarados.
///
/// El paquete NO es autosuficiente: se bajó artefacto a artefacto porque el
/// `package.tgz` no está publicado, así que falta lo que la guía no expone como
/// `.json` suelto. Lo midió la revisión de la directiva 25 del 2026-08-16.
///
/// **Por qué esto es una lista y no una nota**: un `ValueSet` que no se puede
/// expandir devuelve la expansión VACÍA, y una expansión vacía es
/// indistinguible de «todos los códigos valen» y de «no había nada que probar».
/// El caso se salta solo y la prueba sale verde sin haber medido (directiva 30).
/// Con la lista declarada, el hueco de HOY es un dato y el hueco NUEVO de mañana
/// es un rojo.
const CANONICOS_QUE_FALTAN: &[&str] = &[
    // Lo usa `ValueSet-ColombianTerritorialJurisdictionCodes`, que por tanto no
    // se puede expandir sin red.
    "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianTerritorialJurisdiction",
    // Las cinco operaciones que el `CapabilityStatement` declara `SHALL`. Solo
    // hay UN `OperationDefinition` en el paquete, de seis.
    //
    // OJO al canónico: va SIN el segmento `/rda/`, al revés que todo lo demás de
    // la guía. Esta lista se escribió primero con `/rda/` —copiada del informe en
    // vez de medida— y la prueba la tumbó. Se deja anotado porque al actualizar
    // la guía la tentación es volver a «corregirlo».
    "https://fhir.minsalud.gov.co/OperationDefinition/rda-paciente",
    "https://fhir.minsalud.gov.co/OperationDefinition/rda-urgencias",
    "https://fhir.minsalud.gov.co/OperationDefinition/rda-hospitalizacion",
    "https://fhir.minsalud.gov.co/OperationDefinition/rda-consultas",
    "https://fhir.minsalud.gov.co/OperationDefinition/rda-consulta-externa",
    // CERO recursos `NamingSystem` en el paquete, y los ejemplos usan tres como
    // `identifier.system`: la resolución de identificadores de persona no se
    // puede probar offline con lo que hay.
    "https://fhir.minsalud.gov.co/rda/NamingSystem/RNEC",
    "https://fhir.minsalud.gov.co/rda/NamingSystem/RETHUS",
    "https://fhir.minsalud.gov.co/rda/NamingSystem/SSO",
];

// DOS de la lista del informe NO cuelgan por esta vía y se sacaron: la base de la
// guía (`…/rda/`) y `StructureDefinition/DeviceDefinition-HIS`. Aparecen en el
// informe de la revisión porque allí se resolvieron TODOS los campos de tipo
// URL; aquí solo se miran los siete que de verdad se resuelven al validar
// (`system`, `valueSet`, `baseDefinition`, `profile`, `targetProfile`,
// `definition`, `reference`). Dejarlos habría hecho que la lista mintiera en la
// otra dirección — y la prueba lo dice cuando pasa, que es lo que la hace útil
// en los dos sentidos.

/// Todos los recursos del IG, ya leídos.
fn artefactos() -> Vec<(String, serde_json::Value)> {
    let dir = carpeta_ig();
    let mut v = Vec::new();
    for e in std::fs::read_dir(&dir).expect("el IG está").filter_map(|e| e.ok()) {
        if e.path().extension().is_none_or(|x| x != "json") {
            continue;
        }
        let nombre = e.file_name().to_string_lossy().to_string();
        let texto = std::fs::read_to_string(e.path()).expect("se lee");
        v.push((nombre, serde_json::from_str(&texto).expect("es JSON")));
    }
    v
}

/// **UN `CodeSystem` INCOMPLETO NO PUEDE DECIR QUE UN CÓDIGO ES INVÁLIDO.**
///
/// Dos artefactos de la guía —`CodeSystem-ICD10CO` y `CodeSystem-ICD11CO`—
/// declaran como suyo el canónico INTERNACIONAL (`http://hl7.org/fhir/sid/icd-10`
/// y `…/icd-11`) trayendo **395 y 1 004 conceptos**, cuando la CIE-10 tiene unas
/// 14 000 categorías. Llevan `content: "fragment"`, que es exactamente la señal
/// que FHIR define para esto — pero un índice por canónico que no la lea
/// declararía inválido el 97 % de los diagnósticos legítimos.
///
/// En una historia clínica el modo de fallo es **rechazar un diagnóstico real**,
/// así que la regla se escribe y se prueba. Lo señaló la revisión de la
/// directiva 25 del 2026-08-16.
#[test]
fn un_codesystem_incompleto_no_juzga_codigos() {
    let mut fragmentos: Vec<(String, String, String)> = Vec::new();
    let mut completos = 0;
    for (nombre, v) in artefactos() {
        if !nombre.starts_with("CodeSystem-") {
            continue;
        }
        match v["content"].as_str() {
            Some("complete") => completos += 1,
            otro => fragmentos.push((
                nombre,
                v["url"].as_str().unwrap_or("").to_string(),
                otro.unwrap_or("(sin content)").to_string(),
            )),
        }
    }
    // Positivo de control: si el bucle no hubiera leído nada, las dos listas
    // saldrían vacías y la prueba pasaría sin haber mirado.
    assert!(completos > 40, "solo {completos} CodeSystem completos: el barrido no leyó el IG");

    // SIETE fragmentos, no dos. La primera versión de esta prueba decía dos y el
    // banco la tumbó: `CUPS`, `CUMS`, `IUM`, `IUMPrimerNivel` y `MipresINN` son
    // catálogos GRANDES que la guía publica como muestra, con su canónico propio
    // — legítimos como identidad, inservibles para juzgar ausencia.
    let urls: Vec<&str> = fragmentos.iter().map(|(_, u, _)| u.as_str()).collect();
    assert_eq!(
        fragmentos.len(),
        7,
        "fragmentos inesperados; revísalos antes de indexar por canónico: {fragmentos:?}"
    );

    // LOS DOS PELIGROSOS son los que usurpan un canónico que NO es de la guía:
    // ahí el fragmento se hace pasar por la definición internacional. Un índice
    // por `url` que no lea `content` haría que
    // `http://hl7.org/fhir/sid/icd-10` resuelva, dentro de medico, a 395 códigos
    // de unas 14 000 — y el 97 % de los diagnósticos legítimos saldría
    // «inválido». En una historia clínica eso es rechazar un diagnóstico real.
    let usurpadores: Vec<&&str> =
        urls.iter().filter(|u| !u.starts_with("https://fhir.minsalud.gov.co/")).collect();
    assert_eq!(
        usurpadores.len(),
        2,
        "usurpadores de canónico ajeno inesperados: {usurpadores:?}"
    );
    assert!(urls.contains(&"http://hl7.org/fhir/sid/icd-10"), "{urls:?}");
    assert!(urls.contains(&"http://hl7.org/fhir/sid/icd-11"), "{urls:?}");

    // Y la pareja, que es la que hace que esto discrimine: los CINCO de la guía
    // sí llevan su canónico propio, así que la regla NO es «todo fragmento es
    // sospechoso» sino «ninguno juzga, y dos además mienten sobre quién son».
    let propios: Vec<&&str> =
        urls.iter().filter(|u| u.starts_with("https://fhir.minsalud.gov.co/")).collect();
    assert_eq!(propios.len(), 5, "fragmentos propios de la guía: {propios:?}");
}

/// **LOS CANÓNICOS COLGANTES SON EXACTAMENTE LOS DECLARADOS.**
///
/// La pareja de `CANONICOS_QUE_FALTAN`: si mañana la guía referencia uno nuevo
/// que tampoco trae, esta prueba se pone roja en vez de que el banco se lo salte
/// en silencio.
#[test]
fn los_canonicos_que_faltan_son_los_declarados() {
    use std::collections::BTreeSet;
    fn recoger(v: &serde_json::Value, dentro: &mut BTreeSet<String>) {
        match v {
            serde_json::Value::Object(o) => {
                for (k, x) in o {
                    if matches!(
                        k.as_str(),
                        // `definition` y `reference` también: sin ellas, las cinco
                        // `OperationDefinition` que el `CapabilityStatement`
                        // declara `SHALL` no salían como colgantes, y la lista
                        // parecía mentir cuando el ciego era el instrumento.
                        "system"
                            | "valueSet"
                            | "baseDefinition"
                            | "profile"
                            | "targetProfile"
                            | "definition"
                            | "reference"
                    ) {
                        match x {
                            serde_json::Value::String(s) => {
                                dentro.insert(s.clone());
                            }
                            serde_json::Value::Array(xs) => {
                                for y in xs {
                                    if let Some(s) = y.as_str() {
                                        dentro.insert(s.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    recoger(x, dentro);
                }
            }
            serde_json::Value::Array(xs) => xs.iter().for_each(|x| recoger(x, dentro)),
            _ => {}
        }
    }

    let mut presentes: BTreeSet<String> = BTreeSet::new();
    let mut referenciados: BTreeSet<String> = BTreeSet::new();
    for (_, v) in artefactos() {
        if let Some(u) = v["url"].as_str() {
            presentes.insert(u.to_string());
        }
        recoger(&v, &mut referenciados);
    }
    assert!(presentes.len() > 190, "solo {} canónicos presentes: no se leyó el IG", presentes.len());

    // Solo se juzgan los de la propia guía: los de HL7 y la OMS no se esperan
    // aquí, y exigirlos sería pedir que el paquete traiga medio FHIR.
    let colgantes: BTreeSet<&str> = referenciados
        .iter()
        .filter(|u| u.starts_with("https://fhir.minsalud.gov.co/"))
        .filter(|u| !presentes.contains(u.as_str()))
        .map(String::as_str)
        .collect();
    let declarados: BTreeSet<&str> = CANONICOS_QUE_FALTAN.iter().copied().collect();

    let nuevos: Vec<&&str> = colgantes.difference(&declarados).collect();
    assert!(
        nuevos.is_empty(),
        "canónicos colgantes NUEVOS, no declarados. Míralos antes de escribir nada \
         que los resuelva —una expansión vacía se ve igual que «todo vale»—: {nuevos:?}"
    );
    let resueltos: Vec<&&str> = declarados.difference(&colgantes).collect();
    assert!(
        resueltos.is_empty(),
        "estos ya NO cuelgan: sácalos de `CANONICOS_QUE_FALTAN` en vez de dejar la \
         lista mintiendo: {resueltos:?}"
    );
}

/// **EL IG VENDORIZADO ESTÁ COMPLETO Y ES EL QUE DICE SER.**
///
/// Positivo de control de todo lo demás: si la carpeta estuviera vacía o los
/// archivos no fueran perfiles, las pruebas de abajo recorrerían cero
/// exigencias y saldrían verdes sin haber medido nada (directiva 30).
#[test]
fn el_ig_vendorizado_esta_donde_se_espera_y_son_perfiles() {
    let dir = carpeta_ig();
    let archivos: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("no está {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .collect();
    assert_eq!(
        archivos.len(),
        206,
        "el IG tiene 206 artefactos —207 menos el ejemplo excluido por datos personales,\n         ver MANIFIESTO.txt—; hay {}. Si cambia, se rehace el manifiesto a la vez",
        archivos.len()
    );
    let perfiles = archivos
        .iter()
        .filter(|e| e.file_name().to_string_lossy().starts_with("StructureDefinition-"))
        .count();
    assert_eq!(perfiles, 75, "75 perfiles; hay {perfiles}");

    // Y uno concreto abre y dice lo suyo: que la cuenta cuadre no prueba que el
    // contenido sirva.
    let p = perfil("PatientRDA");
    assert_eq!(p["resourceType"], "StructureDefinition");
    assert_eq!(p["url"], "https://fhir.minsalud.gov.co/rda/StructureDefinition/PatientRDA");
    assert!(!exigencias(&p).is_empty(), "PatientRDA no impuso ni una exigencia: algo se rompió");
}

/// **EL BUNDLE DEL RDA DE PACIENTE DECLARA LOS PERFILES QUE DICE**, y existen en
/// la Guía.
///
/// Es barato y cierra un modo de fallo real: una errata en la URL del perfil
/// —`CompositionPatientStatmentRDA`— no la ve nadie, el JSON sigue siendo válido,
/// y el servidor nacional lo rechaza o lo guarda sin perfilar.
#[test]
fn los_perfiles_que_declara_el_bundle_existen_en_la_guia() {
    let b = bundle_de_paciente();
    let mut vistos = 0;
    for entrada in b["entry"].as_array().expect("el Bundle tiene entradas") {
        let r = &entrada["resource"];
        let Some(perfiles) = r["meta"]["profile"].as_array() else { continue };
        for p in perfiles {
            let url = p.as_str().expect("el perfil es una cadena");
            let nombre = url.rsplit('/').next().expect("la URL tiene última parte");
            let ruta = carpeta_ig().join(format!("StructureDefinition-{nombre}.json"));
            assert!(
                ruta.exists(),
                "el recurso {} declara el perfil «{url}» y ese perfil NO está en la Guía",
                r["resourceType"]
            );
            vistos += 1;
        }
    }
    assert!(vistos >= 3, "solo se comprobaron {vistos} perfiles: el Bundle salió casi vacío");
}

/// **LO QUE EL PERFIL EXIGE Y MEDICO NO EMITE**, nombrado uno por uno.
///
/// Esta prueba **está en rojo a propósito hasta que #603 se cierre**, y por eso
/// no afirma conformidad: afirma la LISTA EXACTA de lo que falta, para que
/// arreglar uno no borre en silencio el registro de los otros. El día que la
/// lista se vacíe, el `assert` de abajo lo dirá.
///
/// Los tres huecos que la Guía destapó al vendorizarla (2026-08-16):
///   · `Composition.identifier` — obligatorio con `system` fijo, y no se emite.
///   · `Composition.event` — `min=1`, con DOS códigos y `period.start`/`end`;
///     medico lo emite solo si hay modalidad, con uno y sin periodo.
///   · `section.emptyReason` con `nilknown` en cada sección vacía.
#[test]
fn el_rda_de_paciente_contra_su_perfil() {
    let b = bundle_de_paciente();
    let composicion = recurso(&b, "Composition").expect("el Bundle lleva su Composition");
    let v = contrastar(composicion, &perfil("CompositionPatientStatementRDA"));

    // El positivo de control va PRIMERO: sin exigencias evaluadas, cualquier
    // veredicto de abajo es ruido.
    assert!(
        v.cumplidas > 0,
        "no se evaluó ni una exigencia: el contraste no llegó a correr"
    );

    // LA LISTA SALE DE LA MEDICIÓN, no de lo que yo creía. Nació con siete
    // entradas escritas de memoria; el banco tumbó dos como falso positivo
    // —`Composition.identifier` es `min=0`, así que sus hijos solo se exigen si
    // el padre está, y el ejemplo oficial tampoco lo emite— y una porque `event`
    // sí se emitía.
    //
    // **Quedó VACÍA el 2026-08-16**, al completar el `event` con sus dos códigos
    // y su `period`. Vacía significa: de las exigencias que este banco SABE
    // evaluar, no queda ninguna incumplida. NO significa «conforme» — mira las
    // cifras que imprime al final, que dicen cuántas quedaron sin evaluar.
    let esperadas: BTreeMap<&str, &str> = BTreeMap::new();

    let faltan: Vec<&str> = v
        .incumplidas
        .iter()
        .map(|s| s.split(" — ").next().unwrap_or(s))
        .collect();

    for f in &faltan {
        assert!(
            esperadas.contains_key(f),
            "INCUMPLIMIENTO NUEVO, no estaba en la lista conocida: «{f}».\n\
             Lista completa:\n  {}",
            v.incumplidas.join("\n  ")
        );
    }

    // Y la dirección contraria: si uno de los conocidos DEJA de fallar, es que se
    // arregló — y hay que sacarlo de la lista, no dejarla mintiendo.
    for (conocido, motivo) in &esperadas {
        assert!(
            faltan.contains(conocido),
            "«{conocido}» ya NO incumple ({motivo}): sácalo de la lista de esta prueba"
        );
    }

    eprintln!(
        "RDA de paciente vs CompositionPatientStatementRDA: {} exigencias evaluadas \
         (rebanadas incluidas, resueltas por discriminador), {} incumplidas, {} sin \
         evaluar (rebanada sin discriminador resoluble) y {} que no aplican (padre \
         opcional ausente)",
        v.cumplidas,
        v.incumplidas.len(),
        v.sin_evaluar.len(),
        v.no_aplica.len()
    );
}

/// **LOS TRES RDA CLÍNICOS CONTRA SU PERFIL** (egreso, urgencias, consulta externa).
///
/// La pareja del test de paciente para los otros tres documentos. Antes, el
/// resolver de rebanadas existía y NO se ejercía aquí, así que su no-conformidad
/// estaba **sin medir** (observación de la revisión de seguridad de #603).
///
/// **Los tres RDA clínicos son estructuralmente CONFORMES a nivel de la
/// `Composition`: 0/0/0 incumplidas** (FASE 4, 2026-08-29). El déficit fue 30/37/35
/// → 3/4/2 → **0/0/0**, al construir los recursos que las secciones referencian:
/// `Composition.encounter` (`Encounter`), `sectionProblems`=`Condition`,
/// `sectionPayers`=`Organization` EAPB (no `Coverage` —el perfil pide una
/// Organization—) y `sectionAddendumDocuments`=`DocumentReference` de la epicrisis.
///
/// **Este test mide la `Composition`; que sus REFERENCIAS apunten a recursos
/// CONFORMES lo mide `los_recursos_clinicos_contra_su_perfil`** — emitir la
/// referencia a un `Encounter` no conforme es un verde falso (D28/D33), y por eso
/// los dos tests van juntos.
///
/// **Lo que NO cierra fase 4 en producción, y está medido** (D20): el `Encounter`
/// exige el ENTORNO de atención (config del prestador, aún sin cablear) en los tres,
/// y el TIPO de diagnóstico de ingreso/egreso en urgencias/hospitalización, que
/// medico NO captura ni la Resolución 948 pide para AU/AH. El ensamblador los EXIGE
/// (`Faltas`) y no los inventa; las fixtures los traen del ejemplo oficial, así que
/// esto mide la conformidad ESTRUCTURAL, no que medico ya emita estos RDA.
///
/// La cifra por perfil es una LÍNEA BASE: si las incumplidas suben, hay regresión;
/// si las evaluadas cambian, alguien tocó el ensamblador o el perfil.
#[test]
fn los_rda_clinicos_contra_su_perfil() {
    use medico_dominio::ihce::clinico::{ensamblar as ensamblar_clinico, Encuentro};
    // Línea base de FASE 4: 0 incumplidas en los tres. (encuentro, evaluadas,
    // incumplidas). Llegó a 0/0/0 el 2026-08-29 al construir Encounter + Condition
    // + pagador EAPB + DocumentReference con datos reales (directiva 20: el dato que
    // no está —entorno, tipo de diagnóstico de AU/AH— hace FALTA, no se inventa).
    let base = [
        (Encuentro::ConsultaExterna, 60usize, 0usize),
        (Encuentro::Urgencias, 76, 0),
        (Encuentro::Hospitalizacion, 74, 0),
    ];
    for (enc, evaluadas, incumplidas) in base {
        let mut i = insumo();
        // Egreso y urgencias EXIGEN declarar el desenlace: `None` presenta como
        // vivo (lo mismo que el ensamblador comprueba). Consulta externa no.
        if matches!(enc, Encuentro::Urgencias | Encuentro::Hospitalizacion) {
            i.paciente.fallecido = Some(false);
        }
        let b = ensamblar_clinico(enc, &i)
            .unwrap_or_else(|f| panic!("el RDA de {enc:?} no se ensambla: {f}"));
        let bundle = serde_json::to_value(b).expect("el Bundle serializa");
        let comp = recurso(&bundle, "Composition").expect("el Bundle lleva su Composition");
        let v = contrastar(comp, &perfil(enc.perfil()));

        eprintln!(
            "RDA {enc:?} vs {}: {} evaluadas, {} incumplidas, {} sin evaluar, {} no aplican",
            enc.perfil(),
            v.cumplidas,
            v.incumplidas.len(),
            v.sin_evaluar.len(),
            v.no_aplica.len()
        );
        for x in &v.incumplidas {
            eprintln!("  INCUMPLE {enc:?}: {x}");
        }

        // Control positivo: el resolver de verdad corrió (no está midiendo el vacío).
        assert!(v.cumplidas > 0, "{enc:?}: no se evaluó ni una exigencia");
        // El banco resuelve TODAS las rebanadas de estos perfiles: nada queda
        // «sin evaluar», que es la mitad que este banco vino a cerrar.
        assert!(
            v.sin_evaluar.is_empty(),
            "{enc:?}: quedaron rebanadas sin resolver:\n  {}",
            v.sin_evaluar.join("\n  ")
        );
        // La línea base. Si cambia, hay que mirarla —arriba es regresión, abajo es
        // un arreglo que hay que registrar bajando el número (que no mienta).
        assert_eq!(
            v.cumplidas, evaluadas,
            "{enc:?}: exigencias evaluadas cambió ({} vs {evaluadas} de base)",
            v.cumplidas
        );
        assert_eq!(
            v.incumplidas.len(),
            incumplidas,
            "{enc:?}: incumplidas cambió ({} vs {incumplidas} de base). Si BAJÓ, se \
             arregló una sección: actualiza la base. Si SUBIÓ, es una regresión.",
            v.incumplidas.len()
        );
    }
}

/// **LOS RECURSOS QUE LA `Composition` REFERENCIA, CONTRA SU PROPIO PERFIL**
/// (#603 fase 4, D28/D33).
///
/// El test de arriba mide la `Composition`: que tenga `encounter` y las `entry`.
/// Pero emitir la referencia a un `Encounter` NO conforme es un VERDE FALSO —la
/// `Composition` pasa «tiene encounter», y MinSalud rechaza el recurso—; es justo
/// lo que reventó el primer intento de fase 4. Así que cada recurso construido se
/// contrasta contra SU `StructureDefinition`, no solo la referencia.
#[test]
fn los_recursos_clinicos_contra_su_perfil() {
    use medico_dominio::ihce::clinico::{ensamblar as ensamblar_clinico, Encuentro};

    // (encuentro, perfil del Encounter, lleva pagador EAPB, lleva epicrisis).
    let casos = [
        (Encuentro::ConsultaExterna, "EncounterAmbulatoryRDA", false, true),
        (Encuentro::Urgencias, "EncounterEmergencyRDA", true, true),
        (Encuentro::Hospitalizacion, "EncounterHospitalizationRDA", true, false),
    ];

    // Contrasta un recurso hallado por perfil y exige conformidad TOTAL.
    let exigir_conforme =
        |bundle: &serde_json::Value, nombre: &str, etiqueta: &str| {
            let r = recurso_por_perfil(bundle, nombre)
                .unwrap_or_else(|| panic!("{etiqueta}: el Bundle no trae ningún {nombre}"));
            let v = contrastar(r, &perfil(nombre));
            assert!(v.cumplidas > 0, "{etiqueta}: no se evaluó ni una exigencia de {nombre}");
            assert!(
                v.sin_evaluar.is_empty(),
                "{etiqueta}: {nombre} dejó rebanadas sin resolver:\n  {}",
                v.sin_evaluar.join("\n  ")
            );
            assert!(
                v.incumplidas.is_empty(),
                "{etiqueta}: {nombre} NO es conforme:\n  {}",
                v.incumplidas.join("\n  ")
            );
        };

    for (enc, perfil_enc, con_eapb, con_epicrisis) in casos {
        let mut i = insumo();
        if matches!(enc, Encuentro::Urgencias | Encuentro::Hospitalizacion) {
            i.paciente.fallecido = Some(false);
        }
        let bundle = serde_json::to_value(
            ensamblar_clinico(enc, &i).unwrap_or_else(|f| panic!("{enc:?} no ensambla: {f}")),
        )
        .unwrap();
        let etq = format!("{enc:?}");

        exigir_conforme(&bundle, perfil_enc, &etq); // Encounter
        exigir_conforme(&bundle, "ConditionRDA", &etq); // el/los Condition
        if con_eapb {
            exigir_conforme(&bundle, "HealthBenefitPlanAdminOrganizationRDA", &etq);
        }
        if con_epicrisis {
            exigir_conforme(&bundle, "DocumentReferenceEPIRDA", &etq);
        }
    }
}

/// **CONTROL del resolver de rebanadas ANIDADAS** (D33): que el banco DIGA
/// conforme al Encounter no basta —hay que probar que VERÍA una no-conformidad
/// dentro de una rebanada anidada—. Se le quita al Encounter la
/// `extension:ExtensionDiagnosisType` de su diagnóstico (una rebanada DENTRO de
/// `diagnosis:MainDiagnosis`) y el banco DEBE marcarla incumplida; si no, el
/// resolver anidado diría «conforme» a todo y el 0/0/0 sería ciego.
#[test]
fn el_resolver_anidado_ve_una_extension_que_falta() {
    use medico_dominio::ihce::clinico::{ensamblar as ensamblar_clinico, Encuentro};
    let bundle =
        serde_json::to_value(ensamblar_clinico(Encuentro::ConsultaExterna, &insumo()).unwrap())
            .unwrap();
    let encounter = recurso_por_perfil(&bundle, "EncounterAmbulatoryRDA").expect("Encounter").clone();
    let p = perfil("EncounterAmbulatoryRDA");

    // VERDE: conforme de partida (y evaluó de verdad la rebanada anidada).
    let v = contrastar(&encounter, &p);
    assert!(v.incumplidas.is_empty(), "el Encounter parte conforme: {:?}", v.incumplidas);
    assert!(v.sin_evaluar.is_empty(), "la extensión anidada se resolvió: {:?}", v.sin_evaluar);

    // ROJO: sin la extensión del tipo de diagnóstico, el banco lo ve.
    let mut mutado = encounter;
    mutado["diagnosis"][0]["extension"] = serde_json::json!([]);
    let v = contrastar(&mutado, &p);
    assert!(
        v.incumplidas.iter().any(|x| x.contains("ExtensionDiagnosisType")),
        "el resolver anidado no vio la extensión que falta: {:?}",
        v.incumplidas
    );
}

/// **LA PAREJA DEL ARREGLO DE LA REBANADA AUSENTE** (directiva 33): el banco
/// decidía «obligatoria» por el `min` del SUB-elemento (`title`/`code`, `min=1`) y
/// no por el de la REBANADA, así que marcaba `sectionClarificationNotes` (`min=0`,
/// opcional) como incumplida sin estarlo. El arreglo debe DISCRIMINAR: la opcional
/// ausente NO incumple, y una OBLIGATORIA ausente SÍ —si no, el arreglo habría
/// cegado al banco ante una sección que de verdad falta—.
#[test]
fn rebanada_ausente_opcional_no_incumple_obligatoria_si() {
    use medico_dominio::ihce::clinico::{ensamblar as ensamblar_clinico, Encuentro};
    let mut i = insumo();
    // Urgencias exige declarar el desenlace (`None` presentaría como vivo).
    i.paciente.fallecido = Some(false);
    let bundle =
        serde_json::to_value(ensamblar_clinico(Encuentro::Urgencias, &i).expect("ensambla"))
            .unwrap();
    let comp = recurso(&bundle, "Composition").expect("Composition").clone();
    let perfil = perfil("CompositionEmergencyRDA");

    // VERDE: `sectionClarificationNotes` (min=0) no se emite → «no aplica», nunca
    // incumplida. Antes del arreglo caía en `incumplidas`.
    let v = contrastar(&comp, &perfil);
    assert!(
        !v.incumplidas.iter().any(|x| x.contains("sectionClarificationNotes")),
        "una rebanada OPCIONAL ausente no incumple: {:?}",
        v.incumplidas.iter().filter(|x| x.contains("ClarificationNotes")).collect::<Vec<_>>()
    );
    assert!(
        v.no_aplica.iter().any(|x| x.contains("sectionClarificationNotes")),
        "y debe registrarse como «no aplica», no callarse"
    );

    // ROJO / CONTROL: quito `sectionProblems` (min=1, OBLIGATORIA) del documento y
    // el banco DEBE seguir marcándola incumplida. Esto prueba que el arreglo no
    // cegó la rama de rebanada ausente, sino que ahora mira el min correcto.
    // `comp` ya no se usa tras esto, así que se mueve en vez de clonar.
    let mut mutado = comp;
    mutado["section"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s["code"]["coding"][0]["code"] != "11450-4"); // sectionProblems
    let v2 = contrastar(&mutado, &perfil);
    assert!(
        v2.incumplidas.iter().any(|x| x.contains("sectionProblems")
            && x.contains("obligatoria")),
        "una rebanada OBLIGATORIA ausente SÍ incumple: {:?}",
        v2.incumplidas
    );
}
