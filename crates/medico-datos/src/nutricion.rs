//! Nutrición: valoraciones nutricionales y su plan de alimentación. Port a Rust
//! de `backend/routers/nutricion.py`.
//!
//! Ambas entidades llevan `institucion_id` propio y se filtran por él, como el
//! original (ajeno == inexistente). Al CREAR, la institución se sella desde la
//! del PACIENTE (que debe ser del ámbito), no se confía en el cliente. El plan
//! cuelga de la valoración; los campos de dieta son JSON.

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;

// --- Valoración --------------------------------------------------------------

const VAL_COLS: &str = "v.id, v.paciente_id, v.hospitalizacion_id, v.nutricionista_id, \
    v.fecha_valoracion, v.peso, v.talla, v.imc, v.circunferencia_brazo, v.pliegue_tricipital, \
    v.screening_nutricional, v.puntaje_screening, v.riesgo_nutricional, v.requerimiento_calorico, \
    v.requerimiento_proteico, v.albumina, v.prealbumina, v.diagnostico_nutricional, \
    v.observaciones, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente_nombre, \
    p.numero_documento AS paciente_identificacion, u.nombre AS nutricionista_nombre";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Valoracion {
    pub id: i64,
    pub paciente_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub nutricionista_id: i64,
    pub fecha_valoracion: String,
    pub peso: f64,
    pub talla: f64,
    pub imc: f64,
    pub circunferencia_brazo: Option<f64>,
    pub pliegue_tricipital: Option<f64>,
    pub screening_nutricional: String,
    pub puntaje_screening: Option<i64>,
    pub riesgo_nutricional: String,
    pub requerimiento_calorico: Option<f64>,
    pub requerimiento_proteico: Option<f64>,
    pub albumina: Option<f64>,
    pub prealbumina: Option<f64>,
    pub diagnostico_nutricional: String,
    pub observaciones: Option<String>,
    pub paciente_nombre: String,
    pub paciente_identificacion: String,
    pub nutricionista_nombre: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValoracionCrear {
    pub paciente_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub peso: f64,
    pub talla: f64,
    /// **Se acepta y se IGNORA** (2026-08-08). El índice lo deriva el servidor
    /// de `peso` y `talla` con `medico_dominio::antropometria::imc`; hasta esa
    /// fecha se guardaba el que mandara el cliente, así que
    /// `{"peso":70,"talla":1.70,"imc":18}` entraba en la historia clínica
    /// contradiciéndose a sí mismo.
    ///
    /// Se conserva en el cuerpo —en vez de rechazarlo— para no romperle la
    /// llamada a quien la tuviera escrita: es el mismo criterio que `parcial` en
    /// `/api/rips`. Un campo que se ignora no puede mentir; uno que se guarda,
    /// sí.
    #[serde(default)]
    pub imc: f64,
    pub circunferencia_brazo: Option<f64>,
    pub pliegue_tricipital: Option<f64>,
    pub screening_nutricional: Option<String>,
    pub puntaje_screening: Option<i64>,
    pub riesgo_nutricional: Option<String>,
    pub requerimiento_calorico: Option<f64>,
    pub requerimiento_proteico: Option<f64>,
    pub albumina: Option<f64>,
    pub prealbumina: Option<f64>,
    pub diagnostico_nutricional: String,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ValoracionEditar {
    pub peso: Option<f64>,
    pub talla: Option<f64>,
    /// Se acepta y se ignora, igual que en el alta: el índice se recalcula con
    /// el peso y la talla que queden DESPUÉS de la edición.
    pub imc: Option<f64>,
    pub circunferencia_brazo: Option<f64>,
    pub pliegue_tricipital: Option<f64>,
    pub screening_nutricional: Option<String>,
    pub puntaje_screening: Option<i64>,
    pub riesgo_nutricional: Option<String>,
    pub requerimiento_calorico: Option<f64>,
    pub requerimiento_proteico: Option<f64>,
    pub albumina: Option<f64>,
    pub prealbumina: Option<f64>,
    pub diagnostico_nutricional: Option<String>,
    pub observaciones: Option<String>,
}

/// Qué pasó al dar de alta una valoración.
///
/// **`SinAntropometria` es una variante propia y no un `None` más** (directiva
/// 32): «este paciente no es tuyo» y «este peso no puede ser» se corrigen de
/// formas distintas, y colapsarlas en un solo negativo mandaría a pedir permisos
/// a quien lo que tiene es un cero en la báscula.
#[derive(Debug, Clone, PartialEq)]
pub enum Alta {
    Creada(i64),
    /// El paciente no existe o no es del ámbito. Las dos dan lo mismo.
    NoVisible,
    /// Sin peso o sin talla no hay valoración: el índice no se puede derivar y
    /// el esquema lo exige (`imc REAL NOT NULL`).
    SinAntropometria,
}

/// Lo mismo al editarla.
#[derive(Debug, Clone, PartialEq)]
pub enum Edicion {
    Hecha,
    NoExiste,
    SinAntropometria,
}

// --- Plan --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Plan {
    pub id: i64,
    pub valoracion_id: i64,
    pub paciente_id: i64,
    pub nutricionista_id: i64,
    pub fecha_creacion: String,
    pub tipo_dieta: String,
    pub calorias_totales: f64,
    pub distribucion_calorica: Json<Value>,
    pub menu_diario: Json<Value>,
    pub restricciones: Option<String>,
    pub suplementos: Option<String>,
    pub via_alimentacion: String,
    pub activo: bool,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanCrear {
    pub tipo_dieta: String,
    pub calorias_totales: f64,
    #[serde(default)]
    pub distribucion_calorica: Value,
    #[serde(default)]
    pub menu_diario: Value,
    pub restricciones: Option<String>,
    pub suplementos: Option<String>,
    pub via_alimentacion: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlanEditar {
    pub tipo_dieta: Option<String>,
    pub calorias_totales: Option<f64>,
    pub distribucion_calorica: Option<Value>,
    pub menu_diario: Option<Value>,
    pub restricciones: Option<String>,
    pub suplementos: Option<String>,
    pub via_alimentacion: Option<String>,
    pub observaciones: Option<String>,
    pub activo: Option<bool>,
}

use crate::filtro_ambito;

/// Valoraciones del ámbito, de la más reciente a la más antigua, opcionalmente
/// de un paciente y de un nivel de riesgo.
///
/// **El filtro por riesgo llegó el 2026-08-08 y cierra un defecto silencioso**:
/// `nutricion.service.ts` mandaba `?riesgo=ALTO` desde el selector de la
/// pantalla y este lado solo leía `paciente_id`, así que elegir «Alto» devolvía
/// la lista entera. No fallaba nada — y quien busca a los pacientes en riesgo
/// alto se lleva a todos, que es peor que un error.
///
/// El valor NO se valida aquí: un riesgo que no exista devuelve la lista vacía,
/// que es lo correcto para una consulta. Distinguir «no hay ninguno» de «ese
/// riesgo no existe» es trabajo de quien construye el filtro, y la pantalla lo
/// hace antes de llamar (directiva 20).
pub async fn listar_valoraciones(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: Option<i64>,
    riesgo: Option<&str>,
) -> Result<Vec<Valoracion>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "v.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT {VAL_COLS} FROM valoracion_nutricional v \
         JOIN paciente p ON p.id = v.paciente_id \
         JOIN usuario u ON u.id = v.nutricionista_id WHERE {filtro}"
    );
    if paciente_id.is_some() {
        sql.push_str(" AND v.paciente_id = ?");
    }
    if riesgo.is_some() {
        sql.push_str(" AND v.riesgo_nutricional = ?");
    }
    sql.push_str(" ORDER BY v.fecha_valoracion DESC");
    let mut q = sqlx::query_as::<_, Valoracion>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(p) = paciente_id {
        q = q.bind(p);
    }
    if let Some(r) = riesgo {
        q = q.bind(r.to_string());
    }
    q.fetch_all(pool).await
}

pub async fn obtener_valoracion(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Valoracion>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "v.institucion_id") else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {VAL_COLS} FROM valoracion_nutricional v \
         JOIN paciente p ON p.id = v.paciente_id \
         JOIN usuario u ON u.id = v.nutricionista_id WHERE v.id = ? AND {filtro}"
    );
    let mut q = sqlx::query_as::<_, Valoracion>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Crea una valoración. La institución se sella desde la del paciente (que debe
/// ser del ámbito); `None` si el paciente no es del ámbito.
pub async fn crear_valoracion(
    pool: &crate::Pool,
    ambito: Ambito,
    nutricionista_id: i64,
    v: &ValoracionCrear,
) -> Result<Alta, sqlx::Error> {
    let inst: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(v.paciente_id)
        .fetch_optional(pool)
        .await?;
    let Some((institucion,)) = inst.filter(|(i,)| ambito.puede_ver(Some(*i)) == Visibilidad::Si) else {
        return Ok(Alta::NoVisible);
    };
    // El índice se DERIVA aquí. Si no se puede, no hay valoración: se dice, en
    // vez de dejar que el `NOT NULL` de la columna lo convierta en un 500.
    let Some(indice) = medico_dominio::antropometria::imc(v.peso, v.talla) else {
        return Ok(Alta::SinAntropometria);
    };
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let r = sqlx::query(
        "INSERT INTO valoracion_nutricional \
         (paciente_id, hospitalizacion_id, nutricionista_id, fecha_valoracion, peso, talla, imc, \
          circunferencia_brazo, pliegue_tricipital, screening_nutricional, puntaje_screening, \
          riesgo_nutricional, requerimiento_calorico, requerimiento_proteico, albumina, prealbumina, \
          diagnostico_nutricional, observaciones, institucion_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(v.paciente_id)
    .bind(v.hospitalizacion_id)
    .bind(nutricionista_id)
    .bind(&fecha)
    .bind(v.peso)
    .bind(v.talla)
    .bind(indice)
    .bind(v.circunferencia_brazo)
    .bind(v.pliegue_tricipital)
    .bind(v.screening_nutricional.as_deref().unwrap_or("NRS-2002"))
    .bind(v.puntaje_screening)
    .bind(v.riesgo_nutricional.as_deref().unwrap_or("BAJO"))
    .bind(v.requerimiento_calorico)
    .bind(v.requerimiento_proteico)
    .bind(v.albumina)
    .bind(v.prealbumina)
    .bind(&v.diagnostico_nutricional)
    .bind(&v.observaciones)
    .bind(institucion)
    .execute(pool)
    .await?;
    Ok(Alta::Creada(r.last_insert_rowid()))
}

/// Actualiza los campos provistos (COALESCE deja intactos los ausentes).
///
/// **El IMC se recalcula con el peso y la talla que queden DESPUÉS de la
/// edición**, no con los que vengan en el cuerpo: cambiar solo el peso tiene que
/// mover el índice, y hasta el 2026-08-08 lo dejaba como estaba —o lo ponía a lo
/// que dijera el cliente— y la valoración quedaba contradiciéndose.
pub async fn actualizar_valoracion(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    v: &ValoracionEditar,
) -> Result<Edicion, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Edicion::NoExiste);
    };
    // Se lee lo que hay para saber sobre qué peso y qué talla se deriva. Sin
    // esto habría que fiarse de que el cliente mande los tres campos juntos, que
    // es justo la suposición que dejó entrar índices imposibles.
    let Some(actual) = obtener_valoracion(pool, ambito, id).await? else {
        return Ok(Edicion::NoExiste);
    };
    let peso = v.peso.unwrap_or(actual.peso);
    let talla = v.talla.unwrap_or(actual.talla);
    let Some(indice) = medico_dominio::antropometria::imc(peso, talla) else {
        return Ok(Edicion::SinAntropometria);
    };
    let sql = format!(
        "UPDATE valoracion_nutricional SET \
         peso=COALESCE(?,peso), talla=COALESCE(?,talla), imc=?, \
         circunferencia_brazo=COALESCE(?,circunferencia_brazo), pliegue_tricipital=COALESCE(?,pliegue_tricipital), \
         screening_nutricional=COALESCE(?,screening_nutricional), puntaje_screening=COALESCE(?,puntaje_screening), \
         riesgo_nutricional=COALESCE(?,riesgo_nutricional), requerimiento_calorico=COALESCE(?,requerimiento_calorico), \
         requerimiento_proteico=COALESCE(?,requerimiento_proteico), albumina=COALESCE(?,albumina), \
         prealbumina=COALESCE(?,prealbumina), diagnostico_nutricional=COALESCE(?,diagnostico_nutricional), \
         observaciones=COALESCE(?,observaciones) WHERE id = ? AND {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(v.peso).bind(v.talla).bind(indice)
        .bind(v.circunferencia_brazo).bind(v.pliegue_tricipital)
        .bind(&v.screening_nutricional).bind(v.puntaje_screening)
        .bind(&v.riesgo_nutricional).bind(v.requerimiento_calorico)
        .bind(v.requerimiento_proteico).bind(v.albumina)
        .bind(v.prealbumina).bind(&v.diagnostico_nutricional)
        .bind(&v.observaciones).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(if q.execute(pool).await?.rows_affected() > 0 {
        Edicion::Hecha
    } else {
        Edicion::NoExiste
    })
}

