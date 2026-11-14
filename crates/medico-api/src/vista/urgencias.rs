// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Urgencias: la cola de triage.
//!
//! Sale de `medico_datos::urgencias::cola`, la MISMA función que sirve
//! `GET /api/urgencias/cola`.
//!
//! # El triage manda el ORDEN, y el color no es decoración
//!
//! La cola llega ordenada por gravedad desde la consulta; aquí no se reordena.
//! Reordenar en la pantalla es cómo dos vistas del mismo servicio acaban
//! discrepando sobre a quién le toca — y en urgencias eso no es un detalle de
//! interfaz.
//!
//! Los cinco niveles se pintan con su propia escala y no con la pastilla de
//! estado genérica: aquí el «I» es lo más grave y el «V» lo más leve, al revés
//! que en cualquier otra pantalla, donde lo verde es lo bueno. Usar la misma
//! paleta habría pintado de rojo al que puede esperar.
//!
//! # Lo que todavía NO hace
//!
//! Registrar el triage y dar el egreso, que son las dos escrituras del módulo.
//! El formulario `maud` aún no existe: hoy `POST /api/urgencias/triage` y
//! `PUT /…/egreso` se atienden SOLO por API (JSON), sin pantalla — es la misma
//! limitación que `diagnostico_ingreso`/`_egreso` y el `tipo_diagnostico`, que se
//! capturan por esos endpoints y no por una pantalla. (El frontal Angular que
//! los servía se retiró el 2026-08-10; la pantalla entra con la migración de la
//! captura de la historia.)

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

/// El nivel de triage, con su escala propia.
///
/// `I` es la emergencia y `V` la consulta que puede esperar. Lo que no encaje en
/// los cinco niveles sale neutro: un nivel desconocido pintado de verde diría
/// «puede esperar» sobre alguien de quien no se sabe nada.
fn nivel(t: &str) -> Markup {
    let clase = match t.trim().to_uppercase().as_str() {
        "I" | "1" => "triage triage-1",
        "II" | "2" => "triage triage-2",
        "III" | "3" => "triage triage-3",
        "IV" | "4" => "triage triage-4",
        "V" | "5" => "triage triage-5",
        _ => "triage",
    };
    html! { span class=(clase) { (t) } }
}

/// `GET /panel/urgencias`
pub async fn cola(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let cola = match medico_datos::urgencias::cola(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    // La bitácora, con los MISMOS argumentos que `GET /api/urgencias/cola` —el
    // nombre del recurso sale de la constante que comparten las dos puertas—.
    // Si la pantalla anotara distinto, el rastro del mismo hecho quedaría
    // partido según por dónde se abrió.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_URGENCIAS,
        None,
        cola.len(),
        vec![],
    )
    .await;

    // Los que siguen dentro: sin fecha de egreso. Es el número que se mira al
    // entrar al turno, y se cuenta sobre el mismo conjunto que la tabla.
    let dentro = cola.iter().filter(|a| a.fecha_egreso.is_none()).count();

    pagina(
        "Urgencias",
        Some(crate::vista::panel::menu(&cx, "/panel/urgencias")),
        html! {
            header class="titulo-seccion" { h1 { "Urgencias" } }

            @if cola.is_empty() {
                (piezas::vacio(
                    false,
                    "No hay atenciones de urgencias registradas.",
                    "",
                    "/panel/urgencias",
                ))
            } @else {
                section class="tarjeta resumen" {
                    div { span class="cifra" { (dentro) } span class="sutil" { "en el servicio" } }
                    div { span class="cifra" { (cola.len()) } span class="sutil" { "en la cola" } }
                }
                (piezas::tabla(
                    &["Triage", "Ingreso", "Paciente", "Motivo", "Diagnóstico", "Egreso", "Destino", "RDA"],
                    html! {
                        @for a in &cola {
                            tr {
                                td { (nivel(&a.triage)) }
                                td class="mono" { (a.fecha_ingreso) }
                                td {
                                    a href={ "/panel/pacientes/" (a.paciente_id) } {
                                        "Paciente " (a.paciente_id)
                                    }
                                }
                                td { (a.motivo_consulta) }
                                td { (a.diagnostico_ingreso.clone().unwrap_or_default()) }
                                // SIN egreso se deja vacío: la atención no ha
                                // terminado. Poner la hora de ahora es lo que
                                // hacía el RIPS antes de corregirlo, y reportaba
                                // como dado de alta a quien seguía en el box.
                                td class="mono" { (a.fecha_egreso.clone().unwrap_or_default()) }
                                td { (a.destino_salida.clone().unwrap_or_default()) }
                                td {
                                    a href={ "/panel/rda/urgencias/" (a.id) } { "Preparar RDA" }
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
