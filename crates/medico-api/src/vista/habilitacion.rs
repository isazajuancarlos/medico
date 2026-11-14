// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Habilitación (#249): equipos, comités, infecciones e indicadores.
//!
//! Los cuatro comparten pantalla porque comparten lector: quien la abre es quien
//! prepara la visita, y lo que necesita es ver **lo que está mal** de un vistazo,
//! no cuatro listas completas.
//!
//! # Lo vencido y lo en mora van ARRIBA
//!
//! Un equipo con el mantenimiento vencido en la fila 47 de una tabla ordenada por
//! nombre es un equipo que nadie ve. Las tres secciones empiezan por el resumen
//! de lo que falla y solo después listan.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::habilitacion::{
    self, NuevaSesion, NuevoComite, NuevoEquipo, NuevoEventoIaas, NuevoMantenimiento, TIPOS_COMITE,
    TIPOS_IAAS, TIPOS_MANTENIMIENTO,
};
use medico_datos::indicadores;

use crate::auth::{AdministrarInstitucion, Autorizado, Estado};
use crate::vista::{aviso, averia, contexto, pagina, piezas, EnSesion, Tono};

pub const RUTA: &str = "/panel/habilitacion";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub desde: String,
    #[serde(default)]
    pub hasta: String,
    #[serde(default)]
    pub aviso: String,
}

fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    match clave {
        "" => None,
        "equipo" => Some((Tono::Ok, "Equipo registrado.".into())),
        "mantenimiento" => Some((
            Tono::Ok,
            "Mantenimiento registrado. El próximo se recalculó con la periodicidad del equipo."
                .into(),
        )),
        "comite" => Some((Tono::Ok, "Comité creado.".into())),
        "sesion" => Some((Tono::Ok, "Acta registrada.".into())),
        "iaas" => Some((Tono::Ok, "Evento de infección reportado.".into())),
        "notificado" => Some((Tono::Ok, "Evento notificado, con su fecha.".into())),
        otra => habilitacion::Falta::de_clave(otra).map(|f| (Tono::Error, f.mensaje().to_string())),
    }
}

fn de_vuelta(aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}?aviso={aviso}")).into_response()
}

