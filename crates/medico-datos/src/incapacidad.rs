//! Incapacidades. Port a Rust de `backend/routers/incapacidades.py`.
//!
//! No hay tabla de incapacidad: son una VISTA de solo lectura sobre `historia`
//! (las que llevan `genera_incapacidad`). Filtra por la institución del PACIENTE
//! (historia no lleva institucion_id), como el resto. El resumen se calcula sobre
//! EL MISMO conjunto que lista, no con una consulta aparte que discrepe.
//!
//! `fecha_fin = fecha_inicio + (dias - 1)`: una incapacidad de 1 día empieza y
//! termina el mismo día (el original arrastraba un día de más). Si la consulta
//! está marcada como incapacitante pero sin días (< 1), las fechas van nulas: de
//! ellas depende cuándo se reincorpora el trabajador, no se inventan.

use medico_dominio::{Ambito, Visibilidad};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Incapacidad {
    pub historia_id: i64,
    pub codigo_historia: String,
    pub fecha_consulta: String,
    pub paciente: String,
    pub identificacion: String,
    pub dias: i64,
    pub fecha_inicio: Option<String>,
    pub fecha_fin: Option<String>,
    pub diagnostico: String,
    pub medico: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Detalle {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub base: Incapacidad,
    pub motivo_consulta: Option<String>,
    pub diagnostico_relacionado: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Resumen {
    pub cantidad: i64,
    pub dias_totales: i64,
    pub promedio_dias: f64,
}

/// Filtros de la pantalla, compartidos por listar y resumen.
#[derive(Debug, Clone, Default)]
pub struct Filtros<'a> {
    pub desde: Option<&'a str>,
    pub hasta: Option<&'a str>,
    pub medico_id: Option<i64>,
    pub buscar: Option<&'a str>,
}

// Las fechas se calculan en SQL: inicio = date(fecha); fin = date(fecha, +N días)
// con N = dias-1. `dias`/fechas van a 0/NULL si no hay al menos un día.
const CAMPOS: &str = "h.id AS historia_id, h.codigo_historia, h.fecha AS fecha_consulta, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente, \
    p.numero_documento AS identificacion, \
    CASE WHEN h.dias_incapacidad >= 1 THEN h.dias_incapacidad ELSE 0 END AS dias, \
    CASE WHEN h.dias_incapacidad >= 1 THEN date(h.fecha) ELSE NULL END AS fecha_inicio, \
    CASE WHEN h.dias_incapacidad >= 1 THEN date(h.fecha, '+' || (h.dias_incapacidad - 1) || ' days') ELSE NULL END AS fecha_fin, \
    h.diagnostico_principal AS diagnostico, u.nombre AS medico";

fn where_filtros(ambito: Ambito, f: &Filtros) -> Option<(String, Vec<String>, Option<i64>)> {
    // (cláusula, binds de texto en orden, institucion opcional)
    let inst = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return None,
    };
    let mut clausula = String::from(" WHERE h.genera_incapacidad = 1");
    let mut textos = Vec::new();
    if let Some(d) = f.desde {
        clausula.push_str(" AND date(h.fecha) >= date(?)");
        textos.push(d.to_string());
    }
    if let Some(h) = f.hasta {
        clausula.push_str(" AND date(h.fecha) <= date(?)");
        textos.push(h.to_string());
    }
    if let Some(m) = f.medico_id {
        clausula.push_str(&format!(" AND h.medico_id = {m}"));
    }
    if let Some(b) = f.buscar {
        clausula.push_str(" AND (p.nombre1 LIKE ? OR p.apellido1 LIKE ? OR p.numero_documento LIKE ?)");
        let aguja = format!("%{b}%");
        textos.push(aguja.clone());
        textos.push(aguja.clone());
        textos.push(aguja);
    }
    if inst.is_some() {
        clausula.push_str(" AND p.institucion_id = ?");
    }
    Some((clausula, textos, inst))
}

/// Lista las incapacidades del ámbito según los filtros, de la más reciente a la
/// más antigua.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &Filtros<'_>,
) -> Result<Vec<Incapacidad>, sqlx::Error> {
    let Some((clausula, textos, inst)) = where_filtros(ambito, f) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT {CAMPOS} FROM historia h JOIN paciente p ON p.id = h.paciente_id \
         JOIN usuario u ON u.id = h.medico_id{clausula} ORDER BY h.fecha DESC"
    );
    let mut q = sqlx::query_as::<_, Incapacidad>(&sql);
    for t in &textos {
        q = q.bind(t);
    }
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Cuenta y días del MISMO conjunto que `listar`.
pub async fn resumen(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &Filtros<'_>,
) -> Result<Resumen, sqlx::Error> {
    let Some((clausula, textos, inst)) = where_filtros(ambito, f) else {
        return Ok(Resumen { cantidad: 0, dias_totales: 0, promedio_dias: 0.0 });
    };
    let sql = format!(
        "SELECT COUNT(*) AS cantidad, \
         COALESCE(SUM(CASE WHEN h.dias_incapacidad >= 1 THEN h.dias_incapacidad ELSE 0 END),0) AS dias_totales \
         FROM historia h JOIN paciente p ON p.id = h.paciente_id{clausula}"
    );
    let mut q = sqlx::query_as::<_, (i64, i64)>(&sql);
    for t in &textos {
        q = q.bind(t);
    }
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let (cantidad, dias_totales) = q.fetch_one(pool).await?;
    let promedio_dias = if cantidad > 0 {
        (dias_totales as f64 / cantidad as f64 * 100.0).round() / 100.0
    } else {
        0.0
    };
    Ok(Resumen { cantidad, dias_totales, promedio_dias })
}

