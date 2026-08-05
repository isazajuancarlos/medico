// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

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
}
