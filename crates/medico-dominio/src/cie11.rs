// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Derivación CIE-10 → CIE-11 para los 22 campos que pide la Resolución 948
//! de 2026 (#250).
//!
//! Es el port de `backend/cie11.py`, y va antes que los documentos del RIPS
//! porque los cinco lo necesitan: cada diagnóstico que se emite arrastra su par
//! `codXxxCIE11` / `nomCodXxxCIE11`.
//!
//! # Lo que NO hace, y es lo importante
//!
//! **Solo deriva cuando la equivalencia es exacta y única.** Las otras tres
//! situaciones dejan el campo vacío, y la 948 lo admite —sus propios ejemplos
//! traen `""` y `null` ahí, porque el periodo paralelo no obliga a tener el
//! CIE-11 todavía—:
//!
//! | caso | qué pasa |
//! |---|---|
//! | equivalencia exacta y única | se deriva (8 070 de 12 406 códigos) |
//! | varios destinos (1 402 códigos, hasta 35) | vacío; [`candidatos`] los ofrece todos |
//! | equivalencia por CATEGORÍA de tres caracteres | vacío: es menos específica |
//! | sin equivalencia | vacío |
//!
//! Poner el primero de una lista de 35 tiene la forma correcta, pasa el
//! validador y entra como dato clínico falso: nadie lo revisa después porque
//! nada falló. Y derivar por categoría rebaja en silencio la especificidad de
//! lo que el médico escribió — un `C832` no es un `C83`.
//!
//! # De dónde salen los datos
//!
//! `datos/mapeo_cie10_cie11.json`, empotrado en el binario con `include_str!`
//! igual que CIE-10, CUPS, CUM y DIVIPOLA, y por lo mismo: un consultorio rural
//! no puede depender de que haya internet el día del cierre de mes.
//!
//! Lo escribe `herramientas/generar_mapeo_cie11.py` **en la misma corrida** que
//! el del Python, desde las tablas de la OMS. No es una copia mantenida a mano:
//! si lo fuera, el día que divergieran, el Python y el binario derivarían
//! CIE-11 distintos para el mismo diagnóstico sin que nada fallara.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const MAPEO_JSON: &str = include_str!("../datos/mapeo_cie10_cie11.json");

/// Un candidato CIE-11: el código y su título en la clasificación.
///
/// La URI de la OMS y el capítulo NO se traen: no entran en ningún campo de la
/// 948 y son la mitad del peso del archivo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Destino {
    pub codigo: String,
    pub titulo: String,
}

/// Cómo se llegó al destino. Decide si se puede poner sin preguntar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Nivel {
    /// La tabla de la OMS trae ese código exacto.
    Exacto,
    /// Lo que se encontró fue su categoría de tres caracteres.
    Categoria,
}

#[derive(Debug, Deserialize)]
struct Entrada {
    nivel: Nivel,
    #[serde(default)]
    via: Option<String>,
    destinos: Vec<Destino>,
}

#[derive(Debug, Deserialize)]
struct Archivo {
    release: String,
    version_tabla: String,
    mapa: HashMap<String, Entrada>,
}

/// Lo que se encontró para un CIE-10, con el porqué a la vista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equivalencia<'a> {
    /// El código ya normalizado con el que se encontró — puede no ser el que
    /// entró: `A09` se busca como `A09X`.
    pub cie10: String,
    pub nivel: Nivel,
    /// La categoría por la que se llegó, cuando `nivel` es [`Nivel::Categoria`].
    pub via: Option<&'a str>,
    pub destinos: &'a [Destino],
}

impl Equivalencia<'_> {
    /// Más de un destino posible: nadie puede elegir por el médico.
    pub fn ambigua(&self) -> bool {
        self.destinos.len() > 1
    }

    /// Si se puede poner sin preguntar. Exige **las dos** cosas.
    pub fn derivable(&self) -> bool {
        self.nivel == Nivel::Exacto && self.destinos.len() == 1
    }
}

