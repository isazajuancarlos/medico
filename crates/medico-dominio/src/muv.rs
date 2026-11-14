// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Los maestros del Mecanismo Único de Validación, empotrados (#250).
//!
//! Salen del contenedor del propio validador de MinSalud —`Rips.REFs_*`—, que
//! es la tabla contra la que el MUV juzga lo que se le envía.
//!
//! **Ningún código de catálogo se escribe a mano.** El 2026-07-26 se inventó
//! aquí al lado un mapa de presentaciones (`tableta = 1`, cuando el catálogo
//! dice 66 y el 1 es AMPOLLA); el MUV lo aceptó y devolvió CUV, porque ese campo
//! lo valida por rango y no contra la tabla. Un dato falso que pasa la
//! validación no lo corrige nadie, porque nadie lo ve.

use serde::Deserialize;
use std::sync::OnceLock;

const TIPO_DOC_JSON: &str = include_str!("../datos/muv_tipo_doc.json");
const FINALIDAD_JSON: &str = include_str!("../datos/muv_finalidad.json");
const CAUSA_JSON: &str = include_str!("../datos/muv_causa_motivo.json");
const UPR_JSON: &str = include_str!("../datos/muv_upr.json");
const CONDICION_DESTINO_JSON: &str = include_str!("../datos/muv_condicion_destino.json");
const VIA_INGRESO_JSON: &str = include_str!("../datos/muv_via_ingreso.json");

#[derive(Debug, Deserialize)]
struct FilaTipoDoc {
    #[serde(rename = "TipoDocumentoIdentificacion")]
    tipo: String,
    #[serde(rename = "DocumentoIdentificacion")]
    nombre: String,
    /// Si el MUV lo admite para quien REALIZA la atención. Es la columna que
    /// separa los ocho de los trece.
    #[serde(rename = "RealizaAtencionRips")]
    realiza_atencion: bool,
    /// Si el MUV lo admite para el USUARIO (paciente) del RIPS. Es una columna
    /// distinta de `realiza_atencion`: un menor con TI o RC, o alguien «sin
    /// identificar», es paciente válido pero **no** puede ser quien atendió.
    #[serde(rename = "UsuarioRips")]
    usuario: bool,
}

#[derive(Debug, Deserialize)]
struct ArchivoTipoDoc {
    filas: Vec<FilaTipoDoc>,
}

/// Los tipos de documento que el MUV admite para el profesional que atendió,
/// con su nombre. Ninguno es de menor ni de «sin identificar»: quien atiende es
/// un adulto identificado.
///
/// Es la regla que el MUV aplicó al rechazarnos las consultas de niños, cuando
/// el documento del PACIENTE iba en el campo del médico. Lo grave no fue el
/// rechazo: las atenciones de adultos pasaban, llevando el documento del
/// paciente como si fuera el del profesional.
pub fn tipos_documento_profesional() -> &'static [(String, String)] {
    static CACHE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let archivo: ArchivoTipoDoc = serde_json::from_str(TIPO_DOC_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan");
        archivo
            .filas
            .into_iter()
            .filter(|f| f.realiza_atencion)
            .map(|f| (f.tipo, f.nombre))
            .collect()
    })
}

/// Si un tipo de documento puede ser el de quien realizó la atención.
pub fn admite_como_profesional(tipo: &str) -> bool {
    tipos_documento_profesional().iter().any(|(t, _)| t == tipo)
}

/// Los tipos de documento que el MUV admite para el USUARIO (paciente), con su
/// nombre. A diferencia de [`tipos_documento_profesional`], INCLUYE los de menor
/// y «sin identificar» (TI, RC, CN, MS, AS): un niño es paciente aunque no pueda
/// atender. Es la lista que puebla el selector de captura del paciente (#84).
pub fn tipos_documento_usuario() -> &'static [(String, String)] {
    static CACHE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let archivo: ArchivoTipoDoc = serde_json::from_str(TIPO_DOC_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan");
        archivo
            .filas
            .into_iter()
            .filter(|f| f.usuario)
            .map(|f| (f.tipo, f.nombre))
            .collect()
    })
}

/// Si un tipo de documento puede ser el del usuario (paciente) del RIPS.
pub fn admite_como_usuario(tipo: &str) -> bool {
    tipos_documento_usuario().iter().any(|(t, _)| t == tipo)
}

/// Los códigos admitidos, para el mensaje que ve quien tiene que corregirlo.
pub fn tipos_documento_profesional_listados() -> String {
    tipos_documento_profesional()
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Una finalidad de la tabla de la 948.
///
/// El `rename` va acotado a `deserialize`: **de entrada** se leen los nombres
/// del contenedor del validador (`Codigo`, `AplicaAC`), y **de salida** se
/// sirven los nuestros (`codigo`, `aplica_ac`). Sin acotarlo, la API publicaba
/// la forma del maestro de MinSalud como si fuera nuestro contrato, y la
/// pantalla tendría que conocer una convención ajena que puede cambiar sin
/// avisarnos. Lo destapó la prueba que pide `/api/finalidades` de verdad.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Finalidad {
    #[serde(rename(deserialize = "Codigo"))]
    pub codigo: String,
    #[serde(rename(deserialize = "Nombre"))]
    pub nombre: String,
    /// Si aplica al archivo de CONSULTAS. El maestro lo trae por archivo, y no
    /// son las mismas: de las 34, unas pocas solo valen para procedimientos.
    #[serde(rename(deserialize = "AplicaAC"))]
    pub aplica_ac: bool,
    /// Si aplica al archivo de PROCEDIMIENTOS.
    #[serde(rename(deserialize = "AplicaAP"))]
    pub aplica_ap: bool,
}

