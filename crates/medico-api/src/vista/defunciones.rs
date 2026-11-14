// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Certificado de defunción (#249): la lista y la expedición.
//!
//! Gemelo de `nacimientos`, y por lo mismo: los campos son los del certificado,
//! ni uno más. Lo que cambia es el peso de la firma — aquí quien certifica queda
//! con nombre en un documento que alimenta la estadística vital y el registro
//! civil, así que la capacidad es la de FIRMAR clínico, no la de escribir.
//!
//! # La causa básica se pide aparte, y con su aviso a la vista
//!
//! El formulario podría rellenarla sola con la última de la cadena y casi
//! siempre acertaría. No lo hace: elegirla es un acto médico, y un acierto del
//! 90 % en estadística vital nacional es un 10 % de causas de muerte falsas que
//! nadie va a revisar.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::defuncion::{self, Nueva, MANERAS, SITIOS};

use crate::auth::{Autorizado, Estado, Firmar};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/defunciones";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub desde: String,
    #[serde(default)]
    pub hasta: String,
    #[serde(default)]
    pub aviso: String,
}

fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    match clave {
        "" => None,
        "certificada" => Some((
            Tono::Ok,
            "Certificado de defunción expedido.".to_string(),
        )),
        "anulada" => Some((
            Tono::Ok,
            "Certificado anulado. Sigue en la lista, marcado y con su motivo: un \
             certificado es prueba y no se borra. Ya puede expedirse uno nuevo."
                .to_string(),
        )),
        otra => defuncion::Falta::de_clave(otra).map(|f| (Tono::Error, f.mensaje().to_string())),
    }
}

fn de_vuelta(aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?aviso={aviso}")).into_response()
}

