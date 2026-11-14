// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Pacientes: el listado y la ficha. **La primera pantalla de trabajo real del
//! panel en `maud`**, y por eso lleva escrito lo que las 27 siguientes copian.
//!
//! # De dónde salen los datos
//!
//! De `medico_datos::paciente`, con el `Ambito` de quien mira — las MISMAS
//! funciones que atienden `/api/pacientes`, no una consulta paralela. El
//! aislamiento entre instituciones sigue siendo el de siempre: `listar` acota
//! por `institucion_id` y `obtener` devuelve `None` para lo ajeno, que aquí se
//! dibuja como **404 y nunca 403** (ver abajo).
//!
//! # La bitácora de la Ley 1581 se anota igual que en la API
//!
//! Abrir la ficha de un paciente es un acceso a dato clínico y deja eslabón, con
//! la misma forma (`Objeto::Recurso`) que `GET /api/pacientes/{id}`. Que el
//! HTML lo teja el servidor no lo convierte en otra cosa: si el panel no
//! anotara, migrar una pantalla BORRARÍA rastro que hoy existe, y eso no se ve
//! en ninguna prueba de pantalla.
//!
//! # Sin htmx todavía, y es deliberado
//!
//! El listado es un `<form method="get">` normal. htmx entra cuando haya una
//! interacción que lo pida —el filtro incremental, un borrado con confirmación—,
//! no por estar en el stack: el estado vive en el servidor y una recarga de
//! página es una respuesta correcta.

use axum::extract::{Path, Query, State};
use chrono::Datelike;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use medico_datos::paciente::{self, Paciente, PacienteNuevo};
use medico_dominio::ihce::terminologia;

use crate::auth::{Autorizado, EscribirPaciente, Estado};
use crate::vista::{averia, contexto, pagina, Contexto, EnSesion};

#[derive(Deserialize, Default)]
pub struct Filtro {
    /// Texto libre: nombre, apellido o documento.
    #[serde(default)]
    pub q: String,
}

/// Los años cumplidos, con la zona del PRESTADOR y no la del reloj de la
/// máquina (`medico_dominio::zona`). En el VPS, que corre en UTC, «hoy» a las
/// 19:00 de Medellín ya es mañana — y con un cumpleaños de por medio eso es un
/// año de diferencia en la edad que ve el clínico.
pub(crate) fn edad(nacimiento: &str) -> Option<i64> {
    let n = chrono::NaiveDate::parse_from_str(nacimiento, "%Y-%m-%d").ok()?;
    let hoy = medico_dominio::zona::hoy();
    let mut anios = hoy.years_since(n)? as i64;
    // `years_since` ya devuelve años cumplidos; el ajuste sobra y se deja
    // explícito para que nadie lo "arregle" restando años a mano.
    if anios < 0 {
        anios = 0;
    }
    Some(anios)
}

/// Los MESES cumplidos, con la misma zona del prestador que [`edad`].
///
/// Existe aparte y no como «edad × 12» porque son dos resoluciones de la misma
/// medida y una no se deriva de la otra sin perder información: un lactante de
/// siete meses tiene 0 años, y la referencia antropométrica del original trae
/// una fila por CADA mes del primer año. Redondear a años ahí devolvería la
/// talla del recién nacido para un niño que ya camina.
pub(crate) fn edad_meses(nacimiento: &str) -> Option<i64> {
    let n = chrono::NaiveDate::parse_from_str(nacimiento, "%Y-%m-%d").ok()?;
    let hoy = medico_dominio::zona::hoy();
    if hoy < n {
        return None;
    }
    let mut meses = (hoy.year() as i64 - n.year() as i64) * 12
        + (hoy.month() as i64 - n.month() as i64);
    // El mes solo cuenta si ya se cumplió el día: del 15 de marzo al 14 de
    // abril NO ha pasado un mes, aunque el número del mes haya cambiado.
    if hoy.day() < n.day() {
        meses -= 1;
    }
    Some(meses.max(0))
}

