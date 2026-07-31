//! El servidor de medico: lee la configuración, abre la base y sirve la API.

mod auth;
mod capacidades;
mod claves;
mod config;
mod estaticos;
mod maestros;
mod rutas;
mod sesion;
mod tipos;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    // `hashear <clave>`: produce un hash Argon2 con los MISMOS parámetros que
    // verifica el acceso. Lo usa el seeder de la demo; un hash de otra
    // herramienta con otra configuración deja al usuario fuera y el fallo
    // aparece lejos de su causa.
    // `claves-firma`: imprime un par nuevo de claves de firma (pública, privada)
    // en hex. La privada va en MEDICO_SIGNING_KEY (secreta); la pública en
    // MEDICO_VERIFYING_KEY (puede publicarse). Se ejecuta una vez al instalar.
    if std::env::args().nth(1).as_deref() == Some("claves-firma") {
        let (publica, privada) = guaca::firma::generar_claves();
        println!("MEDICO_VERIFYING_KEY={publica}");
        println!("MEDICO_SIGNING_KEY={privada}");
        return ExitCode::SUCCESS;
    }

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

    // `cargar-maestros <ruta.csv>`: llena el catálogo de medicamentos (CUM del
    // INVIMA) desde su export. Se corre una vez al instalar y al actualizar el
    // maestro. NO pide SECRET_KEY —no firma nada—; solo la base (DATABASE_URL),
    // con el mismo defecto que el servidor.
    if std::env::args().nth(1).as_deref() == Some("cargar-maestros") {
        let ruta = match std::env::args().nth(2) {
            Some(r) if !r.trim().is_empty() => r,
            _ => {
                eprintln!("uso: medico-api cargar-maestros <ruta-cum.csv>");
                return ExitCode::FAILURE;
            }
        };
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://medico.db".into());
        let pool = match medico_datos::abrir(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("no se pudo abrir la base ({url}): {e}");
                return ExitCode::FAILURE;
            }
        };
        return match maestros::cargar(&pool, std::path::Path::new(&ruta)).await {
            Ok(r) => {
                println!(
                    "maestro CUM: {} filas leídas · {} medicamentos distintos · \
                     {} filas insertadas/actualizadas · {} omitidas (sin CUM o sin nombre)",
                    r.filas, r.medicamentos, r.cargadas, r.omitidas
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("cargar-maestros: {e}");
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

    // DIVIPOLA en cada arranque: es idempotente y así regenerar el JSON basta
    // para que el siguiente arranque lo aplique, sin migración nueva. Si falla,
    // se para: sin el catálogo, `cod_municipio` volvería a ser texto libre y el
    // RIPS saldría mal sin que nadie lo note.
    match medico_datos::divipola::sembrar(&pool).await {
        Ok(n) => println!("DIVIPOLA: {n} municipios al día"),
        Err(e) => {
            eprintln!("no se pudo sembrar DIVIPOLA: {e}");
            return ExitCode::FAILURE;
        }
    }

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
        clave_firma: cfg.clave_firma,
        clave_verificacion: cfg.clave_verificacion,
        clave_reposo: cfg.clave_reposo,
        tras_proxy: cfg.tras_proxy,
        capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()),
    };
    let app = rutas::router(estado);

    let dir = format!("{}:{}", cfg.escucha, cfg.puerto);
    let listener = match tokio::net::TcpListener::bind(&dir).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("no se pudo escuchar en {dir}: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("medico-api escuchando en http://{dir}");
    // `with_connect_info` y no `serve(listener, app)` a secas: sin esto, el
    // extractor `ConnectInfo` de `/api/acceso` no encuentra la dirección del
    // cliente y el freno al ensayo de contraseñas mete todos los intentos en el
    // mismo cubo. Seguiría frenando —degrada a «por cuenta»— pero un desconocido
    // podría dejar fuera al dueño de la cuenta, que es justo lo que el freno
    // está diseñado para no hacer.
    let app = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("el servidor terminó con error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
