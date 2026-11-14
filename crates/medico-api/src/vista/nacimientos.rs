// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Registro de nacimiento (#249): la lista y la captura.
//!
//! Es el módulo que desbloquea el arreglo `recienNacidos` de la 948 (#67). No
//! nace de un formulario del VFP —ninguno de los 22 pendientes lo tiene— sino de
//! la norma: los campos de esta pantalla son EXACTAMENTE los que el archivo
//! necesita, ni uno más.
//!
//! # No hay campos de adorno, y es deliberado
//!
//! La tentación al construir un módulo clínico nuevo es capturar todo lo que se
//! sabe del recién nacido: APGAR, talla, perímetro cefálico, hora exacta del
//! pinzamiento. Nada de eso entra hoy. Cada campo que se captura y no se usa es
//! un campo que hay que llenar en cada parto, migrar en cada versión y proteger
//! bajo la Ley 1581 para siempre — y este módulo se construye porque **la
//! facturación lo exige**, no porque tengamos un cliente que pida una historia
//! neonatal. Cuando lo haya, entra con su motivo.
//!
//! # La madre se elige de la lista de pacientes, no se escribe
//!
//! El arreglo se une al usuario de la madre por su documento, así que un nombre
//! tecleado a mano produciría un recién nacido que no se puede colgar de nadie.
//! Se elige por id, y el servidor comprueba que la madre exista **dentro de la
//! institución** antes de escribir.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::html;
use serde::Deserialize;

use medico_datos::recien_nacido::{self, Nuevo, CONDICIONES_DESTINO, TIPOS_DOCUMENTO};

use crate::auth::{Autorizado, AtenderUrgencias, Estado};
use crate::vista::{averia, contexto, pagina, piezas, aviso, Tono, EnSesion};

pub const RUTA: &str = "/panel/nacimientos";

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub desde: String,
    #[serde(default)]
    pub hasta: String,
    #[serde(default)]
    pub aviso: String,
}

fn de_vuelta(a: &str) -> Response {
    Redirect::to(a).into_response()
}

/// Los avisos que la pantalla sabe pintar. La lista es cerrada a propósito: un
/// `aviso` que llegue por la URL y no esté aquí no se pinta, para que nadie
/// pueda hacerle decir a la pantalla lo que quiera con un enlace.
fn texto_del_aviso(clave: &str) -> Option<(Tono, String)> {
    match clave {
        "registrado" => Some((
            Tono::Ok,
            "Nacimiento registrado. Ya entra en el arreglo recienNacidos del \
             RIPS del periodo."
                .to_string(),
        )),
        "" => None,
        // Todo lo demás es un rechazo del dominio, y su texto lo pone el
        // dominio: aquí no se reescribe, porque entonces habría dos versiones
        // del mismo mensaje y se desincronizarían. Una clave que no existe no
        // pinta nada.
        otra => recien_nacido::Falta::de_clave(otra)
            .map(|f| (Tono::Error, f.mensaje().to_string())),
    }
}