/// Nombre y apellidos, saltándose los vacíos.
///
/// El orden es el del RIPS —nombre1, nombre2, apellido1, apellido2— y los
/// espacios se colapsan: una fila con `nombre2` en blanco no puede salir con dos
/// espacios en medio, porque es el nombre que el clínico lee y compara.
///
/// Es `pub(crate)` desde que el despacho de ambulancias tiene que nombrar al
/// paciente en su selector: una segunda copia del mismo criterio saldría con
/// otro orden o con otro espaciado, y entonces el mismo paciente se leería
/// distinto en dos pantallas (directiva 9). Vive aquí porque es del concepto
/// «paciente», no del armazón.
pub(crate) fn nombre_completo(p: &medico_datos::paciente::Paciente) -> String {
    [
        Some(p.nombre1.as_str()),
        p.nombre2.as_deref(),
        Some(p.apellido1.as_str()),
        p.apellido2.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|x| !x.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// `GET /panel/pacientes` — el listado, con búsqueda.
///
/// **El filtro baja a SQL desde el 2026-08-08.** Antes se aplicaba en memoria
/// sobre el padrón entero, con una nota que decía que era una decisión con fecha
/// de caducidad porque `paciente::listar` no aceptaba criterio. La fecha llegó:
/// `paciente::buscar` lo acepta, y de paso la búsqueda gana lo que el filtro en
/// memoria no tenía —pliega tildes y admite varias palabras, así que «maria
/// paez» encuentra a «María Fernanda Páez Gómez»—.
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let aguja = f.q.trim();
    let hallazgo = match medico_datos::paciente::buscar(
        &e.pool,
        u.ambito(),
        &medico_datos::paciente::Criterio {
            buscar: (!aguja.is_empty()).then_some(aguja),
            // El LISTADO no lleva techo: es la pantalla del padrón, y aquí
            // enseñar «los primeros 20» sería el defecto que se acaba de quitar
            // de los desplegables. Quien necesita elegir uno para un formulario
            // usa el buscador acotado (`buscador_paciente`); quien viene aquí
            // viene a ver el padrón.
            limite: None,
        },
    )
    .await
    {
        Ok(h) => h,
        Err(_) => return averia(),
    };
    let filtrados: Vec<_> = hallazgo.filas.iter().collect();

    // Cuántos hay en TOTAL, sin la aguja. Es una consulta aparte y no se deriva
    // de la de arriba porque son dos preguntas distintas: «cuántos casan» y
    // «cuántos hay». Con la aguja puesta, la segunda es la que distingue «no hay
    // ninguno» de «tu búsqueda no encontró», que confundidas hacen pensar que se
    // perdieron los datos. `LIMIT 0` trae la cuenta sin traer ninguna fila.
    let padron = if aguja.is_empty() {
        hallazgo.total
    } else {
        match medico_datos::paciente::buscar(
            &e.pool,
            u.ambito(),
            &medico_datos::paciente::Criterio { buscar: None, limite: Some(0) },
        )
        .await
        {
            Ok(h) => h.total,
            Err(_) => return averia(),
        }
    };

    pagina(
        "Pacientes",
        Some(crate::vista::panel::menu(&cx,"/panel/pacientes")),
        html! {
            header class="titulo-seccion" {
                h1 { "Pacientes" }
                form method="get" action="/panel/pacientes" class="buscador" {
                    label class="oculto" for="q" { "Buscar" }
                    input #q type="search" name="q" value=(f.q)
                          placeholder="Nombre o documento";
                    button type="submit" { "Buscar" }
                }
                a class="boton" href="/panel/pacientes/nuevo" { "+ Nuevo paciente" }
            }

            @if filtrados.is_empty() {
                section class="tarjeta vacio" {
                    @if padron == 0 {
                        p { "Todavía no hay pacientes registrados." }
                    } @else {
                        // Las dos frases son distintas a propósito: «no hay
                        // ninguno» y «tu búsqueda no encontró» significan cosas
                        // distintas, y confundirlas hace pensar que se perdieron
                        // los datos.
                        p { "Ningún paciente coincide con «" (f.q) "»." }
                        p { a href="/panel/pacientes" { "Ver todos" } }
                    }
                }
            } @else {
                p class="sutil" {
                    (filtrados.len())
                    @if filtrados.len() == 1 { " paciente" } @else { " pacientes" }
                    @if !aguja.is_empty() { " de " (padron) }
                }
                div class="tabla-envoltura" {
                    table class="tabla" {
                        thead {
                            tr {
                                th { "Documento" }
                                th { "Nombre" }
                                th { "Edad" }
                                th { "Sexo" }
                                th { "Celular" }
                            }
                        }
                        tbody {
                            @for p in &filtrados {
                                tr {
                                    td class="mono" { (p.tipo_documento) " " (p.numero_documento) }
                                    td {
                                        a href={ "/panel/pacientes/" (p.id) } {
                                            (nombre_completo(p))
                                        }
                                    }
                                    td {
                                        @match edad(&p.fecha_nacimiento) {
                                            // Una fecha ilegible NO se dibuja
                                            // como «0 años»: eso es un dato
                                            // falso en una pantalla clínica.
                                            Some(a) => { (a) }
                                            None => span class="sutil" { "—" },
                                        }
                                    }
                                    td { (p.sexo) }
                                    td { (p.celular.clone().unwrap_or_default()) }
                                }
                            }
                        }
                    }
                }
            }
        },
    )
    .into_response()
}

/// `GET /panel/pacientes/{id}` — la ficha.
///
/// **404 y nunca 403 para el paciente de otra institución.** Un 403 confirmaría
/// que ese id existe, y probando números se enumera el padrón del vecino: eso
/// también es un dato de salud. Es la misma regla que ejerce la API, y la
/// pantalla no la relaja por ser más amable.
pub async fn ficha(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let p = match medico_datos::paciente::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => p,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };

    // El acceso a dato de paciente deja eslabón, y **con los MISMOS argumentos**
    // que `GET /api/pacientes/{id}`: `("ver", "paciente", id, id)`. No es un
    // detalle de estilo — el contenido del eslabón está congelado byte a byte
    // (`el_contenido_canonico_del_recurso_unico_esta_congelado`), así que un
    // verbo distinto aquí partiría el rastro de un mismo hecho en dos según qué
    // pantalla lo hubiera abierto, y las cadenas ya selladas seguirían
    // verificando sin que nada fallara.
    //
    // Best-effort, como en toda la aplicación: si la bitácora falla se grita en
    // el log y NO se bloquea la atención. Pero el intento tiene que estar aquí,
    // o migrar la pantalla BORRARÍA rastro que hoy existe — y eso no lo ve
    // ninguna prueba de pantalla.
    crate::rutas::registrar_acceso(&e, &u, "ver", "paciente", p.id, p.id).await;

    pagina(
        &nombre_completo(&p),
        Some(crate::vista::panel::menu(&cx,"/panel/pacientes")),
        html! {
            nav class="migas" {
                a href="/panel/pacientes" { "← Pacientes" }
            }
            section class="tarjeta" {
                h1 { (nombre_completo(&p)) }
                dl class="datos" {
                    dt { "Documento" }
                    dd class="mono" { (p.tipo_documento) " " (p.numero_documento) }
                    dt { "Fecha de nacimiento" }
                    dd {
                        (p.fecha_nacimiento)
                        @if let Some(a) = edad(&p.fecha_nacimiento) {
                            " (" (a) " años)"
                        }
                    }
                    dt { "Sexo" }
                    dd { (p.sexo) }
                    @if let Some(c) = &p.celular {
                        dt { "Celular" } dd { (c) }
                    }
                    @if let Some(c) = &p.email {
                        dt { "Correo" } dd { (c) }
                    }
                    @if let Some(d) = &p.direccion {
                        dt { "Dirección" } dd { (d) }
                    }
                    // Demografía del PatientRDA (#84/#603). El código se traduce a
                    // su `display` OFICIAL del catálogo; si el catálogo no lo
                    // conoce se muestra el código crudo en vez de inventar un
                    // rótulo (directiva 20).
                    @if let Some(n) = &p.nacionalidad {
                        dt { "Nacionalidad" } dd class="mono" { (n) }
                    }
                    @if let Some(c) = &p.pertenencia_etnica {
                        dt { "Pertenencia étnica" }
                        dd { (terminologia::display(terminologia::SISTEMA_ETNIA, c).unwrap_or(c.as_str())) }
                    }
                    @if let Some(c) = &p.comunidad_etnica {
                        dt { "Comunidad étnica" } dd { (c) }
                    }
                    @if let Some(c) = &p.discapacidad {
                        dt { "Discapacidad" }
                        dd { (terminologia::display(terminologia::SISTEMA_DISCAPACIDAD, c).unwrap_or(c.as_str())) }
                    }
                    @if let Some(c) = &p.identidad_genero {
                        dt { "Identidad de género" }
                        dd { (terminologia::display(terminologia::SISTEMA_IDENTIDAD_GENERO, c).unwrap_or(c.as_str())) }
                    }
                }
                p { a class="boton" href={ "/panel/pacientes/" (p.id) "/editar" } {
                    "Editar paciente"
                } }
            }
            section class="tarjeta" {
                h2 { "Historia clínica" }
                p class="sutil" {
                    "Todavía en la aplicación anterior mientras dura la migración."
                }
                p { a class="boton" href={ "/historias?paciente=" (p.id) } {
                    "Abrir historias ↗"
                } }
            }
        },
    )
    .into_response()
}

