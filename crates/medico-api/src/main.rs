//! El servidor de medico: lee la configuración, abre la base y sirve la API.

mod auth;
mod claves;
mod config;
mod estaticos;
mod rutas;
mod sesion;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    // `hashear <clave>`: produce un hash Argon2 con los MISMOS parámetros que
    // verifica el acceso. Lo usa el seeder de la demo; un hash de otra
    // herramienta con otra configuración deja al usuario fuera y el fallo
    // aparece lejos de su causa.
    if std::env::args().nth(1).as_deref() == Some("hashear") {
        let clave = std::env::args().nth(2).unwrap_or_default();
        if clave.is_empty() {
            eprintln!("uso: medico-api hashear <clave>");
            return ExitCode::FAILURE;
        }
        return match claves::hashear(&clave) {
            Some(h) => {
                println!("{h}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("no se pudo generar el hash");
                ExitCode::FAILURE
            }
        };
    }

    let cfg = match config::Config::desde_entorno() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuración: {e}");
            return ExitCode::FAILURE;
        }
    };

    let pool = match medico_datos::abrir(&cfg.url_bd).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("no se pudo abrir la base ({}): {e}", cfg.url_bd);
            return ExitCode::FAILURE;
        }
    };

    let dir_imagenes = std::path::PathBuf::from(&cfg.dir_imagenes);
    if let Err(e) = std::fs::create_dir_all(&dir_imagenes) {
        eprintln!("no se pudo crear la carpeta de imágenes ({}): {e}", dir_imagenes.display());
        return ExitCode::FAILURE;
    }

    let estado = auth::Estado {
        pool,
        secreto: cfg.secreto_sesion,
        minutos: cfg.minutos_sesion,
        dir_imagenes,
    };
    let app = rutas::router(estado);

    let dir = format!("0.0.0.0:{}", cfg.puerto);
    let listener = match tokio::net::TcpListener::bind(&dir).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("no se pudo escuchar en {dir}: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("medico-api escuchando en http://{dir}");
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("el servidor terminó con error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