/// `GET /panel/habilitacion`
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
    let el_aviso = texto_del_aviso(f.aviso.trim());
    let hoy = medico_dominio::zona::hoy().format("%Y-%m-%d").to_string();

    let equipos = if cx.tiene("EQUIPOS_MEDICOS") {
        match habilitacion::listar_equipos(&e.pool, u.ambito()).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };
    let comites = if cx.tiene("COMITES") {
        match habilitacion::listar_comites(&e.pool, u.ambito()).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };
    let eventos = if cx.tiene("INFECCIONES") {
        match habilitacion::listar_iaas(&e.pool, u.ambito(), None, None).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        Vec::new()
    };

    // El periodo de los indicadores: por defecto el mes en curso.
    let desde = if f.desde.trim().is_empty() {
        format!("{}-01", &hoy[..7])
    } else {
        f.desde.trim().to_string()
    };
    let hasta = if f.hasta.trim().is_empty() {
        hoy.clone()
    } else {
        f.hasta.trim().to_string()
    };
    let inds = if cx.tiene("INDICADORES") {
        match indicadores::del_periodo(&e.pool, u.ambito(), &desde, &hasta).await {
            Ok(v) => v,
            Err(_) => return averia(),
        }
    } else {
        None
    };

    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_HABILITACION,
        None,
        equipos.len() + comites.len() + eventos.len(),
        vec![],
    )
    .await;

    let vencidos = equipos.iter().filter(|x| x.vencido(&hoy) == Some(true)).count();
    let sin_cronograma = equipos.iter().filter(|x| x.sin_cronograma()).count();
    let en_mora = comites.iter().filter(|c| c.en_mora(&hoy) == Some(true)).count();
    let sin_notificar = eventos.iter().filter(|v| !v.notificado).count();

    pagina(
        "Habilitación",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Habilitación" } }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            // Lo que está mal, ARRIBA.
            section class="tarjeta resumen" {
                div { span class="cifra" { (vencidos) } span class="sutil" { "equipos vencidos" } }
                div { span class="cifra" { (sin_cronograma) } span class="sutil" { "sin cronograma" } }
                div { span class="cifra" { (en_mora) } span class="sutil" { "comités en mora" } }
                div { span class="cifra" { (sin_notificar) } span class="sutil" { "IAAS sin notificar" } }
            }

            @if cx.tiene("EQUIPOS_MEDICOS") {
                section class="tarjeta" {
                    h2 { "Equipos biomédicos" }
                    @if equipos.is_empty() {
                        (piezas::vacio(false, "No hay equipos registrados.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Equipo", "Serie", "Ubicación", "Estado", "Cada", "Próximo", "Cronograma"],
                            html! {
                                @for x in &equipos {
                                    tr {
                                        td { (x.nombre) @if let Some(m)=&x.marca { " " span class="sutil" { (m) } } }
                                        td class="mono" { (x.serie.clone().unwrap_or_default()) }
                                        td { (x.ubicacion.clone().unwrap_or_default()) }
                                        td { (piezas::estado(&x.estado)) }
                                        td class="mono" {
                                            @match x.periodicidad_meses {
                                                Some(m) => { (m) " m" }
                                                None => "",
                                            }
                                        }
                                        td class="mono" { (x.proximo_mantenimiento.clone().unwrap_or_default()) }
                                        td {
                                            // TRES estados, no dos: «no se puede
                                            // saber» es un hallazgo distinto de
                                            // «vencido», y en una visita, peor.
                                            @match x.vencido(&hoy) {
                                                Some(true) => (piezas::estado("VENCIDO")),
                                                Some(false) => (piezas::estado("AL DIA")),
                                                None => (piezas::estado("SIN CRONOGRAMA")),
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_equipo()) (formulario_mantenimiento()) }
                }
            }

            @if cx.tiene("COMITES") {
                section class="tarjeta" {
                    h2 { "Comités" }
                    @if comites.is_empty() {
                        (piezas::vacio(false, "No hay comités registrados.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Comité", "Cada", "Sesiones", "Última", "Estado"],
                            html! {
                                @for c in &comites {
                                    tr {
                                        td { (c.nombre) span class="sutil" { " " (c.tipo) } }
                                        td class="mono" {
                                            @match c.periodicidad_meses { Some(m) => { (m) " m" }, None => "" }
                                        }
                                        td class="mono" { (c.sesiones) }
                                        td class="mono" { (c.ultima_sesion.clone().unwrap_or_default()) }
                                        td {
                                            @match c.en_mora(&hoy) {
                                                Some(true) => (piezas::estado("EN MORA")),
                                                Some(false) => (piezas::estado("AL DIA")),
                                                None => (piezas::estado("SIN PERIODICIDAD")),
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_comite()) (formulario_sesion()) }
                }
            }

            @if cx.tiene("INFECCIONES") {
                section class="tarjeta" {
                    h2 { "Infecciones asociadas a la atención" }
                    @if eventos.is_empty() {
                        (piezas::vacio(false, "No hay eventos reportados.", "", RUTA))
                    } @else {
                        (piezas::tabla(
                            &["Detección", "Paciente", "Tipo", "Dispositivo", "Días", "Germen", "Notificado", ""],
                            html! {
                                @for v in &eventos {
                                    tr {
                                        td class="mono" { (v.fecha_deteccion) }
                                        td { a href={ "/panel/pacientes/" (v.paciente_id) } { (v.paciente) } }
                                        td { (v.tipo) }
                                        td { (v.dispositivo_asociado.clone().unwrap_or_default()) }
                                        td class="mono" {
                                            (v.dias_dispositivo.map(|d| d.to_string()).unwrap_or_default())
                                        }
                                        td { (v.microorganismo.clone().unwrap_or_default()) }
                                        td class="mono" {
                                            (v.fecha_notificacion.clone().unwrap_or_default())
                                        }
                                        td {
                                            @if puede && !v.notificado {
                                                form method="post" action={ (RUTA) "/" (v.id) "/notificar" } {
                                                    button type="submit" { "Notificar" }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        ))
                    }
                    @if puede { (formulario_iaas()) }
                }
            }

            @if let Some(i) = &inds {
                section class="tarjeta" {
                    h2 { "Indicadores" }
                    form method="get" action=(RUTA) class="filtros" {
                        label { "Desde" input type="date" name="desde" value=(i.desde); }
                        label { "Hasta" input type="date" name="hasta" value=(i.hasta); }
                        button type="submit" { "Calcular" }
                    }
                    (piezas::tabla(
                        &["Indicador", "Fórmula", "Numerador", "Denominador", "Valor", "Excluidos"],
                        html! {
                            @for x in &i.indicadores {
                                tr {
                                    td { (x.nombre) }
                                    td class="sutil" { (x.formula) }
                                    td class="mono" { (x.numerador) }
                                    td class="mono" { (x.denominador) }
                                    td class="mono" {
                                        // Sin denominador NO sale «0 %»: un cero se
                                        // lee como «se midió y salió perfecto».
                                        @match x.valor {
                                            Some(v) => { (format!("{v:.2}")) " " (x.unidad) }
                                            None => "no hay con qué",
                                        }
                                    }
                                    td class="mono" { (x.excluidos) }
                                }
                            }
                        },
                    ))
                    @if !i.no_calculables.is_empty() {
                        h3 { "Lo que este producto todavía no puede medir" }
                        p class="sutil" {
                            "Se enseña a propósito. Un tablero al que le faltan tres números y \
                             lo dice es más útil que uno con siete de los que tres son inventados."
                        }
                        (piezas::tabla(
                            &["Indicador", "Qué falta"],
                            html! {
                                @for n in &i.no_calculables {
                                    tr { td { (n.nombre) } td { (n.falta) } }
                                }
                            },
                        ))
                    }
                }
            }
        },
    )
    .into_response()
}

fn formulario_equipo() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/equipos" } class="apilado" {
            h3 { "Registrar un equipo" }
            label { "Nombre" input type="text" name="nombre" required; }
            label { "Marca" input type="text" name="marca"; }
            label { "Modelo" input type="text" name="modelo"; }
            label { "Serie" input type="text" name="serie"; }
            label { "Ubicación" input type="text" name="ubicacion"; }
            label { "Registro INVIMA" input type="text" name="registro_invima"; }
            label { "Fecha de adquisición" input type="date" name="fecha_adquisicion"; }
            label {
                "Periodicidad del mantenimiento (meses)"
                input type="number" name="periodicidad_meses" min="1";
                span class="sutil" { " Sin ella, el equipo sale como «sin cronograma»." }
            }
            button type="submit" { "Registrar equipo" }
        }
    }
}

fn formulario_mantenimiento() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/mantenimientos" } class="apilado" {
            h3 { "Registrar un mantenimiento" }
            label { "Equipo (id)" input type="number" name="equipo_id" required min="1"; }
            label {
                "Tipo"
                select name="tipo" required {
                    @for t in TIPOS_MANTENIMIENTO { option value=(t) { (t) } }
                }
            }
            label { "Fecha" input type="date" name="fecha" required; }
            label { "Responsable" input type="text" name="responsable" required; }
            label { input type="checkbox" name="conforme" value="1" checked; " Quedó conforme" }
            label { "Hallazgos (obligatorios si NO quedó conforme)"
                input type="text" name="hallazgos"; }
            label { "Costo" input type="number" name="costo" step="0.01" min="0"; }
            button type="submit" { "Registrar mantenimiento" }
        }
    }
}

fn formulario_comite() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/comites" } class="apilado" {
            h3 { "Crear un comité" }
            label {
                "Tipo"
                select name="tipo" required {
                    @for (t, n) in TIPOS_COMITE { option value=(t) { (n) } }
                }
            }
            label { "Nombre" input type="text" name="nombre" required; }
            label { "Sesiona cada (meses)" input type="number" name="periodicidad_meses" min="1"; }
            button type="submit" { "Crear comité" }
        }
    }
}

fn formulario_sesion() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/sesiones" } class="apilado" {
            h3 { "Registrar un acta" }
            label { "Comité (id)" input type="number" name="comite_id" required min="1"; }
            label { "Fecha" input type="date" name="fecha" required; }
            label { "Asistentes (obligatorio)" input type="text" name="asistentes" required; }
            label { "Temas" input type="text" name="temas"; }
            label { "Decisiones" input type="text" name="decisiones"; }
            button type="submit" { "Registrar acta" }
        }
    }
}

fn formulario_iaas() -> Markup {
    html! {
        form method="post" action={ (RUTA) "/infecciones" } class="apilado" {
            h3 { "Reportar una infección" }
            label { "Paciente (id)" input type="number" name="paciente_id" required min="1"; }
            label {
                "Tipo"
                select name="tipo" required {
                    @for (t, n) in TIPOS_IAAS { option value=(t) { (n) } }
                }
            }
            label { "Fecha de detección" input type="date" name="fecha_deteccion" required; }
            label {
                "Dispositivo asociado"
                input type="text" name="dispositivo_asociado";
                span class="sutil" { " Obligatorio en las asociadas a ventilador, catéter o CVC." }
            }
            label { "Días con el dispositivo" input type="number" name="dias_dispositivo" min="0"; }
            label { "Microorganismo" input type="text" name="microorganismo"; }
            label { "Desenlace" input type="text" name="desenlace"; }
            button type="submit" { "Reportar" }
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

#[derive(Deserialize)]
pub struct FormEquipo {
    nombre: String,
    #[serde(default)]
    marca: String,
    #[serde(default)]
    modelo: String,
    #[serde(default)]
    serie: String,
    #[serde(default)]
    ubicacion: String,
    #[serde(default)]
    registro_invima: String,
    #[serde(default)]
    fecha_adquisicion: String,
    #[serde(default)]
    periodicidad_meses: String,
}

/// `POST /panel/habilitacion/equipos`
pub async fn crear_equipo(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormEquipo>,
) -> Response {
    let u = autorizado.0;
    // Las cuatro pantallas de habilitación son una sola ruta y cuatro
    // licencias: la capa ve la ruta, el módulo lo decide la tabla.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "EQUIPOS_MEDICOS").await {
        return r;
    }
    let n = NuevoEquipo {
        nombre: f.nombre,
        marca: o_nada(f.marca),
        modelo: o_nada(f.modelo),
        serie: o_nada(f.serie),
        ubicacion: o_nada(f.ubicacion),
        registro_invima: o_nada(f.registro_invima),
        fecha_adquisicion: o_nada(f.fecha_adquisicion),
        periodicidad_meses: o_numero(f.periodicidad_meses),
    };
    match habilitacion::registrar_equipo(&e.pool, u.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("equipo"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormMantenimiento {
    equipo_id: i64,
    tipo: String,
    fecha: String,
    responsable: String,
    #[serde(default)]
    conforme: Option<String>,
    #[serde(default)]
    hallazgos: String,
    #[serde(default)]
    costo: String,
}

/// `POST /panel/habilitacion/mantenimientos`
pub async fn crear_mantenimiento(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormMantenimiento>,
) -> Response {
    let u = autorizado.0;
    // Las cuatro pantallas de habilitación son una sola ruta y cuatro
    // licencias: la capa ve la ruta, el módulo lo decide la tabla.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "EQUIPOS_MEDICOS").await {
        return r;
    }
    let n = NuevoMantenimiento {
        equipo_id: f.equipo_id,
        tipo: f.tipo,
        fecha: f.fecha,
        responsable: f.responsable,
        hallazgos: o_nada(f.hallazgos),
        conforme: f.conforme.is_some(),
        costo: f.costo.trim().parse().ok(),
    };
    match habilitacion::registrar_mantenimiento(&e.pool, u.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("mantenimiento"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormComite {
    tipo: String,
    nombre: String,
    #[serde(default)]
    periodicidad_meses: String,
}

/// `POST /panel/habilitacion/comites`
pub async fn crear_comite(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormComite>,
) -> Response {
    let u = autorizado.0;
    // Las cuatro pantallas de habilitación son una sola ruta y cuatro
    // licencias: la capa ve la ruta, el módulo lo decide la tabla.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "COMITES").await {
        return r;
    }
    let n = NuevoComite {
        tipo: f.tipo,
        nombre: f.nombre,
        periodicidad_meses: o_numero(f.periodicidad_meses),
    };
    match habilitacion::crear_comite(&e.pool, u.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("comite"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormSesion {
    comite_id: i64,
    fecha: String,
    asistentes: String,
    #[serde(default)]
    temas: String,
    #[serde(default)]
    decisiones: String,
}

/// `POST /panel/habilitacion/sesiones`
pub async fn crear_sesion(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormSesion>,
) -> Response {
    let u = autorizado.0;
    // Las cuatro pantallas de habilitación son una sola ruta y cuatro
    // licencias: la capa ve la ruta, el módulo lo decide la tabla.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "COMITES").await {
        return r;
    }
    let n = NuevaSesion {
        comite_id: f.comite_id,
        fecha: f.fecha,
        asistentes: f.asistentes,
        temas: o_nada(f.temas),
        decisiones: o_nada(f.decisiones),
    };
    match habilitacion::sesionar(&e.pool, u.ambito(), &n).await {
        Ok(Ok(_)) => de_vuelta("sesion"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

#[derive(Deserialize)]
pub struct FormIaas {
    paciente_id: i64,
    tipo: String,
    fecha_deteccion: String,
    #[serde(default)]
    dispositivo_asociado: String,
    #[serde(default)]
    dias_dispositivo: String,
    #[serde(default)]
    microorganismo: String,
    #[serde(default)]
    desenlace: String,
}

/// `POST /panel/habilitacion/infecciones`
pub async fn reportar_iaas(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Form(f): Form<FormIaas>,
) -> Response {
    let u = autorizado.0;
    // Un evento de infección cuelga de un PACIENTE: es dato clínico, no
    // administrativo, y su licencia es INFECCIONES.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "INFECCIONES").await {
        return r;
    }
    let n = NuevoEventoIaas {
        paciente_id: f.paciente_id,
        hospitalizacion_id: None,
        tipo: f.tipo,
        fecha_deteccion: f.fecha_deteccion,
        dispositivo_asociado: o_nada(f.dispositivo_asociado),
        dias_dispositivo: o_numero(f.dias_dispositivo),
        microorganismo: o_nada(f.microorganismo),
        desenlace: o_nada(f.desenlace),
        observaciones: None,
    };
    match habilitacion::reportar_iaas(&e.pool, u.ambito(), &n, u.id).await {
        Ok(Ok(_)) => de_vuelta("iaas"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}

/// `POST /panel/habilitacion/{id}/notificar`
pub async fn notificar(
    State(e): State<Estado>,
    autorizado: Autorizado<AdministrarInstitucion>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    // Las cuatro pantallas de habilitación son una sola ruta y cuatro
    // licencias: la capa ve la ruta, el módulo lo decide la tabla.
    if let Err(r) = crate::vista::exige_licencia(&e, &u, "INFECCIONES").await {
        return r;
    }
    match habilitacion::notificar_iaas(&e.pool, u.ambito(), id).await {
        Ok(Ok(())) => de_vuelta("notificado"),
        Ok(Err(x)) => de_vuelta(x.clave()),
        Err(_) => averia(),
    }
}
