// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Agenda médica: los horarios mensuales, los bloqueos y los indicadores.
//!
//! Port de `frontend/src/app/pages/agenda-medica/agenda-medica.component.ts`
//! (780 líneas: tres pestañas y dos `MatDialog`).
//!
//! # No es un módulo del catálogo, y eso no es un olvido
//!
//! `/api/agenda` está en `FUERA_DEL_LICENCIAMIENTO` con su motivo escrito: no
//! hay módulo de agenda en el catálogo, y añadir uno **cambia lo que se vende**
//! —es decisión comercial de Juan—. La pantalla hereda esa decisión: vive en
//! `panel::SIN_MODULO`, en el grupo «Administración» del menú, junto al
//! personal.
//!
//! Queda en pie la pregunta que `CLAUDE.md` ya tiene anotada: `TURNOS` figura en
//! el catálogo como pendiente mientras `/api/agenda/turnos` responde gratis. Si
//! son la misma cosa, hay un módulo vendible regalado.
//!
//! # Mirar la agenda no pide capacidad; definirla sí
//!
//! Igual que las nueve rutas JSON: quien agenda desde el mostrador tiene que ver
//! los turnos libres, y un 403 ahí no protege nada. Las seis escrituras exigen
//! `GestionarAgenda`.
//!
//! # Los bloques, sin JavaScript
//!
//! Un mes de agenda son varios bloques —«lunes de 8 a 12», «lunes de 14 a 18»—
//! y el diálogo de Angular los añadía y quitaba con `signal.update`. Aquí el
//! alta dibuja ocho filas en blanco y descarta las que no traen horas; después,
//! la ficha añade de una en una. **No se dibuja una fila por día de la semana**,
//! que sería más simple y estaría mal: perdería la jornada partida, que en una
//! IPS es la norma y no la excepción.
//!
//! # Lo que esta pantalla NO recalcula
//!
//! Ni la oportunidad, ni las tasas, ni si un turno está libre. Todo eso lo
//! decide `medico_datos::agenda` y se enseña tal cual — incluidos sus tres
//! silencios, que son la parte que más fácil se estropea al pintar: «sin datos»
//! no es cero, las citas sin fecha de solicitud se cuentan aparte, y las que
//! traen una solicitud posterior a la cita no se promedian.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::agenda;
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{Estado, GestionarAgenda};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/agenda";
const BLOQUEOS: &str = "/panel/agenda/bloqueos";
const INDICADORES: &str = "/panel/agenda/indicadores";

/// Filas en blanco del alta. Ocho cubre una semana con jornada partida sin
/// llenar la pantalla; la ficha añade más de una en una.
const FILAS_NUEVAS: usize = 8;

/// Tope de filas que se leen del formulario. Barandilla contra un cuerpo
/// fabricado a mano, no un límite del producto.
const MAX_FILAS: usize = 60;

/// Los días, en el orden de la semana y con el número que usa la base
/// (0 = lunes, como el `weekday()` del Python y como el CHECK de la tabla).
const DIAS: [(i64, &str); 7] = [
    (0, "Lunes"),
    (1, "Martes"),
    (2, "Miércoles"),
    (3, "Jueves"),
    (4, "Viernes"),
    (5, "Sábado"),
    (6, "Domingo"),
];

const MESES: [(i64, &str); 12] = [
    (1, "enero"),
    (2, "febrero"),
    (3, "marzo"),
    (4, "abril"),
    (5, "mayo"),
    (6, "junio"),
    (7, "julio"),
    (8, "agosto"),
    (9, "septiembre"),
    (10, "octubre"),
    (11, "noviembre"),
    (12, "diciembre"),
];

/// Los tipos de bloqueo del producto. Salen del `CHECK` de la migración, no de
/// una lista inventada aquí.
const TIPOS_BLOQUEO: [&str; 4] = ["VACACIONES", "INCAPACIDAD", "REUNION", "OTRO"];

fn nombre_dia(n: i64) -> &'static str {
    // Un día que no es ninguno de los siete no se renombra: se dice su número,
    // que es un dato cierto (directiva 20).
    DIAS.iter().find(|(v, _)| *v == n).map(|(_, s)| *s).unwrap_or("día")
}

