//! Usuarios: el acceso, y el origen del `Ambito` con el que todo lo demás filtra.
//!
//! A diferencia de `Paciente`, `Usuario` **no** deriva `Serialize`: lleva el
//! `password_hash`, y un hash que se serializa por descuido termina en un JSON,
//! un log o una respuesta. Lo que el resto del sistema necesita de un usuario
//! —quién es y qué alcance tiene— se expone por métodos, no volcando la fila.

use medico_dominio::{Ambito, RolUsuario};
use serde::Serialize;

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
    /// Versión de sesión (M3). El token la lleva; `auth` rechaza el token con una
    /// versión menor que esta. Subirla revoca todas las sesiones. Ver
    /// [`revocar_sesiones`].
    pub token_version: i64,
}

const COLUMNAS: &str =
    "id, nombre, email, password_hash, rol, institucion_id, activo, token_version";

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

/// Revoca TODAS las sesiones vivas de un usuario subiendo su `token_version` en
/// uno: los tokens ya emitidos llevan la versión anterior y `auth::usuario_de` los
/// rechaza en la siguiente petición. Es el "cerrar sesión en todos los
/// dispositivos" y el sitio donde enganchar "invalidar sesiones tras cambiar la
/// contraseña" cuando exista ese flujo. `false` si el usuario no existe (nada que
/// revocar) — no se inventa un éxito sobre un id que no está.
pub async fn revocar_sesiones(pool: &crate::Pool, id: i64) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("UPDATE usuario SET token_version = token_version + 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// Resumen de un usuario para listar. **No lleva `password_hash`** —ni siquiera
/// se selecciona de la base—, así que es seguro serializarlo, a diferencia de
/// [`Usuario`].
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ResumenUsuario {
    pub id: i64,
    pub nombre: String,
    pub email: String,
    pub rol: String,
    pub institucion_id: Option<i64>,
    pub activo: bool,
}

/// Lista usuarios según el ámbito. Sin el hash: la consulta no lo trae.
pub async fn listar(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<ResumenUsuario>, sqlx::Error> {
    const COLS: &str = "id, nombre, email, rol, institucion_id, activo";
    match ambito {
        Ambito::Superadmin => {
            sqlx::query_as::<_, ResumenUsuario>(&format!("SELECT {COLS} FROM usuario ORDER BY nombre"))
                .fetch_all(pool)
                .await
        }
        Ambito::Institucion(id) => {
            sqlx::query_as::<_, ResumenUsuario>(&format!(
                "SELECT {COLS} FROM usuario WHERE institucion_id = ? ORDER BY nombre"
            ))
            .bind(id)
            .fetch_all(pool)
            .await
        }
        Ambito::Ninguno => Ok(Vec::new()),
    }
}

/// Un médico para poblar selectores (agendar una cita, asignar). Sin datos
/// sensibles: id y nombre.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MedicoResumen {
    pub id: i64,
    pub nombre: String,
}

/// Los médicos ACTIVOS del ámbito. No lleva gate de rol: agendar una cita lo
/// hace cualquier usuario, y para eso necesita la lista de médicos de su IPS.
pub async fn medicos(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<MedicoResumen>, sqlx::Error> {
    match ambito {
        Ambito::Superadmin => {
            sqlx::query_as::<_, MedicoResumen>(
                "SELECT id, nombre FROM usuario WHERE rol = 'medico' AND activo = 1 ORDER BY nombre",
            )
            .fetch_all(pool)
            .await
        }
        Ambito::Institucion(id) => {
            sqlx::query_as::<_, MedicoResumen>(
                "SELECT id, nombre FROM usuario WHERE rol = 'medico' AND activo = 1 \
                 AND institucion_id = ? ORDER BY nombre",
            )
            .bind(id)
            .fetch_all(pool)
            .await
        }
        Ambito::Ninguno => Ok(Vec::new()),
    }
}

/// Datos para crear un usuario. La contraseña llega **ya hasheada**: esta capa
/// nunca ve la clave en claro (la hashea la capa de API).
pub struct UsuarioNuevo {
    pub nombre: String,
    pub email: String,
    pub password_hash: String,
    pub rol: String,
    pub institucion_id: Option<i64>,
}

/// Los tres desenlaces reales de crear un usuario.
pub enum ResultadoCrear {
    Creado(i64),
    SinPermiso,
    EmailEnUso,
}

/// Crea un usuario. La institución del nuevo la fija QUIÉN crea: un admin lo crea
/// en SU institución (el `institucion_id` que venga se ignora); el superadmin la
/// elige (para dar de alta el primer admin de una IPS nueva, o incluso otro
/// superadmin con `None`). El correo es único en todo el sistema.
pub async fn crear(
    pool: &crate::Pool,
    quien: Ambito,
    nuevo: &UsuarioNuevo,
) -> Result<ResultadoCrear, sqlx::Error> {
    let institucion_id = match quien {
        Ambito::Institucion(id) => Some(id),
        Ambito::Superadmin => nuevo.institucion_id,
        Ambito::Ninguno => return Ok(ResultadoCrear::SinPermiso),
    };
    if por_email(pool, &nuevo.email).await?.is_some() {
        return Ok(ResultadoCrear::EmailEnUso);
    }
    let r = sqlx::query(
        "INSERT INTO usuario (nombre, email, password_hash, rol, institucion_id, activo) \
         VALUES (?,?,?,?,?,1)",
    )
    .bind(&nuevo.nombre)
    .bind(&nuevo.email)
    .bind(&nuevo.password_hash)
    .bind(&nuevo.rol)
    .bind(institucion_id)
    .execute(pool)
    .await?;
    Ok(ResultadoCrear::Creado(r.last_insert_rowid()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Base con una institución y tres usuarios: superadmin (sin institución),
    /// un médico de la 1, y uno con un rol que no existe.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
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

    #[tokio::test]
    async fn listar_es_scoped_y_no_trae_el_hash() {
        let bd = base().await;
        // El superadmin ve los tres; la institución 1, sus tres (root es NULL,
        // no de la 1) → en realidad la 1 ve a Ruiz y Raro (2), no a root.
        assert_eq!(listar(&bd, Ambito::Superadmin).await.unwrap().len(), 3);
        assert_eq!(listar(&bd, Ambito::institucion(1)).await.unwrap().len(), 2);
        assert!(listar(&bd, Ambito::Ninguno).await.unwrap().is_empty());
        // El ResumenUsuario ni siquiera tiene campo de hash: es imposible filtrarlo.
    }

    #[tokio::test]
    async fn revocar_sesiones_sube_la_version_solo_del_usuario() {
        let pool = base().await;
        assert_eq!(por_id(&pool, 2).await.unwrap().unwrap().token_version, 0, "de fábrica es 0");
        assert!(revocar_sesiones(&pool, 2).await.unwrap(), "el usuario existe");
        assert_eq!(por_id(&pool, 2).await.unwrap().unwrap().token_version, 1);
        // No tocó a otro usuario.
        assert_eq!(por_id(&pool, 1).await.unwrap().unwrap().token_version, 0);
        // Revocar un id que no existe no inventa un éxito.
        assert!(!revocar_sesiones(&pool, 999).await.unwrap());
    }

    fn nuevo(email: &str, inst: Option<i64>) -> UsuarioNuevo {
        UsuarioNuevo {
            nombre: "Nuevo".into(),
            email: email.into(),
            password_hash: "h-nuevo".into(),
            rol: "medico".into(),
            institucion_id: inst,
        }
    }

    #[tokio::test]
    async fn un_admin_crea_en_su_institucion_ignorando_lo_que_pidan() {
        let bd = base().await;
        // Pide institución 2, pero como el ámbito es de la 1, se crea en la 1.
        let r = crear(&bd, Ambito::institucion(1), &nuevo("n1@x.co", Some(2))).await.unwrap();
        assert!(matches!(r, ResultadoCrear::Creado(_)));
        let u = por_email(&bd, "n1@x.co").await.unwrap().unwrap();
        assert_eq!(u.institucion_id, Some(1), "un admin no crea usuarios en otra institución");
    }

    #[tokio::test]
    async fn el_superadmin_coloca_al_usuario_donde_indica() {
        let bd = base().await;
        let r = crear(&bd, Ambito::Superadmin, &nuevo("n2@x.co", Some(2))).await.unwrap();
        assert!(matches!(r, ResultadoCrear::Creado(_)));
        assert_eq!(por_email(&bd, "n2@x.co").await.unwrap().unwrap().institucion_id, Some(2));
    }

    #[tokio::test]
    async fn sin_ambito_no_crea_y_el_correo_repetido_se_rechaza() {
        let bd = base().await;
        assert!(matches!(crear(&bd, Ambito::Ninguno, &nuevo("n3@x.co", Some(1))).await.unwrap(), ResultadoCrear::SinPermiso));
        // 'ruiz@x.co' ya existe.
        assert!(matches!(crear(&bd, Ambito::institucion(1), &nuevo("ruiz@x.co", None)).await.unwrap(), ResultadoCrear::EmailEnUso));
    }
}
