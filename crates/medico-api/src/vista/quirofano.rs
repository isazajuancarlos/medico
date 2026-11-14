// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Quirófano: la agenda quirúrgica, la lista de verificación y las salas.
//!
//! Port de `frontend/src/app/pages/quirofano/quirofano.component.ts` (894
//! líneas: dos vistas, dos pestañas y un diálogo de detalle).
//!
//! # La máquina de estados NO se copia
//!
//! Quién puede iniciar, completar, cancelar o suspender una cirugía —y qué le
//! pasa a la sala en cada paso— lo decide `medico_dominio::quirofano`. Esta
//! pantalla **pregunta qué transiciones caben** y dibuja solo esos botones; no
//! los deduce del estado con una lista propia, que es lo que hacía el
//! componente. Ofrecer «completar» sobre una cirugía cancelada y que el servidor
//! la rechace es enseñar la regla dos veces.
//!
//! # El vocabulario del estado también sale del dominio
//!
//! `ESTADO_LABEL` vivía en el frontal. Con dos pantallas serían dos diccionarios
//! y el mismo estado se leería distinto en cada una, así que
//! `EstadoCirugia::etiqueta` lo dice una vez.
//!
//! # Dos capacidades distintas en la misma pantalla, y no es un descuido
//!
//! PROGRAMAR una cirugía exige `EscribirClinico` —es un acto de la historia— y
//! CONDUCIRLA (lista de verificación, iniciar, completar, cancelar) exige
//! `AtenderUrgencias`, que es la de quien está en la sala. Son las mismas dos
//! que exigen las rutas JSON, y cada botón se dibuja para quien puede pulsarlo.
//!
//! # El listado deja eslabón, porque el suyo lo deja
//!
//! `GET /api/quirofano/programaciones` anota una colección de la Ley 1581: cruza
//! titulares y devuelve nombre, documento y qué cirugía se le practica a cada
//! uno. Es de los listados de carga más sensible, así que esta pantalla anota
//! **con los mismos argumentos** — verbo, recurso y filtro incluidos. Con un
//! verbo distinto, el rastro de un mismo hecho quedaría partido según qué
//! frontal lo abriera.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::quirofano;
use medico_dominio::quirofano::{Accion, EstadoCirugia};
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{AtenderUrgencias, Estado, EscribirClinico};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/quirofano";
const SALAS: &str = "/panel/quirofano/salas";
const PROGRAMAR: &str = "/panel/quirofano/programar";

/// Las cuatro modalidades de anestesia que el producto ofrece.
const ANESTESIAS: [(&str, &str); 4] = [
    ("GENERAL", "General"),
    ("REGIONAL", "Regional"),
    ("LOCAL", "Local"),
    ("SEDACION", "Sedación"),
];

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "programada" => Some((Tono::Ok, "Cirugía programada.")),
        "checklist" => Some((Tono::Ok, "Lista de verificación actualizada.")),
        "iniciada" => Some((Tono::Ok, "Cirugía iniciada. La sala queda ocupada.")),
        "completada" => Some((
            Tono::Ok,
            "Cirugía completada. La sala vuelve a quedar disponible.",
        )),
        "cancelada" => Some((
            Tono::Ok,
            "Cirugía cancelada. La sala vuelve a quedar disponible.",
        )),
        "suspendida" => Some((
            Tono::Ok,
            "Cirugía suspendida. La sala vuelve a quedar disponible; se puede reanudar o cancelar.",
        )),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

// ── La agenda ────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    #[serde(default)]
    pub estado: String,
}

