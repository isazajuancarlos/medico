//! Autenticación: quién entra (login) y quién es en cada petición (extractor).
//!
//! La lógica vive en funciones que se prueban sin servidor (`autenticar`,
//! `usuario_de`); el extractor `Autenticado` es una capa fina de axum encima.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use medico_datos::usuario::{self, Usuario};

use crate::{claves, sesion};

/// Lo que comparten todas las rutas: la base y el secreto de sesión.
#[derive(Clone)]
pub struct Estado {
    pub pool: medico_datos::Pool,
    pub secreto: Vec<u8>,
    pub minutos: i64,
}

/// El login. Devuelve el valor de la cookie de sesión, o `None`. **Mismo
/// resultado para «no existe», «inactivo» y «clave incorrecta»**: distinguirlos
/// le diría a quien prueba qué correos existen.
pub async fn autenticar(
    pool: &medico_datos::Pool,
    secreto: &[u8],
    minutos: i64,
    email: &str,
    clave: &str,
) -> Option<String> {
    let usuario = usuario::por_email(pool, email).await.ok().flatten()?;
    if !usuario.activo {
        return None;
    }
    if !claves::verificar(clave, &usuario.password_hash) {
        return None;
    }
    Some(sesion::emitir(secreto, usuario.id, minutos))
}

/// El usuario de una petición, a partir de sus cabeceras. Relee el usuario de la
/// base (no confía en el token para el rol) y rechaza al inactivo. `None` si no
/// hay cookie válida.
pub async fn usuario_de(estado: &Estado, cabeceras: &HeaderMap) -> Option<Usuario> {
    let cookie = sesion::de_cabeceras(cabeceras)?;
    let sesion = sesion::verificar(&estado.secreto, &cookie)?;
    let usuario = usuario::por_id(&estado.pool, sesion.usuario_id).await.ok().flatten()?;
    usuario.activo.then_some(usuario)
}

/// Extractor: una ruta que pide `Autenticado` solo se ejecuta con sesión válida;
/// si no, axum responde 401 sin entrar al handler.
pub struct Autenticado(pub Usuario);

impl FromRequestParts<Estado> for Autenticado {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, estado: &Estado) -> Result<Self, StatusCode> {
        match usuario_de(estado, &parts.headers).await {
            Some(u) => Ok(Autenticado(u)),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use axum::http::header;

    const SECRETO: &[u8] = b"un secreto de prueba de mas de treinta y dos bytes";

    async fn base() -> medico_datos::Pool {
        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool).await.unwrap();
        let h = claves::hashear("clave123").unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
             VALUES (2,'Dra Ruiz','ruiz@x.co',?,'medico',1,1),
                    (3,'Retirado','ex@x.co',?,'medico',1,0)",
        )
        .bind(&h).bind(&h)
        .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn autenticar_con_la_clave_correcta_da_cookie() {
        let pool = base().await;
        let cookie = autenticar(&pool, SECRETO, 60, "ruiz@x.co", "clave123").await;
        assert!(cookie.is_some());
        // La cookie es una sesión válida del usuario 2.
        let s = sesion::verificar(SECRETO, &cookie.unwrap()).unwrap();
        assert_eq!(s.usuario_id, 2);
    }

    #[tokio::test]
    async fn la_clave_incorrecta_no_entra() {
        let pool = base().await;
        assert!(autenticar(&pool, SECRETO, 60, "ruiz@x.co", "otra").await.is_none());
    }

    #[tokio::test]
    async fn el_correo_que_no_existe_da_lo_mismo() {
        let pool = base().await;
        assert!(autenticar(&pool, SECRETO, 60, "nadie@x.co", "clave123").await.is_none());
    }

    #[tokio::test]
    async fn el_inactivo_no_entra_aunque_la_clave_sea_correcta() {
        let pool = base().await;
        assert!(autenticar(&pool, SECRETO, 60, "ex@x.co", "clave123").await.is_none());
    }

    #[tokio::test]
    async fn usuario_de_una_cookie_valida_es_el_usuario() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60 };
        let cookie = sesion::emitir(SECRETO, 2, 60);
        let mut cabeceras = HeaderMap::new();
        cabeceras.insert(header::COOKIE, format!("medico_sesion={cookie}").parse().unwrap());
        let u = usuario_de(&estado, &cabeceras).await.unwrap();
        assert_eq!(u.id, 2);
    }

    #[tokio::test]
    async fn sin_cookie_no_hay_usuario() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60 };
        assert!(usuario_de(&estado, &HeaderMap::new()).await.is_none());
    }

    /// Una cookie firmada con OTRO secreto no vale — el token no basta, hay que
    /// haberlo firmado con el secreto del servidor.
    #[tokio::test]
    async fn una_cookie_de_otro_secreto_no_entra() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60 };
        let ajena = sesion::emitir(b"otro secreto largo de mas de 32 bytes!", 2, 60);
        let mut cabeceras = HeaderMap::new();
        cabeceras.insert(header::COOKIE, format!("medico_sesion={ajena}").parse().unwrap());
        assert!(usuario_de(&estado, &cabeceras).await.is_none());
    }
}