// ─────────────────────────── Alta y edición (#84) ───────────────────────────
//
// La captura del paciente en `maud`. Hasta el 2026-08-22 la ficha era de solo
// lectura y un paciente solo nacía por la API; esto le da pantalla, con la
// demografía del RDA (#603) incluida. La capa de datos —aislamiento, procedencia
// `confirmado` de lo tecleado, derivación del municipio— ya la resuelven
// `paciente::crear`/`actualizar`; aquí va el formulario y su validación de borde.

/// `""` (o solo espacios) se guarda como ausencia, no como cadena vacía: un campo
/// opcional en blanco es `None`, no `Some("")` (directiva 20).
fn opcional(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Los valores que pinta el formulario, sin importar si vienen de un paciente ya
/// guardado (edición) o del envío que no pasó la validación (para no perder lo
/// tecleado). Los tres de catálogo son `Option` porque su ausencia es «sin
/// especificar», distinta de un código vacío.
#[derive(Default)]
struct Valores<'a> {
    tipo_documento: &'a str,
    numero_documento: &'a str,
    nombre1: &'a str,
    nombre2: &'a str,
    apellido1: &'a str,
    apellido2: &'a str,
    fecha_nacimiento: &'a str,
    sexo: &'a str,
    celular: &'a str,
    email: &'a str,
    direccion: &'a str,
    ciudad_residencia: &'a str,
    departamento_residencia: &'a str,
    nacionalidad: &'a str,
    pertenencia_etnica: Option<&'a str>,
    comunidad_etnica: &'a str,
    discapacidad: Option<&'a str>,
    identidad_genero: Option<&'a str>,
}

