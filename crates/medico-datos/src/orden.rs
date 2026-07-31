//! Órdenes médicas de una historia. Aislamiento por la cadena
//! historia → paciente → institución.

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Orden {
    pub id: i64,
    pub historia_id: i64,
    pub fecha_orden: String,
    pub estado: String,
    pub tipo: String,
    pub codigo_cups: Option<String>,
    pub descripcion: String,
    pub justificacion: String,
    pub especialidad_destino_id: Option<i64>,
    pub institucion_destino_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrdenNueva {
    pub tipo: String,
    pub descripcion: String,
    pub justificacion: String,
    pub codigo_cups: Option<String>,
    pub especialidad_destino_id: Option<i64>,
    pub institucion_destino_id: Option<i64>,
}

const COLUMNAS: &str = "id, historia_id, fecha_orden, estado, tipo, codigo_cups, descripcion, \
                        justificacion, especialidad_destino_id, institucion_destino_id";

async fn historia_visible(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM historia h JOIN paciente p ON p.id = h.paciente_id WHERE h.id = ?",
    )
    .bind(historia_id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

async fn orden_visible(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM orden o JOIN historia h ON h.id = o.historia_id \
         JOIN paciente p ON p.id = h.paciente_id WHERE o.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

pub async fn listar_por_historia(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<Vec<Orden>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Orden>(&format!("SELECT {COLUMNAS} FROM orden WHERE historia_id = ? ORDER BY id"))
        .bind(historia_id)
        .fetch_all(pool)
        .await
}

pub async fn crear(pool: &crate::Pool, ambito: Ambito, historia_id: i64, o: &OrdenNueva) -> Result<Option<i64>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO orden (historia_id, tipo, codigo_cups, descripcion, justificacion, \
         especialidad_destino_id, institucion_destino_id) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(historia_id)
    .bind(&o.tipo)
    .bind(&o.codigo_cups)
    .bind(&o.descripcion)
    .bind(&o.justificacion)
    .bind(o.especialidad_destino_id)
    .bind(o.institucion_destino_id)
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Cambia el estado de una orden visible. `false` si no existe o no es del ámbito.
pub async fn cambiar_estado(pool: &crate::Pool, ambito: Ambito, id: i64, estado: &str) -> Result<bool, sqlx::Error> {
    if !orden_visible(pool, ambito, id).await? {
        return Ok(false);
    }
    let r = sqlx::query("UPDATE orden SET estado = ? WHERE id = ?")
        .bind(estado)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn eliminar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    if !orden_visible(pool, ambito, id).await? {
        return Ok(false);
    }
    let r = sqlx::query("DELETE FROM orden WHERE id = ?").bind(id).execute(pool).await?;
    Ok(r.rows_affected() > 0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dra','d@x.co','h','medico',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento) VALUES (100,'HC1',10,5,'m','e','J00','plan'),(200,'HC2',20,5,'m','e','J00','plan')")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nueva() -> OrdenNueva {
        OrdenNueva {
            tipo: "Laboratorio".into(),
            descripcion: "Hemograma".into(),
            justificacion: "Control".into(),
            codigo_cups: Some("902210".into()),
            especialidad_destino_id: None,
            institucion_destino_id: None,
        }
    }

    #[tokio::test]
    async fn crear_listar_en_historia_propia() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        let os = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        assert_eq!(os.len(), 1);
        assert_eq!(os[0].id, id);
        // El defecto lo fija la 0027, y es el que compara la interfaz.
        assert_eq!(os[0].estado, "Pendiente");
        assert_eq!(os[0].descripcion, "Hemograma");
    }

    #[tokio::test]
    async fn la_historia_ajena_no_deja_ordenar() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::institucion(1), 200, &nueva()).await.unwrap().is_none());
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 200).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cambiar_estado_y_eliminar_respetan_el_ambito() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        assert!(!cambiar_estado(&pool, Ambito::institucion(2), id, "Completada").await.unwrap());
        assert!(cambiar_estado(&pool, Ambito::institucion(1), id, "Completada").await.unwrap());
        assert!(!eliminar(&pool, Ambito::institucion(2), id).await.unwrap());
        assert!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap());
    }
}
