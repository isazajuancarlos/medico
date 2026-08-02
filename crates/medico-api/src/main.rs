//! El servidor de medico: lee la configuración, abre la base y sirve la API.

mod auth;
mod capacidades;
mod claves;
mod config;
mod estaticos;
mod maestros;
mod modulos;
mod rips;
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

    // `generar-maestro-cum <cum.csv> <salida.json>`: convierte el export del
    // INVIMA al JSON que se EMPOTRA en el binario.
    //
    // ANTES ESTO SE LLAMABA `cargar-maestros` Y ESCRIBÍA EN LA BASE DEL CLIENTE.
    // Se cambió al empotrar el CUM (#77): con el maestro empotrado y sembrado en
    // cada arranque, una carga manual quedaría PISADA por el siguiente reinicio
    // —en silencio— porque el sembrado volvería a poner el maestro del binario.
    // Dos caminos para lo mismo, y el segundo revirtiendo al primero sin avisar.
    //
    // La lógica de lectura NO cambió ni se duplicó: es la misma `maestros::leer`
    // de siempre, con todo lo que costó aprender —normalizar encabezados, mapear
    // por nombre y nunca por posición, componer la concentración de tres columnas
    // porque la que se llama `concentracion` es un código de una letra—. Lo único
    // que cambia es a dónde va el resultado.
    //
    // Se corre A MANO cuando el INVIMA publica un export nuevo, y el JSON
    // resultante se versiona con el código.
    if std::env::args().nth(1).as_deref() == Some("generar-maestro-cum") {
        let (Some(csv), Some(salida)) = (std::env::args().nth(2), std::env::args().nth(3)) else {
            eprintln!("uso: medico-api generar-maestro-cum <cum.csv> <salida.json>");
            return ExitCode::FAILURE;
        };
        return match maestros::generar_json(
            std::path::Path::new(&csv),
            std::path::Path::new(&salida),
        ) {
            Ok(r) => {
                println!(
                    "maestro CUM: {} filas leídas · {} medicamentos distintos · \
                     {} omitidas (sin CUM o sin nombre) -> {salida}",
                    r.filas, r.medicamentos, r.omitidas
                );
                println!(
                    "RECUERDA: el JSON se empotra al compilar. Hace falta recompilar \
                     para que llegue a un despliegue."
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("generar-maestro-cum: {e}");
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

    // Los maestros clínicos, por lo mismo y con el mismo coste medido (46 ms de
    // parseo + 37 ms de escritura para las 26 063 filas). Sin ellos, el
    // diagnóstico y el procedimiento vuelven a ser texto libre y el RIPS se
    // rechaza tres semanas después.
    match medico_datos::clinico::sembrar(&pool).await {
        Ok((cie, cups, cum)) => println!(
            "CIE-10: {cie} diagnósticos · CUPS: {cups} procedimientos · CUM: {cum} medicamentos"
        ),
        Err(e) => {
            eprintln!("no se pudieron sembrar los maestros clínicos: {e}");
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
