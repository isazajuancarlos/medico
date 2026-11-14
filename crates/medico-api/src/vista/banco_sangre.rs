// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Banco de sangre (#249): unidades y transfusiones.
//!
//! # La compatibilidad se enseña ANTES de ofrecer el botón
//!
//! La pantalla no deja elegir una unidad y luego rechaza: enseña por qué una
//! unidad no se puede transfundir —sin tamizar, vencida, ya usada— al lado de la
//! unidad. Descubrir la regla por un rechazo, con un paciente delante, es la
//! peor forma de enterarse.
//!
//! Lo que NO hace la pantalla es calcular la compatibilidad ABO por su cuenta:
//! eso lo decide `medico_dominio::sangre` en el servidor, y aquí solo se pide el
//! grupo del receptor. Dos implementaciones de la misma tabla acaban en
//! desacuerdo, y aquí el desacuerdo mata.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::banco_sangre::{self, NuevaTransfusion, NuevaUnidad};
use medico_dominio::sangre;

use crate::auth::{AdministrarInstitucion, AtenderUrgencias, Autorizado, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/banco-sangre";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
}

fn de_vuelta(aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?aviso={aviso}")).into_response()
}

/// `GET /panel/banco-sangre`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    axum::extract::Query(f): axum::extract::Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let hoy = medico_dominio::zona::hoy().format("%Y-%m-%d").to_string();
    let administra = cx.puede(medico_dominio::rol::Capacidad::AdministrarInstitucion);
    let transfunde = cx.puede(medico_dominio::rol::Capacidad::AtenderUrgenciasHospitalizacion);

    let unidades = match banco_sangre::listar_unidades(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let transfusiones = match banco_sangre::listar_transfusiones(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    crate::rutas::registrar_coleccion(
        &e, &u, "listar", crate::rutas::RECURSO_BANCO_SANGRE, None, transfusiones.len(), vec![],
    )
    .await;

    // Lo que el mensaje de aviso dice. Las faltas del banco llevan texto propio,
    // así que aquí solo se resuelven las tres confirmaciones.
    let el_aviso = match f.aviso.trim() {
        "" => None,
        "unidad" => Some((Tono::Ok, "Unidad registrada, en cuarentena hasta el tamizaje.".to_string())),
        "tamizada" => Some((Tono::Ok, "Tamizaje registrado.".to_string())),
        "transfundida" => Some((Tono::Ok, "Transfusión registrada.".to_string())),
        "reaccion" => Some((Tono::Ok, "Reacción adversa anotada.".to_string())),
        // Conjunto CERRADO: una clave que no exista no pinta nada, en vez de
        // dejar que un enlace le haga decir a la pantalla lo que quiera.
        otra => banco_sangre::mensaje_de_clave(otra).map(|m| (Tono::Error, m.to_string())),
    };

    let disponibles = unidades.iter().filter(|x| x.transfundible(&hoy).is_ok()).count();
    let vencidas = unidades.iter().filter(|x| x.vencida(&hoy) && x.estado != "TRANSFUNDIDA").count();
    let sin_tamizar = unidades.iter().filter(|x| x.tamizaje_conforme.is_none()).count();

    pagina(
        "Banco de sangre",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Banco de sangre" } }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            section class="tarjeta resumen" {
                div { span class="cifra" { (disponibles) } span class="sutil" { "transfundibles" } }
                div { span class="cifra" { (sin_tamizar) } span class="sutil" { "sin tamizar" } }
                div { span class="cifra" { (vencidas) } span class="sutil" { "vencidas" } }
            }

            section class="tarjeta" {
                h2 { "Unidades" }
                @if unidades.is_empty() {
                    (piezas::vacio(false, "No hay unidades registradas.", "", RUTA))
                } @else {
                    (piezas::tabla(
                        &["Código", "Componente", "Grupo", "Vence", "Estado", "¿Transfundible?"],
                        html! {
                            @for x in &unidades {
                                tr {
                                    td class="mono" { (x.codigo) }
                                    td { (x.hemocomponente) }
                                    td class="mono" { (x.grupo) (x.rh) }
                                    td class="mono" { (x.fecha_vencimiento) }
                                    td { (piezas::estado(&x.estado)) }
                                    td {
                                        // El motivo, al lado de la unidad. Nadie
                                        // debería descubrir la regla por un
                                        // rechazo con un paciente delante.
                                        @match x.transfundible(&hoy) {
                                            Ok(()) => (piezas::estado("SI")),
                                            Err(m) => span class="sutil" { (m) },
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
                @if administra { (formulario_unidad()) (formulario_tamizaje()) }
            }

            section class="tarjeta" {
                h2 { "Transfusiones" }
                @if transfusiones.is_empty() {
                    (piezas::vacio(false, "No hay transfusiones registradas.", "", RUTA))
                } @else {
                    (piezas::tabla(
                        &["Fecha", "Paciente", "Unidad", "Componente", "Unidad→Receptor", "Reacción", ""],
                        html! {
                            @for t in &transfusiones {
                                tr {
                                    td class="mono" { (t.fecha) }
                                    td { a href={ "/panel/pacientes/" (t.paciente_id) } { (t.paciente) } }
                                    td class="mono" { (t.codigo_unidad) }
                                    td { (t.hemocomponente) }
                                    td class="mono" {
                                        (t.grupo_unidad) (t.rh_unidad) " → " (t.grupo_receptor) (t.rh_receptor)
                                    }
                                    td {
                                        @if t.reaccion_adversa {
                                            (piezas::estado("SI"))
                                            " " span class="sutil" { (t.descripcion_reaccion.clone().unwrap_or_default()) }
                                        }
                                    }
                                    td {
                                        @if transfunde && !t.reaccion_adversa {
                                            form method="post" action={ (RUTA) "/" (t.id) "/reaccion" } {
                                                input type="text" name="descripcion"
                                                      placeholder="describir la reacción" required;
                                                button type="submit" { "Anotar reacción" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
                @if transfunde { (formulario_transfusion()) }
            }
        },
    )
    .into_response()
}

fn formulario_unidad() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/unidades" } class="apilado" {
            h3 { "Registrar una unidad" }
            label { "Código" input type="text" name="codigo" required; }
            label {
                "Hemocomponente"
                select name="hemocomponente" required {
                    option value="GLOBULOS_ROJOS" { "Glóbulos rojos" }
                    option value="PLASMA" { "Plasma" }
                    option value="PLAQUETAS" { "Plaquetas" }
                    option value="CRIOPRECIPITADO" { "Crioprecipitado" }
                }
            }
            label {
                "Grupo"
                select name="grupo" required { @for g in sangre::GRUPOS { option value=(g) { (g) } } }
            }
            label {
                "RH"
                select name="rh" required { @for r in sangre::RH { option value=(r) { (r) } } }
            }
            label { "Fecha de obtención" input type="date" name="fecha_obtencion"; }
            label { "Vencimiento" input type="date" name="fecha_vencimiento" required; }
            p class="sutil" { "Entra EN CUARENTENA: no se puede transfundir hasta tamizarla." }
            button type="submit" { "Registrar unidad" }
        }
    }
}

fn formulario_tamizaje() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/tamizaje" } class="apilado" {
            h3 { "Registrar tamizaje" }
            label { "Unidad (id)" input type="number" name="unidad_id" required min="1"; }
            label { input type="checkbox" name="conforme" value="1" checked; " Conforme" }
            label {
                "Si NO es conforme, el motivo del descarte"
                input type="text" name="motivo";
                span class="sutil" { " No conforme DESCARTA la unidad." }
            }
            button type="submit" { "Registrar tamizaje" }
        }
    }
}

fn formulario_transfusion() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/transfusiones" } class="apilado" {
            h3 { "Transfundir" }
            label { "Unidad (id)" input type="number" name="unidad_id" required min="1"; }
            label { "Paciente (id)" input type="number" name="paciente_id" required min="1"; }
            label {
                "Grupo del RECEPTOR (confirmado ahora)"
                select name="grupo_receptor" required {
                    option value="" { "—" }
                    @for g in sangre::GRUPOS { option value=(g) { (g) } }
                }
            }
            label {
                "RH del receptor"
                select name="rh_receptor" required {
                    option value="" { "—" }
                    @for r in sangre::RH { option value=(r) { (r) } }
                }
            }
            p class="sutil" {
                "El grupo se confirma con pruebas cruzadas en cada transfusión. El sistema NO lo \
                 recuerda de una vez anterior a propósito: transfundir contra un dato guardado \
                 hace meses es el error que mata."
            }
            label { "Observaciones" input type="text" name="observaciones"; }
            button type="submit" { "Transfundir" }
        }
    }
}

fn o_nada(s: String) -> Option<String> {
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

#[derive(Deserialize)]
pub struct FormUnidad {
    codigo: String,
    hemocomponente: String,
    grupo: String,
    rh: String,
    #[serde(default)]
    fecha_obtencion: String,
    fecha_vencimiento: String,
}

/// `POST /panel/banco-sangre/unidades`
pub async fn crear_unidad(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormUnidad>,
) -> Response {
    let n = NuevaUnidad {
        codigo: f.codigo,
        hemocomponente: f.hemocomponente,
        grupo: f.grupo,
        rh: f.rh,
        fecha_obtencion: o_nada(f.fecha_obtencion),
        fecha_vencimiento: f.fecha_vencimiento,
    };
    match banco_sangre::registrar_unidad(&e.pool, autorizado.0.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("unidad"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormTamizaje {
    unidad_id: i64,
    #[serde(default)]
    conforme: Option<String>,
    #[serde(default)]
    motivo: String,
}

/// `POST /panel/banco-sangre/tamizaje`
pub async fn tamizar(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormTamizaje>,
) -> Response {
    let motivo = o_nada(f.motivo);
    match banco_sangre::tamizar(
        &e.pool,
        autorizado.0.ambito(),
        f.unidad_id,
        f.conforme.is_some(),
        motivo.as_deref(),
    )
    .await
    {
        Ok(Ok(())) => de_vuelta("tamizada"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormTransfusion {
    unidad_id: i64,
    paciente_id: i64,
    grupo_receptor: String,
    rh_receptor: String,
    #[serde(default)]
    observaciones: String,
}

/// `POST /panel/banco-sangre/transfusiones`
pub async fn transfundir(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<FormTransfusion>,
) -> Response {
    let u = autorizado.0;
    let hoy = medico_dominio::zona::hoy().format("%Y-%m-%d").to_string();
    let n = NuevaTransfusion {
        unidad_id: f.unidad_id,
        paciente_id: f.paciente_id,
        grupo_receptor: f.grupo_receptor,
        rh_receptor: f.rh_receptor,
        observaciones: o_nada(f.observaciones),
    };
    match banco_sangre::transfundir(&e.pool, u.ambito(), &n, u.id, &hoy).await {
        Ok(Ok(_)) => de_vuelta("transfundida"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormReaccion {
    descripcion: String,
}

/// `POST /panel/banco-sangre/{id}/reaccion`
pub async fn reaccion(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Path(id): Path<i64>,
    Form(f): Form<FormReaccion>,
) -> Response {
    match banco_sangre::anotar_reaccion(&e.pool, autorizado.0.ambito(), id, &f.descripcion).await {
        Ok(Ok(())) => de_vuelta("reaccion"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}