type FilaFinalidad = Finalidad;

#[derive(Debug, Deserialize)]
struct ArchivoFinalidad {
    filas: Vec<FilaFinalidad>,
}

/// Las 34 finalidades de la 948, ordenadas por código.
///
/// Es lo que una pantalla de captura tiene que ofrecer. Hoy **ninguna la
/// ofrece**: medido el 2026-08-02, `finalidad` no aparece en ningún formulario
/// del frontend, así que nadie ha elegido una nunca y toda historia lleva el
/// defecto. Un campo que el RIPS reporta y que nadie puede rellenar no es un
/// campo opcional: es un dato inventado con sitio reservado.
pub fn finalidades() -> &'static [Finalidad] {
    cargadas()
}

fn cargadas() -> &'static [FilaFinalidad] {
    static CACHE: OnceLock<Vec<FilaFinalidad>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut filas = serde_json::from_str::<ArchivoFinalidad>(FINALIDAD_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas;
        // El maestro NO viene ordenado —su primera fila es la 12— y esto
        // alimenta un desplegable: una lista de 34 opciones en orden arbitrario
        // se busca a ojo cada vez. Se ordena por código, que es el orden de la
        // norma y agrupa las finalidades emparentadas.
        filas.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        filas
    })
}

/// Si el código de finalidad existe en la tabla del validador.
///
/// **Existe por el `10`.** El modelo del Python (`FinalidadConsulta.NO_APLICA`)
/// y el esquema de SQLite (`finalidad TEXT NOT NULL DEFAULT '10'`) ponen `10`
/// en toda consulta donde nadie eligió una finalidad — y `10` NO ESTÁ en la
/// tabla del MUV, que va del 11 al 44 (medido el 2026-08-01 sobre
/// `REFs_FinalidadTecnologiaSalud`, 34 filas).
///
/// Como es un defecto de la base y no una ausencia, el recolector no lo veía:
/// el campo siempre venía «lleno». Aquí se convierte en lo que es —un código que
/// no existe— y sale en el informe de cierre con su motivo. No se sustituye por
/// `11`: eso sería elegir la finalidad del acto médico en nombre de quien lo
/// hizo.
pub fn finalidad_existe(codigo: &str) -> bool {
    cargadas().iter().any(|f| f.codigo == codigo)
}

// ── La finalidad CONTRA EL CUPS del servicio (#331) ──────────────────────────

const FINALIDAD_CUPS_JSON: &str = include_str!("../datos/muv_finalidad_cups.json");

#[derive(Debug, Deserialize)]
struct FilaFinalidadCups {
    #[serde(rename = "Finalidad")]
    finalidad: String,
    #[serde(rename = "CUPS")]
    cups: String,
}

#[derive(Debug, Deserialize)]
struct ArchivoFinalidadCups {
    filas: Vec<FilaFinalidadCups>,
}

/// `CUPS -> finalidades admitidas`, ordenadas y sin repetir.
fn finalidades_por_cups() -> &'static std::collections::BTreeMap<String, Vec<String>> {
    static CACHE: OnceLock<std::collections::BTreeMap<String, Vec<String>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let filas = serde_json::from_str::<ArchivoFinalidadCups>(FINALIDAD_CUPS_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas;
        let mut mapa: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for f in filas {
            mapa.entry(f.cups).or_default().push(f.finalidad);
        }
        for v in mapa.values_mut() {
            v.sort();
            v.dedup();
        }
        mapa
    })
}

/// Qué finalidades admite el MUV **para ese CUPS**.
///
/// `None` significa **«no se puede saber»**, y no «ninguna»: ese CUPS no
/// aparece en la tabla. Las dos cosas son distintas y confundirlas es todo el
/// riesgo de esta función, así que no devuelve un `Vec` vacío para el segundo
/// caso — el tipo obliga al llamante a decidir (es el mismo patrón que
/// `SIN_DUENIO` en el aislamiento, y por la misma razón).
///
/// # LA MEDICIÓN QUE DECIDE EL DISEÑO (directiva 18)
///
/// La ficha de #331 ordenaba comprobar si la tabla es exhaustiva **antes** de
/// construir nada. Medido el 2026-08-07 sobre las 990 filas:
///
/// | | |
/// |---|---|
/// | CUPS distintos en `REFs_FinalidadCups` | **308** |
/// | CUPS del catálogo del MUV (`cups.json.gz`) | **13 640** |
/// | cobertura | **2,26 %** |
/// | CUPS de la tabla ausentes del catálogo | 0 |
///
/// O sea que la tabla es **PARCIAL, y por mucho**. Tratar «no está» como
/// «rechazado» bloquearía el 97,74 % de los procedimientos legítimos: es el
/// sesgo a cerrar de la directiva 21 en su forma más cara, y la propia ficha lo
/// avisaba. Por eso el tercer estado no es un adorno defensivo — es el caso
/// mayoritario.
///
/// # Dónde SÍ aprieta, que es lo que hace que valga la pena
///
/// Donde la tabla habla, dice mucho: de los 308 CUPS cubiertos, **250 admiten
/// solo dos finalidades**. Y el código que este producto usa por defecto en la
/// consulta —`890201`— está cubierto y pasa de 34 opciones a **9**. Su vecino
/// `890202` **no** está: la cobertura es desigual incluso dentro de los códigos
/// que el prestador usa a diario, lo que descarta cualquier atajo del tipo «las
/// consultas siempre están en la tabla».
pub fn finalidades_de_cups(cups: &str) -> Option<&'static [String]> {
    finalidades_por_cups().get(cups).map(|v| v.as_slice())
}

