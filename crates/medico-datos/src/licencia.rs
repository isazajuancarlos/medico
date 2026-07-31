//! La licencia comercial de una institución: cupo de usuarios y vigencia.
//!
//! Regla que no se rompe: **la licencia limita la EXPANSIÓN, nunca la lectura.**
//! Nada de aquí se llama en el login ni en los listados. Una licencia vencida
//! impide crear usuarios, pero jamás ciega la historia clínica: dejar a un
//! hospital sin sus registros por un asunto comercial lo pone a incumplir ante
//! el paciente y la Supersalud.

/// Estado de la licencia de una institución en una fecha dada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    SinLicencia,
    Vencida,
    Vigente { max_usuarios: i64 },
}

/// El estado de la licencia de `institucion_id` frente a `hoy` (YYYY-MM-DD). La
/// comparación es de cadena, que en ese formato es cronológica.
pub async fn estado(pool: &crate::Pool, institucion_id: i64, hoy: &str) -> Result<Estado, sqlx::Error> {
    let fila: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT max_usuarios, vence FROM licencia WHERE institucion_id = ?")
            .bind(institucion_id)
            .fetch_optional(pool)
            .await?;
    Ok(match fila {
        None => Estado::SinLicencia,
        // `vence` es el último día válido: vencida solo si venció ANTES de hoy.
        Some((_, Some(vence))) if vence.as_str() < hoy => Estado::Vencida,
        Some((max, _)) => Estado::Vigente { max_usuarios: max },
    })
}

/// Fija (crea o reemplaza) la licencia de una institución. La usa el superadmin
/// al dar de alta o renovar un plan.
pub async fn fijar(
    pool: &crate::Pool,
    institucion_id: i64,
    max_usuarios: i64,
    vence: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO licencia (institucion_id, max_usuarios, vence) VALUES (?, ?, ?) \
         ON CONFLICT(institucion_id) DO UPDATE SET max_usuarios = excluded.max_usuarios, \
         vence = excluded.vence",
    )
    .bind(institucion_id)
    .bind(max_usuarios)
    .bind(vence)
    .execute(pool)
    .await?;
    Ok(())
}

/// Si se puede o no agregar un usuario a una institución, y por qué no. El cupo
/// se cuenta contra los usuarios ACTIVOS reales (un inactivo no consume cupo),
/// no contra un campo guardado.
#[derive(Debug, PartialEq, Eq)]
pub enum Puerta {
    Abierta,
    SinLicencia,
    Vencida,
    CupoLleno,
}

pub async fn puerta_para_usuario(
    pool: &crate::Pool,
    institucion_id: i64,
    hoy: &str,
) -> Result<Puerta, sqlx::Error> {
    match estado(pool, institucion_id, hoy).await? {
        Estado::SinLicencia => Ok(Puerta::SinLicencia),
        Estado::Vencida => Ok(Puerta::Vencida),
        Estado::Vigente { max_usuarios } => {
            let activos: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM usuario WHERE institucion_id = ? AND activo = 1")
                    .bind(institucion_id)
                    .fetch_one(pool)
                    .await?;
            Ok(if activos >= max_usuarios {
                Puerta::CupoLleno
            } else {
                Puerta::Abierta
            })
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const HOY: &str = "2026-07-28";
    const AYER: &str = "2026-07-27";
    const MANANA: &str = "2026-07-29";

    /// Institución 1 con dos usuarios ACTIVOS y uno inactivo (que no cuenta cupo).
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
             VALUES (1,'A','a@x.co','h','medico',1,1),
                    (2,'B','b@x.co','h','medico',1,1),
                    (3,'C','c@x.co','h','medico',1,0)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn sin_fila_es_sin_licencia() {
        let pool = base().await;
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), Estado::SinLicencia);
    }

    #[tokio::test]
    async fn vence_ayer_esta_vencida_y_vence_hoy_sigue_vigente() {
        let pool = base().await;
        fijar(&pool, 1, 10, Some(AYER)).await.unwrap();
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), Estado::Vencida);
        // El último día es válido: vence hoy, hoy sigue vigente.
        fijar(&pool, 1, 10, Some(HOY)).await.unwrap();
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), Estado::Vigente { max_usuarios: 10 });
        // Perpetua (NULL) siempre vigente.
        fijar(&pool, 1, 10, None).await.unwrap();
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), Estado::Vigente { max_usuarios: 10 });
    }

    #[tokio::test]
    async fn la_puerta_distingue_los_cuatro_casos() {
        let pool = base().await;
        // Sin licencia.
        assert_eq!(puerta_para_usuario(&pool, 1, HOY).await.unwrap(), Puerta::SinLicencia);
        // Vencida.
        fijar(&pool, 1, 10, Some(AYER)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY).await.unwrap(), Puerta::Vencida);
        // Vigente con cupo (2 activos < 10).
        fijar(&pool, 1, 10, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY).await.unwrap(), Puerta::Abierta);
        // Cupo lleno: max 2 y hay 2 activos (el inactivo no cuenta).
        fijar(&pool, 1, 2, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY).await.unwrap(), Puerta::CupoLleno);
    }

    /// El inactivo no consume cupo: con max 3 y 2 activos + 1 inactivo, abre.
    #[tokio::test]
    async fn el_inactivo_no_consume_cupo() {
        let pool = base().await;
        fijar(&pool, 1, 3, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY).await.unwrap(), Puerta::Abierta);
    }
}
