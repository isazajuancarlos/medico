// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Farmacia: inventario, fórmulas pendientes, dispensaciones, movimientos y
//! proveedores.
//!
//! Port de `frontend/src/app/pages/farmacia/farmacia.component.ts` (1 342
//! líneas, cinco pestañas). Es la pantalla más grande de las 28, y las cinco
//! pestañas pasan a ser cinco URL.
//!
//! # Las existencias NO se editan a mano, y aquí se nota
//!
//! `ItemEdicion` no tiene `cantidad_actual` a propósito: el stock solo cambia
//! por una entrada, una salida o un ajuste, y las tres escriben su movimiento en
//! la misma transacción. Esta pantalla respeta eso — el formulario de edición no
//! trae el campo, y la única forma de subir existencias es registrar una entrada
//! con su motivo, su lote y su documento de soporte.
//!
//! # Lo que la pantalla NO decide
//!
//! Ni si un lote se puede mezclar, ni si una fórmula ya se entregó, ni si un
//! lote está vencido. Todo eso lo decide `medico_datos::inventario` con una
//! variante por motivo, y aquí se traduce cada una a su frase — **la misma que
//! responde la ruta JSON**, porque dos frases distintas para el mismo hecho son
//! dos productos.
//!
//! # Leer está abierto; escribir exige `GestionarFarmacia`
//!
//! Igual que las nueve rutas JSON: enfermería necesita saber si queda
//! adrenalina antes de pedirla, y un 403 ahí no protege nada que el listado de
//! medicamentos no diga ya.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::{inventario, proveedor};
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{Estado, GestionarFarmacia};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/farmacia";
const FORMULAS: &str = "/panel/farmacia/formulas";
const DISPENSACIONES: &str = "/panel/farmacia/dispensaciones";
const MOVIMIENTOS: &str = "/panel/farmacia/movimientos";
const PROVEEDORES: &str = "/panel/farmacia/proveedores";
const SUMINISTROS: &str = "/panel/farmacia/suministros";

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "creado" => Some((Tono::Ok, "Item registrado.")),
        "guardado" => Some((Tono::Ok, "Cambios guardados.")),
        "entrada" => Some((Tono::Ok, "Entrada registrada: el stock subió y quedó su movimiento.")),
        "dispensado" => Some((Tono::Ok, "Dispensación registrada.")),
        "suministrado" => Some((Tono::Ok, "Suministro registrado: entra en el arreglo otrosServicios del RIPS.")),
        "proveedor_creado" => Some((Tono::Ok, "Proveedor registrado.")),
        "proveedor_baja" => Some((
            Tono::Ok,
            "Proveedor desactivado: deja de ofrecerse en las entradas y sus remesas siguen \
             explicándose.",
        )),
        "proveedor_alta" => Some((Tono::Ok, "Proveedor reactivado.")),
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
    let entero = v.round().abs() as i64;
    let d = entero.to_string();
    let mut s = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            s.push('.');
        }
        s.push(c);
    }
    format!("$ {s}")
}

// ── El armazón ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub solo_alertas: String,
}

fn cabecera(cx: &Contexto, donde: &str) -> Markup {
    html! {
        header class="titulo-seccion" {
            h1 { "Farmacia" }
            nav class="buscador" {
                a class=[(donde == "inventario").then_some("enlace-activo")] href=(RUTA) {
                    "Inventario"
                }
                a class=[(donde == "formulas").then_some("enlace-activo")] href=(FORMULAS) {
                    "Fórmulas pendientes"
                }
                a class=[(donde == "dispensaciones").then_some("enlace-activo")]
                  href=(DISPENSACIONES) { "Dispensaciones" }
                a class=[(donde == "movimientos").then_some("enlace-activo")] href=(MOVIMIENTOS) {
                    "Movimientos"
                }
                a class=[(donde == "suministros").then_some("enlace-activo")] href=(SUMINISTROS) {
                    "Suministros (otros servicios)"
                }
                a class=[(donde == "proveedores").then_some("enlace-activo")] href=(PROVEEDORES) {
                    "Proveedores"
                }
            }
        }
        @if !cx.puede(Capacidad::GestionarFarmacia) {
            p class="sutil" {
                "Su cuenta puede consultar la farmacia, no escribir en ella. Saber si queda un "
                "medicamento es lo primero que necesita quien atiende."
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
            "No encontrado",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró eso" }
                    p { a href=(RUTA) { "Volver a Farmacia" } }
                }
            },
        ),
    )
        .into_response()
}

// ── Inventario ───────────────────────────────────────────────────────────────

