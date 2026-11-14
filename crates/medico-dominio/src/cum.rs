// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El CatalogoCUMs del INVIMA, para validar el medicamento del AM (#67).
//!
//! Port de la parte de `backend/cum.py` que el RIPS necesita: **decir si un CUM
//! existe**. Aquí no viven el nombre comercial ni el laboratorio ni los
//! principios activos, porque ninguno entra en el archivo de medicamentos; lo
//! que entra es el código, y lo que hace falta es saber si es de verdad.
//!
//! # Por qué se valida contra el catálogo y no solo «que no esté vacío»
//!
//! `codTecnologiaSalud` es el campo que la Resolución 948 (M08) contrasta
//! contra el listado del INVIMA. Un CUM mal digitado **tiene la forma
//! correcta**: pasa cualquier comprobación de estructura, y el MUV rechaza la
//! factura completa tres semanas después. Comprobarlo aquí convierte ese rechazo
//! en una línea del informe de cierre.
//!
//! Y `"00000000"` —el supuesto que este campo llegó a tener— no es un CUM: es un
//! dato falso con forma de dato, que es la clase peor.
//!
//! # De dónde sale la lista
//!
//! De `crates/medico-dominio/datos/cum_codigos.txt`, que escribe
//! `herramientas/generar_cum_para_rust.py` desde el MISMO `cum.json.gz` que lee
//! el Python. Un catálogo escrito a mano sería la segunda lista que mantener, y
//! el 2026-07-26 ya se pagó lo que cuesta eso (el mapa de presentaciones
//! inventado, con `tableta = 1` cuando el catálogo dice 66).

use std::collections::HashSet;
use std::sync::OnceLock;

const CODIGOS: &str = include_str!("../datos/cum_codigos.txt");

static INDICE: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn indice() -> &'static HashSet<&'static str> {
    INDICE.get_or_init(|| CODIGOS.lines().filter(|l| !l.trim().is_empty()).collect())
}

/// Quita espacios y unifica el separador.
///
/// Se acepta `20048021 41` y `20048021_41` además de `20048021-41` porque es lo
/// que llega de un sistema viejo o de una hoja de cálculo. Lo que **no** se hace
/// es completar con ceros ni inventar el consecutivo: a un CUM al que le falta
/// una parte no le falta nada — es otro medicamento.
pub fn normalizar(codigo: Option<&str>) -> String {
    let Some(c) = codigo else { return String::new() };
    let compacto = c.split_whitespace().collect::<Vec<_>>().join(" ");
    compacto.replace(['_', ' '], "-")
}

/// Si el código está en el catálogo del INVIMA. Normaliza antes de mirar.
pub fn existe(codigo: Option<&str>) -> bool {
    let n = normalizar(codigo);
    !n.is_empty() && indice().contains(n.as_str())
}

/// Cuántos códigos trae el catálogo empotrado. Para las pruebas y para poder
/// decir en el arranque qué se está usando.
pub fn total() -> usize {
    indice().len()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_catalogo_empotrado_no_esta_vacio() {
        // El positivo de control de todo lo demás: si el archivo llegara vacío,
        // `existe` diría «no» a todo y las pruebas de rechazo pasarían igual.
        assert!(
            total() > 60_000,
            "el catálogo trae {} códigos; el del INVIMA tiene ~66 000",
            total()
        );
    }

    #[test]
    fn un_cum_del_catalogo_existe_y_uno_inventado_no() {
        // La pareja. El primero se lee del propio archivo empotrado, no se
        // escribe a mano: una aguja inventada validaría contra sí misma.
        let alguno = CODIGOS.lines().next().expect("el catálogo no está vacío");
        assert!(existe(Some(alguno)), "«{alguno}» sale del catálogo");
        assert!(!existe(Some("00000000")), "el supuesto de antes NO es un CUM");
        assert!(!existe(Some("99999999-9")));
    }

    #[test]
    fn los_separadores_de_una_hoja_de_calculo_se_aceptan() {
        let alguno = CODIGOS.lines().next().unwrap().to_string();
        let con_espacio = alguno.replace('-', " ");
        let con_guion_bajo = alguno.replace('-', "_");
        assert!(existe(Some(&con_espacio)), "«{con_espacio}»");
        assert!(existe(Some(&con_guion_bajo)), "«{con_guion_bajo}»");
    }

    #[test]
    fn ni_vacio_ni_ausente_son_un_cum() {
        assert!(!existe(None));
        assert!(!existe(Some("")));
        assert!(!existe(Some("   ")));
        assert_eq!(normalizar(None), "");
        assert_eq!(normalizar(Some("  ")), "");
    }

    #[test]
    fn no_se_completa_con_ceros_ni_se_inventa_el_consecutivo() {
        // `20048021` sin consecutivo NO es el CUM `20048021-41`.
        let alguno = CODIGOS.lines().next().unwrap();
        let (expediente, _) = alguno.split_once('-').expect("todos llevan guion");
        assert!(
            !existe(Some(expediente)),
            "«{expediente}» es medio código y no debe casar con «{alguno}»"
        );
    }
}
