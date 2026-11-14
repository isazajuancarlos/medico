// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El personal de la institución, y **a quién le falta el documento**.
//!
//! Sale de `medico_datos::usuario::listar`, la MISMA función que sirve
//! `GET /api/usuarios`.
//!
//! # La pantalla existe por P11/P12, y por eso el documento va primero
//!
//! `usuario.tipo_documento` y `numero_documento` los exige el RIPS en los cinco
//! archivos, y hasta el 2026-08-03 **ninguna ruta los escribía**: el recolector
//! los anotaba como ausentes en toda atención y no había forma de arreglarlo
//! desde el producto. Por eso el `ResumenUsuario` los trae en el LISTADO —la
//! pregunta que hay que poder responder de un vistazo es «¿a quién le falta?»— y
//! por eso esta pantalla lo dice arriba con un número, no escondido en una
//! columna.
//!
//! No son dato sensible: es el documento del profesional, que va impreso en cada
//! fórmula que firma.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use maud::html;

use crate::auth::Estado;
use crate::auth::GestionarUsuarios;
use crate::vista::{averia, contexto, pagina, piezas, EnSesionCon};

/// `GET /panel/usuarios`
/// **Exige `GestionarUsuarios`, la MISMA capacidad que `GET /api/usuarios`.**
/// Ver [`crate::vista::EnSesionCon`]: la primera versión pedía solo sesión, y eso
/// habría quitado una autorización que el producto ya tenía.
pub async fn listar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let gente = match medico_datos::usuario::listar(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // Se cuenta sobre los ACTIVOS: a quien ya no trabaja aquí no hay que
    // perseguirlo para que dé su documento, y contarlo inflaría un número que
    // existe para saber cuánto falta.
    let sin_documento = gente
        .iter()
        .filter(|g| g.activo)
        .filter(|g| g.numero_documento.as_deref().unwrap_or("").trim().is_empty())
        .count();
    let activos = gente.iter().filter(|g| g.activo).count();

    pagina(
        "Personal",
        Some(crate::vista::panel::menu(&cx, "/panel/usuarios")),
        html! {
            header class="titulo-seccion" { h1 { "Personal" } }

            @if gente.is_empty() {
                (piezas::vacio(false, "No hay usuarios registrados.", "", "/panel/usuarios"))
            } @else {
                section class="tarjeta resumen" {
                    div { span class="cifra" { (activos) } span class="sutil" { "activos" } }
                    div {
                        span class="cifra" { (gente.len() - activos) }
                        span class="sutil" { "inactivos" }
                    }
                    @if sin_documento > 0 {
                        div {
                            span class="cifra" { (sin_documento) }
                            span class="sutil" { "SIN documento" }
                        }
                    }
                }
                @if sin_documento > 0 {
                    (crate::vista::aviso(
                        crate::vista::Tono::Error,
                        "El RIPS exige el documento de quien atiende en los cinco archivos. \
                         Cada profesional sin él deja una falta en TODA atención que firme, \
                         y eso no se puede corregir después sobre el archivo ya enviado.",
                    ))
                }
                (piezas::tabla(
                    &["Nombre", "Correo", "Rol", "Documento", "Estado"],
                    html! {
                        @for g in &gente {
                            tr {
                                td { (g.nombre) }
                                td { (g.email) }
                                td { (g.rol) }
                                td class="mono" {
                                    @match (&g.tipo_documento, &g.numero_documento) {
                                        (Some(t), Some(n)) if !n.trim().is_empty() => {
                                            (t) " " (n)
                                        }
                                        // Se MARCA, no se deja en blanco: un
                                        // hueco silencioso en una columna es lo
                                        // que hizo que P11/P12 pasara meses vacío.
                                        _ => span class="falta" { "falta" },
                                    }
                                }
                                td {
                                    @if g.activo {
                                        (piezas::estado("ACTIVO"))
                                    } @else {
                                        (piezas::estado("INACTIVO"))
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