impl<'a> Valores<'a> {
    fn de_paciente(p: &'a Paciente) -> Self {
        Valores {
            tipo_documento: &p.tipo_documento,
            numero_documento: &p.numero_documento,
            nombre1: &p.nombre1,
            nombre2: p.nombre2.as_deref().unwrap_or(""),
            apellido1: &p.apellido1,
            apellido2: p.apellido2.as_deref().unwrap_or(""),
            fecha_nacimiento: &p.fecha_nacimiento,
            sexo: &p.sexo,
            celular: p.celular.as_deref().unwrap_or(""),
            email: p.email.as_deref().unwrap_or(""),
            direccion: p.direccion.as_deref().unwrap_or(""),
            ciudad_residencia: p.ciudad_residencia.as_deref().unwrap_or(""),
            departamento_residencia: p.departamento_residencia.as_deref().unwrap_or(""),
            nacionalidad: p.nacionalidad.as_deref().unwrap_or(""),
            pertenencia_etnica: p.pertenencia_etnica.as_deref(),
            comunidad_etnica: p.comunidad_etnica.as_deref().unwrap_or(""),
            discapacidad: p.discapacidad.as_deref(),
            identidad_genero: p.identidad_genero.as_deref(),
        }
    }

    fn de_formulario(f: &'a FormPaciente) -> Self {
        let algo = |s: &'a str| (!s.trim().is_empty()).then_some(s);
        Valores {
            tipo_documento: &f.tipo_documento,
            numero_documento: &f.numero_documento,
            nombre1: &f.nombre1,
            nombre2: &f.nombre2,
            apellido1: &f.apellido1,
            apellido2: &f.apellido2,
            fecha_nacimiento: &f.fecha_nacimiento,
            sexo: &f.sexo,
            celular: &f.celular,
            email: &f.email,
            direccion: &f.direccion,
            ciudad_residencia: &f.ciudad_residencia,
            departamento_residencia: &f.departamento_residencia,
            nacionalidad: &f.nacionalidad,
            pertenencia_etnica: algo(&f.pertenencia_etnica),
            comunidad_etnica: &f.comunidad_etnica,
            discapacidad: algo(&f.discapacidad),
            identidad_genero: algo(&f.identidad_genero),
        }
    }
}

