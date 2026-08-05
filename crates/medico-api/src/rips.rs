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
//! 3. **Están los cinco.** El AC, el AP, el AM, el AU y el AH se generan aquí
//!    desde el 2026-08-04, así que `sin_portar` va vacío y **el 409 se quitó**:
//!    existía para no entregar un archivo al que le faltaban documentos, y ya no
//!    falta ninguno.
//!
//!    Lo que NO significa es que el archivo esté completo: significa que si algo
//!    falta, lo dice el recolector con el registro y el campo. La puerta que
//!    sigue en pie es `permitir_supuestos`, que es la que de verdad separa «este
//!    archivo dice lo que hay» de «este archivo rellena lo que no hay».
//!
//!    Sigue faltando el **FEV-RIPS**: el documento ÚNICO que funde los cinco en
//!    uno, que es lo que se le sube al validador para facturar. Eso es una
//!    transformación sobre estos cinco, no un sexto generador.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use medico_dominio::rips::documentos::Documento;
use medico_dominio::rips::{
    documento_ac, documento_ah, documento_am, documento_ap, documento_au, Recolector,
};

use crate::auth::{Autorizado, Estado, GenerarRips};

/// Los documentos que este binario todavía NO emite.
///
/// **Está vacía desde el 2026-08-04**, y por eso el 409 desapareció: los cinco
/// archivos de la 948 se generan aquí. Se conserva la constante —y el campo en
/// la respuesta— porque el día que la norma añada uno, la lista vuelve a
/// llenarse y todo lo que hay montado encima sigue funcionando. Una lista vacía
/// dice «no falta nada», que es distinto de no decir nada.
const SIN_PORTAR: [&str; 0] = [];

#[derive(Debug, Deserialize)]
pub struct Periodo {
    pub inicio: NaiveDate,
    pub fin: NaiveDate,
    /// Genera aunque falten datos obligatorios, reportando lo supuesto.
    #[serde(default)]
    pub permitir_supuestos: bool,
    /// Se conserva por compatibilidad: **ya no hace falta**, porque no falta
    /// ningún documento. Aceptarlo y no exigirlo es lo que evita romperle la
    /// llamada a quien la tenía escrita; rechazarlo sería un error por hacer
    /// bien las cosas.
    #[serde(default)]
    pub parcial: bool,
}

#[derive(Debug, Serialize)]
pub struct Salida {
    /// Los cinco documentos de la Resolución 948.
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
    #[serde(rename = "AP")]
    pub ap: Documento,
    #[serde(rename = "AM")]
    pub am: Documento,
    #[serde(rename = "AU")]
    pub au: Documento,
    #[serde(rename = "AH")]
    pub ah: Documento,
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

    // Aquí estaba la puerta del 409. Se quita porque su motivo se acabó: se
    // negaba a entregar un archivo al que le faltaban documentos, y ya no falta
    // ninguno. Dejarla sería pedir una bandera que no significa nada, y una
    // bandera que no significa nada se pone siempre sin leerla — que es como
    // deja de proteger la siguiente que sí importe (`permitir_supuestos`).

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

    // 422 y no 500: la petición está bien y el servidor también; lo que está mal
    // es un dato de la base, y se dice cuál.
    let dato_corrupto = |texto: String| {
        problema(StatusCode::UNPROCESSABLE_ENTITY, &texto, vec![], vec![])
    };

    let consultas =
        medico_datos::rips::consultas_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .map_err(dato_corrupto)?;
    let procedimientos =
        medico_datos::rips::procedimientos_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .map_err(dato_corrupto)?;
    let medicamentos =
        medico_datos::rips::medicamentos_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .map_err(dato_corrupto)?;
    let urgencias =
        medico_datos::rips::urgencias_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .map_err(dato_corrupto)?;
    let hospitalizaciones =
        medico_datos::rips::hospitalizaciones_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .map_err(dato_corrupto)?;

    // Un solo recolector para los tres documentos: quien cierra el mes necesita
    // UNA lista de lo que hay que corregir, no tres. Cada falta ya dice de qué
    // archivo es y en qué registro.
    let mut rec = if p.permitir_supuestos {
        Recolector::permitiendo_supuestos()
    } else {
        Recolector::estricto()
    };
    let ac = documento_ac(&institucion, &consultas, &mut rec);
    let ap = documento_ap(&institucion, &procedimientos, &mut rec);
    let am = documento_am(&institucion, &medicamentos, &mut rec);
    let au = documento_au(&institucion, &urgencias, &mut rec);
    let ah = documento_ah(&institucion, &hospitalizaciones, &mut rec);

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

    // El archivo SALE del sistema: es una exportación de dato personal, y va a la
    // bitácora con los pacientes que cubrió (#186, Ley 1581). Se anota aquí, con
    // el documento ya generado, y no al entrar: un 409 por parcial o un 422 por
    // dato faltante no exportan nada, y una bitácora que registra lo que no
    // ocurrió es peor que una que no registra.
    //
    // A diferencia de un listado, esta SÍ guarda los ids: el dato del titular
    // salió, y él tiene derecho a saber en qué archivo iba.
    //
    // Y los pacientes salen de los TRES documentos, no solo del AC: alguien que
    // en el periodo solo tuvo una fórmula o un laboratorio va en el archivo
    // igual, y su rastro no puede depender de en cuál de los tres apareció.
    let mut pacientes: Vec<i64> = consultas
        .iter()
        .map(|c| c.paciente.id)
        .chain(procedimientos.iter().map(|x| x.paciente.id))
        .chain(medicamentos.iter().map(|x| x.paciente.id))
        .chain(urgencias.iter().map(|x| x.paciente.id))
        .chain(hospitalizaciones.iter().map(|x| x.paciente.id))
        .collect();
    pacientes.sort_unstable();
    pacientes.dedup();
    let registros = consultas.len()
        + procedimientos.len()
        + medicamentos.len()
        + urgencias.len()
        + hospitalizaciones.len();
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "exportar",
        "rips",
        Some(descripcion_del_alcance(&p)),
        registros,
        pacientes,
    )
    .await;

    Ok(Json(Salida {
        archivos: Archivos { ac, ap, am, au, ah },
        supuestos,
        sin_portar: SIN_PORTAR.to_vec(),
    }))
}

/// Qué se pidió exactamente, para el `filtro` de la bitácora. Lleva las dos
/// puertas permisivas: dentro de un año, «quién exportó qué» no se responde sin
/// saber si aquel archivo llevaba supuestos o le faltaban cuatro documentos.
fn descripcion_del_alcance(p: &Periodo) -> String {
    let mut d = format!("periodo {}..{}", p.inicio, p.fin);
    if p.parcial {
        d.push_str(&format!("; parcial (sin {})", SIN_PORTAR.join(", ")));
    }
    if p.permitir_supuestos {
        d.push_str("; con supuestos");
    }
    d
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