/// `GET /panel/nacimientos`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let desde = (!f.desde.trim().is_empty()).then(|| f.desde.trim());
    let hasta = (!f.hasta.trim().is_empty()).then(|| f.hasta.trim());
    let filas = match recien_nacido::listar(&e.pool, u.ambito(), desde, hasta).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // La lista atraviesa titulares y lleva dato clínico del recién nacido y la
    // identificación de la madre: colección SIN ids, la misma forma que la cola
    // de urgencias (Ley 1581).
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_RECIEN_NACIDOS,
        None,
        filas.len(),
        vec![],
    )
    .await;

    let puede_registrar = cx.puede(medico_dominio::rol::Capacidad::AtenderUrgenciasHospitalizacion);
    let el_aviso = texto_del_aviso(f.aviso.trim());

    pagina(
        "Registro de nacimiento",
        Some(crate::vista::panel::menu(&cx, RUTA)),
        html! {
            header class="titulo-seccion" { h1 { "Registro de nacimiento" } }

            @if let Some((tono, texto)) = el_aviso {
                section class="tarjeta" { (aviso(tono, &texto)) }
            }

            section class="tarjeta" {
                form method="get" action=(RUTA) class="filtros" {
                    label { "Desde" input type="date" name="desde" value=(f.desde); }
                    label { "Hasta" input type="date" name="hasta" value=(f.hasta); }
                    button type="submit" { "Filtrar" }
                }
            }

            @if filas.is_empty() {
                (piezas::vacio(
                    desde.is_some() || hasta.is_some(),
                    "No hay nacimientos registrados.",
                    "Ningún nacimiento en ese rango de fechas.",
                    RUTA,
                ))
            } @else {
                (piezas::tabla(
                    &["Nacimiento", "Madre", "Sem.", "Sexo", "Peso (g)", "Dx", "Destino", "Egreso"],
                    html! {
                        @for r in &filas {
                            tr {
                                td class="mono" { (r.fecha_nacimiento) }
                                td {
                                    a href={ "/panel/pacientes/" (r.madre_paciente_id) } {
                                        (r.madre_nombre)
                                    }
                                    span class="sutil" { " " (r.madre_identificacion) }
                                }
                                td class="mono" { (r.edad_gestacional) }
                                td { (r.sexo_biologico) }
                                td class="mono" { (r.peso_gramos) }
                                td class="mono" { (r.diagnostico_principal) }
                                td {
                                    @match r.condicion_destino.as_deref() {
                                        Some(c) => (etiqueta_destino(c)),
                                        // Sin destino se deja vacío: el recién
                                        // nacido sigue en el servicio. Poner
                                        // «domicilio» por defecto lo daría de
                                        // alta en el RIPS sin que nadie lo diera.
                                        None => "",
                                    }
                                }
                                td class="mono" { (r.fecha_egreso.clone().unwrap_or_default()) }
                            }
                        }
                    },
                ))
                (piezas::recuento(filas.len(), filas.len(), "nacimiento", "nacimientos"))
            }

            @if puede_registrar {
                (formulario())
            }
        },
    )
    .into_response()
}

/// El nombre largo de la condición de destino, del catálogo del MUV. Un código
/// suelto («04») no se lee; el nombre sí.
fn etiqueta_destino(codigo: &str) -> String {
    CONDICIONES_DESTINO
        .iter()
        .find(|(c, _)| *c == codigo)
        .map(|(c, n)| format!("{c} · {n}"))
        // Un código que no está en la tabla se enseña TAL CUAL y no se traduce:
        // si alguna vez entra uno por un importador, hay que verlo, no taparlo.
        .unwrap_or_else(|| codigo.to_string())
}

fn formulario() -> maud::Markup {
    html! {
        section class="tarjeta" {
            h2 { "Registrar un nacimiento" }
            p class="sutil" {
                "Los campos son los que la Resolución 948 exige para el arreglo "
                code { "recienNacidos" } ". El peso va en GRAMOS."
            }
            form method="post" action=(RUTA) class="apilado" {
                label {
                    "Madre (id de paciente)"
                    input type="number" name="madre_paciente_id" required min="1";
                }
                label { "Fecha y hora del nacimiento"
                    input type="datetime-local" name="fecha_nacimiento" required; }
                label { "Edad gestacional (semanas)"
                    input type="number" name="edad_gestacional" required min="20" max="45"; }
                label { "Consultas de control prenatal"
                    input type="number" name="num_consultas_prenatal" required min="0"; }
                label {
                    "Sexo biológico"
                    select name="sexo_biologico" required {
                        option value="" { "—" }
                        option value="M" { "Masculino" }
                        option value="F" { "Femenino" }
                        option value="I" { "Indeterminado o intersexual" }
                    }
                }
                label { "Peso (gramos)"
                    input type="number" name="peso_gramos" required min="200" max="8000"; }
                label { "Diagnóstico principal (CIE-10)"
                    input type="text" name="diagnostico_principal" required maxlength="6"; }
                label {
                    "Documento propio (opcional mientras no lo tenga)"
                    select name="tipo_documento" {
                        option value="" { "— sin documento todavía" }
                        @for t in TIPOS_DOCUMENTO {
                            option value=(t) { (etiqueta_documento(t)) }
                        }
                    }
                }
                label { "Número de documento"
                    input type="text" name="numero_documento" maxlength="20"; }
                label {
                    "Condición de destino al egreso"
                    select name="condicion_destino" {
                        option value="" { "— sigue en el servicio" }
                        @for (c, n) in CONDICIONES_DESTINO {
                            option value=(c) { (c) " · " (n) }
                        }
                    }
                }
                label { "Causa básica de muerte (solo si el destino es 02)"
                    input type="text" name="diagnostico_causa_muerte" maxlength="6"; }
                label { "Fecha y hora del egreso"
                    input type="datetime-local" name="fecha_egreso"; }
                label { "Código VIDA (certificado de nacido vivo)"
                    input type="text" name="codigo_vida" maxlength="256"; }
                button type="submit" { "Registrar nacimiento" }
            }
        }
    }
}