/// `GET /panel/quirofano`
pub async fn agenda(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let estado = f.estado.trim();
    // Un estado que el dominio no conoce NO se pasa a la consulta: devolvería la
    // lista vacía y «no hay cirugías en ese estado» se leería como un hecho
    // (directiva 20).
    if !estado.is_empty() && EstadoCirugia::desde_texto(estado).is_none() {
        return pantalla(
            &cx,
            "Quirófano",
            html! {
                (cabecera(&cx, "agenda", ""))
                section class="tarjeta" {
                    (aviso(Tono::Error, &format!(
                        "«{estado}» no es un estado de cirugía. Los que hay son: {}.",
                        EstadoCirugia::TODOS.map(|x| x.como_texto()).join(", ")
                    )))
                }
            },
        );
    }

    let lista = match quirofano::listar_programaciones(
        &e.pool,
        u.ambito(),
        (!estado.is_empty()).then_some(estado),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // Los MISMOS argumentos que `GET /api/quirofano/programaciones`: este
    // listado cruza titulares y devuelve nombre, documento y procedimiento.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        "quirofano",
        (!estado.is_empty()).then(|| estado.to_string()),
        lista.len(),
        vec![],
    )
    .await;

    pantalla(
        &cx,
        "Quirófano",
        html! {
            (cabecera(&cx, "agenda", estado))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            @if lista.is_empty() {
                (piezas::vacio(
                    !estado.is_empty(),
                    "No hay cirugías programadas.",
                    "Ninguna cirugía está en ese estado.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(lista.len(), lista.len(), "cirugía", "cirugías"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Documento", "Procedimiento", "Sala", "Cirujano",
                      "Verificación", "Estado"],
                    html! {
                        @for p in &lista {
                            tr {
                                td class="mono" { (p.fecha_programada) }
                                td {
                                    a href={ (RUTA) "/" (p.id) } {
                                        (p.paciente_nombre.clone().unwrap_or_else(|| "—".into()))
                                    }
                                }
                                td class="mono" {
                                    (p.paciente_identificacion.clone().unwrap_or_default())
                                }
                                td {
                                    (p.procedimiento)
                                    @if let Some(c) = &p.codigo_cups {
                                        span class="sutil" { " · CUPS " (c) }
                                    }
                                }
                                td { (p.quirofano_nombre.clone().unwrap_or_default()) }
                                td { (p.cirujano_nombre.clone().unwrap_or_else(|| "—".into())) }
                                td { (resumen_checklist(p)) }
                                td { (etiqueta_estado(&p.estado)) }
                            }
                        }
                    },
                ))
            }
        },
    )
}

/// Cuántos puntos de la lista de verificación están hechos, **de cuántos**.
///
/// «3 de 4» y no «75 %»: lo que decide si una cirugía puede empezar es cuál
/// falta, no la proporción, y un porcentaje esconde que el que falta es la
/// valoración preanestésica.
fn resumen_checklist(p: &quirofano::Programacion) -> Markup {
    let puntos = [
        p.ayuno_verificado,
        p.labs_prequirurgicos,
        p.valoracion_preanestesica,
        p.reserva_sangre,
    ];
    let hechos = puntos.iter().filter(|x| **x).count();
    html! {
        span class=[(hechos < puntos.len()).then_some("falta")] {
            (hechos) " de " (puntos.len())
        }
    }
}

/// El estado, con su etiqueta del dominio y el color de la pastilla.
fn etiqueta_estado(texto: &str) -> Markup {
    match EstadoCirugia::desde_texto(texto) {
        Some(e) => html! {
            span class="pastilla" title=(e.como_texto()) { (piezas::estado(e.como_texto())) }
        },
        // **Un estado corrupto se enseña tal cual y marcado.** No se traduce a
        // «programada» ni se esconde: la fila existe y lo que tiene escrito es
        // un dato, aunque sea uno que el dominio no reconoce.
        None => html! { span class="falta" { (texto) " (desconocido)" } },
    }
}

fn cabecera(cx: &Contexto, donde: &str, estado_activo: &str) -> Markup {
    html! {
        header class="titulo-seccion" {
            h1 { "Quirófano" }
            nav class="buscador" {
                a class=[(donde == "agenda").then_some("enlace-activo")] href=(RUTA) { "Agenda" }
                a class=[(donde == "salas").then_some("enlace-activo")] href=(SALAS) { "Salas" }
                @if cx.puede(Capacidad::EscribirClinico) {
                    a href=(PROGRAMAR) { "Programar" }
                }
            }
        }
        @if donde == "agenda" {
            section class="tarjeta" {
                nav class="buscador" {
                    a class=[estado_activo.is_empty().then_some("enlace-activo")] href=(RUTA) {
                        "Todas"
                    }
                    @for s in EstadoCirugia::TODOS {
                        a class=[(estado_activo == s.como_texto()).then_some("enlace-activo")]
                          href={ (RUTA) "?estado=" (s.como_texto()) } { (s.etiqueta()) }
                    }
                }
            }
        }
    }
}

