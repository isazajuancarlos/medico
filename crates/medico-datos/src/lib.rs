//! Acceso a la base. Nadie de aquí para arriba escribe SQL.

pub mod acceso;
pub mod ambulancia;
pub mod capacidad;
pub mod catalogo;
pub mod cita;
pub mod diagnostico;
pub mod divipola;
pub mod documento;
pub mod formula;
pub mod freno;
pub mod historia;
pub mod hospitalizacion;
pub mod imagen;
pub mod incapacidad;
pub mod institucion;
pub mod inventario;
pub mod licencia;
pub mod nutricion;
pub mod orden;
pub mod paciente;
pub mod proveedor;
pub mod referencia;
pub mod rehabilitacion;
pub mod urgencias;
pub mod usuario;

pub type Pool = sqlx::SqlitePool;

/// Resultado de editar o borrar algo que puede estar firmado. `Firmada` NO es
/// `NoExiste`: existe y es del ámbito, pero está cerrado. Lo comparten la
/// historia y los documentos clínicos, que aplican la misma inmutabilidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    NoExiste,
    Firmada,
    Hecho,
}

/// Abre la base y aplica las migraciones pendientes.
///
/// `create_if_missing` está a propósito: el despliegue en sitio es un binario y
/// nada más, y pedirle a un consultorio que cree la base a mano antes de
/// arrancar es justo el paso donde se pierde una instalación.
///
/// Las dos opciones que NO son cosméticas:
///
/// - **`foreign_keys(true)`.** SQLite las trae APAGADAS por defecto, por
///   compatibilidad hacia atrás. Con ellas apagadas, un paciente puede apuntar
///   a una institución que no existe; esa fila queda fuera del alcance de todo
///   filtro por institución, así que no la ve nadie —ni siquiera para
///   corregirla— y sigue ahí. Ver `las_claves_foraneas_estan_activas`.
/// - **WAL.** Lectores concurrentes sin bloquear al escritor. No convierte a
///   SQLite en multi-escritor —ese techo se aceptó con los ojos abiertos en
///   `decisiones/0008`— pero es la diferencia entre un consultorio que va bien
///   y uno que se traba al consultar mientras alguien guarda.
pub async fn abrir(url: &str) -> Result<Pool, sqlx::Error> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    let opciones = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opciones)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
