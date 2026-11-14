// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

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

use medico_dominio::rips::fev;
use medico_dominio::rips::documentos::Documento;
use medico_dominio::rips::{
    anexar_recien_nacidos, documento_ac, documento_ah, documento_am, documento_aos, documento_ap,
    documento_au,
    FuenteInstitucion, FuenteRecienNacido, Recolector,
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
    /// #84: campos que TIENEN valor pero cuya procedencia no lo respalda —sin
    /// procedencia registrada (posible valor por defecto) o de efecto jurídico/
    /// económico sin aval humano—. No impide generar; se muestra a quien firma.
    /// Vacío cuando todos los registros del periodo llevan su procedencia al día.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub procedencia: Vec<Faltante>,
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
    #[serde(rename = "AT")]
    pub aos: Documento,
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
pub(crate) struct Faltante {
    pub archivo: &'static str,
    pub registro: i64,
    pub campo: &'static str,
    pub etiqueta: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
}

/// Lo que produjo una generación, con lo que hace falta para contarla y para
/// anotarla en la bitácora.
///
/// Existe porque **hay dos consumidores** —`GET /api/rips` y la pantalla
/// `/panel/rips`— y la alternativa era que el panel volviera a escribir la
/// secuencia entera: cinco consultas, un recolector compartido, el resumen antes
/// de cerrar y la lista de pacientes de los cinco documentos. Esa copia se queda
/// atrás en el primer arreglo (directiva 1), y el arreglo que se le olvidaría es
/// justo el de la bitácora.
pub(crate) struct Generacion {
    pub salida: Salida,
    /// Cuántos registros aportó cada archivo, en el orden de la 948. Es lo que
    /// la pantalla enseña: «el archivo lleva esto», antes de descargarlo.
    ///
    /// **Seis, no cinco, desde el 2026-08-16**: el `AN` (`recienNacidos`) es el
    /// sexto arreglo de la norma. No es un archivo aparte —viaja dentro del
    /// usuario de la madre— pero se cuenta igual, porque lo que la pantalla
    /// promete es lo que el archivo lleva.
    pub conteos: [(&'static str, usize); 7],
    /// Los recién nacidos del periodo. Se guardan crudos, y no dentro de
    /// `Archivos`, porque el arreglo no se puede colgar hasta que el FEV funde
    /// los cinco documentos en una sola lista de usuarios (ver [`Generacion::fev`]).
    recien_nacidos: Vec<FuenteRecienNacido>,
    /// La que reporta. Hace falta en [`Generacion::fev`] para el `codPrestador`
    /// del arreglo de recién nacidos, que se arma después de fundir.
    institucion: FuenteInstitucion,
    /// Los pacientes que aparecen en el archivo. Para la bitácora de la Ley
    /// 1581, y solo para ella.
    pacientes: Vec<i64>,
    /// Cuántos registros lo componen, sumados.
    registros: usize,
}

impl Generacion {
    /// Lo que la bitácora necesita y nada más: cuántos registros y qué pacientes.
    /// Es un método y no dos campos públicos para que el uso quede nombrado — lo
    /// pide el envío al MUV, que anota su propio eslabón porque el archivo no se
    /// descarga, se entrega a MinSalud.
    pub(crate) fn para_bitacora(&self) -> (usize, Vec<i64>) {
        (self.registros, self.pacientes.clone())
    }
}

/// Por qué no se pudo generar. Es un enum y no una `Response` ya armada porque
/// los dos consumidores la dibujan distinto: `/api` responde JSON y el panel
/// responde una pantalla.
pub(crate) enum Fallo {
    /// La base no respondió. Quien pide no tiene nada que corregir.
    Averia,
    /// La petición se puede corregir: el periodo al revés, un superadmin sin
    /// institución, una institución que ya no existe.
    Peticion { codigo: StatusCode, mensaje: String },
    /// Un dato de la base es imposible, y se dice cuál. La petición está bien y
    /// el servidor también.
    DatoCorrupto(String),
    /// Faltan datos obligatorios: el archivo no puede afirmar lo que no sabe.
    Incompleto(Vec<Faltante>),
}

impl Fallo {
    /// La respuesta JSON, **byte a byte la de antes de repartir esta función**:
    /// los códigos y los cuerpos que ya prueban `generacion_del_rips`.
    fn en_json(self) -> Response {
        match self {
            Fallo::Averia => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            Fallo::Peticion { codigo, mensaje } => problema(codigo, &mensaje, vec![], vec![]),
            Fallo::DatoCorrupto(t) => {
                problema(StatusCode::UNPROCESSABLE_ENTITY, &t, vec![], vec![])
            }
            Fallo::Incompleto(faltantes) => problema(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!(
                    "el RIPS no se puede generar: faltan {} datos obligatorios",
                    faltantes.len()
                ),
                faltantes,
                vec![],
            ),
        }
    }
}

/// Cómo se le pide el FEV-RIPS: los dos sobres, y **el tipo de prestador decide
/// cuál**, no este módulo.
///
/// Sin `num_factura` es un RIPS sin factura y `num_nota` es obligatorio; con él
/// es una FEV en salud y hace falta el AttachedDocument en Base64. Las dos
/// puertas fallan ANTES de enviar, porque un rechazo del MUV cuesta un ciclo
/// completo de reenvío.
#[derive(Debug, Deserialize)]
pub struct Sobre {
    #[serde(flatten)]
    pub periodo: Periodo,
    #[serde(default)]
    pub num_nota: String,
    #[serde(default)]
    pub num_factura: String,
    /// El AttachedDocument de la factura electrónica, ya aprobado por la DIAN.
    #[serde(default)]
    pub xml_fev: String,
}

impl Generacion {
    /// El documento ÚNICO que se envía al MUV, a partir de los cinco.
    ///
    /// La fusión y el sobre viven en el dominio (`medico_dominio::rips::fev`) y
    /// están probados byte a byte contra el `fev_rips.py` del backend, que es la
    /// condición del ADR.
    pub(crate) fn fev(&self, s: &Sobre) -> Result<fev::Paquete, fev::NoSePuede> {
        let a = &self.salida.archivos;
        let docs = [
            a.ac.clone(),
            a.ap.clone(),
            a.am.clone(),
            a.au.clone(),
            a.ah.clone(),
            a.aos.clone(),
        ];
        let mut paquete = if s.num_factura.trim().is_empty() {
            fev::sobre_rips_sin_factura(&docs, &s.num_nota)?
        } else {
            fev::sobre_fev(&docs, s.num_factura.trim(), &s.xml_fev)?
        };

        // El sexto arreglo se cuelga DESPUÉS de fundir, y no antes: hasta aquí
        // hay cinco listas de usuarios distintas y la madre puede estar en
        // varias. La llave es su documento, que es la misma con la que
        // `fev::fundir` agrupa.
        //
        // El recolector de la generación ya se cerró, así que las faltas de los
        // recién nacidos —una defunción sin causa— se anotaron en `generar`,
        // que es donde el operario las lee. Aquí solo se arma el JSON.
        let mut rec = Recolector::permitiendo_supuestos();
        let huerfanos = anexar_recien_nacidos(
            &self.institucion,
            &mut paquete.rips.usuarios,
            &self.recien_nacidos,
            &mut rec,
        );
        if !huerfanos.is_empty() {
            return Err(fev::NoSePuede::RecienNacidosSinAtencionDeLaMadre(huerfanos));
        }
        Ok(paquete)
    }
}

/// `GET /api/rips/fev?inicio=&fin=&num_nota=` — el documento del MUV.
///
/// Va por [`exportar`] y no por `generar_documentos`: el archivo SALE del
/// sistema igual que el de los cinco documentos, así que deja su eslabón con los
/// pacientes que cubrió (Ley 1581). Que sea otro formato no lo hace otra cosa.
pub async fn generar_fev(
    State(e): State<Estado>,
    autorizado: Autorizado<GenerarRips>,
    Query(s): Query<Sobre>,
) -> Result<Json<fev::Paquete>, Response> {
    let g = exportar(&e, &autorizado.0, &s.periodo)
        .await
        .map_err(|f| f.en_json())?;
    match g.fev(&s) {
        Ok(p) => Ok(Json(p)),
        // 422 y no 500: falta un dato que el llamante puede poner.
        Err(x) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "detail": x.mensaje() })),
        )
            .into_response()),
    }
}