// ── Las salas ────────────────────────────────────────────────────────────────

/// `GET /panel/quirofano/salas`
pub async fn salas(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match quirofano::listar_salas(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Salas de cirugía",
        html! {
            (cabecera(&cx, "salas", ""))
            @if lista.is_empty() {
                p class="sutil" { "No hay salas registradas." }
            } @else {
                (piezas::tabla(
                    &["Sala", "Tipo", "Estado", "Equipamiento"],
                    html! {
                        @for s in &lista {
                            tr {
                                td { (s.nombre) }
                                td class="sutil" { (s.tipo) }
                                td { (piezas::estado(&s.estado)) }
                                td class="sutil" { (equipamiento(&s.equipamiento)) }
                            }
                        }
                    },
                ))
            }
        },
    )
}

/// El equipamiento de una sala, que es un JSON de forma abierta.
///
/// Se enseña como pares `clave: valor` y **no se interpreta**: el esquema lo
/// declara libre a propósito, así que inventarle campos aquí haría que un
/// inventario con otra forma se viera vacío en vez de verse raro.
fn equipamiento(v: &serde_json::Value) -> String {
    match v.as_object() {
        Some(o) if !o.is_empty() => o
            .iter()
            .map(|(k, v)| match v.as_str() {
                Some(s) => format!("{k}: {s}"),
                None => format!("{k}: {v}"),
            })
            .collect::<Vec<_>>()
            .join(" · "),
        _ => String::new(),
    }
}

// ── Programar ────────────────────────────────────────────────────────────────

/// `GET /panel/quirofano/programar`
pub async fn formulario(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_formulario(&e, &cx, &u, &HashMap::new(), None).await
}

