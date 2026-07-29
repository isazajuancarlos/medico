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

/// El resultado de `ng build`, copiado a `estaticos/` al empaquetar. La carpeta
/// lleva un `.gitkeep` para que el binario compile en una copia limpia aunque
/// nadie haya construido la interfaz todavía (el CI de Rust no la construye).
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
            "La interfaz no está empaquetada en este binario. Construya con \
             `ng build` en frontend/ y copie frontend/dist/frontend/browser/ a \
             crates/medico-api/estaticos/.",
        )
            .into_response(),
    }
}
