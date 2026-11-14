//! Envía un RIPS ya generado al Mecanismo Único de Validación y trae el CUV.
//!
//! ```text
//! cargo run -p medico-muv --features cli --bin enviar-rips -- ARCHIVO.json
//! ```
//!
//! # Para qué existe
//!
//! El binario del producto ya sabe enviar (`/panel/rips`), pero eso exige tener
//! el servidor levantado, una sesión y el periodo dentro de su base. Esto es la
//! otra puerta: **un archivo que ya existe, subido ahora**. Es lo que hoy se
//! hace a mano desde la aplicación de escritorio, y lo que falla cuando esa
//! aplicación no arranca.
//!
//! # Las credenciales van por ENTORNO, nunca por la línea de órdenes
//!
//! Son del PRESTADOR (caso 2026403888: no existe credencial de integrador), y un
//! argumento queda en el historial del shell y en la lista de procesos, donde lo
//! ve cualquier usuario de la máquina. Es la misma regla que ya sigue
//! `validar_muv.py`.
//!
//! | variable | |
//! |---|---|
//! | `MEDICO_MUV_URL` | dónde escucha el contenedor (defecto `https://localhost:9443`) |
//! | `MEDICO_MUV_TIPO_DOCUMENTO` | `CC`, `CE`… |
//! | `MEDICO_MUV_DOCUMENTO` | el documento de la persona registrada en SISPRO |
//! | `MEDICO_MUV_CLAVE` | su clave |
//! | `MEDICO_MUV_NIT` | el NIT del prestador |
//! | `MEDICO_MUV_TIPO_USUARIO` | solo el independiente: `RE`, `PIN`, `PINx`, `PIE` |
//! | `MEDICO_MUV_DESTINO` | `sin_factura` (defecto) o `fev` |
//! | `MEDICO_MUV_CA` | el certificado del contenedor, en PEM |
//! | `MEDICO_MUV_TLS_INSEGURO` | `true` para no verificarlo — solo contra esta máquina |
//!
//! # Un envío NO se deshace
//!
//! Contra el ambiente de producción, un paquete aceptado obtiene un CUV con
//! efectos ante la EPS y la Supersalud. Por eso esto enseña primero QUÉ va a
//! mandar y a DÓNDE, y espera a que se teclee `enviar`. Con `--sin-preguntar` se
//! salta la confirmación, y entonces el que responde es quien lo escribió en el
//! guion.

use std::io::{Read, Write};
use std::time::Duration;

use medico_muv::{ClienteMuv, Credenciales, Destino, ErrorMuv, Tls};

fn var(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

/// Las cinco obligatorias, o la lista de las que faltan. Media credencial no
/// identifica a nadie, así que no se arranca con parte (directiva 20).
fn credenciales() -> Result<(Credenciales, String), String> {
    let mut faltan = Vec::new();
    let mut pedir = |k: &str| match var(k) {
        Some(v) => v,
        None => {
            faltan.push(k.to_string());
            String::new()
        }
    };
    let tipo_documento = pedir("MEDICO_MUV_TIPO_DOCUMENTO");
    let numero_documento = pedir("MEDICO_MUV_DOCUMENTO");
    let clave = pedir("MEDICO_MUV_CLAVE");
    let nit = pedir("MEDICO_MUV_NIT");
    if !faltan.is_empty() {
        return Err(format!("faltan estas variables de entorno: {}", faltan.join(", ")));
    }
    Ok((
        Credenciales {
            tipo_documento,
            numero_documento,
            clave,
            nit: nit.clone(),
            tipo_usuario: var("MEDICO_MUV_TIPO_USUARIO"),
        },
        nit,
    ))
}

fn tls() -> Tls {
    if var("MEDICO_MUV_TLS_INSEGURO").as_deref() == Some("true") {
        return Tls::SinVerificar;
    }
    match var("MEDICO_MUV_CA") {
        Some(pem) => Tls::Certificado(pem.into_bytes()),
        None => Tls::DelSistema,
    }
}

/// Lee el JSON tolerando el **BOM**, que es lo que escribe el conversor oficial
/// de MinSalud y lo que trae un archivo real del prestador. `serde_json` lo
/// rechaza con «expected value at line 1 column 1», un error que no dice nada
/// sobre la causa — costó un rato la primera vez.
fn leer_json(ruta: &str) -> Result<serde_json::Value, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(ruta)
        .map_err(|e| format!("no se pudo abrir {ruta}: {e}"))?
        .read_to_end(&mut bytes)
        .map_err(|e| format!("no se pudo leer {ruta}: {e}"))?;
    let sin_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    serde_json::from_slice(sin_bom).map_err(|e| format!("{ruta} no es un JSON válido: {e}"))
}