/// Si esta pareja (finalidad, CUPS) la admite el MUV.
///
/// Tres respuestas, y la tercera es la que evita el desastre:
///
/// - `Some(true)` — la tabla cubre ese CUPS y admite esa finalidad.
/// - `Some(false)` — la tabla cubre ese CUPS y **no** admite esa finalidad.
///   Esto el MUV lo rechaza, así que se puede decir antes de enviar.
/// - `None` — ese CUPS no está en la tabla. **No se sabe, y no se supone.**
pub fn finalidad_admitida_para(finalidad: &str, cups: &str) -> Option<bool> {
    finalidades_de_cups(cups).map(|v| v.iter().any(|f| f == finalidad))
}

/// Una causa o motivo de atención de la tabla de la 948.
///
/// El maestro la trae por archivo —`AplicaAC`, `AplicaAU`, `AplicaAH`— y no por
/// AP: los procedimientos no llevan causa.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct CausaMotivo {
    #[serde(rename(deserialize = "Codigo"))]
    pub codigo: String,
    #[serde(rename(deserialize = "Nombre"))]
    pub nombre: String,
    #[serde(rename(deserialize = "AplicaAC"), deserialize_with = "si_no")]
    pub aplica_ac: bool,
    #[serde(rename(deserialize = "AplicaAU"), deserialize_with = "si_no")]
    pub aplica_au: bool,
    #[serde(rename(deserialize = "AplicaAH"), deserialize_with = "si_no")]
    pub aplica_ah: bool,
}

/// Este maestro escribe los sí/no como `"SI"` y `"NO"`, no como booleanos —al
/// revés que el de finalidad, que sí los trae `true`/`false`.
///
/// No es una manía del formato: es que cada tabla del validador se exportó como
/// venía, y suponer el tipo por el nombre de la columna es como se cuelan los
/// errores que nadie ve. Lo que NO se hace es tratar cualquier cosa distinta de
/// «SI» como `false`: un valor inesperado revienta con su texto a la vista, en
/// vez de convertir una tabla mal leída en un catálogo silenciosamente vacío.
fn si_no<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let s = String::deserialize(d)?;
    match s.trim().to_uppercase().as_str() {
        "SI" | "SÍ" | "TRUE" | "1" => Ok(true),
        "NO" | "FALSE" | "0" => Ok(false),
        otro => Err(serde::de::Error::custom(format!(
            "«{otro}» no es SI ni NO en el maestro de causa/motivo"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct ArchivoCausa {
    filas: Vec<CausaMotivo>,
}

/// Las 29 causas o motivos de atención de la 948.
///
/// Mismo hallazgo que con la finalidad, medido el 2026-08-02: el enum del
/// sistema (`CausaExterna`, trece valores del 01 al 15) es la tabla ANTERIOR, y
/// esta va del **21 al 49**. Ninguno de los trece está aquí. El defecto era el
/// `'13'` («enfermedad general»), que en la tabla nueva es el **38**.
pub fn causas_motivo() -> &'static [CausaMotivo] {
    static CACHE: OnceLock<Vec<CausaMotivo>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut filas = serde_json::from_str::<ArchivoCausa>(CAUSA_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas;
        filas.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        filas
    })
}

/// Si el código de causa o motivo existe en la tabla del validador.
pub fn causa_motivo_existe(codigo: &str) -> bool {
    causas_motivo().iter().any(|c| c.codigo == codigo)
}

// ---------------------------------------------------------------------------
// UPR — unidad mínima de dispensación (campo M13 del AM)
//
// Es EL catálogo de la lección. El 2026-07-26 aquí había un mapa escrito a mano
// —`tableta = 1`— y el 1 del catálogo es AMPOLLA; el MUV lo aceptó y devolvió
// CUV porque valida este campo por RANGO (1 a 999) y no contra la tabla. Un
// campo que el validador solo mira por rango es el sitio más fácil para que se
// cuele un dato falso sin que nadie se entere.
// ---------------------------------------------------------------------------

/// Una presentación del catálogo `REFs_UPR`.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Presentacion {
    #[serde(rename(deserialize = "Codigo"))]
    pub codigo: i64,
    #[serde(rename(deserialize = "Nombre"))]
    pub nombre: String,
}

#[derive(Debug, Deserialize)]
struct ArchivoUpr {
    filas: Vec<Presentacion>,
}

/// Las 73 presentaciones del validador, ordenadas por nombre.
pub fn presentaciones() -> &'static [Presentacion] {
    static CACHE: OnceLock<Vec<Presentacion>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut filas = serde_json::from_str::<ArchivoUpr>(UPR_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas;
        filas.sort_by(|a, b| a.nombre.cmp(&b.nombre));
        filas
    })
}

