// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Lo que el RIPS necesita de la base. Las REGLAS están en
//! `medico_dominio::rips`; aquí solo se pone el SQL (#250).
//!
//! # El periodo se pide en hora local y se guarda en UTC
//!
//! `historia.fecha` la escribe el `DEFAULT (datetime('now'))` de SQLite, que es
//! **UTC**. Quien cierra el mes piensa en fechas locales: «del 1 al 31 de
//! marzo». En Colombia son cinco horas de diferencia, así que una atención de
//! las 20:00 del 31 se guarda como el 1 de abril a las 01:00.
//!
//! Filtrar por el texto crudo metería esa consulta en abril y la sacaría del
//! cierre de marzo, y nadie lo notaría hasta que la factura no cuadre. Aquí los
//! límites del periodo se convierten a UTC ANTES de consultar, y las fechas se
//! emiten de vuelta en local: **se filtra y se emite en el mismo marco**. Hacer
//! una cosa en un huso y la otra en otro es peor que hacer las dos mal, porque
//! las atenciones del borde salen en un mes y con fecha de otro.
//!
//! **El huso es el del PRESTADOR, no el del servidor** (`medico_dominio::zona`).
//! Hasta el 2026-08-03 esto usaba `chrono::Local` y por tanto el reloj de la
//! máquina: correcto por casualidad en el portátil de un consultorio, y corrido
//! cinco horas en el VPS de la demostración, que va en UTC.

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use medico_dominio::zona;
use medico_dominio::rips::{
    FuenteConsulta, FuenteInstitucion, FuenteMedicamento, FuentePaciente, FuenteProcedimiento,
    FuenteHospitalizacion, FuenteProfesional, FuenteUrgencia,
};