fn etiqueta_documento(t: &str) -> &'static str {
    match t {
        "CN" => "CN · Certificado de nacido vivo",
        "RC" => "RC · Registro civil",
        "MS" => "MS · Menor sin identificar",
        // No se puede llegar aquí desde `TIPOS_DOCUMENTO`; si alguien añade uno
        // sin etiqueta, se ve que falta en vez de salir en blanco.
        otro => Box::leak(format!("{otro} · sin etiqueta").into_boxed_str()),
    }
}

/// Lo que el formulario envía. Todo cadena: un `<input>` vacío llega como `""`,
/// no como ausente, y convertirlo aquí es lo que permite distinguir «no lo
/// pusieron» de «pusieron cero» (directiva 32).
#[derive(Deserialize)]
pub struct Formulario {
    madre_paciente_id: i64,
    fecha_nacimiento: String,
    edad_gestacional: i64,
    num_consultas_prenatal: i64,
    sexo_biologico: String,
    peso_gramos: i64,
    diagnostico_principal: String,
    #[serde(default)]
    tipo_documento: String,
    #[serde(default)]
    numero_documento: String,
    #[serde(default)]
    condicion_destino: String,
    #[serde(default)]
    diagnostico_causa_muerte: String,
    #[serde(default)]
    fecha_egreso: String,
    #[serde(default)]
    codigo_vida: String,
}

/// `""` es ausencia, no valor. Sin esto, un `<select>` en blanco guardaría la
/// cadena vacía y el `CHECK` de la migración la rechazaría con un mensaje que no
/// se le puede enseñar a nadie.
fn o_nada(s: String) -> Option<String> {
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// `POST /panel/nacimientos`
pub async fn registrar(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Form(f): Form<Formulario>,
) -> Response {
    let u = autorizado.0;
    let n = Nuevo {
        madre_paciente_id: f.madre_paciente_id,
        // La hospitalización se enlazará cuando la pantalla de hospitalización
        // ofrezca «registrar el nacimiento desde aquí». Hoy no se adivina cuál
        // es (directiva 20).
        hospitalizacion_id: None,
        tipo_documento: o_nada(f.tipo_documento),
        numero_documento: o_nada(f.numero_documento),
        fecha_nacimiento: f.fecha_nacimiento,
        edad_gestacional: f.edad_gestacional,
        num_consultas_prenatal: f.num_consultas_prenatal,
        sexo_biologico: f.sexo_biologico,
        peso_gramos: f.peso_gramos,
        diagnostico_principal: f.diagnostico_principal,
        condicion_destino: o_nada(f.condicion_destino),
        diagnostico_causa_muerte: o_nada(f.diagnostico_causa_muerte),
        fecha_egreso: o_nada(f.fecha_egreso),
        codigo_vida: o_nada(f.codigo_vida),
    };
    match recien_nacido::crear(&e.pool, u.ambito(), &n, Some(u.id)).await {
        Ok(Ok(_id)) => de_vuelta(&format!("{RUTA}?aviso=registrado")),
        // El motivo del rechazo lo escribe el dominio y viaja tal cual: dos
        // redacciones del mismo rechazo acaban diciendo cosas distintas.
        Ok(Err(falta)) => de_vuelta(&format!("{RUTA}?aviso={}", falta.clave())),
        Err(_) => averia(),
    }
}