/// El cuerpo que espera la API local: el objeto pelado va DENTRO de `rips`.
///
/// Si el archivo ya viene envuelto se manda tal cual — misma regla que
/// `validar_muv.py:122`, para que los dos acepten los mismos archivos.
fn envolver(crudo: serde_json::Value) -> serde_json::Value {
    if crudo.get("rips").is_some() {
        return crudo;
    }
    serde_json::json!({ "rips": crudo, "xmlFevFile": "" })
}

/// Qué lleva el archivo, para poder mirarlo antes de mandarlo.
fn resumen(rips: &serde_json::Value) -> String {
    let usuarios = rips.get("usuarios").and_then(|u| u.as_array());
    let n = usuarios.map(|u| u.len()).unwrap_or(0);
    let mut servicios: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut fechas: Vec<String> = Vec::new();
    for u in usuarios.into_iter().flatten() {
        if let Some(map) = u.get("servicios").and_then(|s| s.as_object()) {
            for (clase, lista) in map {
                let filas = lista.as_array().map(|l| l.len()).unwrap_or(0);
                *servicios.entry(clase.as_str()).or_default() += filas;
                for s in lista.as_array().into_iter().flatten() {
                    for campo in ["fechaInicioAtencion", "fechaAtencion", "fechaDispensAcion"] {
                        if let Some(f) = s.get(campo).and_then(|v| v.as_str()) {
                            fechas.push(f.to_string());
                        }
                    }
                }
            }
        }
    }
    fechas.sort();
    let periodo = match (fechas.first(), fechas.last()) {
        (Some(a), Some(b)) => format!("{a} → {b}"),
        _ => "(sin fechas)".into(),
    };
    let detalle: Vec<String> = servicios.iter().map(|(k, v)| format!("{v} {k}")).collect();
    format!(
        "  prestador (NIT del archivo): {}\n  nota: {} (tipo {})   factura: {}\n  \
         usuarios: {n}   servicios: {}\n  periodo: {periodo}",
        rips.get("numDocumentoIdObligado").and_then(|v| v.as_str()).unwrap_or("—"),
        rips.get("numNota").and_then(|v| v.as_str()).unwrap_or("—"),
        rips.get("tipoNota").and_then(|v| v.as_str()).unwrap_or("—"),
        rips.get("numFactura").and_then(|v| v.as_str()).unwrap_or("(ninguna)"),
        if detalle.is_empty() { "—".into() } else { detalle.join(", ") },
    )
}