fn cargado() -> &'static Archivo {
    static CACHE: OnceLock<Archivo> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str(MAPEO_JSON)
            .expect("el mapeo va empotrado y las pruebas lo validan")
    })
}

/// La versión de la CIE-11 contra la que se generó el mapeo.
pub fn release() -> &'static str {
    &cargado().release
}

/// La fecha de las tablas de la OMS de las que salió.
pub fn version_tabla() -> &'static str {
    &cargado().version_tabla
}

/// Cuántos CIE-10 tienen alguna equivalencia.
pub fn total_mapeados() -> usize {
    cargado().mapa.len()
}

/// Deja el código en la forma del RIPS: sin punto, en mayúsculas, sin espacios.
///
/// Acepta lo que llegue de fuera —`a00.0`, `A00.0`— porque un código copiado de
/// una remisión en papel viene como venga. Lo que NO hace es completar a cuatro
/// con `X` por su cuenta: eso solo vale para las categorías sin subdivisión, y
/// decidirlo aquí, sin la tabla delante, fabricaría códigos inexistentes.
pub fn normalizar(cie10: Option<&str>) -> String {
    match cie10 {
        None => String::new(),
        Some(c) => c
            .trim()
            .chars()
            .filter(|c| *c != '.')
            .flat_map(char::to_uppercase)
            .collect(),
    }
}

/// La equivalencia, o `None` si el código no está mapeado.
pub fn buscar(cie10: Option<&str>) -> Option<Equivalencia<'static>> {
    let codigo = normalizar(cie10);
    if codigo.is_empty() {
        return None;
    }
    let mapa = &cargado().mapa;
    let (codigo, entrada) = match mapa.get(&codigo) {
        Some(e) => (codigo, e),
        // Una categoría sin subdivisión se escribe `A09` fuera del RIPS y `A09X`
        // dentro. Se acepta la forma corta al consultar.
        None if codigo.len() == 3 => {
            let con_x = format!("{codigo}X");
            let e = mapa.get(&con_x)?;
            (con_x, e)
        }
        None => return None,
    };
    Some(Equivalencia {
        cie10: codigo,
        nivel: entrada.nivel,
        via: entrada.via.as_deref(),
        destinos: &entrada.destinos,
    })
}

/// Los CIE-11 posibles, para que un formulario los ofrezca. Vacío si no hay.
pub fn candidatos(cie10: Option<&str>) -> &'static [Destino] {
    match buscar(cie10) {
        Some(eq) => eq.destinos,
        None => &[],
    }
}

/// El CIE-11 cuando la equivalencia es exacta y única. Si no, `None`.
pub fn derivar(cie10: Option<&str>) -> Option<&'static Destino> {
    let eq = buscar(cie10)?;
    eq.derivable().then(|| &eq.destinos[0])
}