/// Mayúsculas, sin tildes y con los espacios colapsados.
///
/// El catálogo dice `TABLETA` y el médico escribe `tableta`, `Tableta` o
/// ` tableta `. Comparar en crudo dejaría fuera las tres.
fn normalizar_nombre(texto: &str) -> String {
    texto
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'ñ' | 'Ñ' => 'N',
            otro => otro.to_ascii_uppercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// El código de la presentación, o `None` si el catálogo no la nombra.
///
/// `None` **no es un fallo del que llama**: es la respuesta correcta cuando lo
/// escrito no corresponde a ninguna presentación conocida. Quien llama lo anota
/// como falta; aquí nunca se devuelve un código «parecido», que es exactamente
/// lo que produjo el dato falso de julio.
///
/// Se acepta el plural —el catálogo dice `TABLETA` y el médico escribe
/// `tabletas`— porque es una tolerancia de ESCRITURA, no una traducción por
/// parecido: `gotas` y `gotero` siguen siendo cosas distintas y solo entra la
/// que el catálogo nombra. Es la misma regla del Python
/// (`maestros_muv.presentaciones`).
pub fn codigo_presentacion(unidad: Option<&str>) -> Option<i64> {
    let n = normalizar_nombre(unidad?);
    if n.is_empty() {
        return None;
    }
    let mapa = {
        static CACHE: OnceLock<std::collections::HashMap<String, i64>> = OnceLock::new();
        CACHE.get_or_init(|| {
            let mut m = std::collections::HashMap::new();
            for p in presentaciones() {
                let nombre = normalizar_nombre(&p.nombre);
                if !nombre.ends_with('S') {
                    m.entry(format!("{nombre}S")).or_insert(p.codigo);
                }
                m.insert(nombre, p.codigo);
            }
            m
        })
    };
    mapa.get(&n).copied()
}

// ---------------------------------------------------------------------------
// CondicionDestino — condición y destino del usuario al egreso
//
// Campos R09 del AU, H12 del AH y N10 del AN. **La Resolución 948 no trae esta
// tabla**: su ficha remite a `web.sispro.gov.co`, así que el contenedor del
// validador es la única fuente que tenemos sin red.
//
// Lo que había antes en el Python era un mapa escrito a mano —`FALLECIDO: "04"`,
// `HOSPITALIZADO: "02"`— y los dos están CRUZADOS: el 02 es «paciente muerto» y
// el 04 «referido a otra institución». Medido el 2026-08-04: de los seis
// destinos que el sistema guarda, cuatro no casaban con ninguna clave del mapa
// —salían como falta, y con supuestos permitidos como «domicilio»— y los dos
// que sí casaban emitían el código equivocado. Una muerte en urgencias se
// reportaba como un traslado.
// ---------------------------------------------------------------------------

/// Una condición/destino de egreso del catálogo `REFs_CondicionDestino`.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct CondicionDestino {
    #[serde(rename(deserialize = "Codigo"))]
    pub codigo: String,
    #[serde(rename(deserialize = "Nombre"))]
    pub nombre: String,
    /// Si aplica al archivo de URGENCIAS.
    #[serde(rename(deserialize = "AplicaAU"), deserialize_with = "si_no")]
    pub aplica_au: bool,
    /// Si aplica al de HOSPITALIZACIÓN. El `03` (derivado a otro servicio) no.
    #[serde(rename(deserialize = "AplicaAH"), deserialize_with = "si_no")]
    pub aplica_ah: bool,
}

#[derive(Debug, Deserialize)]
struct ArchivoCondicionDestino {
    filas: Vec<CondicionDestino>,
}

/// Las ocho condiciones/destinos de egreso, ordenadas por código.
pub fn condiciones_destino() -> &'static [CondicionDestino] {
    static CACHE: OnceLock<Vec<CondicionDestino>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut filas = serde_json::from_str::<ArchivoCondicionDestino>(CONDICION_DESTINO_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas;
        filas.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        filas
    })
}

/// Si el código existe en el catálogo **y aplica al archivo de urgencias**.
pub fn condicion_destino_de_urgencias_existe(codigo: &str) -> bool {
    condiciones_destino()
        .iter()
        .any(|c| c.codigo == codigo && c.aplica_au)
}

/// El código de egreso que corresponde a un `destino_salida` de urgencias, o
/// `None` si el catálogo no dice ninguno.
///
/// Los cinco que devuelven algo son equivalencias **exactas** con el nombre del
/// catálogo, no parecidos:
///
/// | lo que guarda el sistema | código | nombre en el catálogo |
/// |---|---|---|
/// | `DOMICILIO`       | 01 | PACIENTE CON DESTINO A SU DOMICILIO |
/// | `FALLECIDO`       | 02 | PACIENTE MUERTO |
/// | `HOSPITALIZACION` | 03 | PACIENTE DERIVADO A OTRO SERVICIO |
/// | `REMISION`        | 04 | REFERIDO A OTRA INSTITUCION |
/// | `OBSERVACION`     | 08 | PACIENTE CONTINUA EN EL SERVICIO (CORTE FACTURACION) |
///
/// **`FUGA` devuelve `None` a propósito, y no es un olvido**: el catálogo no
/// tiene ningún código para «el paciente se fue sin que nadie le diera salida».
/// El más cercano por su forma sería el `01`, y el `01` afirma que el paciente
/// salió con destino a su domicilio, o sea que hubo un alta. Elegirlo
/// convertiría un abandono del servicio —que tiene consecuencias clínicas y
/// legales— en una atención terminada con normalidad. Se anota como falta y se
/// le pregunta al prestador qué reporta hoy en ese caso.
pub fn condicion_destino_de_urgencias(destino: Option<&str>) -> Option<&'static str> {
    match destino? {
        "DOMICILIO" => Some("01"),
        "FALLECIDO" => Some("02"),
        "HOSPITALIZACION" => Some("03"),
        "REMISION" => Some("04"),
        "OBSERVACION" => Some("08"),
        _ => None,
    }
}

/// Si el código existe en el catálogo **y aplica al archivo de hospitalización**.
///
/// La columna importa: el `03` («paciente derivado a otro servicio») vale para
/// urgencias y **no** para hospitalización, y el Python lo emitía ahí.
pub fn condicion_destino_de_hospitalizacion_existe(codigo: &str) -> bool {
    condiciones_destino()
        .iter()
        .any(|c| c.codigo == codigo && c.aplica_ah)
}

