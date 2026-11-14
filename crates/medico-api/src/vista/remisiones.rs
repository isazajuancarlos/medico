// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Remisiones: la bandeja, con su resumen por estado.
//!
//! Sale de `medico_datos::referencia`, las MISMAS funciones que sirve
//! `GET /api/remisiones`.
//!
//! # El vocabulario de estados es el del PRODUCTO, y no se traduce aquí
//!
//! «Pendiente», «En Proceso», «Completada», «Cancelada» son los valores del
//! `CHECK` de `orden.estado` (migración `0027`), y son los que escribe la
//! historia clínica. Dos pantallas quedaron muertas por comparar «Pendiente»
//! contra «PENDIENTE» y «ESPECIALISTA» contra «Remisión Especialista», así que
//! aquí se pintan tal cual llegan y el color lo decide `piezas::estado`, que
//! normaliza para MIRAR pero no cambia el dato.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub estado: String,
}

fn no_vacio(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty()).then_some(s)
}

/// `GET /panel/remisiones`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let buscar = no_vacio(&f.q);
    let estado_filtro = no_vacio(&f.estado);
    let (filas, resumen) = match (
        medico_datos::referencia::remisiones_listar(
            &e.pool, u.ambito(), estado_filtro, None, None, buscar,
        )
        .await,
        // El resumen va SIN el filtro de estado a propósito: es el que deja
        // contar cuántas hay en cada uno, y filtrarlo por el estado elegido
        // dejaría los demás contadores en cero — la pantalla diría que no hay
        // remisiones pendientes justo cuando estás mirando las completadas.
        medico_datos::referencia::remisiones_resumen(&e.pool, u.ambito(), None, None, buscar).await,
    ) {
        (Ok(l), Ok(r)) => (l, r),
        _ => return averia(),
    };

    // Un eslabón por petición, con `filtro_remisiones` —la misma función que usa
    // `GET /api/remisiones`—, y con el estado que se aplicó AL LISTADO: el
    // resumen va a propósito sin él, pero lo que se anota aquí es lo que se
    // miró, que es la tabla.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_REMISIONES,
        crate::rutas::filtro_remisiones(estado_filtro, None, None, buscar),
        filas.len(),
        vec![],
    )
    .await;

    let hay_filtro = buscar.is_some() || estado_filtro.is_some();

    pagina(
        "Remisiones",
        Some(crate::vista::panel::menu(&cx, "/panel/remisiones")),
        html! {
            header class="titulo-seccion" {
                h1 { "Remisiones" }
                form method="get" action="/panel/remisiones" class="buscador" {
                    label class="oculto" for="q" { "Buscar" }
                    input #q type="search" name="q" value=(f.q) placeholder="Paciente o documento";
                    label class="oculto" for="estado" { "Estado" }
                    select #estado name="estado" {
                        option value="" selected[f.estado.is_empty()] { "Todos los estados" }
                        @for (nombre, _) in &resumen.por_estado {
                            option value=(nombre) selected[f.estado == *nombre] { (nombre) }
                        }
                    }
                    button type="submit" { "Filtrar" }
                    @if hay_filtro {
                        a class="enlace-sutil" href="/panel/remisiones" { "Quitar" }
                    }
                }
            }

            section class="tarjeta resumen" {
                div { span class="cifra" { (resumen.total) } span class="sutil" { "en total" } }
                div { span class="cifra" { (resumen.abiertas) } span class="sutil" { "abiertas" } }
                @for (nombre, cuantas) in &resumen.por_estado {
                    div { span class="cifra" { (cuantas) } span class="sutil" { (nombre) } }
                }
            }

            @if filas.is_empty() {
                (piezas::vacio(
                    hay_filtro,
                    "Todavía no hay remisiones.",
                    "Ninguna remisión coincide con el filtro.",
                    "/panel/remisiones",
                ))
            } @else {
                (piezas::tabla(
                    &["Fecha", "Paciente", "Documento", "Especialidad", "Institución destino", "Estado"],
                    html! {
                        @for r in &filas {
                            tr {
                                td class="mono" { (r.fecha_orden) }
                                td { (r.paciente) }
                                td class="mono" { (r.identificacion) }
                                td { (r.especialidad_destino.clone().unwrap_or_default()) }
                                td { (r.institucion_destino.clone().unwrap_or_default()) }
                                td { (piezas::estado(&r.estado)) }
                            }
                        }
                    },
                ))
            }
        },
    )
    .into_response()
}
