// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **La pantalla del `nilknown`** (#603 F4.5-C / piezas 2-3): preparar el RDA de un
//! ENCUENTRO —consulta externa, hospitalización o urgencias— y avalar sus secciones
//! vacías.
//!
//! El RDA (Resumen Digital de Atención) que viaja al visor nacional emite una
//! sección vacía con `emptyReason=nilknown` —«revisado, sin nada conocido»— SOLO si
//! un clínico lo confirma (la puerta del #603). Esta pantalla es donde lo confirma:
//! muestra, para un encuentro, qué secciones quedan vacías y por avalar (casillas),
//! qué OTROS datos faltan (informativo, se capturan en otra parte) y si el RDA ya
//! está listo.
//!
//! # Una sola pantalla, tres encuentros (directiva 9)
//!
//! La ruta es `/panel/rda/{tipo}/{id}` con `tipo ∈ {consulta, hospitalizacion,
//! urgencias}`. Todo el flujo (estado, avalar, emitir) es genérico por `(tipo, id)`:
//! el despacho al productor correcto vive en `rda_encuentro`. Lo único propio de
//! cada tipo es de dónde se leen los datos del ENCABEZADO (qué tabla), que resuelve
//! [`encabezado`].
//!
//! # Patrón de la casa, no htmx
//!
//! `form method="post"` + redirección 303 con `?aviso=`, como las demás pantallas
//! que escriben. La cookie de sesión es `SameSite=Strict`: un formulario de otro
//! sitio llega sin sesión, no hay token CSRF que añadir.
//!
//! # Lo que NO se confía del formulario
//!
//! - El **usuario** al que se atribuye el aval sale de la SESIÓN (`u.id`), nunca del
//!   formulario (observación del concilio F4.5 A+B).
//! - Los **códigos** marcados se filtran server-side contra las secciones realmente
//!   avalables de ESTE encuentro: una casilla forjada con un código ajeno no escribe
//!   nada. La barrera de fondo vive además en `rda_aval` (aislamiento del encuentro
//!   y del avalador).
//!
//! # Des-avalar es seguro
//!
//! La casilla refleja el estado; enviar FIJA el conjunto a lo marcado
//! (`rda_aval::fijar_avales`). Desmarcar solo devuelve la sección a «sin confirmar»
//! → el RDA no se emite hasta reavalarla, el lado conservador (nunca un `nilknown`
//! falso).

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use crate::auth::{Estado, EscribirClinico};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesionCon, Tono};
use medico_datos::rda_encuentro::{
    OPERACION_CONSULTA_EXTERNA, OPERACION_HOSPITALIZACION, OPERACION_URGENCIAS,
};

const RUTA: &str = "/panel/rda";
const RUTA_HISTORIAS: &str = "/panel/historias";

/// Traduce el SEGMENTO de la URL (`consulta`|`hospitalizacion`|`urgencias`, que es
/// lo que teclea/enlaza la pantalla) a la CONSTANTE DE OPERACIÓN que usan
/// `rda_encuentro`, `rda_aval` y el transporte (`consulta-externa`|…). Son dos
/// vocabularios: la URL es amable, la operación es canónica. `None` para cualquier
/// otro segmento — la pantalla responde 404, no adivina (directiva 20).
///
/// La URL de consulta se dejó en `consulta` (no `consulta-externa`) porque es la que
/// enlaza la historia desde antes de estas piezas; cambiarla rompería ese enlace.
fn segmento_a_operacion(segmento: &str) -> Option<&'static str> {
    match segmento {
        "consulta" => Some(OPERACION_CONSULTA_EXTERNA),
        "hospitalizacion" => Some(OPERACION_HOSPITALIZACION),
        "urgencias" => Some(OPERACION_URGENCIAS),
        _ => None,
    }
}

/// El nombre legible del tipo (por su constante de operación), para el título.
fn nombre_tipo(operacion: &str) -> &'static str {
    match operacion {
        OPERACION_HOSPITALIZACION => "Hospitalización",
        OPERACION_URGENCIAS => "Urgencias",
        _ => "Consulta externa",
    }
}

/// Construye la URL con el SEGMENTO (no la operación): así el enlace de vuelta casa
/// con la ruta registrada.
fn ruta(segmento: &str, id: i64) -> String {
    format!("{RUTA}/{segmento}/{id}")
}

fn de_vuelta(segmento: &str, id: i64, aviso_clave: &str) -> Response {
    Redirect::to(&format!("{}?aviso={aviso_clave}", ruta(segmento, id))).into_response()
}

