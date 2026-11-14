// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Nutrición clínica: las valoraciones y su plan de alimentación.
//!
//! Port de `frontend/src/app/pages/nutricion/nutricion.component.ts` (787
//! líneas, tres vistas en un `signal<'lista'|'valoracion'|'detalle'>`).
//!
//! # El IMC ya no lo calcula la pantalla
//!
//! Y esa es la diferencia de fondo con la de Angular, que lo derivaba en el
//! navegador (`calcularIMC()`) y lo mandaba en el cuerpo del POST. Ahora lo
//! deriva el servidor, en [`medico_dominio::antropometria::imc`], de manera que
//! el peso y la talla —que se MIDEN— son lo único que esta pantalla envía. Aquí
//! el índice solo se ENSEÑA, con su categoría de la OMS.
//!
//! Eso quita de paso la petición en cada tecla que hacía el `(input)` del
//! formulario: el índice aparece al guardar, que es cuando pasa a existir.
//!
//! # El filtro por riesgo ahora filtra
//!
//! `nutricion.service.ts` mandaba `?riesgo=ALTO` y el servidor solo leía
//! `paciente_id`, así que el selector no hacía nada. Se arregló en la misma
//! pasada; esta pantalla lo usa y **valida el valor antes de consultar**: un
//! riesgo que no existe no se pasa a la consulta, porque devolvería la lista
//! vacía y «no hay ninguno» se leería como «no hay pacientes en riesgo alto».
//!
//! # Leer no pide capacidad; escribir exige `EscribirNutricion`
//!
//! Igual que las cinco rutas JSON. Y el borrado exige la misma capacidad que la
//! escritura, no una de borrado genérica: es la decisión que ya tomó el router
//! —y que llegó tarde, porque el borrado estuvo abierto a cualquier autenticado
//! mientras las cuatro escrituras hermanas ya la exigían—.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::nutricion;
use medico_dominio::antropometria;
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{Estado, EscribirNutricion};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/nutricion";

/// Los tres niveles de riesgo del producto. **No se inventan aquí**: son los que
/// el frontal ofrecía y los que la columna guarda.
const RIESGOS: [&str; 3] = ["BAJO", "MODERADO", "ALTO"];

/// Las herramientas de tamizaje que el producto ofrece.
const SCREENINGS: [&str; 4] = ["NRS-2002", "MST", "MUST", "MNA"];

const DIETAS: [(&str, &str); 10] = [
    ("NORMAL", "Normal"),
    ("BLANDA", "Blanda"),
    ("LIQUIDA", "Líquida"),
    ("HIPOCALORICA", "Hipocalórica"),
    ("HIPOPROTEICA", "Hipoproteica"),
    ("HIPERPROTEICA", "Hiperproteica"),
    ("DIABETICA", "Diabética"),
    ("SIN_GLUTEN", "Sin gluten"),
    ("SIN_LACTOSA", "Sin lactosa"),
    ("OTRA", "Otra"),
];

const VIAS: [(&str, &str); 4] = [
    ("ORAL", "Oral"),
    ("ENTERAL", "Enteral"),
    ("PARENTERAL", "Parenteral"),
    ("MIXTA", "Mixta"),
];

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "creada" => Some((Tono::Ok, "Valoración registrada.")),
        "guardada" => Some((Tono::Ok, "Cambios guardados.")),
        "borrada" => Some((
            Tono::Ok,
            "Valoración eliminada. Su plan de alimentación se fue con ella.",
        )),
        "plan_guardado" => Some((Tono::Ok, "Plan de alimentación guardado.")),
        "sin_antropometria" => Some((
            Tono::Error,
            "La valoración necesita peso y talla: el índice de masa corporal se calcula a \
             partir de ellos y no se escribe a mano.",
        )),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

// ── El listado ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    #[serde(default)]
    pub riesgo: String,
}