async fn dibujar_formulario(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    v: &HashMap<String, String>,
    error: Option<&str>,
) -> Response {
    let (salas, medicos) = match (
        quirofano::listar_salas(&e.pool, u.ambito()).await,
        quirofano::listar_medicos(&e.pool, u.ambito()).await,
    ) {
        (Ok(s), Ok(m)) => (s, m),
        _ => return averia(),
    };
    let puesto = |k: &str| v.get(k).cloned().unwrap_or_default();
    // El buscador y no el padrón entero (ver `buscador_paciente`).
    let q_paciente = puesto("q_paciente");
    let (pacientes, total_pacientes) =
        match crate::vista::buscador_paciente::encontrar(e, u, &q_paciente).await {
            Ok(x) => x,
            Err(_) => return averia(),
        };
    // Solo se ofrecen las salas DISPONIBLES: `crear` rechaza las demás con su
    // motivo, y ofrecer lo que va a fallar es enseñar la regla dos veces. Las
    // otras se dicen abajo, con su estado, en vez de esconderlas.
    let (libres, ocupadas): (Vec<_>, Vec<_>) =
        salas.iter().partition(|s| s.estado == "DISPONIBLE");

    pantalla(
        cx,
        "Programar cirugía",
        html! {
            (piezas::migas(RUTA, "Quirófano"))
            header class="titulo-seccion" { h1 { "Programar una cirugía" } }
            section class="tarjeta" {
                @if let Some(t) = error { (aviso(Tono::Error, t)) }
                @if libres.is_empty() {
                    (aviso(Tono::Error, "No hay ninguna sala disponible ahora mismo."))
                } @else if medicos.is_empty()
                    || (pacientes.is_empty() && q_paciente.trim().is_empty())
                {
                    (aviso(
                        Tono::Error,
                        "Hacen falta un paciente y un cirujano registrados para programar.",
                    ))
                } @else {
                    form method="post" action=(PROGRAMAR) class="apilado" {
                        label for="quirofano_id" { "Sala" }
                        select #quirofano_id name="quirofano_id" required {
                            option value="" { "Elija una" }
                            @for s in &libres {
                                option value=(s.id) selected[puesto("quirofano_id") == s.id.to_string()] {
                                    (s.nombre) " — " (s.tipo)
                                }
                            }
                        }

                        (crate::vista::buscador_paciente::campo(
                            &q_paciente, &puesto("paciente_id"), &pacientes, total_pacientes,
                        ))

                        label for="cirujano_id" { "Cirujano" }
                        select #cirujano_id name="cirujano_id" required {
                            option value="" { "Elija uno" }
                            @for m in &medicos {
                                option value=(m.id) selected[puesto("cirujano_id") == m.id.to_string()] {
                                    (m.nombre_completo) " — " (m.rol)
                                }
                            }
                        }

                        label for="anestesiologo_id" { "Anestesiólogo" }
                        select #anestesiologo_id name="anestesiologo_id" {
                            option value="" { "Sin asignar" }
                            @for m in &medicos {
                                option value=(m.id) selected[puesto("anestesiologo_id") == m.id.to_string()] {
                                    (m.nombre_completo) " — " (m.rol)
                                }
                            }
                        }

                        label for="fecha" { "Fecha y hora" }
                        input #fecha type="datetime-local" name="fecha_programada" required
                              value=(puesto("fecha_programada"));
                        label for="duracion" { "Duración estimada (minutos)" }
                        input #duracion type="number" name="duracion_estimada_min" min="1"
                              value=(if puesto("duracion_estimada_min").is_empty() {
                                  "60".to_string()
                              } else {
                                  puesto("duracion_estimada_min")
                              });

                        label for="procedimiento" { "Procedimiento" }
                        input #procedimiento type="text" name="procedimiento" required
                              value=(puesto("procedimiento")) placeholder="Apendicectomía";
                        label for="cups" { "Código CUPS" }
                        input #cups type="text" name="codigo_cups" value=(puesto("codigo_cups"));

                        label for="anestesia" { "Tipo de anestesia" }
                        select #anestesia name="tipo_anestesia" {
                            option value="" { "Sin definir" }
                            @for (val, nom) in ANESTESIAS {
                                option value=(val) selected[puesto("tipo_anestesia") == val] { (nom) }
                            }
                        }

                        button type="submit" class="principal" { "Programar" }
                    }
                }
            }

            @if !ocupadas.is_empty() {
                section class="tarjeta" {
                    h2 { "Salas que no se pueden usar ahora" }
                    p class="sutil" {
                        "Se dicen en vez de esconderlas: una sala que desaparece de la lista "
                        "sin explicación se lee como un fallo del sistema."
                    }
                    (piezas::tabla(
                        &["Sala", "Estado"],
                        html! {
                            @for s in &ocupadas {
                                tr { td { (s.nombre) } td { (piezas::estado(&s.estado)) } }
                            }
                        },
                    ))
                }
            }
        },
    )
}

/// `POST /panel/quirofano/programar`
pub async fn programar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let entero = |k: &str| campos.get(k).and_then(|v| v.trim().parse::<i64>().ok());
    let (Some(quirofano_id), Some(paciente_id), Some(cirujano_id)) = (
        entero("quirofano_id"),
        entero("paciente_id"),
        entero("cirujano_id"),
    ) else {
        return dibujar_formulario(
            &e,
            &cx,
            &u,
            &campos,
            Some("Elija la sala, el paciente y el cirujano."),
        )
        .await;
    };
    // `datetime-local` manda `2026-09-01T13:00` y el dominio exige RFC 3339. Se
    // traduce aquí, que es donde se conoce el `<input>`; los segundos y la zona
    // los pone la pantalla porque el navegador no los manda.
    let fecha = match campos.get("fecha_programada").map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(f) => format!("{f}:00Z"),
        None => {
            return dibujar_formulario(&e, &cx, &u, &campos, Some("Falta la fecha y la hora."))
                .await
        }
    };
    let nueva = quirofano::ProgramacionNueva {
        quirofano_id,
        paciente_id,
        cirujano_id,
        anestesiologo_id: entero("anestesiologo_id"),
        consentimiento_id: None,
        fecha_programada: fecha,
        duracion_estimada_min: entero("duracion_estimada_min").unwrap_or(60),
        procedimiento: campos
            .get("procedimiento")
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string(),
        codigo_cups: texto(&campos, "codigo_cups"),
        tipo_anestesia: texto(&campos, "tipo_anestesia")
            .filter(|v| ANESTESIAS.iter().any(|(x, _)| x == v)),
    };
    match quirofano::crear(&e.pool, u.ambito(), &nueva).await {
        Ok(Ok(_)) => de_vuelta(RUTA, "programada"),
        // El motivo lleva el campo dentro —«la sala no está disponible (estado:
        // MANTENIMIENTO)»— así que no se redirige: se vuelve al formulario con
        // lo tecleado, igual que en el alta de ambulancias.
        Ok(Err(f)) => dibujar_formulario(&e, &cx, &u, &campos, Some(&f.mensaje())).await,
        Err(_) => averia(),
    }
}

