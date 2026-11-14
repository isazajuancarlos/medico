// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Incapacidades: el listado con su resumen, y el detalle de una.
//!
//! Sale de `medico_datos::incapacidad`, con los MISMOS `Filtros` que sirve
//! `GET /api/incapacidades` — así el listado y el resumen hablan del mismo
//! conjunto, que es la promesa que esa capa ya tenía escrita.
//!
//! # El resumen se pide APARTE y no se calcula sumando la tabla
//!
//! Podría sumarse en el bucle y saldría el mismo número hoy. Se pide a
//! `incapacidad::resumen` porque es la función que garantiza el MISMO conjunto
//! que `listar`, y el día que el listado se pagine —que llegará— sumar lo
//! mostrado daría el total de la página y se leería como el total del periodo.
//! Un número que cambia de significado sin avisar es peor que uno que falta.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
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
    pub desde: String,
    #[serde(default)]
    pub hasta: String,
}

fn no_vacio(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty()).then_some(s)
}

/// `GET /panel/incapacidades`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let filtros = medico_datos::incapacidad::Filtros {
        desde: no_vacio(&f.desde),
        hasta: no_vacio(&f.hasta),
        medico_id: None,
        buscar: no_vacio(&f.q),
    };
    let (filas, resumen) = match (
        medico_datos::incapacidad::listar(&e.pool, u.ambito(), &filtros).await,
        medico_datos::incapacidad::resumen(&e.pool, u.ambito(), &filtros).await,
    ) {
        (Ok(l), Ok(r)) => (l, r),
        _ => return averia(),
    };

    // Un eslabón por petición: la pantalla pide el listado y el recuento, y
    // anota UNO —el del listado, con `filtro_incapacidades`, que es la misma
    // función que usa `GET /api/incapacidades`—. Por las dos rutas JSON son dos
    // peticiones y salen dos eslabones, que es lo que de verdad ocurrió.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_INCAPACIDADES,
        crate::rutas::filtro_incapacidades(&filtros),
        filas.len(),
        vec![],
    )
    .await;

    let hay_filtro = filtros.desde.is_some() || filtros.hasta.is_some() || filtros.buscar.is_some();

    pagina(
        "Incapacidades",
        Some(crate::vista::panel::menu(&cx, "/panel/incapacidades")),
        html! {
            header class="titulo-seccion" {
                h1 { "Incapacidades" }
                form method="get" action="/panel/incapacidades" class="buscador" {
                    label class="oculto" for="q" { "Buscar" }
                    input #q type="search" name="q" value=(f.q) placeholder="Paciente o documento";
                    label class="oculto" for="desde" { "Desde" }
                    input #desde type="date" name="desde" value=(f.desde);
                    label class="oculto" for="hasta" { "Hasta" }
                    input #hasta type="date" name="hasta" value=(f.hasta);
                    button type="submit" { "Filtrar" }
                    @if hay_filtro {
                        a class="enlace-sutil" href="/panel/incapacidades" { "Quitar" }
                    }
                }
            }

            @if filas.is_empty() {
                (piezas::vacio(
                    hay_filtro,
                    "Todavía no se ha expedido ninguna incapacidad.",
                    "Ninguna incapacidad coincide con el filtro.",
                    "/panel/incapacidades",
                ))
            } @else {
                section class="tarjeta resumen" {
                    div { span class="cifra" { (resumen.cantidad) } span class="sutil" { "incapacidades" } }
                    div { span class="cifra" { (resumen.dias_totales) } span class="sutil" { "días en total" } }
                    div {
                        span class="cifra" { (format!("{:.1}", resumen.promedio_dias)) }
                        span class="sutil" { "días de promedio" }
                    }
                }
                (piezas::tabla(
                    &["Fecha", "Historia", "Paciente", "Documento", "Días", "Desde", "Hasta", "Diagnóstico"],
                    html! {
                        @for i in &filas {
                            tr {
                                td class="mono" { (i.fecha_consulta) }
                                td {
                                    a href={ "/panel/incapacidades/" (i.historia_id) } {
                                        (i.codigo_historia)
                                    }
                                }
                                td { (i.paciente) }
                                td class="mono" { (i.identificacion) }
                                td { (i.dias) }
                                td class="mono" { (i.fecha_inicio.clone().unwrap_or_default()) }
                                td class="mono" { (i.fecha_fin.clone().unwrap_or_default()) }
                                td { (i.diagnostico) }
                            }
                        }
                    },
                ))
            }
        },
    )
    .into_response()
}

/// `GET /panel/incapacidades/{historia_id}` — el detalle.
///
/// **404 para la de otra institución**, indistinguible de una que no existe: la
/// misma regla que el resto del producto, y aquí importa igual — con un 403 se
/// enumeran las historias del vecino probando números.
pub async fn detalle(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let d = match medico_datos::incapacidad::detalle(&e.pool, u.ambito(), id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                pagina(
                    "No encontrada",
                    Some(crate::vista::panel::menu(&cx, "/panel/incapacidades")),
                    html! {
                        section class="tarjeta estrecha" {
                            h1 { "No se encontró esa incapacidad" }
                            p { a href="/panel/incapacidades" { "Volver" } }
                        }
                    },
                ),
            )
                .into_response()
        }
        Err(_) => return averia(),
    };

    // Es un acceso a dato clínico de UN paciente identificado, y desde el
    // 2026-08-08 deja eslabón: `incapacidad::Incapacidad` lleva ya el
    // `paciente_id` que la bitácora necesita para atarlo al titular —no lo
    // pedía ninguna pantalla, lo pide el rastro—. El `recurso_id` es el de la
    // HISTORIA, que es por donde se pide, igual que en la ruta JSON.
    crate::rutas::registrar_acceso(
        &e,
        &u,
        "ver",
        crate::rutas::RECURSO_INCAPACIDAD,
        id,
        d.base.paciente_id,
    )
    .await;

    pagina(
        &format!("Incapacidad {}", d.base.codigo_historia),
        Some(crate::vista::panel::menu(&cx, "/panel/incapacidades")),
        html! {
            (piezas::migas("/panel/incapacidades", "Incapacidades"))
            section class="tarjeta" {
                h1 { (d.base.paciente) }
                (piezas::datos(&[
                    ("Documento", Some(d.base.identificacion.clone())),
                    ("Historia", Some(d.base.codigo_historia.clone())),
                    ("Fecha de la consulta", Some(d.base.fecha_consulta.clone())),
                    ("Días", Some(d.base.dias.to_string())),
                    ("Desde", d.base.fecha_inicio.clone()),
                    ("Hasta", d.base.fecha_fin.clone()),
                    ("Diagnóstico", Some(d.base.diagnostico.clone())),
                    ("Diagnóstico relacionado", d.diagnostico_relacionado.clone()),
                    ("Profesional", Some(d.base.medico.clone())),
                    ("Motivo de consulta", d.motivo_consulta.clone()),
                    ("Observaciones", d.observaciones.clone()),
                ]))
            }
        },
    )
    .into_response()
}
