// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Telemedicina (#249): marcar una atención como teleconsulta.
//!
//! La modalidad que se elija aquí **es la que sale al RIPS y al RDA**: la capa de
//! datos la escribe también en `historia`. Se dice en la pantalla porque quien la
//! usa tiene que saber que está tocando lo que se factura.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::html;
use serde::Deserialize;

use medico_datos::teleconsulta::{self, NuevaTeleconsulta, MODALIDADES};

use crate::auth::{Autorizado, EscribirClinico, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/teleconsultas";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
}

fn de_vuelta(aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?aviso={aviso}")).into_response()
}

/// `GET /panel/teleconsultas`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let puede = cx.puede(medico_dominio::rol::Capacidad::EscribirClinico);
    let filas = match teleconsulta::listar(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    crate::rutas::registrar_coleccion(
        &e, &u, "listar", crate::rutas::RECURSO_TELECONSULTA, None, filas.len(), vec![],
    )
    .await;

    let el_aviso = match f.aviso.trim() {
        "" => None,
        "registrada" => Some((
            Tono::Ok,
            "Teleconsulta registrada. La modalidad quedó también en la atención, que es de \
             donde la leen el RIPS y el RDA."
                .to_string(),
        )),
        otra => teleconsulta::Falta::de_clave(otra)
            .map(|x| (Tono::Error, x.mensaje().to_string())),
    };

    pagina(
        "Telemedicina",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Telemedicina" } }
            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }
            @if filas.is_empty() {
                (piezas::vacio(false, "No hay teleconsultas registradas.", "", RUTA))
            } @else {
                (piezas::tabla(
                    &["Atención", "Paciente", "Modalidad", "Otro extremo", "Plataforma", "Incidencias"],
                    html! {
                        @for t in &filas {
                            tr {
                                td class="mono" { (t.fecha_atencion) }
                                td { a href={ "/panel/pacientes/" (t.paciente_id) } { (t.paciente) } }
                                td {
                                    (MODALIDADES.iter().find(|(c,_)| *c == t.modalidad)
                                        .map(|(_,n)| *n).unwrap_or(t.modalidad.as_str()))
                                }
                                td {
                                    (t.institucion_remota.clone()
                                        .or_else(|| t.profesional_remoto.clone())
                                        .unwrap_or_default())
                                }
                                td { (t.plataforma.clone().unwrap_or_default()) }
                                td { (t.incidencias.clone().unwrap_or_default()) }
                            }
                        }
                    },
                ))
            }
            @if puede {
                section class="tarjeta" {
                    form method="post" action=(RUTA) class="apilado" {
                        h3 { "Marcar una atención como teleconsulta" }
                        label { "Atención (id de historia)"
                            input type="number" name="historia_id" required min="1"; }
                        label {
                            "Modalidad"
                            select name="modalidad" required {
                                @for (c, n) in MODALIDADES { option value=(c) { (n) } }
                            }
                        }
                        label { "Institución del otro extremo"
                            input type="text" name="institucion_remota"; }
                        label { "Profesional del otro extremo"
                            input type="text" name="profesional_remoto"; }
                        p class="sutil" {
                            "Hace falta al menos uno de los dos: sin saber quién está al otro \
                             lado, esta ficha no dice nada que la atención no dijera ya."
                        }
                        label { "Plataforma" input type="text" name="plataforma"; }
                        label { "Consentimiento (id)" input type="number" name="consentimiento_id" min="1"; }
                        label { "Incidencias de conexión" input type="text" name="incidencias"; }
                        p class="sutil" {
                            "La modalidad elegida SALE AL RIPS y al RDA: se escribe también en la \
                             atención."
                        }
                        button type="submit" { "Registrar" }
                    }
                }
            }
        },
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct FormTele {
    historia_id: i64,
    modalidad: String,
    #[serde(default)]
    institucion_remota: String,
    #[serde(default)]
    profesional_remoto: String,
    #[serde(default)]
    plataforma: String,
    #[serde(default)]
    consentimiento_id: String,
    #[serde(default)]
    incidencias: String,
}

fn o_nada(s: String) -> Option<String> {
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// `POST /panel/teleconsultas`
pub async fn registrar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Form(f): Form<FormTele>,
) -> Response {
    let n = NuevaTeleconsulta {
        historia_id: f.historia_id,
        modalidad: f.modalidad,
        institucion_remota: o_nada(f.institucion_remota),
        profesional_remoto: o_nada(f.profesional_remoto),
        plataforma: o_nada(f.plataforma),
        consentimiento_id: f.consentimiento_id.trim().parse().ok(),
        incidencias: o_nada(f.incidencias),
    };
    match teleconsulta::registrar(&e.pool, autorizado.0.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("registrada"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}
