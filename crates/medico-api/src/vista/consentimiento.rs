// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Consentimiento informado: el listado con su veredicto de validez.
//!
//! # Lo que esta pantalla NO decide
//!
//! Si un consentimiento vale. Eso lo dice `medico_dominio::consentimiento`, que
//! ya viaja dentro de cada `Lectura` (`valido`, `faltantes`, `resumen`) — la
//! misma respuesta que da `GET /api/consentimiento`. Aquí solo se dibuja.
//!
//! Copiar el criterio a la pantalla sería la peor versión de la directiva 1: un
//! documento con efectos jurídicos (Ley 23/1981, Resolución 1995/1999) cuya
//! validez se juzga en dos sitios que se separan al primer arreglo, y con la
//! pantalla diciendo «válido» sobre algo que el sistema considera incompleto.
//!
//! # El revocado se ve, y se ve DISTINTO de lo inválido
//!
//! Un consentimiento revocado fue válido y el paciente se echó atrás; uno
//! inválido nunca llegó a serlo porque le faltan datos. Las dos cosas se
//! corrigen de forma distinta —una no se corrige— y por eso no comparten
//! etiqueta.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

#[derive(Deserialize, Default)]
pub struct Filtro {
    /// Solo los que hoy son válidos.
    #[serde(default)]
    pub validos: String,
}

/// `GET /panel/consentimiento`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let solo_validos = f.validos == "1";
    let filas = match medico_datos::consentimiento::listar(&e.pool, u.ambito(), None, solo_validos)
        .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // Este listado atraviesa titulares y devuelve nombre y documento de cada
    // uno: es una `Coleccion` de la Ley 1581, igual que su gemelo
    // `GET /api/consentimiento`. Con los MISMOS argumentos —incluida la acción
    // en mayúsculas, que es la que aquél escribe—: un verbo distinto partiría en
    // dos el rastro del mismo hecho según por qué puerta se mirase.
    let filtro = solo_validos.then(|| "solo_validos".to_string());
    crate::rutas::registrar_coleccion(&e, &u, "LISTAR", "consentimiento", filtro, filas.len(), vec![])
        .await;

    pagina(
        "Consentimientos",
        Some(crate::vista::panel::menu(&cx, "/panel/consentimiento")),
        html! {
            header class="titulo-seccion" {
                h1 { "Consentimientos informados" }
                nav class="buscador" {
                    a class=[(!solo_validos).then_some("enlace-activo")]
                      href="/panel/consentimiento" { "Todos" }
                    a class=[solo_validos.then_some("enlace-activo")]
                      href="/panel/consentimiento?validos=1" { "Solo válidos" }
                }
            }

            @if filas.is_empty() {
                (piezas::vacio(
                    solo_validos,
                    "Todavía no se ha registrado ningún consentimiento.",
                    "No hay consentimientos válidos ahora mismo.",
                    "/panel/consentimiento",
                ))
            } @else {
                (piezas::recuento(filas.len(), filas.len(), "consentimiento", "consentimientos"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Procedimiento", "CUPS", "Profesional", "Estado"],
                    html! {
                        @for c in &filas {
                            tr {
                                td class="mono" { (c.fila.fecha_firma) }
                                td {
                                    a href={ "/panel/pacientes/" (c.fila.paciente_id) } {
                                        (c.paciente.clone().unwrap_or_default())
                                    }
                                }
                                td { (c.fila.descripcion_procedimiento) }
                                td class="mono" { (c.fila.codigo_cups.clone().unwrap_or_default()) }
                                td { (c.medico.clone().unwrap_or_default()) }
                                td {
                                    // REVOCADO e INVÁLIDO no comparten etiqueta:
                                    // uno fue válido y el paciente se echó atrás;
                                    // el otro nunca lo fue. Se corrigen distinto
                                    // —el primero no se corrige— y mezclarlos
                                    // mandaría a completar un documento que ya
                                    // no sirve para nada.
                                    @if c.fila.revocado {
                                        (piezas::estado("REVOCADO"))
                                    } @else if c.validez.valido {
                                        (piezas::estado("VALIDO"))
                                    } @else {
                                        (piezas::estado("INCOMPLETO"))
                                    }
                                }
                            }
                        }
                    },
                ))

                // Lo que falta, dicho por el DOMINIO y no por la pantalla. Sin
                // esto, «incompleto» es un veredicto sin qué hacer con él.
                @let incompletos: Vec<_> = filas
                    .iter()
                    .filter(|c| !c.fila.revocado && !c.validez.valido)
                    .collect();
                @if !incompletos.is_empty() {
                    section class="tarjeta" {
                        h2 { "Qué les falta a los incompletos" }
                        ul {
                            @for c in &incompletos {
                                li {
                                    strong { (c.fila.descripcion_procedimiento) }
                                    " — " (c.validez.resumen)
                                }
                            }
                        }
                    }
                }
            }
        },
    )
    .into_response()
}
