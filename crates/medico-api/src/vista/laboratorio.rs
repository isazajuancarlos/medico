// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Laboratorio clínico: la bandeja, el resultado y los perfiles de examen.
//!
//! Port de `frontend/src/app/pages/laboratorio/laboratorio.component.ts` (745
//! líneas, tres vistas en un `signal<'lista'|'procesar'|'perfiles'>`).
//!
//! Es la hermana de [`crate::vista::imagenes`] —las dos salen de
//! `medico_datos::diagnostico`— y comparte con ella todo lo estructural: POST y
//! 303, avisos de lista cerrada, lectura abierta y escritura con capacidad. Lo
//! propio de aquí es **el resultado**, que no es un texto sino una tabla de
//! parámetros con su valor, su unidad, sus referencias y su marca de crítico.
//!
//! # La forma del resultado ya no se escribe en el navegador
//!
//! `resultados_json` es un `Value` a propósito. Pero la forma de dentro existe:
//! la escribía `_filasToPayload` y la leía `_resultadoToFilas`, las dos en el
//! componente de Angular. Con un segundo frontal pasaba a tener dos autores, así
//! que se subió al dominio de datos: [`medico_datos::diagnostico::Parametro`],
//! con su ida y su vuelta probadas. Aquí no se reinventa.
//!
//! # Las filas dinámicas, sin JavaScript
//!
//! Angular añadía y quitaba filas con `signal.update`. Aquí el formulario dibuja
//! **las filas que hay más tres en blanco**, y al guardar se descartan las que no
//! tienen nombre — que es exactamente lo que hacía `_filasToPayload`. Se añaden
//! tres parámetros por pasada; no hay estado que mantener y funciona con el
//! JavaScript apagado.
//!
//! Los campos van con el índice en el NOMBRE (`nombre_0`, `valor_0`, …) y no
//! como listas: una casilla sin marcar no se envía, así que con `critico[]` la
//! columna se desalinearía de las demás y un valor crítico acabaría marcado en
//! la fila de al lado.
//!
//! # Cargar un perfil no guarda nada
//!
//! `?perfil=N` dibuja el formulario con los parámetros de ese perfil y el valor
//! en blanco. Es lo mismo que hacía el botón «cargar desde perfil», y sigue sin
//! tocar la base: un perfil propone QUÉ se mide, no qué salió.
//!
//! # La alerta de valores críticos no depende de que alguien se acuerde
//!
//! `crear_resultado` enciende `valores_criticos` si CUALQUIER parámetro está
//! marcado, aunque la casilla global esté sin marcar. Eso ya estaba y no se toca;
//! lo que la pantalla añade es decirlo, porque una casilla que se enciende sola
//! sin explicación se lee como un fallo.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::diagnostico::{self, Escritura, Parametro, Servicio};
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{Estado, EscribirLaboratorio};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const SERVICIO: Servicio = Servicio::Laboratorio;
const RUTA: &str = "/panel/laboratorio";
const PERFILES: &str = "/panel/laboratorio/perfiles";

/// El mismo texto que responde `/api/laboratorio/…` cuando ya está validado.
const CERRADO: &str = "Este resultado ya está validado y no se modifica: sobre él puede \
                       haberse decidido una conducta. Emita uno nuevo que lo corrija.";

/// Filas en blanco que se ofrecen de más. Tres es lo que cabe pedir de una vez
/// sin llenar la pantalla de huecos; para un hemograma entero está el perfil.
const FILAS_EN_BLANCO: usize = 3;