fn nombre_mes(n: i64) -> &'static str {
    MESES.iter().find(|(v, _)| *v == n).map(|(_, s)| *s).unwrap_or("mes")
}

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "creada" => Some((Tono::Ok, "Agenda creada.")),
        "guardada" => Some((Tono::Ok, "Cambios guardados.")),
        "borrada" => Some((Tono::Ok, "Agenda eliminada.")),
        "bloque_anadido" => Some((Tono::Ok, "Bloque añadido.")),
        "bloque_borrado" => Some((Tono::Ok, "Bloque eliminado.")),
        "bloqueo_creado" => Some((Tono::Ok, "Bloqueo registrado.")),
        "bloqueo_borrado" => Some((Tono::Ok, "Bloqueo eliminado.")),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

// ── Armazón ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    #[serde(default)]
    pub medico_id: Option<i64>,
    #[serde(default)]
    pub anio: Option<i64>,
    #[serde(default)]
    pub mes: Option<i64>,
    #[serde(default)]
    pub desde: Option<String>,
    #[serde(default)]
    pub hasta: Option<String>,
}

fn cabecera(donde: &str) -> Markup {
    html! {
        header class="titulo-seccion" {
            h1 { "Agenda médica" }
            nav class="buscador" {
                a class=[(donde == "horarios").then_some("enlace-activo")] href=(RUTA) {
                    "Horarios mensuales"
                }
                a class=[(donde == "bloqueos").then_some("enlace-activo")] href=(BLOQUEOS) {
                    "Bloqueos"
                }
                a class=[(donde == "indicadores").then_some("enlace-activo")] href=(INDICADORES) {
                    "Indicadores"
                }
            }
        }
        p class="sutil" {
            "Horarios de atención conforme a la Resolución 256 de 2016, la 2003 de 2014 y el "
            "Decreto 780 de 2016."
        }
    }
}

fn pantalla(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA)), cuerpo).into_response()
}

fn no_existe(cx: &Contexto) -> Response {
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrado",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró esa agenda" }
                    p { a href=(RUTA) { "Volver a la agenda médica" } }
                }
            },
        ),
    )
        .into_response()
}

/// El selector de profesional, compartido por los tres formularios.
fn selector_medico(
    nombre: &str,
    medicos: &[agenda::MedicoAgendable],
    elegido: Option<i64>,
    con_todos: bool,
) -> Markup {
    html! {
        select name=(nombre) {
            @if con_todos {
                option value="" selected[elegido.is_none()] { "Todos los profesionales" }
            } @else {
                option value="" { "Elija uno" }
            }
            @for m in medicos {
                option value=(m.id) selected[elegido == Some(m.id)] {
                    (m.nombre) " — " (m.rol)
                }
            }
        }
    }
}

// ── Horarios mensuales ───────────────────────────────────────────────────────

