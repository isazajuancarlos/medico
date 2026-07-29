//! Usuarios: el acceso, y el origen del `Ambito` con el que todo lo demás filtra.
//!
//! A diferencia de `Paciente`, `Usuario` **no** deriva `Serialize`: lleva el
//! `password_hash`, y un hash que se serializa por descuido termina en un JSON,
//! un log o una respuesta. Lo que el resto del sistema necesita de un usuario
//! —quién es y qué alcance tiene— se expone por métodos, no volcando la fila.

use medico_dominio::{Ambito, RolUsuario};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Usuario {
    pub id: i64,
    pub nombre: String,
    pub email: String,
    /// Argon2. Solo lo lee la capa de autenticación, para verificar; no viaja.
    pub password_hash: String,
    /// El rol tal como está en la base (snake_case). Se interpreta con
    /// [`Usuario::rol`], no comparando cadenas por ahí.
    pub rol: String,
    /// `None` solo para el superadmin. `Usuario::ambito` es quien traduce ese
    /// `None`, en un solo sitio, para que «sin institución» no se confunda con
    /// «superadmin».
    pub institucion_id: Option<i64>,
    pub activo: bool,
}

const COLUMNAS: &str =
    "id, nombre, email, password_hash, rol, institucion_id, activo";

impl Usuario {
    /// El rol tipado, o `None` si la cadena de la base no nombra un rol conocido.
    pub fn rol(&self) -> Option<RolUsuario> {
        RolUsuario::desde_str(&self.rol)
    }

    /// El ámbito de visibilidad de este usuario. **Falla cerrado**: un rol que
    /// no se reconoce no ve nada (`Ninguno`), nunca lo ve todo. Un rol corrupto
    /// es un problema de datos que se manifiesta como «este usuario no ve nada»
    /// —visible y acotado—, no como una fuga.
    pub fn ambito(&self) -> Ambito {
        match self.rol() {
            Some(rol) => rol.ambito(self.institucion_id),
            None => Ambito::Ninguno,
        }
    }
}

/// Busca un usuario por su id. Lo usa cada petición autenticada para releer el
/// usuario del token: rol e institución se leen de la base en cada petición, no
/// se llevan en el token, porque un rol dentro del token sobrevive a que un
/// administrador lo revoque.
pub async fn por_id(pool: &crate::Pool, id: i64) -> Result<Option<Usuario>, sqlx::Error> {
    sqlx::query_as::<_, Usuario>(&format!("SELECT {COLUMNAS} FROM usuario WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Busca un usuario por su correo (el camino del login). No lleva `Ambito`: es
/// anterior a la autenticación —todavía no se sabe quién es—. Devuelve la fila
/// esté activa o no; rechazar al inactivo y verificar la contraseña es trabajo
/// de la capa de autenticación, no de esta. El correo es único, así que hay como
/// mucho una.
pub async fn por_email(pool: &crate::Pool, email: &str) -> Result<Option<Usuario>, sqlx::Error> {
    sqlx::query_as::<_, Usuario>(&format!("SELECT {COLUMNAS} FROM usuario WHERE email = ?"))
        .bind(email)
        .fetch_optional(pool)
        .await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Base con una institución y tres usuarios: superadmin (sin institución),
    /// un médico de la 1, y uno con un rol que no existe.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
             VALUES (1,'Root','root@x.co','h-root','superadmin',NULL,1),
                    (2,'Dra Ruiz','ruiz@x.co','h-ruiz','medico',1,1),
                    (3,'Raro','raro@x.co','h-raro','dios',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn por_email_trae_al_usuario_con_su_hash() {
        let pool = base().await;
        let u = por_email(&pool, "ruiz@x.co").await.unwrap().unwrap();
        assert_eq!(u.id, 2);
        assert_eq!(u.password_hash, "h-ruiz", "el hash tiene que llegar para verificarlo");
    }

    #[tokio::test]
    async fn por_email_desconocido_es_none() {
        let pool = base().await;
        assert!(por_email(&pool, "nadie@x.co").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn el_superadmin_sin_institucion_ve_todo() {
        let pool = base().await;
        let u = por_email(&pool, "root@x.co").await.unwrap().unwrap();
        assert_eq!(u.ambito(), Ambito::Superadmin);
    }

    #[tokio::test]
    async fn el_medico_queda_acotado_a_su_institucion() {
        let pool = base().await;
        let u = por_email(&pool, "ruiz@x.co").await.unwrap().unwrap();
        assert_eq!(u.ambito(), Ambito::Institucion(1));
    }

    /// El rol corrupto falla CERRADO: no ve nada, aunque tenga institución.
    #[tokio::test]
    async fn un_rol_desconocido_no_ve_nada() {
        let pool = base().await;
        let u = por_email(&pool, "raro@x.co").await.unwrap().unwrap();
        assert_eq!(u.rol(), None);
        assert_eq!(u.ambito(), Ambito::Ninguno, "un rol corrupto no puede conceder acceso");
    }
}
