// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Facturación: facturas, sus líneas, las glosas y los contratos.
//!
//! Port de `frontend/src/app/pages/facturacion/facturacion.component.ts` (1 531
//! líneas). Es la pantalla más grande del producto y la última de las grandes
//! que quedaba.
//!
//! # Las transiciones las decide el DOMINIO, y aquí se pregunta
//!
//! `EstadoFactura::siguientes` dice a dónde puede ir una factura desde donde
//! está, y esta pantalla dibuja **solo esos** destinos. El componente de Angular
//! decidía con su propia lectura —pintaba «Anular» para cualquier factura que no
//! estuviera pagada ni anulada— y el servidor volvía a decidirlo al recibir: la
//! regla enseñada dos veces, con la copia libre de quedarse atrás. Ese `Anulada`
//! que la pantalla pintaba, de hecho, **faltaba en el `CHECK` de la migración
//! 0044** hasta que la 0045 lo añadió.
//!
//! # Lo que no se puede editar no se ofrece
//!
//! Solo un BORRADOR admite edición, líneas nuevas y radicación. Una factura
//! radicada ante una EPS ya existe fuera de este sistema: por eso anular no es
//! borrar, y por eso una pagada es terminal.
//!
//! # Los importes se pintan, no se recalculan
//!
//! `valor_bruto`, `valor_neto` y el total de cada línea los calcula
//! `medico_datos::facturacion` en la misma transacción que los escribe.
//! Recalcularlos aquí daría el mismo número hoy y otro el día que cambie la
//! definición de un descuento, y entonces habría dos totales con el mismo
//! nombre (directiva 32).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::facturacion;
use medico_dominio::facturacion::EstadoFactura;
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{Estado, GestionarFacturacion};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/facturacion";
const GLOSAS: &str = "/panel/facturacion/glosas";
const CONTRATOS: &str = "/panel/facturacion/contratos";

/// Los seis estados, en el orden en que ocurren. Sale del dominio para que el
/// filtro no se quede corto cuando entre uno nuevo.
const ESTADOS: [EstadoFactura; 6] = [
    EstadoFactura::Borrador,
    EstadoFactura::Radicada,
    EstadoFactura::Aceptada,
    EstadoFactura::Glosada,
    EstadoFactura::Pagada,
    EstadoFactura::Anulada,
];

/// Los cuatro tipos de tarifa que el dominio admite. Escribir otro lo rechaza
/// con `ReferenciaInvalida`; ofrecer una lista cerrada evita el viaje.
const TARIFAS: [&str; 4] = ["ISS", "SOAT", "CUPS", "PERSONALIZADA"];

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "creada" => Some((Tono::Ok, "Factura creada en borrador.")),
        "guardada" => Some((Tono::Ok, "Cambios guardados.")),
        "linea" => Some((Tono::Ok, "Línea añadida.")),
        "linea_borrada" => Some((Tono::Ok, "Línea eliminada.")),
        "radicada" => Some((
            Tono::Ok,
            "Factura radicada. Desde aquí ya no se edita: existe fuera de este sistema.",
        )),
        "estado" => Some((Tono::Ok, "Estado actualizado.")),
        "glosa" => Some((Tono::Ok, "Glosa registrada.")),
        "glosa_respondida" => Some((Tono::Ok, "Glosa respondida.")),
        "contrato" => Some((Tono::Ok, "Contrato registrado.")),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

fn texto(c: &HashMap<String, String>, k: &str) -> Option<String> {
    c.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn entero(c: &HashMap<String, String>, k: &str) -> Option<i64> {
    c.get(k).and_then(|v| v.trim().parse::<i64>().ok())
}

fn decimal(c: &HashMap<String, String>, k: &str) -> Option<f64> {
    c.get(k).and_then(|v| v.trim().parse::<f64>().ok())
}

/// Pesos con separador de miles, el mismo criterio que en estadísticas.
fn pesos(v: f64) -> String {
    let ent = v.round().abs() as i64;
    let d = ent.to_string();
    let mut s = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            s.push('.');
        }
        s.push(c);
    }
    if v < 0.0 {
        format!("-$ {s}")
    } else {
        format!("$ {s}")
    }
}