/// Tope de filas que se leen del formulario. Es una barandilla contra un cuerpo
/// fabricado a mano, no un límite del producto: el perfil más largo de un
/// laboratorio clínico no llega a cincuenta parámetros.
const MAX_FILAS: usize = 200;

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "guardado" => Some((Tono::Ok, "Resultados guardados.")),
        "validado" => Some((
            Tono::Ok,
            "Resultado validado. La orden queda completada y el resultado, de solo lectura.",
        )),
        "iniciado" => Some((Tono::Ok, "La orden pasa a «En Proceso».")),
        "perfil_creado" => Some((Tono::Ok, "Perfil creado.")),
        "perfil_retirado" => Some((
            Tono::Ok,
            "Perfil retirado. Deja de ofrecerse y los resultados que lo usaron no cambian.",
        )),
        "cerrado" => Some((Tono::Error, CERRADO)),
        "sin_parametros" => Some((
            Tono::Error,
            "No se guardó nada: un resultado sin ningún parámetro con nombre no dice qué se \
             midió. Escriba al menos uno, o cargue un perfil.",
        )),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

// ── La bandeja ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub aviso: String,
    /// Cargar los parámetros de un perfil en el formulario, sin guardar nada.
    #[serde(default)]
    pub perfil: Option<i64>,
}

