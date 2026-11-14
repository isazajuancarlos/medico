// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Promoción y prevención (#249): los cuatro programas.
//!
//! Una pantalla con el programa como filtro, y no cuatro pantallas iguales: lo
//! que cambia entre `PRENATAL` y `CRECIMIENTO_DESARROLLO` es la población y el
//! esquema de controles, no la forma de trabajar con ellos.
//!
//! `VACUNACION` es la excepción y tiene su propia sección: no lleva controles
//! sino DOSIS, con su lote.
//!
//! # El filtro por lote está a la vista, y no escondido en un informe
//!
//! La pregunta «¿a quién le pusimos este lote?» se hace el día que hay un retiro
//! del mercado, con prisa. Si vive en un informe aparte, ese día nadie la
//! encuentra.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::pyp::{self, NuevaInscripcion, NuevaVacuna, NuevoControl, PROGRAMAS};

use crate::auth::{AtenderUrgencias, Autorizado, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/pyp";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub programa: String,
    #[serde(default)]
    pub lote: String,
    #[serde(default)]
    pub inscripcion_id: Option<i64>,
    #[serde(default)]
    pub aviso: String,
}

fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    match clave {
        "" => None,
        "inscrito" => Some((Tono::Ok, "Paciente inscrito en el programa.".into())),
        "controlado" => Some((Tono::Ok, "Control registrado.".into())),
        "egresado" => Some((Tono::Ok, "Inscripción egresada.".into())),
        "aplicada" => Some((Tono::Ok, "Dosis registrada con su lote.".into())),
        otra => pyp::Falta::de_clave(otra).map(|f| (Tono::Error, f.mensaje().to_string())),
    }
}

fn de_vuelta(programa: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?programa={programa}&aviso={aviso}")).into_response()
}