/// El detalle de una incapacidad del ámbito para imprimir la certificación.
/// `None` si la historia es ajena, no existe o no es incapacitante.
pub async fn detalle(
    pool: &crate::Pool,
    ambito: Ambito,
    historia_id: i64,
) -> Result<Option<Detalle>, sqlx::Error> {
    let inst = match ambito {
        Ambito::Superadmin => "1 = 1".to_string(),
        Ambito::Institucion(_) => "p.institucion_id = ?".to_string(),
        Ambito::Ninguno => return Ok(None),
    };
    let sql = format!(
        "SELECT {CAMPOS}, h.motivo_consulta, h.diagnostico_relacionado1 AS diagnostico_relacionado, \
         h.plan_tratamiento AS observaciones \
         FROM historia h JOIN paciente p ON p.id = h.paciente_id JOIN usuario u ON u.id = h.medico_id \
         WHERE h.id = ? AND h.genera_incapacidad = 1 AND {inst}"
    );
    let mut q = sqlx::query_as::<_, Detalle>(&sql).bind(historia_id);
    if let Ambito::Institucion(i) = ambito {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let url = format!(
            "sqlite:file:incap_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dr House','h@x.co','x','medico',1,1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F',1),(20,'CC','222','Beto','Paz','1985-01-01','M',2)").execute(&pool).await.unwrap();
        pool
    }

    async fn historia(pool: &crate::Pool, id: i64, paciente: i64, genera: i64, dias: i64, fecha: &str) {
        sqlx::query(
            "INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,\
             enfermedad_actual,diagnostico_principal,plan_tratamiento,fecha,genera_incapacidad,dias_incapacidad) \
             VALUES (?,?,?,5,'dolor','x','J00','reposo',?,?,?)",
        )
        .bind(id).bind(format!("HC{id}")).bind(paciente).bind(fecha).bind(genera).bind(dias)
        .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn lista_solo_incapacidades_del_ambito_con_fechas_calculadas() {
        let pool = base().await;
        historia(&pool, 100, 10, 1, 3, "2026-06-10T09:00:00Z").await;   // incapacita 3 días
        historia(&pool, 101, 10, 0, 0, "2026-06-11T09:00:00Z").await;   // NO incapacita
        historia(&pool, 200, 20, 1, 5, "2026-06-12T09:00:00Z").await;   // otra institución
        let lista = listar(&pool, Ambito::institucion(1), &Filtros::default()).await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].historia_id, 100);
        assert_eq!(lista[0].paciente, "Ana Ruiz");
        assert_eq!(lista[0].dias, 3);
        // 3 días desde el 10 → termina el 12 (no el 13: sin día de más).
        assert_eq!(lista[0].fecha_inicio.as_deref(), Some("2026-06-10"));
        assert_eq!(lista[0].fecha_fin.as_deref(), Some("2026-06-12"));
        // La institución 2 ve la suya, no la de la 1.
        assert_eq!(listar(&pool, Ambito::institucion(2), &Filtros::default()).await.unwrap().len(), 1);
        assert!(listar(&pool, Ambito::Ninguno, &Filtros::default()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn el_resumen_cuadra_con_la_lista_filtrada() {
        let pool = base().await;
        historia(&pool, 100, 10, 1, 3, "2026-06-10T09:00:00Z").await;
        historia(&pool, 102, 10, 1, 5, "2026-07-10T09:00:00Z").await;
        // Sin filtro: 2 incapacidades, 8 días, promedio 4.
        let r = resumen(&pool, Ambito::institucion(1), &Filtros::default()).await.unwrap();
        assert_eq!(r.cantidad, 2);
        assert_eq!(r.dias_totales, 8);
        assert_eq!(r.promedio_dias, 4.0);
        // Con filtro de fecha (solo junio): 1 incapacidad, 3 días — y la lista coincide.
        let f = Filtros { desde: Some("2026-06-01"), hasta: Some("2026-06-30"), ..Default::default() };
        assert_eq!(resumen(&pool, Ambito::institucion(1), &f).await.unwrap().cantidad, 1);
        assert_eq!(listar(&pool, Ambito::institucion(1), &f).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn el_detalle_trae_los_campos_de_la_certificacion_y_respeta_el_ambito() {
        let pool = base().await;
        historia(&pool, 100, 10, 1, 3, "2026-06-10T09:00:00Z").await;
        let d = detalle(&pool, Ambito::institucion(1), 100).await.unwrap().unwrap();
        assert_eq!(d.base.dias, 3);
        assert_eq!(d.motivo_consulta.as_deref(), Some("dolor"));
        assert_eq!(d.observaciones.as_deref(), Some("reposo"));
        // Ajeno / no incapacitante: no hay detalle.
        assert!(detalle(&pool, Ambito::institucion(2), 100).await.unwrap().is_none());
    }
}