/// `GET /panel/farmacia`
pub async fn inventario(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let hoy = medico_datos::hoy();
    let solo_alertas = f.solo_alertas == "1";
    let filtros = inventario::Filtros {
        q: (!f.q.trim().is_empty()).then(|| f.q.trim().to_string()),
        // `solo_alertas` es su propio filtro y NO la suma de `stock_bajo` y
        // `por_vencer`: combinarlas con AND daría solo lo que está bajo mínimo Y
        // además vence, casi nada, justo cuando se pide ver todo lo que reclama
        // atención. Está decidido en el dominio de datos; aquí se respeta.
        solo_alertas: solo_alertas.then_some(true),
        ..Default::default()
    };
    let (items, stats) = match (
        inventario::listar(&e.pool, u.ambito(), &filtros, &hoy).await,
        inventario::stats(&e.pool, u.ambito(), &hoy).await,
    ) {
        (Ok(i), Ok(s)) => (i, s),
        _ => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFarmacia);

    pantalla(
        &cx,
        "Farmacia",
        html! {
            (cabecera(&cx, "inventario"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }

            // Las cifras son del inventario ENTERO, no de lo filtrado: es la
            // misma lección de la pantalla de ambulancias, donde teclear en el
            // buscador ponía «SOAT vencido: 0».
            section class="tarjeta resumen" {
                div { span class="cifra" { (stats.total_items) } span class="sutil" { "items" } }
                div { span class="cifra" { (stats.stock_bajo) } span class="sutil" { "bajo mínimo" } }
                div { span class="cifra" { (stats.por_vencer) } span class="sutil" { "por vencer" } }
                div { span class="cifra" { (stats.vencidos) } span class="sutil" { "vencidos" } }
                div {
                    span class="cifra" { (stats.formulas_pendientes) }
                    span class="sutil" { a href=(FORMULAS) { "fórmulas pendientes" } }
                }
                div {
                    span class="cifra" { (pesos(stats.valor_total_inventario)) }
                    span class="sutil" { "en existencias" }
                }
            }
            @if stats.vencidos > 0 {
                (aviso(Tono::Error, &format!(
                    "{} item(s) VENCIDOS. No se pueden dispensar: hay que darlos de baja.",
                    stats.vencidos
                )))
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="buscador" {
                    input type="search" name="q" value=(f.q) placeholder="Nombre o lote";
                    @if solo_alertas {
                        input type="hidden" name="solo_alertas" value="1";
                    }
                    button type="submit" { "Buscar" }
                }
                nav class="buscador" {
                    a class=[(!solo_alertas).then_some("enlace-activo")] href=(RUTA) { "Todo" }
                    a class=[solo_alertas.then_some("enlace-activo")]
                      href={ (RUTA) "?solo_alertas=1" } { "Solo lo que reclama atención" }
                }
            }

            @if items.is_empty() {
                (piezas::vacio(
                    solo_alertas || !f.q.trim().is_empty(),
                    "No hay items en el inventario.",
                    "Nada coincide con ese filtro.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(items.len(), stats.total_items as usize, "item", "items"))
                (piezas::tabla(
                    &["Nombre", "Lote", "Vence", "Existencias", "Mínimo", "Ubicación", "Valor", ""],
                    html! {
                        @for i in &items {
                            tr {
                                td {
                                    a href={ (RUTA) "/" (i.id) } { (i.nombre) }
                                    @if i.controlado {
                                        span class="pastilla pastilla-mal" { "controlado" }
                                    }
                                    @if i.requiere_refrigeracion {
                                        span class="sutil" { " · cadena de frío" }
                                    }
                                }
                                td class="mono" { (i.lote) }
                                td { (vencimiento(i)) }
                                td class="mono" {
                                    span class=[i.alerta_stock.then_some("falta")] {
                                        (i.cantidad_actual) " " (i.unidad_medida)
                                    }
                                }
                                td class="mono sutil" { (i.cantidad_minima) }
                                td class="sutil" { (i.ubicacion) }
                                td class="mono" { (pesos(i.valor_total)) }
                                td {
                                    @if i.alerta_stock { span class="pastilla pastilla-curso" { "bajo mínimo" } }
                                }
                            }
                        }
                    },
                ))
            }

            @if puede {
                (formulario_alta(None))
            }
        },
    )
}

/// El vencimiento con sus TRES estados. **«Sin registrar» no es «no vence»**: es
/// que el sistema no puede afirmar nada, y por eso no lo afirma — la misma regla
/// que el SOAT de una ambulancia.
fn vencimiento(i: &inventario::Item) -> Markup {
    html! {
        @match &i.fecha_vencimiento {
            None => span class="falta" { "sin registrar" },
            Some(f) if i.alerta_vencimiento => {
                span class="pastilla pastilla-mal" { (f) }
            }
            Some(f) => span class="sutil mono" { (f) },
        }
    }
}

fn formulario_alta(error: Option<&str>) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Nuevo item" }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            form method="post" action=(RUTA) class="apilado" {
                label for="nombre" { "Nombre" }
                input #nombre type="text" name="nombre" required placeholder="Adrenalina 1 mg/mL";
                label for="tipo" { "Tipo" }
                select #tipo name="tipo" {
                    option value="MEDICAMENTO" { "Medicamento" }
                    option value="INSUMO" { "Insumo" }
                    option value="DISPOSITIVO" { "Dispositivo" }
                }
                label for="lote" { "Lote" }
                input #lote type="text" name="lote";
                label for="vence" { "Fecha de vencimiento" }
                input #vence type="date" name="fecha_vencimiento";
                label for="invima" { "Registro INVIMA" }
                input #invima type="text" name="registro_invima";
                label for="cantidad" { "Existencias iniciales" }
                input #cantidad type="number" name="cantidad_actual" min="0" value="0";
                label for="minima" { "Cantidad mínima" }
                input #minima type="number" name="cantidad_minima" min="0" value="0";
                label for="unidad" { "Unidad de medida" }
                input #unidad type="text" name="unidad_medida" placeholder="ampolla";
                label for="ubicacion" { "Ubicación" }
                input #ubicacion type="text" name="ubicacion" placeholder="Estante A";
                label for="costo" { "Costo unitario" }
                input #costo type="number" name="costo_unitario" step="0.01" min="0";
                label {
                    input type="checkbox" name="requiere_refrigeracion" value="1";
                    " Requiere cadena de frío"
                }
                label {
                    input type="checkbox" name="controlado" value="1";
                    " Medicamento controlado"
                }
                button type="submit" class="principal" { "Registrar" }
            }
        }
    }
}