/// Un `<select>` cuyas opciones SALEN del CodeSystem (`terminologia::opciones`),
/// no de una lista escrita a mano: ofrecer un código que el servidor nacional no
/// reconoce dejaría teclear algo que después se rechaza.
fn selector(nombre: &str, etiqueta: &str, sistema: &str, actual: Option<&str>) -> Markup {
    html! {
        label {
            (etiqueta)
            select name=(nombre) {
                option value="" selected[actual.is_none()] { "— sin especificar —" }
                @for (cod, disp) in terminologia::opciones(sistema) {
                    option value=(cod) selected[actual == Some(cod)] { (disp) " (" (cod) ")" }
                }
            }
        }
    }
}

/// El formulario, reutilizado por alta y edición. `error` pinta un aviso arriba.
fn campos_formulario(accion: &str, boton: &str, v: &Valores, error: Option<&str>) -> Markup {
    html! {
        @if let Some(e) = error {
            p class="aviso error" { (e) }
        }
        form method="post" action=(accion) class="apilado" {
            fieldset {
                legend { "Identificación" }
                label {
                    "Tipo de documento"
                    // Los trece que el validador admite para el USUARIO (incluye
                    // los de menor y sin identificar), del maestro empotrado.
                    select name="tipo_documento" required {
                        option value="" selected[v.tipo_documento.is_empty()] { "— elija —" }
                        @for (cod, nom) in medico_dominio::muv::tipos_documento_usuario() {
                            option value=(cod) selected[v.tipo_documento == cod] { (nom) " (" (cod) ")" }
                        }
                    }
                }
                label {
                    "Número de documento"
                    input type="text" name="numero_documento" value=(v.numero_documento) required;
                }
            }
            fieldset {
                legend { "Nombre" }
                label { "Primer nombre" input type="text" name="nombre1" value=(v.nombre1) required; }
                label { "Segundo nombre" input type="text" name="nombre2" value=(v.nombre2); }
                label { "Primer apellido" input type="text" name="apellido1" value=(v.apellido1) required; }
                label { "Segundo apellido" input type="text" name="apellido2" value=(v.apellido2); }
            }
            fieldset {
                legend { "Datos básicos" }
                label { "Fecha de nacimiento" input type="date" name="fecha_nacimiento" value=(v.fecha_nacimiento) required; }
                label {
                    "Sexo"
                    select name="sexo" required {
                        option value="" selected[v.sexo.is_empty()] { "— elija —" }
                        option value="M" selected[v.sexo == "M"] { "Masculino" }
                        option value="F" selected[v.sexo == "F"] { "Femenino" }
                    }
                }
                label { "Celular" input type="text" name="celular" value=(v.celular); }
                label { "Correo" input type="email" name="email" value=(v.email); }
                label { "Dirección" input type="text" name="direccion" value=(v.direccion); }
                label { "Ciudad de residencia" input type="text" name="ciudad_residencia" value=(v.ciudad_residencia); }
                label { "Departamento de residencia" input type="text" name="departamento_residencia" value=(v.departamento_residencia); }
                p class="sutil" {
                    "El código de municipio se DERIVA de la ciudad y se marca «derivado» "
                    "hasta que alguien lo confirme; no se teclea aquí."
                }
            }
            fieldset {
                legend { "Demografía (Resumen Digital de Atención — RDA)" }
                label {
                    "Nacionalidad "
                    span class="sutil" { "(código ISO 3166-1, p. ej. 170 = Colombia)" }
                    input type="text" name="nacionalidad" value=(v.nacionalidad) inputmode="numeric";
                }
                (selector("pertenencia_etnica", "Pertenencia étnica", terminologia::SISTEMA_ETNIA, v.pertenencia_etnica))
                label { "Comunidad étnica " span class="sutil" { "(texto libre)" }
                        input type="text" name="comunidad_etnica" value=(v.comunidad_etnica); }
                (selector("discapacidad", "Discapacidad", terminologia::SISTEMA_DISCAPACIDAD, v.discapacidad))
                (selector("identidad_genero", "Identidad de género", terminologia::SISTEMA_IDENTIDAD_GENERO, v.identidad_genero))
            }
            div class="acciones" {
                button type="submit" { (boton) }
                a class="boton secundario" href="/panel/pacientes" { "Cancelar" }
            }
        }
    }
}