/// `GET /api/rips?inicio=&fin=&permitir_supuestos=&parcial=`
pub async fn generar(
    State(e): State<Estado>,
    autorizado: Autorizado<GenerarRips>,
    Query(p): Query<Periodo>,
) -> Result<Json<Salida>, Response> {
    match exportar(&e, &autorizado.0, &p).await {
        Ok(g) => Ok(Json(g.salida)),
        Err(f) => Err(f.en_json()),
    }
}

/// Generar el archivo **y anotar que salió**.
///
/// Es la puerta por la que se exporta, y por eso las dos cosas van juntas en una
/// sola función: separarlas dejaría al segundo consumidor —la pantalla del
/// panel— sin la línea de la bitácora, que es exactamente el olvido que la Ley
/// 1581 no perdona. Quien solo quiera MIRAR lo que llevaría el archivo llama a
/// [`generar_documentos`], que no exporta nada.
pub(crate) async fn exportar(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    p: &Periodo,
) -> Result<Generacion, Fallo> {
    let g = generar_documentos(e, u, p).await?;

    // El archivo SALE del sistema: es una exportación de dato personal, y va a la
    // bitácora con los pacientes que cubrió (#186, Ley 1581). Se anota aquí, con
    // el documento ya generado, y no al entrar: un 422 por dato faltante no
    // exporta nada, y una bitácora que registra lo que no ocurrió es peor que una
    // que no registra.
    //
    // A diferencia de un listado, esta SÍ guarda los ids: el dato del titular
    // salió, y él tiene derecho a saber en qué archivo iba.
    crate::rutas::registrar_coleccion(
        e,
        u,
        "exportar",
        "rips",
        Some(descripcion_del_alcance(p)),
        g.registros,
        g.pacientes.clone(),
    )
    .await;

    Ok(g)
}

