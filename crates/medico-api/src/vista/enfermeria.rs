// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El turno de enfermería: kardex, hoja de administración, notas y plan de
//! cuidados (#249, bloque hospitalario).
//!
//! # Una pantalla, CUATRO módulos, y cada sección mira su licencia
//!
//! `KARDEX`, `MAR`, `ENFERMERIA` y `PLAN_CUIDADO` son cuatro módulos del catálogo
//! y se licencian por separado, pero son **un solo momento de trabajo**: la
//! enfermera del turno mira lo que hay que poner, lo pone y lo anota. Partirlo en
//! cuatro pantallas obligaría a navegar tres veces para hacer una cosa.
//!
//! Lo que no se hace es enseñar lo que no se compró: cada sección se dibuja solo
//! si `cx.tiene(…)`. Sin eso, la pantalla ofrecería un formulario que la API
//! rechaza con 403 — que es enseñar la regla dos veces y en desacuerdo.
//!
//! # El kardex y la hoja son la MISMA verdad mirada de dos formas
//!
//! El kardex es «qué hay ordenado y cuánto se ha cumplido»; la hoja, «qué pasó,
//! en orden». Se dibujan juntos a propósito: el número del kardex sale de contar
//! las filas de la hoja, así que si alguna vez discrepan, se ve aquí.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::enfermeria::{self, NuevaAdministracion, ESTADOS};

use crate::auth::{Autorizado, EscribirEnfermeria, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/enfermeria";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub paciente_id: Option<i64>,
    #[serde(default)]
    pub aviso: String,
}

fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    match clave {
        "" => None,
        "registrada" => Some((
            Tono::Ok,
            "Administración registrada en la hoja del paciente.".to_string(),
        )),
        "anulada" => Some((
            Tono::Ok,
            "Registro anulado. La fila sigue en la hoja, marcada y con su motivo: \
             un MAR no se borra."
                .to_string(),
        )),
        otra => enfermeria::Falta::de_clave(otra).map(|f| (Tono::Error, f.mensaje().to_string())),
    }
}

