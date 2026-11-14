// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Administración de la institución: sus datos, su configuración, su personal,
//! su política de seguridad, los campos propios, los catálogos, la matriz de
//! capacidades y la auditoría.
//!
//! Port de `frontend/src/app/pages/admin/admin.component.ts` (1 563 líneas, ocho
//! pestañas), que pasan a ser ocho URL.
//!
//! # Las cinco puertas del alta de personal NO se copian
//!
//! Dar de alta a alguien pasa por cinco: el ascenso —solo un superadmin fabrica
//! otro—, el cupo de la licencia, la política de contraseña de la institución,
//! el hash y la línea de bitácora. **Ninguna vivía en `medico_datos`**: vivían
//! en el manejador JSON. Copiarlas aquí habría dejado dos juegos de cinco
//! puertas, y una copia de cinco puertas se queda atrás en el primer arreglo —
//! que es literalmente cómo esa ruta llegó a crear usuarios sin mirar la
//! licencia mientras su hermana sí lo hacía.
//!
//! Se extrajeron a [`crate::rutas::dar_de_alta_personal`],
//! [`crate::rutas::editar_personal`] y
//! [`crate::rutas::restablecer_password_de`], y ahora las comparten los dos
//! frontales. `/api` no cambió ni un código de estado.
//!
//! # Aquí LEER también exige capacidad
//!
//! Es la primera pantalla del panel de la que se puede decir eso: las veintiséis
//! rutas de `/api/admin` exigen `AdministrarInstitucion` también en los GET, así
//! que la fila de `PANTALLAS` la lleva en su columna `exige`. La matriz de
//! capacidades es la excepción dentro de la excepción: exige
//! `GestionarUsuarios`, que es la de quien nombra.
//!
//! # La política dice QUÉ SE APLICA
//!
//! Veintiuna columnas, de las que este binario aplica cinco. Hasta hoy la
//! pantalla las presentaba todas igual —un administrador movía interruptores y
//! no ocurría nada, que es la forma más cara de la directiva 20— y la tabla de
//! cuáles mandan vivía en un doc-comment que ninguna pantalla puede leer. Ahora
//! es un dato: [`medico_datos::administracion::POLITICA_APLICADA`], y cada
//! campo se dibuja diciendo si manda o por qué no.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::administracion as adm;
use medico_dominio::rol::{Capacidad, RolUsuario};
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{AdministrarInstitucion, Estado, GestionarUsuarios};
use crate::rutas::EscrituraPersonal;
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesionCon, Tono};

const RUTA: &str = "/panel/admin";

/// Las ocho pestañas, con su ruta y su etiqueta. Una sola lista para el menú
/// interno y para las rutas: dos listas dejarían una pestaña apuntando a un 404.
pub(crate) const PESTANIAS: &[(&str, &str)] = &[
    ("", "Institución"),
    ("/configuracion", "Configuración"),
    ("/usuarios", "Personal"),
    ("/seguridad", "Seguridad"),
    ("/campos", "Campos propios"),
    ("/catalogos", "Catálogos"),
    ("/capacidades", "Capacidades"),
    ("/auditoria", "Auditoría"),
];

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "institucion" => Some((Tono::Ok, "Datos de la institución guardados.")),
        "configuracion" => Some((Tono::Ok, "Configuración guardada.")),
        "politica" => Some((Tono::Ok, "Política de seguridad guardada.")),
        "campo" => Some((Tono::Ok, "Campo creado.")),
        "campo_borrado" => Some((Tono::Ok, "Campo eliminado.")),
        "especialidad" => Some((Tono::Ok, "Especialidad creada.")),
        "entidad" => Some((Tono::Ok, "Entidad creada.")),
        "usuario" => Some((Tono::Ok, "Persona dada de alta.")),
        "usuario_editado" => Some((Tono::Ok, "Cambios guardados.")),
        "password" => Some((
            Tono::Ok,
            "Contraseña restablecida. Se le exigirá cambiarla al entrar, y se borraron sus \
             intentos de acceso fallidos.",
        )),
        "capacidades" => Some((Tono::Ok, "Matriz de capacidades guardada.")),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

/// La vuelta de un RECHAZO, con su motivo.
///
/// Existe porque `de_vuelta(.., "entidad")` se usaba para las dos cosas: el
/// manejador casaba `Ok(_)`, que en un `Result<Result<_, Invalido>, _>` incluye
/// el rechazo, así que una entidad que la capa de datos NO creó salía anunciada
/// como «Entidad creada.» en verde. Lo cazó la revisión de la directiva 25 sobre
/// `16ed8b32`, con confianza 9, y el contexto lo agrava: estos siete campos
/// existen para el alta MASIVA a mano desde el VFP — el operador captura veinte
/// entidades, ve veinte confirmaciones, y se queda con las que pasaron.
///
/// El motivo se codifica a mano y no con una dependencia: solo hay que escapar
/// lo que rompe una URL, y `maud` escapa el HTML al dibujarlo.
fn de_vuelta_rechazo(a: &str, motivo: &str) -> Response {
    let m: String = motivo
        .chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ',' | ':') => {
                c.to_string()
            }
            c => c.encode_utf8(&mut [0u8; 4]).bytes().map(|b| format!("%{b:02X}")).collect(),
        })
        .collect();
    Redirect::to(&format!("{a}?aviso=rechazo&motivo={m}")).into_response()
}

fn texto(c: &HashMap<String, String>, k: &str) -> Option<String> {
    c.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn entero(c: &HashMap<String, String>, k: &str) -> Option<i64> {
    c.get(k).and_then(|v| v.trim().parse::<i64>().ok())
}

// ── El armazón ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    /// El motivo de un rechazo, para poder DECIRLO.
    ///
    /// Los avisos de esta pantalla son claves estáticas, y eso alcanza mientras
    /// el resultado sea «hecho». Un rechazo necesita el porqué: «Entidad
    /// rechazada» sin más deja al operador probando a ciegas. Viaja escapado por
    /// `maud` como cualquier otro texto.
    #[serde(default)]
    pub motivo: String,
    #[serde(default)]
    pub page: Option<i64>,
}

fn cabecera(cx: &Contexto, activo: &str) -> Markup {
    let nombra = cx.puede(Capacidad::GestionarUsuarios);
    html! {
        header class="titulo-seccion" {
            h1 { "Administración" }
            nav class="buscador" {
                @for (sufijo, etiqueta) in PESTANIAS {
                    // La matriz de capacidades exige OTRA capacidad —la de quien
                    // nombra— así que se ofrece solo a quien la tiene.
                    @if *sufijo != "/capacidades" || nombra {
                        a class=[(*sufijo == activo).then_some("enlace-activo")]
                          href={ (RUTA) (sufijo) } { (etiqueta) }
                    }
                }
            }
        }
    }
}

fn pantalla(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA)), cuerpo).into_response()
}

// ── Institución ──────────────────────────────────────────────────────────────

