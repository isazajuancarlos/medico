// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! `GET /api/rips` — generar el RIPS de un periodo (#250, #236).
//!
//! Port de `backend/routers/rips.py`, con las dos reglas que aquel aprendió a la
//! mala y una tercera que aparece por ser un port a medias.
//!
//! 1. **La institución sale del usuario autenticado, nunca de un parámetro.**
//!    Aquel llamado iba sin institución y el generador entonces no filtraba
//!    nada: el archivo salía con las atenciones de TODAS las instituciones.
//! 2. **Un RIPS incompleto no se genera solo.** Se puede forzar, pero hay que
//!    pedirlo (`permitir_supuestos=true`), y lo supuesto viaja EN la respuesta:
//!    quien lo firma tiene que ver qué lleva inventado antes de firmarlo, no
//!    encontrarlo en un log aparte.
//! 3. **Y aquí solo existe el AC.** Los otros cuatro documentos —AP, AM, AU,
//!    AH— todavía no están portados. Un archivo con las consultas y sin los
//!    procedimientos ni los medicamentos **tiene la forma de un RIPS completo**:
//!    se puede subir, el validador lo acepta, y lo que falta no aparece por
//!    ningún lado — es sub-reporte, que en una factura es dinero que no se
//!    cobra, y ante la Supersalud, información incompleta.
//!
//!    Por eso el endpoint **se niega por defecto** (409) y hay que pedirle
//!    `parcial=true` para que emita solo el AC, igual que con los supuestos: el
//!    camino permisivo existe, lo pide el llamante, y la respuesta dice a gritos
//!    qué falta. Mientras tanto el RIPS de verdad se genera con el Python, que
//!    es el producto vivo.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use medico_dominio::rips::documentos::Documento;
use medico_dominio::rips::{documento_ac, Recolector};

use crate::auth::{Autorizado, Estado, GenerarRips};

/// Los documentos que este binario todavía NO emite. Se nombran en la respuesta
/// y en el error: una lista vacía sería la manera de que nadie se enterara.
const SIN_PORTAR: [&str; 4] = ["AP", "AM", "AU", "AH"];

#[derive(Debug, Deserialize)]
pub struct Periodo {
    pub inicio: NaiveDate,
    pub fin: NaiveDate,
    /// Genera aunque falten datos obligatorios, reportando lo supuesto.
    #[serde(default)]
    pub permitir_supuestos: bool,
    /// Acepta un archivo con SOLO el AC. Ver la cabecera del módulo.
    #[serde(default)]
    pub parcial: bool,
}

#[derive(Debug, Serialize)]
pub struct Salida {
    /// Los documentos generados, por nombre: hoy solo `AC`.
    pub archivos: Archivos,
    /// Cuántos valores se supusieron, por etiqueta. Vacío si no se supuso nada.
    pub supuestos: std::collections::BTreeMap<String, usize>,
    /// Los que este binario no sabe emitir todavía. Viaja SIEMPRE.
    pub sin_portar: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Archivos {
    #[serde(rename = "AC")]
    pub ac: Documento,
}

#[derive(Debug, Serialize)]
struct Problema {
    mensaje: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    faltantes: Vec<Faltante>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sin_portar: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct Faltante {
    archivo: &'static str,
    registro: i64,
    campo: &'static str,
    etiqueta: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    motivo: Option<String>,
}

/// `GET /api/rips?inicio=&fin=&permitir_supuestos=&parcial=`
pub async fn generar(
    State(e): State<Estado>,
    autorizado: Autorizado<GenerarRips>,
    Query(p): Query<Periodo>,
) -> Result<Json<Salida>, Response> {
    let u = autorizado.0;
    let Some(institucion_id) = u.institucion_id else {
        // El superadmin no es de nadie, y el RIPS es de una institución. Se pide
        // que diga cuál en vez de generar el de todas juntas, que es justo la
        // fuga que el Python arregló.
        return Err(problema(
            StatusCode::BAD_REQUEST,
            "el usuario no está asociado a ninguna institución; no se puede \
             determinar de quién sería el reporte",
            vec![],
            vec![],
        ));
    };

    if p.fin < p.inicio {
        return Err(problema(
            StatusCode::BAD_REQUEST,
            &format!(
                "el periodo va del {} al {}, que termina antes de empezar",
                p.inicio, p.fin
            ),
            vec![],
            vec![],
        ));
    }

    if !p.parcial {
        return Err(problema(
            StatusCode::CONFLICT,
            &format!(
                "este binario solo emite el AC; {} todavía no están portados. Un \
                 archivo sin ellos parece completo y no lo está: las atenciones \
                 que falten no se cobran y nadie las echa de menos. Genere el \
                 RIPS con el sistema Python, o pida `parcial=true` si lo que \
                 quiere es revisar el AC.",
                SIN_PORTAR.join(", ")
            ),
            vec![],
            SIN_PORTAR.to_vec(),
        ));
    }

    let institucion = medico_datos::rips::institucion_que_reporta(&e.pool, institucion_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .ok_or_else(|| {
            problema(
                StatusCode::NOT_FOUND,
                "la institución del usuario no existe",
                vec![],
                vec![],
            )
        })?;

    let consultas =
        medico_datos::rips::consultas_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .map_err(|dato_corrupto| {
                // 422 y no 500: la petición está bien y el servidor también; lo
                // que está mal es un dato de la base, y se dice cuál.
                problema(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    &dato_corrupto,
                    vec![],
                    vec![],
                )
            })?;

    let mut rec = if p.permitir_supuestos {
        Recolector::permitiendo_supuestos()
    } else {
        Recolector::estricto()
    };
    let ac = documento_ac(&institucion, &consultas, &mut rec);

    // El resumen se saca ANTES de cerrar, porque `cerrar` consume el recolector.
    let supuestos = resumen(&rec);
    let faltantes: Vec<Faltante> = rec
        .faltantes()
        .iter()
        .map(|f| Faltante {
            archivo: f.archivo,
            registro: f.registro_id,
            campo: f.campo,
            etiqueta: f.etiqueta,
            motivo: f.motivo.clone(),
        })
        .collect();

    if rec.cerrar().is_err() {
        return Err(problema(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!(
                "el RIPS no se puede generar: faltan {} datos obligatorios",
                faltantes.len()
            ),
            faltantes,
            vec![],
        ));
    }

    Ok(Json(Salida {
        archivos: Archivos { ac },
        supuestos,
        sin_portar: SIN_PORTAR.to_vec(),
    }))
}

/// Cuántos valores se supusieron, por etiqueta. Es el `_resumen_supuestos` del
/// Python, y va en la respuesta y no en un log: quien firma tiene que verlo.
fn resumen(rec: &Recolector) -> std::collections::BTreeMap<String, usize> {
    let mut m = std::collections::BTreeMap::new();
    for f in rec.faltantes() {
        *m.entry(f.etiqueta.to_string()).or_insert(0) += 1;
    }
    m
}

fn problema(
    codigo: StatusCode,
    mensaje: &str,
    faltantes: Vec<Faltante>,
    sin_portar: Vec<&'static str>,
) -> Response {
    (
        codigo,
        Json(Problema {
            mensaje: mensaje.to_string(),
            faltantes,
            sin_portar,
        }),
    )
        .into_response()
}