/// `GET /panel/enfermeria?paciente_id=`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    // Sin paciente no hay turno que enseñar. **No se elige uno por defecto**: la
    // pantalla del primer paciente de la lista sería un dato clínico de alguien
    // que nadie pidió ver, y quedaría anotado en la bitácora a su nombre.
    let Some(paciente_id) = f.paciente_id else {
        return pagina(
            "Enfermería",
            Some(crate::vista::panel::menu(&cx, RUTA)),
            html! {
                header class="titulo-seccion" { h1 { "Enfermería" } }
                section class="tarjeta" {
                    p { "Elija un paciente para ver su kardex y su hoja de administración." }
                    form method="get" action=(RUTA) class="filtros" {
                        label { "Paciente (id)" input type="number" name="paciente_id" required min="1"; }
                        button type="submit" { "Abrir el turno" }
                    }
                }
            },
        )
        .into_response();
    };

    let puede_escribir = cx.puede(medico_dominio::rol::Capacidad::EscribirEnfermeria);
    let el_aviso = texto_del_aviso(f.aviso.trim());

    let kardex = if cx.tiene("KARDEX") {
        match enfermeria::kardex(&e.pool, u.ambito(), paciente_id).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };
    let hoja = if cx.tiene("MAR") {
        match enfermeria::hoja_del_paciente(&e.pool, u.ambito(), paciente_id).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };

    // La bitácora, una vez por recurso mirado y atada al TITULAR.
    if cx.tiene("KARDEX") {
        crate::rutas::registrar_coleccion_de_un_titular(
            &e, &u, "listar", crate::rutas::RECURSO_KARDEX, paciente_id, None, kardex.len(),
        )
        .await;
    }
    if cx.tiene("MAR") {
        crate::rutas::registrar_coleccion_de_un_titular(
            &e, &u, "listar", crate::rutas::RECURSO_MAR, paciente_id, None, hoja.len(),
        )
        .await;
    }

    pagina(
        "Enfermería",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" {
                h1 { "Enfermería" }
                (piezas::migas(&format!("/panel/pacientes/{paciente_id}"), "Ver la ficha del paciente"))
            }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            @if cx.tiene("KARDEX") {
                section class="tarjeta" {
                    h2 { "Kardex" }
                    p class="sutil" {
                        "Lo ordenado y su cumplimiento. No es una tabla propia: sale de las \
                         órdenes del paciente y de lo que se ha anotado de cada una."
                    }
                    @if kardex.is_empty() {
                        (piezas::vacio(false, "Este paciente no tiene órdenes de medicamento.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Medicamento", "Dosis ordenada", "Puestas", "Omitidas", "Última"],
                            html! {
                                @for k in &kardex {
                                    tr {
                                        td { (k.medicamento) }
                                        td { (k.dosis) }
                                        td class="mono" { (k.administradas) }
                                        // Las omitidas van al lado y no escondidas:
                                        // un kardex que solo contara lo puesto
                                        // escondería justo lo que hay que mirar.
                                        td class="mono" { (k.omitidas) }
                                        td class="mono" { (k.ultima.clone().unwrap_or_default()) }
                                    }
                                }
                            },
                        ))
                    }
                }
            }

            @if cx.tiene("MAR") {
                section class="tarjeta" {
                    h2 { "Hoja de administración" }
                    @if hoja.is_empty() {
                        (piezas::vacio(false, "Todavía no hay nada anotado en este turno.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Hora", "Medicamento", "Estado", "Dosis", "Vía", "Quién", "Motivo", ""],
                            html! {
                                @for a in &hoja {
                                    tr class=@if a.anulada { "anulada" } @else { "" } {
                                        td class="mono" { (a.fecha) }
                                        td { (a.medicamento) }
                                        td { (piezas::estado(&a.estado)) }
                                        td { (a.dosis_administrada.clone().unwrap_or_default()) }
                                        td { (a.via.clone().unwrap_or_default()) }
                                        td { (a.quien) }
                                        td {
                                            @if a.anulada {
                                                "ANULADA: " (a.motivo_anulacion.clone().unwrap_or_default())
                                            } @else {
                                                (a.motivo.clone().unwrap_or_default())
                                            }
                                        }
                                        td {
                                            // No hay botón de BORRAR, y no es un
                                            // olvido: un MAR es prueba. Lo que hay
                                            // es anular, con motivo obligatorio.
                                            @if puede_escribir && !a.anulada {
                                                form method="post"
                                                     action={ (RUTA) "/" (a.id) "/anular" } {
                                                    input type="hidden" name="paciente_id" value=(paciente_id);
                                                    input type="text" name="motivo"
                                                          placeholder="motivo de la anulación" required;
                                                    button type="submit" { "Anular" }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }

                    @if puede_escribir && !kardex.is_empty() {
                        (formulario(paciente_id, &kardex))
                    }
                }
            }
        },
    )
    .into_response()
}

fn formulario(paciente_id: i64, kardex: &[enfermeria::LineaKardex]) -> Markup {
    html! {
        form method="post" action=(RUTA) class="apilado" {
            h3 { "Anotar una administración" }
            input type="hidden" name="paciente_id" value=(paciente_id);
            label {
                "Orden"
                select name="formula_id" required {
                    @for k in kardex {
                        option value=(k.formula_id) { (k.medicamento) " — " (k.dosis) }
                    }
                }
            }
            label {
                "Estado"
                select name="estado" required {
                    @for est in ESTADOS { option value=(est) { (est) } }
                }
            }
            label { "Dosis administrada (obligatoria si se puso)"
                input type="text" name="dosis_administrada"; }
            label { "Vía" input type="text" name="via"; }
            label { "Motivo (obligatorio si NO se puso)"
                input type="text" name="motivo"; }
            label { "Hora a la que estaba programada"
                input type="datetime-local" name="programada_para"; }
            label { "Observaciones" input type="text" name="observaciones"; }
            button type="submit" { "Anotar" }
        }
    }
}

#[derive(Deserialize)]
pub struct FormularioAdministracion {
    paciente_id: i64,
    formula_id: i64,
    estado: String,
    #[serde(default)]
    dosis_administrada: String,
    #[serde(default)]
    via: String,
    #[serde(default)]
    motivo: String,
    #[serde(default)]
    programada_para: String,
    #[serde(default)]
    observaciones: String,
}

fn o_nada(s: String) -> Option<String> {
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn de_vuelta(paciente_id: i64, aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?paciente_id={paciente_id}&aviso={aviso}")).into_response()
}

/// `POST /panel/enfermeria`
pub async fn anotar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirEnfermeria>,
    Form(f): Form<FormularioAdministracion>,
) -> Response {
    let u = autorizado.0;
    // La pantalla la abren las cuatro licencias del bloque; esta escritura entra
    // en `administracion_medicamento`, que es MAR y solo MAR.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "MAR").await {
        return r;
    }
    let paciente = f.paciente_id;
    let n = NuevaAdministracion {
        formula_id: f.formula_id,
        // El ingreso se enlazará cuando la pantalla se abra DESDE la
        // hospitalización. Hoy no se adivina cuál es (directiva 20).
        hospitalizacion_id: None,
        programada_para: o_nada(f.programada_para),
        estado: f.estado,
        motivo: o_nada(f.motivo),
        dosis_administrada: o_nada(f.dosis_administrada),
        via: o_nada(f.via),
        observaciones: o_nada(f.observaciones),
    };
    match enfermeria::administrar(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta(paciente, "registrada"),
        Ok(Err(falta)) => de_vuelta(paciente, falta.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormularioAnulacion {
    paciente_id: i64,
    motivo: String,
}

/// `POST /panel/enfermeria/{id}/anular`
pub async fn anular(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirEnfermeria>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Form(f): Form<FormularioAnulacion>,
) -> Response {
    let u = autorizado.0;
    // Anular una administración es escribir en MAR, igual que ponerla.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "MAR").await {
        return r;
    }
    match enfermeria::anular(&e.pool, u.ambito(), id, &f.motivo, u.id).await {
        Ok(Ok(())) => de_vuelta(f.paciente_id, "anulada"),
        Ok(Err(falta)) => de_vuelta(f.paciente_id, falta.clave()),
        Err(_) => averia(),
    }
}