/// Los avisos, traducidos por lista cerrada: el valor llega del cliente, así que lo
/// que no esté aquí no dice nada (directiva 23).
fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "avalado" => Some((Tono::Ok, "Aval actualizado. Las secciones marcadas quedan confirmadas «sin hallazgos».")),
        "emitido" => Some((
            Tono::Ok,
            "El RDA quedó encolado para transmitirse al visor nacional. El envío ocurre en segundo plano, con reintentos.",
        )),
        "faltan_para_emitir" => Some((
            Tono::Error,
            "El RDA no se pudo emitir: aún faltan datos o secciones por confirmar.",
        )),
        "ihce_no_configurado" => Some((
            Tono::Error,
            "El transporte al mecanismo IHCE no está configurado en este despliegue: el RDA no se puede emitir todavía.",
        )),
        "sin_institucion" => Some((
            Tono::Error,
            "El RDA lo emite una institución prestadora; esta cuenta no tiene una asignada.",
        )),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct Visto {
    #[serde(default)]
    aviso: String,
}

fn pantalla(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA_HISTORIAS)), cuerpo).into_response()
}

/// El 404, igual para «no existe» y «es de otra institución»: distinguirlos diría
/// qué hay al otro lado.
fn no_existe(cx: &Contexto) -> Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        pagina(
            "No encontrada",
            Some(crate::vista::panel::menu(cx, RUTA_HISTORIAS)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró ese encuentro" }
                    p { "Puede que el enlace esté mal o que el encuentro no exista." }
                    p { a href=(RUTA_HISTORIAS) { "Volver a Historias clínicas" } }
                }
            },
        ),
    )
        .into_response()
}

/// El encabezado del encuentro para la pantalla: su `paciente_id` (para el eslabón
/// de acceso y el 404) y las filas de datos a mostrar. Es lo ÚNICO propio de cada
/// tipo; lee la tabla correspondiente con el ámbito. `Ok(None)` = no existe o es de
/// otra sede (404 indistinguible).
struct Encabezado {
    paciente_id: i64,
    filas: Vec<(&'static str, Option<String>)>,
}

async fn encabezado(
    e: &Estado,
    ambito: medico_dominio::Ambito,
    tipo: &str,
    id: i64,
) -> Result<Option<Encabezado>, sqlx::Error> {
    match tipo {
        OPERACION_HOSPITALIZACION => {
            let Some(h) = medico_datos::hospitalizacion::obtener(&e.pool, ambito, id).await? else {
                return Ok(None);
            };
            Ok(Some(Encabezado {
                paciente_id: h.paciente_id,
                filas: vec![
                    ("Ingreso", Some(h.fecha_ingreso)),
                    ("Egreso", h.fecha_egreso),
                    ("Cama / servicio", Some(format!("{} · {}", h.cama_numero, h.cama_servicio))),
                    ("Dx. ingreso", h.diagnostico_ingreso),
                    ("Dx. egreso", h.diagnostico_egreso),
                ],
            }))
        }
        OPERACION_URGENCIAS => {
            let Some(u) = medico_datos::urgencias::obtener(&e.pool, ambito, id).await? else {
                return Ok(None);
            };
            Ok(Some(Encabezado {
                paciente_id: u.paciente_id,
                filas: vec![
                    ("Triage", Some(u.triage)),
                    ("Motivo", Some(u.motivo_consulta)),
                    ("Ingreso", Some(u.fecha_ingreso)),
                    ("Destino", u.destino_salida),
                    ("Dx. ingreso", u.diagnostico_ingreso),
                ],
            }))
        }
        // Consulta externa (el defecto validado por `tipo_valido`).
        _ => {
            let Some(h) = medico_datos::historia::obtener(&e.pool, ambito, id).await? else {
                return Ok(None);
            };
            Ok(Some(Encabezado {
                paciente_id: h.paciente_id,
                filas: vec![
                    ("Historia", Some(h.codigo_historia)),
                    ("Fecha", Some(h.fecha)),
                    ("Diagnóstico principal", Some(h.diagnostico_principal)),
                ],
            }))
        }
    }
}

/// `GET /panel/rda/{tipo}/{id}` — la preparación del RDA del encuentro.
pub async fn preparar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path((tipo, id)): Path<(String, i64)>,
    Query(v): Query<Visto>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(op) = segmento_a_operacion(&tipo) else {
        return no_existe(&cx);
    };
    // El RDA lo emite una institución; el superadmin (sin institución) no prepara.
    let Some(institucion_id) = u.institucion_id else {
        return pantalla(
            &cx,
            "Preparar RDA",
            html! {
                (piezas::migas(RUTA_HISTORIAS, "Historias clínicas"))
                section class="tarjeta estrecha" {
                    h1 { "Sin institución" }
                    p { "El RDA lo emite una institución prestadora; esta cuenta no tiene una asignada." }
                }
            },
        );
    };

