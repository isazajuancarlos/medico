// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Turnos del personal (#249). No es la agenda: es quién está de servicio.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::html;
use serde::Deserialize;

use medico_datos::turnos::{self, NuevoTurno, TIPOS};

use crate::auth::{AdministrarInstitucion, Autorizado, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/turnos";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub desde: String,
    #[serde(default)]
    pub hasta: String,
    #[serde(default)]
    pub aviso: String,
}

fn de_vuelta(aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?aviso={aviso}")).into_response()
}

/// Conjunto cerrado, como en el resto: una clave inventada no pinta nada.
fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    Some(match clave {
        "" => return None,
        "programado" => (Tono::Ok, "Turno programado.".into()),
        "quitado" => (Tono::Ok, "Turno retirado; el horario queda libre.".into()),
        "usuario_inexistente" => (Tono::Error, "esa persona no existe en esta institución".into()),
        "tipo" => (Tono::Error, "el tipo va PRESENCIAL o DISPONIBILIDAD".into()),
        "servicio" => (Tono::Error, "el servicio es obligatorio".into()),
        "fin_antes_del_inicio" => (Tono::Error, "el turno termina antes de empezar".into()),
        "solapa" => (
            Tono::Error,
            "esa persona ya tiene un turno en ese horario: un turno solapado es un turno que \
             alguien no va a cubrir"
                .into(),
        ),
        "turno_inexistente" => (Tono::Error, "ese turno no existe".into()),
        _ => return None,
    })
}

/// `GET /panel/turnos`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let puede = cx.puede(medico_dominio::rol::Capacidad::AdministrarInstitucion);
    let desde = (!f.desde.trim().is_empty()).then(|| f.desde.trim());
    let hasta = (!f.hasta.trim().is_empty()).then(|| f.hasta.trim());
    let filas = match turnos::listar(&e.pool, u.ambito(), desde, hasta).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    // Sin bitácora a propósito: un turno es la planta de personal, no dato de
    // paciente, y la Ley 1581 ata eslabones al titular de un dato de salud.

    pagina(
        "Turnos",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Turnos" } }
            @if let Some((tono, texto)) = texto_del_aviso(f.aviso.trim()) {
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
                (piezas::vacio(desde.is_some() || hasta.is_some(),
                               "No hay turnos programados.", "Ninguno en ese rango.", RUTA))
            } @else {
                (piezas::tabla(
                    &["Inicio", "Fin", "Quién", "Rol", "Servicio", "Tipo", ""],
                    html! {
                        @for t in &filas {
                            tr {
                                td class="mono" { (t.inicio) }
                                td class="mono" { (t.fin) }
                                td { (t.quien) }
                                td class="sutil" { (t.rol) }
                                td { (t.servicio) }
                                td { (piezas::estado(&t.tipo)) }
                                td {
                                    @if puede {
                                        form method="post" action={ (RUTA) "/" (t.id) "/quitar" } {
                                            button type="submit" { "Quitar" }
                                        }
                                    }
                                }
                            }
                        }
                    },
                ))
            }
            @if puede {
                section class="tarjeta" {
                    form method="post" action=(RUTA) class="apilado" {
                        h3 { "Programar un turno" }
                        label { "Persona (id de usuario)"
                            input type="number" name="usuario_id" required min="1"; }
                        label { "Servicio" input type="text" name="servicio" required; }
                        label {
                            "Tipo"
                            select name="tipo" required {
                                @for (c, n) in TIPOS { option value=(c) { (n) } }
                            }
                        }
                        label { "Inicio" input type="datetime-local" name="inicio" required; }
                        label { "Fin" input type="datetime-local" name="fin" required; }
                        label { "Notas" input type="text" name="notas"; }
                        p class="sutil" {
                            "Una persona no puede tener dos turnos que se crucen. Empezar justo \
                             cuando termina el anterior sí: eso es el relevo."
                        }
                        button type="submit" { "Programar" }
                    }
                }
            }
        },
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct FormTurno {
    usuario_id: i64,
    servicio: String,
    tipo: String,
    inicio: String,
    fin: String,
    #[serde(default)]
    notas: String,
}

/// `POST /panel/turnos`
pub async fn programar(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormTurno>,
) -> Response {
    let notas = f.notas.trim().to_string();
    let n = NuevoTurno {
        usuario_id: f.usuario_id,
        servicio: f.servicio,
        tipo: f.tipo,
        inicio: f.inicio,
        fin: f.fin,
        notas: (!notas.is_empty()).then_some(notas),
    };
    match turnos::programar(&e.pool, autorizado.0.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("programado"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

/// `POST /panel/turnos/{id}/quitar`
pub async fn quitar(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Path(id): Path<i64>,
) -> Response {
    match turnos::quitar(&e.pool, autorizado.0.ambito(), id).await {
        Ok(Ok(())) => de_vuelta("quitado"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}
