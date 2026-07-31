//! Instituciones (IPS). Las gestiona el **superadmin**: dar de alta una IPS es
//! registrar un cliente del sistema, no una acción de dentro de una institución.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Institucion {
    pub id: i64,
    pub nombre: String,
    pub nit: String,
    pub activa: bool,
}

const COLUMNAS: &str = "id, nombre, nit, activa";

/// Lista según el ámbito: el superadmin ve todas; un usuario de una institución,
/// solo la suya; sin ámbito, ninguna.
pub async fn listar(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Institucion>, sqlx::Error> {
    match ambito {
        Ambito::Superadmin => {
            sqlx::query_as::<_, Institucion>(&format!("SELECT {COLUMNAS} FROM institucion ORDER BY nombre"))
                .fetch_all(pool)
                .await
        }
        Ambito::Institucion(id) => {
            sqlx::query_as::<_, Institucion>(&format!("SELECT {COLUMNAS} FROM institucion WHERE id = ?"))
                .bind(id)
                .fetch_all(pool)
                .await
        }
        Ambito::Ninguno => Ok(Vec::new()),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstitucionNueva {
    pub nombre: String,
    pub nit: String,
}

/// Los tres desenlaces reales de crear una institución. El NIT es único en todo
/// el sistema.
pub enum ResultadoCrear {
    Creada(i64),
    SinPermiso,
    NitEnUso,
}

/// Crea una institución. **Solo el superadmin.**
pub async fn crear(
    pool: &crate::Pool,
    quien: Ambito,
    nueva: &InstitucionNueva,
) -> Result<ResultadoCrear, sqlx::Error> {
    if quien != Ambito::Superadmin {
        return Ok(ResultadoCrear::SinPermiso);
    }
    let ya: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM institucion WHERE nit = ?")
        .bind(&nueva.nit)
        .fetch_one(pool)
        .await?;
    if ya > 0 {
        return Ok(ResultadoCrear::NitEnUso);
    }
    let r = sqlx::query("INSERT INTO institucion (nombre, nit) VALUES (?, ?)")
        .bind(&nueva.nombre)
        .bind(&nueva.nit)
        .execute(pool)
        .await?;
    Ok(ResultadoCrear::Creada(r.last_insert_rowid()))
}

/// Los módulos licenciados a una institución (#65).
///
/// Devuelve la lista tal como está guardada. Un JSON ilegible —tocado a mano, o
/// de una versión posterior— **NO cae al defecto**: eso le devolvería en
/// silencio a la institución módulos que quizá no compró, o le quitaría los que
/// sí. Se propaga el error y la petición falla, que es la única respuesta que
/// no inventa una licencia (directiva 20).
pub async fn modulos_habilitados(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let fila: Option<(String,)> =
        sqlx::query_as("SELECT modulos_habilitados FROM institucion WHERE id = ?")
            .bind(institucion_id)
            .fetch_optional(pool)
            .await?;
    let Some((crudo,)) = fila else { return Ok(None) };
    serde_json::from_str::<Vec<String>>(&crudo)
        .map(Some)
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

/// Fija los módulos licenciados. El llamante ya validó que todos existen en el
/// catálogo: aquí no se filtra nada en silencio.
pub async fn fijar_modulos(
    pool: &crate::Pool,
    institucion_id: i64,
    modulos: &[String],
) -> Result<bool, sqlx::Error> {
    // Ordenados y sin repetidos: la licencia es un CONJUNTO, y guardarla dos
    // veces igual con distinto orden haría que dos instituciones idénticas se
    // vieran distintas al comparar.
    let mut v: Vec<&str> = modulos.iter().map(|s| s.as_str()).collect();
    v.sort_unstable();
    v.dedup();
    let json = serde_json::to_string(&v).expect("una lista de cadenas siempre serializa");
    let r = sqlx::query("UPDATE institucion SET modulos_habilitados = ? WHERE id = ?")
        .bind(&json)
        .bind(institucion_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn el_superadmin_ve_todas_y_un_usuario_solo_la_suya() {
        let pool = base().await;
        assert_eq!(listar(&pool, Ambito::Superadmin).await.unwrap().len(), 2);
        let suyas = listar(&pool, Ambito::institucion(1)).await.unwrap();
        assert_eq!(suyas.len(), 1);
        assert_eq!(suyas[0].id, 1);
        assert!(listar(&pool, Ambito::Ninguno).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn solo_el_superadmin_crea() {
        let pool = base().await;
        let nueva = InstitucionNueva { nombre: "Tres".into(), nit: "902".into() };
        assert!(matches!(crear(&pool, Ambito::Superadmin, &nueva).await.unwrap(), ResultadoCrear::Creada(_)));
        assert!(matches!(crear(&pool, Ambito::institucion(1), &nueva).await.unwrap(), ResultadoCrear::SinPermiso));
        assert!(matches!(crear(&pool, Ambito::Ninguno, &nueva).await.unwrap(), ResultadoCrear::SinPermiso));
    }

    #[tokio::test]
    async fn un_nit_repetido_no_pasa() {
        let pool = base().await;
        let repetida = InstitucionNueva { nombre: "Otra".into(), nit: "900".into() };
        assert!(matches!(crear(&pool, Ambito::Superadmin, &repetida).await.unwrap(), ResultadoCrear::NitEnUso));
    }
}