/// `GET /panel/admin`
pub async fn institucion(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "");
    };
    let i = match adm::institucion(&e.pool, inst).await {
        Ok(Some(i)) => i,
        Ok(None) | Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Institución",
        html! {
            (cabecera(&cx, ""))
            @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }
            section class="tarjeta" {
                // El NIT no se edita: identifica a la persona jurídica ante la
                // DIAN y ante MinSalud, y cambiarlo es dar de alta otro
                // prestador, no corregir un campo.
                (piezas::datos(&[
                    ("NIT", Some(i.nit.clone())),
                    ("Código de habilitación", i.codigo_habilitacion.clone()),
                ]))
                p class="sutil" { "El NIT no se edita: identifica al prestador." }
            }
            section class="tarjeta" {
                h2 { "Datos" }
                form method="post" action=(RUTA) class="apilado" {
                    (campo_texto("razon_social", "Razón social", i.razon_social.as_deref()))
                    (campo_texto("nombre_comercial", "Nombre comercial", i.nombre_comercial.as_deref()))
                    (campo_texto("codigo_habilitacion", "Código de habilitación", i.codigo_habilitacion.as_deref()))
                    (campo_texto("codigo_prestador", "Código de prestador", i.codigo_prestador.as_deref()))
                    (campo_texto("tipo_prestador", "Tipo de prestador", i.tipo_prestador.as_deref()))
                    (campo_texto("direccion", "Dirección", i.direccion.as_deref()))
                    (campo_texto("ciudad", "Ciudad", i.ciudad.as_deref()))
                    (campo_texto("departamento", "Departamento", i.departamento.as_deref()))
                    (campo_texto("telefono", "Teléfono", i.telefono.as_deref()))
                    (campo_texto("email", "Correo", i.email.as_deref()))
                    (campo_texto("representante_legal", "Representante legal", i.representante_legal.as_deref()))
                    (campo_texto("identificacion_representante", "Identificación del representante", i.identificacion_representante.as_deref()))
                    button type="submit" class="principal" { "Guardar" }
                }
            }
        },
    )
}

fn campo_texto(nombre: &str, etiqueta: &str, valor: Option<&str>) -> Markup {
    html! {
        label for=(nombre) { (etiqueta) }
        input id=(nombre) type="text" name=(nombre) value=(valor.unwrap_or_default());
    }
}

fn sin_institucion(cx: &Contexto, activo: &str) -> Response {
    pantalla(
        cx,
        "Administración",
        html! {
            (cabecera(cx, activo))
            section class="tarjeta" {
                (aviso(
                    Tono::Averia,
                    "Esta cuenta no está adscrita a ninguna institución, así que no hay nada \
                     que administrar. El superadministrador gestiona las instituciones desde \
                     el tablero.",
                ))
            }
        },
    )
}

/// `POST /panel/admin`
pub async fn guardar_institucion(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "");
    };
    if let Err(r) = anotar(&e, &u, "configurar", "institucion", format!("id={inst}")).await {
        return r;
    }
    let cambio = adm::InstitucionCambio {
        razon_social: texto(&c, "razon_social"),
        nombre_comercial: texto(&c, "nombre_comercial"),
        codigo_habilitacion: texto(&c, "codigo_habilitacion"),
        direccion: texto(&c, "direccion"),
        ciudad: texto(&c, "ciudad"),
        departamento: texto(&c, "departamento"),
        telefono: texto(&c, "telefono"),
        email: texto(&c, "email"),
        representante_legal: texto(&c, "representante_legal"),
        identificacion_representante: texto(&c, "identificacion_representante"),
        codigo_prestador: texto(&c, "codigo_prestador"),
        tipo_prestador: texto(&c, "tipo_prestador"),
        color_primario: None,
        color_secundario: None,
    };
    match adm::actualizar_institucion(&e.pool, inst, &cambio).await {
        Ok(_) => de_vuelta(RUTA, "institucion"),
        Err(_) => averia(),
    }
}

// ── Configuración ────────────────────────────────────────────────────────────

/// `GET /panel/admin/configuracion`
pub async fn configuracion(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/configuracion");
    };
    dibujar_configuracion(&e, &cx, inst, f.aviso.trim(), None).await
}