/// `GET /panel/nutricion`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let riesgo = f.riesgo.trim();
    // **Un riesgo que no existe NO se pasa a la consulta.** Devolvería la lista
    // vacía, y «no hay ninguno en riesgo alto» es exactamente lo que no se puede
    // decir por error en esta pantalla (directiva 20).
    if !riesgo.is_empty() && !RIESGOS.contains(&riesgo) {
        return pantalla(
            &cx,
            "Nutrición",
            html! {
                (cabecera(&cx, riesgo))
                section class="tarjeta" {
                    (aviso(Tono::Error, &format!(
                        "«{riesgo}» no es un nivel de riesgo de este producto. Los que hay son: {}.",
                        RIESGOS.join(", ")
                    )))
                }
            },
        );
    }

    let lista = match nutricion::listar_valoraciones(
        &e.pool,
        u.ambito(),
        None,
        (!riesgo.is_empty()).then_some(riesgo),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::EscribirNutricion);
    // El buscador y no el padrón entero (ver `buscador_paciente`). La primera
    // vez se pinta sin aguja: sale la primera página y su recuento.
    let (pacientes, total_pacientes) = if puede {
        match crate::vista::buscador_paciente::encontrar(&e, &u, "").await {
            Ok(x) => x,
            Err(_) => return averia(),
        }
    } else {
        (Vec::new(), 0)
    };

    pantalla(
        &cx,
        "Nutrición",
        html! {
            (cabecera(&cx, riesgo))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            @if lista.is_empty() {
                (piezas::vacio(
                    !riesgo.is_empty(),
                    "No hay valoraciones nutricionales registradas.",
                    "Ninguna valoración está en ese nivel de riesgo.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(lista.len(), lista.len(), "valoración", "valoraciones"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Documento", "Peso", "Talla", "IMC", "Tamizaje", "Riesgo"],
                    html! {
                        @for v in &lista {
                            tr {
                                td class="mono" { (v.fecha_valoracion) }
                                td { a href={ (RUTA) "/" (v.id) } { (v.paciente_nombre) } }
                                td class="mono" { (v.paciente_identificacion) }
                                td class="mono" { (format!("{:.1}", v.peso)) " kg" }
                                td class="mono" { (format!("{:.2}", v.talla)) " m" }
                                td { (indice(v.imc)) }
                                td class="sutil" {
                                    (v.screening_nutricional)
                                    @if let Some(p) = v.puntaje_screening {
                                        " · " (p)
                                    }
                                }
                                td { (piezas::estado(&v.riesgo_nutricional)) }
                            }
                        }
                    },
                ))
            }

            @if puede {
                (formulario_alta(&pacientes, total_pacientes, None))
            }
        },
    )
}

/// El índice con su categoría al lado. **La categoría no se recalcula en el
/// HTML**: sale de `antropometria::categoria_imc`, que es la misma función para
/// las dos pantallas mientras convivan.
fn indice(imc: f64) -> Markup {
    html! {
        span class="mono" { (format!("{imc:.1}")) }
        " "
        span class="sutil" { (antropometria::categoria_imc(imc)) }
    }
}

fn cabecera(cx: &Contexto, riesgo_activo: &str) -> Markup {
    html! {
        header class="titulo-seccion" {
            h1 { "Nutrición clínica" }
            nav class="buscador" {
                a class=[riesgo_activo.is_empty().then_some("enlace-activo")] href=(RUTA) {
                    "Todas"
                }
                @for r in RIESGOS {
                    a class=[(riesgo_activo == r).then_some("enlace-activo")]
                      href={ (RUTA) "?riesgo=" (r) } { "Riesgo " (r.to_lowercase()) }
                }
            }
        }
        @if !cx.puede(Capacidad::EscribirNutricion) {
            p class="sutil" { "Su cuenta puede consultar las valoraciones, no escribirlas." }
        }
    }
}

// ── El alta ──────────────────────────────────────────────────────────────────