/// Generar los cinco documentos. **No exporta nada y no anota nada.**
///
/// La usa la revisión previa del panel, que enseña lo que llevaría el archivo
/// sin llegar a entregarlo.
pub(crate) async fn generar_documentos(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    p: &Periodo,
) -> Result<Generacion, Fallo> {
    let Some(institucion_id) = u.institucion_id else {
        // El superadmin no es de nadie, y el RIPS es de una institución. Se pide
        // que diga cuál en vez de generar el de todas juntas, que es justo la
        // fuga que el Python arregló.
        return Err(Fallo::Peticion {
            codigo: StatusCode::BAD_REQUEST,
            mensaje: "el usuario no está asociado a ninguna institución; no se puede \
                      determinar de quién sería el reporte"
                .to_string(),
        });
    };

    if p.fin < p.inicio {
        return Err(Fallo::Peticion {
            codigo: StatusCode::BAD_REQUEST,
            mensaje: format!(
                "el periodo va del {} al {}, que termina antes de empezar",
                p.inicio, p.fin
            ),
        });
    }

    // Aquí estaba la puerta del 409. Se quita porque su motivo se acabó: se
    // negaba a entregar un archivo al que le faltaban documentos, y ya no falta
    // ninguno. Dejarla sería pedir una bandera que no significa nada, y una
    // bandera que no significa nada se pone siempre sin leerla — que es como
    // deja de proteger la siguiente que sí importe (`permitir_supuestos`).

    let institucion = medico_datos::rips::institucion_que_reporta(&e.pool, institucion_id)
        .await
        .map_err(|_| Fallo::Averia)?
        .ok_or(Fallo::Peticion {
            codigo: StatusCode::NOT_FOUND,
            mensaje: "la institución del usuario no existe".to_string(),
        })?;

    // 422 y no 500: la petición está bien y el servidor también; lo que está mal
    // es un dato de la base, y se dice cuál.
    let consultas =
        medico_datos::rips::consultas_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;
    let procedimientos =
        medico_datos::rips::procedimientos_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;
    let medicamentos =
        medico_datos::rips::medicamentos_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;
    let urgencias =
        medico_datos::rips::urgencias_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;
    let hospitalizaciones =
        medico_datos::rips::hospitalizaciones_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;
    let recien_nacidos =
        medico_datos::rips::recien_nacidos_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;
    let otros_servicios =
        medico_datos::rips::otros_servicios_del_periodo(&e.pool, institucion_id, p.inicio, p.fin)
            .await
            .map_err(|_| Fallo::Averia)?
            .map_err(Fallo::DatoCorrupto)?;

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
    let aos = documento_aos(&institucion, &otros_servicios, &mut rec);
    // El AN no produce documento: se cuelga al fundir. Pero sus faltas —una
    // defunción sin causa básica— se recogen AQUÍ, con el mismo recolector que
    // los cinco, porque quien cierra el mes necesita UNA lista. Se anexa sobre
    // una copia de los usuarios del AH que se descarta: lo que interesa de esta
    // llamada son las faltas, no el resultado.
    {
        let mut usuarios_de_prueba = ah.usuarios.clone();
        anexar_recien_nacidos(
            &institucion,
            &mut usuarios_de_prueba,
            &recien_nacidos,
            &mut rec,
        );
    }

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

    // #84: la auditoría de procedencia. Se recoge ANTES de cerrar, como el resumen
    // y las faltas, porque `cerrar` consume el recolector. No bloquea la
    // generación —es advertencia—, así que va en la salida, no en el error.
    let procedencia: Vec<Faltante> = rec
        .sin_respaldo()
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
        return Err(Fallo::Incompleto(faltantes));
    }

    // Los pacientes salen de los CINCO documentos, no solo del AC: alguien que en
    // el periodo solo tuvo una fórmula o un laboratorio va en el archivo igual, y
    // su rastro no puede depender de en cuál de los cinco apareció.
    let mut pacientes: Vec<i64> = consultas
        .iter()
        .map(|c| c.paciente.id)
        .chain(procedimientos.iter().map(|x| x.paciente.id))
        .chain(medicamentos.iter().map(|x| x.paciente.id))
        .chain(urgencias.iter().map(|x| x.paciente.id))
        .chain(hospitalizaciones.iter().map(|x| x.paciente.id))
        .chain(otros_servicios.iter().map(|x| x.paciente.id))
        // La madre de cada recién nacido también aparece en el archivo, y la
        // bitácora de la Ley 1581 ata el eslabón al TITULAR: si no entra aquí,
        // exportar un nacimiento no deja rastro de a quién cubrió.
        .chain(recien_nacidos.iter().map(|x| x.madre_paciente_id))
        .collect();
    pacientes.sort_unstable();
    pacientes.dedup();
    let conteos = [
        ("AC", consultas.len()),
        ("AP", procedimientos.len()),
        ("AM", medicamentos.len()),
        ("AU", urgencias.len()),
        ("AH", hospitalizaciones.len()),
        ("AN", recien_nacidos.len()),
        ("AT", otros_servicios.len()),
    ];
    let registros = conteos.iter().map(|(_, n)| n).sum();

    Ok(Generacion {
        salida: Salida {
            archivos: Archivos { ac, ap, am, au, ah, aos },
            supuestos,
            sin_portar: SIN_PORTAR.to_vec(),
            procedencia,
        },
        conteos,
        recien_nacidos,
        institucion,
        pacientes,
        registros,
    })
}

/// Qué se pidió exactamente, para el `filtro` de la bitácora. Lleva las dos
/// puertas permisivas: dentro de un año, «quién exportó qué» no se responde sin
/// saber si aquel archivo llevaba supuestos o le faltaban cuatro documentos.
fn descripcion_del_alcance(p: &Periodo) -> String {
    let mut d = format!("periodo {}..{}", p.inicio, p.fin);
    // La cláusula solo se escribe si de verdad falta un documento. Con
    // `SIN_PORTAR` vacío, la versión anterior dejaba «; parcial (sin )» en la
    // bitácora de la Ley 1581 — y ese registro se lee dentro de un año, cuando
    // nadie recuerde que aquel paréntesis vacío significaba «no faltaba nada».
    // Un rastro que hay que interpretar es un rastro que se interpreta mal.
    if p.parcial && !SIN_PORTAR.is_empty() {
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
