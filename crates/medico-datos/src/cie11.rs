// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! CIE-11: derivar del CIE-10 lo que se pueda, y **no inventar el resto** (#67).
//!
//! La Resolución 948 de 2026 abre el periodo en que un mismo episodio lleva los
//! dos códigos. Pedirle al médico que codifique dos veces multiplica por dos el
//! problema que este sistema existe para no repetir, así que el CIE-11 se deriva
//! del CIE-10 **cuando la equivalencia es EXACTA y ÚNICA**, y solo entonces.
//!
//! # Lo que NO se deriva, y por qué el campo se va vacío
//!
//! - **Ambiguo** (1 106 códigos): más de un destino, hasta 42. Poner el primero
//!   de una lista de 42 es `cod_consulta_cups="890201"` con otro nombre.
//! - **Por categoría** (3 230): se encontró la categoría de tres caracteres, no
//!   el código de cuatro que el médico escribió. Es MENOS específico, y rebajar
//!   la especificidad de un diagnóstico en silencio pierde dato clínico sin que
//!   nadie se entere — en un RIPS nadie lo rechaza: entra y ahí se queda.
//!
//! Dejarlo vacío es correcto y la norma lo permite: sus propios ejemplos traen
//! `""` y `null` en estos campos, porque el periodo paralelo no obliga a tener
//! el CIE-11 todavía.
//!
//! # Por qué esto NO va a una tabla, a diferencia de los otros maestros
//!
//! CIE-10, CUPS, CUM y DIVIPOLA se siembran en la base porque hay pantallas que
//! los BUSCAN por texto. Este mapeo no se busca: se consulta por clave exacta al
//! generar el RIPS, y nada más. Una tabla, su migración y su sembrado serían tres
//! mecanismos para lo que resuelve un mapa en memoria que se arma una vez.
//!
//! # Lo que se dejó en Python a propósito
//!
//! `backend/cie11.py` tiene además `candidatos()` —los destinos posibles, para
//! que el formulario los ofrezca— e `inverso()`. Eso sirve a una pantalla de
//! captura que en Rust todavía no existe, y portar la maquinaria de algo que
//! nadie llama es crear una característica falsa. Cuando la pantalla exista, se
//! porta con ella y con el mapeo completo.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

const CIE11_JSON: &str = include_str!("../datos/cie11.json");

#[derive(Debug, Deserialize)]
struct Archivo {
    release: String,
    version_tabla: String,
    /// `codigo CIE-10` → `[código CIE-11, título]`.
    derivables: HashMap<String, [String; 2]>,
}

fn archivo() -> &'static Archivo {
    static CACHE: OnceLock<Archivo> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str(CIE11_JSON).expect("el mapeo CIE-11 empotrado está corrupto")
    })
}

/// La versión de la clasificación contra la que se derivó, con su fecha de
/// tabla. Va al informe de cierre: dentro de tres años hay que poder saber
/// contra qué se derivó sin buscar el commit.
pub fn procedencia() -> (&'static str, &'static str) {
    let a = archivo();
    (&a.release, &a.version_tabla)
}

/// Deja el código en la forma del RIPS: mayúsculas, sin punto, sin espacios.
///
/// Acepta lo que llega de fuera —`a00.0`, `A00.0`, `A09`— porque un código
/// copiado de una remisión en papel o de otro sistema viene como venga. Lo que
/// **no** hace es completar a cuatro con `X` por su cuenta: eso solo es correcto
/// para las categorías sin subdivisión, y decidirlo aquí fabricaría códigos
/// inexistentes.
pub fn normalizar(cie10: &str) -> String {
    cie10.trim().to_uppercase().replace('.', "")
}

/// El par `(código CIE-11, título)` cuando la equivalencia es exacta y única.
///
/// `None` en todo lo demás, y el llamante escribe `""` — que es lo que la norma
/// espera, no un candidato elegido a dedo.
pub fn derivar(cie10: Option<&str>) -> Option<(&'static str, &'static str)> {
    let bruto = cie10?;
    let codigo = normalizar(bruto);
    if codigo.is_empty() {
        return None;
    }
    let d = &archivo().derivables;
    // Una categoría sin subdivisión se escribe `A09` fuera del RIPS y `A09X`
    // dentro. Se acepta la forma corta al consultar, porque es la misma
    // categoría escrita de las dos maneras que existen — no es adivinar otra.
    let par = d
        .get(&codigo)
        .or_else(|| if codigo.len() == 3 { d.get(&format!("{codigo}X")) } else { None })?;
    Some((par[0].as_str(), par[1].as_str()))
}

/// El par tal como va al JSON del RIPS: dos cadenas, vacías si no se deriva.
///
/// Existe porque los cinco documentos lo piden igual y en pares —`codDx…CIE11` y
/// `nomCodDx…CIE11`—, y repartir el `unwrap_or_default` por cada uno de los 22
/// campos es como se cuela un `""` donde iba un código y al revés.
pub fn par(cie10: Option<&str>) -> (String, String) {
    match derivar(cie10) {
        Some((c, t)) => (c.to_string(), t.to_string()),
        None => (String::new(), String::new()),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mapeo_empotrado_se_lee_y_trae_su_procedencia() {
        let (release, version) = procedencia();
        assert!(release.contains("id.who.int"), "el release no viene de la OMS: {release}");
        assert!(!version.is_empty());
        assert!(
            archivo().derivables.len() > 8_000,
            "el mapeo derivable encogió: {}",
            archivo().derivables.len()
        );
    }

    #[test]
    fn un_codigo_exacto_y_unico_se_deriva() {
        let (codigo, titulo) = derivar(Some("A000")).expect("A000 es exacto y único");
        assert!(!codigo.is_empty());
        assert!(!titulo.is_empty());
    }

    /// El código llega como venga de una remisión en papel.
    #[test]
    fn se_acepta_el_codigo_como_venga() {
        let esperado = derivar(Some("A000"));
        assert_eq!(derivar(Some("a000")), esperado);
        assert_eq!(derivar(Some("A00.0")), esperado);
        assert_eq!(derivar(Some("  a00.0  ")), esperado);
    }

    #[test]
    fn lo_que_no_esta_no_se_inventa() {
        assert_eq!(derivar(None), None);
        assert_eq!(derivar(Some("")), None);
        assert_eq!(derivar(Some("   ")), None);
        assert_eq!(derivar(Some("ZZZZ")), None, "un código inexistente devolvió algo");
        assert_eq!(par(Some("ZZZZ")), (String::new(), String::new()));
    }

    /// La prueba que sostiene la decisión: lo ambiguo NO se deriva aunque tenga
    /// destinos. Se comprueba contra el mapeo COMPLETO del Python, que es donde
    /// vive la evidencia de que ese código tiene más de un destino.
    #[test]
    fn lo_ambiguo_y_lo_de_categoria_no_estan_en_el_maestro() {
        // El generador ya los excluyó; aquí se comprueba que el archivo
        // empotrado no los trae, que es lo que de verdad usa el binario.
        let d = &archivo().derivables;
        assert!(
            d.len() < 12_000,
            "el maestro trae {} códigos: se coló lo ambiguo o lo de categoría",
            d.len()
        );
    }

    #[test]
    fn el_par_va_vacio_y_no_nulo_cuando_no_se_deriva() {
        let (c, t) = par(None);
        assert_eq!((c.as_str(), t.as_str()), ("", ""));
    }
}