fn formulario_alta(
    pacientes: &[medico_datos::paciente::Paciente],
    total_pacientes: i64,
    error: Option<&str>,
) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Nueva valoración" }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            @if pacientes.is_empty() && total_pacientes == 0 {
                p class="sutil" { "No hay pacientes registrados a los que valorar." }
            } @else {
                form method="post" action=(RUTA) class="apilado" {
                    (crate::vista::buscador_paciente::campo("", "", pacientes, total_pacientes))

                    label for="peso" { "Peso (kg)" }
                    input #peso type="number" name="peso" step="0.1" min="0.1" required
                          placeholder="70.0";
                    label for="talla" { "Talla (m)" }
                    input #talla type="number" name="talla" step="0.01" min="0.1" required
                          placeholder="1.70";
                    // El IMC NO es un campo: se deriva de los dos de arriba, en
                    // el servidor. Un input aquí —aunque fuera de solo lectura—
                    // volvería a sugerir que es un dato que alguien escribe.
                    p class="sutil" {
                        "El índice de masa corporal se calcula con el peso y la talla; no se "
                        "escribe."
                    }

                    label for="screening" { "Herramienta de tamizaje" }
                    select #screening name="screening_nutricional" {
                        @for s in SCREENINGS { option value=(s) { (s) } }
                    }
                    label for="puntaje" { "Puntaje del tamizaje" }
                    input #puntaje type="number" name="puntaje_screening";

                    label for="riesgo" { "Riesgo nutricional" }
                    select #riesgo name="riesgo_nutricional" {
                        @for r in RIESGOS { option value=(r) { (r) } }
                    }

                    label for="diagnostico" { "Diagnóstico nutricional" }
                    input #diagnostico type="text" name="diagnostico_nutricional" required;

                    label for="circunferencia" { "Circunferencia del brazo (cm)" }
                    input #circunferencia type="number" name="circunferencia_brazo" step="0.1";
                    label for="pliegue" { "Pliegue tricipital (mm)" }
                    input #pliegue type="number" name="pliegue_tricipital" step="0.1";
                    label for="calorico" { "Requerimiento calórico (kcal/día)" }
                    input #calorico type="number" name="requerimiento_calorico" step="1";
                    label for="proteico" { "Requerimiento proteico (g/día)" }
                    input #proteico type="number" name="requerimiento_proteico" step="0.1";
                    label for="albumina" { "Albúmina (g/dL)" }
                    input #albumina type="number" name="albumina" step="0.01";
                    label for="prealbumina" { "Prealbúmina (mg/dL)" }
                    input #prealbumina type="number" name="prealbumina" step="0.1";

                    label for="observaciones" { "Observaciones" }
                    textarea #observaciones name="observaciones" rows="2" {}

                    button type="submit" class="principal" { "Registrar la valoración" }
                }
            }
        }
    }
}

fn numero(campos: &HashMap<String, String>, k: &str) -> Option<f64> {
    campos.get(k).and_then(|v| v.trim().parse::<f64>().ok())
}

fn entero(campos: &HashMap<String, String>, k: &str) -> Option<i64> {
    campos.get(k).and_then(|v| v.trim().parse::<i64>().ok())
}