/// `GET /panel/pyp?programa=&lote=&inscripcion_id=`
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

    // Solo los programas que la institución tenga en su plan. Enseñar los cuatro
    // a quien compró uno es ofrecer lo que la API va a rechazar.
    let suyos: Vec<(&str, &str, &str)> = PROGRAMAS
        .iter()
        .copied()
        .filter(|(m, _, _)| cx.tiene(m))
        .collect();
    if suyos.is_empty() {
        return averia();
    }

    // **Un programa que no está en el plan NO ensancha el listado.** Antes esto
    // era un `.filter(...)` sobre un `Option`, y el valor no autorizado caía a
    // `None` — que en la capa de datos significaba «sin filtro de programa», o
    // sea LOS CUATRO. Fallaba abierta por la puerta por la que se quería cerrar,
    // y sin filtro en la URL hacía lo mismo: quien compró solo vacunación veía
    // las inscripciones prenatales y de salud reproductiva de la institución.
    // Ahora se enumera lo que SÍ se puede ver y se pasa como conjunto.
    let pedido = f.programa.trim();
    let programa = suyos
        .iter()
        .find(|(m, _, _)| *m == pedido)
        .map(|(m, _, _)| (*m).to_string());
    let a_listar: Vec<&str> = if pedido.is_empty() {
        suyos.iter().map(|(m, _, _)| *m).collect()
    } else {
        // Ni licenciado ni conocido: no se lista nada. Enseñar de menos es el
        // lado seguro del error (directiva 20).
        programa.iter().map(|_| pedido).collect()
    };

    let es_vacunacion = programa.as_deref() == Some(pyp::VACUNACION);

    let inscripciones = if es_vacunacion {
        Vec::new()
    } else {
        match pyp::listar_inscripciones(&e.pool, u.ambito(), &a_listar).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    };
    let lote = (!f.lote.trim().is_empty()).then(|| f.lote.trim());
    let vacunas = if cx.tiene(pyp::VACUNACION) {
        match pyp::listar_vacunas(&e.pool, u.ambito(), lote).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };
    // Los controles de una inscripción llevan hallazgos, conducta, riesgo y
    // semanas de gestación — la nota clínica del programa. La inscripción tiene
    // que ser de un programa LICENCIADO, y no basta con que sea de la
    // institución: el ámbito ya lo comprueba `controles_de`, y lo que faltaba es
    // la otra mitad. Se responde igual para «ajena», «inexistente» y «de un
    // programa que no compró»: la lista vacía no dice cuál de las tres es.
    let controles = match f.inscripcion_id {
        Some(id) => {
            let suya = match pyp::programa_de_inscripcion(&e.pool, u.ambito(), id).await {
                Ok(p) => p.is_some_and(|p| suyos.iter().any(|(m, _, _)| *m == p)),
                Err(_) => return averia(),
            };
            if suya {
                match pyp::controles_de(&e.pool, u.ambito(), id).await {
                    Ok(v) => v,
                    Err(_) => return averia(),
                }
            } else {
                Vec::new()
            }
        }
        None => Vec::new(),
    };

    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_PYP,
        programa.clone(),
        inscripciones.len() + vacunas.len(),
        vec![],
    )
    .await;

    pagina(
        "Promoción y prevención",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Promoción y prevención" } }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="filtros" {
                    label {
                        "Programa"
                        select name="programa" {
                            option value="" { "Todos" }
                            @for (m, _, nombre) in &suyos {
                                option value=(m) selected[programa.as_deref() == Some(*m)] { (nombre) }
                            }
                        }
                    }
                    button type="submit" { "Ver" }
                }
            }

            @if !es_vacunacion {
                section class="tarjeta" {
                    h2 { "Inscripciones" }
                    @if inscripciones.is_empty() {
                        (piezas::vacio(programa.is_some(), "No hay inscripciones.", "Ninguna en ese programa.", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Programa", "Paciente", "Desde", "Controles", "Último", "Estado", ""],
                            html! {
                                @for i in &inscripciones {
                                    tr {
                                        td { (pyp::nombre_de(&i.programa).unwrap_or(&i.programa)) }
                                        td {
                                            a href={ "/panel/pacientes/" (i.paciente_id) } { (i.paciente) }
                                            span class="sutil" { " " (i.identificacion) }
                                        }
                                        td class="mono" { (i.fecha_inscripcion) }
                                        // Inscrito y con CERO controles es el dato
                                        // que mide la adherencia: no se esconde.
                                        td class="mono" { (i.controles) }
                                        td class="mono" { (i.ultimo_control.clone().unwrap_or_default()) }
                                        td {
                                            (piezas::estado(&i.estado))
                                            @if let Some(m) = &i.motivo_egreso {
                                                " " span class="sutil" { (m) }
                                            }
                                        }
                                        td {
                                            a href={ (RUTA) "?programa=" (i.programa) "&inscripcion_id=" (i.id) } {
                                                "Ver controles"
                                            }
                                            @if puede && i.estado == "ACTIVA" {
                                                form method="post" action={ (RUTA) "/" (i.id) "/egresar" } {
                                                    input type="hidden" name="programa" value=(i.programa);
                                                    input type="text" name="motivo" placeholder="motivo de egreso" required;
                                                    button type="submit" { "Egresar" }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_inscripcion(&suyos)) }
                }

                @if let Some(id) = f.inscripcion_id {
                    section class="tarjeta" {
                        h2 { "Controles de la inscripción " (id) }
                        @if controles.is_empty() {
                            (piezas::vacio(false, "Inscrito y sin controles todavía.", "", RUTA))
                        } @else {
                            (piezas::tabla(
                                &["#", "Fecha", "Semanas", "Riesgo", "Hallazgos", "Conducta", "Próximo", "Profesional"],
                                html! {
                                    @for c in &controles {
                                        tr {
                                            td class="mono" { (c.numero_control) }
                                            td class="mono" { (c.fecha) }
                                            td class="mono" {
                                                (c.semanas_gestacion.map(|s| s.to_string()).unwrap_or_default())
                                            }
                                            td { (c.riesgo.clone().unwrap_or_default()) }
                                            td { (c.hallazgos.clone().unwrap_or_default()) }
                                            td { (c.conducta.clone().unwrap_or_default()) }
                                            td class="mono" { (c.proximo_control.clone().unwrap_or_default()) }
                                            td { (c.profesional) }
                                        }
                                    }
                                },
                            ))
                        }
                        @if puede { (formulario_control(id, programa.as_deref())) }
                    }
                }
            }

            @if cx.tiene(pyp::VACUNACION) {
                section class="tarjeta" {
                    h2 { "Vacunación" }
                    p class="sutil" {
                        "El lote es obligatorio: es lo que permite encontrar a quién se le \
                         aplicó un biológico el día que se retire del mercado."
                    }
                    form method="get" action=(RUTA) class="filtros" {
                        input type="hidden" name="programa" value=(f.programa);
                        label { "Buscar por lote" input type="text" name="lote" value=(f.lote); }
                        button type="submit" { "Buscar" }
                    }
                    @if vacunas.is_empty() {
                        (piezas::vacio(lote.is_some(), "No hay dosis registradas.", "Ninguna de ese lote.", RUTA))
                    } @else {
                        (piezas::recuento(vacunas.len(), vacunas.len(), "dosis", "dosis"))
                        (piezas::tabla(
                            &["Fecha", "Paciente", "Biológico", "Dosis", "Lote", "Vence", "Vía", "Aplicó"],
                            html! {
                                @for v in &vacunas {
                                    tr {
                                        td class="mono" { (v.fecha) }
                                        td {
                                            a href={ "/panel/pacientes/" (v.paciente_id) } { (v.paciente) }
                                            span class="sutil" { " " (v.identificacion) }
                                        }
                                        td { (v.biologico) }
                                        td class="mono" { (v.dosis_numero) }
                                        td class="mono" { (v.lote) }
                                        td class="mono" { (v.vencimiento_lote.clone().unwrap_or_default()) }
                                        td { (v.via.clone().unwrap_or_default()) }
                                        td { (v.aplicador) }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_vacuna()) }
                }
            }
        },
    )
    .into_response()
}

fn formulario_inscripcion(suyos: &[(&str, &str, &str)]) -> Markup {
    html! {
        form method="post" action={ (RUTA) "/inscribir" } class="apilado" {
            h3 { "Inscribir a un paciente" }
            label { "Paciente (id)" input type="number" name="paciente_id" required min="1"; }
            label {
                "Programa"
                select name="programa" required {
                    @for (m, _, nombre) in suyos { option value=(m) { (nombre) } }
                }
            }
            label { "Fecha de inscripción" input type="date" name="fecha_inscripcion" required; }
            button type="submit" { "Inscribir" }
        }
    }
}

fn formulario_control(inscripcion_id: i64, programa: Option<&str>) -> Markup {
    let es_prenatal = programa == Some(pyp::PRENATAL);
    html! {
        form method="post" action={ (RUTA) "/controlar" } class="apilado" {
            h3 { "Registrar un control" }
            input type="hidden" name="inscripcion_id" value=(inscripcion_id);
            input type="hidden" name="programa" value=(programa.unwrap_or_default());
            label { "Fecha" input type="datetime-local" name="fecha" required; }
            label {
                @if es_prenatal { "Semanas de gestación (obligatorias en prenatal)" }
                @else { "Semanas de gestación (solo en prenatal)" }
                input type="number" name="semanas_gestacion" min="1" max="45" required[es_prenatal];
            }
            label {
                "Riesgo"
                select name="riesgo" {
                    // Sin clasificar es legítimo: no siempre se decide en el
                    // primer control. Un «BAJO» por defecto sería inventarlo.
                    option value="" { "— sin clasificar" }
                    option value="BAJO" { "Bajo" }
                    option value="ALTO" { "Alto" }
                }
            }
            label { "Hallazgos" input type="text" name="hallazgos"; }
            label { "Conducta" input type="text" name="conducta"; }
            label { "Próximo control" input type="date" name="proximo_control"; }
            button type="submit" { "Registrar control" }
        }
    }
}

fn formulario_vacuna() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/aplicar" } class="apilado" {
            h3 { "Registrar una dosis" }
            label { "Paciente (id)" input type="number" name="paciente_id" required min="1"; }
            label { "Fecha" input type="datetime-local" name="fecha" required; }
            label { "Biológico" input type="text" name="biologico" required; }
            label { "Número de dosis" input type="number" name="dosis_numero" required min="1"; }
            label { "Lote (obligatorio)" input type="text" name="lote" required; }
            label { "Vencimiento del lote" input type="date" name="vencimiento_lote"; }
            label { "Vía" input type="text" name="via"; }
            label { "Sitio de aplicación" input type="text" name="sitio_aplicacion"; }
            label { "Observaciones" input type="text" name="observaciones"; }
            button type="submit" { "Registrar dosis" }
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

/// El programa del catálogo que nombra esta cadena, canonizado.
///
/// El texto del formulario no sirve para preguntar por la licencia: hay que
/// pasarlo antes por el catálogo. Un `"PRENATAL "` con un espacio, o un programa
/// inventado, tienen que dejar de ser una cadena y pasar a ser un módulo o nada.
fn del_catalogo(s: &str) -> Option<&'static str> {
    PROGRAMAS.iter().find(|(m, _, _)| *m == s.trim()).map(|(m, _, _)| *m)
}

#[derive(Deserialize)]
pub struct FormInscripcion {
    paciente_id: i64,
    programa: String,
    fecha_inscripcion: String,
}

/// `POST /panel/pyp/inscribir`
pub async fn inscribir(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<FormInscripcion>,
) -> Response {
    let u = autorizado.0;
    let programa = f.programa.clone();
    // El módulo de esta escritura ES el programa. La capa de licencia no puede
    // saberlo: las cuatro filas de `PANTALLAS` apuntan a `/panel/pyp`, así que
    // cualquiera de las cuatro abre la ruta — para LEER es lo correcto, para
    // inscribir no. Se canoniza contra el catálogo antes de preguntar.
    let Some(modulo) = del_catalogo(&programa) else {
        return de_vuelta(&programa, pyp::Falta::ProgramaDesconocido.clave());
    };
    if let Err(r) = crate::vista::exige_licencia(&e, &u, modulo).await {
        return r;
    }
    let n = NuevaInscripcion {
        paciente_id: f.paciente_id,
        programa: f.programa,
        fecha_inscripcion: f.fecha_inscripcion,
    };
    match pyp::inscribir(&e.pool, u.ambito(), &n, Some(u.id)).await {
        Ok(Ok(_)) => de_vuelta(&programa, "inscrito"),
        Ok(Err(x)) => de_vuelta(&programa, x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormControl {
    inscripcion_id: i64,
    #[serde(default)]
    programa: String,
    fecha: String,
    #[serde(default)]
    semanas_gestacion: String,
    #[serde(default)]
    riesgo: String,
    #[serde(default)]
    hallazgos: String,
    #[serde(default)]
    conducta: String,
    #[serde(default)]
    proximo_control: String,
}

/// `POST /panel/pyp/controlar`
pub async fn controlar(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<FormControl>,
) -> Response {
    let u = autorizado.0;
    let programa = f.programa.clone();
    // El módulo lo decide el programa de la INSCRIPCIÓN, no el campo del
    // formulario —que solo sirve para volver a la pantalla y lo escribe el
    // cliente—. Una inscripción ajena o inexistente se responde igual.
    let real = match pyp::programa_de_inscripcion(&e.pool, u.ambito(), f.inscripcion_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return de_vuelta(&programa, pyp::Falta::InscripcionInexistente.clave()),
        Err(_) => return averia(),
    };
    let Some(modulo) = del_catalogo(&real) else {
        return averia();
    };
    if let Err(r) = crate::vista::exige_licencia(&e, &u, modulo).await {
        return r;
    }
    let n = NuevoControl {
        inscripcion_id: f.inscripcion_id,
        fecha: f.fecha,
        semanas_gestacion: o_numero(f.semanas_gestacion),
        riesgo: o_nada(f.riesgo),
        hallazgos: o_nada(f.hallazgos),
        conducta: o_nada(f.conducta),
        proximo_control: o_nada(f.proximo_control),
    };
    match pyp::controlar(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta(&programa, "controlado"),
        Ok(Err(x)) => de_vuelta(&programa, x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormEgreso {
    #[serde(default)]
    programa: String,
    motivo: String,
}

/// `POST /panel/pyp/{id}/egresar`
pub async fn egresar(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Path(id): Path<i64>,
    Form(f): Form<FormEgreso>,
) -> Response {
    let u = autorizado.0;
    // `es_del_programa`, abajo, ata el campo del formulario a la inscripción;
    // esto ata el campo a la LICENCIA. Hacen falta los dos: sin el primero se
    // egresa el programa de otro, sin el segundo se egresa un programa que no
    // se compró.
    let Some(modulo) = del_catalogo(&f.programa) else {
        return de_vuelta(&f.programa, pyp::Falta::ProgramaDesconocido.clave());
    };
    if let Err(r) = crate::vista::exige_licencia(&e, &u, modulo).await {
        return r;
    }
    // El programa viene del formulario, así que se COMPRUEBA contra la
    // inscripción antes de tocarla: si no coincide, se responde como si la
    // inscripción no existiera. La pantalla vive bajo `/panel/pyp`, que las
    // cuatro licencias abren, y sin esto una de ellas egresaría el programa de
    // otra.
    match pyp::es_del_programa(&e.pool, u.ambito(), id, &f.programa).await {
        Ok(false) => return de_vuelta(&f.programa, pyp::Falta::InscripcionInexistente.clave()),
        Err(_) => return averia(),
        Ok(true) => {}
    }
    match pyp::egresar(&e.pool, u.ambito(), id, &f.motivo).await {
        Ok(Ok(())) => de_vuelta(&f.programa, "egresado"),
        Ok(Err(x)) => de_vuelta(&f.programa, x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormVacuna {
    paciente_id: i64,
    fecha: String,
    biologico: String,
    dosis_numero: i64,
    lote: String,
    #[serde(default)]
    vencimiento_lote: String,
    #[serde(default)]
    via: String,
    #[serde(default)]
    sitio_aplicacion: String,
    #[serde(default)]
    observaciones: String,
}

/// `POST /panel/pyp/aplicar`
pub async fn aplicar(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<FormVacuna>,
) -> Response {
    let u = autorizado.0;
    // La dosis es siempre del módulo de vacunación, comparta pantalla con quien
    // comparta.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, pyp::VACUNACION).await {
        return r;
    }
    let n = NuevaVacuna {
        paciente_id: f.paciente_id,
        inscripcion_id: None,
        fecha: f.fecha,
        biologico: f.biologico,
        dosis_numero: f.dosis_numero,
        lote: f.lote,
        vencimiento_lote: o_nada(f.vencimiento_lote),
        via: o_nada(f.via),
        sitio_aplicacion: o_nada(f.sitio_aplicacion),
        observaciones: o_nada(f.observaciones),
    };
    match pyp::aplicar(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta(pyp::VACUNACION, "aplicada"),
        Ok(Err(x)) => de_vuelta(pyp::VACUNACION, x.clave()),
        Err(_) => averia(),
    }
}
