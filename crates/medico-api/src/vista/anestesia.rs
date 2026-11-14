// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Los dos extremos del acto anestésico (#249): quién puede entrar a cirugía y
//! quién puede salir de recuperación.
//!
//! Una pantalla para los dos módulos, con la misma regla que enfermería: cada
//! sección se dibuja solo si `cx.tiene(…)`, porque se licencian por separado y
//! ofrecer un formulario que la API rechaza es enseñar la regla dos veces y en
//! desacuerdo.
//!
//! # El aviso del Aldrete bajo se escribe en el formulario, no en el error
//!
//! Que se pueda salir con Aldrete bajo dando una razón no es una excepción
//! escondida: va escrito al lado de la casilla, porque quien saca a un paciente
//! a UCI a las tres de la mañana no debería descubrir la regla por un rechazo.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::anestesia::{
    self, NuevaRecuperacion, NuevaValoracion, Salida, ALDRETE_PARA_SALIR, DESTINOS,
};

use crate::auth::{Autorizado, AtenderUrgencias, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/anestesia";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
}

fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    match clave {
        "" => None,
        "valorada" => Some((Tono::Ok, "Valoración preanestésica registrada.".into())),
        "recibido" => Some((Tono::Ok, "Paciente recibido en recuperación.".into())),
        "salio" => Some((Tono::Ok, "Salida de recuperación registrada.".into())),
        otra => anestesia::Falta::de_clave(otra).map(|f| (Tono::Error, f.mensaje().to_string())),
    }
}

fn de_vuelta(aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?aviso={aviso}")).into_response()
}

/// `GET /panel/anestesia`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let puede = cx.puede(medico_dominio::rol::Capacidad::AtenderUrgenciasHospitalizacion);
    let el_aviso = texto_del_aviso(f.aviso.trim());

    let valoraciones = if cx.tiene("PREANESTESIA") {
        match anestesia::listar_valoraciones(&e.pool, u.ambito()).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };
    let recuperaciones = if cx.tiene("RECUPERACION") {
        match anestesia::listar_recuperaciones(&e.pool, u.ambito()).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };

    if cx.tiene("PREANESTESIA") {
        crate::rutas::registrar_coleccion(
            &e, &u, "listar", crate::rutas::RECURSO_PREANESTESIA, None, valoraciones.len(), vec![],
        )
        .await;
    }
    if cx.tiene("RECUPERACION") {
        crate::rutas::registrar_coleccion(
            &e, &u, "listar", crate::rutas::RECURSO_RECUPERACION, None, recuperaciones.len(), vec![],
        )
        .await;
    }

    pagina(
        "Anestesia",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Anestesia" } }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            @if cx.tiene("PREANESTESIA") {
                section class="tarjeta" {
                    h2 { "Valoración preanestésica" }
                    @if valoraciones.is_empty() {
                        (piezas::vacio(false, "No hay valoraciones registradas.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Fecha", "Paciente", "Procedimiento", "ASA", "Mallampati", "Ayuno", "Apto", "Anestesiólogo"],
                            html! {
                                @for v in &valoraciones {
                                    tr {
                                        td class="mono" { (v.fecha) }
                                        td { a href={ "/panel/pacientes/" (v.paciente_id) } { (v.paciente) } }
                                        td { (v.procedimiento) }
                                        td class="mono" {
                                            (romano(v.asa))
                                            @if v.asa_urgencia { "E" }
                                        }
                                        td class="mono" { (v.mallampati.map(|m| m.to_string()).unwrap_or_default()) }
                                        td class="mono" {
                                            @match v.ayuno_horas {
                                                Some(h) => { (h) " h" }
                                                // Sin ayuno declarado se deja vacío:
                                                // un cero inventado diría que el
                                                // paciente comió hace un momento.
                                                None => "",
                                            }
                                        }
                                        td {
                                            @if v.apto { (piezas::estado("APTO")) }
                                            @else {
                                                (piezas::estado("NO APTO"))
                                                " " span class="sutil" { (v.motivo_no_apto.clone().unwrap_or_default()) }
                                            }
                                        }
                                        td { (v.anestesiologo) }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_valoracion()) }
                }
            }

            @if cx.tiene("RECUPERACION") {
                section class="tarjeta" {
                    h2 { "Recuperación postanestésica" }
                    @if recuperaciones.is_empty() {
                        (piezas::vacio(false, "No hay pacientes en recuperación.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Ingreso", "Paciente", "Procedimiento", "Aldrete", "Salida", "Destino", ""],
                            html! {
                                @for r in &recuperaciones {
                                    tr {
                                        td class="mono" { (r.ingreso_recuperacion) }
                                        td { a href={ "/panel/pacientes/" (r.paciente_id) } { (r.paciente) } }
                                        td { (r.procedimiento) }
                                        td class="mono" {
                                            (r.aldrete_ingreso)
                                            @if let Some(s) = r.aldrete_salida { " → " (s) }
                                        }
                                        td class="mono" { (r.salida_recuperacion.clone().unwrap_or_default()) }
                                        td { (r.destino.clone().unwrap_or_default()) }
                                        td {
                                            @if puede && r.salida_recuperacion.is_none() {
                                                (formulario_salida(r.id))
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_recibir()) }
                }
            }
        },
    )
    .into_response()
}

/// La ASA se escribe en romanos, que es como se dice y como está en la escala.
/// Un «2» en la hoja se lee como cualquier otra cifra de la fila.
fn romano(n: i64) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        // El `CHECK` lo impide; si algún día entra por un importador, se ve.
        _ => "?",
    }
}

fn formulario_valoracion() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/valorar" } class="apilado" {
            h3 { "Registrar una valoración" }
            label { "Cirugía (id de la programación)"
                input type="number" name="programacion_id" required min="1"; }
            label {
                "Clasificación ASA"
                select name="asa" required {
                    @for n in 1..=6 { option value=(n) { (romano(n)) } }
                }
            }
            label { input type="checkbox" name="asa_urgencia" value="1"; " Cirugía de urgencia (sufijo E)" }
            label { "Mallampati (1 a 4, en blanco si no se exploró)"
                input type="number" name="mallampati" min="1" max="4"; }
            label { "Horas de ayuno (en blanco si no se declaró)"
                input type="number" name="ayuno_horas" min="0"; }
            label { "Comorbilidades" input type="text" name="comorbilidades"; }
            label { "Plan anestésico" input type="text" name="plan_anestesico"; }
            label { input type="checkbox" name="riesgos_explicados" value="1"; " Riesgos explicados al paciente" }
            label { input type="checkbox" name="apto" value="1" checked; " Apto para cirugía" }
            label { "Si NO es apto, por qué" input type="text" name="motivo_no_apto"; }
            label { "Observaciones" input type="text" name="observaciones"; }
            button type="submit" { "Registrar valoración" }
        }
    }
}

