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

    // `sembrar-demo`: crea la institución y el usuario de la DEMOSTRACIÓN
    // pública, con las credenciales que el propio despliegue publica. Es
    // idempotente: se puede correr en cada arranque o cada noche.
    //
    // Vive en el binario, y no en un SQL suelto, por lo mismo que `hashear`: el
    // hash tiene que salir de los MISMOS parámetros de Argon2 que verifica el
    // acceso. Un hash de otra herramienta deja al usuario fuera y el fallo
    // aparece lejos de su causa.
    if std::env::args().nth(1).as_deref() == Some("sembrar-demo") {
        return sembrar_demo().await;
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
        demo: cfg.demo.clone(),
        capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()),
    };
    if let Some(d) = &cfg.demo {
        // A gritos y en el log, no de tapadillo: quien arranque esto por error
        // en la máquina de un cliente tiene que enterarse en el primer arranque.
        println!(
            "MODO DEMOSTRACIÓN: /api/demo publica las credenciales «{}» a quien pregunte, \
             sin sesión. Esto NO es una instalación de cliente.",
            d.correo
        );
    }
    let app = rutas::router(estado);

    let dir = format!("{}:{}", cfg.escucha, cfg.puerto);
    let listener = match tokio::net::TcpListener::bind(&dir).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("no se pudo escuchar en {dir}: {e}");
            return ExitCode::FAILURE;
        }
    };

    // La zona se dice SIEMPRE, no solo cuando se configura. Es el dato que
    // decide qué día es hoy —la edad del paciente, el vencimiento de la
    // licencia— y con qué hora se reporta cada atención en el RIPS, y hasta el
    // 2026-08-03 salía del reloj de la máquina sin que nadie lo supiera: en el
    // VPS de la demostración, que va en UTC, eso corría cinco horas todas las
    // atenciones y el archivo se aceptaba igual. Un dato así no puede quedar
    // implícito; si el log no lo nombra, nadie lo mira hasta que no cuadra algo.
    println!(
        "zona horaria del prestador: {} (hoy son las {}). Se cambia con MEDICO_ZONA.",
        cfg.zona,
        medico_dominio::zona::ahora().format("%Y-%m-%d %H:%M")
    );
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

/// Siembra la institución y el usuario de la demostración pública.
///
/// Se apoya en las MISMAS variables que hacen que `/api/demo` publique las
/// credenciales, así que no puede sembrarse un usuario distinto del que la
/// pantalla anuncia: si `MEDICO_DEMO_CORREO` y `MEDICO_DEMO_CLAVE` no están, no
/// hay nada que sembrar y se dice, en vez de inventar un usuario «demo» que
/// nadie pidió (directiva 20).
///
/// La licencia se fija con **todo el catálogo**, no con una lista escrita a
/// mano: así el menú de la demostración enseña el producto completo, y cada
/// módulo aparece como `habilitado` o `pendiente` según lo que el binario sirva
/// de verdad. Una lista aparte se desincronizaría al primer módulo nuevo.
async fn sembrar_demo() -> ExitCode {
    let cfg = match config::Config::desde_entorno() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuración inválida: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(demo) = cfg.demo else {
        eprintln!(
            "no hay nada que sembrar: falta MEDICO_DEMO_CORREO y MEDICO_DEMO_CLAVE. \
             Son las mismas que publica /api/demo, para que la pantalla no anuncie \
             unas credenciales distintas de las que existen."
        );
        return ExitCode::FAILURE;
    };
    let pool = match medico_datos::abrir(&cfg.url_bd).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("no se pudo abrir la base ({}): {e}", cfg.url_bd);
            return ExitCode::FAILURE;
        }
    };
    let Some(hash) = claves::hashear(&demo.clave) else {
        eprintln!("no se pudo generar el hash de la clave de demostración");
        return ExitCode::FAILURE;
    };

    let modulos: Vec<String> = medico_dominio::modulo::CATALOGO
        .iter()
        .map(|m| m.nombre.to_string())
        .collect();
    let json = serde_json::to_string(&modulos).expect("una lista de cadenas siempre serializa");

    let r = sqlx::query(
        "INSERT INTO institucion (id, nombre, nit, activa, modulos_habilitados)
         VALUES (1, 'IPS de demostración — Xiliux', '900000000-0', 1, ?)
         ON CONFLICT(id) DO UPDATE SET modulos_habilitados = excluded.modulos_habilitados",
    )
    .bind(&json)
    .execute(&pool)
    .await;
    if let Err(e) = r {
        eprintln!("no se pudo crear la institución de demostración: {e}");
        return ExitCode::FAILURE;
    }

    // `ON CONFLICT(email)`: al resembrar se repone la clave publicada, que es
    // justo lo que hace falta si alguien la cambió durante la demostración.
    let r = sqlx::query(
        "INSERT INTO usuario (nombre, email, password_hash, rol, institucion_id, activo)
         VALUES ('Doctora de demostración', ?, ?, 'admin', 1, 1)
         ON CONFLICT(email) DO UPDATE SET password_hash = excluded.password_hash, activo = 1",
    )
    .bind(&demo.correo)
    .bind(&hash)
    .execute(&pool)
    .await;
    if let Err(e) = r {
        eprintln!("no se pudo crear el usuario de demostración: {e}");
        return ExitCode::FAILURE;
    }

    // LAS SALAS DE QUIRÓFANO, porque sin ellas el módulo no se puede usar.
    //
    // Nada las crea: no hay endpoint de alta ni en Python ni en Rust, y en el
    // producto vivo salen de `seed.py`. Sin una sala, `POST
    // /api/quirofano/programaciones` responde 404 a todo y la pantalla queda
    // muerta — o sea que portar el módulo sin esto sería entregar algo que no
    // funciona en una instalación nueva.
    //
    // Administrar salas —darlas de alta, renombrarlas, ponerlas en
    // mantenimiento— no tiene pantalla en NINGUNA de las dos pilas. Es un hueco
    // real del producto y su sitio natural es el módulo de administración.
    let salas = [
        ("Quirófano 1", "GENERAL"),
        ("Quirófano 2", "GENERAL"),
        ("Sala de partos", "PARTOS"),
        ("Cirugía ambulatoria", "AMBULATORIA"),
    ];
    for (nombre, tipo) in salas {
        let r = sqlx::query(
            "INSERT INTO quirofano (institucion_id, nombre, tipo) VALUES (1, ?, ?)
             ON CONFLICT(institucion_id, nombre) DO NOTHING",
        )
        .bind(nombre)
        .bind(tipo)
        .execute(&pool)
        .await;
        if let Err(e) = r {
            eprintln!("no se pudo crear la sala «{nombre}»: {e}");
            return ExitCode::FAILURE;
        }
    }

    println!(
        "demostración sembrada: institución 1 con {} módulos licenciados, el usuario «{}» y {} salas de quirófano.",
        modulos.len(),
        demo.correo,
        salas.len()
    );
    ExitCode::SUCCESS
}