/// El motivo de un rechazo del dominio, con **el mismo texto que responde la
/// ruta JSON**: dos frases distintas para el mismo hecho son dos productos.
fn motivo(r: &facturacion::Resultado) -> String {
    use facturacion::Resultado as R;
    match r {
        R::Hecho => String::new(),
        R::NoExiste => "No se encontró eso en esta institución.".into(),
        R::EstadoNoAdmite { estado } => format!(
            "Una factura en estado {estado} no admite esta operación. Solo se editan, borran y \
             radican las que están en BORRADOR."
        ),
        R::TransicionInvalida { desde, hacia, permitidas } => {
            if permitidas.is_empty() {
                format!("{desde} es un estado final: no se puede pasar a {hacia}.")
            } else {
                format!(
                    "Transición no permitida: {desde} → {hacia}. Desde {desde} solo se puede ir \
                     a {}.",
                    permitidas.join(", ")
                )
            }
        }
        R::SinValor => {
            "No se puede radicar una factura sin valor: añádale líneas primero.".into()
        }
        R::ReferenciaInvalida { que } => match *que {
            "contrato" => "Ese contrato no existe o no es de esta institución.".into(),
            "tipo_tarifa" => "El tipo de tarifa tiene que ser ISS, SOAT, CUPS o PERSONALIZADA.".into(),
            otro => format!("No existe la {otro} que indica."),
        },
        R::NumeroRepetido { numero } => {
            format!("Ya hay un contrato con el número {numero} en esta institución.")
        }
        R::FaltaTexto { campo } => format!("Falta «{campo}», y sin él la fila no dice nada."),
        R::FueraDelRango { siguiente, hasta } => format!(
            "La factura número {siguiente} se saldría del rango que autoriza su resolución de la \
             DIAN (hasta {hasta}). Emitirla la invalidaría: amplíe la resolución o cierre la \
             numeración."
        ),
        R::ResolucionIncompleta => "La resolución de facturación tiene rango pero le falta el \
             número: complétela en la configuración antes de facturar."
            .into(),
    }
}

// ── El armazón ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub q: String,
}

fn cabecera(cx: &Contexto, donde: &str) -> Markup {
    html! {
        header class="titulo-seccion" {
            h1 { "Facturación" }
            nav class="buscador" {
                a class=[(donde == "facturas").then_some("enlace-activo")] href=(RUTA) {
                    "Facturas"
                }
                a class=[(donde == "glosas").then_some("enlace-activo")] href=(GLOSAS) { "Glosas" }
                a class=[(donde == "contratos").then_some("enlace-activo")] href=(CONTRATOS) {
                    "Contratos"
                }
            }
        }
        @if !cx.puede(Capacidad::GestionarFacturacion) {
            p class="sutil" {
                "Su cuenta puede consultar la facturación, no escribirla. Quien responde en "
                "ventanilla por el estado de una factura no tiene por qué poder radicarla."
            }
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
            "No encontrada",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró esa factura" }
                    p { a href=(RUTA) { "Volver a Facturación" } }
                }
            },
        ),
    )
        .into_response()
}

// ── Facturas ─────────────────────────────────────────────────────────────────

/// `GET /panel/facturacion`
pub async fn facturas(
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
    // lista vacía y «no hay facturas así» se leería como un hecho.
    if !estado.is_empty() && EstadoFactura::desde_texto(estado).is_none() {
        return pantalla(
            &cx,
            "Facturación",
            html! {
                (cabecera(&cx, "facturas"))
                section class="tarjeta" {
                    (aviso(Tono::Error, &format!(
                        "«{estado}» no es un estado de factura. Los que hay son: {}.",
                        ESTADOS.map(|x| x.como_texto()).join(", ")
                    )))
                }
            },
        );
    }
    dibujar_facturas(&e, &cx, &u, estado, f.q.trim(), f.aviso.trim(), None).await
}

