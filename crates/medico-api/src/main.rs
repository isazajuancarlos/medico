//! El servidor de medico: lee la configuración, abre la base y sirve la API.

// `result_large_err`: FALSO POSITIVO en este crate, y por eso se silencia a nivel
// de crate y no función a función. El idiom de axum de estos manejadores es
// `Result<T, Response>` —el `Err` lleva un `axum::http::Response` para
// cortocircuitar en una respuesta HTTP—; el lint lo marca «grande», pero el `Ok`
// es del mismo orden y boxearlo (`Box<Response>`) sería churn en 13 firmas y sus
// call-sites sin ganar nada medible en manejadores async. Empezó a disparar con
// clippy 1.98 (2026-08-18): estaba ROJO en `main` antes de tocar nada, no es una
// regresión de ningún cambio de features. Si un día se quiere boxear de verdad,
// se quita este allow y se ve la lista completa.
#![allow(clippy::result_large_err)]

mod auth;
mod capacidades;
mod claves;
mod config;
mod ihce;
mod muv;
mod licencia;
mod maestros;
mod modulos;
mod pixeles;
mod respaldo;
mod retencion;
mod rips;
mod rutas;
mod sesion;
mod tipos;
mod vista;

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

    // `respaldar <carpeta>` / `restaurar <carpeta> [--forzar]`: copia verificada
    // de la base y las imágenes, y su vuelta (B4.a, #733). Leen `DATABASE_URL` y
    // `MEDICO_DIR_IMAGENES` DIRECTAMENTE —con los mismos defaults que `config.rs`—
    // y no `Config::desde_entorno()`, para no exigir `SECRET_KEY`: restaurar en
    // una máquina nueva ocurre antes de que el secreto esté puesto.
    let url_bd = || std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://medico.db".into());
    let dir_imgs = || std::env::var("MEDICO_DIR_IMAGENES").unwrap_or_else(|_| "storage/imagenes".into());
    // El archivo bajo custodia de las supresiones (#758): si está configurado, entra
    // en el respaldo (es la única copia del expediente purgado). Si no, no hay nada
    // que copiar y el respaldo es completo igual.
    let dir_arch = || std::env::var("MEDICO_RETENCION_DIR").ok();

    if std::env::args().nth(1).as_deref() == Some("respaldar") {
        let Some(carpeta) = std::env::args().nth(2) else {
            eprintln!("uso: medico-api respaldar <carpeta-destino>");
            return ExitCode::FAILURE;
        };
        return match respaldo::respaldar(
            &url_bd(),
            &dir_imgs(),
            dir_arch().as_deref(),
            std::path::Path::new(&carpeta),
        )
        .await
        {
            Ok(r) => {
                println!(
                    "respaldo verificado en {carpeta}: {} filas en {} tablas, \
                     {} imágenes, {} KiB de base",
                    r.total_filas(),
                    r.filas.len(),
                    r.imagenes,
                    r.bytes_bd / 1024
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("respaldar: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if std::env::args().nth(1).as_deref() == Some("restaurar") {
        let Some(carpeta) = std::env::args().nth(2) else {
            eprintln!("uso: medico-api restaurar <carpeta> [--forzar]");
            return ExitCode::FAILURE;
        };
        let forzar = std::env::args().any(|a| a == "--forzar");
        return match respaldo::restaurar(
            std::path::Path::new(&carpeta),
            &url_bd(),
            &dir_imgs(),
            dir_arch().as_deref(),
            forzar,
        )
        .await
        {
            Ok(r) => {
                println!(
                    "restaurado desde {carpeta}: {} filas, {} imágenes",
                    r.total_filas(),
                    r.imagenes
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("restaurar: {e}");
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
    let municipios = match medico_datos::divipola::sembrar(&pool).await {
        Ok(n) => {
            println!("DIVIPOLA: {n} municipios al día");
            n
        }
        Err(e) => {
            eprintln!("no se pudo sembrar DIVIPOLA: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Los maestros clínicos, por lo mismo y con el mismo coste medido (46 ms de
    // parseo + 37 ms de escritura para las 26 063 filas). Sin ellos, el
    // diagnóstico y el procedimiento vuelven a ser texto libre y el RIPS se
    // rechaza tres semanas después.
    match medico_datos::clinico::sembrar(&pool).await {
        Ok(m) => {
            println!(
                "CIE-10: {} diagnósticos · CUPS: {} procedimientos · CUM: {} medicamentos · \
                 unidades de dispensación: {}",
                m.cie10, m.cups, m.cum, m.unidades
            );

            // DE DÓNDE SALIÓ CADA MAESTRO Y DE CUÁNDO, EN CADA ARRANQUE.
            //
            // Un maestro caducado o no oficial NO FALLA: deja pasar códigos que
            // ya no existen, rechaza los nuevos, y la primera señal llega como
            // GLOSA semanas después. Hasta el 2026-08-14 tampoco se podía saber
            // de cuándo eran — los cinco declaraban metadatos distintos e
            // incompletos y esto no decía nada.
            //
            // Sale SIEMPRE, avise o no, por el mismo motivo que `MEDICO_ZONA` y
            // que `alertas_pasada`: un arranque silencioso significaría a la vez
            // «todo bien» y «esto no llegó a correr».
            //
            // Y NO BLOQUEA. Dejar un consultorio sin sistema por un catálogo
            // viejo es peor que el catálogo viejo (modo degradado, #71).
            let filas = [m.cie10, m.cups, m.cum, municipios, m.unidades];
            let procedencias = medico_datos::clinico::procedencias();
            println!("procedencia de los maestros:");
            for (i, p) in procedencias.iter().enumerate() {
                println!("{}", p.renglon(filas.get(i).copied().unwrap_or(0)));
            }
            for p in procedencias.iter().filter(|p| p.hay_que_decirlo()) {
                if let Some(a) = &p.advertencia {
                    eprintln!("AVISO · {}: {a}", p.maestro);
                } else {
                    eprintln!(
                        "AVISO · {}: no declara de cuándo es, así que no se puede \
                         saber si caducó. Contrástalo antes de facturar con él.",
                        p.maestro
                    );
                }
            }
        }
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

    // NAMESPACE POR INSTANCIA (#758, rediseño del fail-safe #2, 2026-09-27): cada base
    // guarda sus imágenes en `dir_imagenes/{uuid}/{id}.dat` (uuid de `instancia`, migración
    // 0077), así que un `MEDICO_DIR_IMAGENES`/`DATABASE_URL` cruzado entre bases de uuid
    // DISTINTO escribe, sirve, reconcilia y purga SOLO en la subcarpeta de ESTA base —no lee,
    // exfiltra ni borra la PHI de otra—. Reemplaza de raíz al sello `.instancia` + su ventana
    // de bootstrap + el brazo `Ajena` (retirados; D1: quita la causa, no contiene el síntoma;
    // 4º concilio midió que el sello TOFU dejaba dos vías más de borrado/fuga). LÍMITE: dos
    // CLONES vivos (mismo uuid) sobre el mismo dir quedan fuera de cobertura (D26; ver
    // `respaldo::phi_de_otra_instancia`), y `restaurar` rechaza pisar la PHI de otro uuid.
    //
    // Aquí solo se MIGRA el layout legado plano (`{id}.dat`) al nuevo; es idempotente y
    // best-effort (una migración parcial es segura: servir usa `ruta_archivo`, reconciliar
    // solo mira la subcarpeta, y las filas no migradas se reintentan el próximo arranque).
    match medico_datos::imagen::migrar_a_namespace_por_instancia(&pool, &dir_imagenes).await {
        Ok(0) => {}
        Ok(n) => println!("imágenes: {n} archivo(s) migrados al namespace por instancia (#758)"),
        Err(e) => eprintln!(
            "imágenes: la migración al namespace por instancia falló (se reintenta el próximo arranque): {e}"
        ),
    }

    // El envío al MUV se resuelve ANTES de construir el estado, y el arranque
    // DICE en qué quedó — esté configurado o no. Un servicio que calla cuando no
    // está configurado se ve igual que uno sano, y eso se descubre el día del
    // cierre de mes (directiva 30).
    let muv_config = muv::config_desde_entorno();
    match &muv_config {
        Some(c) => println!(
            "MUV: envío configurado contra {} (NIT {}){}",
            c.base_url,
            c.nit,
            match c.tls {
                medico_muv::Tls::SinVerificar =>
                    "  ¡SIN VERIFICAR EL CERTIFICADO! Solo es defendible si el contenedor \
                     corre en esta misma máquina.",
                medico_muv::Tls::Certificado(_) => "  (verificando contra su certificado)",
                medico_muv::Tls::DelSistema => "  (verificando con las raíces del sistema)",
            }
        ),
        None => println!(
            "MUV: envío NO configurado — el archivo se descarga y se sube a mano. \
             Para activarlo: MEDICO_MUV_URL, MEDICO_MUV_TIPO_DOCUMENTO, MEDICO_MUV_DOCUMENTO, \
             MEDICO_MUV_CLAVE y MEDICO_MUV_NIT."
        ),
    }

    // La config del transporte IHCE (#603 F4.6) se resuelve UNA VEZ: la usa el
    // Estado para saber si la pantalla puede EMITIR, y —más abajo— la tarea de
    // fondo la consume para construir el cliente real. `None` = sin credenciales
    // de sandbox, el caso normal.
    let cfg_ihce = ihce::config_desde_entorno();

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
        ihce_configurado: cfg_ihce.is_some(),
        entorno_atencion: cfg.entorno_atencion.clone(),
        capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()),
        muv: muv_config.map(std::sync::Arc::new),
    };
    // El entorno de atención se ANUNCIA siempre —esté configurado o por defecto—,
    // porque viaja en cada RDA de encuentro al visor nacional: un despliegue
    // extramural que herede el `05` Institucional tiene que verlo, no descubrirlo
    // (directiva 30, mismo criterio que la zona).
    println!(
        "entorno de atención (RDA): {} — {}",
        estado.entorno_atencion,
        medico_dominio::ihce::terminologia::display(
            medico_dominio::ihce::terminologia::SISTEMA_ENTORNO,
            &estado.entorno_atencion,
        )
        .unwrap_or("desconocido"),
    );
    if let Some(d) = &cfg.demo {
        // A gritos y en el log, no de tapadillo: quien arranque esto por error
        // en la máquina de un cliente tiene que enterarse en el primer arranque.
        println!(
            "MODO DEMOSTRACIÓN: /api/demo publica las credenciales «{}» a quien pregunte, \
             sin sesión. Esto NO es una instalación de cliente.",
            d.correo
        );
    }
    // Cola de envío de RDA al mecanismo IHCE (#603 fase 4b): con configuración,
    // una tarea de fondo la drena con reintentos y backoff, sin bloquear la
    // operación clínica. SIN configuración —el caso normal hasta tener las
    // credenciales de sandbox— no arranca, y se DICE en el log: «no envía RDA» no
    // debe confundirse con «falla en silencio» (directiva 30).
    match cfg_ihce {
        Some(cfg_ihce) => {
            let cliente = medico_ihce::ClienteIhce::nuevo(cfg_ihce);
            let pool_cola = estado.pool.clone();
            tokio::spawn(async move { ihce::correr_cola(pool_cola, cliente).await });
            println!("cola IHCE: ACTIVA (drenando envíos de RDA en segundo plano)");
        }
        None => println!("cola IHCE: inactiva (sin configuración MEDICO_IHCE_*)"),
    }

    // Retención y supresión por defunción (#758, Ley 1581): con configuración, una
    // tarea de fondo barre a diario los expedientes de fallecidos hace ≥20 años,
    // los archiva cifrados bajo custodia y los purga. SIN configuración —el caso
    // normal hasta que el prestador ponga `MEDICO_RETENCION_KEY`/`_DIR`— NO arranca
    // y se DICE (directiva 30): «no suprime» no es «falla en silencio». El binario
    // no cambia de comportamiento observable sin esa configuración.
    match retencion::config_desde_entorno() {
        Some(cfg_ret) => {
            let pool_ret = estado.pool.clone();
            tokio::spawn(async move { retencion::correr(pool_ret, cfg_ret).await });
            println!(
                "retención por defunción: ACTIVA (barrido diario, {} años, archivo cifrado bajo custodia)",
                std::env::var("MEDICO_RETENCION_ANOS").unwrap_or_else(|_| "20".into())
            );
        }
        None => println!(
            "retención por defunción: inactiva (sin MEDICO_RETENCION_KEY / MEDICO_RETENCION_DIR / MEDICO_SIGNING_KEY)"
        ),
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
    // La dirección REAL a la que ligó, no la que se pidió. Con `PORT=0` —que es
    // como arranca el banco de navegador, para no chocar con nada— lo
    // configurado es `…:0` y el log estaría diciendo un puerto que no existe.
    // Un mensaje de arranque que miente sobre dónde está escuchando el servidor
    // es de los que cuestan una tarde.
    let ligado = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| dir.clone());
    println!("medico-api escuchando en http://{ligado}");
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
