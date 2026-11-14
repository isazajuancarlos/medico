// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Citas: la agenda del día, con su filtro de fecha.
//!
//! Sale de `medico_datos::cita::listar`, la MISMA función que sirve
//! `GET /api/citas`, con el `Ambito` de quien mira.
//!
//! # Los `None` se dejan VACÍOS, y ésa es toda la pantalla
//!
//! `tipo` y `duracion_minutos` son `Option` a propósito: las citas anteriores a
//! la migración `0035` no los traen, y eso significa **no se registró**, no
//! «media hora» ni «primera vez». El frontal de Angular rellenaba «Primera Vez»
//! para todas — un dato que nadie escribió, con la forma correcta, en la pantalla
//! desde la que se agenda. Aquí se dibuja el hueco.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

#[derive(Deserialize, Default)]
pub struct Filtro {
    /// `YYYY-MM-DD`. Vacío = todas.
    #[serde(default)]
    pub fecha: String,
}

/// `GET /panel/citas`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let fecha = f.fecha.trim();
    // La fecha se pasa a la CONSULTA, no se filtra en memoria: la agenda de un
    // día es el caso normal y traerse el año entero para quedarse con un día es
    // lo que hace lenta una pantalla que se abre cada mañana.
    let citas = match medico_datos::cita::listar(
        &e.pool,
        u.ambito(),
        (!fecha.is_empty()).then_some(fecha),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // Con los MISMOS argumentos que `GET /api/citas`: la agenda del día lleva el
    // nombre del paciente de cada cita.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_CITAS,
        crate::rutas::filtro_de(&[("fecha", (!fecha.is_empty()).then_some(fecha))]),
        citas.len(),
        vec![],
    )
    .await;

    pagina(
        "Citas",
        Some(crate::vista::panel::menu(&cx, "/panel/citas")),
        html! {
            header class="titulo-seccion" {
                h1 { "Citas" }
                form method="get" action="/panel/citas" class="buscador" {
                    label class="oculto" for="fecha" { "Fecha" }
                    input #fecha type="date" name="fecha" value=(f.fecha);
                    button type="submit" { "Ver" }
                    @if !fecha.is_empty() {
                        a class="enlace-sutil" href="/panel/citas" { "Todas" }
                    }
                }
            }

            @if citas.is_empty() {
                (piezas::vacio(
                    !fecha.is_empty(),
                    "Todavía no hay citas agendadas.",
                    "No hay citas para esa fecha.",
                    "/panel/citas",
                ))
            } @else {
                (piezas::recuento(citas.len(), citas.len(), "cita", "citas"))
                (piezas::tabla(
                    &["Fecha", "Hora", "Paciente", "Profesional", "Tipo", "Duración", "Estado"],
                    html! {
                        @for c in &citas {
                            tr {
                                td class="mono" { (c.fecha) }
                                td class="mono" { (c.hora) }
                                td {
                                    a href={ "/panel/pacientes/" (c.paciente_id) } {
                                        (c.paciente_nombre)
                                    }
                                }
                                td { (c.medico_nombre.clone().unwrap_or_default()) }
                                // VACÍO y no «Primera Vez»: en las citas viejas
                                // el tipo no se registró, y ponerle uno sería
                                // afirmar en nombre de quien agendó.
                                td { (c.tipo.clone().unwrap_or_default()) }
                                td {
                                    @match c.duracion_minutos {
                                        Some(m) => { (m) " min" }
                                        None => "",
                                    }
                                }
                                td { (piezas::estado(&c.estado)) }
                            }
                        }
                    },
                ))
            }
        },
    )
    .into_response()
}