#[allow(clippy::too_many_arguments)]
async fn dibujar_facturas(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    estado: &str,
    q: &str,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let filtros = facturacion::FiltrosFactura {
        estado: (!estado.is_empty()).then(|| estado.to_string()),
        q: (!q.is_empty()).then(|| q.to_string()),
        page: 1,
        size: 100,
        ..Default::default()
    };
    let (lista, stats, entidades, contratos) = match (
        facturacion::listar_facturas(&e.pool, u.ambito(), &filtros).await,
        facturacion::stats(&e.pool, u.ambito()).await,
        facturacion::listar_entidades(&e.pool).await,
        facturacion::listar_contratos(&e.pool, u.ambito(), Some(true)).await,
    ) {
        (Ok(l), Ok(s), Ok(en), Ok(co)) => (l, s, en, co),
        _ => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFacturacion);

    pantalla(
        cx,
        "Facturación",
        html! {
            (cabecera(cx, "facturas"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }

            // Las cifras son de TODAS las facturas, no de lo filtrado.
            section class="tarjeta resumen" {
                div { span class="cifra" { (stats.total_facturas) } span class="sutil" { "facturas" } }
                div { span class="cifra" { (stats.borradores) } span class="sutil" { "en borrador" } }
                div { span class="cifra" { (stats.radicadas) } span class="sutil" { "radicadas" } }
                div { span class="cifra" { (stats.glosadas) } span class="sutil" { "glosadas" } }
                div {
                    span class="cifra" { (pesos(stats.valor_total_radicado)) }
                    span class="sutil" { "radicado" }
                }
                div {
                    span class="cifra" { (pesos(stats.valor_total_pagado)) }
                    span class="sutil" { "pagado" }
                }
            }
            @if stats.glosas_pendientes > 0 {
                (aviso(Tono::Averia, &format!(
                    "{} glosa(s) sin responder por {}. Una glosa sin respuesta a tiempo se da \
                     por aceptada.",
                    stats.glosas_pendientes,
                    pesos(stats.valor_glosado_pendiente)
                )))
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="buscador" {
                    input type="search" name="q" value=(q) placeholder="Número o entidad";
                    @if !estado.is_empty() {
                        input type="hidden" name="estado" value=(estado);
                    }
                    button type="submit" { "Buscar" }
                }
                nav class="buscador" {
                    a class=[estado.is_empty().then_some("enlace-activo")] href=(RUTA) { "Todas" }
                    @for s in ESTADOS {
                        a class=[(estado == s.como_texto()).then_some("enlace-activo")]
                          href={ (RUTA) "?estado=" (s.como_texto()) } { (s.como_texto()) }
                    }
                }
            }

            @if lista.is_empty() {
                (piezas::vacio(
                    !estado.is_empty() || !q.is_empty(),
                    "No hay facturas registradas.",
                    "Ninguna factura coincide con ese filtro.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(lista.len(), stats.total_facturas as usize, "factura", "facturas"))
                (piezas::tabla(
                    &["Número", "Entidad", "Periodo", "Líneas", "Valor neto", "Estado", "Glosas"],
                    html! {
                        @for f in &lista {
                            tr {
                                td { a href={ (RUTA) "/" (f.id) } { (f.numero_factura) } }
                                td { (f.entidad_nombre) }
                                td class="mono sutil" { (f.periodo_inicio) " – " (f.periodo_fin) }
                                td class="mono" { (f.total_detalles) }
                                td class="mono" { (pesos(f.valor_neto)) }
                                td { (piezas::estado(&f.estado)) }
                                td {
                                    @if f.glosas_pendientes > 0 {
                                        span class="pastilla pastilla-mal" {
                                            (f.glosas_pendientes) " sin responder"
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
                    h2 { "Nueva factura" }
                    @if let Some(t) = error { (aviso(Tono::Error, t)) }
                    @if entidades.is_empty() {
                        p class="sutil" {
                            "No hay entidades registradas: una factura se emite a nombre de un "
                            "pagador."
                        }
                    } @else {
                        form method="post" action=(RUTA) class="apilado" {
                            label for="entidad" { "Entidad" }
                            select #entidad name="entidad_id" required {
                                option value="" { "Elija una" }
                                @for en in &entidades {
                                    option value=(en.id) { (en.nombre) " — NIT " (en.nit) }
                                }
                            }
                            label for="contrato" { "Contrato" }
                            select #contrato name="contrato_id" {
                                option value="" { "Sin contrato" }
                                @for c in &contratos {
                                    option value=(c.id) { (c.nombre) " — " (c.numero_contrato) }
                                }
                            }
                            label for="ini" { "Periodo desde" }
                            input #ini type="date" name="periodo_inicio" required;
                            label for="fin" { "Periodo hasta" }
                            input #fin type="date" name="periodo_fin" required;
                            label for="desc" { "Descuentos" }
                            input #desc type="number" name="descuentos" step="0.01" min="0" value="0";
                            label for="obs" { "Observaciones" }
                            input #obs type="text" name="observaciones";
                            p class="sutil" {
                                "Nace en BORRADOR y sin líneas. Se le añaden dentro y se radica "
                                "cuando tenga valor."
                            }
                            button type="submit" class="principal" { "Crear la factura" }
                        }
                    }
                }
            }
        },
    )
}

/// `POST /panel/facturacion`
pub async fn crear(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(institucion_id) = u.institucion_id else {
        return dibujar_facturas(
            &e,
            &cx,
            &u,
            "",
            "",
            "",
            Some("La facturación es de una institución y esta cuenta no está adscrita a ninguna."),
        )
        .await;
    };
    let Some(entidad_id) = entero(&c, "entidad_id") else {
        return dibujar_facturas(&e, &cx, &u, "", "", "", Some("Elija la entidad pagadora.")).await;
    };
    let nueva = facturacion::FacturaNueva {
        entidad_id,
        contrato_id: entero(&c, "contrato_id"),
        periodo_inicio: c.get("periodo_inicio").cloned().unwrap_or_default(),
        periodo_fin: c.get("periodo_fin").cloned().unwrap_or_default(),
        descuentos: decimal(&c, "descuentos").unwrap_or(0.0),
        observaciones: texto(&c, "observaciones"),
        detalles: Vec::new(),
    };
    match facturacion::crear_factura(&e.pool, u.ambito(), institucion_id, &nueva).await {
        Ok(facturacion::ResultadoAlta::Creado(id)) => {
            de_vuelta(&format!("{RUTA}/{id}"), "creada")
        }
        Ok(facturacion::ResultadoAlta::Fallo(r)) => {
            let m = motivo(&r);
            dibujar_facturas(&e, &cx, &u, "", "", "", Some(&m)).await
        }
        Err(_) => averia(),
    }
}

// ── La ficha ─────────────────────────────────────────────────────────────────

/// `GET /panel/facturacion/{id}`
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
    dibujar_ficha(&e, &cx, &u, id, f.aviso.trim(), None).await
}

async fn dibujar_ficha(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    id: i64,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let f = match facturacion::obtener_factura(&e.pool, u.ambito(), id).await {
        Ok(Some(f)) => f,
        Ok(None) => return no_existe(cx),
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFacturacion);
    let estado = EstadoFactura::desde_texto(&f.cabecera.estado);
    // **A dónde puede ir lo dice el DOMINIO.** El componente de Angular pintaba
    // «Anular» para cualquier factura que no estuviera pagada ni anulada, y ese
    // estado ni siquiera existía en el `CHECK` de la migración hasta la 0045.
    let destinos: &[EstadoFactura] = estado.map(|s| s.siguientes()).unwrap_or(&[]);
    let editable = estado.is_some_and(|s| s.admite_edicion());

    pantalla(
        cx,
        "Factura",
        html! {
            (piezas::migas(RUTA, "Facturación"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            header class="titulo-seccion" {
                h1 { (f.cabecera.numero_factura) }
                (piezas::estado(&f.cabecera.estado))
            }
            @if estado.is_none() {
                (aviso(
                    Tono::Averia,
                    "Esta factura tiene un estado que el sistema no reconoce, así que no se \
                     puede mover ni editar. Es un dato de la base que hay que corregir.",
                ))
            }

            section class="tarjeta" {
                div class="resumen" {
                    div {
                        span class="cifra" { (pesos(f.cabecera.valor_bruto)) }
                        span class="sutil" { "bruto" }
                    }
                    div {
                        span class="cifra" { (pesos(f.cabecera.descuentos)) }
                        span class="sutil" { "descuentos" }
                    }
                    div {
                        span class="cifra" { (pesos(f.cabecera.valor_neto)) }
                        span class="sutil" { "neto" }
                    }
                }
                (piezas::datos(&[
                    ("Entidad", Some(f.cabecera.entidad_nombre.clone())),
                    ("Contrato", f.cabecera.contrato_nombre.clone()),
                    ("Fecha de la factura", Some(f.cabecera.fecha_factura.clone())),
                    ("Periodo", Some(format!("{} – {}", f.cabecera.periodo_inicio, f.cabecera.periodo_fin))),
                    ("Radicada el", f.cabecera.fecha_radicado.clone()),
                    ("Número de radicado", f.cabecera.num_radicado.clone()),
                    ("Observaciones", f.cabecera.observaciones.clone()),
                ]))
            }

            section class="tarjeta" {
                h2 { "Líneas" }
                @if f.detalles.is_empty() {
                    p class="sutil" { "Esta factura no tiene líneas: su valor es cero." }
                } @else {
                    (piezas::tabla(
                        &["Servicio", "CUPS", "Cantidad", "Valor unitario", "Total", "Paciente", ""],
                        html! {
                            @for d in &f.detalles {
                                tr {
                                    td { (d.servicio) }
                                    td class="mono sutil" { (d.codigo_cups.clone().unwrap_or_default()) }
                                    td class="mono" { (d.cantidad) }
                                    td class="mono" { (pesos(d.valor_unitario)) }
                                    td class="mono" { (pesos(d.valor_total)) }
                                    td class="sutil" { (d.paciente_nombre.clone().unwrap_or_default()) }
                                    td {
                                        @if puede && editable {
                                            form method="post"
                                                 action={ (RUTA) "/" (id) "/detalles/" (d.id) "/borrar" } {
                                                button type="submit" class="enlace" { "Quitar" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
                @if puede && editable {
                    form method="post" action={ (RUTA) "/" (id) "/detalles" } class="apilado" {
                        label for="servicio" { "Servicio" }
                        input #servicio type="text" name="servicio" required
                              placeholder="Consulta de medicina general";
                        label for="cups" { "Código CUPS" }
                        input #cups type="text" name="codigo_cups";
                        label for="cant" { "Cantidad" }
                        input #cant type="number" name="cantidad" min="1" value="1";
                        label for="vu" { "Valor unitario" }
                        input #vu type="number" name="valor_unitario" step="0.01" min="0" required;
                        button type="submit" { "Añadir la línea" }
                    }
                } @else if !editable && estado.is_some() {
                    p class="sutil" {
                        "Las líneas solo se tocan en BORRADOR. Una factura radicada ya existe "
                        "ante la entidad: se corrige con una nota, no reescribiéndola."
                    }
                }
            }

            // El formulario de glosa solo se ofrece cuando el dominio la admite:
            // la EPS glosa lo que RECIBIÓ, y un borrador no ha salido de aquí.
            (seccion_glosas(id, &f.glosas, puede && estado.is_some_and(|s| s.admite_glosa())))

            @if puede {
                @if editable {
                    section class="tarjeta" {
                        h2 { "Radicar" }
                        p class="sutil" {
                            "Desde aquí la factura deja de editarse. Sin número de radicado se "
                            "guarda solo la fecha."
                        }
                        form method="post" action={ (RUTA) "/" (id) "/radicar" } class="apilado" {
                            label for="rad" { "Número de radicado" }
                            input #rad type="text" name="num_radicado";
                            button type="submit" class="principal"
                                    data-confirmar="Pulse otra vez para radicarla" {
                                "Radicar"
                            }
                        }
                    }
                }
                @if !destinos.is_empty() {
                    section class="tarjeta" {
                        h2 { "Cambiar el estado" }
                        // SOLO los destinos que el dominio permite desde aquí.
                        form method="post" action={ (RUTA) "/" (id) "/estado" } class="apilado" {
                            select name="destino" required {
                                @for d in destinos {
                                    option value=(d.como_texto()) { (d.como_texto()) }
                                }
                            }
                            button type="submit"
                                    data-confirmar="Pulse otra vez para cambiarlo" {
                                "Cambiar"
                            }
                        }
                    }
                } @else if estado.is_some() {
                    p class="sutil" {
                        "Esta factura está en un estado final: no se mueve de aquí. Una pagada "
                        "no vuelve atrás y una anulada no resucita — se emite otra."
                    }
                }
            }
        },
    )
}

/// `puede_glosar` es «tiene la capacidad **y** el estado la admite»: son dos
/// condiciones y las dos se preguntan fuera, la segunda al dominio.
fn seccion_glosas(id: i64, glosas: &[facturacion::Glosa], puede: bool) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Glosas" }
            @if glosas.is_empty() {
                p class="sutil" { "Esta factura no tiene glosas." }
            } @else {
                (piezas::tabla(
                    &["Fecha", "Ítem glosado", "Motivo", "Valor", "Estado", "Respuesta"],
                    html! {
                        @for g in glosas {
                            tr {
                                td class="mono" { (g.fecha_glosa) }
                                td { (g.item_glosado) }
                                td class="sutil" { (g.motivo) }
                                td class="mono" { (pesos(g.valor_glosado)) }
                                td { (piezas::estado(&g.estado)) }
                                td {
                                    @match &g.respuesta {
                                        Some(r) => (r),
                                        None if puede => (formulario_respuesta(g.id)),
                                        None => span class="falta" { "sin responder" },
                                    }
                                }
                            }
                        }
                    },
                ))
            }
            @if puede {
                form method="post" action={ (RUTA) "/" (id) "/glosas" } class="apilado" {
                    h3 { "Registrar una glosa" }
                    label for="item" { "Ítem glosado" }
                    input #item type="text" name="item_glosado" required;
                    label for="motivo" { "Motivo" }
                    input #motivo type="text" name="motivo" required;
                    label for="valor" { "Valor glosado" }
                    input #valor type="number" name="valor_glosado" step="0.01" min="0";
                    button type="submit" { "Registrar" }
                }
            }
        }
    }
}

/// El formulario de respuesta, en la fila de su glosa.
///
/// «Aceptar» y «responder» son cosas distintas y van en el mismo envío porque el
/// dominio las pide juntas: aceptar la glosa es reconocer el descuento, y
/// rechazarla es sostener el cobro. Sin decir cuál, la respuesta no significa
/// nada.
fn formulario_respuesta(glosa_id: i64) -> Markup {
    html! {
        form method="post" action={ (GLOSAS) "/" (glosa_id) "/responder" } class="apilado" {
            input type="text" name="respuesta" required placeholder="Respuesta";
            label {
                input type="checkbox" name="aceptar" value="1";
                " Aceptar la glosa (reconocer el descuento)"
            }
            button type="submit" { "Responder" }
        }
    }
}

// ── Escrituras sobre la factura ──────────────────────────────────────────────

/// Traduce el resultado del dominio: lo bueno redirige, lo malo vuelve a
/// dibujar la ficha con el motivo dentro.
async fn tras_escribir(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    id: i64,
    r: facturacion::Resultado,
    bien: &str,
) -> Response {
    match r {
        facturacion::Resultado::Hecho => de_vuelta(&format!("{RUTA}/{id}"), bien),
        facturacion::Resultado::NoExiste => no_existe(cx),
        otro => {
            let m = motivo(&otro);
            dibujar_ficha(e, cx, u, id, "", Some(&m)).await
        }
    }
}

/// `POST /panel/facturacion/{id}/detalles`
pub async fn agregar_detalle(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let d = facturacion::DetalleNuevo {
        servicio: c.get("servicio").cloned().unwrap_or_default().trim().to_string(),
        codigo_cups: texto(&c, "codigo_cups"),
        cantidad: entero(&c, "cantidad").unwrap_or(1),
        valor_unitario: decimal(&c, "valor_unitario").unwrap_or(0.0),
        paciente_id: entero(&c, "paciente_id"),
        historia_id: entero(&c, "historia_id"),
    };
    match facturacion::agregar_detalle(&e.pool, u.ambito(), id, &d).await {
        Ok(r) => tras_escribir(&e, &cx, &u, id, r, "linea").await,
        Err(_) => averia(),
    }
}

/// `POST /panel/facturacion/{id}/detalles/{detalle_id}/borrar`
pub async fn borrar_detalle(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Path((id, detalle_id)): Path<(i64, i64)>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    match facturacion::eliminar_detalle(&e.pool, u.ambito(), id, detalle_id).await {
        Ok(r) => tras_escribir(&e, &cx, &u, id, r, "linea_borrada").await,
        Err(_) => averia(),
    }
}

/// `POST /panel/facturacion/{id}/radicar`
pub async fn radicar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let r = facturacion::Radicacion {
        num_radicado: texto(&c, "num_radicado"),
        // Sin fecha: la pone el dominio con la del PRESTADOR, no con el reloj de
        // la máquina. Mandarla desde aquí sería reintroducir el error que ese
        // comentario documenta.
        fecha_radicado: None,
    };
    match facturacion::radicar(&e.pool, u.ambito(), id, &r).await {
        Ok(res) => tras_escribir(&e, &cx, &u, id, res, "radicada").await,
        Err(_) => averia(),
    }
}

/// `POST /panel/facturacion/{id}/estado`
pub async fn cambiar_estado(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    // El destino se lee con el vocabulario del DOMINIO. Uno que no exista no se
    // pasa a la consulta: se dice, en vez de dejarlo llegar como texto libre.
    let Some(destino) = c.get("destino").and_then(|s| EstadoFactura::desde_texto(s.trim())) else {
        return dibujar_ficha(
            &e,
            &cx,
            &u,
            id,
            "",
            Some("Ese no es un estado de factura de este producto."),
        )
        .await;
    };
    match facturacion::cambiar_estado(&e.pool, u.ambito(), id, destino).await {
        Ok(r) => tras_escribir(&e, &cx, &u, id, r, "estado").await,
        Err(_) => averia(),
    }
}

/// `POST /panel/facturacion/{id}/glosas`
pub async fn agregar_glosa(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let g = facturacion::GlosaNueva {
        item_glosado: c.get("item_glosado").cloned().unwrap_or_default().trim().to_string(),
        motivo: c.get("motivo").cloned().unwrap_or_default().trim().to_string(),
        valor_glosado: decimal(&c, "valor_glosado").unwrap_or(0.0),
    };
    match facturacion::agregar_glosa(&e.pool, u.ambito(), id, &g).await {
        Ok(facturacion::ResultadoAlta::Creado(_)) => de_vuelta(&format!("{RUTA}/{id}"), "glosa"),
        Ok(facturacion::ResultadoAlta::Fallo(r)) => tras_escribir(&e, &cx, &u, id, r, "glosa").await,
        Err(_) => averia(),
    }
}

// ── Glosas ───────────────────────────────────────────────────────────────────

/// `GET /panel/facturacion/glosas`
pub async fn glosas(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match facturacion::listar_glosas(&e.pool, u.ambito(), None).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFacturacion);
    let pendientes = lista.iter().filter(|g| g.respuesta.is_none()).count();

    pantalla(
        &cx,
        "Glosas",
        html! {
            (cabecera(&cx, "glosas"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            @if lista.is_empty() {
                p class="sutil" { "No hay glosas registradas." }
            } @else {
                p class="sutil" {
                    (lista.len()) " glosa(s), " (pendientes) " sin responder."
                }
                (piezas::tabla(
                    &["Fecha", "Factura", "Ítem", "Motivo", "Valor", "Estado", "Respuesta"],
                    html! {
                        @for g in &lista {
                            tr {
                                td class="mono" { (g.fecha_glosa) }
                                td { a href={ (RUTA) "/" (g.factura_id) } { (g.numero_factura) } }
                                td { (g.item_glosado) }
                                td class="sutil" { (g.motivo) }
                                td class="mono" { (pesos(g.valor_glosado)) }
                                td { (piezas::estado(&g.estado)) }
                                td {
                                    @match &g.respuesta {
                                        Some(r) => (r),
                                        None if puede => (formulario_respuesta(g.id)),
                                        None => span class="falta" { "sin responder" },
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

/// `POST /panel/facturacion/glosas/{id}/responder`
pub async fn responder_glosa(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let r = facturacion::RespuestaGlosa {
        respuesta: c.get("respuesta").cloned().unwrap_or_default().trim().to_string(),
        aceptar: c.contains_key("aceptar"),
    };
    match facturacion::responder_glosa(&e.pool, u.ambito(), id, &r).await {
        Ok(facturacion::Resultado::Hecho) => de_vuelta(GLOSAS, "glosa_respondida"),
        Ok(facturacion::Resultado::NoExiste) => no_existe(&cx),
        Ok(otro) => {
            // El motivo lleva texto dentro, así que se dibuja en la pantalla de
            // glosas en vez de viajar en la URL.
            let m = motivo(&otro);
            let lista = match facturacion::listar_glosas(&e.pool, u.ambito(), None).await {
                Ok(v) => v,
                Err(_) => return averia(),
            };
            let puede = cx.puede(Capacidad::GestionarFacturacion);
            pantalla(
                &cx,
                "Glosas",
                html! {
                    (cabecera(&cx, "glosas"))
                    section class="tarjeta" { (aviso(Tono::Error, &m)) }
                    @if !lista.is_empty() {
                        (piezas::tabla(
                            &["Factura", "Ítem", "Estado"],
                            html! {
                                @for g in &lista {
                                    tr {
                                        td { a href={ (RUTA) "/" (g.factura_id) } { (g.numero_factura) } }
                                        td { (g.item_glosado) }
                                        td {
                                            (piezas::estado(&g.estado))
                                            @if g.respuesta.is_none() && puede {
                                                (formulario_respuesta(g.id))
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
        Err(_) => averia(),
    }
}

// ── Contratos ────────────────────────────────────────────────────────────────

/// `GET /panel/facturacion/contratos`
pub async fn contratos(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_contratos(&e, &cx, &u, f.aviso.trim(), None).await
}

async fn dibujar_contratos(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let (lista, entidades) = match (
        // Sin filtro: TODOS, activos e inactivos. Esta es la pantalla que los
        // administra; el selector de la factura pide solo los activos.
        facturacion::listar_contratos(&e.pool, u.ambito(), None).await,
        facturacion::listar_entidades(&e.pool).await,
    ) {
        (Ok(l), Ok(en)) => (l, en),
        _ => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFacturacion);

    pantalla(
        cx,
        "Contratos",
        html! {
            (cabecera(cx, "contratos"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            @if lista.is_empty() {
                p class="sutil" { "No hay contratos registrados." }
            } @else {
                (piezas::tabla(
                    &["Contrato", "Número", "Entidad", "Vigencia", "Tarifa", "Incremento", "Estado"],
                    html! {
                        @for c in &lista {
                            tr {
                                td { (c.nombre) }
                                td class="mono" { (c.numero_contrato) }
                                td { (c.entidad_nombre) }
                                td class="mono sutil" { (c.fecha_inicio) " – " (c.fecha_fin) }
                                td { (c.tipo_tarifa) }
                                td class="mono" {
                                    (format!("{:.1}", c.porcentaje_incremento).replace('.', ","))
                                    " %"
                                }
                                td {
                                    @if c.activo {
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
                section class="tarjeta" {
                    h2 { "Nuevo contrato" }
                    @if let Some(t) = error { (aviso(Tono::Error, t)) }
                    @if entidades.is_empty() {
                        p class="sutil" { "No hay entidades con las que contratar." }
                    } @else {
                        form method="post" action=(CONTRATOS) class="apilado" {
                            label for="entidad" { "Entidad" }
                            select #entidad name="entidad_id" required {
                                option value="" { "Elija una" }
                                @for en in &entidades {
                                    option value=(en.id) { (en.nombre) " — NIT " (en.nit) }
                                }
                            }
                            label for="nombre" { "Nombre" }
                            input #nombre type="text" name="nombre" required;
                            label for="numero" { "Número de contrato" }
                            input #numero type="text" name="numero_contrato" required;
                            label for="ini" { "Vigente desde" }
                            input #ini type="date" name="fecha_inicio" required;
                            label for="fin" { "Vigente hasta" }
                            input #fin type="date" name="fecha_fin" required;
                            label for="tarifa" { "Tipo de tarifa" }
                            select #tarifa name="tipo_tarifa" {
                                @for t in TARIFAS { option value=(t) { (t) } }
                            }
                            label for="inc" { "Porcentaje de incremento" }
                            input #inc type="number" name="porcentaje_incremento" step="0.1"
                                  value="0";
                            label for="obs" { "Observaciones" }
                            input #obs type="text" name="observaciones";
                            button type="submit" class="principal" { "Registrar" }
                        }
                    }
                }
            }
        },
    )
}

/// `POST /panel/facturacion/contratos`
pub async fn crear_contrato(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFacturacion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(institucion_id) = u.institucion_id else {
        return dibujar_contratos(
            &e,
            &cx,
            &u,
            "",
            Some("Los contratos son de una institución y esta cuenta no está adscrita a ninguna."),
        )
        .await;
    };
    let Some(entidad_id) = entero(&c, "entidad_id") else {
        return dibujar_contratos(&e, &cx, &u, "", Some("Elija la entidad.")).await;
    };
    let nuevo = facturacion::ContratoNuevo {
        entidad_id,
        nombre: c.get("nombre").cloned().unwrap_or_default().trim().to_string(),
        numero_contrato: c.get("numero_contrato").cloned().unwrap_or_default().trim().to_string(),
        fecha_inicio: c.get("fecha_inicio").cloned().unwrap_or_default(),
        fecha_fin: c.get("fecha_fin").cloned().unwrap_or_default(),
        tipo_tarifa: texto(&c, "tipo_tarifa")
            .filter(|t| TARIFAS.contains(&t.as_str()))
            .unwrap_or_else(|| "CUPS".into()),
        porcentaje_incremento: decimal(&c, "porcentaje_incremento").unwrap_or(0.0),
        observaciones: texto(&c, "observaciones"),
    };
    match facturacion::crear_contrato(&e.pool, u.ambito(), institucion_id, &nuevo).await {
        Ok(facturacion::ResultadoAlta::Creado(_)) => de_vuelta(CONTRATOS, "contrato"),
        Ok(facturacion::ResultadoAlta::Fallo(r)) => {
            let m = motivo(&r);
            dibujar_contratos(&e, &cx, &u, "", Some(&m)).await
        }
        Err(_) => averia(),
    }
}