fn texto(campos: &HashMap<String, String>, k: &str) -> Option<String> {
    campos.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

// ── La ficha ─────────────────────────────────────────────────────────────────

/// `GET /panel/quirofano/{id}`
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
    let p = match quirofano::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => p,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let conduce = cx.puede(Capacidad::AtenderUrgenciasHospitalizacion);
    let estado = EstadoCirugia::desde_texto(&p.estado);
    // **Qué se puede hacer lo dice el DOMINIO**, no una lista de esta pantalla.
    // Preguntándoselo, un botón imposible no se dibuja y la regla se enseña una
    // sola vez.
    let cabe = |a: Accion| {
        estado.is_some_and(|s| medico_dominio::quirofano::transicion(s, a).is_ok())
    };

    pantalla(
        &cx,
        "Cirugía",
        html! {
            (piezas::migas(RUTA, "Quirófano"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            header class="titulo-seccion" {
                h1 { (p.procedimiento) }
                (etiqueta_estado(&p.estado))
            }
            @if estado.is_none() {
                (aviso(
                    Tono::Averia,
                    "Esta cirugía tiene un estado que el sistema no reconoce, así que no se \
                     puede mover de sitio. Es un dato de la base que hay que corregir.",
                ))
            }

            section class="tarjeta" {
                (piezas::datos(&[
                    ("Paciente", p.paciente_nombre.clone()),
                    ("Documento", p.paciente_identificacion.clone()),
                    ("Sala", p.quirofano_nombre.clone()),
                    ("Cirujano", p.cirujano_nombre.clone()),
                    ("Anestesiólogo", p.anestesiologo_nombre.clone()),
                    ("Fecha programada", Some(p.fecha_programada.clone())),
                    ("Duración estimada", Some(format!("{} min", p.duracion_estimada_min))),
                    ("CUPS", p.codigo_cups.clone()),
                    ("Tipo de anestesia", p.tipo_anestesia.clone()),
                    ("Hora de inicio real", p.hora_inicio_real.clone()),
                    ("Hora de fin real", p.hora_fin_real.clone()),
                ]))
            }

            (seccion_checklist(&p, conduce))

            @if let Some(nota) = &p.nota_quirurgica {
                section class="tarjeta" {
                    h2 { "Nota quirúrgica" }
                    (piezas::datos(&[
                        ("Nota", Some(nota.clone())),
                        ("Hallazgos", p.hallazgos_quirurgicos.clone()),
                        ("Sangrado estimado", p.sangrado_estimado_ml.map(|x| format!("{x} mL"))),
                        ("Especímenes a patología", p.especimenes_patologia.clone()),
                        ("Complicaciones", p.complicaciones.clone()),
                    ]))
                }
            }

            @if conduce {
                @if cabe(Accion::Iniciar) {
                    // Reanudar es `Iniciar` sobre una SUSPENDIDA: el mismo verbo,
                    // pero se registra la hora de REINICIO sin borrar la del primer
                    // inicio (#537). La pantalla lo dice para no llamar «iniciar» a
                    // lo que continúa algo ya empezado.
                    @let reanuda = estado == Some(EstadoCirugia::Suspendida);
                    section class="tarjeta" {
                        h2 { (if reanuda { "Reanudar" } else { "Iniciar" }) }
                        p class="sutil" {
                            @if reanuda {
                                "La sala vuelve a ocuparse y se registra la hora de reinicio, \
                                 sin borrar la del primer inicio."
                            } @else {
                                "La sala pasa a ocupada y se registra la hora de inicio."
                            }
                        }
                        (falta_checklist(&p))
                        form method="post" action={ (RUTA) "/" (p.id) "/iniciar" } {
                            button type="submit" class="principal" {
                                (if reanuda { "Reanudar la cirugía" } else { "Iniciar la cirugía" })
                            }
                        }
                    }
                }
                @if cabe(Accion::Completar) {
                    section class="tarjeta" {
                        h2 { "Completar" }
                        p class="sutil" {
                            "Cierra la cirugía y libera la sala. La nota se puede escribir "
                            "ahora o después: obligarla aquí dejaría cirugías abiertas."
                        }
                        form method="post" action={ (RUTA) "/" (p.id) "/completar" }
                             class="apilado" {
                            label for="nota" { "Nota quirúrgica" }
                            textarea #nota name="nota_quirurgica" rows="3" {}
                            label for="hallazgos" { "Hallazgos" }
                            textarea #hallazgos name="hallazgos_quirurgicos" rows="2" {}
                            label for="sangrado" { "Sangrado estimado (mL)" }
                            input #sangrado type="number" name="sangrado_estimado_ml" min="0";
                            label for="especimenes" { "Especímenes a patología" }
                            input #especimenes type="text" name="especimenes_patologia";
                            label for="complicaciones" { "Complicaciones" }
                            textarea #complicaciones name="complicaciones" rows="2" {}
                            button type="submit" class="principal"
                                    data-confirmar="Pulse otra vez para completarla" {
                                "Completar la cirugía"
                            }
                        }
                    }
                }
                @if cabe(Accion::Suspender) {
                    section class="tarjeta" {
                        h2 { "Suspender" }
                        p class="sutil" {
                            "«Hoy no, pero sigue pendiente»: la sala se libera y la cirugía queda \
                             SUSPENDIDA, desde donde luego se reanuda o se cancela."
                        }
                        form method="post" action={ (RUTA) "/" (p.id) "/suspender" } {
                            button type="submit"
                                    data-confirmar="Pulse otra vez para suspenderla" {
                                "Suspender la cirugía"
                            }
                        }
                    }
                }
                @if cabe(Accion::Cancelar) {
                    section class="tarjeta" {
                        h2 { "Cancelar" }
                        form method="post" action={ (RUTA) "/" (p.id) "/cancelar" }
                             class="apilado" {
                            label for="motivo" { "Motivo" }
                            input #motivo type="text" name="motivo" required
                                  placeholder="Por qué no se hace";
                            button type="submit"
                                    data-confirmar="Pulse otra vez para cancelarla" {
                                "Cancelar la cirugía"
                            }
                        }
                    }
                }
            }
        },
    )
}

/// La lista de verificación, con su formulario si quien mira conduce la cirugía.
fn seccion_checklist(p: &quirofano::Programacion, conduce: bool) -> Markup {
    let cerrada = EstadoCirugia::desde_texto(&p.estado).is_some_and(|e| e.es_final());
    html! {
        section class="tarjeta" {
            h2 { "Lista de verificación prequirúrgica" }
            @if conduce && !cerrada {
                form method="post" action={ (RUTA) "/" (p.id) "/checklist" } class="apilado" {
                    label {
                        input type="checkbox" name="ayuno_verificado" value="1"
                              checked[p.ayuno_verificado];
                        " Ayuno verificado"
                    }
                    label {
                        input type="checkbox" name="labs_prequirurgicos" value="1"
                              checked[p.labs_prequirurgicos];
                        " Laboratorios prequirúrgicos"
                    }
                    label {
                        input type="checkbox" name="valoracion_preanestesica" value="1"
                              checked[p.valoracion_preanestesica];
                        " Valoración preanestésica"
                    }
                    label {
                        input type="checkbox" name="reserva_sangre" value="1"
                              checked[p.reserva_sangre];
                        " Reserva de sangre"
                    }
                    label for="hemo" { "Hemoclasificación" }
                    // Vaciarlo BORRA el grupo, y es deliberado: en una lista de
                    // verificación «no lo sé» y «O+ equivocado» no son lo mismo.
                    input #hemo type="text" name="hemoclasificacion"
                          value=(p.hemoclasificacion.clone().unwrap_or_default())
                          placeholder="O+";
                    p class="sutil" {
                        "Dejar la hemoclasificación en blanco la BORRA: «no lo sé» y un grupo "
                        "equivocado no son lo mismo."
                    }
                    button type="submit" { "Guardar la verificación" }
                }
            } @else {
                (piezas::datos(&[
                    ("Ayuno verificado", Some(si_no(p.ayuno_verificado))),
                    ("Laboratorios prequirúrgicos", Some(si_no(p.labs_prequirurgicos))),
                    ("Valoración preanestésica", Some(si_no(p.valoracion_preanestesica))),
                    ("Reserva de sangre", Some(si_no(p.reserva_sangre))),
                    ("Hemoclasificación", p.hemoclasificacion.clone()),
                ]))
                @if cerrada {
                    p class="sutil" { "La cirugía está cerrada: la verificación ya no se toca." }
                }
            }
        }
    }
}

fn si_no(v: bool) -> String {
    if v { "sí".into() } else { "no".into() }
}

/// Qué puntos de la verificación faltan, dichos POR SU NOMBRE.
///
/// **No impide iniciar**: quién puede hacerlo lo decide la máquina de estados, y
/// una urgencia se opera con la lista a medias. Lo que no puede pasar es que se
/// inicie sin que nadie se entere de qué falta.
fn falta_checklist(p: &quirofano::Programacion) -> Markup {
    let faltan: Vec<&str> = [
        (!p.ayuno_verificado).then_some("ayuno"),
        (!p.labs_prequirurgicos).then_some("laboratorios"),
        (!p.valoracion_preanestesica).then_some("valoración preanestésica"),
    ]
    .into_iter()
    .flatten()
    .collect();
    html! {
        @if !faltan.is_empty() {
            (aviso(Tono::Averia, &format!(
                "Sin verificar: {}. No impide iniciar —una urgencia se opera igual— pero \
                 queda dicho.",
                faltan.join(", ")
            )))
        }
    }
}

// ── Las transiciones ─────────────────────────────────────────────────────────

/// `POST /panel/quirofano/{id}/checklist`
pub async fn checklist(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AtenderUrgencias>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    // Las cuatro casillas se mandan SIEMPRE con su valor resultante —marcada o
    // no—, porque el formulario las trae todas: es un `Some(false)` explícito y
    // no un «no lo toques». Así desmarcar una la desmarca de verdad.
    let cambio = quirofano::ChecklistCambio {
        ayuno_verificado: Some(campos.contains_key("ayuno_verificado")),
        labs_prequirurgicos: Some(campos.contains_key("labs_prequirurgicos")),
        valoracion_preanestesica: Some(campos.contains_key("valoracion_preanestesica")),
        reserva_sangre: Some(campos.contains_key("reserva_sangre")),
        // Vacío BORRA, que es lo que el dominio ya decidió.
        hemoclasificacion: Some(
            campos.get("hemoclasificacion").map(|s| s.trim().to_string()).unwrap_or_default(),
        ),
    };
    match quirofano::actualizar_checklist(&e.pool, u.ambito(), id, &cambio).await {
        Ok(r) => tras_mover(&e, &cx, &u, id, r, "checklist").await,
        Err(_) => averia(),
    }
}

/// `POST /panel/quirofano/{id}/iniciar`
pub async fn iniciar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AtenderUrgencias>,
    Path(id): Path<i64>,
) -> Response {
    mover(e, u, id, Accion::Iniciar, Default::default(), "iniciada").await
}

/// `POST /panel/quirofano/{id}/completar`
pub async fn completar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AtenderUrgencias>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cierre = quirofano::CierreCirugia {
        nota_quirurgica: texto(&campos, "nota_quirurgica"),
        hallazgos_quirurgicos: texto(&campos, "hallazgos_quirurgicos"),
        sangrado_estimado_ml: campos
            .get("sangrado_estimado_ml")
            .and_then(|v| v.trim().parse::<i64>().ok()),
        especimenes_patologia: texto(&campos, "especimenes_patologia"),
        complicaciones: texto(&campos, "complicaciones"),
        motivo: None,
    };
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    // La misma regla que `/api`: cerrar la sala lo hace quien conduce, pero la
    // DESCRIPCIÓN QUIRÚRGICA es acto médico. Hasta el 2026-08-10 esta puerta la
    // dejaba escribir a todo el equipo y la de la API no dejaba cerrar al
    // anestesiólogo; cada una acertaba en una mitad.
    if medico_dominio::quirofano::lleva_descripcion_quirurgica(
        cierre.nota_quirurgica.as_deref(),
        cierre.hallazgos_quirurgicos.as_deref(),
        cierre.complicaciones.as_deref(),
    ) && !cx.puede(medico_dominio::rol::Capacidad::EscribirClinico)
    {
        return pantalla(
            &cx,
            "Quirófano",
            html! {
                section class="tarjeta" {
                    (aviso(
                        Tono::Error,
                        "La descripción quirúrgica —nota, hallazgos y complicaciones— es un \
                         anexo de la historia clínica que diligencia el médico \
                         (Res. 1995/1999). Puede cerrar la cirugía sin ella.",
                    ))
                    p { a href=(RUTA) { "Volver" } }
                }
            },
        );
    }
    mover(e, u, id, Accion::Completar, cierre, "completada").await
}