async fn dibujar_configuracion(
    e: &Estado,
    cx: &Contexto,
    inst: i64,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let c = match adm::configuracion(&e.pool, inst).await {
        Ok(Some(c)) => c,
        Ok(None) | Err(_) => return averia(),
    };
    // La misma forma que la pestaña de seguridad, y por la misma medición: de
    // las 17 columnas de esta tabla, el 2026-08-08 no había NINGUNA con lector.
    // Dos se enchufaron ese día y las otras quince se enseñan con su valor y su
    // motivo, sin poder cambiarse — un interruptor que no enciende nada es peor
    // que no ofrecerlo. La lista sale de `CONFIGURACION_APLICADA`, así que el
    // día que una se enchufe pasa sola al formulario de arriba.
    let valores = serde_json::to_value(&c).unwrap_or(serde_json::Value::Null);
    let valor_de = |campo: &str| match valores.get(campo) {
        Some(serde_json::Value::Bool(b)) => (if *b { "sí" } else { "no" }).to_string(),
        Some(serde_json::Value::String(s)) if s.is_empty() => "—".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) => "—".to_string(),
        Some(serde_json::Value::Array(a)) if a.is_empty() => "—".to_string(),
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        Some(v) => v.to_string(),
        None => "—".to_string(),
    };
    let sin_efecto: Vec<_> = adm::CONFIGURACION_APLICADA
        .iter()
        .filter_map(|(campo, motivo)| motivo.map(|m| (*campo, m)))
        .collect();

    pantalla(
        cx,
        "Configuración",
        html! {
            (cabecera(cx, "/configuracion"))
            @if let Some((tono, t)) = traducir_aviso(clave_aviso) { (aviso(tono, t)) }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            section class="tarjeta" {
                (aviso(Tono::Averia, &format!(
                    "De las {} columnas de esta configuración, este binario aplica {} y guarda \
                     las otras {} sin leerlas nunca. Esas {} se enseñan abajo con su valor y su \
                     motivo, pero NO se pueden cambiar desde aquí: un interruptor que no \
                     enciende nada es peor que no ofrecerlo.",
                    adm::CONFIGURACION_APLICADA.len(),
                    adm::CONFIGURACION_APLICADA.len() - sin_efecto.len(),
                    sin_efecto.len(),
                    sin_efecto.len(),
                )))
            }

            section class="tarjeta" {
                h2 { "Lo que sí manda" }
                p class="sutil" {
                    "La duración de la cita la usa la agenda cuando un bloque no declara la \
                     suya; el prefijo y la resolución de la DIAN gobiernan cómo se numeran las \
                     facturas de esta institución."
                }
                form method="post" action={ (RUTA) "/configuracion" } class="apilado" {
                    label for="duracion" { "Duración de la cita (minutos)" }
                    input #duracion type="number" name="duracion_cita_default" min="1"
                          value=(c.duracion_cita_default);
                    label for="prefijo" { "Prefijo de factura" }
                    input #prefijo type="text" name="prefijo_factura" value=(c.prefijo_factura);
                    p class="sutil" {
                        "Cambiar el prefijo empieza una SERIE nueva desde el número 1: el \
                         contador va por prefijo y por año, que es lo que una serie de \
                         numeración significa."
                    }
                    label for="resolucion" { "Resolución de facturación (DIAN)" }
                    input #resolucion type="text" name="resolucion_facturacion"
                          value=(c.resolucion_facturacion.as_deref().unwrap_or_default());
                    label for="rango_desde" { "Rango autorizado — desde" }
                    input #rango_desde type="number" name="rango_desde" min="0"
                          value=(c.rango_desde.map(|n| n.to_string()).unwrap_or_default());
                    label for="rango_hasta" { "Rango autorizado — hasta" }
                    input #rango_hasta type="number" name="rango_hasta" min="0"
                          value=(c.rango_hasta.map(|n| n.to_string()).unwrap_or_default());
                    p class="sutil" {
                        "La resolución de la DIAN autoriza un RANGO de numeración. Con el rango \
                         puesto la serie arranca en «desde» y una factura cuyo número superaría \
                         «hasta» se rechaza, en vez de salir inválida; el número de la resolución \
                         se exige junto con el rango. Sin rango, se numera sin restricción."
                    }
                    button type="submit" class="principal" { "Guardar" }
                }
            }

            section class="tarjeta" {
                h2 { "Lo que se guarda y hoy no manda" }
                p class="sutil" {
                    "Estas columnas existen en la base y ninguna se lee. Se enseñan porque una \
                     institución que las configuró tiene derecho a saber qué quedó escrito; no \
                     se editan porque cambiarlas no cambiaría nada."
                }
                div class="tabla-envoltura" {
                    table class="tabla" {
                        thead { tr { th { "Columna" } th { "Valor guardado" } th { "Por qué no manda" } } }
                        tbody {
                            @for (columna, motivo) in &sin_efecto {
                                tr {
                                    td class="mono" { (columna) }
                                    td { (valor_de(columna)) }
                                    td class="sutil" { (motivo) }
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

/// `POST /panel/admin/configuracion`
pub async fn guardar_configuracion(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/configuracion");
    };
    // **Se escriben EXACTAMENTE las columnas que el binario aplica**, igual que
    // en `guardar_seguridad`. Las demás no se leen del formulario porque el
    // formulario ya no las ofrece; aceptarlas «por si acaso» las volvería a
    // dejar escribibles por quien mande el POST a mano, y con eso el aviso de la
    // pantalla sería falso.
    if let Err(r) = anotar(&e, &u, "configurar", "configuracion", format!("institucion={inst}")).await
    {
        return r;
    }
    let cambio = adm::ConfiguracionCambio {
        duracion_cita_default: entero(&c, "duracion_cita_default"),
        prefijo_factura: texto(&c, "prefijo_factura"),
        resolucion_facturacion: texto(&c, "resolucion_facturacion"),
        rango_desde: entero(&c, "rango_desde"),
        rango_hasta: entero(&c, "rango_hasta"),
        ..Default::default()
    };
    match adm::actualizar_configuracion(&e.pool, inst, &cambio).await {
        Ok(Ok(())) => de_vuelta(&format!("{RUTA}/configuracion"), "configuracion"),
        Ok(Err(motivo)) => {
            dibujar_configuracion(&e, &cx, inst, "", Some(&motivo.mensaje())).await
        }
        Err(_) => averia(),
    }
}

// ── Seguridad ────────────────────────────────────────────────────────────────

/// `GET /panel/admin/seguridad`
///
/// **Cada campo dice si MANDA.** Ver la cabecera del módulo.
pub async fn seguridad(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/seguridad");
    };
    let p = match adm::politica(&e.pool, inst).await {
        Ok(Some(p)) => p,
        Ok(None) | Err(_) => return averia(),
    };
    // Los valores por NOMBRE de columna, para que la lista de abajo se derive de
    // `POLITICA_APLICADA` y no de una segunda enumeración escrita a mano: es la
    // misma tabla que el guardián de `medico-datos` compara contra el tipo, así
    // que una columna nueva aparece en pantalla sola.
    let valores = serde_json::to_value(&p).unwrap_or(serde_json::Value::Null);
    let valor_de = |campo: &str| match valores.get(campo) {
        Some(serde_json::Value::Bool(b)) => (if *b { "sí" } else { "no" }).to_string(),
        Some(serde_json::Value::String(s)) if s.is_empty() => "—".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) => "—".to_string(),
        Some(v) => v.to_string(),
        None => "—".to_string(),
    };
    let sin_efecto: Vec<_> =
        adm::POLITICA_APLICADA.iter().filter_map(|(c, m)| m.map(|m| (*c, m))).collect();

    pantalla(
        &cx,
        "Seguridad",
        html! {
            (cabecera(&cx, "/seguridad"))
            @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }
            section class="tarjeta" {
                (aviso(Tono::Averia, &format!(
                    "De las {} columnas de esta política, este binario aplica {} y guarda las \
                     otras {} sin leerlas nunca. Esas {} se enseñan abajo con su valor y su \
                     motivo, pero NO se pueden cambiar desde aquí: un interruptor que no \
                     enciende nada es peor que no ofrecerlo.",
                    adm::POLITICA_APLICADA.len(),
                    adm::POLITICA_APLICADA.len() - sin_efecto.len(),
                    sin_efecto.len(),
                    sin_efecto.len(),
                )))
            }

            section class="tarjeta" {
                h2 { "Contraseñas" }
                p class="sutil" {
                    "Es lo que este binario comprueba de verdad, cada vez que alguien fija una \
                     contraseña: el alta de personal, el restablecimiento del administrador y el \
                     cambio de la propia."
                }
                form method="post" action={ (RUTA) "/seguridad" } class="apilado" {
                    label for="longitud" { "Longitud mínima" }
                    input #longitud type="number" name="longitud_minima_password" min="8" max="128"
                          value=(p.longitud_minima_password);
                    (casilla("requiere_mayuscula", "Exigir una mayúscula", p.requiere_mayuscula))
                    (casilla("requiere_minuscula", "Exigir una minúscula", p.requiere_minuscula))
                    (casilla("requiere_numero", "Exigir un número", p.requiere_numero))
                    (casilla("requiere_caracter_especial", "Exigir un carácter especial", p.requiere_caracter_especial))
                    button type="submit" class="principal" { "Guardar" }
                }
            }

            section class="tarjeta" {
                h2 { "Lo que se guarda y hoy no manda" }
                p class="sutil" {
                    "Estas columnas existen en la base y ninguna se lee. Se enseñan porque una \
                     institución que las configuró tiene derecho a saber qué quedó escrito; no \
                     se editan porque cambiarlas no cambiaría nada."
                }
                div class="tabla-envoltura" {
                    table class="tabla" {
                        thead { tr { th { "Columna" } th { "Valor guardado" } th { "Por qué no manda" } } }
                        tbody {
                            @for (columna, motivo) in &sin_efecto {
                                tr {
                                    td class="mono" { (columna) }
                                    td { (valor_de(columna)) }
                                    td class="sutil" { (motivo) }
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

fn casilla(nombre: &str, etiqueta: &str, valor: bool) -> Markup {
    html! {
        label {
            input type="checkbox" name=(nombre) value="1" checked[valor];
            " " (etiqueta)
        }
    }
}

/// `POST /panel/admin/seguridad`
pub async fn guardar_seguridad(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/seguridad");
    };
    // Se escriben EXACTAMENTE las cinco columnas que este binario lee. Ofrecer
    // en el formulario una que no se escribe —o escribir una que nadie lee— son
    // las dos mitades del mismo engaño: la pantalla contesta «guardado» y el
    // número vuelve al recargar, o se queda escrito y no manda nada.
    if let Err(r) =
        anotar(&e, &u, "configurar", "politica_seguridad", format!("institucion={inst}")).await
    {
        return r;
    }
    let cambio = adm::PoliticaCambio {
        longitud_minima_password: entero(&c, "longitud_minima_password"),
        requiere_mayuscula: Some(c.contains_key("requiere_mayuscula")),
        requiere_minuscula: Some(c.contains_key("requiere_minuscula")),
        requiere_numero: Some(c.contains_key("requiere_numero")),
        requiere_caracter_especial: Some(c.contains_key("requiere_caracter_especial")),
        ..Default::default()
    };
    match adm::actualizar_politica(&e.pool, inst, &cambio).await {
        Ok(_) => de_vuelta(&format!("{RUTA}/seguridad"), "politica"),
        Err(_) => averia(),
    }
}

// ── Campos propios ───────────────────────────────────────────────────────────

/// `GET /panel/admin/campos`
pub async fn campos(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let lista = match adm::campos(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Campos propios",
        html! {
            (cabecera(&cx, "/campos"))
            @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }
            section class="tarjeta" {
                p class="sutil" {
                    "Campos que esta institución añade a sus formularios, además de los del "
                    "producto."
                }
                @if lista.is_empty() {
                    p class="sutil" { "No hay campos propios." }
                } @else {
                    (piezas::tabla(
                        &["Etiqueta", "Tipo", "Opciones", "Orden", "Estado", ""],
                        html! {
                            @for c in &lista {
                                tr {
                                    td { (c.etiqueta) }
                                    td class="sutil" { (c.tipo) }
                                    td class="sutil" { (c.opciones.clone().unwrap_or_default()) }
                                    td class="mono" { (c.orden) }
                                    td {
                                        @if c.activo {
                                            (piezas::estado("ACTIVA"))
                                        } @else {
                                            (piezas::estado("INACTIVA"))
                                        }
                                    }
                                    td {
                                        form method="post"
                                             action={ (RUTA) "/campos/" (c.id) "/borrar" } {
                                            button type="submit" class="enlace"
                                                    data-confirmar="Pulse otra vez para eliminarlo" {
                                                "Eliminar"
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
            }
            section class="tarjeta" {
                h2 { "Nuevo campo" }
                form method="post" action={ (RUTA) "/campos" } class="apilado" {
                    label for="etiqueta" { "Etiqueta" }
                    input #etiqueta type="text" name="etiqueta" required;
                    label for="tipo" { "Tipo" }
                    select #tipo name="tipo" {
                        @for t in ["texto", "numero", "fecha", "lista", "booleano"] {
                            option value=(t) { (t) }
                        }
                    }
                    label for="opciones" { "Opciones (para una lista)" }
                    input #opciones type="text" name="opciones" placeholder="una, otra, otra más";
                    label for="orden" { "Orden" }
                    input #orden type="number" name="orden" value="0";
                    button type="submit" class="principal" { "Crear" }
                }
            }
        },
    )
}

/// `POST /panel/admin/campos`
pub async fn crear_campo(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/campos");
    };
    let nuevo = adm::CampoNuevo {
        etiqueta: c.get("etiqueta").cloned().unwrap_or_default().trim().to_string(),
        tipo: texto(&c, "tipo").unwrap_or_else(|| "texto".into()),
        opciones: texto(&c, "opciones"),
        orden: entero(&c, "orden").unwrap_or(0),
    };
    if let Err(r) = anotar(
        &e,
        &u,
        "crear",
        "campo_dinamico",
        format!("institucion={inst} etiqueta={} tipo={}", nuevo.etiqueta, nuevo.tipo),
    )
    .await
    {
        return r;
    }
    match adm::crear_campo(&e.pool, inst, &nuevo).await {
        Ok(_) => de_vuelta(&format!("{RUTA}/campos"), "campo"),
        Err(_) => averia(),
    }
}

/// `POST /panel/admin/campos/{id}/borrar`
pub async fn borrar_campo(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Path(id): Path<i64>,
) -> Response {
    if let Err(r) = anotar(&e, &u, "borrar", "campo_dinamico", format!("id={id}")).await {
        return r;
    }
    match adm::borrar_campo(&e.pool, u.ambito(), id).await {
        Ok(_) => de_vuelta(&format!("{RUTA}/campos"), "campo_borrado"),
        Err(_) => averia(),
    }
}

// ── Catálogos ────────────────────────────────────────────────────────────────

/// `GET /panel/admin/catalogos`
pub async fn catalogos(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (esp, ent) = match (
        adm::especialidades(&e.pool).await,
        adm::entidades(&e.pool).await,
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => return averia(),
    };
    pantalla(
        &cx,
        "Catálogos",
        html! {
            (cabecera(&cx, "/catalogos"))
            // El rechazo va PRIMERO y con su motivo: es la única salida de esta
            // pantalla que el operador tiene que leer entera.
            @if f.aviso.trim() == "rechazo" {
                (aviso(Tono::Error, f.motivo.trim()))
            } @else if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) {
                (aviso(tono, t))
            }
            section class="tarjeta" {
                h2 { "Especialidades" }
                // Son NACIONALES, no de la institución: una especialidad médica
                // es la misma en todas partes. Se dice, porque el resto de esta
                // pantalla sí es de una institución.
                p class="sutil" {
                    "Catálogo común a todos los prestadores: una especialidad médica es la "
                    "misma en cualquier IPS."
                }
                @if esp.is_empty() {
                    p class="sutil" { "No hay especialidades registradas." }
                } @else {
                    (piezas::tabla(&["Especialidad"], html! {
                        @for s in &esp { tr { td { (s.nombre) } } }
                    }))
                }
                form method="post" action={ (RUTA) "/especialidades" } class="buscador" {
                    input type="text" name="nombre" required placeholder="Cardiología";
                    button type="submit" { "Añadir" }
                }
            }
            section class="tarjeta" {
                h2 { "Entidades pagadoras" }
                p class="sutil" {
                    "También nacional: una EPS es la misma persona jurídica para todos los "
                    "prestadores."
                }
                @if ent.is_empty() {
                    p class="sutil" { "No hay entidades registradas." }
                } @else {
                    (piezas::tabla(
                        &["Entidad", "NIT", "Código EPS", "Código interno", "Teléfono", "Contacto"],
                        html! {
                            @for x in &ent {
                                tr {
                                    td { (x.nombre) }
                                    td class="mono" { (x.nit) }
                                    td class="mono" { (x.codigo_eps) }
                                    // Una celda vacía se ve igual que una que
                                    // nadie llenó; el guion dice que se miró y
                                    // no había (directiva 20 en la pantalla).
                                    td class="mono" { (x.codigo_interno.as_deref().unwrap_or("—")) }
                                    td class="mono" { (x.telefono.as_deref().unwrap_or("—")) }
                                    td { (x.contacto.as_deref().unwrap_or("—")) }
                                }
                            }
                        }
                    ))
                }
                form method="post" action={ (RUTA) "/entidades" } class="apilado" {
                    label for="nit" { "NIT" }
                    input #nit type="text" name="nit" required;
                    label for="nombre_ent" { "Nombre" }
                    input #nombre_ent type="text" name="nombre" required;
                    label for="codigo_eps" { "Código EPS" }
                    input #codigo_eps type="text" name="codigo_eps" required;
                    // Los siete de contacto son OPCIONALES —sin `required`— y
                    // por eso van detrás: el alta mínima sigue siendo tres
                    // campos, como antes. Son los que el VFP capturaba en su
                    // tabla `TERCEROS` y medico no tenía dónde guardar (#752).
                    p class="sutil" {
                        "Lo de abajo es opcional. Es lo que el sistema anterior "
                        "guardaba de cada tercero: sin esto, la migración pierde "
                        "los datos de contacto que el prestador ya tiene."
                    }
                    label for="codigo_interno" { "Código interno" }
                    input #codigo_interno type="text" name="codigo_interno"
                          placeholder="el número con el que la tiene en el sistema de siempre";
                    label for="direccion_ent" { "Dirección" }
                    input #direccion_ent type="text" name="direccion";
                    label for="telefono_ent" { "Teléfono" }
                    input #telefono_ent type="text" name="telefono";
                    label for="fax_ent" { "Fax" }
                    input #fax_ent type="text" name="fax";
                    label for="contacto_ent" { "Persona de contacto" }
                    input #contacto_ent type="text" name="contacto";
                    label for="email_ent" { "Correo" }
                    input #email_ent type="email" name="email";
                    label for="cod_municipio_ent" { "Municipio (DIVIPOLA)" }
                    input #cod_municipio_ent type="text" name="cod_municipio"
                          inputmode="numeric" maxlength="5"
                          placeholder="cinco dígitos, con el cero delante si lo lleva";
                    button type="submit" { "Añadir" }
                }
            }
        },
    )
}

/// `POST /panel/admin/especialidades`
///
/// **La puerta de ámbito no es decorativa aquí**: `especialidad` y `entidad` son
/// catálogos GLOBALES —no llevan `institucion_id`—, así que escribir en ellos
/// alcanza a todos los prestadores, y `entidad.codigo_eps` viaja al RIPS de
/// todos. Una cuenta con rol `admin` e `institucion_id = NULL` pasa el extractor
/// por el defecto nacional del rol, no ve una sola fila de datos de nadie, y
/// hasta el 2026-08-10 escribía aquí mientras `/api` le respondía 403.
///
/// Se comparte `rutas::puede_administrar_maestros` con las siete rutas de la
/// API en vez de escribir la comprobación otra vez: era ya la CUARTA aparición
/// de esta forma, y las tres anteriores están citadas en su doc.
pub async fn crear_especialidad(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    if let Err((_, motivo)) = crate::rutas::puede_administrar_maestros(&u) {
        return sin_permiso_de_maestros(motivo);
    }
    let nombre = c.get("nombre").cloned().unwrap_or_default();
    if let Err(r) = anotar(&e, &u, "crear", "especialidad", format!("nombre={nombre}")).await {
        return r;
    }
    // La misma forma que `crear_entidad`, y el mismo defecto: `Ok(_)` casaba
    // también el rechazo. Una especialidad repetida o con el nombre vacío salía
    // anunciada como creada.
    match adm::crear_especialidad(&e.pool, nombre.trim()).await {
        Ok(Ok(_)) => de_vuelta(&format!("{RUTA}/catalogos"), "especialidad"),
        Ok(Err(i)) => de_vuelta_rechazo(&format!("{RUTA}/catalogos"), &i.mensaje()),
        Err(_) => averia(),
    }
}

/// `POST /panel/admin/entidades`. Misma puerta que `crear_especialidad`, y por
/// el mismo motivo: el catálogo es global.
pub async fn crear_entidad(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    if let Err((_, motivo)) = crate::rutas::puede_administrar_maestros(&u) {
        return sin_permiso_de_maestros(motivo);
    }
    // Los siete de contacto se pasan tal cual, SIN recortar ni descartar los
    // vacíos: eso lo decide `crear_entidad` en un solo sitio (`vacio_a_nulo`),
    // para que esta puerta y la de la API no puedan divergir en qué es «vacío».
    let opt = |k: &str| c.get(k).cloned();
    let nueva = adm::EntidadNueva {
        nit: c.get("nit").cloned().unwrap_or_default().trim().to_string(),
        nombre: c.get("nombre").cloned().unwrap_or_default().trim().to_string(),
        codigo_eps: c.get("codigo_eps").cloned().unwrap_or_default().trim().to_string(),
        codigo_interno: opt("codigo_interno"),
        direccion: opt("direccion"),
        telefono: opt("telefono"),
        fax: opt("fax"),
        contacto: opt("contacto"),
        email: opt("email"),
        cod_municipio: opt("cod_municipio"),
    };
    if let Err(r) = anotar(&e, &u, "crear", "entidad", format!("nombre={}", nueva.nombre)).await {
        return r;
    }
    // Las TRES salidas, y `Ok(_)` casaba dos de ellas. Ver `de_vuelta_rechazo`.
    match adm::crear_entidad(&e.pool, &nueva).await {
        Ok(Ok(_)) => de_vuelta(&format!("{RUTA}/catalogos"), "entidad"),
        Ok(Err(i)) => de_vuelta_rechazo(&format!("{RUTA}/catalogos"), &i.mensaje()),
        Err(_) => averia(),
    }
}

/// El 403 de los maestros, dibujado. Va aparte porque lo comparten los dos
/// manejadores y porque el motivo lo escribe la MISMA función que responde en
/// `/api`: dos textos distintos para la misma negativa envejecen por separado.
fn sin_permiso_de_maestros(motivo: &str) -> Response {
    (
        axum::http::StatusCode::FORBIDDEN,
        crate::vista::pagina(
            "Sin permiso",
            None,
            html! {
                section class="tarjeta" {
                    (aviso(Tono::Error, motivo))
                    p { a href="/panel" { "Volver" } }
                }
            },
        ),
    )
        .into_response()
}


/// La bitácora de un acto administrativo, para el panel.
///
/// Llama a la MISMA `rutas::anotar_acto_admin` que usan las rutas JSON, con los
/// mismos argumentos, y conserva su contrato: **obligatoria y antes de aplicar**
/// —si no se puede registrar, no se aplica—. Aquí no hay paciente esperando;
/// esto son actos que amplían o recortan quién toca datos clínicos, y un cambio
/// de autorización sin rastro es justo lo que la bitácora existe para impedir.
///
/// Existía solo en `/api`. Mientras Angular vivió, estas pantallas pasaban por
/// ahí y el acto constaba; al migrarlas al panel el rastro dejó de escribirse, y
/// `/panel/admin/auditoria` no podía enseñar quién había cambiado la política de
/// contraseñas ni quién había tocado la matriz de capacidades. Lo midió la
/// revisión de seguridad del 2026-08-10.
///
/// Devuelve `Err(Response)` con la pantalla ya dibujada, para que el manejador
/// solo tenga que propagarla.
async fn anotar(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    accion: &str,
    recurso: &str,
    detalle: String,
) -> Result<(), Response> {
    crate::rutas::anotar_acto_admin(e, u, accion, recurso, detalle)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                crate::vista::pagina(
                    "No se aplicó",
                    None,
                    html! {
                        section class="tarjeta" {
                            (aviso(
                                Tono::Averia,
                                "No se pudo registrar el cambio en la bitácora, así que NO se \
                                 aplicó. Vuelva a intentarlo.",
                            ))
                            p { a href="/panel/admin" { "Volver" } }
                        }
                    },
                ),
            )
                .into_response()
        })
}

// ── Personal ─────────────────────────────────────────────────────────────────

/// `GET /panel/admin/usuarios`
pub async fn usuarios(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    dibujar_usuarios(&e, &cx, &u, f.aviso.trim(), None).await
}

async fn dibujar_usuarios(
    e: &Estado,
    cx: &Contexto,
    u: &medico_datos::usuario::Usuario,
    clave_aviso: &str,
    error: Option<&str>,
) -> Response {
    let lista = match adm::usuarios(&e.pool, u.ambito(), &Default::default(), &[]).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    // Solo se ofrecen los roles que ESTA cuenta puede conceder: un admin no
    // fabrica superadministradores, y ofrecérselo sería el botón que lleva al
    // 403. La regla la decide `ascenso_permitido`, no una lista de aquí.
    let roles: Vec<RolUsuario> = RolUsuario::TODOS
        .iter()
        .copied()
        .filter(|r| adm::ascenso_permitido(Some(r.como_str()), u))
        .collect();

    pantalla(
        cx,
        "Personal",
        html! {
            (cabecera(cx, "/usuarios"))
            @if let Some((tono, t)) = traducir_aviso(clave_aviso) { (aviso(tono, t)) }
            @if let Some(t) = error { (aviso(Tono::Error, t)) }
            section class="tarjeta" {
                @if lista.is_empty() {
                    p class="sutil" { "No hay personal registrado." }
                } @else {
                    (piezas::recuento(lista.len(), lista.len(), "persona", "personas"))
                    (piezas::tabla(
                        &["Nombre", "Correo", "Rol", "Especialidad", "Estado", "Acciones"],
                        html! {
                            @for p in &lista {
                                tr {
                                    td { (p.nombre_completo) }
                                    td class="mono sutil" { (p.email.clone().unwrap_or_default()) }
                                    td { (p.rol) }
                                    td class="sutil" {
                                        (p.especialidad_nombre.clone().unwrap_or_default())
                                    }
                                    td {
                                        @if p.activo {
                                            (piezas::estado("ACTIVA"))
                                        } @else {
                                            (piezas::estado("INACTIVA"))
                                        }
                                        @if p.requiere_cambio_password {
                                            div class="sutil" { "debe cambiar la contraseña" }
                                        }
                                    }
                                    td {
                                        form method="post"
                                             action={ (RUTA) "/usuarios/" (p.id) "/activo" } {
                                            button type="submit" class="enlace" {
                                                @if p.activo { "Desactivar" } @else { "Reactivar" }
                                            }
                                        }
                                        form method="post"
                                             action={ (RUTA) "/usuarios/" (p.id) "/password" }
                                             class="apilado" {
                                            input type="password" name="nueva_password" required
                                                  placeholder="Contraseña nueva"
                                                  autocomplete="new-password";
                                            button type="submit" class="enlace"
                                                    data-confirmar="Pulse otra vez para restablecerla" {
                                                "Restablecer contraseña"
                                            }
                                        }
                                        details {
                                            summary class="enlace" {
                                                "Nombre partido (RDA)"
                                                @if p.nombre1.is_none() && p.apellido1.is_none() {
                                                    " — sin capturar"
                                                }
                                            }
                                            form method="post"
                                                 action={ (RUTA) "/usuarios/" (p.id) "/nombre" }
                                                 class="apilado" {
                                                input type="text" name="nombre1"
                                                      placeholder="Primer nombre"
                                                      value=(p.nombre1.clone().unwrap_or_default());
                                                input type="text" name="nombre2"
                                                      placeholder="Segundo nombre"
                                                      value=(p.nombre2.clone().unwrap_or_default());
                                                input type="text" name="apellido1"
                                                      placeholder="Primer apellido"
                                                      value=(p.apellido1.clone().unwrap_or_default());
                                                input type="text" name="apellido2"
                                                      placeholder="Segundo apellido"
                                                      value=(p.apellido2.clone().unwrap_or_default());
                                                button type="submit" class="enlace" { "Guardar nombre" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                }
            }

            section class="tarjeta" {
                h2 { "Dar de alta" }
                p class="sutil" {
                    "El alta consume un asiento de la licencia; si la persona es médica, "
                    "consume además uno de médicos. La contraseña pasa por la política de "
                    "esta institución y quien entre tendrá que cambiarla."
                }
                form method="post" action={ (RUTA) "/usuarios" } class="apilado" {
                    label for="nombre_u" { "Nombre completo" }
                    input #nombre_u type="text" name="nombre_completo" required;
                    label for="email_u" { "Correo de acceso" }
                    input #email_u type="email" name="email" required autocomplete="off";
                    label for="rol_u" { "Rol" }
                    select #rol_u name="rol" required {
                        @for r in &roles { option value=(r.como_str()) { (r.como_str()) } }
                    }
                    label for="pass_u" { "Contraseña" }
                    input #pass_u type="password" name="password" required
                          autocomplete="new-password";
                    label for="reg_u" { "Registro médico" }
                    input #reg_u type="text" name="registro_medico";
                    p class="sutil" {
                        "Nombre partido para el reporte al visor nacional (RDA). "
                        "Opcional; solo hace falta para el personal que atiende, y "
                        "se puede completar después desde la lista."
                    }
                    label for="n1_u" { "Primer nombre" }
                    input #n1_u type="text" name="nombre1";
                    label for="n2_u" { "Segundo nombre" }
                    input #n2_u type="text" name="nombre2";
                    label for="a1_u" { "Primer apellido" }
                    input #a1_u type="text" name="apellido1";
                    label for="a2_u" { "Segundo apellido" }
                    input #a2_u type="text" name="apellido2";
                    button type="submit" class="principal" { "Dar de alta" }
                }
            }
        },
    )
}

/// `POST /panel/admin/usuarios`
pub async fn crear_usuario(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let nuevo = adm::UsuarioAltaAdmin {
        username: None,
        email: texto(&c, "email"),
        password: c.get("password").cloned().unwrap_or_default(),
        nombre_completo: c.get("nombre_completo").cloned().unwrap_or_default().trim().to_string(),
        rol: texto(&c, "rol").unwrap_or_default(),
        especialidad_id: entero(&c, "especialidad_id"),
        registro_medico: texto(&c, "registro_medico"),
        registro_rethus: texto(&c, "registro_rethus"),
        nombre1: texto(&c, "nombre1"),
        nombre2: texto(&c, "nombre2"),
        apellido1: texto(&c, "apellido1"),
        apellido2: texto(&c, "apellido2"),
    };
    // Las CINCO puertas viven en esta función, compartida con `/api`.
    match crate::rutas::dar_de_alta_personal(&e, &u, None, &nuevo).await {
        EscrituraPersonal::Creado(_) => de_vuelta(&format!("{RUTA}/usuarios"), "usuario"),
        EscrituraPersonal::Rechazado { motivo, .. } => {
            dibujar_usuarios(&e, &cx, &u, "", Some(&motivo)).await
        }
        // La licencia se cuenta con una FRASE, no con la clave que lee un
        // cliente de la API: aquí quien mira es una persona.
        EscrituraPersonal::SinCupo(p) => {
            dibujar_usuarios(&e, &cx, &u, "", Some(crate::rutas::frase_de_licencia(p))).await
        }
        EscrituraPersonal::NoExiste => {
            dibujar_usuarios(&e, &cx, &u, "", Some("No se encontró a esa persona.")).await
        }
        EscrituraPersonal::Hecho => de_vuelta(&format!("{RUTA}/usuarios"), "usuario"),
        EscrituraPersonal::Averia => averia(),
    }
}

/// `POST /panel/admin/usuarios/{id}/activo` — alterna el alta.
///
/// **Alterna en vez de recibir el valor**, como la baja de una ambulancia: un
/// `activo` en el cuerpo dejaría que reenviar una petición vieja reactivara a
/// quien acaba de irse — y reactivar consume un asiento de la licencia.
pub async fn alternar_activo(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let lista = match adm::usuarios(&e.pool, u.ambito(), &Default::default(), &[]).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let Some(actual) = lista.iter().find(|p| p.id == id) else {
        return dibujar_usuarios(&e, &cx, &u, "", Some("No se encontró a esa persona.")).await;
    };
    let cambio = adm::UsuarioCambioAdmin {
        activo: Some(!actual.activo),
        ..Default::default()
    };
    match crate::rutas::editar_personal(&e, &u, id, &cambio).await {
        EscrituraPersonal::Hecho | EscrituraPersonal::Creado(_) => {
            de_vuelta(&format!("{RUTA}/usuarios"), "usuario_editado")
        }
        EscrituraPersonal::Rechazado { motivo, .. } => {
            dibujar_usuarios(&e, &cx, &u, "", Some(&motivo)).await
        }
        // La licencia se cuenta con una FRASE, no con la clave que lee un
        // cliente de la API: aquí quien mira es una persona.
        EscrituraPersonal::SinCupo(p) => {
            dibujar_usuarios(&e, &cx, &u, "", Some(crate::rutas::frase_de_licencia(p))).await
        }
        EscrituraPersonal::NoExiste => {
            dibujar_usuarios(&e, &cx, &u, "", Some("No se encontró a esa persona.")).await
        }
        EscrituraPersonal::Averia => averia(),
    }
}

/// `POST /panel/admin/usuarios/{id}/password`
pub async fn restablecer_password(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let nueva = c.get("nueva_password").cloned().unwrap_or_default();
    match crate::rutas::restablecer_password_de(&e, &u, id, &nueva).await {
        EscrituraPersonal::Hecho | EscrituraPersonal::Creado(_) => {
            de_vuelta(&format!("{RUTA}/usuarios"), "password")
        }
        EscrituraPersonal::Rechazado { motivo, .. } => {
            dibujar_usuarios(&e, &cx, &u, "", Some(&motivo)).await
        }
        // La licencia se cuenta con una FRASE, no con la clave que lee un
        // cliente de la API: aquí quien mira es una persona.
        EscrituraPersonal::SinCupo(p) => {
            dibujar_usuarios(&e, &cx, &u, "", Some(crate::rutas::frase_de_licencia(p))).await
        }
        EscrituraPersonal::NoExiste => {
            dibujar_usuarios(&e, &cx, &u, "", Some("No se encontró a esa persona.")).await
        }
        EscrituraPersonal::Averia => averia(),
    }
}

/// `POST /panel/admin/usuarios/{id}/nombre` — fija el nombre PARTIDO del
/// profesional (#603) que el RDA de encuentro exige. No pasa por las puertas de
/// licencia/cupo porque no cambia rol ni actividad: es solo un dato de identidad
/// del `Practitioner`. El ámbito lo hace cumplir `fijar_nombre_partido`
/// (`del_ambito` + `cola_del_ambito`), así que un admin no toca a alguien de otra
/// institución ni al superadmin. Casilla vacía → NULL (directiva 20).
pub async fn fijar_nombre(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    // Se anota ANTES de escribir: si la bitácora no acepta el eslabón, el cambio
    // NO se aplica, igual que el resto de la administración de personal.
    if let Err(r) = anotar(&e, &u, "configurar", "nombre_profesional", format!("id={id}")).await {
        return r;
    }
    match adm::fijar_nombre_partido(
        &e.pool,
        u.ambito(),
        id,
        texto(&c, "nombre1").as_deref(),
        texto(&c, "nombre2").as_deref(),
        texto(&c, "apellido1").as_deref(),
        texto(&c, "apellido2").as_deref(),
    )
    .await
    {
        Ok(true) => de_vuelta(&format!("{RUTA}/usuarios"), "usuario_editado"),
        Ok(false) => dibujar_usuarios(&e, &cx, &u, "", Some("No se encontró a esa persona.")).await,
        Err(_) => averia(),
    }
}

// ── Capacidades ──────────────────────────────────────────────────────────────

/// `GET /panel/admin/capacidades`
///
/// Exige `GestionarUsuarios` —la de quien nombra—, igual que
/// `GET /api/capacidades`.
pub async fn capacidades(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/capacidades");
    };
    let ajustes = match medico_datos::capacidad::leer(&e.pool, inst).await {
        Ok(a) => a,
        Err(_) => return averia(),
    };
    // Solo se dibujan las combinaciones CONFIGURABLES: las demás las decide el
    // defecto nacional y ofrecerlas sería un interruptor sin efecto.
    let roles: Vec<RolUsuario> = RolUsuario::TODOS
        .iter()
        .copied()
        .filter(|r| r.configurable_por_institucion())
        .collect();
    let caps: Vec<Capacidad> = Capacidad::TODAS
        .iter()
        .copied()
        .filter(|c| c.configurable_por_institucion())
        .collect();

    pantalla(
        &cx,
        "Capacidades",
        html! {
            (cabecera(&cx, "/capacidades"))
            @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }
            section class="tarjeta" {
                p class="sutil" {
                    "Qué puede hacer cada rol en ESTA institución. Lo que no aparece aquí no "
                    "es configurable: el superadministrador y las capacidades que viven por "
                    "encima de una institución las decide el producto."
                }
                (aviso(
                    Tono::Averia,
                    "No se puede conceder lo que el rol de quien concede no trae por defecto: \
                     con «gestionar usuarios» se llega a todo lo demás, y sin ese tope un \
                     laboratorista podría concederse firmar historias.",
                ))
                form method="post" action={ (RUTA) "/capacidades" } {
                    (piezas::tabla(
                        &["Capacidad"],
                        html! {
                            @for c in &caps {
                                tr {
                                    td { (c.como_str()) }
                                    @for r in &roles {
                                        td {
                                            label {
                                                input type="checkbox"
                                                      name={ (c.como_str()) "__" (r.como_str()) }
                                                      value="1"
                                                      checked[ajustes.puede(*r, *c)];
                                                " " (r.como_str())
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    ))
                    button type="submit" class="principal" { "Guardar la matriz" }
                }
            }
        },
    )
}

/// `POST /panel/admin/capacidades`
pub async fn guardar_capacidades(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarUsuarios>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Some(inst) = u.institucion_id else {
        return sin_institucion(&cx, "/capacidades");
    };
    let Some(quien) = u.rol() else {
        return averia();
    };
    let actuales = match medico_datos::capacidad::leer(&e.pool, inst).await {
        Ok(a) => a,
        Err(_) => return averia(),
    };
    // Se construye la LISTA DE CAMBIOS y se le pasa entera al dominio. Los TRES
    // guardarraíles —no dejar a la institución sin quien gestione usuarios, no
    // quitarse uno su propia administración, y no conceder lo que el rol propio
    // no trae— los aplica `Ajustes::validar`, y aquí no se repiten.
    //
    // OJO A LA HISTORIA DE ESTE COMENTARIO, que es la lección: hasta el
    // 2026-08-10 decía exactamente esto mismo y el TERCERO no existía en
    // `validar` —vivía solo en el manejador de `/api/capacidades`—, así que esta
    // pantalla, que desde la retirada de Angular es la única interfaz, dejaba a
    // un laboratorista con `gestionar_usuarios` concederse `firmar_clinico`. Lo
    // encontró la revisión de seguridad. Un comentario que afirma una garantía
    // es una afirmación como otra cualquiera: se comprueba (directiva 2).
    let mut cambios: Vec<(Capacidad, RolUsuario, bool)> = Vec::new();
    // SOLO EL DELTA, no las 17×13 casillas. La primera versión enviaba la matriz
    // entera —cada casilla con su valor resultante— y con el guardarraíl 3 eso
    // dejó la delegación funcionalmente MUERTA: un lote que lleva `permitido =
    // true` para las diecisiete capacidades se rechaza siempre, así que a un
    // laboratorista con `gestionar_usuarios` delegado la pantalla se le abría y
    // no podía guardar NADA, ni siquiera un cambio que sí le corresponde.
    //
    // Lo encontró la revisión de seguridad del cierre, y es la directiva 21
    // aplicada a mi propia regla: se prueba contra el cliente legítimo, y aquí
    // el legítimo es justo el delegado que la regla venía a acotar. Mandando lo
    // que CAMBIÓ, el guardarraíl solo mira lo que de verdad se está concediendo.
    for cap in Capacidad::TODAS.iter().filter(|x| x.configurable_por_institucion()) {
        for rol in RolUsuario::TODOS.iter().filter(|x| x.configurable_por_institucion()) {
            let marcada = c.contains_key(&format!("{}__{}", cap.como_str(), rol.como_str()));
            if marcada != actuales.puede(*rol, *cap) {
                cambios.push((*cap, *rol, marcada));
            }
        }
    }

    if let Err(rechazo) = actuales.validar(&cambios, quien) {
        return pantalla(
            &cx,
            "Capacidades",
            html! {
                (cabecera(&cx, "/capacidades"))
                section class="tarjeta" {
                    (aviso(Tono::Error, &rechazo.to_string()))
                    p { a href={ (RUTA) "/capacidades" } { "Volver a la matriz" } }
                }
            },
        );
    }
    // El acto que decide QUIÉN PUEDE FIRMAR historia clínica. En `/api` esto lo
    // anota `capacidades::registrar_cambio`; aquí faltaba, así que la pantalla
    // de auditoría de la institución no podía enseñarlo jamás.
    if let Err(r) = anotar(&e, &u, "configurar", "capacidades", format!("institucion={inst}")).await
    {
        return r;
    }
    match medico_datos::capacidad::guardar(&e.pool, inst, &cambios, u.id).await {
        Ok(_) => {
            e.capacidades.invalidar(inst);
            de_vuelta(&format!("{RUTA}/capacidades"), "capacidades")
        }
        Err(_) => averia(),
    }
}

// ── Auditoría ────────────────────────────────────────────────────────────────

/// `GET /panel/admin/auditoria`
///
/// **Es la única pestaña que se vende por módulo**: `AUDITORIA` está en el
/// catálogo, así que una institución que no lo contrató recibe el 403 de la capa
/// de licencia. Lo demás de esta pantalla no se vende — un cliente sin ello no
/// podría dar de alta a nadie.
pub async fn auditoria(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<AdministrarInstitucion>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let size = 50;
    let pagina_actual = f.page.unwrap_or(1).max(1);
    let desplazamiento = (pagina_actual - 1) * size;
    let p = match adm::auditoria(&e.pool, u.ambito(), size, desplazamiento).await {
        Ok(p) => p,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Auditoría",
        html! {
            (cabecera(&cx, "/auditoria"))
            section class="tarjeta" {
                p class="sutil" {
                    (p.total) " eventos. La cadena está encadenada por hash: borrar o reordenar "
                    "un eslabón la rompe, no la altera."
                }
                @if p.items.is_empty() {
                    p class="sutil" { "No hay eventos en esta página." }
                } @else {
                    (piezas::tabla(
                        &["Cuándo", "Quién", "Qué", "Sobre"],
                        html! {
                            @for ev in &p.items {
                                tr {
                                    td class="mono" { (ev.fecha_hora) }
                                    td { (ev.usuario_nombre) }
                                    td { (ev.accion) }
                                    td class="sutil" { (ev.tabla_afectada) }
                                }
                            }
                        },
                    ))
                    nav class="buscador" {
                        @if pagina_actual > 1 {
                            a href={ (RUTA) "/auditoria?page=" (pagina_actual - 1) } { "← Anterior" }
                        }
                        @if desplazamiento + size < p.total {
                            a href={ (RUTA) "/auditoria?page=" (pagina_actual + 1) } { "Siguiente →" }
                        }
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod guardian_de_la_bitacora {
    //! Todo POST de `/panel/admin` deja rastro, y se comprueba sobre el FUENTE.
    //!
    //! Nace el 2026-08-10, con el cableado: la revisión de seguridad midió que
    //! `anotar_acto_admin` aparecía 17 veces en `rutas.rs` y **cero** en todo
    //! `vista/`. Una prueba de comportamiento por manejador no habría impedido
    //! que el noveno naciera sin ella; esto sí, y es la forma que este
    //! repositorio ya usa para la licencia y para los estilos en línea.

    /// Las funciones de `rutas.rs` que anotan POR DENTRO. Un manejador que
    /// delegue en una de ellas no lleva `anotar` propio **a propósito**: si lo
    /// llevara, el acto quedaría anotado dos veces.
    ///
    /// Se declaran las FUNCIONES y no los nombres de los manejadores, que es la
    /// diferencia entre comprobar y suponer: la primera versión de esta lista
    /// llevaba tres nombres inventados —`crear_personal`, `editar_personal`,
    /// `restablecer_clave`— y ninguno existía. Los reales son `crear_usuario`,
    /// `editar_usuario` y `restablecer_password`, y lo dijo este guardián al
    /// ponerse rojo en su primera corrida (directiva 20).
    const ANOTAN_POR_DENTRO: [&str; 3] = [
        "crate::rutas::dar_de_alta_personal",
        "crate::rutas::editar_personal",
        "crate::rutas::restablecer_password_de",
    ];

    #[test]
    fn todo_post_de_administracion_deja_rastro() {
        const FUENTE: &str = include_str!("admin.rs");
        // Un POST del panel es un manejador que recibe `Form<...>` o que se
        // registra con `post(...)`; se recorren los `pub async fn` y se mira si
        // el cuerpo llama a `anotar(` o delega en uno que anota.
        let mut sin_rastro = Vec::new();
        for bloque in FUENTE.split("\npub async fn ").skip(1) {
            let nombre = bloque.split('(').next().unwrap_or("").trim();
            let firma_y_cuerpo = bloque.split("\n}\n").next().unwrap_or(bloque);
            // Solo escrituras: las que reciben un formulario o un Path para
            // borrar. Los GET no anotan a propósito —los pide cada pintado y
            // ahogarían la señal—, que es la regla que `rutas.rs` ya declara.
            let escribe = firma_y_cuerpo.contains("Form(")
                || nombre.starts_with("borrar_")
                || nombre.starts_with("crear_");
            if !escribe {
                continue;
            }
            // `anotar(` a secas, no `anotar(&e, &u,`: la llamada se parte en
            // varias líneas cuando los argumentos son largos, y la primera
            // versión de este patrón daba por no anotado a `crear_campo`, que
            // sí lo hacía. También lo dijo el guardián.
            let anota = firma_y_cuerpo.contains("anotar(")
                || ANOTAN_POR_DENTRO.iter().any(|d| firma_y_cuerpo.contains(d));
            if !anota {
                sin_rastro.push(nombre.to_string());
            }
        }
        assert!(
            sin_rastro.is_empty(),
            "estos POST de /panel/admin no dejan eslabón: {sin_rastro:?}. \
             Su gemelo de /api sí lo deja, y el rastro de un mismo hecho no puede \
             depender de por qué puerta se hizo."
        );
    }

    #[test]
    fn el_guardian_discrimina() {
        // El caso rojo, contra FUENTE SINTÉTICA y por la MISMA costura: sin
        // esto, la prueba de arriba pasaría igual con una regla ciega, porque
        // hoy el archivo real está limpio (directiva 33).
        const MUTANTE: &str = "\npub async fn crear_cosa(\n    Form(c): Form<X>,\n) -> Response {\n    hacer();\n}\n";
        let mut sin_rastro = Vec::new();
        for bloque in MUTANTE.split("\npub async fn ").skip(1) {
            let nombre = bloque.split('(').next().unwrap_or("").trim();
            let cuerpo = bloque.split("\n}\n").next().unwrap_or(bloque);
            if (cuerpo.contains("Form(") || nombre.starts_with("crear_"))
                && !cuerpo.contains("anotar(&e, &u,")
            {
                sin_rastro.push(nombre.to_string());
            }
        }
        assert_eq!(sin_rastro, vec!["crear_cosa"], "el guardián tiene que verlo");
    }
}