/// El código de egreso que corresponde al estado con que se cierra una
/// hospitalización, o `None` si el catálogo no dice ninguno.
///
/// | estado del sistema | código | nombre en el catálogo |
/// |---|---|---|
/// | `ALTA_MEDICA`     | 01 | PACIENTE CON DESTINO A SU DOMICILIO |
/// | `ALTA_VOLUNTARIA` | 01 | PACIENTE CON DESTINO A SU DOMICILIO |
/// | `REMISION`        | 04 | REFERIDO A OTRA INSTITUCION |
/// | `DEFUNCION`       | 02 | PACIENTE MUERTO |
///
/// El mapa del Python decía `DEFUNCION = "04"` —que es el traslado— y
/// `REMISION = "03"`, que **no aplica al AH**. `ACTIVA` no está: una
/// hospitalización abierta no se reporta, y el generador la descarta antes.
pub fn condicion_destino_de_hospitalizacion(estado: Option<&str>) -> Option<&'static str> {
    match estado? {
        "ALTA_MEDICA" | "ALTA_VOLUNTARIA" => Some("01"),
        "DEFUNCION" => Some("02"),
        "REMISION" => Some("04"),
        _ => None,
    }
}

/// Una vía de ingreso al servicio de salud (`REFs_ViaIngresoServicioSalud`).
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct ViaIngreso {
    #[serde(rename(deserialize = "Codigo"))]
    pub codigo: String,
    #[serde(rename(deserialize = "Nombre"))]
    pub nombre: String,
    #[serde(rename(deserialize = "AplicaAP"), deserialize_with = "si_no")]
    pub aplica_ap: bool,
    #[serde(rename(deserialize = "AplicaAH"), deserialize_with = "si_no")]
    pub aplica_ah: bool,
}

#[derive(Debug, Deserialize)]
struct ArchivoViaIngreso {
    filas: Vec<ViaIngreso>,
}

/// Las catorce vías de ingreso, ordenadas por código.
pub fn vias_ingreso() -> &'static [ViaIngreso] {
    static CACHE: OnceLock<Vec<ViaIngreso>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut filas = serde_json::from_str::<ArchivoViaIngreso>(VIA_INGRESO_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas;
        filas.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        filas
    })
}

/// Si el código existe y aplica al archivo de hospitalización.
pub fn via_ingreso_de_hospitalizacion_existe(codigo: &str) -> bool {
    vias_ingreso()
        .iter()
        .any(|v| v.codigo == codigo && v.aplica_ah)
}

/// El código de vía de ingreso que corresponde a cómo entró el paciente.
///
/// | vía del sistema | código | nombre en el catálogo |
/// |---|---|---|
/// | `URGENCIAS`  | 03 | DERIVADO DE URGENCIAS |
/// | `PROGRAMADA` | 02 | DERIVADO DE CONSULTA EXTERNA |
/// | `REMISION`   | 13 | REFERIDO DE OTRA INSTITUCION |
///
/// **El mapa del Python tenía las dos primeras INTERCAMBIADAS** —`URGENCIAS`
/// daba el 02 y `PROGRAMADA` el 03—, así que quien entró por urgencias se
/// reportaba como programado y al revés. Y `REMISION` daba el `04` («derivado de
/// hospitalización»), que el catálogo marca como **no aplicable al AH**: el MUV
/// lo rechaza.
///
/// `PROGRAMADA` no tiene nombre propio en el catálogo; se traduce al 02 porque
/// un ingreso programado se origina en la consulta externa. Es una derivación,
/// no un parecido: las otras trece filas nombran orígenes que este sistema no
/// distingue (sala de partos, telemedicina, jornada de salud).
pub fn via_ingreso_de_hospitalizacion(via: Option<&str>) -> Option<&'static str> {
    match via? {
        "URGENCIAS" => Some("03"),
        "PROGRAMADA" => Some("02"),
        "REMISION" => Some("13"),
        _ => None,
    }
}

