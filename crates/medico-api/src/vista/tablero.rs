// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El tablero: las cifras del día, y las instituciones para el superadmin.
//!
//! Sale de `medico_datos::estadisticas::resumen` y de `institucion::listar`, las
//! MISMAS funciones que sirven `/api/estadisticas/resumen` y
//! `/api/instituciones`.
//!
//! # Por qué el tablero y superadmin están en el mismo archivo
//!
//! Porque son la misma pregunta con dos alcances: «cómo va esto». Para quien
//! pertenece a una institución, las cifras de su prestador; para el superadmin,
//! el listado de instituciones — que es lo único que le corresponde ver, porque
//! el panel clínico es de UN prestador (ver `vista::institucion_de`).
//!
//! # La tasa de ocupación NO se recalcula aquí
//!
//! Viene con el `Resumen`, y ya cuesta explicarla una vez. Recalcularla en la
//! pantalla dividiendo dos campos daría el mismo número hoy y otro el día que la
//! definición cambie —qué cuenta como cama disponible, si las de aislamiento
//! entran— y entonces habría dos tasas de ocupación con el mismo nombre. Es la
//! directiva 32: un valor con dos significados son dos campos, y aquí no hay dos.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, EnSesion};

fn cifra(valor: impl std::fmt::Display, etiqueta: &str) -> Markup {
    html! {
        div { span class="cifra" { (valor) } span class="sutil" { (etiqueta) } }
    }
}

/// `GET /panel/tablero`
pub async fn ver(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    // El superadmin no tiene institución, así que no tiene cifras de prestador.
    // **Se le manda a su consola**, y hasta el 2026-08-08 no era eso: esta
    // función le dibujaba aquí mismo un listado de instituciones de solo
    // lectura, sin menú y sin un solo enlace — o sea que entraba, veía los
    // nombres de sus clientes y ahí se acababa el producto para él. Con
    // `/panel/superadmin` viva, ese listado era además el MISMO listado en dos
    // sitios (directiva 1).
    if matches!(u.ambito(), medico_dominio::Ambito::Superadmin) {
        return axum::response::Redirect::to("/panel/superadmin").into_response();
    }
    // Lo demás —la cuenta sin institución y la avería de base— lo resuelve
    // `contexto` igual que en las otras trece pantallas.
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let r = match medico_datos::estadisticas::resumen(&e.pool, u.ambito()).await {
        Ok(r) => r,
        Err(_) => return averia(),
    };

    pagina(
        "Tablero",
        Some(crate::vista::panel::menu(&cx, "/panel/tablero")),
        html! {
            header class="titulo-seccion" {
                h1 { "Tablero" }
                p class="sutil" { "Al " (r.fecha) }
            }

            section class="tarjeta" {
                h2 { "Atención" }
                div class="resumen" {
                    (cifra(r.citas_hoy, "citas hoy"))
                    (cifra(r.citas_mes, "citas del mes"))
                    (cifra(r.citas_atendidas_mes, "atendidas"))
                    (cifra(format!("{:.1} %", r.tasa_asistencia), "asistencia"))
                    (cifra(r.consultas_mes, "consultas del mes"))
                    (cifra(r.total_pacientes, "pacientes"))
                }
            }

            section class="tarjeta" {
                h2 { "Servicio" }
                div class="resumen" {
                    (cifra(r.urgencias_activas, "en urgencias"))
                    (cifra(r.hospitalizados, "hospitalizados"))
                    (cifra(r.camas_ocupadas, "camas ocupadas"))
                    (cifra(r.camas_disponibles, "camas libres"))
                    (cifra(r.total_camas, "camas en total"))
                    // La tasa viene calculada del dominio. Ver la cabecera.
                    (cifra(format!("{:.1} %", r.tasa_ocupacion), "ocupación"))
                }
            }

            section class="tarjeta" {
                h2 { "Facturación" }
                div class="resumen" {
                    (cifra(r.facturas_pendientes, "facturas pendientes"))
                    (cifra(format!("$ {:.0}", r.valor_pendiente), "valor pendiente"))
                }
                p class="sutil" {
                    "«Pendiente» aquí es BORRADOR + RADICADA. El panel de facturación "
                    "usa otra definición para «facturado», y las dos son correctas: "
                    "cuál es LA cifra del producto está por decidir."
                }
            }
        },
    )
    .into_response()
}