pub async fn eliminar_valoracion(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    // El plan cuelga con ON DELETE CASCADE.
    let sql = format!("DELETE FROM valoracion_nutricional WHERE id = ? AND {filtro}");
    let mut q = sqlx::query(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

const PLAN_COLS: &str = "id, valoracion_id, paciente_id, nutricionista_id, fecha_creacion, \
    tipo_dieta, calorias_totales, distribucion_calorica, menu_diario, restricciones, \
    suplementos, via_alimentacion, activo, observaciones";

/// El plan de una valoración del ámbito. `None` si la valoración es ajena o no
/// tiene plan.
pub async fn obtener_plan(
    pool: &crate::Pool,
    ambito: Ambito,
    valoracion_id: i64,
) -> Result<Option<Plan>, sqlx::Error> {
    // La valoración padre debe ser del ámbito.
    if obtener_valoracion(pool, ambito, valoracion_id).await?.is_none() {
        return Ok(None);
    }
    sqlx::query_as::<_, Plan>(&format!(
        "SELECT {PLAN_COLS} FROM plan_alimentacion WHERE valoracion_id = ? ORDER BY id DESC LIMIT 1"
    ))
    .bind(valoracion_id)
    .fetch_optional(pool)
    .await
}

/// Crea el plan de una valoración del ámbito (sella paciente e institución desde
/// la valoración). `None` si la valoración es ajena o no existe.
pub async fn crear_plan(
    pool: &crate::Pool,
    ambito: Ambito,
    valoracion_id: i64,
    nutricionista_id: i64,
    p: &PlanCrear,
) -> Result<Option<i64>, sqlx::Error> {
    let Some(val) = obtener_valoracion(pool, ambito, valoracion_id).await? else {
        return Ok(None);
    };
    let via = p.via_alimentacion.as_deref().unwrap_or("ORAL");
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    // institucion desde la del paciente de la valoración.
    let institucion: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(val.paciente_id)
        .fetch_optional(pool)
        .await?;
    let r = sqlx::query(
        "INSERT INTO plan_alimentacion \
         (valoracion_id, paciente_id, nutricionista_id, fecha_creacion, tipo_dieta, calorias_totales, \
          distribucion_calorica, menu_diario, restricciones, suplementos, via_alimentacion, activo, \
          observaciones, institucion_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,1,?,?)",
    )
    .bind(valoracion_id)
    .bind(val.paciente_id)
    .bind(nutricionista_id)
    .bind(&fecha)
    .bind(&p.tipo_dieta)
    .bind(p.calorias_totales)
    .bind(Json(&p.distribucion_calorica))
    .bind(Json(&p.menu_diario))
    .bind(&p.restricciones)
    .bind(&p.suplementos)
    .bind(via)
    .bind(&p.observaciones)
    .bind(institucion.map(|(i,)| i))
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

pub async fn actualizar_plan(
    pool: &crate::Pool,
    ambito: Ambito,
    plan_id: i64,
    p: &PlanEditar,
) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    let sql = format!(
        "UPDATE plan_alimentacion SET \
         tipo_dieta=COALESCE(?,tipo_dieta), calorias_totales=COALESCE(?,calorias_totales), \
         distribucion_calorica=COALESCE(?,distribucion_calorica), menu_diario=COALESCE(?,menu_diario), \
         restricciones=COALESCE(?,restricciones), suplementos=COALESCE(?,suplementos), \
         via_alimentacion=COALESCE(?,via_alimentacion), observaciones=COALESCE(?,observaciones), \
         activo=COALESCE(?,activo) WHERE id = ? AND {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(&p.tipo_dieta)
        .bind(p.calorias_totales)
        .bind(p.distribucion_calorica.as_ref().map(Json))
        .bind(p.menu_diario.as_ref().map(Json))
        .bind(&p.restricciones)
        .bind(&p.suplementos)
        .bind(&p.via_alimentacion)
        .bind(&p.observaciones)
        .bind(p.activo)
        .bind(plan_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let url = format!(
            "sqlite:file:nutri_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Nutri','n@x.co','x','nutricionista',1,1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)").execute(&pool).await.unwrap();
        pool
    }

    fn valoracion(paciente_id: i64) -> ValoracionCrear {
        ValoracionCrear {
            paciente_id,
            hospitalizacion_id: None,
            peso: 70.0, talla: 1.70, imc: 24.2,
            circunferencia_brazo: None, pliegue_tricipital: None,
            screening_nutricional: None, puntaje_screening: None, riesgo_nutricional: None,
            requerimiento_calorico: None, requerimiento_proteico: None,
            albumina: None, prealbumina: None,
            diagnostico_nutricional: "Adecuado".into(), observaciones: None,
        }
    }

    /// Crea una valoración del paciente indicado y devuelve su id.
    async fn creada(pool: &crate::Pool, paciente: i64) -> i64 {
        match crear_valoracion(pool, Ambito::institucion(1), 5, &valoracion(paciente)).await.unwrap() {
            Alta::Creada(id) => id,
            otro => panic!("no se creó: {otro:?}"),
        }
    }

    /// **El IMC lo deriva el servidor, y el del cliente se IGNORA.**
    ///
    /// Hasta el 2026-08-08 se guardaba tal cual, así que
    /// `{"peso":70,"talla":1.70,"imc":18}` entraba en la historia clínica
    /// contradiciéndose a sí mismo. El caso rojo es exactamente ése.
    #[tokio::test]
    async fn el_imc_se_deriva_y_el_que_manda_el_cliente_se_ignora() {
        let pool = base().await;
        let mut v = valoracion(10);
        v.peso = 70.0;
        v.talla = 1.70;
        v.imc = 18.0; // mentira
        let Alta::Creada(id) = crear_valoracion(&pool, Ambito::institucion(1), 5, &v).await.unwrap()
        else { panic!() };
        let guardada = obtener_valoracion(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(
            guardada.imc, 24.2,
            "se guardó el índice que mandó el cliente en vez del que sale de su peso y su talla"
        );

        // Y editar el índice a mano tampoco cuela.
        let edit = ValoracionEditar { imc: Some(18.0), ..Default::default() };
        assert_eq!(
            actualizar_valoracion(&pool, Ambito::institucion(1), id, &edit).await.unwrap(),
            Edicion::Hecha
        );
        let guardada = obtener_valoracion(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(guardada.imc, 24.2, "el cliente reescribió un valor derivado");
    }

    /// **Sin peso o sin talla no hay valoración**, y se dice cuál es el
    /// problema: colapsarlo con «el paciente no es tuyo» mandaría a pedir
    /// permisos a quien lo que tiene es un cero en la báscula.
    #[tokio::test]
    async fn sin_peso_o_sin_talla_no_se_crea_ni_se_edita() {
        let pool = base().await;
        let mut v = valoracion(10);
        v.peso = 0.0;
        assert_eq!(
            crear_valoracion(&pool, Ambito::institucion(1), 5, &v).await.unwrap(),
            Alta::SinAntropometria
        );
        // La pareja: con peso, entra. Sin esto, un alta que rechazara todo
        // pasaría la línea de arriba.
        let id = creada(&pool, 10).await;
        let edit = ValoracionEditar { talla: Some(0.0), ..Default::default() };
        assert_eq!(
            actualizar_valoracion(&pool, Ambito::institucion(1), id, &edit).await.unwrap(),
            Edicion::SinAntropometria
        );
        // Y no se escribió nada: la talla sigue siendo la de antes.
        let g = obtener_valoracion(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(g.talla, 1.70);
    }

    #[tokio::test]
    async fn crear_listar_y_obtener_respetan_el_ambito() {
        let pool = base().await;
        let id = creada(&pool, 10).await;
        // Paciente ajeno: no se crea.
        assert_eq!(
            crear_valoracion(&pool, Ambito::institucion(1), 5, &valoracion(20)).await.unwrap(),
            Alta::NoVisible
        );
        // La ve su institución, enriquecida; no la otra.
        let v = obtener_valoracion(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(v.paciente_nombre, "Ana Ruiz");
        assert_eq!(v.screening_nutricional, "NRS-2002"); // defecto
        assert!(obtener_valoracion(&pool, Ambito::institucion(2), id).await.unwrap().is_none());
        assert_eq!(listar_valoraciones(&pool, Ambito::institucion(1), None, None).await.unwrap().len(), 1);
        assert!(listar_valoraciones(&pool, Ambito::institucion(2), None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn actualizar_es_parcial_y_scoped() {
        let pool = base().await;
        let id = creada(&pool, 10).await;
        let edit = ValoracionEditar { peso: Some(72.0), ..Default::default() };
        // Ajeno no toca.
        assert_eq!(
            actualizar_valoracion(&pool, Ambito::institucion(2), id, &edit).await.unwrap(),
            Edicion::NoExiste
        );
        assert_eq!(
            actualizar_valoracion(&pool, Ambito::institucion(1), id, &edit).await.unwrap(),
            Edicion::Hecha
        );
        let v = obtener_valoracion(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(v.peso, 72.0);
        assert_eq!(v.talla, 1.70); // lo no provisto quedó intacto
        // **Y el índice se movió con el peso.** 72 / 1,70² = 24,9.
        assert_eq!(v.imc, 24.9, "cambió el peso y el IMC se quedó en el de antes");
    }

    #[tokio::test]
    async fn el_plan_cuelga_de_la_valoracion_y_guarda_json() {
        let pool = base().await;
        let vid = creada(&pool, 10).await;
        let plan = PlanCrear {
            tipo_dieta: "Hiposódica".into(),
            calorias_totales: 1800.0,
            distribucion_calorica: json!({"cho": 50, "prot": 20, "grasa": 30}),
            menu_diario: json!({"desayuno": "avena"}),
            restricciones: Some("sin sal".into()),
            suplementos: None,
            via_alimentacion: None,
            observaciones: None,
        };
        // Sobre valoración ajena, no se crea.
        assert!(crear_plan(&pool, Ambito::institucion(2), vid, 5, &plan).await.unwrap().is_none());
        assert!(crear_plan(&pool, Ambito::institucion(1), vid, 5, &plan).await.unwrap().is_some());
        let p = obtener_plan(&pool, Ambito::institucion(1), vid).await.unwrap().unwrap();
        assert_eq!(p.via_alimentacion, "ORAL"); // defecto
        assert_eq!(p.distribucion_calorica.0["cho"], 50);
        assert!(p.activo);
        // La otra institución no ve el plan.
        assert!(obtener_plan(&pool, Ambito::institucion(2), vid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn eliminar_valoracion_se_lleva_su_plan() {
        let pool = base().await;
        let vid = creada(&pool, 10).await;
        crear_plan(&pool, Ambito::institucion(1), vid, 5, &PlanCrear {
            tipo_dieta: "General".into(), calorias_totales: 2000.0,
            distribucion_calorica: json!({}), menu_diario: json!({}),
            restricciones: None, suplementos: None, via_alimentacion: None, observaciones: None,
        }).await.unwrap().unwrap();
        // Ajeno no borra.
        assert!(!eliminar_valoracion(&pool, Ambito::institucion(2), vid).await.unwrap());
        assert!(eliminar_valoracion(&pool, Ambito::institucion(1), vid).await.unwrap());
        assert!(obtener_valoracion(&pool, Ambito::institucion(1), vid).await.unwrap().is_none());
        // El plan se fue con ella (cascade): consultarlo por la valoración ya no da nada.
        assert!(obtener_plan(&pool, Ambito::Superadmin, vid).await.unwrap().is_none());
    }
}