/// `POST /panel/quirofano/{id}/cancelar`
pub async fn cancelar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AtenderUrgencias>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cierre = quirofano::CierreCirugia {
        motivo: texto(&campos, "motivo"),
        ..Default::default()
    };
    mover(e, u, id, Accion::Cancelar, cierre, "cancelada").await
}

/// `POST /panel/quirofano/{id}/suspender`
pub async fn suspender(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AtenderUrgencias>,
    Path(id): Path<i64>,
) -> Response {
    mover(e, u, id, Accion::Suspender, Default::default(), "suspendida").await
}

async fn mover(
    e: Estado,
    u: medico_datos::usuario::Usuario,
    id: i64,
    accion: Accion,
    datos: quirofano::CierreCirugia,
    bien: &str,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match quirofano::mover(&e.pool, u.ambito(), id, accion, &datos).await {
        Ok(r) => tras_mover(&e, &cx, &u, id, r, bien).await,
        Err(_) => averia(),
    }
}

/// Traduce el resultado del dominio a la pantalla siguiente.
///
/// `Rechazado` y `EstadoCorrupto` llevan el motivo DENTRO, así que no se
/// redirigen: se vuelve a dibujar la ficha con él. El aviso de la URL es una
/// lista cerrada y no puede llevar texto del servidor.
async fn tras_mover(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    id: i64,
    r: quirofano::Resultado,
    bien: &str,
) -> Response {
    match r {
        quirofano::Resultado::Hecho => de_vuelta(&format!("{RUTA}/{id}"), bien),
        quirofano::Resultado::NoExiste => no_existe(cx),
        quirofano::Resultado::Rechazado(motivo) => con_motivo(e, cx, u, id, &motivo).await,
        quirofano::Resultado::EstadoCorrupto(estado) => {
            con_motivo(
                e,
                cx,
                u,
                id,
                &format!(
                    "Esta cirugía tiene el estado «{estado}», que el sistema no reconoce. \
                     No se puede mover hasta que se corrija en la base."
                ),
            )
            .await
        }
    }
}

async fn con_motivo(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    id: i64,
    motivo: &str,
) -> Response {
    let Ok(Some(p)) = quirofano::obtener(&e.pool, u.ambito(), id).await else {
        return no_existe(cx);
    };
    (
        StatusCode::CONFLICT,
        pagina(
            "Cirugía",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                (piezas::migas(RUTA, "Quirófano"))
                header class="titulo-seccion" {
                    h1 { (p.procedimiento) }
                    (etiqueta_estado(&p.estado))
                }
                section class="tarjeta" {
                    (aviso(Tono::Error, motivo))
                    p { a href={ (RUTA) "/" (id) } { "Volver a la cirugía" } }
                }
            },
        ),
    )
        .into_response()
}

// ── Piezas ───────────────────────────────────────────────────────────────────

fn pantalla(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA)), cuerpo).into_response()
}

fn no_existe(cx: &Contexto) -> Response {
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrada",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró esa cirugía" }
                    p { a href=(RUTA) { "Volver a Quirófano" } }
                }
            },
        ),
    )
        .into_response()
}
