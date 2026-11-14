// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Compatibilidad transfusional (#249, `BANCO_SANGRE`).
//!
//! Vive en el DOMINIO y no en la capa de datos porque es una regla, no una
//! consulta: se prueba con valores escritos a mano, en microsegundos, y sin base.
//! Es la pieza de este producto donde un error no se factura mal — mata.
//!
//! # El grupo del receptor NO se busca: se registra en el momento
//!
//! Medido antes de escribir: `paciente` **no tiene grupo sanguíneo**. Lo único
//! parecido es `programacion_quirurgica.hemoclasificacion`, que es texto libre de
//! una cirugía concreta.
//!
//! Eso no es un hueco que haya que tapar con una columna en `paciente`: es la
//! práctica correcta. El grupo se confirma con pruebas cruzadas ANTES de cada
//! transfusión, y transfundir contra un dato guardado hace meses es exactamente
//! el error que mata. Aquí se exige en cada transfusión, y sin él no hay
//! compatibilidad que calcular — se responde «no se puede saber», nunca «sí».
//!
//! # La regla es de PLASMA al revés que de GLÓBULOS
//!
//! Un concentrado de glóbulos rojos aporta antígenos: el receptor no puede
//! recibir antígenos que no tenga. El plasma aporta ANTICUERPOS: la relación se
//! invierte, y el donante universal de plasma es AB, no O. Un módulo que aplicara
//! la tabla de glóbulos al plasma daría el resultado exactamente contrario en los
//! casos que importan.

/// Los cuatro grupos ABO.
pub const GRUPOS: [&str; 4] = ["O", "A", "B", "AB"];
/// Los dos RH, escritos como los escribe un banco de sangre.
pub const RH: [&str; 2] = ["+", "-"];

/// Qué se va a transfundir. La compatibilidad depende de esto y no es un detalle
/// de etiqueta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hemocomponente {
    /// Concentrado de glóbulos rojos: aporta ANTÍGENOS.
    GlobulosRojos,
    /// Plasma fresco congelado: aporta ANTICUERPOS, y la relación se invierte.
    Plasma,
    /// Plaquetas. Se prefiere isogrupo; la compatibilidad ABO se maneja como en
    /// glóbulos porque el concentrado lleva plasma residual.
    Plaquetas,
    /// Crioprecipitado. Como el plasma.
    Crioprecipitado,
}

impl Hemocomponente {
    pub fn codigo(self) -> &'static str {
        match self {
            Hemocomponente::GlobulosRojos => "GLOBULOS_ROJOS",
            Hemocomponente::Plasma => "PLASMA",
            Hemocomponente::Plaquetas => "PLAQUETAS",
            Hemocomponente::Crioprecipitado => "CRIOPRECIPITADO",
        }
    }

    pub fn de_codigo(c: &str) -> Option<Hemocomponente> {
        match c {
            "GLOBULOS_ROJOS" => Some(Hemocomponente::GlobulosRojos),
            "PLASMA" => Some(Hemocomponente::Plasma),
            "PLAQUETAS" => Some(Hemocomponente::Plaquetas),
            "CRIOPRECIPITADO" => Some(Hemocomponente::Crioprecipitado),
            _ => None,
        }
    }

    /// ¿La compatibilidad de este componente va como el PLASMA (invertida)?
    fn invertida(self) -> bool {
        matches!(
            self,
            Hemocomponente::Plasma | Hemocomponente::Crioprecipitado
        )
    }
}

/// El veredicto. **Tres y no dos**: «no se puede saber» no es «no compatible», y
/// sobre todo no es «compatible». Un booleano obligaría a elegir uno de los dos
/// y los dos serían mentira.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredicto {
    Compatible,
    /// Incompatible, con el motivo escrito para que salga en la pantalla y en la
    /// bitácora — no un `false` que nadie puede explicar después.
    Incompatible(String),
    /// Falta un dato. Nunca se resuelve a favor.
    NoSePuedeSaber(String),
}

impl Veredicto {
    pub fn compatible(&self) -> bool {
        matches!(self, Veredicto::Compatible)
    }
}

