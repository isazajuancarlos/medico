//! Rehabilitación: planes y sesiones de terapia. Port a Rust de
//! `backend/routers/rehabilitacion.py`.
//!
//! Mismo patrón que nutrición: institucion_id propio por fila, la sesión cuelga
//! del plan (ON DELETE CASCADE), y al crear se sella la institución desde la del
//! PACIENTE (que debe ser del ámbito). Crear una sesión incrementa
//! `sesiones_realizadas` del plan, en la misma transacción.

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

fn filtro_ambito(ambito: Ambito, col: &str) -> Option<(String, Option<i64>)> {
    match ambito {
        Ambito::Superadmin => Some(("1 = 1".into(), None)),
        Ambito::Institucion(i) => Some((format!("{col} = ?"), Some(i))),
        Ambito::Ninguno => None,
    }
}

const PLAN_COLS: &str = "pl.id, pl.paciente_id, pl.terapeuta_id, pl.fecha_creacion, \
    pl.diagnostico_funcional, pl.objetivos, pl.sesiones_autorizadas, pl.sesiones_realizadas, \
    pl.tipo_terapia, pl.frecuencia, pl.duracion_sesion_min, pl.activo, pl.fecha_cierre, \
    pl.resultado_final, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente_nombre, \
    p.numero_documento AS paciente_identificacion, u.nombre AS terapeuta_nombre";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Plan {
    pub id: i64,
    pub paciente_id: i64,
    pub terapeuta_id: i64,
    pub fecha_creacion: String,
    pub diagnostico_funcional: String,
    pub objetivos: String,
    pub sesiones_autorizadas: i64,
    pub sesiones_realizadas: i64,
    pub tipo_terapia: String,
    pub frecuencia: String,
    pub duracion_sesion_min: i64,
    pub activo: bool,
    pub fecha_cierre: Option<String>,
    pub resultado_final: Option<String>,
    pub paciente_nombre: String,
    pub paciente_identificacion: String,
    pub terapeuta_nombre: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanCrear {
    pub paciente_id: i64,
    pub diagnostico_funcional: String,
    pub objetivos: String,
    pub sesiones_autorizadas: Option<i64>,
    pub tipo_terapia: String,
    pub frecuencia: String,
    pub duracion_sesion_min: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlanEditar {
    pub diagnostico_funcional: Option<String>,
    pub objetivos: Option<String>,
    pub sesiones_autorizadas: Option<i64>,
    pub tipo_terapia: Option<String>,
    pub frecuencia: Option<String>,
    pub duracion_sesion_min: Option<i64>,
    pub activo: Option<bool>,
    pub fecha_cierre: Option<String>,
    pub resultado_final: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Sesion {
    pub id: i64,
    pub plan_id: i64,
    pub paciente_id: i64,
    pub terapeuta_id: i64,
    pub fecha_sesion: String,
    pub numero_sesion: i64,
    pub evaluacion_funcional: String,
    pub dolor_eva: Option<i64>,
    pub rango_movimiento: Option<String>,
    pub tecnicas_aplicadas: String,
    pub ejercicios_domicilio: Option<String>,
    pub progreso: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesionCrear {
    pub evaluacion_funcional: String,
    pub dolor_eva: Option<i64>,
    pub rango_movimiento: Option<String>,
    pub tecnicas_aplicadas: String,
    pub ejercicios_domicilio: Option<String>,
    pub progreso: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SesionEditar {
    pub evaluacion_funcional: Option<String>,
    pub dolor_eva: Option<i64>,
    pub rango_movimiento: Option<String>,
    pub tecnicas_aplicadas: Option<String>,
    pub ejercicios_domicilio: Option<String>,
    pub progreso: Option<String>,
    pub observaciones: Option<String>,
}

pub async fn listar_planes(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: Option<i64>,
) -> Result<Vec<Plan>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "pl.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT {PLAN_COLS} FROM plan_rehabilitacion pl \
         JOIN paciente p ON p.id = pl.paciente_id \
         JOIN usuario u ON u.id = pl.terapeuta_id WHERE {filtro}"
    );
    if paciente_id.is_some() {
        sql.push_str(" AND pl.paciente_id = ?");
    }
    sql.push_str(" ORDER BY pl.fecha_creacion DESC");
    let mut q = sqlx::query_as::<_, Plan>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(p) = paciente_id {
        q = q.bind(p);
    }
    q.fetch_all(pool).await
}

pub async fn obtener_plan(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<Plan>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "pl.institucion_id") else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {PLAN_COLS} FROM plan_rehabilitacion pl \
         JOIN paciente p ON p.id = pl.paciente_id \
         JOIN usuario u ON u.id = pl.terapeuta_id WHERE pl.id = ? AND {filtro}"
    );
    let mut q = sqlx::query_as::<_, Plan>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

pub async fn crear_plan(
    pool: &crate::Pool,
    ambito: Ambito,
    terapeuta_id: i64,
    p: &PlanCrear,
) -> Result<Option<i64>, sqlx::Error> {
    let inst: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(p.paciente_id)
        .fetch_optional(pool)
        .await?;
    let Some((institucion,)) = inst.filter(|(i,)| ambito.puede_ver(Some(*i)) == Visibilidad::Si) else {
        return Ok(None);
    };
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let r = sqlx::query(
        "INSERT INTO plan_rehabilitacion \
         (paciente_id, terapeuta_id, fecha_creacion, diagnostico_funcional, objetivos, \
          sesiones_autorizadas, tipo_terapia, frecuencia, duracion_sesion_min, institucion_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(p.paciente_id)
    .bind(terapeuta_id)
    .bind(&fecha)
    .bind(&p.diagnostico_funcional)
    .bind(&p.objetivos)
    .bind(p.sesiones_autorizadas.unwrap_or(10))
    .bind(&p.tipo_terapia)
    .bind(&p.frecuencia)
    .bind(p.duracion_sesion_min.unwrap_or(45))
    .bind(institucion)
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

pub async fn actualizar_plan(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    p: &PlanEditar,
) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    let sql = format!(
        "UPDATE plan_rehabilitacion SET \
         diagnostico_funcional=COALESCE(?,diagnostico_funcional), objetivos=COALESCE(?,objetivos), \
         sesiones_autorizadas=COALESCE(?,sesiones_autorizadas), tipo_terapia=COALESCE(?,tipo_terapia), \
         frecuencia=COALESCE(?,frecuencia), duracion_sesion_min=COALESCE(?,duracion_sesion_min), \
         activo=COALESCE(?,activo), fecha_cierre=COALESCE(?,fecha_cierre), \
         resultado_final=COALESCE(?,resultado_final) WHERE id = ? AND {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(&p.diagnostico_funcional).bind(&p.objetivos)
        .bind(p.sesiones_autorizadas).bind(&p.tipo_terapia)
        .bind(&p.frecuencia).bind(p.duracion_sesion_min)
        .bind(p.activo).bind(&p.fecha_cierre)
        .bind(&p.resultado_final).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

const SESION_COLS: &str = "id, plan_id, paciente_id, terapeuta_id, fecha_sesion, numero_sesion, \
    evaluacion_funcional, dolor_eva, rango_movimiento, tecnicas_aplicadas, ejercicios_domicilio, \
    progreso, observaciones";

/// Sesiones de un plan del ámbito, de la más reciente a la más antigua. Vacío si
/// el plan es ajeno o no existe.
pub async fn listar_sesiones(
    pool: &crate::Pool,
    ambito: Ambito,
    plan_id: i64,
) -> Result<Vec<Sesion>, sqlx::Error> {
    if obtener_plan(pool, ambito, plan_id).await?.is_none() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Sesion>(&format!(
        "SELECT {SESION_COLS} FROM sesion_terapia WHERE plan_id = ? ORDER BY fecha_sesion DESC, id DESC"
    ))
    .bind(plan_id)
    .fetch_all(pool)
    .await
}

/// Una sesión del ámbito por id. `None` si es ajena o no existe.
pub async fn obtener_sesion(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Sesion>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(None);
    };
    let sql = format!("SELECT {SESION_COLS} FROM sesion_terapia WHERE id = ? AND {filtro}");
    let mut q = sqlx::query_as::<_, Sesion>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Registra una sesión en un plan del ámbito e incrementa `sesiones_realizadas`
/// en la misma transacción. `None` si el plan es ajeno o no existe.
pub async fn crear_sesion(
    pool: &crate::Pool,
    ambito: Ambito,
    plan_id: i64,
    terapeuta_id: i64,
    s: &SesionCrear,
) -> Result<Option<i64>, sqlx::Error> {
    let Some(plan) = obtener_plan(pool, ambito, plan_id).await? else {
        return Ok(None);
    };
    let institucion: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(plan.paciente_id)
        .fetch_optional(pool)
        .await?;
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    // El número de sesión es el siguiente al contador actual del plan.
    let numero = plan.sesiones_realizadas + 1;
    let mut tx = pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO sesion_terapia \
         (plan_id, paciente_id, terapeuta_id, fecha_sesion, numero_sesion, evaluacion_funcional, dolor_eva, \
          rango_movimiento, tecnicas_aplicadas, ejercicios_domicilio, progreso, observaciones, institucion_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(plan_id)
    .bind(plan.paciente_id)
    .bind(terapeuta_id)
    .bind(&fecha)
    .bind(numero)
    .bind(&s.evaluacion_funcional)
    .bind(s.dolor_eva)
    .bind(&s.rango_movimiento)
    .bind(&s.tecnicas_aplicadas)
    .bind(&s.ejercicios_domicilio)
    .bind(&s.progreso)
    .bind(&s.observaciones)
    .bind(institucion.map(|(i,)| i))
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE plan_rehabilitacion SET sesiones_realizadas = sesiones_realizadas + 1 WHERE id = ?")
        .bind(plan_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(r.last_insert_rowid()))
}

pub async fn actualizar_sesion(
    pool: &crate::Pool,
    ambito: Ambito,
    sesion_id: i64,
    s: &SesionEditar,
) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    let sql = format!(
        "UPDATE sesion_terapia SET \
         evaluacion_funcional=COALESCE(?,evaluacion_funcional), dolor_eva=COALESCE(?,dolor_eva), \
         rango_movimiento=COALESCE(?,rango_movimiento), tecnicas_aplicadas=COALESCE(?,tecnicas_aplicadas), \
         ejercicios_domicilio=COALESCE(?,ejercicios_domicilio), progreso=COALESCE(?,progreso), \
         observaciones=COALESCE(?,observaciones) WHERE id = ? AND {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(&s.evaluacion_funcional).bind(s.dolor_eva)
        .bind(&s.rango_movimiento).bind(&s.tecnicas_aplicadas)
        .bind(&s.ejercicios_domicilio).bind(&s.progreso)
        .bind(&s.observaciones).bind(sesion_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let url = format!(
            "sqlite:file:rehab_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Tera','t@x.co','x','terapeuta',1,1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)").execute(&pool).await.unwrap();
        pool
    }

    fn plan(paciente_id: i64) -> PlanCrear {
        PlanCrear {
            paciente_id,
            diagnostico_funcional: "Limitación de rodilla".into(),
            objetivos: "Recuperar rango".into(),
            sesiones_autorizadas: None,
            tipo_terapia: "FISICA".into(),
            frecuencia: "3/semana".into(),
            duracion_sesion_min: None,
        }
    }

    fn sesion() -> SesionCrear {
        SesionCrear {
            evaluacion_funcional: "flexión 90°".into(),
            dolor_eva: Some(4),
            rango_movimiento: Some("90°".into()),
            tecnicas_aplicadas: "movilización".into(),
            ejercicios_domicilio: None,
            progreso: Some("MEJORIA".into()),
            observaciones: None,
        }
    }

    #[tokio::test]
    async fn crear_plan_scoped_y_enriquecido_con_defectos() {
        let pool = base().await;
        let id = crear_plan(&pool, Ambito::institucion(1), 5, &plan(10)).await.unwrap().unwrap();
        assert!(crear_plan(&pool, Ambito::institucion(1), 5, &plan(20)).await.unwrap().is_none()); // ajeno
        let p = obtener_plan(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p.paciente_nombre, "Ana Ruiz");
        assert_eq!(p.terapeuta_nombre, "Tera");
        assert_eq!(p.sesiones_autorizadas, 10); // defecto
        assert_eq!(p.duracion_sesion_min, 45);  // defecto
        assert_eq!(p.sesiones_realizadas, 0);
        assert!(obtener_plan(&pool, Ambito::institucion(2), id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn crear_sesion_incrementa_el_contador_y_respeta_el_ambito() {
        let pool = base().await;
        let plan_id = crear_plan(&pool, Ambito::institucion(1), 5, &plan(10)).await.unwrap().unwrap();
        // Plan ajeno: no se crea sesión.
        assert!(crear_sesion(&pool, Ambito::institucion(2), plan_id, 5, &sesion()).await.unwrap().is_none());
        crear_sesion(&pool, Ambito::institucion(1), plan_id, 5, &sesion()).await.unwrap().unwrap();
        crear_sesion(&pool, Ambito::institucion(1), plan_id, 5, &sesion()).await.unwrap().unwrap();
        let p = obtener_plan(&pool, Ambito::institucion(1), plan_id).await.unwrap().unwrap();
        assert_eq!(p.sesiones_realizadas, 2);
        let ses = listar_sesiones(&pool, Ambito::institucion(1), plan_id).await.unwrap();
        assert_eq!(ses.len(), 2);
        // Numeradas 1 y 2 (la lista viene de la más reciente a la más antigua).
        assert_eq!(ses[0].numero_sesion, 2);
        assert_eq!(ses[1].numero_sesion, 1);
        assert!(listar_sesiones(&pool, Ambito::institucion(2), plan_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn actualizar_plan_es_parcial_y_scoped() {
        let pool = base().await;
        let id = crear_plan(&pool, Ambito::institucion(1), 5, &plan(10)).await.unwrap().unwrap();
        let edit = PlanEditar { activo: Some(false), resultado_final: Some("alta".into()), ..Default::default() };
        assert!(!actualizar_plan(&pool, Ambito::institucion(2), id, &edit).await.unwrap()); // ajeno
        assert!(actualizar_plan(&pool, Ambito::institucion(1), id, &edit).await.unwrap());
        let p = obtener_plan(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert!(!p.activo);
        assert_eq!(p.resultado_final.as_deref(), Some("alta"));
        assert_eq!(p.tipo_terapia, "FISICA"); // intacto
    }
}
