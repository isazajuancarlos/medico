// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Transporte asistencial: el parque de ambulancias y sus traslados.
//!
//! Port de `frontend/src/app/pages/ambulancias/ambulancias.component.ts` (673
//! líneas: la pantalla, el diálogo de alta y el de despacho).
//!
//! # Las pestañas y los diálogos se convierten en URL
//!
//! Angular tenía un `signal<'parque'|'traslados'>` y dos `MatDialog`. Aquí son
//! cinco direcciones —parque, traslados, alta, ficha y despacho—, que es lo que
//! aquel estado imitaba: con una URL por cosa, el «atrás» funciona, una ficha se
//! enlaza y recargar no pierde el sitio. Un diálogo modal es además el peor sitio
//! para un formulario de ocho campos en el portátil de un consultorio.
//!
//! # La regla del despacho NO se copia: se pregunta
//!
//! El componente de Angular calculaba las ambulancias despachables con un
//! `computed` que repetía la regla del servidor, y su comentario lo admitía. Esa
//! copia se queda atrás en el primer arreglo, y entonces la pantalla ofrece lo
//! que el servidor niega. Aquí se llama a
//! [`medico_datos::ambulancia::motivo_para_no_despachar`], que es la MISMA
//! función que usa `despachar` — extraída para esto (directiva 1).
//!
//! Y con ella se puede hacer algo que Angular no hacía: **decir por qué** no sale
//! cada una, en vez de esconderla. Una ambulancia que desaparece de la lista sin
//! explicación se lee como un fallo del sistema.
//!
//! # Escribir exige `GestionarAmbulancias`; mirar, no
//!
//! Igual que `/api/ambulancias`: saber si hay una ambulancia disponible es lo
//! primero que necesita quien atiende una urgencia, y un 403 ahí no protege nada.
//! Las seis escrituras exigen la capacidad, y sus botones solo se dibujan para
//! quien puede pulsarlos.
//!
//! # Los tres estados del SOAT vienen del servidor y no se recalculan
//!
//! `soat_vencido`, `soat_por_vencer` y `soat_sin_registrar` los trae ya resueltos
//! `ambulancia::listar`. **«Sin registrar» no es «al día»**: no saber no es estar
//! en regla, y tampoco es motivo para no despachar una urgencia. Los tres se
//! pintan distinto a propósito.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::ambulancia::{self, Ambulancia, NoSale};
use medico_dominio::rol::Capacidad;
use serde::Deserialize;

use crate::auth::{Estado, GestionarAmbulancias};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/ambulancias";
const TRASLADOS: &str = "/panel/ambulancias/traslados";

// ── Avisos ───────────────────────────────────────────────────────────────────

/// Lo que salió bien en la escritura anterior, traducido por **lista cerrada**:
/// el valor viaja en la URL del 303, así que lo teclea cualquiera y aceptar
/// texto libre sería dejar que eligiera lo que la pantalla afirma (directiva 23).
///
/// Aquí solo caben los finales SIN datos dentro. Los que llevan un dato —«ya hay
/// una ambulancia con placa WXY123», «está en el traslado #4»— no se redirigen:
/// se vuelve a dibujar el formulario con el motivo y con lo que la persona había
/// escrito, que además es lo único que no le obliga a teclearlo todo otra vez.
fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "creada" => Some((Tono::Ok, "Ambulancia registrada.")),
        "guardada" => Some((Tono::Ok, "Cambios guardados.")),
        "de_baja" => Some((
            Tono::Ok,
            "Ambulancia dada de baja: deja de poder despacharse y sigue explicando los \
             traslados que ya hizo.",
        )),
        "reactivada" => Some((Tono::Ok, "Ambulancia reactivada.")),
        "despachada" => Some((Tono::Ok, "Ambulancia despachada.")),
        "finalizado" => Some((Tono::Ok, "Traslado finalizado: la ambulancia queda disponible.")),
        "cancelado" => Some((
            Tono::Ok,
            "Traslado cancelado. La ambulancia queda disponible y el traslado no registra \
             ninguna llegada, porque no llegó a ninguna parte.",
        )),
        "ya_cerrado" => Some((
            Tono::Error,
            "Ese traslado ya estaba cerrado. Su estado se ve en la tabla.",
        )),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

// ── El parque ────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub aviso: String,
}