fn formulario_recibir() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/recibir" } class="apilado" {
            h3 { "Recibir en recuperación" }
            label { "Cirugía (id de la programación)"
                input type="number" name="programacion_id" required min="1"; }
            label { "Aldrete al ingreso (0 a 10)"
                input type="number" name="aldrete_ingreso" required min="0" max="10"; }
            label { "Complicaciones" input type="text" name="complicaciones"; }
            label { "Observaciones" input type="text" name="observaciones"; }
            button type="submit" { "Recibir" }
        }
    }
}

fn formulario_salida(id: i64) -> Markup {
    html! {
        form method="post" action={ (RUTA) "/" (id) "/salida" } {
            input type="number" name="aldrete_salida" required min="0" max="10" placeholder="Aldrete";
            select name="destino" required {
                @for (c, n) in DESTINOS { option value=(c) { (n) } }
            }
            input type="text" name="observaciones" placeholder="razón si sale bajo";
            button type="submit" { "Dar salida" }
            // La regla, al lado de la casilla y no en el mensaje de error.
            span class="sutil" {
                " Por debajo de " (ALDRETE_PARA_SALIR) " hay que decir por qué."
            }
        }
    }
}

fn o_nada(s: String) -> Option<String> {
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn o_numero(s: String) -> Option<i64> {
    s.trim().parse().ok()
}

#[derive(Deserialize)]
pub struct FormValoracion {
    programacion_id: i64,
    asa: i64,
    #[serde(default)]
    asa_urgencia: Option<String>,
    #[serde(default)]
    mallampati: String,
    #[serde(default)]
    ayuno_horas: String,
    #[serde(default)]
    comorbilidades: String,
    #[serde(default)]
    plan_anestesico: String,
    #[serde(default)]
    riesgos_explicados: Option<String>,
    #[serde(default)]
    apto: Option<String>,
    #[serde(default)]
    motivo_no_apto: String,
    #[serde(default)]
    observaciones: String,
}

/// `POST /panel/anestesia/valorar`
pub async fn valorar(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<FormValoracion>,
) -> Response {
    let u = autorizado.0;
    // Los dos extremos del acto anestésico comparten pantalla y se licencian
    // por separado: la valoración es PREANESTESIA.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "PREANESTESIA").await {
        return r;
    }
    let n = NuevaValoracion {
        programacion_id: f.programacion_id,
        asa: f.asa,
        asa_urgencia: f.asa_urgencia.is_some(),
        mallampati: o_numero(f.mallampati),
        ayuno_horas: o_numero(f.ayuno_horas),
        comorbilidades: o_nada(f.comorbilidades),
        plan_anestesico: o_nada(f.plan_anestesico),
        riesgos_explicados: f.riesgos_explicados.is_some(),
        apto: f.apto.is_some(),
        motivo_no_apto: o_nada(f.motivo_no_apto),
        observaciones: o_nada(f.observaciones),
    };
    match anestesia::valorar(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta("valorada"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormRecibir {
    programacion_id: i64,
    aldrete_ingreso: i64,
    #[serde(default)]
    complicaciones: String,
    #[serde(default)]
    observaciones: String,
}

/// `POST /panel/anestesia/recibir`
pub async fn recibir(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<FormRecibir>,
) -> Response {
    let u = autorizado.0;
    // Recibir en recuperación es RECUPERACION, no PREANESTESIA.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "RECUPERACION").await {
        return r;
    }
    let n = NuevaRecuperacion {
        programacion_id: f.programacion_id,
        aldrete_ingreso: f.aldrete_ingreso,
        complicaciones: o_nada(f.complicaciones),
        observaciones: o_nada(f.observaciones),
    };
    match anestesia::recibir(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta("recibido"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormSalida {
    aldrete_salida: i64,
    destino: String,
    #[serde(default)]
    observaciones: String,
}

/// `POST /panel/anestesia/{id}/salida`
pub async fn salida(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Path(id): Path<i64>,
    Form(f): Form<FormSalida>,
) -> Response {
    let u = autorizado.0;
    // La salida cierra el mismo documento que abrió `recibir`.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "RECUPERACION").await {
        return r;
    }
    let s = Salida {
        aldrete_salida: f.aldrete_salida,
        destino: f.destino,
        observaciones: o_nada(f.observaciones),
    };
    match anestesia::dar_salida(&e.pool, u.ambito(), id, &s).await {
        Ok(Ok(())) => de_vuelta("salio"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}