/// `GET /panel/agenda`
pub async fn horarios(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let medicos = match agenda::medicos(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let agendas = match agenda::listar(&e.pool, u.ambito(), f.medico_id, f.anio, f.mes).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarAgenda);
    let hoy = medico_datos::hoy();
    let anio_actual: i64 = hoy[..4].parse().unwrap_or(2026);

    pantalla(
        &cx,
        "Agenda médica",
        html! {
            (cabecera("horarios"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="buscador" {
                    (selector_medico("medico_id", &medicos, f.medico_id, true))
                    select name="anio" {
                        @for a in (anio_actual - 1)..=(anio_actual + 1) {
                            option value=(a) selected[f.anio == Some(a)] { (a) }
                        }
                    }
                    select name="mes" {
                        option value="" selected[f.mes.is_none()] { "Todos los meses" }
                        @for (v, n) in MESES {
                            option value=(v) selected[f.mes == Some(v)] { (n) }
                        }
                    }
                    button type="submit" { "Filtrar" }
                    a class="enlace-sutil" href=(RUTA) { "Quitar el filtro" }
                }
            }

            @if agendas.is_empty() {
                (piezas::vacio(
                    f.medico_id.is_some() || f.mes.is_some() || f.anio.is_some(),
                    "No hay agendas configuradas.",
                    "Ninguna agenda coincide con ese filtro.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(agendas.len(), agendas.len(), "agenda", "agendas"))
                (piezas::tabla(
                    &["Profesional", "Mes", "Estado"],
                    html! {
                        @for a in &agendas {
                            tr {
                                td { a href={ (RUTA) "/" (a.id) } { (a.medico_nombre) } }
                                td { (nombre_mes(a.mes)) " de " (a.anio) }
                                td {
                                    @if a.activa {
                                        (piezas::estado("ACTIVA"))
                                    } @else {
                                        (piezas::estado("INACTIVA"))
                                    }
                                }
                            }
                        }
                    },
                ))
            }

            @if puede {
                (formulario_alta(&medicos, anio_actual, None))
            }
        },
    )
}

/// El alta de una agenda mensual, con sus bloques.
fn formulario_alta(
    medicos: &[agenda::MedicoAgendable],
    anio_actual: i64,
    error: Option<&str>,
) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Nueva agenda mensual" }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            @if medicos.is_empty() {
                p class="sutil" {
                    "No hay profesionales a los que agendar. Se agenda a quien ATIENDE a un "
                    "paciente citado: al laboratorista o al regente de farmacia no se les "
                    "agenda una cita, se les manda una orden."
                }
            } @else {
                form method="post" action=(RUTA) class="apilado" {
                    label for="medico_id" { "Profesional" }
                    (selector_medico("medico_id", medicos, None, false))
                    label for="anio" { "Año" }
                    select #anio name="anio" {
                        @for a in anio_actual..=(anio_actual + 1) {
                            option value=(a) { (a) }
                        }
                    }
                    label for="mes" { "Mes" }
                    select #mes name="mes" {
                        @for (v, n) in MESES { option value=(v) { (n) } }
                    }
                    label for="observaciones" { "Observaciones" }
                    input #observaciones type="text" name="observaciones";

                    h3 { "Bloques de atención" }
                    p class="sutil" {
                        "Duración mínima por consulta: medicina general ≥ 20 min, "
                        "especialista ≥ 30 min (Circular 049 de 2008 de la Supersalud). "
                        "Un mismo día puede llevar varios bloques: la jornada partida es "
                        "la norma."
                    }
                    (filas_de_bloque(FILAS_NUEVAS))
                    p class="sutil" { "Las filas sin horas se descartan." }
                    button type="submit" class="principal" { "Crear la agenda" }
                }
            }
        }
    }
}

/// Las filas de bloque de un formulario. Los índices van en el NOMBRE del campo,
/// por lo mismo que en laboratorio: un `<select>` vacío y una casilla sin marcar
/// no se envían igual, y con listas paralelas las columnas se desalinean.
fn filas_de_bloque(cuantas: usize) -> Markup {
    html! {
        (piezas::tabla(
            &["Día", "Desde", "Hasta", "Minutos por turno"],
            html! {
                @for i in 0..cuantas {
                    tr {
                        td {
                            select name={ "dia_" (i) } {
                                @for (v, n) in DIAS { option value=(v) { (n) } }
                            }
                        }
                        td { input type="time" name={ "inicio_" (i) }; }
                        td { input type="time" name={ "fin_" (i) }; }
                        td {
                            input type="number" name={ "duracion_" (i) } min="5" max="240"
                                  placeholder="30";
                        }
                    }
                }
            },
        ))
    }
}

/// Lee las filas de bloque del formulario. **Una fila sin las dos horas no es un
/// bloque** y se descarta: media franja no dice cuándo se atiende.
fn bloques_del_formulario(campos: &HashMap<String, String>) -> Vec<agenda::BloqueNuevo> {
    let texto = |k: String| campos.get(&k).cloned().unwrap_or_default();
    (0..MAX_FILAS)
        .filter_map(|i| {
            let inicio = texto(format!("inicio_{i}")).trim().to_string();
            let fin = texto(format!("fin_{i}")).trim().to_string();
            if inicio.is_empty() || fin.is_empty() {
                return None;
            }
            Some(agenda::BloqueNuevo {
                dia_semana: texto(format!("dia_{i}")).parse().unwrap_or(0),
                hora_inicio: inicio,
                hora_fin: fin,
                // Sin valor, la tabla pone 30. **No se rellena aquí**: el
                // defecto está declarado en el esquema y escribirlo también en
                // la pantalla sería un segundo sitio donde cambiarlo.
                duracion_slot_min: texto(format!("duracion_{i}")).trim().parse().ok(),
            })
        })
        .collect()
}