/// `GET /panel/laboratorio`
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
    if !estado.is_empty() && !diagnostico::ESTADOS.contains(&estado) {
        return pantalla(
            &cx,
            "Laboratorio",
            html! {
                (cabecera(&cx, "bandeja", ""))
                section class="tarjeta" {
                    (aviso(Tono::Error, &format!(
                        "«{estado}» no es un estado de este producto. Los que hay son: {}.",
                        diagnostico::ESTADOS.join(", ")
                    )))
                }
            },
        );
    }

    let ordenes = match diagnostico::listar_ordenes(
        &e.pool,
        u.ambito(),
        SERVICIO,
        (!estado.is_empty()).then_some(estado),
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    pantalla(
        &cx,
        "Laboratorio",
        html! {
            (cabecera(&cx, "bandeja", estado))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            @if ordenes.is_empty() {
                (piezas::vacio(
                    !estado.is_empty(),
                    "No hay órdenes de laboratorio.",
                    "Ninguna orden está en ese estado.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(ordenes.len(), ordenes.len(), "orden", "órdenes"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Documento", "Examen", "Solicitante", "Estado", "Resultado"],
                    html! {
                        @for o in &ordenes {
                            tr {
                                td class="mono" { (o.fecha_orden) }
                                td {
                                    a href={ (RUTA) "/" (o.id) } {
                                        (o.paciente_nombre.clone().unwrap_or_else(|| "—".into()))
                                    }
                                }
                                td class="mono" {
                                    (o.paciente_identificacion.clone().unwrap_or_default())
                                }
                                td {
                                    (o.descripcion)
                                    @if let Some(c) = &o.codigo_cups {
                                        span class="sutil" { " · CUPS " (c) }
                                    }
                                }
                                td { (o.medico_nombre.clone().unwrap_or_else(|| "—".into())) }
                                td { (piezas::estado(&o.estado)) }
                                td {
                                    @if o.tiene_resultado {
                                        (piezas::estado("EMITIDO"))
                                    } @else {
                                        span class="falta" { "pendiente" }
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

fn cabecera(cx: &Contexto, donde: &str, estado_activo: &str) -> Markup {
    let puede = cx.puede(Capacidad::EscribirLaboratorio);
    html! {
        header class="titulo-seccion" {
            h1 { "Laboratorio clínico" }
            nav class="buscador" {
                a class=[(donde == "bandeja").then_some("enlace-activo")] href=(RUTA) { "Bandeja" }
                a class=[(donde == "perfiles").then_some("enlace-activo")] href=(PERFILES) {
                    "Perfiles de examen"
                }
            }
        }
        @if donde == "bandeja" {
            section class="tarjeta" {
                nav class="buscador" {
                    a class=[estado_activo.is_empty().then_some("enlace-activo")] href=(RUTA) {
                        "Todas"
                    }
                    @for s in diagnostico::ESTADOS {
                        a class=[(estado_activo == s).then_some("enlace-activo")]
                          href={ (RUTA) "?estado=" (s) } { (s) }
                    }
                }
            }
        }
        @if !puede && donde != "bandeja" {
            p class="sutil" {
                "Su cuenta puede consultar el laboratorio, no escribir en él."
            }
        }
    }
}

// ── El resultado ─────────────────────────────────────────────────────────────

/// `GET /panel/laboratorio/{id}`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let orden = match diagnostico::obtener_orden(&e.pool, u.ambito(), SERVICIO, id).await {
        Ok(Some(o)) => o,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let resultado = match diagnostico::resultado_de_orden(&e.pool, u.ambito(), id).await {
        Ok(r) => r,
        Err(_) => return averia(),
    };
    let perfiles = match diagnostico::listar_perfiles(&e.pool, u.ambito()).await {
        Ok(p) => p,
        Err(_) => return averia(),
    };

    // Qué filas se dibujan, en este orden de preferencia:
    //  1. las del resultado guardado —lo que ya se midió—;
    //  2. las que propone el perfil pedido con `?perfil=`;
    //  3. ninguna, y solo quedan las tres en blanco.
    //
    // El perfil NO pisa lo guardado: quien abre un resultado a medias y elige un
    // perfil por error no puede perder los valores que ya había escrito.
    let del_resultado = resultado
        .as_ref()
        .map(|r| diagnostico::parametros_de(&r.resultados_json))
        .unwrap_or_default();
    let (filas, perfil_aplicado) = if !del_resultado.is_empty() {
        (del_resultado, None)
    } else if let Some(pid) = f.perfil {
        match perfiles.iter().find(|p| p.id == pid) {
            Some(p) => (
                diagnostico::parametros_de_perfil(&p.examenes),
                Some(p.nombre.clone()),
            ),
            // Un perfil que no es de esta institución no existe para ella. Se
            // dibuja el formulario vacío en vez de un 404: la orden sí existe.
            None => (Vec::new(), None),
        }
    } else {
        (Vec::new(), None)
    };

    let puede = cx.puede(Capacidad::EscribirLaboratorio);
    let validado = resultado.as_ref().is_some_and(|r| r.validado);
    let editable = puede && !validado;

    pantalla(
        &cx,
        "Resultado de laboratorio",
        html! {
            (piezas::migas(RUTA, "Laboratorio clínico"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            header class="titulo-seccion" {
                h1 { "Resultado de laboratorio" }
                (piezas::estado(&orden.estado))
            }

            section class="tarjeta" {
                (piezas::datos(&[
                    ("Paciente", orden.paciente_nombre.clone()),
                    ("Documento", orden.paciente_identificacion.clone()),
                    ("Examen solicitado", Some(orden.descripcion.clone())),
                    ("CUPS", orden.codigo_cups.clone()),
                    ("Solicitante", orden.medico_nombre.clone()),
                    ("Fecha de la orden", Some(orden.fecha_orden.clone())),
                    ("Indicación clínica", Some(orden.justificacion.clone())),
                ]))
                @if orden.estado == "Pendiente" && puede {
                    form method="post" action={ (RUTA) "/" (orden.id) "/iniciar" } {
                        button type="submit" { "Iniciar el procesamiento" }
                    }
                }
            }

            @if editable {
                @if del_resultado_vacio(&filas) && !perfiles.is_empty() {
                    section class="tarjeta" {
                        h2 { "Cargar un perfil" }
                        p class="sutil" {
                            "Trae los parámetros del perfil con el valor en blanco. No guarda "
                            "nada todavía."
                        }
                        nav class="buscador" {
                            @for p in &perfiles {
                                a href={ (RUTA) "/" (orden.id) "?perfil=" (p.id) } { (p.nombre) }
                            }
                        }
                    }
                }
                (formulario(orden.id, &filas, resultado.as_ref(), perfil_aplicado.as_deref()))
            } @else {
                (documento(&filas, resultado.as_ref(), validado, puede))
            }
        },
    )
}

fn del_resultado_vacio(filas: &[(String, Parametro)]) -> bool {
    filas.is_empty()
}

/// El formulario de captura: una fila por parámetro más tres en blanco.
fn formulario(
    orden_id: i64,
    filas: &[(String, Parametro)],
    r: Option<&diagnostico::ResultadoLab>,
    perfil: Option<&str>,
) -> Markup {
    let total = (filas.len() + FILAS_EN_BLANCO).min(MAX_FILAS);
    html! {
        section class="tarjeta" {
            h2 { "Resultados" }
            @if let Some(nombre) = perfil {
                (aviso(
                    Tono::Ok,
                    &format!("Cargado el perfil «{nombre}». Todavía no se ha guardado nada."),
                ))
            }
            form method="post" action={ (RUTA) "/" (orden_id) "/resultado" } {
                (piezas::tabla(
                    &["Parámetro", "Valor", "Unidad", "Ref. mín.", "Ref. máx.", "Crítico"],
                    html! {
                        @for i in 0..total {
                            @let f = filas.get(i);
                            tr {
                                td {
                                    input type="text" name={ "nombre_" (i) }
                                          value=(f.map(|x| x.0.clone()).unwrap_or_default())
                                          placeholder="Hemoglobina";
                                }
                                td {
                                    input type="text" name={ "valor_" (i) }
                                          value=(f.map(|x| x.1.valor.clone()).unwrap_or_default());
                                }
                                td {
                                    input type="text" name={ "unidad_" (i) }
                                          value=(f.map(|x| x.1.unidad.clone()).unwrap_or_default());
                                }
                                td {
                                    input type="text" name={ "ref_min_" (i) }
                                          value=(f.and_then(|x| x.1.ref_min.clone()).unwrap_or_default());
                                }
                                td {
                                    input type="text" name={ "ref_max_" (i) }
                                          value=(f.and_then(|x| x.1.ref_max.clone()).unwrap_or_default());
                                }
                                td {
                                    input type="checkbox" name={ "critico_" (i) } value="1"
                                          checked[f.is_some_and(|x| x.1.critico)];
                                }
                            }
                        }
                    },
                ))
                p class="sutil" {
                    "Se guardan las filas que tengan nombre; las demás se descartan. "
                    "Guarde para pedir " (FILAS_EN_BLANCO) " filas más."
                }

                div class="apilado" {
                    label for="observaciones" { "Observaciones de quien procesa" }
                    textarea #observaciones name="observaciones" rows="3"
                        placeholder="Notas, interferencias, calidad de la muestra…" {
                        (r.and_then(|r| r.observaciones.clone()).unwrap_or_default())
                    }

                    label {
                        input type="checkbox" name="valores_criticos" value="1"
                              checked[r.is_some_and(|r| r.valores_criticos)];
                        " Contiene valores críticos que hay que notificar al médico de inmediato"
                    }
                    p class="sutil" {
                        "Marcar un parámetro como crítico enciende esta alerta por sí solo, "
                        "aunque la casilla quede sin marcar: es el servidor quien lo decide, "
                        "para que no dependa de que alguien se acuerde."
                    }

                    button type="submit" class="principal" { "Guardar resultados" }
                }
            }
        }

        // Validar va en su PROPIO formulario: son dos acciones y una de ellas no
        // se deshace. Con las dos en el mismo formulario, un «Enter» en cualquier
        // campo dispararía la primera del HTML.
        @if r.is_some() {
            section class="tarjeta" {
                h2 { "Validar" }
                p class="sutil" {
                    "Al validar, el resultado queda de SOLO LECTURA y la orden pasa a "
                    "«Completada». Sobre un resultado validado puede haberse decidido una "
                    "conducta, así que no se corrige: se emite otro. Guarde antes los cambios."
                }
                form method="post" action={ (RUTA) "/" (orden_id) "/validar" } {
                    button type="submit" class="principal"
                           data-confirmar="Pulse otra vez para validar y cerrar" {
                        "Validar y completar"
                    }
                }
            }
        }
    }
}

/// El resultado como documento: lo que ve quien no puede escribir y lo que queda
/// tras validar.
fn documento(
    filas: &[(String, Parametro)],
    r: Option<&diagnostico::ResultadoLab>,
    validado: bool,
    puede: bool,
) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Resultados" }
            @if validado {
                (aviso(Tono::Ok, "Resultado validado: es de solo lectura. Para corregirlo se emite uno nuevo."))
            } @else if !puede {
                p class="sutil" {
                    "Su cuenta no tiene el permiso para escribir resultados de laboratorio. \
                     Puede leerlos."
                }
            }
            @match r {
                None => p class="sutil" { "Esta orden todavía no tiene resultado." },
                Some(r) => {
                    @if r.valores_criticos {
                        (aviso(
                            Tono::Error,
                            "Contiene valores críticos: hay que notificarlo al médico tratante.",
                        ))
                    }
                    @if filas.is_empty() {
                        p class="sutil" { "El resultado no registra ningún parámetro." }
                    } @else {
                        (piezas::tabla(
                            &["Parámetro", "Valor", "Unidad", "Referencia", ""],
                            html! {
                                @for (nombre, p) in filas {
                                    tr {
                                        td { (nombre) }
                                        td class="mono" { (p.valor) }
                                        td class="sutil" { (p.unidad) }
                                        td class="sutil mono" { (referencia(p)) }
                                        td {
                                            @if p.critico {
                                                span class="pastilla pastilla-mal" { "crítico" }
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }
                    (piezas::datos(&[
                        ("Observaciones", r.observaciones.clone()),
                        ("Procesado por", r.bioanalista_nombre.clone()),
                        ("Fecha del resultado", Some(r.fecha_resultado.clone())),
                        ("Validado el", r.fecha_validacion.clone()),
                    ]))
                }
            }
        }
    }
}

/// El rango de referencia, con los tres casos escritos distinto. **«Sin
/// referencia» no se pinta como un rango**: un examen sin rango pactado no es un
/// examen dentro de rango.
fn referencia(p: &Parametro) -> String {
    match (p.ref_min.as_deref(), p.ref_max.as_deref()) {
        (Some(a), Some(b)) => format!("{a} – {b}"),
        (Some(a), None) => format!("≥ {a}"),
        (None, Some(b)) => format!("≤ {b}"),
        (None, None) => "—".into(),
    }
}

// ── Escrituras del resultado ─────────────────────────────────────────────────

/// Lee las filas del formulario. Los índices van en el NOMBRE del campo y no
/// como listas: una casilla sin marcar no se envía, así que `critico[]` se
/// desalinearía de las demás columnas y un valor crítico acabaría marcado en la
/// fila de al lado.
fn filas_del_formulario(campos: &HashMap<String, String>) -> Vec<(String, Parametro)> {
    let texto = |k: String| campos.get(&k).cloned().unwrap_or_default();
    let opcional = |k: String| {
        let t = texto(k);
        (!t.trim().is_empty()).then(|| t.trim().to_string())
    };
    (0..MAX_FILAS)
        .filter_map(|i| {
            let nombre = texto(format!("nombre_{i}"));
            if nombre.trim().is_empty() {
                return None;
            }
            Some((
                nombre.trim().to_string(),
                Parametro {
                    valor: texto(format!("valor_{i}")).trim().to_string(),
                    unidad: texto(format!("unidad_{i}")).trim().to_string(),
                    ref_min: opcional(format!("ref_min_{i}")),
                    ref_max: opcional(format!("ref_max_{i}")),
                    critico: campos.contains_key(&format!("critico_{i}")),
                },
            ))
        })
        .collect()
}

/// `POST /panel/laboratorio/{id}/resultado`
pub async fn guardar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirLaboratorio>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let filas = filas_del_formulario(&campos);
    if filas.is_empty() {
        // **No se guarda un resultado sin parámetros.** No es una regla nueva de
        // esta pantalla: el botón de Angular estaba deshabilitado con la lista
        // vacía. Aquí además se dice, en vez de no hacer nada.
        return de_vuelta(&format!("{RUTA}/{id}"), "sin_parametros");
    }
    let nuevo = diagnostico::ResultadoNuevo {
        resultados_json: diagnostico::json_de_parametros(&filas),
        observaciones: campos
            .get("observaciones")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        valores_criticos: campos.contains_key("valores_criticos"),
    };
    // `crear_resultado` actualiza si ya existe, igual que en imagenología:
    // decidir aquí entre crear y actualizar sería una segunda copia de esa
    // decisión.
    match diagnostico::crear_resultado(&e.pool, u.ambito(), id, u.id, &nuevo).await {
        Ok(r) => tras_escribir(&cx, id, r, "guardado"),
        Err(_) => averia(),
    }
}

/// `POST /panel/laboratorio/{id}/validar`
pub async fn validar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirLaboratorio>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match diagnostico::validar_resultado(&e.pool, u.ambito(), id, u.id).await {
        Ok(r) => tras_escribir(&cx, id, r, "validado"),
        Err(_) => averia(),
    }
}

/// `POST /panel/laboratorio/{id}/iniciar`
pub async fn iniciar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirLaboratorio>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match diagnostico::cambiar_estado_orden(&e.pool, u.ambito(), SERVICIO, id, "En Proceso").await {
        Ok(r) => tras_escribir(&cx, id, r, "iniciado"),
        Err(_) => averia(),
    }
}

fn tras_escribir<T>(cx: &Contexto, id: i64, r: Escritura<T>, bien: &str) -> Response {
    match r {
        Escritura::Hecho(_) => de_vuelta(&format!("{RUTA}/{id}"), bien),
        Escritura::Cerrado => de_vuelta(&format!("{RUTA}/{id}"), "cerrado"),
        Escritura::NoExiste => no_existe(cx),
        // El estado lo pone una constante de este módulo, no el cliente.
        Escritura::EstadoInvalido => averia(),
        // `Incompleto` es de la firma del informe radiológico; por laboratorio
        // no llega. Si llegara, es un desacuerdo entre dos capas nuestras.
        Escritura::Incompleto { .. } => averia(),
    }
}

// ── Los perfiles ─────────────────────────────────────────────────────────────

/// `GET /panel/laboratorio/perfiles`
pub async fn perfiles(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match diagnostico::listar_perfiles(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    pantalla(&cx, "Perfiles de examen", cuerpo_perfiles(&cx, &lista, f.aviso.trim(), None))
}

fn cuerpo_perfiles(
    cx: &Contexto,
    lista: &[diagnostico::Perfil],
    clave_aviso: &str,
    error: Option<&str>,
) -> Markup {
    let puede = cx.puede(Capacidad::EscribirLaboratorio);
    html! {
        (cabecera(cx, "perfiles", ""))
        @if let Some((tono, texto)) = traducir_aviso(clave_aviso) {
            (aviso(tono, texto))
        }
        section class="tarjeta" {
            p class="sutil" {
                "Un perfil es la lista de parámetros de un examen —qué se mide, en qué unidad y "
                "entre qué referencias—. Sus referencias dependen del método y del equipo, así "
                "que cada institución tiene los suyos."
            }
            @if lista.is_empty() {
                p class="sutil" { "No hay perfiles registrados." }
            } @else {
                (piezas::tabla(
                    &["Perfil", "Descripción", "Parámetros", ""],
                    html! {
                        @for p in lista {
                            tr {
                                td { (p.nombre) }
                                td class="sutil" { (p.descripcion.clone().unwrap_or_default()) }
                                td class="sutil" {
                                    @let ps = diagnostico::parametros_de_perfil(&p.examenes);
                                    @if ps.is_empty() {
                                        span class="falta" { "ninguno" }
                                    } @else {
                                        (ps.iter().map(|(n, _)| n.as_str())
                                            .collect::<Vec<_>>().join(", "))
                                    }
                                }
                                td {
                                    @if puede {
                                        form method="post"
                                             action={ (PERFILES) "/" (p.id) "/retirar" } {
                                            // Retirar NO borra: el perfil deja de
                                            // ofrecerse y los resultados que lo
                                            // usaron siguen explicándose. Aun así
                                            // pide dos pulsaciones, porque quien
                                            // lo retira no lo ve venir.
                                            button type="submit" class="enlace"
                                                    data-confirmar="Pulse otra vez para retirarlo" {
                                                "Retirar"
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
                h2 { "Nuevo perfil" }
                @if let Some(t) = error { (aviso(Tono::Error, t)) }
                form method="post" action=(PERFILES) class="apilado" {
                    label for="nombre" { "Nombre" }
                    input #nombre type="text" name="nombre" required placeholder="Hemograma";
                    label for="descripcion" { "Descripción" }
                    input #descripcion type="text" name="descripcion";
                    (piezas::tabla(
                        &["Parámetro", "Unidad", "Ref. mín.", "Ref. máx."],
                        html! {
                            @for i in 0..6 {
                                tr {
                                    td { input type="text" name={ "nombre_" (i) }; }
                                    td { input type="text" name={ "unidad_" (i) }; }
                                    td { input type="text" name={ "ref_min_" (i) }; }
                                    td { input type="text" name={ "ref_max_" (i) }; }
                                }
                            }
                        },
                    ))
                    p class="sutil" { "Las filas sin nombre se descartan." }
                    button type="submit" class="principal" { "Crear el perfil" }
                }
            }
        }
    }
}

/// `POST /panel/laboratorio/perfiles`
pub async fn crear_perfil(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirLaboratorio>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    use diagnostico::ResultadoPerfil as R;
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(institucion_id) = u.institucion_id else {
        return con_error_de_perfil(
            &e,
            &cx,
            &u,
            "Los perfiles de examen son de una institución —sus referencias dependen del método \
             y del equipo— y esta cuenta no está adscrita a ninguna.",
        )
        .await;
    };
    // Los parámetros del perfil se leen con la misma función que las filas del
    // resultado y se guardan como la LISTA que el perfil usa. El valor y la
    // marca de crítico se tiran: un perfil dice qué se mide, no qué salió.
    let examenes: Vec<serde_json::Value> = filas_del_formulario(&campos)
        .into_iter()
        .map(|(nombre, p)| {
            serde_json::json!({
                "nombre": nombre,
                "unidad": p.unidad,
                "ref_min": p.ref_min.unwrap_or_default(),
                "ref_max": p.ref_max.unwrap_or_default(),
            })
        })
        .collect();
    let nuevo = diagnostico::PerfilNuevo {
        nombre: campos.get("nombre").cloned().unwrap_or_default(),
        descripcion: campos
            .get("descripcion")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        examenes: serde_json::Value::Array(examenes),
    };
    match diagnostico::crear_perfil(&e.pool, u.ambito(), institucion_id, &nuevo).await {
        Ok(R::Creado(_)) => de_vuelta(PERFILES, "perfil_creado"),
        Ok(R::SinNombre) => {
            con_error_de_perfil(&e, &cx, &u, "El perfil necesita nombre: es como se elige.").await
        }
        Ok(R::NombreRepetido(n)) => {
            con_error_de_perfil(&e, &cx, &u, &format!("Ya hay un perfil «{n}» en esta institución."))
                .await
        }
        Ok(R::NoExiste) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// Vuelve a dibujar la pantalla de perfiles con el motivo del rechazo.
///
/// **No redirige**, porque el motivo lleva un dato dentro —el nombre repetido— y
/// el aviso de la URL es una lista cerrada: es la misma decisión que en el alta
/// de ambulancias.
async fn con_error_de_perfil(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    motivo: &str,
) -> Response {
    let lista = match diagnostico::listar_perfiles(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    (
        StatusCode::OK,
        pagina(
            "Perfiles de examen",
            Some(crate::vista::panel::menu(cx, RUTA)),
            cuerpo_perfiles(cx, &lista, "", Some(motivo)),
        ),
    )
        .into_response()
}

/// `POST /panel/laboratorio/perfiles/{id}/retirar`
pub async fn retirar_perfil(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirLaboratorio>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match diagnostico::eliminar_perfil(&e.pool, u.ambito(), id).await {
        Ok(true) => de_vuelta(PERFILES, "perfil_retirado"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
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
            "No encontrado",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró eso" }
                    p { "Puede que el enlace esté mal o que ya no exista." }
                    p { a href=(RUTA) { "Volver al laboratorio" } }
                }
            },
        ),
    )
        .into_response()
}
