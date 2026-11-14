// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Censo diario de hospitalización (#249).
//!
//! Es el papel que se lee cada mañana: quién estaba, quién entró, quién salió y
//! quién queda. No tiene tabla — se deriva de `hospitalizacion` y `cama`.
//!
//! # El descuadre se enseña ARRIBA, no en una nota al pie
//!
//! Un censo que no cuadra está mal, y quien lo firma tiene que verlo antes de
//! firmarlo. Por eso el aviso va antes de los números y no debajo: un dato roto
//! escondido al final de la página es un dato roto que se firma igual.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use medico_datos::censo;

use crate::auth::Estado;
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/censo";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub fecha: String,
}

/// `GET /panel/censo?fecha=AAAA-MM-DD`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    // Sin fecha, HOY. Aquí sí es legítimo el defecto —el censo de hoy es lo que
    // se pide el 99 % de las veces— y no es relleno de un dato ausente: es el
    // valor por omisión de un filtro, y se enseña en la casilla para que se vea
    // cuál se está mirando.
    let hoy = medico_dominio::zona::hoy().format("%Y-%m-%d").to_string();
    let fecha = if f.fecha.trim().is_empty() { hoy } else { f.fecha.trim().to_string() };

    let c = match censo::del_dia(&e.pool, u.ambito(), &fecha).await {
        Ok(Some(c)) => c,
        // `None` = la fecha no se puede leer, o el ámbito no ve nada. Se dice,
        // en vez de enseñar un censo vacío que se lee como «no hubo nadie».
        Ok(None) => {
            return pagina(
                "Censo diario",
                Some(crate::vista::panel::menu(&cx, RUTA)),
                html! {
                    header class="titulo-seccion" { h1 { "Censo diario" } }
                    section class="tarjeta" {
                        (aviso(Tono::Error, &format!(
                            "No se puede armar el censo de «{fecha}». La fecha va en formato \
                             AAAA-MM-DD."
                        )))
                        (buscador(&fecha))
                    }
                },
            )
            .into_response()
        }
        Err(_) => return averia(),
    };

    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_CENSO,
        Some(format!("censo del {fecha}")),
        c.camas_ocupadas as usize,
        vec![],
    )
    .await;

    pagina(
        "Censo diario",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Censo diario" } }

            // El descuadre, ANTES de los números.
            @for d in &c.descuadres {
                section class="tarjeta" { (aviso(Tono::Error, d)) }
            }

            section class="tarjeta" { (buscador(&c.fecha)) }

            section class="tarjeta resumen" {
                div { span class="cifra" { (c.existencia_inicial) } span class="sutil" { "existencia inicial" } }
                div { span class="cifra" { (c.ingresos) } span class="sutil" { "ingresos" } }
                div { span class="cifra" { (c.egresos.total()) } span class="sutil" { "egresos" } }
                div { span class="cifra" { (c.existencia_final) } span class="sutil" { "existencia final" } }
            }

            section class="tarjeta" {
                h2 { "Egresos del día" }
                (piezas::tabla(
                    &["Alta médica", "Alta voluntaria", "Remisión", "Defunción", "Sin clasificar"],
                    html! {
                        tr {
                            td class="mono" { (c.egresos.alta_medica) }
                            td class="mono" { (c.egresos.alta_voluntaria) }
                            td class="mono" { (c.egresos.remision) }
                            td class="mono" { (c.egresos.defuncion) }
                            td class="mono" { (c.egresos.sin_clasificar) }
                        }
                    },
                ))
            }

            section class="tarjeta" {
                h2 { "Camas" }
                p class="sutil" {
                    "Ocupación AHORA: " (c.camas_ocupadas) " de " (c.camas_totales)
                    @if let Some(o) = c.ocupacion { " (" (format!("{o:.1}")) " %)" }
                    ". Es la foto de este momento, no la del día que se consulta: una cama no \
                     guarda su historia."
                }
                @if c.camas.is_empty() {
                    (piezas::vacio(false, "Esta institución no tiene camas registradas.", "", RUTA))
                } @else {
                    (piezas::tabla(
                        &["Cama", "Piso", "Servicio", "Estado", "Paciente", "Desde"],
                        html! {
                            @for cm in &c.camas {
                                tr {
                                    td class="mono" { (cm.numero) }
                                    td { (cm.piso) }
                                    td { (cm.servicio) }
                                    td { (piezas::estado(&cm.estado)) }
                                    td {
                                        @match (&cm.paciente, cm.paciente_id) {
                                            (Some(n), Some(id)) => {
                                                a href={ "/panel/pacientes/" (id) } { (n) }
                                            }
                                            _ => "",
                                        }
                                    }
                                    td class="mono" { (cm.fecha_ingreso.clone().unwrap_or_default()) }
                                }
                            }
                        },
                    ))
                }
            }
        },
    )
    .into_response()
}

fn buscador(fecha: &str) -> maud::Markup {
    html! {
        form method="get" action=(RUTA) class="filtros" {
            label { "Día" input type="date" name="fecha" value=(fecha); }
            button type="submit" { "Ver el censo" }
        }
    }
}