/// `GET /panel/ambulancias`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let estado = f.estado.trim();
    if !estado.is_empty() && !ambulancia::ESTADOS.contains(&estado) {
        // Pasarlo a la consulta devolvería la lista vacía, que se lee como «no
        // hay ambulancias» (directiva 20).
        return pantalla(
            &cx,
            "Ambulancias",
            html! {
                (cabecera(&cx, "parque"))
                section class="tarjeta" {
                    (aviso(Tono::Error, &format!(
                        "«{estado}» no es un estado de este producto. Los que hay son: {}.",
                        ambulancia::ESTADOS.join(", ")
                    )))
                }
            },
        );
    }

    let hoy = medico_datos::hoy();
    // DOS consultas a propósito: el resumen es del parque ENTERO y la tabla es
    // de lo filtrado. Angular calculaba las cifras sobre la lista ya filtrada,
    // así que «SOAT vencido: 0» aparecía en cuanto alguien tecleaba en el
    // buscador — un cero que significaba «no en estos resultados» y se leía como
    // «ninguna».
    let todas = match ambulancia::listar(&e.pool, u.ambito(), &Default::default(), &hoy).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let filtros = ambulancia::FiltrosAmbulancia {
        q: (!f.q.trim().is_empty()).then(|| f.q.trim().to_string()),
        estado: (!estado.is_empty()).then(|| estado.to_string()),
        tipo: None,
        activa: None,
    };
    let hay_filtro = filtros.q.is_some() || filtros.estado.is_some();
    let mostradas = if hay_filtro {
        match ambulancia::listar(&e.pool, u.ambito(), &filtros, &hoy).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        todas.clone()
    };
    let en_curso = match ambulancia::listar_traslados(
        &e.pool,
        u.ambito(),
        &ambulancia::FiltrosTraslado {
            estado: Some("En Curso".into()),
            ..Default::default()
        },
    )
    .await
    {
        Ok(v) => v.len(),
        Err(_) => return averia(),
    };

    let puede = cx.puede(Capacidad::GestionarAmbulancias);
    let salen = todas.iter().filter(|a| ambulancia::motivo_para_no_despachar(a).is_none()).count();
    let vencido = todas.iter().filter(|a| a.activa && a.soat_vencido).count();
    let sin_registrar = todas.iter().filter(|a| a.activa && a.soat_sin_registrar).count();

    pantalla(
        &cx,
        "Ambulancias",
        html! {
            (cabecera(&cx, "parque"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }

            section class="tarjeta resumen" {
                div { span class="cifra" { (salen) } span class="sutil" { "pueden salir ahora" } }
                div {
                    span class="cifra" { (en_curso) }
                    span class="sutil" { a href=(TRASLADOS) { "traslados en curso" } }
                }
                div { span class="cifra" { (vencido) } span class="sutil" { "con el SOAT vencido" } }
                div {
                    span class="cifra" { (sin_registrar) }
                    span class="sutil" { "sin SOAT registrado" }
                }
            }

            @if vencido > 0 {
                (aviso(Tono::Error, &format!(
                    "{vencido} ambulancia(s) no pueden circular: el SOAT venció."
                )))
            }
            @if sin_registrar > 0 {
                (aviso(Tono::Averia, &format!(
                    "{sin_registrar} sin SOAT registrado. El sistema no puede afirmar que estén \
                     al día, y por eso no lo afirma — tampoco lo usa para negarles una salida."
                )))
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="buscador" {
                    input type="search" name="q" value=(f.q) placeholder="Placa o número interno";
                    // El estado se conserva al buscar: si no, teclear en el
                    // buscador borraría el filtro sin decirlo.
                    @if !estado.is_empty() {
                        input type="hidden" name="estado" value=(estado);
                    }
                    button type="submit" { "Buscar" }
                }
                nav class="buscador" {
                    a class=[estado.is_empty().then_some("enlace-activo")] href=(RUTA) { "Todos" }
                    @for s in ambulancia::ESTADOS {
                        a class=[(estado == s).then_some("enlace-activo")]
                          href={ (RUTA) "?estado=" (s) } { (s) }
                    }
                }
            }

            @if mostradas.is_empty() {
                (piezas::vacio(
                    hay_filtro,
                    "No hay ambulancias registradas.",
                    "Ninguna ambulancia coincide con ese filtro.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(mostradas.len(), todas.len(), "ambulancia", "ambulancias"))
                (piezas::tabla(
                    &["Placa", "Tipo", "Vehículo", "Estado", "SOAT", ""],
                    html! {
                        @for a in &mostradas {
                            tr {
                                td {
                                    a href={ (RUTA) "/" (a.id) } { (a.placa) }
                                    @if let Some(n) = &a.numero_interno {
                                        div class="sutil" { (n) }
                                    }
                                }
                                td { (a.tipo) " " span class="sutil" { (nombre_tipo(&a.tipo)) } }
                                td class="sutil" {
                                    (a.marca.clone().unwrap_or_default())
                                    " " (a.modelo.clone().unwrap_or_default())
                                }
                                td {
                                    @if a.activa {
                                        (piezas::estado(&a.estado))
                                    } @else {
                                        (piezas::estado("DADA DE BAJA"))
                                    }
                                    @if let Some(t) = a.traslado_en_curso {
                                        div class="sutil" { "traslado #" (t) }
                                    }
                                }
                                td { (soat(a)) }
                                td {
                                    @if puede && a.traslado_en_curso.is_none() {
                                        form method="post"
                                             action={ (RUTA) "/" (a.id) "/baja" } {
                                            button type="submit" class="enlace" {
                                                @if a.activa { "Dar de baja" } @else { "Reactivar" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                ))
            }
        },
    )
}

/// El SOAT, con sus TRES estados dibujados distinto. «Sin registrar» no se pinta
/// como «vigente» ni como «vencido» porque no es ninguno de los dos.
fn soat(a: &Ambulancia) -> Markup {
    html! {
        @if a.soat_sin_registrar {
            span class="falta" { "sin registrar" }
        } @else if a.soat_vencido {
            span class="pastilla pastilla-mal" {
                "venció " (a.vence_soat.clone().unwrap_or_default())
            }
        } @else if a.soat_por_vencer {
            span class="pastilla pastilla-curso" {
                "vence " (a.vence_soat.clone().unwrap_or_default())
            }
        } @else {
            span class="sutil" { (a.vence_soat.clone().unwrap_or_default()) }
        }
    }
}

fn nombre_tipo(t: &str) -> &'static str {
    match t {
        "TAB" => "básico",
        "TAM" => "medicalizado",
        // No se le inventa nombre a un tipo que no conocemos (directiva 20).
        _ => "",
    }
}

/// La cabecera común: título, las dos vistas y las acciones que quien mira puede
/// hacer.
fn cabecera(cx: &Contexto, donde: &str) -> Markup {
    let puede = cx.puede(Capacidad::GestionarAmbulancias);
    html! {
        header class="titulo-seccion" {
            h1 { "Ambulancias y traslados" }
            nav class="buscador" {
                a class=[(donde == "parque").then_some("enlace-activo")] href=(RUTA) { "Parque" }
                a class=[(donde == "traslados").then_some("enlace-activo")] href=(TRASLADOS) {
                    "Traslados"
                }
                @if puede {
                    a href={ (RUTA) "/despachar" } { "Despachar" }
                    a href={ (RUTA) "/nueva" } { "Nueva ambulancia" }
                }
            }
        }
    }
}

// ── Los traslados ────────────────────────────────────────────────────────────

/// `GET /panel/ambulancias/traslados`
pub async fn traslados(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match ambulancia::listar_traslados(&e.pool, u.ambito(), &Default::default()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarAmbulancias);

    pantalla(
        &cx,
        "Traslados",
        html! {
            (cabecera(&cx, "traslados"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            @if lista.is_empty() {
                (piezas::vacio(false, "No hay traslados registrados.", "", TRASLADOS))
            } @else {
                (piezas::recuento(lista.len(), lista.len(), "traslado", "traslados"))
                (piezas::tabla(
                    &["Salida", "Paciente", "Ambulancia", "Tipo", "Ruta", "Llegada", "Estado", ""],
                    html! {
                        @for t in &lista {
                            tr {
                                td class="mono" { (t.fecha_salida) }
                                td {
                                    (t.paciente_nombre.clone().unwrap_or_else(|| "—".into()))
                                    div class="sutil mono" {
                                        (t.paciente_identificacion.clone().unwrap_or_default())
                                    }
                                }
                                td {
                                    (t.ambulancia_placa.clone().unwrap_or_default())
                                    div class="sutil" {
                                        (t.ambulancia_tipo.clone().unwrap_or_default())
                                    }
                                }
                                td { (t.tipo_traslado) }
                                td {
                                    (t.origen) " → " (t.destino)
                                    div class="sutil" { (t.motivo) }
                                }
                                td class="mono" {
                                    @match &t.fecha_llegada {
                                        Some(f) => (f),
                                        // Un cancelado no llegó a ninguna parte,
                                        // y se dice en vez de dejar el hueco.
                                        None => span class="falta" { "no llegó" },
                                    }
                                }
                                td { (piezas::estado(&t.estado)) }
                                td {
                                    @if puede && t.estado == "En Curso" {
                                        form method="post"
                                             action={ (TRASLADOS) "/" (t.id) "/finalizar" } {
                                            button type="submit" class="enlace" { "Finalizar" }
                                        }
                                        form method="post"
                                             action={ (TRASLADOS) "/" (t.id) "/cancelar" } {
                                            // Cancelar deja el traslado sin
                                            // llegada para siempre: es un cierre,
                                            // no una pausa.
                                            button type="submit" class="enlace"
                                                    data-confirmar="Pulse otra vez para cancelarlo" {
                                                "Cancelar"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                ))
            }
        },
    )
}

/// `POST /panel/ambulancias/traslados/{id}/finalizar`
pub async fn finalizar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
    Path(id): Path<i64>,
) -> Response {
    cerrar(&e, &u, id, false).await
}

/// `POST /panel/ambulancias/traslados/{id}/cancelar`
pub async fn cancelar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
    Path(id): Path<i64>,
) -> Response {
    cerrar(&e, &u, id, true).await
}

async fn cerrar(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    id: i64,
    cancelado: bool,
) -> Response {
    use ambulancia::ResultadoCierre as R;
    // `Cierre` vacío: la llegada es AHORA. La pantalla no ofrece escribirla a
    // mano, así que `LlegadaAntesDeSalida` no puede salir por aquí — y si algún
    // día se ofrece, ese final tendrá que dibujarse, no traducirse a «avería».
    let c = ambulancia::Cierre::default();
    match ambulancia::cerrar(&e.pool, u.ambito(), id, &c, cancelado).await {
        Ok(R::Hecho(_)) => de_vuelta(TRASLADOS, if cancelado { "cancelado" } else { "finalizado" }),
        Ok(R::YaCerrado { .. }) => de_vuelta(TRASLADOS, "ya_cerrado"),
        Ok(R::NoExiste) => no_existe_traslado(e, u).await,
        Ok(R::LlegadaAntesDeSalida { .. }) => averia(),
        Err(_) => averia(),
    }
}

// ── Alta y ficha de una ambulancia ───────────────────────────────────────────

/// Lo que llega del formulario de la ambulancia. Todo texto: es lo que manda un
/// `<form>`, y los vacíos se convierten en AUSENCIA y no en cadena vacía.
#[derive(Deserialize, Default)]
pub struct AmbulanciaEnviada {
    #[serde(default)]
    placa: String,
    #[serde(default)]
    tipo: String,
    #[serde(default)]
    numero_interno: String,
    #[serde(default)]
    marca: String,
    #[serde(default)]
    modelo: String,
    #[serde(default)]
    vence_soat: String,
    #[serde(default)]
    vence_tecnomecanica: String,
    #[serde(default)]
    observaciones: String,
}

fn opcional(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.trim().to_string())
}

impl AmbulanciaEnviada {
    fn nueva(&self) -> ambulancia::AmbulanciaNueva {
        ambulancia::AmbulanciaNueva {
            placa: self.placa.trim().to_string(),
            tipo: opcional(&self.tipo),
            numero_interno: opcional(&self.numero_interno),
            marca: opcional(&self.marca),
            modelo: opcional(&self.modelo),
            vence_soat: opcional(&self.vence_soat),
            vence_tecnomecanica: opcional(&self.vence_tecnomecanica),
            observaciones: opcional(&self.observaciones),
        }
    }

    /// Los cambios de una edición. **`estado` y `activa` NO viajan aquí**: el
    /// estado lo pone el despacho —fijarlo a mano haría que dejara de decir
    /// dónde está el vehículo, y el dominio lo rechaza— y la baja tiene su
    /// propio botón, con su propio significado.
    fn edicion(&self) -> ambulancia::AmbulanciaEdicion {
        ambulancia::AmbulanciaEdicion {
            placa: opcional(&self.placa),
            tipo: opcional(&self.tipo),
            numero_interno: opcional(&self.numero_interno),
            marca: opcional(&self.marca),
            modelo: opcional(&self.modelo),
            estado: None,
            vence_soat: opcional(&self.vence_soat),
            vence_tecnomecanica: opcional(&self.vence_tecnomecanica),
            observaciones: opcional(&self.observaciones),
            activa: None,
        }
    }
}

/// El formulario, tanto para el alta como para la ficha. `valores` es lo que hay
/// que pintar dentro: la ambulancia guardada, o lo que la persona había escrito
/// cuando el guardado falló — que es lo que evita teclearlo todo otra vez.
fn campos(accion: String, valores: &AmbulanciaEnviada, etiqueta: &str) -> Markup {
    html! {
        form method="post" action=(accion) class="apilado" {
            label for="placa" { "Placa" }
            input #placa type="text" name="placa" required value=(valores.placa)
                  placeholder="WXY123";

            label for="tipo" { "Tipo" }
            select #tipo name="tipo" {
                @for t in ambulancia::TIPOS {
                    option value=(t) selected[valores.tipo == *t] {
                        (t) " — " (nombre_tipo(t))
                    }
                }
            }

            label for="numero_interno" { "Número interno" }
            input #numero_interno type="text" name="numero_interno" value=(valores.numero_interno);

            label for="marca" { "Marca" }
            input #marca type="text" name="marca" value=(valores.marca);

            label for="modelo" { "Modelo" }
            input #modelo type="text" name="modelo" value=(valores.modelo);

            label for="vence_soat" { "Vence el SOAT" }
            input #vence_soat type="date" name="vence_soat" value=(valores.vence_soat);

            label for="vence_tecnomecanica" { "Vence la tecnomecánica" }
            input #vence_tecnomecanica type="date" name="vence_tecnomecanica"
                  value=(valores.vence_tecnomecanica);

            label for="observaciones" { "Observaciones" }
            textarea #observaciones name="observaciones" rows="2" { (valores.observaciones) }

            button type="submit" class="principal" { (etiqueta) }
        }
    }
}

/// `GET /panel/ambulancias/nueva`
pub async fn nueva(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let vacia = AmbulanciaEnviada {
        // El defecto es TAB, como en la pantalla anterior: es la modalidad
        // corriente, y el `<select>` obliga a elegir una de las dos igual.
        tipo: "TAB".into(),
        ..Default::default()
    };
    pantalla(&cx, "Nueva ambulancia", formulario_alta(&vacia, None))
}

fn formulario_alta(v: &AmbulanciaEnviada, error: Option<&str>) -> Markup {
    html! {
        (piezas::migas(RUTA, "Ambulancias"))
        header class="titulo-seccion" { h1 { "Nueva ambulancia" } }
        section class="tarjeta" {
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            (campos(format!("{RUTA}/nueva"), v, "Registrar"))
        }
    }
}

/// `POST /panel/ambulancias/nueva`
pub async fn crear(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
    Form(f): Form<AmbulanciaEnviada>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(institucion_id) = u.institucion_id else {
        // Es el mismo final que da `POST /api/ambulancias`: el parque es de una
        // institución. Al superadmin no se le inventa una.
        return pantalla(
            &cx,
            "Nueva ambulancia",
            formulario_alta(
                &f,
                Some(
                    "El parque de ambulancias es de una institución y esta cuenta no está \
                     adscrita a ninguna.",
                ),
            ),
        );
    };
    match ambulancia::crear(
        &e.pool,
        u.ambito(),
        institucion_id,
        &f.nueva(),
        &medico_datos::hoy(),
    )
    .await
    {
        Ok(ambulancia::ResultadoAlta::Hecho(_)) => de_vuelta(RUTA, "creada"),
        Ok(otro) => pantalla(&cx, "Nueva ambulancia", formulario_alta(&f, Some(&motivo_alta(otro)))),
        Err(_) => averia(),
    }
}

/// Los finales del alta que no son «hecho», con el MISMO texto que responde la
/// ruta JSON: dos frases distintas para el mismo hecho serían dos productos.
fn motivo_alta(r: ambulancia::ResultadoAlta) -> String {
    use ambulancia::ResultadoAlta as R;
    match r {
        R::Hecho(_) => String::new(),
        R::NoExiste => "Esa ambulancia ya no existe.".into(),
        R::SinPlaca => "La ambulancia necesita placa: es lo que la identifica ante el tránsito \
                        y ante el asegurador."
            .into(),
        R::TipoInvalido => format!(
            "El tipo tiene que ser {} — las dos modalidades del transporte asistencial.",
            ambulancia::TIPOS.join(" o ")
        ),
        R::EstadoInvalido => "«En Traslado» lo pone el despacho y lo quita el cierre del \
                              traslado, no la edición."
            .into(),
        R::PlacaRepetida(p) => {
            format!("Ya hay una ambulancia con placa {p} en esta institución.")
        }
    }
}

/// `GET /panel/ambulancias/{id}`
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
    let a = match ambulancia::obtener(&e.pool, u.ambito(), id, &medico_datos::hoy()).await {
        Ok(Some(a)) => a,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    pantalla(&cx, "Ambulancia", cuerpo_ficha(&cx, &a, &desde(&a), None, f.aviso.trim()))
}

/// La ambulancia guardada, con forma de formulario.
fn desde(a: &Ambulancia) -> AmbulanciaEnviada {
    AmbulanciaEnviada {
        placa: a.placa.clone(),
        tipo: a.tipo.clone(),
        numero_interno: a.numero_interno.clone().unwrap_or_default(),
        marca: a.marca.clone().unwrap_or_default(),
        modelo: a.modelo.clone().unwrap_or_default(),
        vence_soat: a.vence_soat.clone().unwrap_or_default(),
        vence_tecnomecanica: a.vence_tecnomecanica.clone().unwrap_or_default(),
        observaciones: a.observaciones.clone().unwrap_or_default(),
    }
}

fn cuerpo_ficha(
    cx: &Contexto,
    a: &Ambulancia,
    valores: &AmbulanciaEnviada,
    error: Option<&str>,
    clave_aviso: &str,
) -> Markup {
    let puede = cx.puede(Capacidad::GestionarAmbulancias);
    html! {
        (piezas::migas(RUTA, "Ambulancias"))
        @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
            (aviso(tono, texto))
        }
        header class="titulo-seccion" {
            h1 { (a.placa) }
            @if a.activa { (piezas::estado(&a.estado)) } @else { (piezas::estado("DADA DE BAJA")) }
        }

        section class="tarjeta" {
            (piezas::datos(&[
                ("Tipo", Some(format!("{} — {}", a.tipo, nombre_tipo(&a.tipo)))),
                ("Número interno", a.numero_interno.clone()),
                ("Vehículo", opcional(&format!(
                    "{} {}",
                    a.marca.clone().unwrap_or_default(),
                    a.modelo.clone().unwrap_or_default()
                ))),
                ("Vence la tecnomecánica", a.vence_tecnomecanica.clone()),
                ("Observaciones", a.observaciones.clone()),
            ]))
            p { "SOAT: " (soat(a)) }
            @if a.tecnomecanica_vencida {
                (aviso(Tono::Error, "La tecnomecánica está vencida."))
            }
            // Por qué NO puede salir, dicho con el dato que hace falta para
            // resolverlo. Es la misma respuesta que daría el despacho.
            @match ambulancia::motivo_para_no_despachar(a) {
                Some(m) => (aviso(Tono::Averia, &frase_no_sale(&m))),
                None => (aviso(Tono::Ok, "Puede despacharse ahora mismo.")),
            }
        }

        @if puede {
            section class="tarjeta" {
                h2 { "Editar" }
                @if let Some(t) = error { (aviso(Tono::Error, t)) }
                (campos(format!("{RUTA}/{}", a.id), valores, "Guardar"))
            }
            @if a.traslado_en_curso.is_none() {
                section class="tarjeta" {
                    h2 { @if a.activa { "Dar de baja" } @else { "Reactivar" } }
                    p class="sutil" {
                        @if a.activa {
                            "Dejará de poder despacharse y seguirá explicando los traslados \
                             que ya hizo. Se puede reactivar."
                        } @else {
                            "Volverá a estar disponible para el despacho."
                        }
                    }
                    form method="post" action={ (RUTA) "/" (a.id) "/baja" } {
                        button type="submit" {
                            @if a.activa { "Dar de baja" } @else { "Reactivar" }
                        }
                    }
                }
            }
        }
    }
}

/// `POST /panel/ambulancias/{id}`
pub async fn guardar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
    Path(id): Path<i64>,
    Form(f): Form<AmbulanciaEnviada>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match ambulancia::actualizar(&e.pool, u.ambito(), id, &f.edicion(), &medico_datos::hoy()).await {
        Ok(ambulancia::ResultadoAlta::Hecho(_)) => {
            de_vuelta(&format!("{RUTA}/{id}"), "guardada")
        }
        Ok(ambulancia::ResultadoAlta::NoExiste) => no_existe(&cx),
        Ok(otro) => {
            // Se vuelve a dibujar la ficha con lo que la persona escribió. Para
            // eso hace falta la ambulancia GUARDADA, que es lo que da el resto
            // de la pantalla: si no se puede leer, es avería.
            let motivo = motivo_alta(otro);
            match ambulancia::obtener(&e.pool, u.ambito(), id, &medico_datos::hoy()).await {
                Ok(Some(a)) => pantalla(
                    &cx,
                    "Ambulancia",
                    cuerpo_ficha(&cx, &a, &f, Some(&motivo), ""),
                ),
                Ok(None) => no_existe(&cx),
                Err(_) => averia(),
            }
        }
        Err(_) => averia(),
    }
}

/// `POST /panel/ambulancias/{id}/baja` — alterna la baja.
///
/// **Alterna en vez de recibir el valor**, que es lo que hacía la pantalla
/// anterior: un campo `activa` en el formulario dejaría que una petición vieja
/// —el botón «atrás» y otra vez enviar— reactivara lo que alguien acaba de dar
/// de baja. El estado de partida se lee aquí.
pub async fn baja(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let hoy = medico_datos::hoy();
    let a = match ambulancia::obtener(&e.pool, u.ambito(), id, &hoy).await {
        Ok(Some(a)) => a,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let cambios = ambulancia::AmbulanciaEdicion {
        activa: Some(!a.activa),
        ..Default::default()
    };
    match ambulancia::actualizar(&e.pool, u.ambito(), id, &cambios, &hoy).await {
        Ok(ambulancia::ResultadoAlta::Hecho(_)) => de_vuelta(
            &format!("{RUTA}/{id}"),
            if a.activa { "de_baja" } else { "reactivada" },
        ),
        Ok(ambulancia::ResultadoAlta::NoExiste) => no_existe(&cx),
        Ok(_) | Err(_) => averia(),
    }
}

// ── El despacho ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct DespachoEnviado {
    /// Lo que se había tecleado en el buscador de paciente. Viaja con el
    /// formulario para que un error de validación no borre la búsqueda: sin
    /// esto, quien se equivoca en el origen vuelve a una lista vacía y tiene
    /// que encontrar a su paciente otra vez.
    #[serde(default)]
    q_paciente: String,
    #[serde(default)]
    ambulancia_id: String,
    #[serde(default)]
    paciente_id: String,
    #[serde(default)]
    tipo_traslado: String,
    #[serde(default)]
    origen: String,
    #[serde(default)]
    destino: String,
    #[serde(default)]
    motivo: String,
    #[serde(default)]
    conductor: String,
    #[serde(default)]
    personal_asistencial: String,
    #[serde(default)]
    observaciones: String,
}

/// `GET /panel/ambulancias/despachar`
pub async fn despacho(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_despacho(&e, &cx, &u, &DespachoEnviado::default(), None).await
}

/// `POST /panel/ambulancias/despachar`
pub async fn despachar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarAmbulancias>,
    Form(f): Form<DespachoEnviado>,
) -> Response {
    use ambulancia::ResultadoDespacho as R;
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (Ok(ambulancia_id), Ok(paciente_id)) =
        (f.ambulancia_id.parse::<i64>(), f.paciente_id.parse::<i64>())
    else {
        return dibujar_despacho(
            &e,
            &cx,
            &u,
            &f,
            Some("Elija una ambulancia y un paciente."),
        )
        .await;
    };
    let t = ambulancia::TrasladoNuevo {
        ambulancia_id,
        paciente_id,
        remision_id: None,
        tipo_traslado: opcional(&f.tipo_traslado),
        origen: f.origen.trim().to_string(),
        destino: f.destino.trim().to_string(),
        motivo: f.motivo.trim().to_string(),
        conductor: opcional(&f.conductor),
        personal_asistencial: opcional(&f.personal_asistencial),
        observaciones: opcional(&f.observaciones),
    };
    let r = match ambulancia::despachar(&e.pool, u.ambito(), u.id, &t, &medico_datos::hoy()).await {
        Ok(r) => r,
        Err(_) => return averia(),
    };
    // Cada negativa lleva el dato con el que se resuelve, con el MISMO texto que
    // responde la ruta JSON: quien coordina una urgencia no puede hacer nada con
    // un «no se pudo».
    let motivo = match r {
        R::Hecho(_) => return de_vuelta(TRASLADOS, "despachada"),
        R::NoExiste => "Esa ambulancia o ese paciente no existen en esta institución.".to_string(),
        R::Ocupada { traslado_id } => {
            format!("Esa ambulancia está en el traslado #{traslado_id}. Ciérrelo o despache otra.")
        }
        R::NoDisponible { estado } => {
            format!("Esa ambulancia está «{estado}» y no se puede despachar.")
        }
        R::SoatVencido { vencio } => format!(
            "El SOAT de esa ambulancia venció el {vencio}: no puede circular. Despache otra \
             mientras se renueva."
        ),
        R::TipoInvalido => format!(
            "El traslado es {} — primario desde el sitio del evento, secundario entre \
             prestadores.",
            ambulancia::TIPOS_TRASLADO.join(" o ")
        ),
    };
    dibujar_despacho(&e, &cx, &u, &f, Some(&motivo)).await
}

fn frase_no_sale(m: &NoSale) -> String {
    match m {
        NoSale::Ocupada { traslado_id } => format!("está en el traslado #{traslado_id}"),
        NoSale::NoDisponible { estado } => format!("está «{estado}»"),
        NoSale::SoatVencido { vencio } => format!("el SOAT venció el {vencio}"),
    }
}

async fn dibujar_despacho(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    v: &DespachoEnviado,
    error: Option<&str>,
) -> Response {
    let hoy = medico_datos::hoy();
    let parque = match ambulancia::listar(&e.pool, u.ambito(), &Default::default(), &hoy).await {
        Ok(a) => a,
        Err(_) => return averia(),
    };
    // El buscador, no el padrón entero: `encontrar` acota en SQL y trae además
    // cuántos casan, que es lo que deja decir «coinciden 84, se enseñan 20».
    let (pacientes, total_pacientes) =
        match crate::vista::buscador_paciente::encontrar(e, u, &v.q_paciente).await {
            Ok(x) => x,
            Err(_) => return averia(),
        };
    // La regla la decide el dominio, no esta pantalla: `motivo_para_no_despachar`
    // es la MISMA que aplica `despachar`.
    let (salen, no_salen): (Vec<_>, Vec<_>) = parque
        .iter()
        .map(|a| (a, ambulancia::motivo_para_no_despachar(a)))
        .partition(|(_, m)| m.is_none());

    pantalla(
        cx,
        "Despachar",
        html! {
            (piezas::migas(RUTA, "Ambulancias"))
            header class="titulo-seccion" { h1 { "Despachar una ambulancia" } }
            section class="tarjeta" {
                @if let Some(t) = error { (aviso(Tono::Error, t)) }
                @if salen.is_empty() {
                    (aviso(
                        Tono::Error,
                        "Ninguna ambulancia puede salir ahora mismo. Abajo está el motivo de \
                         cada una.",
                    ))
                } @else if pacientes.is_empty() && v.q_paciente.trim().is_empty() {
                    (aviso(
                        Tono::Error,
                        "No hay pacientes registrados: un traslado se registra a nombre de \
                         alguien.",
                    ))
                } @else {
                    form method="post" action={ (RUTA) "/despachar" } class="apilado" {
                        label for="ambulancia_id" { "Ambulancia" }
                        select #ambulancia_id name="ambulancia_id" required {
                            option value="" { "Elija una" }
                            @for (a, _) in &salen {
                                option value=(a.id) selected[v.ambulancia_id == a.id.to_string()] {
                                    (a.placa) " — " (a.tipo)
                                    @if a.soat_sin_registrar { " (SOAT sin registrar)" }
                                }
                            }
                        }

                        (crate::vista::buscador_paciente::campo(
                            &v.q_paciente, &v.paciente_id, &pacientes, total_pacientes,
                        ))

                        label for="tipo_traslado" { "Tipo de traslado" }
                        select #tipo_traslado name="tipo_traslado" {
                            @for t in ambulancia::TIPOS_TRASLADO {
                                option value=(t) selected[v.tipo_traslado == *t] {
                                    (t) " — " (explicacion_traslado(t))
                                }
                            }
                        }

                        label for="origen" { "Origen" }
                        input #origen type="text" name="origen" required value=(v.origen)
                              placeholder="Vía Las Palmas km 12";

                        label for="destino" { "Destino" }
                        input #destino type="text" name="destino" required value=(v.destino)
                              placeholder="Urgencias sede norte";

                        label for="motivo" { "Motivo" }
                        input #motivo type="text" name="motivo" required value=(v.motivo)
                              placeholder="Accidente de tránsito";

                        label for="conductor" { "Conductor" }
                        input #conductor type="text" name="conductor" value=(v.conductor);

                        label for="personal" { "Personal asistencial" }
                        input #personal type="text" name="personal_asistencial"
                              value=(v.personal_asistencial);

                        label for="observaciones" { "Observaciones" }
                        textarea #observaciones name="observaciones" rows="2" {
                            (v.observaciones)
                        }

                        button type="submit" class="principal" { "Despachar" }
                    }
                }
            }

            @if !no_salen.is_empty() {
                section class="tarjeta" {
                    h2 { "Las que no pueden salir" }
                    p class="sutil" {
                        "Se dicen en vez de esconderlas: una ambulancia que desaparece de la \
                         lista sin explicación se lee como un fallo del sistema."
                    }
                    (piezas::tabla(
                        &["Placa", "Por qué"],
                        html! {
                            @for (a, m) in &no_salen {
                                tr {
                                    td { a href={ (RUTA) "/" (a.id) } { (a.placa) } }
                                    td { (m.as_ref().map(frase_no_sale).unwrap_or_default()) }
                                }
                            }
                        },
                    ))
                }
            }
        },
    )
}

fn explicacion_traslado(t: &str) -> &'static str {
    match t {
        "Primario" => "desde el sitio del evento",
        "Secundario" => "entre prestadores",
        _ => "",
    }
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
                    h1 { "No se encontró esa ambulancia" }
                    p { "Puede que el enlace esté mal o que ya no exista." }
                    p { a href=(RUTA) { "Volver al parque" } }
                }
            },
        ),
    )
        .into_response()
}

async fn no_existe_traslado(e: &Estado, u: &medico_datos::usuario::Usuario) -> Response {
    let cx = match contexto(e, u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrado",
            Some(crate::vista::panel::menu(&cx, TRASLADOS)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró ese traslado" }
                    p { a href=(TRASLADOS) { "Volver a Traslados" } }
                }
            },
        ),
    )
        .into_response()
}