/// `POST /panel/farmacia`
pub async fn crear(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(institucion_id) = u.institucion_id else {
        // Un superadmin no tiene institución: no hay respuesta correcta a «¿de
        // quién es este medicamento?» y no se inventa una. Es el mismo final que
        // da `POST /api/farmacia/inventario`.
        return pantalla(
            &cx,
            "Farmacia",
            html! {
                (cabecera(&cx, "inventario"))
                (formulario_alta(Some(
                    "El inventario es de una institución y esta cuenta no está adscrita a \
                     ninguna.",
                )))
            },
        );
    };
    let nuevo = inventario::ItemNuevo {
        nombre: c.get("nombre").cloned().unwrap_or_default().trim().to_string(),
        tipo: texto(&c, "tipo"),
        categoria: texto(&c, "categoria"),
        presentacion: texto(&c, "presentacion"),
        lote: texto(&c, "lote"),
        fecha_vencimiento: texto(&c, "fecha_vencimiento"),
        registro_invima: texto(&c, "registro_invima"),
        cantidad_actual: entero(&c, "cantidad_actual"),
        cantidad_minima: entero(&c, "cantidad_minima"),
        unidad_medida: texto(&c, "unidad_medida"),
        ubicacion: texto(&c, "ubicacion"),
        costo_unitario: decimal(&c, "costo_unitario"),
        requiere_refrigeracion: Some(c.contains_key("requiere_refrigeracion")),
        controlado: Some(c.contains_key("controlado")),
    };
    match inventario::crear(&e.pool, u.ambito(), institucion_id, &nuevo).await {
        Ok(Some(_)) => de_vuelta(RUTA, "creado"),
        Ok(None) => pantalla(
            &cx,
            "Farmacia",
            html! {
                (cabecera(&cx, "inventario"))
                (formulario_alta(Some("El item necesita nombre.")))
            },
        ),
        Err(_) => averia(),
    }
}

// ── La ficha de un item ──────────────────────────────────────────────────────

