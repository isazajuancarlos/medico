//! La interfaz Angular viaja **dentro** del binario.
//!
//! Es lo que convierte el despliegue en copiar un archivo, y tiene una
//! consecuencia de seguridad que no es accesoria: al servirse el SPA desde el
//! mismo origen que la API, la sesión es una cookie `HttpOnly` + `SameSite=Strict`.
//! No hay token en `localStorage` que un XSS pueda exfiltrar, ni CORS ni CSRF que
//! resolver. Un consultorio sin internet tampoco puede depender de un CDN: lo que
//! no esté dentro del binario, no existe.

use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

/// El resultado de `ng build`, copiado a `estaticos/` por `construir.sh`. La
/// carpeta lleva un `.gitkeep` para que el binario compile en una copia limpia
/// aunque nadie haya construido la interfaz todavía (el CI de Rust no la
/// construye, y corre en *debug*).
///
/// En *release* eso ya no puede ocurrir por descuido: `build.rs` rompe la
/// compilación si falta `estaticos/index.html`. El 404 de abajo queda como red
/// para el binario de *debug* y para el que se pidió a propósito con
/// `MEDICO_SIN_INTERFAZ=1`.
#[derive(Embed)]
#[folder = "estaticos/"]
struct Interfaz;

pub async fn servir(uri: Uri) -> Response {
    let ruta = uri.path().trim_start_matches('/');
    let ruta = if ruta.is_empty() { "index.html" } else { ruta };

    if let Some(archivo) = Interfaz::get(ruta) {
        let tipo = archivo.metadata.mimetype();
        return ([(header::CONTENT_TYPE, tipo)], archivo.data).into_response();
    }

    // Las rutas del enrutador del SPA (/pacientes, /citas…) las resuelve el
    // navegador: se le devuelve el index. Pero a un `/api/...` desconocido se le
    // responde 404, para que un error no llegue disfrazado de página.
    if ruta.starts_with("api/") {
        return (StatusCode::NOT_FOUND, "no existe").into_response();
    }

    match Interfaz::get("index.html") {
        Some(index) => ([(header::CONTENT_TYPE, "text/html")], index.data).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "La interfaz no está empaquetada en este binario. Construya el \
             artefacto completo con `./construir.sh` en la raíz del repositorio.",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El `index.html` empotrado **puede ejecutarse bajo `script-src 'self'`** y no
    /// habla con nadie de fuera.
    ///
    /// Cierra dos fallos que no se ven mirando la pantalla en el portátil:
    ///
    /// 1. Con `inlineCritical` encendido, Angular emite
    ///    `<link … media="print" onload="this.media='all'">`. Ese `onload` es
    ///    script en línea: la CSP no lo ejecuta, la hoja se queda en «print» y la
    ///    aplicación se ve sin estilos **sin un solo error**. Se apagó en
    ///    `angular.json` — una opción que un `ng update` puede revertir en
    ///    silencio.
    /// 2. El index cargaba Inter de `fonts.googleapis.com`. En una historia
    ///    clínica eso le contaba a un tercero quién usa el sistema y desde dónde;
    ///    y en un consultorio sin internet la tipografía no llegaba. Ahora viaja
    ///    dentro del paquete.
    #[test]
    fn el_index_empotrado_no_ejecuta_nada_en_linea_ni_llama_a_nadie() {
        let Some(index) = Interfaz::get("index.html") else {
            eprintln!("sin interfaz empaquetada: nada que comprobar");
            return;
        };
        let html = String::from_utf8_lossy(&index.data);

        for pista in [" onload=", " onclick=", " onerror=", " onchange=", " onsubmit="] {
            assert!(
                !html.contains(pista),
                "el index trae «{}», que `script-src 'self'` NO ejecuta. \
                 Casi seguro volvió `inlineCritical` en angular.json.",
                pista.trim()
            );
        }
        for trozo in html.split("<script").skip(1) {
            let etiqueta = trozo.split('>').next().unwrap_or("");
            assert!(etiqueta.contains("src="), "hay un <script> en línea en el index");
        }
        // Se mira el ATRIBUTO y no la cadena suelta, para que un comentario que
        // explique por qué no hay enlaces externos no haga fallar la prueba que
        // comprueba justo eso.
        for atributo in ["href=\"http", "src=\"http"] {
            assert!(
                !html.contains(atributo),
                "el index apunta fuera ({atributo}…): `default-src 'self'` lo bloquea, \
                 en una instalación sin internet no cargaría, y es un tercero \
                 enterándose de que se está usando una historia clínica"
            );
        }
    }
}