/// El par `(código, título)` que la 948 pide junto a cada CIE-10, o dos cadenas
/// vacías cuando no se puede derivar.
///
/// Es el `_par_cie11` del Python, y se emite **desde el CIE-10 de origen**, no
/// desde el que acabe saliendo: si el diagnóstico falta y entrara un supuesto,
/// derivar de él daría un CIE-11 para un diagnóstico que nadie hizo.
pub fn par(cie10: Option<&str>) -> (&'static str, &'static str) {
    match derivar(cie10) {
        Some(d) => (&d.codigo, &d.titulo),
        None => ("", ""),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mapeo_empotrado_se_parsea_y_trae_lo_esperado() {
        // Si el archivo se regenera y encoge a la mitad, esto lo dice. El número
        // exacto no se fija: la tabla de la OMS crece entre versiones.
        assert!(total_mapeados() > 11_500, "solo {}", total_mapeados());
        assert!(release().starts_with("http://id.who.int/icd/release/11/"));
        assert_eq!(version_tabla(), "2025-01-24");
    }

    #[test]
    fn exacto_y_unico_se_deriva() {
        let d = derivar(Some("A000")).expect("A000 es exacto y único");
        assert_eq!(d.codigo, "1A00&XN8P1&XN8P1");
        assert!(d.titulo.starts_with("Cholera due to Vibrio cholerae O1"));
        assert_eq!(par(Some("A000")).0, "1A00&XN8P1&XN8P1");
    }

    #[test]
    fn ambiguo_no_se_deriva_pero_se_ofrecen_todos() {
        // A022 (salmonelosis con artritis) tiene más de un destino. Elegir el
        // primero sería inventar un diagnóstico con la forma correcta.
        let eq = buscar(Some("A022")).expect("A022 está mapeado");
        assert!(eq.ambigua(), "A022 dejó de ser ambiguo: revisar la prueba");
        assert_eq!(eq.nivel, Nivel::Exacto, "es exacto Y ambiguo a la vez");
        assert!(!eq.derivable());
        assert_eq!(derivar(Some("A022")), None);
        assert_eq!(par(Some("A022")), ("", ""));
        assert!(candidatos(Some("A022")).len() > 1);
    }

    #[test]
    fn por_categoria_no_se_deriva_aunque_el_destino_sea_unico() {
        // C832 llega por su categoría C83. El destino es único, y aun así no se
        // pone: `2A8Z` es «linfoma de células B, no especificado» y el médico
        // escribió uno que SÍ especifica.
        let eq = buscar(Some("C832")).expect("C832 está mapeado");
        assert_eq!(eq.nivel, Nivel::Categoria);
        assert_eq!(eq.via, Some("C83"));
        assert_eq!(eq.destinos.len(), 1, "único destino, y aun así no deriva");
        assert!(!eq.derivable());
        assert_eq!(derivar(Some("C832")), None);
    }

    #[test]
    fn sin_equivalencia_no_inventa() {
        assert_eq!(derivar(Some("ZZZ9")), None);
        assert_eq!(buscar(Some("ZZZ9")), None);
        assert_eq!(candidatos(Some("ZZZ9")).len(), 0);
        assert_eq!(par(Some("ZZZ9")), ("", ""));
    }

    #[test]
    fn la_ausencia_no_es_un_codigo() {
        assert_eq!(derivar(None), None);
        assert_eq!(derivar(Some("")), None);
        assert_eq!(derivar(Some("   ")), None);
        assert_eq!(par(None), ("", ""));
    }

    #[test]
    fn normaliza_lo_que_llega_de_fuera() {
        assert_eq!(normalizar(Some("a00.0")), "A000");
        assert_eq!(normalizar(Some(" A00.0 ")), "A000");
        assert_eq!(normalizar(Some("A000")), "A000");
        assert_eq!(normalizar(None), "");
        // Y encontrarlo no depende de la forma en que venga escrito.
        assert_eq!(derivar(Some("a00.0")), derivar(Some("A000")));
    }

    #[test]
    fn la_categoria_corta_se_busca_con_la_x() {
        // `A09` fuera del RIPS es `A09X` dentro. Se acepta la forma corta, y la
        // equivalencia devuelve el código con el que de verdad se encontró.
        let eq = buscar(Some("A09")).expect("A09 se busca como A09X");
        assert_eq!(eq.cie10, "A09X");
    }

    #[test]
    fn no_completa_a_cuatro_por_su_cuenta() {
        // Lo contrario de la prueba anterior, y es lo que la hace discriminar:
        // la X solo se prueba porque la tabla TIENE `A09X`. Un código de tres
        // que no exista ni con X ni sin ella no se convierte en nada.
        assert_eq!(buscar(Some("ZZZ")), None);
    }

    #[test]
    fn el_par_sale_del_mapeo_y_no_de_un_titulo_escrito_a_mano() {
        // El `nomCod*CIE11` no se pide aparte ni se inventa: viene del propio
        // destino, así que no puede decir una cosa distinta del código.
        let d = derivar(Some("A000")).unwrap();
        assert_eq!(par(Some("A000")), (d.codigo.as_str(), d.titulo.as_str()));
    }
}