/// `GET /panel/farmacia/{id}`
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
    let hoy = medico_datos::hoy();
    let i = match inventario::obtener(&e.pool, u.ambito(), id, &hoy).await {
        Ok(Some(i)) => i,
        Ok(None) => return no_existe(cx),
        Err(_) => return averia(),
    };
    let movs = match inventario::movimientos(&e.pool, u.ambito(), Some(id), None, 1).await {
        Ok(m) => m,
        Err(_) => return averia(),
    };
    let provs = match proveedor::listar(
        &e.pool,
        u.ambito(),
        &proveedor::FiltrosProveedor { q: None, activo: Some(true) },
    )
    .await
    {
        Ok(p) => p,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFarmacia);

    pantalla(
        cx,
        "Item de inventario",
        html! {
            (piezas::migas(RUTA, "Farmacia"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            header class="titulo-seccion" {
                h1 { (i.nombre) }
                @if i.alerta_stock { span class="pastilla pastilla-curso" { "bajo mínimo" } }
                @if i.alerta_vencimiento { span class="pastilla pastilla-mal" { "vencimiento" } }
            }

            section class="tarjeta" {
                div class="resumen" {
                    div {
                        span class="cifra" { (i.cantidad_actual) }
                        span class="sutil" { (i.unidad_medida) " en existencia" }
                    }
                    div {
                        span class="cifra" { (i.cantidad_minima) }
                        span class="sutil" { "mínimo" }
                    }
                    div {
                        span class="cifra" { (pesos(i.valor_total)) }
                        span class="sutil" { "valor" }
                    }
                }
                (piezas::datos(&[
                    ("Lote", Some(i.lote.clone())),
                    ("Vence", i.fecha_vencimiento.clone()),
                    ("Tipo", Some(i.tipo.clone())),
                    ("Categoría", i.categoria.clone()),
                    ("Presentación", i.presentacion.clone()),
                    ("Registro INVIMA", i.registro_invima.clone()),
                    ("Ubicación", Some(i.ubicacion.clone())),
                    ("Costo unitario", Some(pesos(i.costo_unitario))),
                    ("Cadena de frío", Some(if i.requiere_refrigeracion { "sí".into() } else { "no".into() })),
                    ("Controlado", Some(if i.controlado { "sí".into() } else { "no".into() })),
                ]))
            }

            @if puede {
                section class="tarjeta" {
                    h2 { "Registrar una entrada" }
                    p class="sutil" {
                        "Es la ÚNICA forma de subir las existencias: cada entrada deja su "
                        "movimiento con quién, cuándo y por qué. Editar el número a mano sería "
                        "una puerta trasera al histórico."
                    }
                    form method="post" action={ (RUTA) "/" (i.id) "/entrada" } class="apilado" {
                        label for="cantidad" { "Cantidad que entra" }
                        input #cantidad type="number" name="cantidad" min="1" required;
                        label for="motivo" { "Motivo" }
                        input #motivo type="text" name="motivo" required
                              placeholder="Compra, donación, traslado…";
                        label for="lote_e" { "Lote de la remesa" }
                        input #lote_e type="text" name="lote" value=(i.lote)
                              placeholder="El mismo lote";
                        p class="sutil" {
                            "Un lote distinto NO se suma a este: vencen en fechas distintas y "
                            "dejarían de poder rastrearse por separado. Se da de alta como item "
                            "nuevo."
                        }
                        label for="vence_e" { "Fecha de vencimiento" }
                        input #vence_e type="date" name="fecha_vencimiento";
                        label for="costo_e" { "Costo unitario" }
                        input #costo_e type="number" name="costo_unitario" step="0.01" min="0";
                        label for="proveedor" { "Proveedor" }
                        select #proveedor name="proveedor_id" {
                            // Vacío a propósito: hay entradas que no son una
                            // compra —una donación, un traslado— y obligar a
                            // inventar un proveedor produciría el dato falso que
                            // este módulo evita en todo lo demás.
                            option value="" { "Sin proveedor (donación, traslado, ajuste)" }
                            @for p in &provs {
                                option value=(p.id) { (p.razon_social) }
                            }
                        }
                        label for="soporte" { "Documento de soporte" }
                        input #soporte type="text" name="documento_soporte"
                              placeholder="Factura o remisión";
                        button type="submit" class="principal" { "Registrar la entrada" }
                    }
                }

                section class="tarjeta" {
                    h2 { "Corregir la ficha" }
                    p class="sutil" {
                        "Las existencias NO se editan aquí: se mueven con una entrada o una "
                        "salida, que dejan rastro."
                    }
                    form method="post" action={ (RUTA) "/" (i.id) } class="apilado" {
                        label for="nombre_e" { "Nombre" }
                        input #nombre_e type="text" name="nombre" value=(i.nombre);
                        label for="minima_e" { "Cantidad mínima" }
                        input #minima_e type="number" name="cantidad_minima" min="0"
                              value=(i.cantidad_minima);
                        label for="ubicacion_e" { "Ubicación" }
                        input #ubicacion_e type="text" name="ubicacion" value=(i.ubicacion);
                        label for="invima_e" { "Registro INVIMA" }
                        input #invima_e type="text" name="registro_invima"
                              value=(i.registro_invima.clone().unwrap_or_default());
                        label {
                            input type="checkbox" name="controlado" value="1" checked[i.controlado];
                            " Medicamento controlado"
                        }
                        label {
                            input type="checkbox" name="requiere_refrigeracion" value="1"
                                  checked[i.requiere_refrigeracion];
                            " Requiere cadena de frío"
                        }
                        button type="submit" class="principal" { "Guardar" }
                    }
                }
            }

            section class="tarjeta" {
                h2 { "Movimientos" }
                @if movs.is_empty() {
                    p class="sutil" { "Este item no registra movimientos." }
                } @else {
                    (tabla_movimientos(&movs))
                }
            }
        },
    )
}

fn tabla_movimientos(movs: &[inventario::Movimiento]) -> Markup {
    piezas::tabla(
        &["Fecha", "Tipo", "Cantidad", "Item", "Motivo", "Quién", "Proveedor", "Soporte"],
        html! {
            @for m in movs {
                tr {
                    td class="mono" { (m.fecha) }
                    td { (piezas::estado(&m.tipo_movimiento)) }
                    td class="mono" { (m.cantidad) }
                    td { (m.item_nombre.clone().unwrap_or_default()) }
                    td class="sutil" { (m.motivo) }
                    td class="sutil" { (m.usuario_nombre.clone().unwrap_or_default()) }
                    td class="sutil" { (m.proveedor_nombre.clone().unwrap_or_default()) }
                    td class="sutil mono" { (m.documento_soporte.clone().unwrap_or_default()) }
                }
            }
        },
    )
}