/// `POST /panel/agenda` — crear la agenda con sus bloques.
pub async fn crear(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let numero = |k: &str| campos.get(k).and_then(|v| v.trim().parse::<i64>().ok());
    let (Some(medico_id), Some(anio), Some(mes)) =
        (numero("medico_id"), numero("anio"), numero("mes"))
    else {
        return con_error_de_alta(&e, &cx, &u, "Elija el profesional, el año y el mes.").await;
    };
    let nueva = agenda::AgendaNueva {
        medico_id,
        anio,
        mes,
        observaciones: campos
            .get("observaciones")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        bloques: bloques_del_formulario(&campos),
    };
    match agenda::crear(&e.pool, u.ambito(), u.id, &nueva).await {
        Ok(Some(_)) => de_vuelta(RUTA, "creada"),
        // `crear` devuelve `None` para cuatro cosas: el médico no es del ámbito,
        // una hora ilegible o invertida, ya hay agenda de ese mes, o el mes no
        // existe. Se dicen las cuatro porque desde aquí no se pueden distinguir
        // —y decir solo una mandaría a corregir lo que estaba bien—.
        Ok(None) => {
            con_error_de_alta(
                &e,
                &cx,
                &u,
                "No se pudo crear la agenda. Puede ser que ese profesional ya tenga agenda \
                 de ese mes, que alguna hora esté al revés o mal escrita, o que el \
                 profesional no sea de esta institución.",
            )
            .await
        }
        Err(_) => averia(),
    }
}