fn confirmar(url: &str) -> bool {
    // El ambiente de verdad lo dice la respuesta, pero el registro al que se
    // apunta ya avisa: si la URL no huele a stage, se trata como producción.
    let produccion = !url.to_lowercase().contains("stage");
    println!();
    if produccion {
        println!("  ⚠  Esto parece el ambiente de PRODUCCIÓN: un paquete aceptado obtiene un");
        println!("     CUV con efectos ante la EPS y la Supersalud. No se deshace.");
    } else {
        println!("  Ambiente de PRUEBAS por la URL. Recuerde: contra stage, solo datos sintéticos.");
    }
    print!("\n  Escriba «enviar» para continuar: ");
    let _ = std::io::stdout().flush();
    let mut linea = String::new();
    if std::io::stdin().read_line(&mut linea).is_err() {
        return false;
    }
    linea.trim().eq_ignore_ascii_case("enviar")
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sin_preguntar = args.iter().any(|a| a == "--sin-preguntar");
    let Some(ruta) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!(
            "uso: enviar-rips ARCHIVO.json [--sin-preguntar]\n\
             \n\
             Las credenciales van por entorno: MEDICO_MUV_URL, MEDICO_MUV_TIPO_DOCUMENTO,\n\
             MEDICO_MUV_DOCUMENTO, MEDICO_MUV_CLAVE, MEDICO_MUV_NIT."
        );
        return std::process::ExitCode::from(2);
    };

    let crudo = match leer_json(ruta) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("✗ {e}");
            return std::process::ExitCode::from(2);
        }
    };
    let (credenciales, nit_cfg) = match credenciales() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("✗ {e}");
            return std::process::ExitCode::from(2);
        }
    };
    let url = var("MEDICO_MUV_URL").unwrap_or_else(|| "https://localhost:9443".into());
    let destino = match var("MEDICO_MUV_DESTINO").as_deref() {
        Some("fev") => Destino::Fev,
        _ => Destino::SinFactura,
    };

    let paquete = envolver(crudo);
    let rips = paquete.get("rips").unwrap_or(&paquete);

    println!("\n  archivo: {ruta}");
    println!("{}", resumen(rips));
    println!("  destino: {url}  ({})", destino.ruta());

    // El NIT del archivo y el de las credenciales tienen que ser el mismo: la
    // misma comprobación que hace el panel, y por el mismo motivo — radicar con
    // el NIT del vecino no se deshace.
    if let Some(nit_archivo) = rips.get("numDocumentoIdObligado").and_then(|v| v.as_str())
        && nit_archivo.trim() != nit_cfg.trim()
    {
        eprintln!(
            "\n✗ El archivo declara el prestador {nit_archivo} y las credenciales son del \
             NIT {nit_cfg}. No se envía."
        );
        return std::process::ExitCode::from(2);
    }

    if !sin_preguntar && !confirmar(&url) {
        println!("\n  Cancelado. No se envió nada.");
        return std::process::ExitCode::SUCCESS;
    }

    let cliente = match ClienteMuv::nuevo(&url, Duration::from_secs(300), tls()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\n✗ {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    println!("\n  Iniciando sesión…");
    if let Err(e) = cliente.login(&credenciales).await {
        eprintln!("✗ {e}");
        if matches!(e, ErrorMuv::NoDisponible(_)) {
            eprintln!(
                "\n  Si dice «connection refused», el contenedor del validador no está\n  \
                 levantado. Compruebe que escucha en {url}."
            );
        }
        return std::process::ExitCode::FAILURE;
    }

    println!("  Enviando…");
    let r = match cliente.cargar(&paquete, destino).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("✗ {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    // El resultado crudo se guarda SIEMPRE, aceptado o no: es la constancia de
    // lo que contestó el Ministerio, y es lo que se mira cuando alguien pregunta
    // por qué no se radicó. Al lado del archivo, como hace el conversor oficial.
    let salida = format!("{ruta}.resultado.json");
    match serde_json::to_string_pretty(&r.crudo) {
        Ok(txt) => {
            if let Err(e) = std::fs::write(&salida, txt) {
                eprintln!("  (aviso: no se pudo guardar {salida}: {e})");
            } else {
                println!("  Respuesta guardada en {salida}");
            }
        }
        Err(e) => eprintln!("  (aviso: respuesta no serializable: {e})"),
    }

    println!();
    if let Some(a) = &r.ambiente {
        println!("  ambiente: {a}");
    }
    if let Some(p) = r.proceso_id {
        println!("  proceso:  {p}");
    }

    if r.aceptado {
        println!("\n  ✓ ACEPTADO");
        println!("    CUV: {}", r.cuv.clone().unwrap_or_default());
        if let Some(f) = &r.fecha_radicacion {
            println!("    radicado: {f}");
        }
        let avisos = r.notificaciones().len();
        if avisos > 0 {
            println!(
                "\n    {avisos} notificación(es). No impidieron el CUV, pero desde el\n    \
                 1-jun-2026 varias reglas de ese grupo pasaron a causal de rechazo."
            );
            for h in r.notificaciones() {
                println!("      · {} {}", h.codigo, h.descripcion);
                if !h.path_fuente.is_empty() {
                    println!("        en {}", h.path_fuente);
                }
            }
        }
        return std::process::ExitCode::SUCCESS;
    }

    println!("\n  ✗ NO ACEPTADO — no hay CUV.");
    let campos = r.campos_a_corregir();
    if !campos.is_empty() {
        println!("\n  Qué corregir, por campo:");
        for (campo, lista) in &campos {
            println!("    {campo}");
            for h in lista {
                let clase = if h.es_notificacion() { "aviso" } else { "rechazo" };
                println!("      · [{clase}] {} {}", h.codigo, h.descripcion);
                if !h.observaciones.is_empty() {
                    println!("        {}", h.observaciones);
                }
            }
        }
    }
    let sueltos: Vec<_> = r.hallazgos.iter().filter(|h| h.path_fuente.is_empty()).collect();
    if !sueltos.is_empty() {
        println!("\n  Sin campo asociado:");
        for h in sueltos {
            println!("    · {} {}", h.codigo, h.descripcion);
        }
    }
    std::process::ExitCode::FAILURE
}
