//! Fórmulas (prescripciones) de una historia. Aislamiento por la cadena
//! historia → paciente → institución: quien no ve la historia no toca sus fórmulas.

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Formula {
    pub id: i64,
    pub historia_id: i64,
    pub medicamento: String,
    pub codigo_cum: Option<String>,
    pub dosis: String,
    pub cantidad: f64,
    pub unidad: Option<String>,
    pub dias_tratamiento: Option<i64>,
    pub observaciones: Option<String>,
    pub fecha_formula: String,
    pub dispensada: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormulaNueva {
    pub medicamento: String,
    pub codigo_cum: Option<String>,
    pub dosis: String,
    pub cantidad: f64,
    pub unidad: Option<String>,
    pub dias_tratamiento: Option<i64>,
    pub observaciones: Option<String>,
}

const COLUMNAS: &str = "id, historia_id, medicamento, codigo_cum, dosis, cantidad, unidad, \
                        dias_tratamiento, observaciones, fecha_formula, dispensada";

/// ¿Es visible para el ámbito la historia `historia_id`? (por su paciente)
async fn historia_visible(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM historia h JOIN paciente p ON p.id = h.paciente_id WHERE h.id = ?",
    )
    .bind(historia_id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

/// ¿Es visible para el ámbito la fórmula `id`? (por su historia → paciente)
async fn formula_visible(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM formula f JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id WHERE f.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

pub async fn listar_por_historia(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<Vec<Formula>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Formula>(&format!("SELECT {COLUMNAS} FROM formula WHERE historia_id = ? ORDER BY id"))
        .bind(historia_id)
        .fetch_all(pool)
        .await
}

/// Crea una fórmula en una historia visible. `None` si la historia no es del ámbito.
pub async fn crear(pool: &crate::Pool, ambito: Ambito, historia_id: i64, f: &FormulaNueva) -> Result<Option<i64>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO formula (historia_id, medicamento, codigo_cum, dosis, cantidad, unidad, \
         dias_tratamiento, observaciones) VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(historia_id)
    .bind(&f.medicamento)
    .bind(&f.codigo_cum)
    .bind(&f.dosis)
    .bind(f.cantidad)
    .bind(&f.unidad)
    .bind(f.dias_tratamiento)
    .bind(&f.observaciones)
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Actualiza una fórmula visible. `false` si no existe o no es del ámbito.
pub async fn actualizar(pool: &crate::Pool, ambito: Ambito, id: i64, f: &FormulaNueva) -> Result<bool, sqlx::Error> {
    if !formula_visible(pool, ambito, id).await? {
        return Ok(false);
    }
    let r = sqlx::query(
        "UPDATE formula SET medicamento=?, codigo_cum=?, dosis=?, cantidad=?, unidad=?, \
         dias_tratamiento=?, observaciones=? WHERE id=?",
    )
    .bind(&f.medicamento)
    .bind(&f.codigo_cum)
    .bind(&f.dosis)
    .bind(f.cantidad)
    .bind(&f.unidad)
    .bind(f.dias_tratamiento)
    .bind(&f.observaciones)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

/// Qué pasó al borrar una fórmula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoBorrado {
    Hecho,
    /// No existe o no es del ámbito. Los dos casos dan lo mismo a propósito.
    NoExiste,
    /// **Ya se entregó al paciente.** Borrar la prescripción dejaría en el
    /// sistema una entrega de medicamento sin nada que la respalde: la farmacia
    /// habría dispensado «sin fórmula». La constancia de la entrega y su
    /// prescripción se sostienen o caen juntas, y ninguna de las dos se corrige
    /// borrando — se corrige con la fórmula nueva que el médico firme.
    YaDispensada { entregas: i64 },
}

/// Elimina una fórmula visible que no se haya dispensado. Ver [`ResultadoBorrado`].
pub async fn eliminar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<ResultadoBorrado, sqlx::Error> {
    if !formula_visible(pool, ambito, id).await? {
        return Ok(ResultadoBorrado::NoExiste);
    }
    let entregas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dispensacion WHERE formula_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if entregas > 0 {
        return Ok(ResultadoBorrado::YaDispensada { entregas });
    }
    let r = sqlx::query("DELETE FROM formula WHERE id = ?").bind(id).execute(pool).await?;
    Ok(if r.rows_affected() > 0 { ResultadoBorrado::Hecho } else { ResultadoBorrado::NoExiste })
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
        // Una historia en cada institución (id 100 de la 1, id 200 de la 2).
        sqlx::query("INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento) VALUES (100,'HC1',10,5,'m','e','J00','plan'),(200,'HC2',20,5,'m','e','J00','plan')")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nueva() -> FormulaNueva {
        FormulaNueva {
            medicamento: "Acetaminofén 500mg".into(),
            codigo_cum: Some("19900001-1".into()),
            dosis: "1 cada 8h".into(),
            cantidad: 30.0,
            unidad: Some("tableta".into()),
            dias_tratamiento: Some(10),
            observaciones: None,
        }
    }

    #[tokio::test]
    async fn crear_listar_en_historia_propia() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        let fs = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        assert_eq!(fs.len(), 1);
        assert_eq!(fs[0].id, id);
        assert_eq!(fs[0].cantidad, 30.0);
        assert!(!fs[0].dispensada);
    }

    /// No se crea ni se lista una fórmula sobre la historia de otra institución.
    #[tokio::test]
    async fn la_historia_ajena_no_deja_formular() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::institucion(1), 200, &nueva()).await.unwrap().is_none());
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 200).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn actualizar_y_eliminar_respetan_el_ambito() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        // La 2 no puede tocarla.
        assert!(!actualizar(&pool, Ambito::institucion(2), id, &nueva()).await.unwrap());
        assert_eq!(eliminar(&pool, Ambito::institucion(2), id).await.unwrap(), ResultadoBorrado::NoExiste);
        // La 1 sí.
        let mut cambio = nueva();
        cambio.dosis = "1 cada 12h".into();
        assert!(actualizar(&pool, Ambito::institucion(1), id, &cambio).await.unwrap());
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), ResultadoBorrado::Hecho);
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap().is_empty());
    }

    /// Una fórmula ya entregada no se borra: dejaría una dispensación sin
    /// prescripción que la respalde.
    #[tokio::test]
    async fn una_formula_ya_dispensada_no_se_borra() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO item_inventario (id, institucion_id, nombre, lote, cantidad_actual) \
             VALUES (7, 1, 'Acetaminofén 500mg', 'L-1', 100)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO dispensacion \
             (formula_id, item_inventario_id, paciente_id, regente_farmacia_id, \
              cantidad_dispensada, lote_dispensado) VALUES (?, 7, 10, 5, 30, 'L-1')",
        ).bind(id).execute(&pool).await.unwrap();

        assert_eq!(
            eliminar(&pool, Ambito::institucion(1), id).await.unwrap(),
            ResultadoBorrado::YaDispensada { entregas: 1 }
        );
        assert_eq!(listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap().len(), 1);
    }
}