    let enc = match encabezado(&e, u.ambito(), op, id).await {
        Ok(Some(x)) => x,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let paciente = match medico_datos::paciente::obtener(&e.pool, u.ambito(), enc.paciente_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };

    let estado = match medico_datos::rda_encuentro::estado_de_preparacion(
        &e.pool,
        institucion_id,
        op,
        id,
        &e.entorno_atencion,
    )
    .await
    {
        Ok(s) => s,
        Err(_) => return averia(),
    };

    // Abrir la preparación LEE el encuentro del paciente: deja el eslabón.
    crate::rutas::registrar_acceso(
        &e,
        &u,
        &format!("preparar_rda:{op}"),
        crate::rutas::RECURSO_AVAL_RDA,
        id,
        enc.paciente_id,
    )
    .await;

    let nombre_paciente = format!("{} {}", paciente.nombre1, paciente.apellido1);
    let mut datos = vec![
        ("Paciente", Some(nombre_paciente)),
        ("Documento", Some(format!("{} {}", paciente.tipo_documento, paciente.numero_documento))),
    ];
    datos.extend(enc.filas.iter().map(|(k, v)| (*k, v.clone())));

    pantalla(
        &cx,
        "Preparar RDA",
        html! {
            (piezas::migas(RUTA_HISTORIAS, "Historias clínicas"))
            @if let Some((tono, texto)) = traducir_aviso(v.aviso.trim()) {
                (aviso(tono, texto))
            }

            header class="titulo-seccion" {
                h1 { "Preparar RDA — " (nombre_tipo(op)) }
            }

            section class="tarjeta" {
                (piezas::datos(&datos))
            }

            // El estado general.
            section class="tarjeta" {
                @if estado.listo {
                    (aviso(Tono::Ok, "El RDA está listo para transmitir: todas las secciones revisadas y sin otras faltas."))
                } @else {
                    p { "El RDA aún no está listo. Confirme las secciones vacías revisadas y complete los datos que faltan." }
                }
            }

            // Transmitir al visor nacional (#603 F4.6). Avalar NO es enviar: emitir
            // es un acto aparte —transmite datos clínicos al Estado—, y solo se
            // ofrece cuando el transporte IHCE está configurado.
            section class="tarjeta" {
                h2 { "Transmitir al visor nacional" }
                @if !e.ihce_configurado {
                    p class="ayuda" {
                        "El transporte al mecanismo IHCE no está configurado en este despliegue "
                        "(faltan las credenciales del prestador), así que el RDA todavía no se puede emitir."
                    }
                } @else if estado.listo {
                    form method="post" action=(format!("{}/emitir", ruta(&tipo, id))) {
                        p class="ayuda" {
                            "El RDA está listo. Al emitir se encola para transmitirse al visor nacional; "
                            "el envío ocurre en segundo plano, con reintentos."
                        }
                        div class="acciones" {
                            button type="submit" class="primario" { "Emitir RDA" }
                        }
                    }
                } @else {
                    p class="ayuda" {
                        "El RDA se podrá emitir cuando esté listo: confirme las secciones y complete los datos que faltan."
                    }
                }
            }

            // Las secciones avalables, como casillas.
            section class="tarjeta" {
                h2 { "Secciones vacías por confirmar" }
                p class="ayuda" {
                    "Marque cada sección que revisó y en la que el paciente no tiene nada conocido que reportar. "
                    "Solo así el RDA la declara «sin hallazgos» (nunca por defecto)."
                }
                @if estado.secciones.is_empty() {
                    p { "No hay secciones vacías por confirmar en este encuentro." }
                } @else {
                    form method="post" action=(format!("{}/avalar", ruta(&tipo, id))) {
                        @for s in &estado.secciones {
                            label class="casilla" {
                                input type="checkbox" name="seccion" value=(s.codigo) checked[s.avalada];
                                " " (s.titulo)
                            }
                        }
                        div class="acciones" {
                            button type="submit" class="primario" { "Guardar aval" }
                        }
                    }
                }
            }

            // Las otras faltas, informativas.
            @if !estado.otras_faltas.is_empty() {
                section class="tarjeta" {
                    h2 { "Otros datos que faltan" }
                    p class="ayuda" { "Estos NO se avalan aquí: se capturan en su propia pantalla." }
                    ul {
                        @for f in &estado.otras_faltas {
                            li { (f) }
                        }
                    }
                }
            }
        },
    )
}

/// `POST /panel/rda/{tipo}/{id}/avalar` — fija el aval a lo marcado.
pub async fn avalar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path((tipo, id)): Path<(String, i64)>,
    Form(campos): Form<Vec<(String, String)>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(op) = segmento_a_operacion(&tipo) else {
        return no_existe(&cx);
    };
    let Some(institucion_id) = u.institucion_id else {
        return de_vuelta(&tipo, id, "sin_institucion");
    };