fn texto(campos: &HashMap<String, String>, k: &str) -> Option<String> {
    campos.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Un valor de una lista cerrada, o `None`. **Lo que no está en la lista no se
/// guarda**: son columnas que la pantalla compara y un valor libre las deja
/// invisibles para todos los filtros.
fn de_lista(campos: &HashMap<String, String>, k: &str, lista: &[&str]) -> Option<String> {
    texto(campos, k).filter(|v| lista.contains(&v.as_str()))
}

/// `POST /panel/nutricion`
pub async fn crear(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirNutricion>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (Some(paciente_id), Some(peso), Some(talla)) = (
        entero(&campos, "paciente_id"),
        numero(&campos, "peso"),
        numero(&campos, "talla"),
    ) else {
        return con_error(&e, &cx, &u, "Elija el paciente y escriba el peso y la talla.").await;
    };
    let nueva = nutricion::ValoracionCrear {
        paciente_id,
        hospitalizacion_id: None,
        peso,
        talla,
        // Se manda cero porque el campo sigue en el cuerpo por compatibilidad y
        // el servidor lo IGNORA: el índice lo deriva él.
        imc: 0.0,
        circunferencia_brazo: numero(&campos, "circunferencia_brazo"),
        pliegue_tricipital: numero(&campos, "pliegue_tricipital"),
        screening_nutricional: de_lista(&campos, "screening_nutricional", &SCREENINGS),
        puntaje_screening: entero(&campos, "puntaje_screening"),
        riesgo_nutricional: de_lista(&campos, "riesgo_nutricional", &RIESGOS),
        requerimiento_calorico: numero(&campos, "requerimiento_calorico"),
        requerimiento_proteico: numero(&campos, "requerimiento_proteico"),
        albumina: numero(&campos, "albumina"),
        prealbumina: numero(&campos, "prealbumina"),
        diagnostico_nutricional: texto(&campos, "diagnostico_nutricional").unwrap_or_default(),
        observaciones: texto(&campos, "observaciones"),
    };
    match nutricion::crear_valoracion(&e.pool, u.ambito(), u.id, &nueva).await {
        Ok(nutricion::Alta::Creada(_)) => de_vuelta(RUTA, "creada"),
        Ok(nutricion::Alta::SinAntropometria) => de_vuelta(RUTA, "sin_antropometria"),
        // El paciente no es de esta institución: se dibuja igual que «no
        // existe», que es lo que le corresponde saber.
        Ok(nutricion::Alta::NoVisible) => {
            con_error(&e, &cx, &u, "Ese paciente no es de esta institución.").await
        }
        Err(_) => averia(),
    }
}

async fn con_error(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    motivo: &str,
) -> Response {
    let (pacientes, total_pacientes) =
        match crate::vista::buscador_paciente::encontrar(e, u, "").await {
            Ok(x) => x,
            Err(_) => return averia(),
        };
    pantalla(
        cx,
        "Nutrición",
        html! {
            (cabecera(cx, ""))
            (formulario_alta(&pacientes, total_pacientes, Some(motivo)))
        },
    )
}

// ── La ficha ─────────────────────────────────────────────────────────────────

/// `GET /panel/nutricion/{id}`
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
    let v = match nutricion::obtener_valoracion(&e.pool, u.ambito(), id).await {
        Ok(Some(v)) => v,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let plan = match nutricion::obtener_plan(&e.pool, u.ambito(), id).await {
        Ok(p) => p,
        Err(_) => return averia(),
    };
    let puede = cx.puede(Capacidad::EscribirNutricion);

    pantalla(
        &cx,
        "Valoración nutricional",
        html! {
            (piezas::migas(RUTA, "Nutrición clínica"))
            @if let Some((tono, texto)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, texto))
            }
            header class="titulo-seccion" {
                h1 { (v.paciente_nombre) }
                (piezas::estado(&v.riesgo_nutricional))
            }

            section class="tarjeta" {
                h2 { "Antropometría" }
                div class="resumen" {
                    div {
                        span class="cifra" { (format!("{:.1}", v.peso)) }
                        span class="sutil" { "kg" }
                    }
                    div {
                        span class="cifra" { (format!("{:.2}", v.talla)) }
                        span class="sutil" { "m de talla" }
                    }
                    div {
                        span class="cifra" { (format!("{:.1}", v.imc)) }
                        span class="sutil" { (antropometria::categoria_imc(v.imc)) }
                    }
                }
                p class="sutil" {
                    "El índice se calcula con el peso y la talla de arriba; no se escribe."
                }
                (piezas::datos(&[
                    ("Documento", Some(v.paciente_identificacion.clone())),
                    ("Fecha de la valoración", Some(v.fecha_valoracion.clone())),
                    ("Nutricionista", Some(v.nutricionista_nombre.clone())),
                    ("Tamizaje", Some(v.screening_nutricional.clone())),
                    ("Puntaje", v.puntaje_screening.map(|p| p.to_string())),
                    ("Circunferencia del brazo", v.circunferencia_brazo.map(|x| format!("{x:.1} cm"))),
                    ("Pliegue tricipital", v.pliegue_tricipital.map(|x| format!("{x:.1} mm"))),
                    ("Requerimiento calórico", v.requerimiento_calorico.map(|x| format!("{x:.0} kcal/día"))),
                    ("Requerimiento proteico", v.requerimiento_proteico.map(|x| format!("{x:.1} g/día"))),
                    ("Albúmina", v.albumina.map(|x| format!("{x:.2} g/dL"))),
                    ("Prealbúmina", v.prealbumina.map(|x| format!("{x:.1} mg/dL"))),
                    ("Diagnóstico nutricional", Some(v.diagnostico_nutricional.clone())),
                    ("Observaciones", v.observaciones.clone()),
                ]))
            }

            @if puede {
                section class="tarjeta" {
                    h2 { "Corregir la antropometría" }
                    p class="sutil" {
                        "Al cambiar el peso o la talla, el índice se recalcula solo."
                    }
                    form method="post" action={ (RUTA) "/" (v.id) } class="apilado" {
                        label for="peso" { "Peso (kg)" }
                        input #peso type="number" name="peso" step="0.1" min="0.1"
                              value=(format!("{:.1}", v.peso));
                        label for="talla" { "Talla (m)" }
                        input #talla type="number" name="talla" step="0.01" min="0.1"
                              value=(format!("{:.2}", v.talla));
                        label for="riesgo" { "Riesgo nutricional" }
                        select #riesgo name="riesgo_nutricional" {
                            @for r in RIESGOS {
                                option value=(r) selected[v.riesgo_nutricional == r] { (r) }
                            }
                        }
                        label for="observaciones" { "Observaciones" }
                        textarea #observaciones name="observaciones" rows="2" {
                            (v.observaciones.clone().unwrap_or_default())
                        }
                        button type="submit" class="principal" { "Guardar" }
                    }
                }
            }

            (seccion_plan(v.id, plan.as_ref(), puede))

            @if puede {
                section class="tarjeta" {
                    h2 { "Eliminar la valoración" }
                    p class="sutil" {
                        "Se lleva su plan de alimentación por delante y no se deshace."
                    }
                    form method="post" action={ (RUTA) "/" (v.id) "/borrar" } {
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

/// El plan de alimentación: lo que hay, o el formulario para escribirlo.
fn seccion_plan(valoracion_id: i64, p: Option<&nutricion::Plan>, puede: bool) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Plan de alimentación" }
            @match p {
                None => p class="sutil" { "Esta valoración todavía no tiene plan." },
                Some(p) => {
                    (piezas::datos(&[
                        ("Tipo de dieta", Some(etiqueta_de(&DIETAS, &p.tipo_dieta))),
                        ("Vía de alimentación", Some(etiqueta_de(&VIAS, &p.via_alimentacion))),
                        ("Calorías totales", Some(format!("{:.0} kcal/día", p.calorias_totales))),
                        ("Restricciones", p.restricciones.clone()),
                        ("Suplementos", p.suplementos.clone()),
                        ("Observaciones", p.observaciones.clone()),
                        ("Creado el", Some(p.fecha_creacion.clone())),
                    ]))
                    @if !p.activo {
                        (aviso(Tono::Averia, "Este plan está inactivo."))
                    }
                }
            }
            @if puede {
                form method="post" action={ (RUTA) "/" (valoracion_id) "/plan" } class="apilado" {
                    label for="tipo_dieta" { "Tipo de dieta" }
                    select #tipo_dieta name="tipo_dieta" {
                        @for (v, n) in DIETAS {
                            option value=(v) selected[p.is_some_and(|x| x.tipo_dieta == v)] { (n) }
                        }
                    }
                    label for="via" { "Vía de alimentación" }
                    select #via name="via_alimentacion" {
                        @for (v, n) in VIAS {
                            option value=(v) selected[p.is_some_and(|x| x.via_alimentacion == v)] { (n) }
                        }
                    }
                    label for="calorias" { "Calorías totales (kcal/día)" }
                    input #calorias type="number" name="calorias_totales" step="1" min="0" required
                          value=(p.map(|x| format!("{:.0}", x.calorias_totales)).unwrap_or_default());
                    label for="restricciones" { "Restricciones alimentarias" }
                    textarea #restricciones name="restricciones" rows="2" {
                        (p.and_then(|x| x.restricciones.clone()).unwrap_or_default())
                    }
                    label for="suplementos" { "Suplementos" }
                    textarea #suplementos name="suplementos" rows="2" {
                        (p.and_then(|x| x.suplementos.clone()).unwrap_or_default())
                    }
                    label for="obs_plan" { "Observaciones del plan" }
                    textarea #obs_plan name="observaciones" rows="2" {
                        (p.and_then(|x| x.observaciones.clone()).unwrap_or_default())
                    }
                    button type="submit" class="principal" {
                        @if p.is_some() { "Guardar el plan" } @else { "Crear el plan" }
                    }
                }
            }
        }
    }
}