/// La institución que reporta, o `None` si no existe.
///
/// El código de habilitación puede venir vacío: es un dato que hay que capturar
/// y no uno que se invente. El generador lo verá vacío y lo dirá.
pub async fn institucion_que_reporta(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<FuenteInstitucion>, sqlx::Error> {
    let fila: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT nit, codigo_habilitacion FROM institucion WHERE id = ?")
            .bind(institucion_id)
            .fetch_optional(pool)
            .await?;
    Ok(fila.map(|(nit, hab)| FuenteInstitucion {
        nit,
        codigo_habilitacion: hab.unwrap_or_default(),
    }))
}

/// Los dos extremos del periodo, ya en el texto UTC con el que se guarda.
///
/// `hasta` incluye el día entero: el cierre de marzo lleva lo del 31 a las
/// 23:59, no solo lo de las 00:00.
fn limites_utc(desde: NaiveDate, hasta: NaiveDate) -> (String, String) {
    (a_texto_utc(desde, 0, 0, 0), a_texto_utc(hasta, 23, 59, 59))
}

fn a_texto_utc(dia: NaiveDate, h: u32, m: u32, s: u32) -> String {
    let local = dia
        .and_hms_opt(h, m, s)
        .expect("hora fija y válida por construcción");
    // La zona es la del PRESTADOR (`medico_dominio::zona`), no la del reloj de
    // la máquina. Hasta el 2026-08-03 esto era `Local`, y en el VPS de la demo
    // —que corre en UTC— movía el periodo entero cinco horas sin que nada
    // fallara.
    //
    // Un instante local puede no existir (el salto del horario de verano se come
    // una hora). Colombia no cambia la hora, pero el binario tiene que poder
    // correr en otro sitio: si no existe, se interpreta el texto como UTC en vez
    // de inventar un desplazamiento.
    match zona::a_utc(local) {
        Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => local.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

/// Lee el texto que guarda SQLite y lo devuelve como instante local.
///
/// `registro` es cómo se llama la fila que trae esa fecha —«la historia», «la
/// orden», «la fórmula»— y no es cosmético: el id que se pasa es el de ESA
/// tabla. El mensaje decía «la historia» viniera de donde viniera, así que un
/// procedimiento con la fecha corrupta mandaba a buscar una historia con el id
/// de la orden. Es la fila equivocada, y en el mejor caso no existe.
///
/// Acepta las dos formas que existen en la base: la del `DEFAULT` de SQLite
/// (`2026-03-15 09:30:00`) y la RFC 3339 que escribe el código Rust
/// (`2026-03-15T09:30:00Z`). **Lo que no se puede leer no se sustituye por
/// «ahora»**: se devuelve el error con el id de la fila, porque una fecha
/// inventada en el RIPS es una atención reportada en un día en que no ocurrió.
fn fecha_local(texto: &str, registro: &str, id: i64) -> Result<NaiveDateTime, String> {
    if let Ok(t) = DateTime::parse_from_rfc3339(texto) {
        return Ok(t.with_timezone(&zona::zona()).naive_local());
    }
    let naive = NaiveDateTime::parse_from_str(texto, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| format!("{registro} {id} tiene «{texto}» como fecha: {e}"))?;
    Ok(zona::desde_utc(naive))
}

/// Busca una historia cuya fecha no tenga forma de fecha, ANTES de filtrar.
///
/// Existe porque el filtro del periodo compara TEXTO, y una fecha corrupta no
/// falla ahí: se queda fuera del rango y la atención **desaparece del informe**
/// sin que nadie se entere. Eso es peor que un error — es una atención prestada
/// que no se reporta, y no hay nada en el archivo que insinúe que faltaba algo.
///
/// El `GLOB` pide la forma `AAAA-MM-DD…`, que es la única con la que la
/// comparación de texto ordena igual que el tiempo. Cuesta una consulta por
/// generación, sobre un índice de institución, y convierte una omisión
/// silenciosa en una línea que dice qué historia arreglar.
async fn fecha_ilegible_de(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let mala: Option<(i64, String)> = sqlx::query_as(
        "SELECT h.id, h.fecha FROM historia h \
         JOIN paciente p ON p.id = h.paciente_id \
         WHERE p.institucion_id = ? \
           AND h.fecha NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]*' \
         ORDER BY h.id LIMIT 1",
    )
    .bind(institucion_id)
    .fetch_optional(pool)
    .await?;
    Ok(mala.map(|(id, texto)| {
        format!(
            "la historia {id} tiene «{texto}» como fecha, que no es una fecha. \
             No se genera el RIPS: con ese valor la atención no entra en ningún \
             periodo y se perdería del informe sin avisar."
        )
    }))
}

/// Las consultas del periodo, de UNA institución, listas para el AC.
///
/// Ordenadas por fecha e id: sin un orden fijo, dos corridas del mismo periodo
/// darían consecutivos distintos para las mismas atenciones y el archivo dejaría
/// de ser reproducible.
///
/// El filtro por institución va sobre el PACIENTE, que es donde vive
/// `institucion_id` — la historia cuelga de él.
pub async fn consultas_del_periodo(
    pool: &crate::Pool,
    institucion_id: i64,
    desde: NaiveDate,
    hasta: NaiveDate,
) -> Result<Result<Vec<FuenteConsulta>, String>, sqlx::Error> {
    if let Some(mala) = fecha_ilegible_de(pool, institucion_id).await? {
        return Ok(Err(mala));
    }
    let (ini, fin) = limites_utc(desde, hasta);
    let filas: Vec<Fila> = sqlx::query_as(
        "SELECT h.id, h.fecha, h.cod_consulta_cups, h.modalidad_grupo_servicio, \
                h.grupo_servicios, h.cod_servicio, h.finalidad, h.causa_externa, \
                h.diagnostico_principal, h.diagnostico_relacionado1, \
                h.diagnostico_relacionado2, h.diagnostico_relacionado3, \
                h.tipo_diagnostico, h.valor_servicio, h.concepto_recaudo, \
                h.valor_pago_moderador, h.num_fev_pago_moderador, h.genera_incapacidad, \
                p.id AS paciente_id, \
                p.tipo_documento   AS p_tipo_documento, \
                p.numero_documento AS p_numero_documento, \
                p.tipo_usuario, p.fecha_nacimiento, p.sexo, p.cod_pais, \
                p.cod_municipio, p.zona, p.cod_pais_origen, \
                u.tipo_documento   AS m_tipo_documento, \
                u.numero_documento AS m_numero_documento \
         FROM historia h \
         JOIN paciente p ON p.id = h.paciente_id \
         JOIN usuario  u ON u.id = h.medico_id \
         WHERE p.institucion_id = ? AND h.fecha >= ? AND h.fecha <= ? \
         ORDER BY h.fecha, h.id",
    )
    .bind(institucion_id)
    .bind(&ini)
    .bind(&fin)
    .fetch_all(pool)
    .await?;

    let mut salida = Vec::with_capacity(filas.len());
    for f in filas {
        let fecha = match fecha_local(&f.fecha, "la historia", f.id) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        salida.push(FuenteConsulta {
            id: f.id,
            fecha,
            // La `cita` de este esquema todavía no guarda número de
            // autorización, así que va vacío — como en el Python, que también lo
            // deja en "" cuando la cita no lo trae. No se inventa uno.
            num_autorizacion: None,
            cod_consulta_cups: f.cod_consulta_cups,
            modalidad_grupo_servicio: f.modalidad_grupo_servicio,
            grupo_servicios: f.grupo_servicios,
            cod_servicio: f.cod_servicio,
            finalidad: f.finalidad,
            causa_externa: f.causa_externa,
            diagnostico_principal: f.diagnostico_principal,
            diagnostico_relacionado1: f.diagnostico_relacionado1,
            diagnostico_relacionado2: f.diagnostico_relacionado2,
            diagnostico_relacionado3: f.diagnostico_relacionado3,
            tipo_diagnostico: f.tipo_diagnostico,
            valor_servicio: f.valor_servicio,
            concepto_recaudo: f.concepto_recaudo,
            valor_pago_moderador: f.valor_pago_moderador,
            num_fev_pago_moderador: f.num_fev_pago_moderador,
            genera_incapacidad: f.genera_incapacidad != 0,
            paciente: FuentePaciente {
                id: f.paciente_id,
                tipo_documento: f.p_tipo_documento,
                numero_documento: f.p_numero_documento,
                tipo_usuario: f.tipo_usuario,
                fecha_nacimiento: f.fecha_nacimiento,
                sexo: f.sexo,
                cod_pais: f.cod_pais,
                cod_municipio: f.cod_municipio,
                zona: f.zona,
                cod_pais_origen: f.cod_pais_origen,
            },
            profesional: FuenteProfesional {
                tipo_documento: f.m_tipo_documento,
                numero_documento: f.m_numero_documento,
            },
        });
    }
    Ok(Ok(salida))
}

/// Los procedimientos del periodo, de UNA institución, listos para el AP.
///
/// Son las **órdenes** colgadas de una historia. El periodo se filtra por la
/// fecha de la HISTORIA y no por `orden.fecha_orden`, que es lo que hace el
/// Python: lo que el RIPS reporta es la atención, y una orden escrita al día
/// siguiente pertenece a la atención en que se ordenó. Si se filtrara por la
/// fecha de la orden, un examen pedido el 31 a las 20:00 y registrado el 1 se
/// iría al mes siguiente mientras su consulta se queda en este — y las dos
/// mitades de la misma atención acabarían en facturas distintas.
///
/// El aislamiento va por el PACIENTE, igual que en el AC: `orden` no tiene
/// `institucion_id`, cuelga de la historia, que cuelga del paciente.
pub async fn procedimientos_del_periodo(
    pool: &crate::Pool,
    institucion_id: i64,
    desde: NaiveDate,
    hasta: NaiveDate,
) -> Result<Result<Vec<FuenteProcedimiento>, String>, sqlx::Error> {
    if let Some(mala) = fecha_ilegible_de(pool, institucion_id).await? {
        return Ok(Err(mala));
    }
    let (ini, fin) = limites_utc(desde, hasta);
    let filas: Vec<FilaProcedimiento> = sqlx::query_as(
        "SELECT o.id, h.fecha, o.codigo_cups, o.tipo, o.via_ingreso_servicio, \
                o.grupo_servicios, o.cod_servicio, h.finalidad, \
                h.diagnostico_principal, h.diagnostico_relacionado1, \
                o.valor_servicio, o.concepto_recaudo, o.valor_pago_moderador, \
                o.num_fev_pago_moderador, \
                p.id AS paciente_id, \
                p.tipo_documento   AS p_tipo_documento, \
                p.numero_documento AS p_numero_documento, \
                p.tipo_usuario, p.fecha_nacimiento, p.sexo, p.cod_pais, \
                p.cod_municipio, p.zona, p.cod_pais_origen, \
                u.tipo_documento   AS m_tipo_documento, \
                u.numero_documento AS m_numero_documento \
         FROM orden o \
         JOIN historia h ON h.id = o.historia_id \
         JOIN paciente p ON p.id = h.paciente_id \
         JOIN usuario  u ON u.id = h.medico_id \
         WHERE p.institucion_id = ? AND h.fecha >= ? AND h.fecha <= ? \
         ORDER BY h.fecha, o.id",
    )
    .bind(institucion_id)
    .bind(&ini)
    .bind(&fin)
    .fetch_all(pool)
    .await?;

    let mut salida = Vec::with_capacity(filas.len());
    for f in filas {
        let fecha = match fecha_local(&f.fecha, "la orden", f.id) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        salida.push(FuenteProcedimiento {
            id: f.id,
            fecha,
            cod_procedimiento_cups: f.codigo_cups,
            tipo: f.tipo,
            via_ingreso_servicio: f.via_ingreso_servicio,
            grupo_servicios: f.grupo_servicios,
            cod_servicio: f.cod_servicio,
            finalidad: f.finalidad,
            diagnostico_principal: f.diagnostico_principal,
            diagnostico_relacionado: f.diagnostico_relacionado1,
            valor_servicio: f.valor_servicio,
            concepto_recaudo: f.concepto_recaudo,
            valor_pago_moderador: f.valor_pago_moderador,
            num_fev_pago_moderador: f.num_fev_pago_moderador,
            paciente: FuentePaciente {
                id: f.paciente_id,
                tipo_documento: f.p_tipo_documento,
                numero_documento: f.p_numero_documento,
                tipo_usuario: f.tipo_usuario,
                fecha_nacimiento: f.fecha_nacimiento,
                sexo: f.sexo,
                cod_pais: f.cod_pais,
                cod_municipio: f.cod_municipio,
                zona: f.zona,
                cod_pais_origen: f.cod_pais_origen,
            },
            profesional: FuenteProfesional {
                tipo_documento: f.m_tipo_documento,
                numero_documento: f.m_numero_documento,
            },
        });
    }
    Ok(Ok(salida))
}

/// Las fórmulas del periodo, de UNA institución, listas para el AM.
///
/// Mismo criterio que los procedimientos: el periodo lo fija la fecha de la
/// HISTORIA, no `formula.fecha_formula`.
///
/// **Se emiten todas las fórmulas del periodo, dispensadas o no**, que es lo que
/// hace el Python. Filtrar por `dispensada` sería una decisión distinta —el AM
/// es el archivo de lo *dispensado o administrado*— y no se toma de paso en un
/// port: cambiaría lo que se factura, y hay que preguntarle al prestador si su
/// sistema marca ese campo de verdad.
pub async fn medicamentos_del_periodo(
    pool: &crate::Pool,
    institucion_id: i64,
    desde: NaiveDate,
    hasta: NaiveDate,
) -> Result<Result<Vec<FuenteMedicamento>, String>, sqlx::Error> {
    if let Some(mala) = fecha_ilegible_de(pool, institucion_id).await? {
        return Ok(Err(mala));
    }
    let (ini, fin) = limites_utc(desde, hasta);
    let filas: Vec<FilaMedicamento> = sqlx::query_as(
        "SELECT f.id, h.fecha, f.codigo_cum, f.tipo_medicamento, f.medicamento, \
                h.diagnostico_principal, f.unidad, f.cantidad, f.dias_tratamiento, \
                f.valor_unitario, f.valor_servicio, f.concepto_recaudo, \
                f.valor_pago_moderador, f.num_fev_pago_moderador, \
                p.id AS paciente_id, \
                p.tipo_documento   AS p_tipo_documento, \
                p.numero_documento AS p_numero_documento, \
                p.tipo_usuario, p.fecha_nacimiento, p.sexo, p.cod_pais, \
                p.cod_municipio, p.zona, p.cod_pais_origen, \
                u.tipo_documento   AS m_tipo_documento, \
                u.numero_documento AS m_numero_documento \
         FROM formula f \
         JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id \
         JOIN usuario  u ON u.id = h.medico_id \
         WHERE p.institucion_id = ? AND h.fecha >= ? AND h.fecha <= ? \
         ORDER BY h.fecha, f.id",
    )
    .bind(institucion_id)
    .bind(&ini)
    .bind(&fin)
    .fetch_all(pool)
    .await?;

    let mut salida = Vec::with_capacity(filas.len());
    for f in filas {
        let fecha = match fecha_local(&f.fecha, "la fórmula", f.id) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        salida.push(FuenteMedicamento {
            id: f.id,
            // El AM lleva la fecha SIN hora (`fechaDispensAdmon`), pero se
            // convierte igual a local antes de quedarse con el día: quedarse con
            // los diez primeros caracteres del texto UTC devolvería el día
            // siguiente para toda atención posterior a las 19:00 en Colombia.
            fecha: fecha.date(),
            codigo_cum: f.codigo_cum,
            tipo_medicamento: f.tipo_medicamento,
            medicamento: f.medicamento,
            diagnostico_principal: f.diagnostico_principal,
            unidad: f.unidad,
            cantidad: Some(f.cantidad),
            dias_tratamiento: f.dias_tratamiento,
            valor_unitario: f.valor_unitario,
            valor_servicio: f.valor_servicio,
            concepto_recaudo: f.concepto_recaudo,
            valor_pago_moderador: f.valor_pago_moderador,
            num_fev_pago_moderador: f.num_fev_pago_moderador,
            paciente: FuentePaciente {
                id: f.paciente_id,
                tipo_documento: f.p_tipo_documento,
                numero_documento: f.p_numero_documento,
                tipo_usuario: f.tipo_usuario,
                fecha_nacimiento: f.fecha_nacimiento,
                sexo: f.sexo,
                cod_pais: f.cod_pais,
                cod_municipio: f.cod_municipio,
                zona: f.zona,
                cod_pais_origen: f.cod_pais_origen,
            },
            profesional: FuenteProfesional {
                tipo_documento: f.m_tipo_documento,
                numero_documento: f.m_numero_documento,
            },
        });
    }
    Ok(Ok(salida))
}

/// Las atenciones de urgencias del periodo, de UNA institución, listas para el AU.
///
/// **Solo las que ya egresaron.** Una atención sin `fecha_egreso` no ha
/// terminado y no se puede facturar; el sistema viejo le ponía la hora de la
/// generación, o sea que un paciente todavía en urgencias se reportaba como dado
/// de alta en el instante en que alguien pidió el archivo. Entrará en el cierre
/// del periodo en que de verdad salga.
///
/// El periodo se filtra por el INGRESO, como el Python: una atención que empieza
/// el 31 de marzo y termina el 1 de abril es del cierre de marzo.
///
/// `atencion_urgencias` cuelga del paciente directamente —no pasa por la
/// historia—, así que aquí el filtro de institución es un `JOIN` menos.
pub async fn urgencias_del_periodo(
    pool: &crate::Pool,
    institucion_id: i64,
    desde: NaiveDate,
    hasta: NaiveDate,
) -> Result<Result<Vec<FuenteUrgencia>, String>, sqlx::Error> {
    if let Some(mala) = fecha_ilegible_de_urgencias(pool, institucion_id).await? {
        return Ok(Err(mala));
    }
    let (ini, fin) = limites_utc(desde, hasta);
    let filas: Vec<FilaUrgencia> = sqlx::query_as(
        "SELECT a.id, a.fecha_ingreso, a.fecha_egreso, a.causa_motivo, \
                a.diagnostico_ingreso, a.diagnostico_egreso, a.destino_salida, \
                a.diagnostico_causa_muerte, \
                p.id AS paciente_id, \
                p.tipo_documento   AS p_tipo_documento, \
                p.numero_documento AS p_numero_documento, \
                p.tipo_usuario, p.fecha_nacimiento, p.sexo, p.cod_pais, \
                p.cod_municipio, p.zona, p.cod_pais_origen \
         FROM atencion_urgencias a \
         JOIN paciente p ON p.id = a.paciente_id \
         WHERE p.institucion_id = ? AND a.fecha_ingreso >= ? AND a.fecha_ingreso <= ? \
           AND a.fecha_egreso IS NOT NULL \
         ORDER BY a.fecha_ingreso, a.id",
    )
    .bind(institucion_id)
    .bind(&ini)
    .bind(&fin)
    .fetch_all(pool)
    .await?;

    let mut salida = Vec::with_capacity(filas.len());
    for f in filas {
        let ingreso = match fecha_local(&f.fecha_ingreso, "la atención de urgencias", f.id) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        // El egreso no puede llegar nulo: lo excluye el `WHERE`. Si llegara
        // ilegible se dice cuál, igual que con el ingreso — una fecha de egreso
        // inventada es un alta en un día en que no ocurrió.
        let egreso = match f
            .fecha_egreso
            .as_deref()
            .map(|t| fecha_local(t, "la atención de urgencias", f.id))
        {
            Some(Ok(v)) => v,
            Some(Err(e)) => return Ok(Err(e)),
            None => continue,
        };
        salida.push(FuenteUrgencia {
            id: f.id,
            fecha_ingreso: ingreso,
            fecha_egreso: egreso,
            causa_motivo: f.causa_motivo,
            diagnostico_ingreso: f.diagnostico_ingreso,
            diagnostico_egreso: f.diagnostico_egreso,
            destino_salida: f.destino_salida,
            diagnostico_causa_muerte: f.diagnostico_causa_muerte,
            paciente: FuentePaciente {
                id: f.paciente_id,
                tipo_documento: f.p_tipo_documento,
                numero_documento: f.p_numero_documento,
                tipo_usuario: f.tipo_usuario,
                fecha_nacimiento: f.fecha_nacimiento,
                sexo: f.sexo,
                cod_pais: f.cod_pais,
                cod_municipio: f.cod_municipio,
                zona: f.zona,
                cod_pais_origen: f.cod_pais_origen,
            },
        });
    }
    Ok(Ok(salida))
}

/// El mismo guardián de `fecha_ilegible_de`, sobre la tabla de urgencias.
///
/// Va aparte porque la atención de urgencias **no cuelga de una historia**: una
/// fecha corrupta aquí no la ve el guardián del AC, y la atención desaparecería
/// del archivo sin que nada fallara.
async fn fecha_ilegible_de_urgencias(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let mala: Option<(i64, String)> = sqlx::query_as(
        "SELECT a.id, a.fecha_ingreso FROM atencion_urgencias a \
         JOIN paciente p ON p.id = a.paciente_id \
         WHERE p.institucion_id = ? \
           AND a.fecha_ingreso NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]*' \
         ORDER BY a.id LIMIT 1",
    )
    .bind(institucion_id)
    .fetch_optional(pool)
    .await?;
    Ok(mala.map(|(id, texto)| {
        format!(
            "la atención de urgencias {id} tiene «{texto}» como fecha de ingreso, \
             que no es una fecha. No se genera el RIPS: con ese valor la atención \
             no entra en ningún periodo y se perdería del informe sin avisar."
        )
    }))
}

/// Las hospitalizaciones del periodo, de UNA institución, listas para el AH.
///
/// **Solo las cerradas.** Una hospitalización activa no tiene estancia que
/// facturar; el sistema viejo la reportaba igual, con el egreso puesto en el
/// instante de la generación. Se exige `fecha_egreso` **y** que el estado no sea
/// `ACTIVA`: son dos formas de decir lo mismo y en una base real se
/// desincronizan, así que se piden las dos.
///
/// El periodo lo fija el INGRESO, como el Python: una estancia que empieza en
/// marzo y termina en abril es del cierre de marzo.
pub async fn hospitalizaciones_del_periodo(
    pool: &crate::Pool,
    institucion_id: i64,
    desde: NaiveDate,
    hasta: NaiveDate,
) -> Result<Result<Vec<FuenteHospitalizacion>, String>, sqlx::Error> {
    if let Some(mala) = fecha_ilegible_de_hospitalizacion(pool, institucion_id).await? {
        return Ok(Err(mala));
    }
    let (ini, fin) = limites_utc(desde, hasta);
    let filas: Vec<FilaHospitalizacion> = sqlx::query_as(
        "SELECT o.id, o.fecha_ingreso, o.fecha_egreso, o.via_ingreso, o.estado, \
                o.causa_motivo, o.diagnostico_ingreso, o.diagnostico_egreso, \
                o.diagnostico_causa_muerte, \
                p.id AS paciente_id, \
                p.tipo_documento   AS p_tipo_documento, \
                p.numero_documento AS p_numero_documento, \
                p.tipo_usuario, p.fecha_nacimiento, p.sexo, p.cod_pais, \
                p.cod_municipio, p.zona, p.cod_pais_origen \
         FROM hospitalizacion o \
         JOIN paciente p ON p.id = o.paciente_id \
         WHERE p.institucion_id = ? AND o.fecha_ingreso >= ? AND o.fecha_ingreso <= ? \
           AND o.fecha_egreso IS NOT NULL AND o.estado <> 'ACTIVA' \
         ORDER BY o.fecha_ingreso, o.id",
    )
    .bind(institucion_id)
    .bind(&ini)
    .bind(&fin)
    .fetch_all(pool)
    .await?;

    let mut salida = Vec::with_capacity(filas.len());
    for f in filas {
        let ingreso = match fecha_local(&f.fecha_ingreso, "la hospitalización", f.id) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        let egreso = match f
            .fecha_egreso
            .as_deref()
            .map(|t| fecha_local(t, "la hospitalización", f.id))
        {
            Some(Ok(v)) => v,
            Some(Err(e)) => return Ok(Err(e)),
            None => continue,
        };
        salida.push(FuenteHospitalizacion {
            id: f.id,
            fecha_ingreso: ingreso,
            fecha_egreso: egreso,
            via_ingreso: f.via_ingreso,
            estado: f.estado,
            causa_motivo: f.causa_motivo,
            diagnostico_ingreso: f.diagnostico_ingreso,
            diagnostico_egreso: f.diagnostico_egreso,
            diagnostico_causa_muerte: f.diagnostico_causa_muerte,
            paciente: FuentePaciente {
                id: f.paciente_id,
                tipo_documento: f.p_tipo_documento,
                numero_documento: f.p_numero_documento,
                tipo_usuario: f.tipo_usuario,
                fecha_nacimiento: f.fecha_nacimiento,
                sexo: f.sexo,
                cod_pais: f.cod_pais,
                cod_municipio: f.cod_municipio,
                zona: f.zona,
                cod_pais_origen: f.cod_pais_origen,
            },
        });
    }
    Ok(Ok(salida))
}

/// El guardián de fechas, sobre la tabla de hospitalización. Va aparte por lo
/// mismo que el de urgencias: no cuelga de una historia.
async fn fecha_ilegible_de_hospitalizacion(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let mala: Option<(i64, String)> = sqlx::query_as(
        "SELECT o.id, o.fecha_ingreso FROM hospitalizacion o \
         JOIN paciente p ON p.id = o.paciente_id \
         WHERE p.institucion_id = ? \
           AND o.fecha_ingreso NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]*' \
         ORDER BY o.id LIMIT 1",
    )
    .bind(institucion_id)
    .fetch_optional(pool)
    .await?;
    Ok(mala.map(|(id, texto)| {
        format!(
            "la hospitalización {id} tiene «{texto}» como fecha de ingreso, que no \
             es una fecha. No se genera el RIPS: con ese valor la estancia no \
             entra en ningún periodo y se perdería del informe sin avisar."
        )
    }))
}

/// La fila cruda del AH. Tampoco trae profesional.
#[derive(sqlx::FromRow)]
struct FilaHospitalizacion {
    id: i64,
    fecha_ingreso: String,
    fecha_egreso: Option<String>,
    via_ingreso: Option<String>,
    estado: Option<String>,
    causa_motivo: Option<String>,
    diagnostico_ingreso: Option<String>,
    diagnostico_egreso: Option<String>,
    diagnostico_causa_muerte: Option<String>,
    paciente_id: i64,
    p_tipo_documento: String,
    p_numero_documento: String,
    tipo_usuario: Option<String>,
    fecha_nacimiento: String,
    sexo: String,
    cod_pais: Option<String>,
    cod_municipio: Option<String>,
    zona: Option<String>,
    cod_pais_origen: Option<String>,
}

/// La fila cruda del AU. No trae profesional: el AU no admite los campos
/// P11/P12, y pedirlos aquí habría sido la manera de que acabaran en el JSON.
#[derive(sqlx::FromRow)]
struct FilaUrgencia {
    id: i64,
    fecha_ingreso: String,
    fecha_egreso: Option<String>,
    causa_motivo: Option<String>,
    diagnostico_ingreso: Option<String>,
    diagnostico_egreso: Option<String>,
    destino_salida: Option<String>,
    diagnostico_causa_muerte: Option<String>,
    paciente_id: i64,
    p_tipo_documento: String,
    p_numero_documento: String,
    tipo_usuario: Option<String>,
    fecha_nacimiento: String,
    sexo: String,
    cod_pais: Option<String>,
    cod_municipio: Option<String>,
    zona: Option<String>,
    cod_pais_origen: Option<String>,
}

/// La fila cruda del AP. Mismo aliasado que la del AC y por lo mismo: `id`,
/// `tipo_documento` y `numero_documento` están en tres tablas del `SELECT`.
#[derive(sqlx::FromRow)]
struct FilaProcedimiento {
    id: i64,
    fecha: String,
    codigo_cups: Option<String>,
    tipo: String,
    via_ingreso_servicio: Option<String>,
    grupo_servicios: Option<String>,
    cod_servicio: Option<i64>,
    finalidad: Option<String>,
    diagnostico_principal: Option<String>,
    diagnostico_relacionado1: Option<String>,
    valor_servicio: Option<i64>,
    concepto_recaudo: Option<String>,
    valor_pago_moderador: Option<i64>,
    num_fev_pago_moderador: Option<String>,
    paciente_id: i64,
    p_tipo_documento: String,
    p_numero_documento: String,
    tipo_usuario: Option<String>,
    fecha_nacimiento: String,
    sexo: String,
    cod_pais: Option<String>,
    cod_municipio: Option<String>,
    zona: Option<String>,
    cod_pais_origen: Option<String>,
    m_tipo_documento: Option<String>,
    m_numero_documento: Option<String>,
}

/// La fila cruda del AM.
#[derive(sqlx::FromRow)]
struct FilaMedicamento {
    id: i64,
    fecha: String,
    codigo_cum: Option<String>,
    tipo_medicamento: Option<String>,
    medicamento: String,
    diagnostico_principal: Option<String>,
    unidad: Option<String>,
    /// `REAL` en la base: la cantidad puede venir fraccionaria, y el dominio se
    /// niega a truncarla.
    cantidad: f64,
    dias_tratamiento: Option<i64>,
    valor_unitario: Option<i64>,
    valor_servicio: Option<i64>,
    concepto_recaudo: Option<String>,
    valor_pago_moderador: Option<i64>,
    num_fev_pago_moderador: Option<String>,
    paciente_id: i64,
    p_tipo_documento: String,
    p_numero_documento: String,
    tipo_usuario: Option<String>,
    fecha_nacimiento: String,
    sexo: String,
    cod_pais: Option<String>,
    cod_municipio: Option<String>,
    zona: Option<String>,
    cod_pais_origen: Option<String>,
    m_tipo_documento: Option<String>,
    m_numero_documento: Option<String>,
}

/// La fila cruda. `FromRow` casa por NOMBRE de columna, no por posición, y tres
/// tablas del `SELECT` repiten nombres —`id`, `tipo_documento` y
/// `numero_documento` están en `paciente` y en `usuario`—. Por eso el SQL las
/// aliasa: sin alias, sqlx tomaría la primera coincidencia y el documento del
/// médico saldría siendo el del paciente, que es EXACTAMENTE el fallo que este
/// port viene a arreglar y no se habría notado.
#[derive(sqlx::FromRow)]
struct Fila {
    id: i64,
    fecha: String,
    cod_consulta_cups: Option<String>,
    modalidad_grupo_servicio: Option<String>,
    grupo_servicios: Option<String>,
    cod_servicio: Option<i64>,
    finalidad: Option<String>,
    causa_externa: Option<String>,
    diagnostico_principal: Option<String>,
    diagnostico_relacionado1: Option<String>,
    diagnostico_relacionado2: Option<String>,
    diagnostico_relacionado3: Option<String>,
    tipo_diagnostico: Option<String>,
    valor_servicio: Option<i64>,
    concepto_recaudo: Option<String>,
    valor_pago_moderador: Option<i64>,
    num_fev_pago_moderador: Option<String>,
    genera_incapacidad: i64,
    paciente_id: i64,
    p_tipo_documento: String,
    p_numero_documento: String,
    tipo_usuario: Option<String>,
    fecha_nacimiento: String,
    sexo: String,
    cod_pais: Option<String>,
    cod_municipio: Option<String>,
    zona: Option<String>,
    cod_pais_origen: Option<String>,
    m_tipo_documento: Option<String>,
    m_numero_documento: Option<String>,
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos instituciones, dos médicos, dos pacientes. La 2 existe para que las
    /// pruebas de aislamiento tengan algo que NO deba salir.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401'),(2,'Dos','900999','050010999901')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
             tipo_documento,numero_documento) \
             VALUES (5,'Dra','d@x.co','h','medico',1,1,'CC','71234567'), \
                    (6,'Dr Dos','d2@x.co','h','medico',2,1,'CC','80000000'), \
                    (7,'Sin doc','s@x.co','h','medico',1,1,NULL,NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id,tipo_usuario,cod_pais,cod_municipio,zona,\
             cod_pais_origen) \
             VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1,'01','170','05001','01','170'), \
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2,'01','170','05001','01','170')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    /// Inserta una historia con la fecha EXACTA en el texto UTC de SQLite.
    async fn historia(pool: &crate::Pool, id: i64, paciente: i64, medico: i64, fecha_utc: &str) {
        sqlx::query(
            "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,creado_por,\
             motivo_consulta,enfermedad_actual,plan_tratamiento,diagnostico_principal,\
             cod_consulta_cups,valor_servicio,concepto_recaudo,finalidad) \
             VALUES (?,?,?,?,?,?,'dolor','desde ayer','reposo','A000','890201',45000,'05','11')",
        )
        .bind(id)
        .bind(fecha_utc)
        .bind(format!("H{id}"))
        .bind(paciente)
        .bind(medico)
        .bind(medico)
        .execute(pool)
        .await
        .unwrap();
    }

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).unwrap()
    }

    #[tokio::test]
    async fn trae_la_consulta_con_su_paciente_y_su_medico() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;

        let cs = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].paciente.numero_documento, "1");
        // El del MÉDICO, no el del paciente: son columnas homónimas de dos
        // tablas y el SELECT las aliasa por eso.
        assert_eq!(cs[0].profesional.numero_documento.as_deref(), Some("71234567"));
        assert_eq!(cs[0].paciente.cod_pais_origen.as_deref(), Some("170"));
    }

    #[tokio::test]
    async fn no_trae_las_de_otra_institucion() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;
        historia(&pool, 2, 20, 6, "2026-03-15 14:30:00").await;

        let cs = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cs.len(), 1, "la de la institución 2 no puede salir");
        assert_eq!(cs[0].id, 1);
    }

    #[tokio::test]
    async fn el_periodo_se_pide_en_local_y_la_base_guarda_utc() {
        // Una atención del 31 de marzo a las 20:00 en Bogotá se guarda como el 1
        // de abril a las 01:00 UTC. Tiene que salir en el cierre de MARZO y con
        // fecha de marzo — filtrar en un huso y emitir en otro la pondría en
        // abril, o en marzo pero fechada en abril.
        let pool = base().await;
        // La hora se construye con la zona DEL PRESTADOR, no con la del reloj de
        // la máquina: así la prueba dice lo mismo aquí que en el CI, que va en UTC.
        let en_utc = zona::a_utc(dia(2026, 3, 31).and_hms_opt(20, 0, 0).unwrap())
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        historia(&pool, 1, 10, 5, &en_utc).await;

        let marzo = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(marzo.len(), 1, "guardada como {en_utc}, se cierra en marzo");
        assert_eq!(
            marzo[0].fecha.format("%Y-%m-%d %H:%M").to_string(),
            "2026-03-31 20:00",
            "y se reporta con la hora a la que ocurrió"
        );

        let abril = consultas_del_periodo(&pool, 1, dia(2026, 4, 1), dia(2026, 4, 30))
            .await
            .unwrap()
            .unwrap();
        assert!(abril.is_empty(), "y NO se cuenta dos veces en abril");
    }

    #[tokio::test]
    async fn el_ultimo_dia_entra_entero() {
        let pool = base().await;
        let en_utc = zona::a_utc(dia(2026, 3, 31).and_hms_opt(23, 45, 0).unwrap())
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        historia(&pool, 1, 10, 5, &en_utc).await;

        let cs = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cs.len(), 1, "el 31 incluye hasta las 23:59, no hasta las 00:00");
    }

    #[tokio::test]
    async fn el_orden_es_fijo_para_que_los_consecutivos_no_bailen() {
        let pool = base().await;
        historia(&pool, 3, 10, 5, "2026-03-20 10:00:00").await;
        historia(&pool, 1, 10, 5, "2026-03-10 10:00:00").await;
        historia(&pool, 2, 10, 5, "2026-03-15 10:00:00").await;

        let cs = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        let ids: Vec<i64> = cs.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![1, 2, 3], "por fecha, no por orden de inserción");
    }

    #[tokio::test]
    async fn una_fecha_ilegible_no_se_sustituye_por_hoy() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "el martes pasado").await;

        let r = consultas_del_periodo(&pool, 1, dia(1900, 1, 1), dia(2100, 1, 1))
            .await
            .unwrap();
        let e = r.expect_err("una fecha que no se puede leer tiene que doler");
        assert!(e.contains("historia 1"), "dice CUÁL corregir: {e}");
    }

    #[tokio::test]
    async fn la_fecha_corrupta_no_se_escapa_por_el_filtro_del_periodo() {
        // El caso que motivó la comprobación previa, y que NO se ve sin ella: el
        // filtro compara texto, «el martes pasado» no cae en ningún rango, y la
        // atención se caía del informe en silencio. Una consulta que existió y
        // no se reporta es peor que un error: no deja rastro de que faltaba.
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 10:00:00").await;
        historia(&pool, 2, 10, 5, "el martes pasado").await;

        let r = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap();
        let e = r.expect_err("no se puede cerrar marzo con una historia sin fecha");
        assert!(e.contains("historia 2"), "{e}");
        assert!(e.contains("se perdería"), "dice por qué importa: {e}");
    }

    #[tokio::test]
    async fn la_fecha_corrupta_de_otra_institucion_no_bloquea_esta() {
        // La comprobación se acota a la institución que reporta. Si mirara todas,
        // el descuido de una IPS impediría cerrar el mes a las demás — y eso es
        // exactamente la restricción que la directiva 21 manda probar contra la
        // operación normal antes de escribirla.
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 10:00:00").await;
        historia(&pool, 2, 20, 6, "el martes pasado").await;

        let cs = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .expect("lo de la institución 2 no es asunto de la 1");
        assert_eq!(cs.len(), 1);
    }

    #[tokio::test]
    async fn el_medico_sin_documento_llega_vacio_y_no_se_rellena() {
        let pool = base().await;
        historia(&pool, 1, 10, 7, "2026-03-15 14:30:00").await;

        let cs = consultas_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cs[0].profesional.tipo_documento, None);
        assert_eq!(cs[0].profesional.numero_documento, None);
    }

    // ---------------------------------------------------------------------
    // AP y AM. Las dos consultas nuevas repiten las reglas del AC —periodo en
    // local, aislamiento por el paciente, orden fijo—, así que se prueban las
    // tres otra vez: una regla que se cumple en una consulta y no en la de al
    // lado es exactamente lo que no se ve mirando el código.
    // ---------------------------------------------------------------------

    async fn orden(pool: &crate::Pool, id: i64, historia_id: i64, tipo: &str, cups: Option<&str>) {
        sqlx::query(
            "INSERT INTO orden (id,historia_id,tipo,codigo_cups,descripcion,justificacion) \
             VALUES (?,?,?,?,'algo','porque')",
        )
        .bind(id)
        .bind(historia_id)
        .bind(tipo)
        .bind(cups)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn formula(pool: &crate::Pool, id: i64, historia_id: i64, cantidad: f64) {
        sqlx::query(
            "INSERT INTO formula (id,historia_id,medicamento,codigo_cum,dosis,cantidad,unidad,\
             dias_tratamiento) VALUES (?,?,'Acetaminofén','20048021-41','1 c/8h',?,'tableta',10)",
        )
        .bind(id)
        .bind(historia_id)
        .bind(cantidad)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn trae_el_procedimiento_con_la_fecha_de_la_historia() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;
        orden(&pool, 100, 1, "Laboratorio", Some("902210")).await;

        let ps = procedimientos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].id, 100, "el id es el de la ORDEN, donde se corrige");
        assert_eq!(ps[0].cod_procedimiento_cups.as_deref(), Some("902210"));
        assert_eq!(ps[0].tipo, "Laboratorio");
        // De la historia, no de la orden: la orden no tiene diagnóstico propio.
        assert_eq!(ps[0].diagnostico_principal.as_deref(), Some("A000"));
        assert_eq!(ps[0].finalidad.as_deref(), Some("11"));
        // El del MÉDICO, no el del paciente.
        assert_eq!(ps[0].profesional.numero_documento.as_deref(), Some("71234567"));
        assert_eq!(ps[0].paciente.numero_documento, "1");
        // Los campos que ninguna pantalla captura llegan vacíos, no rellenos.
        assert_eq!(ps[0].via_ingreso_servicio, None);
        assert_eq!(ps[0].valor_servicio, None);
    }

    #[tokio::test]
    async fn los_procedimientos_de_otra_institucion_no_salen() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;
        historia(&pool, 2, 20, 6, "2026-03-15 14:30:00").await;
        orden(&pool, 100, 1, "Laboratorio", Some("902210")).await;
        orden(&pool, 200, 2, "Laboratorio", Some("902210")).await;

        let ps = procedimientos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ps.len(), 1, "la orden de la institución 2 no puede salir");
        assert_eq!(ps[0].id, 100);
    }

    #[tokio::test]
    async fn el_procedimiento_sigue_el_periodo_de_su_atencion() {
        // Una orden de una atención del 31 de marzo a las 20:00 en Bogotá (1 de
        // abril a las 01:00 UTC) va en el cierre de MARZO, con su consulta. Si
        // fuera al otro, las dos mitades de la misma atención acabarían en
        // facturas distintas.
        let pool = base().await;
        let utc = zona::a_utc(
            NaiveDate::from_ymd_opt(2026, 3, 31)
                .unwrap()
                .and_hms_opt(20, 0, 0)
                .unwrap(),
        )
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
        historia(&pool, 1, 10, 5, &utc).await;
        orden(&pool, 100, 1, "Cirugía", Some("902210")).await;

        let marzo = procedimientos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(marzo.len(), 1, "es del cierre de marzo");
        assert_eq!(marzo[0].fecha.date(), dia(2026, 3, 31), "y con fecha de marzo");

        let abril = procedimientos_del_periodo(&pool, 1, dia(2026, 4, 1), dia(2026, 4, 30))
            .await
            .unwrap()
            .unwrap();
        assert!(abril.is_empty(), "y no se cuenta dos veces");
    }

    #[tokio::test]
    async fn el_orden_de_los_procedimientos_es_fijo() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;
        orden(&pool, 300, 1, "Laboratorio", Some("902210")).await;
        orden(&pool, 100, 1, "Terapia", Some("930101")).await;

        let ps = procedimientos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        let ids: Vec<i64> = ps.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![100, 300], "sin orden fijo los consecutivos bailan");
    }

    #[tokio::test]
    async fn trae_la_formula_con_su_cantidad_sin_redondear() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;
        formula(&pool, 100, 1, 2.5).await;

        let ms = medicamentos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].cantidad, Some(2.5), "la fracción llega entera al dominio");
        assert_eq!(ms[0].codigo_cum.as_deref(), Some("20048021-41"));
        assert_eq!(ms[0].unidad.as_deref(), Some("tableta"));
        assert_eq!(ms[0].dias_tratamiento, Some(10));
        // M05: el diagnóstico sale de la historia en que se formuló.
        assert_eq!(ms[0].diagnostico_principal.as_deref(), Some("A000"));
        assert_eq!(ms[0].profesional.numero_documento.as_deref(), Some("71234567"));
        assert_eq!(ms[0].tipo_medicamento, None, "no lo captura ninguna pantalla");
    }

    #[tokio::test]
    async fn las_formulas_de_otra_institucion_no_salen() {
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-03-15 14:30:00").await;
        historia(&pool, 2, 20, 6, "2026-03-15 14:30:00").await;
        formula(&pool, 100, 1, 30.0).await;
        formula(&pool, 200, 2, 30.0).await;

        let ms = medicamentos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].id, 100);
    }

    #[tokio::test]
    async fn la_fecha_de_dispensacion_es_el_dia_local_y_no_el_del_texto_utc() {
        // El AM emite la fecha SIN hora. Quedarse con los diez primeros
        // caracteres del texto UTC daría el día siguiente para toda atención
        // posterior a las 19:00 en Colombia — un medicamento dispensado el 31 de
        // marzo reportado el 1 de abril, en otro cierre.
        let pool = base().await;
        historia(&pool, 1, 10, 5, "2026-04-01 01:00:00").await;
        formula(&pool, 100, 1, 30.0).await;

        let ms = medicamentos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ms.len(), 1, "cae en el cierre de marzo");
        assert_eq!(ms[0].fecha, dia(2026, 3, 31), "y con fecha del 31 de marzo");
    }

    #[tokio::test]
    async fn una_fecha_ilegible_tampoco_deja_generar_el_ap_ni_el_am() {
        // El mismo guardián del AC: una historia con fecha corrupta se queda
        // fuera del filtro de texto y su procedimiento desaparecería del informe
        // sin que nada fallara.
        let pool = base().await;
        historia(&pool, 1, 10, 5, "sin fecha").await;
        orden(&pool, 100, 1, "Laboratorio", Some("902210")).await;
        formula(&pool, 200, 1, 30.0).await;

        let ap = procedimientos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap();
        let am = medicamentos_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap();
        assert!(ap.is_err(), "el AP tiene que negarse");
        assert!(am.is_err(), "y el AM también");
        assert!(ap.unwrap_err().contains("no es una fecha"));
    }

    async fn urgencia(
        pool: &crate::Pool,
        id: i64,
        paciente: i64,
        ingreso: &str,
        egreso: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO atencion_urgencias (id,paciente_id,triage,motivo_consulta,\
             enfermera_triage_id,fecha_ingreso,diagnostico_ingreso,destino_salida,fecha_egreso,\
             causa_motivo,diagnostico_egreso) \
             VALUES (?,?,'TRIAGE_2','dolor torácico',5,?,'A000','DOMICILIO',?,'38','A000')",
        )
        .bind(id)
        .bind(paciente)
        .bind(ingreso)
        .bind(egreso)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn la_atencion_sin_egreso_no_entra_en_el_archivo() {
        // La que sigue en urgencias no ha terminado y no se puede facturar. El
        // sistema viejo le ponía la hora de la generación: un paciente todavía
        // dentro se reportaba como dado de alta.
        let pool = base().await;
        urgencia(&pool, 1, 10, "2026-03-15 14:00:00", Some("2026-03-15 18:00:00")).await;
        urgencia(&pool, 2, 10, "2026-03-16 14:00:00", None).await;

        let us = urgencias_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(us.len(), 1, "solo la que egresó");
        assert_eq!(us[0].id, 1);
        assert_eq!(us[0].destino_salida.as_deref(), Some("DOMICILIO"));
        assert_eq!(us[0].causa_motivo.as_deref(), Some("38"));
        assert_eq!(us[0].diagnostico_causa_muerte, None);
    }

    #[tokio::test]
    async fn las_urgencias_de_otra_institucion_no_salen() {
        let pool = base().await;
        urgencia(&pool, 1, 10, "2026-03-15 14:00:00", Some("2026-03-15 18:00:00")).await;
        urgencia(&pool, 2, 20, "2026-03-15 14:00:00", Some("2026-03-15 18:00:00")).await;

        let us = urgencias_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(us.len(), 1, "la del paciente de la institución 2 no puede salir");
        assert_eq!(us[0].paciente.id, 10);
    }

    #[tokio::test]
    async fn la_urgencia_se_pide_en_local_aunque_la_base_guarde_utc() {
        // Un ingreso del 31 de marzo a las 20:00 en Bogotá se guarda como el 1 de
        // abril a las 01:00 UTC, y va en el cierre de MARZO con fecha de marzo.
        let pool = base().await;
        let utc = |h: u32| {
            zona::a_utc(
                NaiveDate::from_ymd_opt(2026, 3, 31)
                    .unwrap()
                    .and_hms_opt(h, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
        };
        urgencia(&pool, 1, 10, &utc(20), Some(&utc(23))).await;

        let us = urgencias_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(us.len(), 1);
        assert_eq!(us[0].fecha_ingreso.date(), dia(2026, 3, 31));
        assert_eq!(us[0].fecha_egreso.date(), dia(2026, 3, 31));
    }

    #[tokio::test]
    async fn una_fecha_de_urgencias_ilegible_no_se_escapa() {
        // El guardián del AC mira `historia`, y la atención de urgencias NO
        // cuelga de una historia: sin este, una fecha corrupta aquí dejaría la
        // atención fuera del archivo sin que nada fallara.
        let pool = base().await;
        urgencia(&pool, 1, 10, "sin fecha", Some("2026-03-15 18:00:00")).await;

        let r = urgencias_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap();
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("atención de urgencias 1"));
    }

    async fn hospitalizacion(
        pool: &crate::Pool,
        id: i64,
        paciente: i64,
        ingreso: &str,
        egreso: Option<&str>,
        estado: &str,
    ) {
        sqlx::query(
            "INSERT OR IGNORE INTO cama (id,institucion_id,numero,piso,servicio,estado) \
             VALUES (1,1,'101','1','GENERAL','OCUPADA')",
        )
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,\
             fecha_egreso,via_ingreso,estado,diagnostico_ingreso,diagnostico_egreso,causa_motivo) \
             VALUES (?,?,1,5,?,?,'URGENCIAS',?,'A000','A000','38')",
        )
        .bind(id)
        .bind(paciente)
        .bind(ingreso)
        .bind(egreso)
        .bind(estado)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn la_hospitalizacion_abierta_no_entra_en_el_archivo() {
        // Dos formas de decir «sigue dentro» —sin egreso y estado ACTIVA— y se
        // exigen las DOS: en una base real se desincronizan, y con cualquiera de
        // las dos rotas se reportaría una estancia que no ha terminado.
        let pool = base().await;
        hospitalizacion(&pool, 1, 10, "2026-03-10 13:00:00", Some("2026-03-14 16:30:00"), "ALTA_MEDICA").await;
        hospitalizacion(&pool, 2, 10, "2026-03-11 13:00:00", None, "ACTIVA").await;
        hospitalizacion(&pool, 3, 10, "2026-03-12 13:00:00", Some("2026-03-13 10:00:00"), "ACTIVA").await;

        let hs = hospitalizaciones_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hs.len(), 1, "solo la cerrada de verdad");
        assert_eq!(hs[0].id, 1);
        assert_eq!(hs[0].via_ingreso.as_deref(), Some("URGENCIAS"));
        assert_eq!(hs[0].causa_motivo.as_deref(), Some("38"));
    }

    #[tokio::test]
    async fn las_hospitalizaciones_de_otra_institucion_no_salen() {
        let pool = base().await;
        hospitalizacion(&pool, 1, 10, "2026-03-10 13:00:00", Some("2026-03-14 16:30:00"), "ALTA_MEDICA").await;
        hospitalizacion(&pool, 2, 20, "2026-03-10 13:00:00", Some("2026-03-14 16:30:00"), "ALTA_MEDICA").await;

        let hs = hospitalizaciones_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].paciente.id, 10);
    }

    #[tokio::test]
    async fn la_estancia_es_del_cierre_en_que_empezo() {
        // Ingresa el 31 de marzo a las 20:00 (1 de abril 01:00 UTC) y sale en
        // abril: va en el cierre de MARZO, y una sola vez.
        let pool = base().await;
        let utc = |d: u32, h: u32| {
            zona::a_utc(
                NaiveDate::from_ymd_opt(2026, if d > 31 { 4 } else { 3 }, if d > 31 { d - 31 } else { d })
                    .unwrap()
                    .and_hms_opt(h, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
        };
        hospitalizacion(&pool, 1, 10, &utc(31, 20), Some(&utc(33, 9)), "ALTA_MEDICA").await;

        let marzo = hospitalizaciones_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(marzo.len(), 1);
        assert_eq!(marzo[0].fecha_ingreso.date(), dia(2026, 3, 31));
        assert_eq!(marzo[0].fecha_egreso.date(), dia(2026, 4, 2), "el egreso es de abril");

        let abril = hospitalizaciones_del_periodo(&pool, 1, dia(2026, 4, 1), dia(2026, 4, 30))
            .await
            .unwrap()
            .unwrap();
        assert!(abril.is_empty(), "no se cuenta dos veces");
    }

    #[tokio::test]
    async fn una_fecha_de_hospitalizacion_ilegible_no_se_escapa() {
        let pool = base().await;
        hospitalizacion(&pool, 1, 10, "sin fecha", Some("2026-03-14 16:30:00"), "ALTA_MEDICA").await;
        let r = hospitalizaciones_del_periodo(&pool, 1, dia(2026, 3, 1), dia(2026, 3, 31))
            .await
            .unwrap();
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("la hospitalización 1"));
    }

    #[tokio::test]
    async fn el_error_nombra_la_fila_que_hay_que_ir_a_arreglar() {
        // Hay DOS caminos para una fecha ilegible y solo uno estaba probado:
        //
        //   * el guardián (`fecha_ilegible_de*`), que caza lo que no tiene forma
        //     de fecha — «sin fecha»;
        //   * `fecha_local`, que caza lo que SÍ tiene la forma y aun así no es
        //     una fecha — «2026-13-45 99:99:99» pasa el GLOB y no se parsea.
        //
        // El segundo decía «la historia {id}» viniera el id de donde viniera, y
        // en el AP ese id es el de la ORDEN. Mandaba a buscar una historia que,
        // con suerte, ni existe.
        let imposible = "2026-13-45 99:99:99";

        let pool = base().await;
        historia(&pool, 1, 10, 5, imposible).await;
        orden(&pool, 100, 1, "Laboratorio", Some("902210")).await;
        formula(&pool, 200, 1, 30.0).await;
        let ap = procedimientos_del_periodo(&pool, 1, dia(2026, 1, 1), dia(2026, 12, 31))
            .await
            .unwrap();
        let am = medicamentos_del_periodo(&pool, 1, dia(2026, 1, 1), dia(2026, 12, 31))
            .await
            .unwrap();
        let ac = consultas_del_periodo(&pool, 1, dia(2026, 1, 1), dia(2026, 12, 31))
            .await
            .unwrap();
        assert!(ap.clone().unwrap_err().contains("la orden 100"), "{ap:?}");
        assert!(am.clone().unwrap_err().contains("la fórmula 200"), "{am:?}");
        assert!(ac.clone().unwrap_err().contains("la historia 1"), "{ac:?}");
        // Y el caso rojo del propio mensaje: el del AP NO puede decir «historia».
        assert!(!ap.unwrap_err().contains("la historia"));

        let pool = base().await;
        urgencia(&pool, 7, 10, imposible, Some("2026-03-15 18:00:00")).await;
        let au = urgencias_del_periodo(&pool, 1, dia(2026, 1, 1), dia(2026, 12, 31))
            .await
            .unwrap();
        assert!(
            au.clone().unwrap_err().contains("la atención de urgencias 7"),
            "{au:?}"
        );
    }

    #[tokio::test]
    async fn la_institucion_trae_sus_dos_identificadores() {
        let pool = base().await;
        let i = institucion_que_reporta(&pool, 1).await.unwrap().unwrap();
        assert_eq!(i.nit, "900123456", "numDocumentoIdObligado");
        assert_eq!(i.codigo_habilitacion, "050010123401", "codPrestador");
        assert!(institucion_que_reporta(&pool, 99).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn sin_codigo_de_habilitacion_llega_vacio() {
        let pool = base().await;
        sqlx::query("UPDATE institucion SET codigo_habilitacion = NULL WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let i = institucion_que_reporta(&pool, 1).await.unwrap().unwrap();
        assert_eq!(i.codigo_habilitacion, "", "vacío, no un código inventado");
    }
}
