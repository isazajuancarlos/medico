// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Contrarreferencia: lo que vuelve del especialista.
//!
//! Sale de `medico_datos::referencia::contrarref_listar`, la MISMA función que
//! sirve `GET /api/contrarreferencia`.
//!
//! # Las PENDIENTES son el filtro por defecto, y es la razón de la pantalla
//!
//! Una contrarreferencia sin recibir es un paciente que volvió del especialista
//! y cuyo informe nadie ha leído todavía. El listado completo es el archivo; lo
//! que hay que mirar cada día son las que faltan por recibir, así que ése es el
//! estado en el que se abre — y se dice cuál está puesto, porque una lista
//! filtrada sin avisar se lee como la lista entera.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

#[derive(Deserialize, Default)]
pub struct Filtro {
    /// `1` = mostrar también las ya recibidas.
    #[serde(default)]
    pub todas: String,
}

/// `GET /panel/contrarreferencia`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let todas = f.todas == "1";
    let filas =
        match medico_datos::referencia::contrarref_listar(&e.pool, u.ambito(), !todas).await {
            Ok(v) => v,
            Err(_) => return averia(),
        };

    pagina(
        "Contrarreferencia",
        Some(crate::vista::panel::menu(&cx, "/panel/contrarreferencia")),
        html! {
            header class="titulo-seccion" {
                h1 { "Contrarreferencia" }
                nav class="buscador" {
                    a class=[(!todas).then_some("enlace-activo")]
                      href="/panel/contrarreferencia" { "Sin recibir" }
                    a class=[todas.then_some("enlace-activo")]
                      href="/panel/contrarreferencia?todas=1" { "Todas" }
                }
            }

            @if filas.is_empty() {
                (piezas::vacio(
                    !todas,
                    "Todavía no ha vuelto ninguna contrarreferencia.",
                    "No hay contrarreferencias sin recibir.",
                    "/panel/contrarreferencia?todas=1",
                ))
            } @else {
                (piezas::recuento(filas.len(), filas.len(), "contrarreferencia", "contrarreferencias"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Diagnóstico final", "Seguimiento", "Control sugerido", "Estado"],
                    html! {
                        @for c in &filas {
                            tr {
                                td class="mono" { (c.fecha_creacion) }
                                td {
                                    a href={ "/panel/pacientes/" (c.paciente_id) } { (c.paciente) }
                                }
                                td { (c.diagnostico_final) }
                                td {
                                    @if c.requiere_seguimiento { "Sí" } @else { "No" }
                                }
                                td class="mono" { (c.fecha_control_sugerida.clone().unwrap_or_default()) }
                                td {
                                    @if c.recibida {
                                        (piezas::estado("RECIBIDA"))
                                    } @else {
                                        (piezas::estado("PENDIENTE"))
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