/// Los antígenos ABO que lleva un grupo.
fn antigenos(grupo: &str) -> Option<(bool, bool)> {
    match grupo {
        "O" => Some((false, false)),
        "A" => Some((true, false)),
        "B" => Some((false, true)),
        "AB" => Some((true, true)),
        _ => None,
    }
}

/// ¿Puede este receptor recibir esta unidad?
///
/// Los grupos llegan como texto porque así vienen de la base y del formulario; lo
/// que no se acepta es un grupo que no esté en [`GRUPOS`] — y ahí la respuesta es
/// `NoSePuedeSaber`, no `Incompatible`: un grupo mal escrito no dice nada sobre
/// la sangre, dice algo sobre el teclado.
pub fn compatible(
    componente: Hemocomponente,
    grupo_unidad: &str,
    rh_unidad: &str,
    grupo_receptor: &str,
    rh_receptor: &str,
) -> Veredicto {
    let (gu, gr) = (grupo_unidad.trim(), grupo_receptor.trim());
    let (ru, rr) = (rh_unidad.trim(), rh_receptor.trim());

    let (Some((a_u, b_u)), Some((a_r, b_r))) = (antigenos(gu), antigenos(gr)) else {
        return Veredicto::NoSePuedeSaber(format!(
            "grupo ABO no reconocido (unidad «{gu}», receptor «{gr}»): los grupos son O, A, B y AB"
        ));
    };
    if !RH.contains(&ru) || !RH.contains(&rr) {
        return Veredicto::NoSePuedeSaber(format!(
            "RH no reconocido (unidad «{ru}», receptor «{rr}»): se escribe + o −"
        ));
    }

    // ABO. En glóbulos, el receptor no puede recibir un antígeno que no tenga.
    // En plasma la relación se INVIERTE: es el plasma el que trae anticuerpos
    // contra los antígenos que su donante no tiene.
    let choque = if componente.invertida() {
        (a_r && !a_u) || (b_r && !b_u)
    } else {
        (a_u && !a_r) || (b_u && !b_r)
    };
    if choque {
        return Veredicto::Incompatible(format!(
            "ABO incompatible: unidad {gu} para receptor {gr} en {}",
            componente.codigo()
        ));
    }

    // RH. Solo aplica a los componentes con glóbulos: un receptor RH negativo no
    // puede recibir glóbulos RH positivos. En plasma no hay antígeno D que
    // transfundir, así que no se rechaza — rechazarlo descartaría unidades útiles
    // sin ganar seguridad (directiva 21).
    if !componente.invertida() && ru == "+" && rr == "-" {
        return Veredicto::Incompatible(format!(
            "RH incompatible: unidad RH+ para receptor RH− en {}",
            componente.codigo()
        ));
    }

    Veredicto::Compatible
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use Hemocomponente::*;

    fn v(c: Hemocomponente, gu: &str, ru: &str, gr: &str, rr: &str) -> Veredicto {
        compatible(c, gu, ru, gr, rr)
    }

    /// La tabla completa de glóbulos rojos, ABO, en las 16 combinaciones.
    ///
    /// Se escribe entera y no por regla: la regla es la que se está probando, y
    /// derivar el caso esperado de la misma fórmula que se mide es la forma más
    /// limpia de que una prueba diga que sí a todo.
    #[test]
    fn la_tabla_abo_de_globulos_rojos_entera() {
        // (unidad, receptor, compatible)
        let esperado = [
            ("O", "O", true), ("O", "A", true), ("O", "B", true), ("O", "AB", true),
            ("A", "O", false), ("A", "A", true), ("A", "B", false), ("A", "AB", true),
            ("B", "O", false), ("B", "A", false), ("B", "B", true), ("B", "AB", true),
            ("AB", "O", false), ("AB", "A", false), ("AB", "B", false), ("AB", "AB", true),
        ];
        for (u, r, ok) in esperado {
            assert_eq!(
                v(GlobulosRojos, u, "+", r, "+").compatible(),
                ok,
                "glóbulos {u} → {r}"
            );
        }
    }

    /// **El plasma va AL REVÉS**, y es la mitad del valor de este módulo. El
    /// donante universal de plasma es AB, no O; y O es el receptor universal de
    /// plasma, no el donante.
    #[test]
    fn el_plasma_invierte_la_tabla() {
        // AB es donante universal de plasma…
        for r in GRUPOS {
            assert!(v(Plasma, "AB", "+", r, "+").compatible(), "plasma AB → {r}");
        }
        // …y O NO lo es, al revés que en glóbulos.
        assert!(!v(Plasma, "O", "+", "A", "+").compatible(), "plasma O → A no");
        assert!(v(GlobulosRojos, "O", "+", "A", "+").compatible(), "pero glóbulos O → A sí");
        // O es receptor universal de plasma.
        for u in GRUPOS {
            assert!(v(Plasma, u, "+", "O", "+").compatible(), "plasma {u} → O");
        }
        // Y AB NO lo es en plasma, al revés que en glóbulos.
        assert!(!v(Plasma, "O", "+", "AB", "+").compatible());
        assert!(v(GlobulosRojos, "O", "+", "AB", "+").compatible());
    }

    /// El crioprecipitado se comporta como el plasma. Sin esta prueba, alguien
    /// podría meterlo en el grupo de los glóbulos y las dos tablas del módulo
    /// seguirían verdes.
    #[test]
    fn el_crioprecipitado_va_como_el_plasma() {
        assert!(v(Crioprecipitado, "AB", "+", "O", "+").compatible());
        assert!(!v(Crioprecipitado, "O", "+", "AB", "+").compatible());
    }

    #[test]
    fn el_rh_negativo_no_recibe_globulos_positivos() {
        assert!(!v(GlobulosRojos, "O", "+", "O", "-").compatible());
        assert!(v(GlobulosRojos, "O", "-", "O", "-").compatible());
        // Y al revés SÍ: un RH+ puede recibir RH−.
        assert!(v(GlobulosRojos, "O", "-", "O", "+").compatible());
    }

    /// En plasma no hay antígeno D que transfundir, así que el RH no rechaza.
    /// Rechazarlo descartaría unidades útiles sin ganar seguridad (directiva 21).
    #[test]
    fn el_rh_no_aplica_al_plasma() {
        assert!(v(Plasma, "AB", "+", "AB", "-").compatible());
    }

    /// **Un dato que falta NO se resuelve a favor.** Es la regla que separa este
    /// módulo de uno peligroso: sin grupo del receptor no hay «compatible».
    #[test]
    fn lo_que_no_se_puede_saber_no_es_compatible() {
        for caso in [
            v(GlobulosRojos, "O", "+", "", "+"),
            v(GlobulosRojos, "O", "+", "Z", "+"),
            v(GlobulosRojos, "", "+", "O", "+"),
            v(GlobulosRojos, "O", "positivo", "O", "+"),
            v(GlobulosRojos, "O", "+", "O", ""),
        ] {
            assert!(
                matches!(caso, Veredicto::NoSePuedeSaber(_)),
                "un dato que falta tiene que dar NoSePuedeSaber, no {caso:?}"
            );
            assert!(!caso.compatible(), "y desde luego no compatible");
        }
    }

    /// El incompatible DICE por qué. Un `false` que nadie puede explicar después
    /// no sirve en la bitácora de un evento adverso.
    #[test]
    fn el_incompatible_lleva_su_motivo() {
        let Veredicto::Incompatible(m) = v(GlobulosRojos, "A", "+", "O", "+") else {
            panic!("A → O tenía que ser incompatible");
        };
        assert!(m.contains("ABO"), "el motivo dice qué falló: {m}");
        let Veredicto::Incompatible(m) = v(GlobulosRojos, "O", "+", "O", "-") else {
            panic!("RH+ → RH− tenía que ser incompatible");
        };
        assert!(m.contains("RH"));
    }

    #[test]
    fn los_hemocomponentes_van_y_vuelven_por_su_codigo() {
        for c in [GlobulosRojos, Plasma, Plaquetas, Crioprecipitado] {
            assert_eq!(Hemocomponente::de_codigo(c.codigo()), Some(c));
        }
        assert_eq!(Hemocomponente::de_codigo("SUERO"), None);
    }
}