/// `POST /panel/farmacia/{id}` — corregir la ficha.
pub async fn guardar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let edit = inventario::ItemEdicion {
        nombre: texto(&c, "nombre"),
        cantidad_minima: entero(&c, "cantidad_minima"),
        ubicacion: texto(&c, "ubicacion"),
        registro_invima: texto(&c, "registro_invima"),
        controlado: Some(c.contains_key("controlado")),
        requiere_refrigeracion: Some(c.contains_key("requiere_refrigeracion")),
        ..Default::default()
    };
    match inventario::actualizar(&e.pool, u.ambito(), id, &edit).await {
        Ok(true) => de_vuelta(&format!("{RUTA}/{id}"), "guardado"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// `POST /panel/farmacia/{id}/entrada`
pub async fn entrada(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    use inventario::ResultadoEntrada as R;
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let entrada = inventario::Entrada {
        cantidad: entero(&c, "cantidad").unwrap_or(0),
        motivo: c.get("motivo").cloned().unwrap_or_default().trim().to_string(),
        lote: texto(&c, "lote"),
        fecha_vencimiento: texto(&c, "fecha_vencimiento"),
        costo_unitario: decimal(&c, "costo_unitario"),
        proveedor_id: entero(&c, "proveedor_id"),
        documento_soporte: texto(&c, "documento_soporte"),
    };
    let r = match inventario::entrada(&e.pool, u.ambito(), id, u.id, &entrada, &medico_datos::hoy())
        .await
    {
        Ok(r) => r,
        Err(_) => return averia(),
    };
    // Cada negativa con SU frase, la misma que responde la ruta JSON: dos frases
    // distintas para el mismo hecho son dos productos.
    let motivo = match r {
        R::Hecha(_) => return de_vuelta(&format!("{RUTA}/{id}"), "entrada"),
        R::NoExiste => return no_existe(&cx),
        R::CantidadInvalida => "La cantidad que entra tiene que ser mayor que cero.".to_string(),
        R::LoteDistinto { actual, quedan } => format!(
            "Quedan {quedan} unidades del lote {actual}. Un lote nuevo no se suma al anterior: \
             vencen en fechas distintas y dejarían de poder rastrearse por separado. Dé de alta \
             el lote nuevo como item."
        ),
        R::ProveedorNoValido => "Ese proveedor no es de esta institución o está inactivo. La \
                                 entrada no se registró: guardarla sin procedencia habría dejado \
                                 una remesa que usted cree tener rastreada."
            .to_string(),
    };
    dibujar_ficha(&e, &cx, &u, id, "", Some(&motivo)).await
}

// ── Fórmulas pendientes y dispensación ───────────────────────────────────────

/// `GET /panel/farmacia/formulas`
pub async fn formulas(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_formulas(&e, &cx, &u, f.aviso.trim(), None).await
}

async fn dibujar_formulas(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let hoy = medico_datos::hoy();
    let (pendientes, items) = match (
        inventario::formulas_pendientes(&e.pool, u.ambito(), None).await,
        inventario::listar(&e.pool, u.ambito(), &Default::default(), &hoy).await,
    ) {
        (Ok(p), Ok(i)) => (p, i),
        _ => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFarmacia);

    pantalla(
        cx,
        "Fórmulas pendientes",
        html! {
            (cabecera(cx, "formulas"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            @if pendientes.is_empty() {
                p class="sutil" { "No hay fórmulas esperando entrega." }
            } @else {
                (piezas::recuento(pendientes.len(), pendientes.len(), "fórmula", "fórmulas"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Medicamento", "Dosis", "Cantidad", "Médico", ""],
                    html! {
                        @for p in &pendientes {
                            tr {
                                td class="mono" { (p.fecha_formula) }
                                td { (p.paciente_nombre.clone().unwrap_or_else(|| "—".into())) }
                                td {
                                    (p.medicamento)
                                    @if let Some(c) = &p.codigo_cum {
                                        span class="sutil" { " · CUM " (c) }
                                    }
                                }
                                td class="sutil" { (p.dosis) }
                                td class="mono" {
                                    (p.cantidad) " " (p.unidad.clone().unwrap_or_default())
                                }
                                td class="sutil" { (p.medico_nombre.clone().unwrap_or_default()) }
                                td {
                                    @if puede && !items.is_empty() {
                                        (formulario_dispensa(p, &items))
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

/// El formulario de entrega de UNA fórmula, en su propia fila.
///
/// El lote no se pide: lo pone el item del que se descuenta. El campo existe en
/// la API para que un cliente pueda AFIRMAR cuál cree que es —y que se le
/// rechace si no coincide—, y esta pantalla no tiene nada que afirmar porque el
/// item lo elige aquí mismo.
fn formulario_dispensa(p: &inventario::FormulaPendiente, items: &[inventario::Item]) -> Markup {
    html! {
        form method="post" action={ (RUTA) "/dispensar" } class="apilado" {
            input type="hidden" name="formula_id" value=(p.id);
            select name="item_inventario_id" required {
                option value="" { "Item del que se descuenta" }
                @for i in items {
                    @if i.cantidad_actual > 0 {
                        option value=(i.id) {
                            (i.nombre) " · lote " (i.lote) " · " (i.cantidad_actual) " "
                            (i.unidad_medida)
                        }
                    }
                }
            }
            input type="number" name="cantidad_dispensada" min="1" required
                  value=(p.cantidad.round() as i64);
            button type="submit" { "Dispensar" }
        }
    }
}

/// `POST /panel/farmacia/dispensar`
pub async fn dispensar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    use inventario::ResultadoDispensa as R;
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let (Some(formula_id), Some(item_inventario_id)) =
        (entero(&c, "formula_id"), entero(&c, "item_inventario_id"))
    else {
        return dibujar_formulas(&e, &cx, &u, "", Some("Elija el item del que se descuenta."))
            .await;
    };
    let d = inventario::DispensarNueva {
        formula_id,
        item_inventario_id,
        cantidad_dispensada: entero(&c, "cantidad_dispensada").unwrap_or(0),
        // Vacío: se toma el del item. Ver la nota de `formulario_dispensa`.
        lote_dispensado: String::new(),
        observaciones: texto(&c, "observaciones"),
    };
    let r = match inventario::dispensar(&e.pool, u.ambito(), u.id, &d, &medico_datos::hoy()).await {
        Ok(r) => r,
        Err(_) => return averia(),
    };
    let motivo = match r {
        R::Hecha(_) => return de_vuelta(FORMULAS, "dispensado"),
        R::NoExiste => "Esa fórmula o ese item no existen en esta institución.".to_string(),
        R::CantidadInvalida => "La cantidad a dispensar tiene que ser mayor que cero.".to_string(),
        R::YaCompleta => "Esta fórmula ya se entregó completa.".to_string(),
        R::ExcedePrescrito { pendiente } => format!(
            "De esta fórmula quedan {pendiente} por entregar; no se puede dispensar más de lo \
             prescrito."
        ),
        R::SinExistencias { disponible } => {
            format!("Solo quedan {disponible} unidades de ese item.")
        }
        R::LoteVencido { vencio } => format!(
            "Ese lote venció el {vencio}. No se dispensa: hay que darlo de baja por vencimiento."
        ),
        R::LoteNoCoincide { del_item } => {
            format!("El lote indicado no es el del item seleccionado, que es el {del_item}.")
        }
    };
    dibujar_formulas(&e, &cx, &u, "", Some(&motivo)).await
}

// ── Históricos ───────────────────────────────────────────────────────────────

/// `GET /panel/farmacia/dispensaciones`
pub async fn dispensaciones(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match inventario::dispensaciones(&e.pool, u.ambito(), 1).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Dispensaciones",
        html! {
            (cabecera(&cx, "dispensaciones"))
            @if lista.is_empty() {
                p class="sutil" { "No hay dispensaciones registradas." }
            } @else {
                (piezas::tabla(
                    &["Fecha", "Paciente", "Medicamento", "Item", "Lote", "Cantidad", "Quién"],
                    html! {
                        @for d in &lista {
                            tr {
                                td class="mono" { (d.fecha_dispensacion) }
                                td { (d.paciente_nombre.clone().unwrap_or_default()) }
                                td { (d.medicamento_nombre.clone().unwrap_or_default()) }
                                td class="sutil" { (d.item_nombre.clone().unwrap_or_default()) }
                                td class="mono" { (d.lote_dispensado) }
                                td class="mono" { (d.cantidad_dispensada) }
                                td class="sutil" { (d.regente_nombre.clone().unwrap_or_default()) }
                            }
                        }
                    },
                ))
            }
        },
    )
}

// ── Suministros (otros servicios / AT del RIPS) ──────────────────────────────

/// `GET /panel/farmacia/suministros` — el formulario para suministrar un
/// insumo/dispositivo DENTRO de una atención (#67, arreglo `otrosServicios`).
///
/// A diferencia de la dispensación de un medicamento —que cuelga de una fórmula—
/// esto cuelga de la ATENCIÓN (`historia_id`) y captura su FACTURACIÓN: de ella
/// sale lo que se le cobra a la EPS, así que los valores se piden aquí y no se
/// suponen. El desplegable ofrece SOLO ítems que no son medicamento; un
/// medicamento va por «Fórmulas pendientes».
pub async fn suministros(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_suministros(&e, &cx, &u, "", None).await
}

async fn dibujar_suministros(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let hoy = medico_datos::hoy();
    let items = match inventario::listar(&e.pool, u.ambito(), &Default::default(), &hoy).await {
        Ok(i) => i,
        Err(_) => return averia(),
    };
    // Solo lo que NO es medicamento y tiene existencias: un medicamento va en el
    // AM, y ofrecerlo aquí sería el doble cobro que la regla del dominio impide.
    let disponibles: Vec<&inventario::Item> = items
        .iter()
        .filter(|i| i.tipo != "MEDICAMENTO" && i.cantidad_actual > 0)
        .collect();
    let puede = cx.puede(Capacidad::GestionarFarmacia);

    pantalla(
        cx,
        "Suministros",
        html! {
            (cabecera(cx, "suministros"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            p class="sutil" {
                "Un insumo o dispositivo entregado en una atención. Cuelga de la historia "
                "(no de una fórmula) y su facturación entra en el arreglo "
                mono { "otrosServicios" } " del RIPS."
            }
            @if !puede {
                p class="sutil" { "Su cuenta puede consultar la farmacia, no escribir en ella." }
            } @else if disponibles.is_empty() {
                p class="sutil" {
                    "No hay insumos ni dispositivos con existencias. En «Inventario» se dan de "
                    "alta (tipo INSUMO, DISPOSITIVO o REACTIVO)."
                }
            } @else {
                (formulario_suministro(&disponibles))
            }
        },
    )
}

/// El formulario de un suministro. Los `vr*` van como enteros en pesos; el
/// concepto de recaudo trae «05» (no recaudo) por defecto, que es el caso normal
/// de un insumo cubierto — se cambia si aplica moderador.
fn formulario_suministro(items: &[&inventario::Item]) -> Markup {
    html! {
        form method="post" action=(SUMINISTROS) class="apilado tarjeta estrecha" {
            label { "Atención (id de la historia)"
                input type="number" name="historia_id" min="1" required
                      placeholder="p. ej. 100";
            }
            label { "Insumo o dispositivo"
                select name="item_inventario_id" required {
                    option value="" { "Elija el ítem" }
                    @for i in items {
                        option value=(i.id) {
                            (i.nombre) " · lote " (i.lote) " · " (i.cantidad_actual) " "
                            (i.unidad_medida)
                        }
                    }
                }
            }
            label { "Cantidad"
                input type="number" name="cantidad_dispensada" min="1" required value="1";
            }
            label { "Valor unitario (vrUnitOS)"
                input type="number" name="valor_unitario" min="0" required placeholder="pesos";
            }
            label { "Valor del servicio (vrServicio)"
                input type="number" name="valor_servicio" min="0" required placeholder="pesos";
            }
            label { "Valor de la dispensación (vrDispensacion)"
                input type="number" name="valor_dispensacion" min="0" value="0";
            }
            label { "Número de autorización"
                input type="text" name="num_autorizacion" maxlength="30" placeholder="opcional";
            }
            label { "Id MIPRES"
                input type="text" name="id_mipres" maxlength="30" placeholder="opcional";
            }
            label { "Concepto de recaudo"
                input type="text" name="concepto_recaudo" maxlength="2" value="05";
            }
            label { "Valor pago moderador"
                input type="number" name="valor_pago_moderador" min="0" value="0";
            }
            label { "Observaciones"
                input type="text" name="observaciones" maxlength="200" placeholder="opcional";
            }
            button type="submit" { "Registrar suministro" }
        }
    }
}

/// `POST /panel/farmacia/suministros`
pub async fn suministrar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    use inventario::ResultadoSuministro as R;
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let (Some(historia_id), Some(item_inventario_id)) =
        (entero(&c, "historia_id"), entero(&c, "item_inventario_id"))
    else {
        return dibujar_suministros(&e, &cx, &u, "", Some("Indique la atención y el ítem."))
            .await;
    };
    let s = inventario::SuministroNuevo {
        historia_id,
        item_inventario_id,
        cantidad_dispensada: entero(&c, "cantidad_dispensada").unwrap_or(0),
        lote_dispensado: String::new(),
        observaciones: texto(&c, "observaciones"),
        num_autorizacion: texto(&c, "num_autorizacion"),
        id_mipres: texto(&c, "id_mipres"),
        valor_unitario: entero(&c, "valor_unitario"),
        valor_dispensacion: entero(&c, "valor_dispensacion"),
        valor_servicio: entero(&c, "valor_servicio"),
        concepto_recaudo: texto(&c, "concepto_recaudo"),
        valor_pago_moderador: entero(&c, "valor_pago_moderador"),
        num_fev_pago_moderador: texto(&c, "num_fev_pago_moderador"),
    };
    let r = match inventario::suministrar_insumo(&e.pool, u.ambito(), u.id, &s, &medico_datos::hoy())
        .await
    {
        Ok(r) => r,
        Err(_) => return averia(),
    };
    let motivo = match r {
        R::Hecho { .. } => return de_vuelta(SUMINISTROS, "suministrado"),
        R::NoExiste => "Esa atención o ese ítem no existen en esta institución.".to_string(),
        R::Firmada => {
            "La historia está firmada: no se le pueden colgar más suministros.".to_string()
        }
        R::CantidadInvalida => "La cantidad tiene que ser mayor que cero.".to_string(),
        R::EsMedicamento => {
            "Ese ítem es un medicamento; su entrega se registra en «Fórmulas pendientes», no aquí."
                .to_string()
        }
        R::SinExistencias { disponible } => {
            format!("Solo quedan {disponible} unidades de ese ítem.")
        }
        R::LoteVencido { vencio } => {
            format!("Ese lote venció el {vencio}: no se suministra.")
        }
        R::LoteNoCoincide { del_item } => {
            format!("El lote indicado no es el del ítem, que es el {del_item}.")
        }
    };
    dibujar_suministros(&e, &cx, &u, "", Some(&motivo)).await
}

/// `GET /panel/farmacia/movimientos`
pub async fn movimientos(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match inventario::movimientos(&e.pool, u.ambito(), None, None, 1).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Movimientos",
        html! {
            (cabecera(&cx, "movimientos"))
            @if lista.is_empty() {
                p class="sutil" { "No hay movimientos registrados." }
            } @else {
                (tabla_movimientos(&lista))
            }
        },
    )
}

// ── Proveedores ──────────────────────────────────────────────────────────────

/// `GET /panel/farmacia/proveedores`
pub async fn proveedores(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_proveedores(&e, &cx, &u, f.aviso.trim(), None).await
}

async fn dibujar_proveedores(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    // Sin filtro: TODOS, activos e inactivos. Esta es la pantalla que los
    // administra; el selector de la entrada es el que pide solo los activos.
    let lista = match proveedor::listar(&e.pool, u.ambito(), &Default::default()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::GestionarFarmacia);
    pantalla(
        cx,
        "Proveedores",
        html! {
            (cabecera(cx, "proveedores"))
            @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
                (aviso(tono, texto))
            }
            @if lista.is_empty() {
                p class="sutil" { "No hay proveedores registrados." }
            } @else {
                (piezas::tabla(
                    &["Razón social", "NIT", "Contacto", "Suministra", "Remesas", "Estado", ""],
                    html! {
                        @for p in &lista {
                            tr {
                                td { (p.razon_social) }
                                td class="mono" { (p.nit.clone().unwrap_or_default()) }
                                td class="sutil" {
                                    (p.contacto.clone().unwrap_or_default())
                                    @if let Some(t) = &p.telefono { " · " (t) }
                                }
                                td class="sutil" { (p.suministra.clone().unwrap_or_default()) }
                                td class="mono" { (p.entradas) }
                                td {
                                    @if p.activo {
                                        (piezas::estado("ACTIVA"))
                                    } @else {
                                        (piezas::estado("INACTIVA"))
                                    }
                                }
                                td {
                                    @if puede {
                                        form method="post"
                                             action={ (PROVEEDORES) "/" (p.id) "/baja" } {
                                            button type="submit" class="enlace" {
                                                @if p.activo { "Desactivar" } @else { "Reactivar" }
                                            }
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
                    h2 { "Nuevo proveedor" }
                    @if let Some(t) = error { (aviso(Tono::Error, t)) }
                    form method="post" action=(PROVEEDORES) class="apilado" {
                        label for="razon" { "Razón social" }
                        input #razon type="text" name="razon_social" required;
                        label for="nit" { "NIT" }
                        input #nit type="text" name="nit";
                        label for="contacto" { "Contacto" }
                        input #contacto type="text" name="contacto";
                        label for="telefono" { "Teléfono" }
                        input #telefono type="text" name="telefono";
                        label for="email" { "Correo" }
                        input #email type="email" name="email";
                        label for="ciudad" { "Ciudad" }
                        input #ciudad type="text" name="ciudad";
                        label for="suministra" { "Qué suministra" }
                        input #suministra type="text" name="suministra";
                        button type="submit" class="principal" { "Registrar" }
                    }
                }
            }
        },
    )
}

/// `POST /panel/farmacia/proveedores`
pub async fn crear_proveedor(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    use proveedor::ResultadoAlta as R;
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(institucion_id) = u.institucion_id else {
        return dibujar_proveedores(
            &e,
            &cx,
            &u,
            "",
            Some("Los proveedores son de una institución y esta cuenta no está adscrita a ninguna."),
        )
        .await;
    };
    let nuevo = proveedor::ProveedorNuevo {
        razon_social: c.get("razon_social").cloned().unwrap_or_default().trim().to_string(),
        nit: texto(&c, "nit"),
        contacto: texto(&c, "contacto"),
        telefono: texto(&c, "telefono"),
        email: texto(&c, "email"),
        direccion: texto(&c, "direccion"),
        ciudad: texto(&c, "ciudad"),
        suministra: texto(&c, "suministra"),
        observaciones: None,
    };
    match proveedor::crear(&e.pool, u.ambito(), institucion_id, &nuevo).await {
        Ok(R::Creado(_)) => de_vuelta(PROVEEDORES, "proveedor_creado"),
        Ok(otro) => {
            let motivo = motivo_proveedor(otro);
            dibujar_proveedores(&e, &cx, &u, "", Some(&motivo)).await
        }
        Err(_) => averia(),
    }
}

fn motivo_proveedor(r: proveedor::ResultadoAlta) -> String {
    use proveedor::ResultadoAlta as R;
    match r {
        R::Creado(_) => String::new(),
        R::NoExiste => "Ese proveedor ya no existe.".into(),
        R::SinRazonSocial => "El proveedor necesita razón social: es como se identifica en una \
                              factura."
            .into(),
        R::NitRepetido(n) => format!("Ya hay un proveedor con NIT {n} en esta institución."),
    }
}

/// `POST /panel/farmacia/proveedores/{id}/baja` — alterna la baja.
///
/// **Alterna en vez de recibir el valor**, igual que la baja de una ambulancia:
/// un `activo` en el cuerpo dejaría que reenviar una petición vieja reactivara
/// lo que alguien acaba de desactivar.
pub async fn baja_proveedor(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarFarmacia>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let actual = match proveedor::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => p,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let cambios = proveedor::ProveedorEdicion {
        activo: Some(!actual.activo),
        ..Default::default()
    };
    match proveedor::actualizar(&e.pool, u.ambito(), id, &cambios).await {
        Ok(proveedor::ResultadoAlta::Creado(_)) => de_vuelta(
            PROVEEDORES,
            if actual.activo { "proveedor_baja" } else { "proveedor_alta" },
        ),
        Ok(proveedor::ResultadoAlta::NoExiste) => no_existe(&cx),
        Ok(_) | Err(_) => averia(),
    }
}