/// Las finalidades que el maestro marca como aplicables al archivo de consultas.
pub fn finalidades_de_consulta() -> Vec<&'static str> {
    cargadas()
        .iter()
        .filter(|f| f.aplica_ac)
        .map(|f| f.codigo.as_str())
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn son_los_ocho_que_el_muv_nombro_al_rechazarnos() {
        // El mensaje literal del MUV: «valores permitidos CC,CE,CD,PA,SC,PE,DE,PT».
        let mut codigos: Vec<&str> = tipos_documento_profesional()
            .iter()
            .map(|(t, _)| t.as_str())
            .collect();
        codigos.sort_unstable();
        assert_eq!(codigos, ["CC", "CD", "CE", "DE", "PA", "PE", "PT", "SC"]);
    }

    #[test]
    fn los_de_menor_no_atienden_a_nadie() {
        // RC (registro civil) y TI (tarjeta de identidad) SÍ están en el maestro
        // —un paciente puede tenerlos— pero no como quien realiza la atención.
        // Es justo el caso que destapó el fallo.
        assert!(!admite_como_profesional("RC"));
        assert!(!admite_como_profesional("TI"));
        assert!(admite_como_profesional("CC"));
    }

    #[test]
    fn el_filtro_descarta_de_verdad_algunas_filas() {
        // Si `RealizaAtencionRips` dejara de leerse, pasarían las trece y esta
        // prueba lo dice. Sin ella, un filtro que no filtra se ve igual de verde.
        let archivo: ArchivoTipoDoc = serde_json::from_str(TIPO_DOC_JSON).unwrap();
        assert_eq!(archivo.filas.len(), 13);
        assert_eq!(tipos_documento_profesional().len(), 8);
    }

    #[test]
    fn el_usuario_admite_las_trece_incluidas_las_de_menor() {
        // El paciente puede ser un menor o alguien sin identificar, así que su
        // lista es la de `UsuarioRips` (las trece), NO la del profesional (ocho).
        assert_eq!(tipos_documento_usuario().len(), 13, "todas las de UsuarioRips");
        // Justo las que el profesional excluye tienen que ESTAR aquí — es lo que
        // distingue las dos listas y evita reusar la del profesional por error.
        for t in ["RC", "TI", "CN", "MS", "AS"] {
            assert!(admite_como_usuario(t), "{t} es paciente válido");
            assert!(!admite_como_profesional(t), "{t} NO atiende: la pareja que discrimina");
        }
        assert!(admite_como_usuario("CC"));
        assert!(!admite_como_usuario("ZZ"), "un código que no está no se admite");
    }

    #[test]
    fn el_diez_de_finalidad_no_existe_en_la_tabla_del_muv() {
        // La razón entera de que `finalidad_existe` exista. El defecto de la
        // base pone `10` en toda consulta sin finalidad elegida, y `10` no está.
        assert!(!finalidad_existe("10"));
        assert!(finalidad_existe("11"));
        assert!(finalidad_existe("44"));
        assert!(!finalidad_existe("45"));
        assert!(!finalidad_existe(""));
    }

    #[test]
    fn la_tableta_no_es_el_uno_y_el_uno_es_la_ampolla() {
        // La lección de julio, escrita como prueba. El mapa inventado decía
        // `tableta = 1`; el catálogo dice que el 1 es AMPOLLA.
        let tableta = codigo_presentacion(Some("tableta")).expect("está en el catálogo");
        assert_ne!(tableta, 1, "el 1 es AMPOLLA, no TABLETA");
        assert_eq!(codigo_presentacion(Some("ampolla")), Some(1));
        // Y el valor sale del ARTEFACTO, no de una constante escrita aquí: se
        // busca en el maestro empotrado por su nombre.
        let del_maestro = presentaciones()
            .iter()
            .find(|p| p.nombre.eq_ignore_ascii_case("TABLETA"))
            .expect("TABLETA está en REFs_UPR")
            .codigo;
        assert_eq!(tableta, del_maestro);
    }

    #[test]
    fn la_presentacion_tolera_la_escritura_pero_no_se_parece_a_nada() {
        // Lo que SÍ ve: mayúsculas, plural, espacios de sobra, tildes.
        let t = codigo_presentacion(Some("tableta"));
        assert_eq!(codigo_presentacion(Some("TABLETAS")), t);
        assert_eq!(codigo_presentacion(Some("  Tableta  ")), t);
        // Y lo que NO: una unidad que el catálogo no nombra no se aproxima a la
        // más parecida. `None` es la respuesta correcta, y quien llama la anota.
        assert_eq!(codigo_presentacion(Some("unidad")), None);
        assert_eq!(codigo_presentacion(Some("pastillita")), None);
        assert_eq!(codigo_presentacion(None), None);
        assert_eq!(codigo_presentacion(Some("   ")), None);
    }

    #[test]
    fn el_catalogo_de_presentaciones_esta_completo() {
        // Positivo de control: si el JSON no se leyera, todo lo de arriba daría
        // `None` y las pruebas de «no se parece a nada» pasarían igual.
        assert_eq!(presentaciones().len(), 73);
    }

    #[test]
    fn el_muerto_es_el_02_y_no_el_04() {
        // El mapa escrito a mano del Python decía FALLECIDO=04 y
        // HOSPITALIZADO=02, cruzados. Los códigos se leen del ARTEFACTO —el
        // maestro empotrado— y no de constantes escritas aquí, que es lo que
        // haría que la prueba se diera la razón a sí misma.
        let nombre_de = |c: &str| {
            condiciones_destino()
                .iter()
                .find(|x| x.codigo == c)
                .map(|x| x.nombre.as_str())
                .unwrap_or("(no existe)")
        };
        assert_eq!(
            condicion_destino_de_urgencias(Some("FALLECIDO")),
            Some("02")
        );
        assert!(nombre_de("02").contains("MUERTO"), "{}", nombre_de("02"));
        assert!(
            nombre_de("04").contains("REFERIDO A OTRA"),
            "el 04 es un traslado, no una muerte: {}",
            nombre_de("04")
        );
    }

    #[test]
    fn la_fuga_no_se_reporta_como_un_alta() {
        // La pareja: cinco destinos tienen código exacto y uno NO tiene ninguno.
        for (destino, codigo) in [
            ("DOMICILIO", "01"),
            ("FALLECIDO", "02"),
            ("HOSPITALIZACION", "03"),
            ("REMISION", "04"),
            ("OBSERVACION", "08"),
        ] {
            assert_eq!(
                condicion_destino_de_urgencias(Some(destino)),
                Some(codigo),
                "{destino}"
            );
            assert!(
                condicion_destino_de_urgencias_existe(codigo),
                "{codigo} tiene que estar en el catálogo y aplicar al AU"
            );
        }
        assert_eq!(
            condicion_destino_de_urgencias(Some("FUGA")),
            None,
            "el catálogo no tiene código para una fuga, y el 01 afirmaría un alta"
        );
        assert_eq!(condicion_destino_de_urgencias(None), None);
        assert_eq!(condicion_destino_de_urgencias(Some("LO QUE SEA")), None);
    }

    #[test]
    fn el_catalogo_de_condicion_destino_esta_completo_y_discrimina_por_archivo() {
        // Positivo de control + que la columna AplicaAH se lee de verdad: el 03
        // (derivado a otro servicio) aplica al AU y NO al AH.
        assert_eq!(condiciones_destino().len(), 8);
        let tres = condiciones_destino().iter().find(|c| c.codigo == "03").unwrap();
        assert!(tres.aplica_au);
        assert!(!tres.aplica_ah, "el 03 no aplica a hospitalización");
        assert!(!condicion_destino_de_urgencias_existe("99"));
    }

    #[test]
    fn la_via_de_ingreso_no_esta_intercambiada() {
        // El mapa del Python daba el 02 a URGENCIAS y el 03 a PROGRAMADA, que es
        // exactamente al revés. Los nombres se leen del ARTEFACTO.
        let nombre_de = |c: &str| {
            vias_ingreso()
                .iter()
                .find(|x| x.codigo == c)
                .map(|x| x.nombre.as_str())
                .unwrap_or("(no existe)")
        };
        assert_eq!(via_ingreso_de_hospitalizacion(Some("URGENCIAS")), Some("03"));
        assert!(nombre_de("03").contains("URGENCIAS"), "{}", nombre_de("03"));
        assert_eq!(via_ingreso_de_hospitalizacion(Some("PROGRAMADA")), Some("02"));
        assert!(nombre_de("02").contains("CONSULTA EXTERNA"), "{}", nombre_de("02"));
    }

    #[test]
    fn ninguna_via_ni_condicion_del_ah_usa_un_codigo_que_no_le_aplica() {
        // El Python emitía el 04 de vía («derivado de hospitalización») y el 03
        // de condición («derivado a otro servicio»), y el catálogo marca los dos
        // como NO aplicables al AH. El MUV rechaza el archivo por eso.
        for via in ["URGENCIAS", "PROGRAMADA", "REMISION"] {
            let c = via_ingreso_de_hospitalizacion(Some(via)).expect(via);
            assert!(
                via_ingreso_de_hospitalizacion_existe(c),
                "«{via}» da el {c}, que no aplica al AH"
            );
        }
        for estado in ["ALTA_MEDICA", "ALTA_VOLUNTARIA", "REMISION", "DEFUNCION"] {
            let c = condicion_destino_de_hospitalizacion(Some(estado)).expect(estado);
            assert!(
                condicion_destino_de_hospitalizacion_existe(c),
                "«{estado}» da el {c}, que no aplica al AH"
            );
        }
        // Y el caso rojo: los dos códigos que el Python usaba NO aplican.
        assert!(!via_ingreso_de_hospitalizacion_existe("04"));
        assert!(!condicion_destino_de_hospitalizacion_existe("03"));
    }

    #[test]
    fn el_muerto_del_ah_tambien_es_el_02() {
        assert_eq!(condicion_destino_de_hospitalizacion(Some("DEFUNCION")), Some("02"));
        assert_eq!(condicion_destino_de_hospitalizacion(Some("REMISION")), Some("04"));
        // ACTIVA no se traduce: una hospitalización abierta no se reporta, y el
        // Python le daba un «00» que no existe en ninguna tabla.
        assert_eq!(condicion_destino_de_hospitalizacion(Some("ACTIVA")), None);
        assert!(!condiciones_destino().iter().any(|c| c.codigo == "00"));
    }

    #[test]
    fn el_catalogo_de_vias_esta_completo_y_discrimina_por_archivo() {
        assert_eq!(vias_ingreso().len(), 14);
        let uno = vias_ingreso().iter().find(|v| v.codigo == "01").unwrap();
        assert!(uno.aplica_ap);
        assert!(!uno.aplica_ah, "demanda espontánea no aplica a hospitalización");
    }

    #[test]
    fn la_tabla_de_finalidad_va_del_11_al_44() {
        let mut c: Vec<&str> = finalidades().iter().map(|f| f.codigo.as_str()).collect();
        c.sort_unstable();
        assert_eq!(c.len(), 34);
        assert_eq!(*c.first().unwrap(), "11");
        assert_eq!(*c.last().unwrap(), "44");
    }

    #[test]
    fn el_trece_de_causa_tampoco_existe_en_la_tabla_del_muv() {
        // El mismo defecto que el «10», en el campo de al lado: el enum del
        // sistema es la tabla anterior entera.
        assert!(!causa_motivo_existe("13"), "«13» era el defecto de causa_externa");
        assert!(!causa_motivo_existe("01"));
        assert!(!causa_motivo_existe("15"));
        // La tabla nueva empieza en 21 y «enfermedad general» es el 38.
        assert!(causa_motivo_existe("21"));
        assert!(causa_motivo_existe("38"));
        assert!(causa_motivo_existe("49"));
        assert!(!causa_motivo_existe("50"));
        assert_eq!(causas_motivo().len(), 29);
    }

    #[test]
    fn los_catalogos_salen_ordenados_por_codigo() {
        // Alimentan desplegables. El maestro no viene ordenado —su primera fila
        // es la 12— y una lista de 34 opciones en orden arbitrario obliga a
        // buscar a ojo en cada consulta.
        let f: Vec<&str> = finalidades().iter().map(|x| x.codigo.as_str()).collect();
        let mut ordenada = f.clone();
        ordenada.sort_unstable();
        assert_eq!(f, ordenada);
        assert_eq!(f[0], "11");

        let c: Vec<&str> = causas_motivo().iter().map(|x| x.codigo.as_str()).collect();
        let mut ordenada = c.clone();
        ordenada.sort_unstable();
        assert_eq!(c, ordenada);
        assert_eq!(c[0], "21");
    }

    #[test]
    fn el_si_no_de_texto_se_lee_y_discrimina() {
        // Este maestro trae «SI»/«NO» y no booleanos. Si `si_no` devolviera
        // siempre `true`, las 29 aplicarían a todo y esto lo dice: hay dos que
        // NO valen para urgencias ni hospitalización.
        let no_urgencias = causas_motivo().iter().filter(|c| !c.aplica_au).count();
        assert_eq!(no_urgencias, 2, "el «NO» se está leyendo como «SI»");
        assert_eq!(causas_motivo().iter().filter(|c| !c.aplica_ah).count(), 2);
        assert!(causas_motivo().iter().all(|c| c.aplica_ac), "todas valen para AC");
    }

    #[test]
    fn enfermedad_general_es_el_38_y_se_lee_del_maestro() {
        // El nombre no se escribe aquí: se comprueba que el maestro lo trae. Si
        // un día la tabla lo moviera, esto lo dice en vez de dejar un 38 escrito
        // a mano en el código.
        let c = causas_motivo().iter().find(|c| c.codigo == "38").unwrap();
        assert_eq!(c.nombre, "ENFERMEDAD GENERAL");
        assert!(c.aplica_ac, "tiene que valer para consultas");
    }

    #[test]
    fn no_todas_las_finalidades_aplican_a_consultas() {
        // Si `AplicaAC` dejara de leerse, saldrían las 34 y esto lo dice.
        let de_consulta = finalidades_de_consulta();
        assert!(!de_consulta.is_empty());
        assert!(de_consulta.len() < finalidades().len(),
                "el filtro AplicaAC no está filtrando nada");
    }

    #[test]
    fn lo_desconocido_no_se_admite() {
        assert!(!admite_como_profesional(""));
        assert!(!admite_como_profesional("XX"));
        // Y no se acepta por parecido: el MUV compara la cadena exacta.
        assert!(!admite_como_profesional("cc"));
    }

    // ── La finalidad contra el CUPS (#331) ───────────────────────────────────

    /// **Las tres respuestas se distinguen**, que es la prueba entera. Con dos
    /// —«admitida» / «no admitida»— el 97,74 % de los CUPS caería en la segunda
    /// y el producto rechazaría procedimientos legítimos.
    #[test]
    fn la_pareja_finalidad_cups_tiene_tres_respuestas() {
        // Cubierto y admitido: 890201 admite la 11 (ver la medición del doc).
        assert_eq!(finalidad_admitida_para("11", "890201"), Some(true));
        // Cubierto y NO admitido: la 12 no está entre las nueve de 890201.
        assert_eq!(finalidad_admitida_para("12", "890201"), Some(false));
        // NO CUBIERTO: no se sabe. **No es `Some(false)`**, y ésa es la línea
        // que impide bloquear el resto del catálogo.
        assert_eq!(finalidad_admitida_para("11", "890202"), None);
        assert_eq!(finalidades_de_cups("890202"), None);
    }

    /// La medición que justifica el tercer estado, convertida en prueba.
    ///
    /// Si un día la tabla creciera hasta cubrir el catálogo entero, esto se
    /// pondría rojo y habría que revisar la decisión — que es justo lo que se
    /// quiere: la regla depende de un hecho medido, y el hecho puede cambiar.
    #[test]
    fn la_tabla_es_parcial_y_por_eso_no_se_puede_rechazar_lo_ausente() {
        let cubiertos = finalidades_por_cups().len();
        assert_eq!(cubiertos, 308, "cambió la cobertura de la tabla: revisa la regla");
        assert!(
            cubiertos < 1000,
            "la tabla pasó a cubrir {cubiertos} CUPS: si ya es exhaustiva, «no está» \
             podría significar «rechazado» y esta decisión hay que retomarla"
        );
    }

    /// Donde la tabla habla, aprieta: es lo que hace que la pantalla gane algo.
    #[test]
    fn para_un_cups_cubierto_la_lista_se_acorta_de_verdad() {
        let nueve = finalidades_de_cups("890201").expect("890201 está cubierto");
        assert_eq!(nueve.len(), 9, "890201 admitía nueve finalidades");
        assert!(nueve.len() < finalidades().len(), "no acorta nada respecto de las 34");
        // Ordenadas y sin repetir: alimenta un desplegable.
        let mut ord = nueve.to_vec();
        ord.sort();
        ord.dedup();
        assert_eq!(ord.as_slice(), nueve);
    }

    /// Todo lo que la tabla nombra existe en los OTROS maestros. Sin esto, una
    /// tabla desincronizada ofrecería finalidades que el validador no conoce.
    #[test]
    fn la_tabla_no_nombra_finalidades_que_no_existan() {
        let malas: Vec<&str> = finalidades_por_cups()
            .values()
            .flatten()
            .filter(|f| !finalidad_existe(f))
            .map(|s| s.as_str())
            .collect();
        assert!(malas.is_empty(), "finalidades que no están en el maestro: {malas:?}");
    }

    /// El control de que el maestro se cargó —sin él, un JSON vacío haría pasar
    /// en verde todo lo de arriba salvo el conteo— **y el número real**.
    ///
    /// La ficha de #331 habla de «990 filas», y son 990 filas. Pero solo **890
    /// parejas distintas**: el maestro trae 91 parejas repetidas, 100 filas de
    /// más, cada una con su `Id` propio (los 990 `Id` son únicos, así que no es
    /// un error de extracción nuestro — viene así del contenedor del validador).
    ///
    /// Se deduplica, y por eso el conteo que se afirma aquí es 890 y no 990. Da
    /// igual para decidir, porque una pareja repetida admite lo mismo dos veces;
    /// se escribe para que el día que alguien compare este número con la ficha
    /// no crea que se perdieron cien filas por el camino.
    #[test]
    fn el_maestro_de_parejas_se_carga_entero_y_deduplicado() {
        let total: usize = finalidades_por_cups().values().map(|v| v.len()).sum();
        assert_eq!(total, 890, "el maestro trae 890 parejas distintas de sus 990 filas");
    }
}