/// El envío del formulario. Todo `String` porque un formulario HTML manda `""`,
/// no ausencia; [`FormPaciente::a_nuevo`] traduce `""` → `None`. `procedencias`
/// NO está aquí: lo pone el servidor, nunca el cliente.
#[derive(Deserialize, Default)]
pub struct FormPaciente {
    #[serde(default)] tipo_documento: String,
    #[serde(default)] numero_documento: String,
    #[serde(default)] nombre1: String,
    #[serde(default)] nombre2: String,
    #[serde(default)] apellido1: String,
    #[serde(default)] apellido2: String,
    #[serde(default)] fecha_nacimiento: String,
    #[serde(default)] sexo: String,
    #[serde(default)] celular: String,
    #[serde(default)] email: String,
    #[serde(default)] direccion: String,
    #[serde(default)] ciudad_residencia: String,
    #[serde(default)] departamento_residencia: String,
    #[serde(default)] nacionalidad: String,
    #[serde(default)] pertenencia_etnica: String,
    #[serde(default)] comunidad_etnica: String,
    #[serde(default)] discapacidad: String,
    #[serde(default)] identidad_genero: String,
}

impl FormPaciente {
    /// El primer campo OBLIGATORIO que falta, o `None` si están todos. Se valida
    /// en el servidor y no solo con el `required` del HTML: el `required` lo
    /// obedece un navegador y lo ignora cualquier cliente que hable HTTP.
    fn faltan(&self) -> Option<&'static str> {
        if self.tipo_documento.trim().is_empty() { return Some("Falta el tipo de documento."); }
        if self.numero_documento.trim().is_empty() { return Some("Falta el número de documento."); }
        if self.nombre1.trim().is_empty() { return Some("Falta el primer nombre."); }
        if self.apellido1.trim().is_empty() { return Some("Falta el primer apellido."); }
        if self.fecha_nacimiento.trim().is_empty() { return Some("Falta la fecha de nacimiento."); }
        if self.sexo.trim().is_empty() { return Some("Falta el sexo."); }
        None
    }

    /// Arma el `PacienteNuevo`. `base` son los campos que ESTA pantalla no
    /// captura (`tipo_usuario`, `cod_pais`, `zona`, `entidad_id`, `cod_municipio`):
    /// al EDITAR se conservan del registro existente, porque `actualizar`
    /// reescribe TODAS las columnas y mandarlos en `None` los borraría (directiva
    /// 20). Al CREAR, `base` es `None` y nacen vacíos; el municipio lo deriva
    /// después `crear` desde la ciudad.
    fn a_nuevo(&self, base: Option<&Paciente>) -> PacienteNuevo {
        PacienteNuevo {
            tipo_documento: self.tipo_documento.trim().to_string(),
            numero_documento: self.numero_documento.trim().to_string(),
            nombre1: self.nombre1.trim().to_string(),
            nombre2: opcional(&self.nombre2),
            apellido1: self.apellido1.trim().to_string(),
            apellido2: opcional(&self.apellido2),
            fecha_nacimiento: self.fecha_nacimiento.trim().to_string(),
            sexo: self.sexo.trim().to_string(),
            celular: opcional(&self.celular),
            email: opcional(&self.email),
            direccion: opcional(&self.direccion),
            ciudad_residencia: opcional(&self.ciudad_residencia),
            departamento_residencia: opcional(&self.departamento_residencia),
            nacionalidad: opcional(&self.nacionalidad),
            pertenencia_etnica: opcional(&self.pertenencia_etnica),
            comunidad_etnica: opcional(&self.comunidad_etnica),
            discapacidad: opcional(&self.discapacidad),
            identidad_genero: opcional(&self.identidad_genero),
            // Preservados del existente al editar; None al crear.
            cod_municipio: base.and_then(|b| b.cod_municipio.clone()),
            tipo_usuario: base.and_then(|b| b.tipo_usuario.clone()),
            cod_pais: base.and_then(|b| b.cod_pais.clone()),
            zona: base.and_then(|b| b.zona.clone()),
            entidad_id: base.and_then(|b| b.entidad_id),
        }
    }
}

