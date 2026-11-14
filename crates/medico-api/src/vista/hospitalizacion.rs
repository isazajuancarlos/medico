// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Hospitalización: el censo de camas y quién está en cada una.
//!
//! Sale de `medico_datos::hospitalizacion`, las MISMAS funciones que sirven
//! `GET /api/hospitalizacion` y `/camas`.
//!
//! # Se listan las ACTIVAS, y el archivo va aparte
//!
//! La pregunta del turno es «quién está ingresado ahora». Mezclar los egresados
//! en la misma tabla hace que el número de arriba no coincida con las filas y
//! obliga a leer la columna de egreso para saber de quién se habla. Con el
//! filtro se ve el archivo, y se dice cuál está puesto.
//!
//! # Y `Cama.estado` es texto, no un booleano
//!
//! `"DISPONIBLE"` / `"OCUPADA"`. Está en la lista de detalles del modelo que ya
//! han costado tiempo en este proyecto, así que se compara contra el valor y no
//! se deriva de si hay alguien dentro: la cama en mantenimiento no está ocupada
//! y tampoco está disponible.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

#[derive(Deserialize, Default)]
pub struct Filtro {
    /// `1` = incluir las que ya egresaron.
    #[serde(default)]
    pub todas: String,
}

/// `GET /panel/hospitalizacion`
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let todas = f.todas == "1";
    let (ingresos, camas) = match (
        medico_datos::hospitalizacion::listar(
            &e.pool,
            u.ambito(),
            (!todas).then_some("ACTIVA"),
        )
        .await,
        medico_datos::hospitalizacion::listar_camas(&e.pool, u.ambito(), None, None).await,
    ) {
        (Ok(i), Ok(c)) => (i, c),
        _ => return averia(),
    };

    // UN eslabón por PETICIÓN, no por consulta: esta pantalla pide el censo y el
    // mapa de camas, y anota una sola vez —por el censo, que es el que lleva
    // nombre, documento y diagnóstico—. Las camas no anotan aquí por lo mismo
    // que no anota `GET /api/hospitalizacion/camas`: no sale el dato de nadie.
    //
    // El filtro se anota con el MISMO valor que la ruta JSON: `"ACTIVA"` cuando
    // se miran los ingresados, nada cuando se mira el archivo entero.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_HOSPITALIZACIONES,
        (!todas).then(|| "ACTIVA".to_string()),
        ingresos.len(),
        vec![],
    )
    .await;

    // Se cuenta por el ESTADO de la cama, no por si hay alguien dentro: una cama
    // en mantenimiento no está ocupada ni disponible, y derivarlo la metería en
    // uno de los dos cubos sin que nadie lo haya decidido.
    let libres = camas.iter().filter(|c| c.estado == "DISPONIBLE").count();
    let ocupadas = camas.iter().filter(|c| c.estado == "OCUPADA").count();
    let otras = camas.len() - libres - ocupadas;

    pagina(
        "Hospitalización",
        Some(crate::vista::panel::menu(&cx, "/panel/hospitalizacion")),
        html! {
            header class="titulo-seccion" {
                h1 { "Hospitalización" }
                nav class="buscador" {
                    a class=[(!todas).then_some("enlace-activo")]
                      href="/panel/hospitalizacion" { "Ingresados" }
                    a class=[todas.then_some("enlace-activo")]
                      href="/panel/hospitalizacion?todas=1" { "Con egresados" }
                }
            }

            section class="tarjeta resumen" {
                div { span class="cifra" { (ocupadas) } span class="sutil" { "camas ocupadas" } }
                div { span class="cifra" { (libres) } span class="sutil" { "camas libres" } }
                @if otras > 0 {
                    // Se dicen aparte y con su nombre: meterlas en «libres»
                    // ofrecería una cama que no se puede dar.
                    div { span class="cifra" { (otras) } span class="sutil" { "en otro estado" } }
                }
                div { span class="cifra" { (camas.len()) } span class="sutil" { "camas en total" } }
            }

            @if ingresos.is_empty() {
                (piezas::vacio(
                    !todas,
                    "No hay hospitalizaciones registradas.",
                    "No hay nadie ingresado ahora mismo.",
                    "/panel/hospitalizacion?todas=1",
                ))
            } @else {
                (piezas::tabla(
                    &["Ingreso", "Paciente", "Documento", "Cama", "Servicio", "Vía", "Tratante", "Egreso", "Estado", "RDA"],
                    html! {
                        @for h in &ingresos {
                            tr {
                                td class="mono" { (h.fecha_ingreso) }
                                td {
                                    a href={ "/panel/pacientes/" (h.paciente_id) } {
                                        (h.paciente_nombre)
                                    }
                                }
                                td class="mono" { (h.paciente_identificacion) }
                                td class="mono" { (h.cama_numero) " · piso " (h.cama_piso) }
                                td { (h.cama_servicio) }
                                td { (h.via_ingreso) }
                                td { (h.medico_nombre) }
                                // Vacío mientras siga dentro. Poner la hora de
                                // ahora es lo que hacía el RIPS antes de
                                // corregirlo: reportaba como dado de alta a quien
                                // seguía ingresado.
                                td class="mono" { (h.fecha_egreso.clone().unwrap_or_default()) }
                                td { (piezas::estado(&h.estado)) }
                                td {
                                    a href={ "/panel/rda/hospitalizacion/" (h.id) } { "Preparar RDA" }
                                }
                            }
                        }
                    },
                ))
            }
        },
    )
    .into_response()
}