    // El paciente_id (para el eslabón) sale del encuentro, con el ámbito. Lo ajeno
    // es 404 —no se avala el encuentro de otra sede—.
    let enc = match encabezado(&e, u.ambito(), op, id).await {
        Ok(Some(x)) => x,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };

    // Las secciones realmente AVALABLES de este encuentro. El formulario no se
    // cree: solo se avala lo que el productor declara vacío y avalable, así que un
    // código forjado no escribe nada (observación O3 del concilio).
    let estado = match medico_datos::rda_encuentro::estado_de_preparacion(
        &e.pool,
        institucion_id,
        op,
        id,
        &e.entorno_atencion,
    )
    .await
    {
        Ok(s) => s,
        Err(_) => return averia(),
    };
    let avalables: std::collections::HashSet<&str> =
        estado.secciones.iter().map(|s| s.codigo.as_str()).collect();
    let marcadas: Vec<&str> = campos
        .iter()
        .filter(|(k, _)| k == "seccion")
        .map(|(_, v)| v.as_str())
        .filter(|c| avalables.contains(c))
        .collect();

    let sello = medico_dominio::zona::ahora().to_rfc3339();
    match medico_datos::rda_aval::fijar_avales(
        &e.pool,
        u.ambito(),
        u.id,
        op,
        id,
        &sello,
        &marcadas,
    )
    .await
    {
        Ok(()) => {}
        // El aislamiento de encuentro/avalador ya lo cubrió la lectura de arriba; un
        // error aquí es de base.
        Err(_) => return averia(),
    }

    crate::rutas::registrar_acceso(
        &e,
        &u,
        &format!("avalar_rda:{op}"),
        crate::rutas::RECURSO_AVAL_RDA,
        id,
        enc.paciente_id,
    )
    .await;

    de_vuelta(&tipo, id, "avalado")
}

/// `POST /panel/rda/{tipo}/{id}/emitir` — EMITE el RDA del encuentro (#603 F4.6):
/// si está listo, lo encola para transmitirse al mecanismo IHCE.
///
/// Es un acto SEPARADO de avalar: avalar atesta secciones vacías; emitir transmite
/// datos clínicos al visor nacional. Por eso:
///
/// - **Se cierra si el transporte no está configurado** (`e.ihce_configurado`):
///   encolar un Bundle que la tarea de fondo nunca drenaría sería un falso
///   «enviado» (directiva 20). El usuario recibe el motivo, no un éxito mudo.
/// - **Solo encola lo `Listo`**: la puerta la aplica `preparar_y_encolar`, que ante
///   `Faltan` no escribe nada. Aquí se traduce a un aviso.
///
/// No transmite en línea: la tarea de fondo `ihce::correr_cola` drena la cola con
/// reintentos. El envío que llegó a la plataforma deja su eslabón en `acceso`.
pub async fn emitir(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path((tipo, id)): Path<(String, i64)>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(op) = segmento_a_operacion(&tipo) else {
        return no_existe(&cx);
    };
    let Some(institucion_id) = u.institucion_id else {
        return de_vuelta(&tipo, id, "sin_institucion");
    };

    // La puerta del transporte: sin credenciales no hay a dónde enviar, así que no
    // se encola —se dice—. Antes de tocar el encuentro: si no se puede emitir, da
    // igual de quién sea.
    if !e.ihce_configurado {
        return de_vuelta(&tipo, id, "ihce_no_configurado");
    }

    // El paciente_id (para el eslabón) sale del encuentro, con el ámbito. Lo ajeno
    // es 404 —no se emite el RDA de otra sede—.
    let enc = match encabezado(&e, u.ambito(), op, id).await {
        Ok(Some(x)) => x,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };

    match medico_datos::rda_encuentro::preparar_y_encolar(
        &e.pool,
        institucion_id,
        op,
        id,
        &e.entorno_atencion,
        chrono::Utc::now(),
    )
    .await
    {
        Ok(medico_datos::rda_encuentro::Emision::Encolado(_)) => {
            // El RDA salió hacia la plataforma nacional: deja el eslabón (Ley 1581).
            crate::rutas::registrar_acceso(
                &e,
                &u,
                &format!("emitir_rda:{op}"),
                crate::rutas::RECURSO_AVAL_RDA,
                id,
                enc.paciente_id,
            )
            .await;
            de_vuelta(&tipo, id, "emitido")
        }
        Ok(medico_datos::rda_encuentro::Emision::Faltan(_)) => {
            de_vuelta(&tipo, id, "faltan_para_emitir")
        }
        Err(_) => averia(),
    }
}