async fn con_error_de_alta(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    motivo: &str,
) -> Response {
    let medicos = match agenda::medicos(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let anio: i64 = medico_datos::hoy()[..4].parse().unwrap_or(2026);
    pantalla(
        cx,
        "Agenda médica",
        html! {
            (cabecera("horarios"))
            (formulario_alta(&medicos, anio, Some(motivo)))
        },
    )
}

// ── La ficha de una agenda ───────────────────────────────────────────────────

/// `GET /panel/agenda/{id}`
pub async fn ficha(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let a = match agenda::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(a)) => a,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarAgenda);

    pantalla(
        &cx,
        "Agenda",
        html! {
            (piezas::migas(RUTA, "Agenda médica"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            header class="titulo-seccion" {
                h1 { (a.agenda.medico_nombre) " · " (nombre_mes(a.agenda.mes)) " de " (a.agenda.anio) }
                @if a.agenda.activa {
                    (piezas::estado("ACTIVA"))
                } @else {
                    (piezas::estado("INACTIVA"))
                }
            }

            section class="tarjeta" {
                h2 { "Bloques de atención" }
                @if a.bloques.is_empty() {
                    // «Sin bloques» NO es «no atiende ese mes»: es una agenda
                    // que existe y no ofrece un solo turno. Se dice, porque el
                    // efecto en la pantalla de citas es el mismo y la causa no.
                    (aviso(
                        Tono::Averia,
                        "Esta agenda no tiene ningún bloque: no ofrece turnos. No es que el \
                         profesional no atienda ese mes — es que nadie le definió el horario.",
                    ))
                } @else {
                    (piezas::tabla(
                        &["Día", "Desde", "Hasta", "Minutos por turno", "Estado", ""],
                        html! {
                            @for b in &a.bloques {
                                tr {
                                    td { (nombre_dia(b.dia_semana)) }
                                    td class="mono" { (b.hora_inicio) }
                                    td class="mono" { (b.hora_fin) }
                                    td class="mono" { (b.duracion_slot_min) }
                                    td {
                                        @if b.activo {
                                            (piezas::estado("ACTIVA"))
                                        } @else {
                                            (piezas::estado("INACTIVA"))
                                        }
                                    }
                                    td {
                                        @if puede {
                                            form method="post"
                                                 action={ (RUTA) "/" (a.agenda.id) "/bloques/" (b.id) "/borrar" } {
                                                button type="submit" class="enlace"
                                                        data-confirmar="Pulse otra vez para eliminarlo" {
                                                    "Eliminar"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
            }

            @if puede {
                section class="tarjeta" {
                    h2 { "Añadir un bloque" }
                    form method="post" action={ (RUTA) "/" (a.agenda.id) "/bloques" } {
                        (filas_de_bloque(1))
                        button type="submit" { "Añadir" }
                    }
                }

                section class="tarjeta" {
                    h2 { "Estado y observaciones" }
                    p class="sutil" {
                        "Una agenda inactiva no ofrece turnos y no se borra: los que ya se "
                        "dieron siguen explicándose."
                    }
                    form method="post" action={ (RUTA) "/" (a.agenda.id) } class="apilado" {
                        label {
                            input type="checkbox" name="activa" value="1" checked[a.agenda.activa];
                            " Agenda activa"
                        }
                        label for="observaciones" { "Observaciones" }
                        input #observaciones type="text" name="observaciones"
                              value=(a.agenda.observaciones.clone().unwrap_or_default());
                        button type="submit" class="principal" { "Guardar" }
                    }
                }

                section class="tarjeta" {
                    h2 { "Eliminar la agenda" }
                    p class="sutil" {
                        "Se va con sus bloques. Si solo quiere que deje de ofrecer turnos, "
                        "desactívela."
                    }
                    form method="post" action={ (RUTA) "/" (a.agenda.id) "/borrar" } {
                        button type="submit" class="enlace"
                                data-confirmar="Pulse otra vez para eliminarla" {
                            "Eliminar"
                        }
                    }
                }
            }
        },
    )
}

/// `POST /panel/agenda/{id}` — estado y observaciones.
///
/// **La casilla decide, y aquí sí es correcto** —al revés que la baja de una
/// ambulancia—: esto es un formulario con el estado dentro, que se lee de la
/// pantalla que la persona está mirando, no un botón que alterna. Reenviarlo
/// deja lo mismo que ya había.
pub async fn guardar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let observaciones = campos.get("observaciones").map(|s| s.trim()).unwrap_or("");
    match agenda::actualizar(
        &e.pool,
        u.ambito(),
        id,
        Some(campos.contains_key("activa")),
        Some(observaciones),
    )
    .await
    {
        Ok(true) => de_vuelta(&format!("{RUTA}/{id}"), "guardada"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// `POST /panel/agenda/{id}/borrar`
pub async fn borrar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match agenda::borrar(&e.pool, u.ambito(), id).await {
        Ok(true) => de_vuelta(RUTA, "borrada"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// `POST /panel/agenda/{id}/bloques`
pub async fn anadir_bloque(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(b) = bloques_del_formulario(&campos).into_iter().next() else {
        // Sin horas no hay bloque. Se vuelve a la ficha sin escribir nada: no
        // hay aviso porque no ha pasado nada que contar.
        return Redirect::to(&format!("{RUTA}/{id}")).into_response();
    };
    match agenda::anadir_bloque(&e.pool, u.ambito(), id, &b).await {
        Ok(Some(_)) => de_vuelta(&format!("{RUTA}/{id}"), "bloque_anadido"),
        Ok(None) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// `POST /panel/agenda/{id}/bloques/{bloque_id}/borrar`
pub async fn borrar_bloque(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Path((id, bloque_id)): Path<(i64, i64)>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match agenda::borrar_bloque(&e.pool, u.ambito(), id, bloque_id).await {
        Ok(true) => de_vuelta(&format!("{RUTA}/{id}"), "bloque_borrado"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

// ── Bloqueos ─────────────────────────────────────────────────────────────────

/// `GET /panel/agenda/bloqueos`
pub async fn bloqueos(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_bloqueos(&e, &cx, &u, f.aviso.trim(), None).await
}

async fn dibujar_bloqueos(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let medicos = match agenda::medicos(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let lista = match agenda::listar_bloqueos(&e.pool, u.ambito(), None, None, None).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarAgenda);

    pantalla(
        cx,
        "Bloqueos de agenda",
        html! {
            (cabecera("bloqueos"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            section class="tarjeta" {
                p class="sutil" {
                    "Vacaciones, incapacidades, reuniones u otros periodos sin atención. "
                    "Un bloqueo no borra las citas que ya estaban: las deja marcadas para "
                    "que alguien las reprograme."
                }
                @if lista.is_empty() {
                    p class="sutil" { "No hay bloqueos registrados." }
                } @else {
                    (piezas::tabla(
                        &["Profesional", "Tipo", "Desde", "Hasta", "Motivo", ""],
                        html! {
                            @for b in &lista {
                                tr {
                                    td { (b.medico_nombre) }
                                    td { (piezas::estado(&b.tipo)) }
                                    td class="mono" { (b.inicio) }
                                    td class="mono" { (b.fin) }
                                    td { (b.motivo) }
                                    td {
                                        @if puede {
                                            form method="post"
                                                 action={ (BLOQUEOS) "/" (b.id) "/borrar" } {
                                                button type="submit" class="enlace"
                                                        data-confirmar="Pulse otra vez para eliminarlo" {
                                                    "Eliminar"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
            }

            @if puede {
                section class="tarjeta" {
                    h2 { "Nuevo bloqueo" }
                    @if let Some(t) = error { (aviso(Tono::Error, t)) }
                    form method="post" action=(BLOQUEOS) class="apilado" {
                        label for="medico_id" { "Profesional" }
                        (selector_medico("medico_id", &medicos, None, false))
                        label for="inicio" { "Desde" }
                        input #inicio type="datetime-local" name="inicio" required;
                        label for="fin" { "Hasta" }
                        input #fin type="datetime-local" name="fin" required;
                        label for="tipo" { "Tipo" }
                        select #tipo name="tipo" {
                            @for t in TIPOS_BLOQUEO { option value=(t) { (t) } }
                        }
                        label for="motivo" { "Motivo" }
                        input #motivo type="text" name="motivo" required
                              placeholder="Descripción del bloqueo";
                        button type="submit" class="principal" { "Registrar el bloqueo" }
                    }
                }
            }
        },
    )
}

/// `POST /panel/agenda/bloqueos`
pub async fn crear_bloqueo(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(medico_id) = campos.get("medico_id").and_then(|v| v.trim().parse::<i64>().ok()) else {
        return dibujar_bloqueos(&e, &cx, &u, "", Some("Elija el profesional.")).await;
    };
    // `datetime-local` manda `2026-08-10T08:00`; la base guarda
    // `YYYY-MM-DD HH:MM`. La T se cambia aquí y no en el dominio: `es_instante`
    // define el formato del producto, y traducirlo es trabajo de la pantalla que
    // habla con un `<input>` concreto.
    let instante = |k: &str| {
        campos
            .get(k)
            .map(|s| s.trim().replace('T', " "))
            .unwrap_or_default()
    };
    let nuevo = agenda::BloqueoNuevo {
        medico_id,
        inicio: instante("inicio"),
        fin: instante("fin"),
        motivo: campos.get("motivo").cloned().unwrap_or_default().trim().to_string(),
        tipo: campos
            .get("tipo")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    };
    match agenda::crear_bloqueo(&e.pool, u.ambito(), u.id, &nuevo).await {
        Ok(Some(_)) => de_vuelta(BLOQUEOS, "bloqueo_creado"),
        Ok(None) => {
            dibujar_bloqueos(
                &e,
                &cx,
                &u,
                "",
                Some(
                    "No se pudo registrar el bloqueo. Revise que las dos fechas estén \
                     completas, que la de fin sea posterior a la de inicio y que el \
                     profesional sea de esta institución.",
                ),
            )
            .await
        }
        Err(_) => averia(),
    }
}

/// `POST /panel/agenda/bloqueos/{id}/borrar`
pub async fn borrar_bloqueo(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAgenda>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match agenda::borrar_bloqueo(&e.pool, u.ambito(), id).await {
        Ok(true) => de_vuelta(BLOQUEOS, "bloqueo_borrado"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

// ── Indicadores ──────────────────────────────────────────────────────────────

/// `GET /panel/agenda/indicadores`
///
/// **Nada de esto se calcula aquí.** Los tres silencios del dominio se enseñan
/// tal cual, y son la parte que más fácil se estropea al pintar: «sin datos» no
/// es cero, las citas sin fecha de solicitud se cuentan aparte, y las que traen
/// una solicitud posterior a la cita no se promedian —darían días negativos que
/// MEJORAN el indicador—.
pub async fn indicadores(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let medicos = match agenda::medicos(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let hoy = medico_datos::hoy();
    let desde = f.desde.clone().filter(|s| !s.is_empty()).unwrap_or(format!("{}-01", &hoy[..7]));
    let hasta = f.hasta.clone().filter(|s| !s.is_empty()).unwrap_or(hoy.clone());
    let d = match agenda::indicadores(&e.pool, u.ambito(), f.medico_id, &desde, &hasta).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    let sin_medir = d.oportunidad_medidas + d.oportunidad_sin_dato;

    pantalla(
        &cx,
        "Indicadores de agenda",
        html! {
            (cabecera("indicadores"))
            section class="tarjeta" {
                form method="get" action=(INDICADORES) class="buscador" {
                    (selector_medico("medico_id", &medicos, f.medico_id, true))
                    label for="desde" class="sutil" { "Desde" }
                    input #desde type="date" name="desde" value=(d.desde);
                    label for="hasta" class="sutil" { "Hasta" }
                    input #hasta type="date" name="hasta" value=(d.hasta);
                    button type="submit" { "Consultar" }
                }
                p class="sutil" { "Periodo: del " (d.desde) " al " (d.hasta) "." }
            }

            section class="tarjeta resumen" {
                div { span class="cifra" { (d.atendidas) } span class="sutil" { "atendidas" } }
                div { span class="cifra" { (d.canceladas) } span class="sutil" { "canceladas" } }
                div { span class="cifra" { (d.no_asistio) } span class="sutil" { "no asistieron" } }
                div { span class="cifra" { (d.total) } span class="sutil" { "programadas" } }
            }

            section class="tarjeta" {
                h2 { "Oportunidad" }
                p {
                    @match d.oportunidad_dias_promedio {
                        // **«Sin datos» NO es cero**, y cero sería el mejor
                        // resultado posible: el más engañoso de todos.
                        None => span class="falta" { "sin datos" },
                        Some(v) => {
                            strong { (format!("{v:.1}").replace('.', ",")) }
                            " días de promedio"
                        }
                    }
                }
                p class="sutil" {
                    "Medida sobre " (d.oportunidad_medidas) " de " (sin_medir) " citas. "
                    "Meta: ≤ " (d.meta_medicina_general_dias) " días en medicina general y "
                    "≤ " (d.meta_especialista_dias) " en especialista (" (d.normativa) ")."
                }
                @if let Some(v) = d.oportunidad_dias_promedio {
                    @if v > d.meta_medicina_general_dias as f64 {
                        (aviso(
                            Tono::Error,
                            "El promedio supera la meta de medicina general.",
                        ))
                    }
                }
                @if d.oportunidad_sin_dato > 0 {
                    (aviso(Tono::Averia, &format!(
                        "{} cita(s) sin fecha de solicitud: no cuentan para la oportunidad. \
                         Registre la fecha en que el paciente PIDIÓ la cita para que entren.",
                        d.oportunidad_sin_dato
                    )))
                }
                @if d.oportunidad_incoherentes > 0 {
                    (aviso(Tono::Error, &format!(
                        "{} cita(s) con la solicitud POSTERIOR a la cita. Es imposible, así que \
                         no se promedian: darían días negativos que mejorarían el indicador. \
                         Hay que corregir la fecha en esas citas.",
                        d.oportunidad_incoherentes
                    )))
                }
            }

            section class="tarjeta" {
                h2 { "Inasistencia y cancelación" }
                (piezas::datos(&[
                    ("Tasa de inasistencia", d.tasa_inasistencia_pct.map(porcentaje)),
                    ("Tasa de cancelación", d.tasa_cancelacion_pct.map(porcentaje)),
                ]))
                @if d.tasa_inasistencia_pct.is_none() {
                    // Las dos tasas son `None` cuando no hubo citas. Se dice, en
                    // vez de dejar la ficha en blanco: «no hubo citas» y «nadie
                    // faltó» son cosas distintas.
                    p class="sutil" { "Sin citas en el periodo: no hay tasa que calcular." }
                }
            }
        },
    )
}

fn porcentaje(v: f64) -> String {
    format!("{} %", format!("{v:.1}").replace('.', ","))
}