/// `GET /panel/defunciones`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let desde = (!f.desde.trim().is_empty()).then(|| f.desde.trim());
    let hasta = (!f.hasta.trim().is_empty()).then(|| f.hasta.trim());
    let filas = match defuncion::listar(&e.pool, u.ambito(), desde, hasta).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_DEFUNCIONES,
        None,
        filas.len(),
        vec![],
    )
    .await;

    let puede_certificar = cx.puede(medico_dominio::rol::Capacidad::FirmarClinico);
    let el_aviso = texto_del_aviso(f.aviso.trim());

    pagina(
        "Certificado de defunción",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Certificado de defunción" } }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="filtros" {
                    label { "Desde" input type="date" name="desde" value=(f.desde); }
                    label { "Hasta" input type="date" name="hasta" value=(f.hasta); }
                    button type="submit" { "Filtrar" }
                }
            }

            @if filas.is_empty() {
                (piezas::vacio(
                    desde.is_some() || hasta.is_some(),
                    "No hay certificados de defunción registrados.",
                    "Ninguno en ese rango de fechas.",
                    RUTA,
                ))
            } @else {
                (piezas::tabla(
                    &["Defunción", "Paciente", "Sitio", "Manera", "Causa básica", "Certifica", ""],
                    html! {
                        @for d in &filas {
                            tr class=@if d.anulada { "anulada" } @else { "" } {
                                td class="mono" { (d.fecha_defuncion) }
                                td {
                                    a href={ "/panel/pacientes/" (d.paciente_id) } { (d.paciente_nombre) }
                                    span class="sutil" { " " (d.paciente_identificacion) }
                                }
                                td { (etiqueta(&SITIOS, &d.sitio)) }
                                td { (etiqueta(&MANERAS, &d.manera)) }
                                td class="mono" { (d.causa_basica) }
                                td { (d.quien_certifica) }
                                td {
                                    @if d.anulada {
                                        span class="sutil" {
                                            "ANULADO: " (d.motivo_anulacion.clone().unwrap_or_default())
                                        }
                                    } @else if puede_certificar {
                                        form method="post" action={ (RUTA) "/" (d.id) "/anular" } {
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

            @if puede_certificar { (formulario()) }
        },
    )
    .into_response()
}

/// El nombre largo del código. Lo que no está en la tabla se enseña tal cual: si
/// entrara un valor por un importador, hay que verlo, no traducirlo a nada.
fn etiqueta(tabla: &[(&str, &str); 4], codigo: &str) -> String {
    tabla
        .iter()
        .find(|(c, _)| *c == codigo)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| codigo.to_string())
}

fn formulario() -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Expedir un certificado" }
            p class="sutil" {
                "La cadena de causas va de lo inmediato a lo remoto. La "
                strong { "causa básica" }
                " se declara aparte: el sistema no la deduce de la cadena, porque \
                 elegirla es un acto médico y es la que va a la estadística vital."
            }
            form method="post" action=(RUTA) class="apilado" {
                label { "Paciente (id)" input type="number" name="paciente_id" required min="1"; }
                label { "Fecha y hora de la defunción"
                    input type="datetime-local" name="fecha_defuncion" required; }
                label {
                    "Sitio"
                    select name="sitio" required {
                        @for (c, n) in SITIOS { option value=(c) { (n) } }
                    }
                }
                label {
                    "Manera de muerte"
                    select name="manera" required {
                        @for (c, n) in MANERAS { option value=(c) { (n) } }
                    }
                }
                label { "Causa directa (CIE-10)" input type="text" name="causa_directa" maxlength="6"; }
                label { "Antecedente a" input type="text" name="antecedente_a" maxlength="6"; }
                label { "Antecedente b" input type="text" name="antecedente_b" maxlength="6"; }
                label { "Antecedente c" input type="text" name="antecedente_c" maxlength="6"; }
                label {
                    "Causa básica (CIE-10) — obligatoria"
                    input type="text" name="causa_basica" required maxlength="6";
                }
                label { "Número del certificado" input type="text" name="numero_certificado"; }
                label { "Hospitalización (id, si murió durante un ingreso)"
                    input type="number" name="hospitalizacion_id" min="1"; }
                label { "Observaciones (obligatorias si la manera es violenta)"
                    input type="text" name="observaciones"; }
                button type="submit" { "Expedir certificado" }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Formulario {
    paciente_id: i64,
    #[serde(default)]
    hospitalizacion_id: Option<i64>,
    fecha_defuncion: String,
    sitio: String,
    manera: String,
    #[serde(default)]
    causa_directa: String,
    #[serde(default)]
    antecedente_a: String,
    #[serde(default)]
    antecedente_b: String,
    #[serde(default)]
    antecedente_c: String,
    causa_basica: String,
    #[serde(default)]
    numero_certificado: String,
    #[serde(default)]
    observaciones: String,
}

fn o_nada(s: String) -> Option<String> {
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// `POST /panel/defunciones`
pub async fn certificar(
    State(e): State<Estado>,
    autorizado: Autorizado<Firmar>,
    Form(f): Form<Formulario>,
) -> Response {
    let u = autorizado.0;
    let n = Nueva {
        paciente_id: f.paciente_id,
        hospitalizacion_id: f.hospitalizacion_id,
        atencion_urgencias_id: None,
        fecha_defuncion: f.fecha_defuncion,
        sitio: f.sitio,
        manera: f.manera,
        causa_directa: o_nada(f.causa_directa),
        antecedente_a: o_nada(f.antecedente_a),
        antecedente_b: o_nada(f.antecedente_b),
        antecedente_c: o_nada(f.antecedente_c),
        causa_basica: f.causa_basica,
        numero_certificado: o_nada(f.numero_certificado),
        observaciones: o_nada(f.observaciones),
    };
    match defuncion::certificar(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta("certificada"),
        Ok(Err(falta)) => de_vuelta(falta.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct MotivoAnulacion {
    motivo: String,
}

/// `POST /panel/defunciones/{id}/anular`
pub async fn anular(
    State(e): State<Estado>,
    autorizado: Autorizado<Firmar>,
    Path(id): Path<i64>,
    Form(f): Form<MotivoAnulacion>,
) -> Response {
    let u = autorizado.0;
    match defuncion::anular(&e.pool, u.ambito(), id, &f.motivo, u.id).await {
        Ok(Ok(())) => de_vuelta("anulada"),
        Ok(Err(falta)) => de_vuelta(falta.clave()),
        Err(_) => averia(),
    }
}
