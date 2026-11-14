// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Los documentos clínicos: evoluciones, epicrisis, interconsultas y notas
//! operatorias.
//!
//! # UNA pantalla y no cuatro, porque son UNA familia
//!
//! En Angular son cuatro componentes (evoluciones 526 líneas, epicrisis 629,
//! interconsultas 600, nota-operatoria 680 — **2 435 en total**) que hacen lo
//! mismo con otro título: listan `documento` filtrando por `tipo`. La API ya lo
//! tenía resuelto así —`/api/documentos` con el tipo como filtro, una sola
//! familia de rutas— y aquí se sigue esa forma en vez de repetirla cuatro veces.
//!
//! Es la directiva 9 aplicada antes de escribir, no después: cuatro copias de
//! una tabla se separan en el primer arreglo, y este producto ya pagó eso con
//! `orden.estado` —dos pantallas muertas por comparar «Pendiente» contra
//! «PENDIENTE»—.
//!
//! Cada tipo tiene su ruta y su entrada de menú, así que para quien mira siguen
//! siendo cuatro secciones; lo que no hay son cuatro implementaciones.
//!
//! # Y los cuatro tipos son los del CATÁLOGO, no una lista de aquí
//!
//! `EPICRISIS`, `INTERCONSULTAS`, `NOTA_OPERATORIA` y `EVOLUCIONES` son módulos
//! con su licencia propia, y los cuatro declaran el mismo `prefijo`
//! (`/api/documentos`). Que la licencia de uno abra los cuatro es una decisión ya
//! tomada y escrita en `licencia.rs`; aquí lo que importa es que el `tipo` con
//! el que se filtra sea el que la base guarda.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use maud::html;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

/// Un tipo de documento clínico: cómo se llama en la base, en la ruta y en la
/// pantalla.
///
/// El primer campo es el valor de `documento.tipo` **tal y como la base lo
/// guarda**. No se normaliza ni se compara sin distinguir mayúsculas: este
/// producto ya tuvo dos pantallas muertas por comparar un vocabulario contra
/// otro, y la lección fue usar el del dato.
pub struct Tipo {
    pub en_la_base: &'static str,
    pub ruta: &'static str,
    pub titulo: &'static str,
}

pub const TIPOS: &[Tipo] = &[
    Tipo { en_la_base: "EVOLUCION", ruta: "/panel/evoluciones", titulo: "Evoluciones" },
    Tipo { en_la_base: "EPICRISIS", ruta: "/panel/epicrisis", titulo: "Epicrisis" },
    Tipo {
        en_la_base: "INTERCONSULTA",
        ruta: "/panel/interconsultas",
        titulo: "Interconsultas",
    },
    Tipo {
        en_la_base: "NOTA_OPERATORIA",
        ruta: "/panel/notas-operatorias",
        titulo: "Notas operatorias",
    },
];

async fn worklist(e: &Estado, u: &medico_datos::usuario::Usuario, t: &Tipo) -> Response {
    let cx = match contexto(e, u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let docs = match medico_datos::documento::listar_ambito(
        &e.pool,
        u.ambito(),
        None,
        Some(t.en_la_base),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // Los cuatro listados atraviesan titulares y devuelven nombre y documento de
    // cada uno: `Coleccion` de la Ley 1581, con los MISMOS argumentos que
    // `GET /api/documentos` sin `paciente_id` —incluido el `tipo=` del filtro—.
    // Aquí no hay caso `DeUnTitular`: esta pantalla no filtra por paciente, y
    // fabricarle uno sería un campo que nadie manda (directiva 20).
    crate::rutas::registrar_coleccion(
        e,
        u,
        "listar",
        "documentos",
        Some(format!("tipo={}", t.en_la_base)),
        docs.len(),
        vec![],
    )
    .await;

    // Un documento FIRMADO no se edita: sobre él ya se decidió una conducta y se
    // corrige emitiendo otro (Ley 527/1999). El número va arriba porque es lo
    // que dice cuánto trabajo queda abierto en el servicio.
    let sin_firmar = docs.iter().filter(|d| !d.firmada).count();

    pagina(
        t.titulo,
        Some(crate::vista::panel::menu(&cx, t.ruta)),
        html! {
            header class="titulo-seccion" { h1 { (t.titulo) } }

            @if docs.is_empty() {
                (piezas::vacio(
                    false,
                    &format!("Todavía no hay {} registradas.", t.titulo.to_lowercase()),
                    "",
                    t.ruta,
                ))
            } @else {
                section class="tarjeta resumen" {
                    div { span class="cifra" { (docs.len()) } span class="sutil" { "en total" } }
                    div { span class="cifra" { (sin_firmar) } span class="sutil" { "sin firmar" } }
                }
                (piezas::tabla(
                    &["Fecha", "Paciente", "Documento", "Título", "Profesional", "Firma"],
                    html! {
                        @for d in &docs {
                            tr {
                                td class="mono" { (d.fecha) }
                                td {
                                    a href={ "/panel/pacientes/" (d.paciente_id) } {
                                        (d.paciente_nombre)
                                    }
                                }
                                td class="mono" { (d.paciente_identificacion) }
                                td { (d.titulo) }
                                td { (d.medico_nombre) }
                                td {
                                    @if d.firmada {
                                        (piezas::estado("FIRMADA"))
                                    } @else {
                                        (piezas::estado("BORRADOR"))
                                    }
                                }
                            }
                        }
                    },
                ))
            }
        },
    )
    .into_response()
}

// Un manejador por tipo: axum necesita una función por ruta, y así cada una
// queda nombrada en el router. El cuerpo es el mismo para las cuatro.
pub async fn evoluciones(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    worklist(&e, &u, &TIPOS[0]).await
}
pub async fn epicrisis(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    worklist(&e, &u, &TIPOS[1]).await
}
pub async fn interconsultas(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    worklist(&e, &u, &TIPOS[2]).await
}
pub async fn notas_operatorias(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    worklist(&e, &u, &TIPOS[3]).await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **La tabla de tipos y la de pantallas no pueden divergir.**
    ///
    /// Cada tipo declara su ruta, y esa ruta tiene que estar en
    /// `licencia::PANTALLAS` — que es la que construye el menú Y cierra la
    /// puerta. Una ruta aquí que no esté allí se serviría sin licencia y sin
    /// aparecer en el menú: invisible y abierta, que es la peor combinación.
    #[test]
    fn cada_tipo_tiene_su_pantalla_declarada() {
        for t in TIPOS {
            assert!(
                crate::licencia::PANTALLAS.iter().any(|p| p.ruta == t.ruta),
                "«{}» sirve {} y no está en PANTALLAS: se serviría sin licencia",
                t.titulo,
                t.ruta
            );
        }
    }

    /// Y los valores de `tipo` son distintos entre sí: con dos iguales, dos
    /// pantallas mostrarían la misma lista y nadie lo notaría.
    #[test]
    fn los_cuatro_tipos_son_distintos() {
        let mut vistos = std::collections::HashSet::new();
        for t in TIPOS {
            assert!(vistos.insert(t.en_la_base), "tipo repetido: {}", t.en_la_base);
            assert!(vistos.insert(t.ruta), "ruta repetida: {}", t.ruta);
        }
    }
}