/// Dibuja la página del formulario (alta o edición, o su reintento con error).
fn pagina_formulario(
    cx: &Contexto,
    accion: &str,
    titulo: &str,
    boton: &str,
    volver: (&str, &str),
    v: &Valores,
    error: Option<&str>,
) -> Response {
    pagina(
        titulo,
        Some(crate::vista::panel::menu(cx, "/panel/pacientes")),
        html! {
            nav class="migas" { a href=(volver.0) { (volver.1) } }
            section class="tarjeta" {
                h1 { (titulo) }
                (campos_formulario(accion, boton, v, error))
            }
        },
    )
    .into_response()
}

/// `GET /panel/pacientes/nuevo` — el formulario de alta, en blanco.
pub async fn nuevo(State(e): State<Estado>, autorizado: Autorizado<EscribirPaciente>) -> Response {
    let u = autorizado.0;
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    pagina_formulario(
        &cx,
        "/panel/pacientes/nuevo",
        "Nuevo paciente",
        "Registrar",
        ("/panel/pacientes", "← Pacientes"),
        &Valores::default(),
        None,
    )
}

/// `POST /panel/pacientes/nuevo`
pub async fn crear(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirPaciente>,
    Form(f): Form<FormPaciente>,
) -> Response {
    let u = autorizado.0;
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let reintentar = |error: &str| {
        pagina_formulario(
            &cx,
            "/panel/pacientes/nuevo",
            "Nuevo paciente",
            "Registrar",
            ("/panel/pacientes", "← Pacientes"),
            &Valores::de_formulario(&f),
            Some(error),
        )
    };
    if let Some(err) = f.faltan() {
        return reintentar(err);
    }
    match paciente::crear(&e.pool, u.ambito(), &f.a_nuevo(None)).await {
        Ok(Some(id)) => Redirect::to(&format!("/panel/pacientes/{id}")).into_response(),
        // Superadmin o cuenta sin institución: crear un paciente es un acto de una
        // IPS concreta. Se dice, no se cae en silencio.
        Ok(None) => reintentar("Solo una institución puede registrar pacientes."),
        Err(_) => averia(),
    }
}

/// `GET /panel/pacientes/{id}/editar` — el formulario con los datos actuales.
pub async fn editar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirPaciente>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let p = match medico_datos::paciente::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => p,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    // Leer los datos del paciente para editarlos es un acceso a dato clínico y
    // deja el MISMO eslabón que la ficha (`ver`/`paciente`), o migrar la edición
    // al panel BORRARÍA rastro que la API sí deja.
    crate::rutas::registrar_acceso(&e, &u, "ver", "paciente", p.id, p.id).await;
    let accion = format!("/panel/pacientes/{id}/editar");
    pagina_formulario(
        &cx,
        &accion,
        "Editar paciente",
        "Guardar cambios",
        (&format!("/panel/pacientes/{id}"), "← Ficha"),
        &Valores::de_paciente(&p),
        None,
    )
}

/// `POST /panel/pacientes/{id}/editar`
pub async fn guardar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirPaciente>,
    Path(id): Path<i64>,
    Form(f): Form<FormPaciente>,
) -> Response {
    let u = autorizado.0;
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let accion = format!("/panel/pacientes/{id}/editar");
    if let Some(err) = f.faltan() {
        return pagina_formulario(
            &cx,
            &accion,
            "Editar paciente",
            "Guardar cambios",
            (&format!("/panel/pacientes/{id}"), "← Ficha"),
            &Valores::de_formulario(&f),
            Some(err),
        );
    }
    // Cargar el existente PRESERVA los campos que la pantalla no captura y a la vez
    // aísla: un id ajeno da `None` → 404, nunca 403 (la regla del módulo).
    let base = match medico_datos::paciente::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => p,
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    match paciente::actualizar(&e.pool, u.ambito(), id, &f.a_nuevo(Some(&base))).await {
        Ok(true) => Redirect::to(&format!("/panel/pacientes/{id}")).into_response(),
        Ok(false) => no_existe(&cx),
        Err(_) => averia(),
    }
}

/// La pantalla del 404. Se dibuja **igual** para «no existe» y para «es de otra
/// institución», que es justo lo que la hace servir.
fn no_existe(cx: &Contexto) -> Response {
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrado",
            Some(crate::vista::panel::menu(cx, "/panel/pacientes")),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró ese paciente" }
                    p { "Puede que el enlace esté mal o que ya no exista." }
                    p { a href="/panel/pacientes" { "Volver a Pacientes" } }
                }
            },
        ),
    )
        .into_response()
}