/// La etiqueta legible de un código de lista cerrada. **Un código que no está en
/// la lista se enseña tal cual** en vez de traducirse a nada: es un dato cierto,
/// y renombrarlo escondería que la fila trae algo que el producto no ofrece.
fn etiqueta_de(lista: &[(&str, &str)], codigo: &str) -> String {
    lista
        .iter()
        .find(|(v, _)| *v == codigo)
        .map(|(_, n)| (*n).to_string())
        .unwrap_or_else(|| codigo.to_string())
}

/// `POST /panel/nutricion/{id}` — corregir peso, talla, riesgo y observaciones.
pub async fn guardar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirNutricion>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let edit = nutricion::ValoracionEditar {
        peso: numero(&campos, "peso"),
        talla: numero(&campos, "talla"),
        // El índice no viaja: lo deriva el servidor con lo que quede.
        imc: None,
        riesgo_nutricional: de_lista(&campos, "riesgo_nutricional", &RIESGOS),
        observaciones: texto(&campos, "observaciones"),
        ..Default::default()
    };
    match nutricion::actualizar_valoracion(&e.pool, u.ambito(), id, &edit).await {
        Ok(nutricion::Edicion::Hecha) => de_vuelta(&format!("{RUTA}/{id}"), "guardada"),
        Ok(nutricion::Edicion::SinAntropometria) => {
            de_vuelta(&format!("{RUTA}/{id}"), "sin_antropometria")
        }
        Ok(nutricion::Edicion::NoExiste) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// `POST /panel/nutricion/{id}/borrar`
pub async fn borrar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirNutricion>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match nutricion::eliminar_valoracion(&e.pool, u.ambito(), id).await {
        Ok(true) => de_vuelta(RUTA, "borrada"),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// `POST /panel/nutricion/{id}/plan` — crear o actualizar.
///
/// Se decide por lo que HAY, no por un campo del formulario: preguntárselo al
/// cliente dejaría que un reenvío creara un segundo plan sobre la misma
/// valoración.
pub async fn plan(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirNutricion>,
    Path(id): Path<i64>,
    Form(campos): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    // Se guarda el PLAN entero, no un `is_some()`, porque `actualizar_plan`
    // necesita **su id** y no el de la valoración. Son dos series distintas:
    // `plan_alimentacion` tiene PK propia AUTOINCREMENT y no hay `UNIQUE` sobre
    // `valoracion_id` (migración 0018), así que divergen en cuanto una
    // valoración no tiene plan — que es el caso normal.
    //
    // Hasta el 2026-08-10 aquí se pasaba `id` (la valoración) a un parámetro que
    // se llama `plan_id`, y el `UPDATE … WHERE id = ?` **sobrescribía el plan de
    // otra valoración**, casi siempre de otro paciente: dieta, vía de
    // alimentación, calorías y restricciones, conservando el titular y el
    // nutricionista originales — o sea, sin autor. Y la pantalla contestaba
    // «plan guardado». Lo encontró la revisión de seguridad; la prueba que había
    // no lo veía porque usaba UNA valoración y UN plan, el caso degenerado en
    // que los dos ids coinciden.
    let plan_existente = match nutricion::obtener_plan(&e.pool, u.ambito(), id).await {
        Ok(p) => p,
        Err(_) => return averia(),
    };
    let calorias = numero(&campos, "calorias_totales").unwrap_or(0.0);
    let tipo = de_lista(&campos, "tipo_dieta", &DIETAS.map(|(v, _)| v));
    let via = de_lista(&campos, "via_alimentacion", &VIAS.map(|(v, _)| v));

    let hecho = if let Some(plan) = plan_existente {
        let edit = nutricion::PlanEditar {
            tipo_dieta: tipo,
            calorias_totales: Some(calorias),
            restricciones: texto(&campos, "restricciones"),
            suplementos: texto(&campos, "suplementos"),
            via_alimentacion: via,
            observaciones: texto(&campos, "observaciones"),
            ..Default::default()
        };
        // `plan.id`, no `id`. La fila ya viene validada por ámbito y por su
        // valoración padre, que es lo que hace segura esta sustitución.
        match nutricion::actualizar_plan(&e.pool, u.ambito(), plan.id, &edit).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        let nuevo = nutricion::PlanCrear {
            tipo_dieta: tipo.unwrap_or_else(|| "NORMAL".into()),
            calorias_totales: calorias,
            distribucion_calorica: serde_json::Value::Null,
            menu_diario: serde_json::Value::Null,
            restricciones: texto(&campos, "restricciones"),
            suplementos: texto(&campos, "suplementos"),
            via_alimentacion: via,
            observaciones: texto(&campos, "observaciones"),
        };
        match nutricion::crear_plan(&e.pool, u.ambito(), id, u.id, &nuevo).await {
            Ok(v) => v.is_some(),
            Err(_) => return averia(),
        }
    };
    if hecho {
        de_vuelta(&format!("{RUTA}/{id}"), "plan_guardado")
    } else {
        no_existe(&cx)
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
                    h1 { "No se encontró esa valoración" }
                    p { a href=(RUTA) { "Volver a Nutrición" } }
                }
            },
        ),
    )
        .into_response()
}
