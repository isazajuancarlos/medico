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
use medico_dominio::rips::{FuenteConsulta, FuenteInstitucion, FuentePaciente, FuenteProfesional};

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
/// Acepta las dos formas que existen en la base: la del `DEFAULT` de SQLite
/// (`2026-03-15 09:30:00`) y la RFC 3339 que escribe el código Rust
/// (`2026-03-15T09:30:00Z`). **Lo que no se puede leer no se sustituye por
/// «ahora»**: se devuelve el error con el id de la fila, porque una fecha
/// inventada en el RIPS es una atención reportada en un día en que no ocurrió.
fn fecha_local(texto: &str, historia_id: i64) -> Result<NaiveDateTime, String> {
    if let Ok(t) = DateTime::parse_from_rfc3339(texto) {
        return Ok(t.with_timezone(&zona::zona()).naive_local());
    }
    let naive = NaiveDateTime::parse_from_str(texto, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| format!("la historia {historia_id} tiene «{texto}» como fecha: {e}"))?;
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
        let fecha = match fecha_local(&f.fecha, f.id) {
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
