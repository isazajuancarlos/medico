//! Las rutas HTTP. Todo lo protegido pide `Autenticado`, así que una ruta sin
//! esa firma es, de un vistazo, una ruta pública — no hay que leerla entera para
//! saberlo.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use medico_dominio::RolUsuario;
use serde::{Deserialize, Serialize};

use crate::auth::{autenticar, Autenticado, Estado};
use crate::sesion;
use medico_datos::paciente::{self, Paciente};

pub fn router(estado: Estado) -> Router {
    Router::new()
        .route("/api/acceso", post(acceso))
        .route("/api/salir", post(salir))
        .route("/api/yo", get(yo))
        .route("/api/pacientes", get(listar_pacientes))
        .with_state(estado)
}

#[derive(Serialize)]
struct Ok2 {
    ok: bool,
}

#[derive(Deserialize)]
struct Credenciales {
    email: String,
    clave: String,
}

/// Login. 200 con la cookie de sesión, o 401. Un solo mensaje para todos los
/// fallos: no se dice si el correo existe.
async fn acceso(State(e): State<Estado>, Json(cred): Json<Credenciales>) -> Response {
    match autenticar(&e.pool, &e.secreto, e.minutos, &cred.email, &cred.clave).await {
        Some(cookie) => (
            StatusCode::OK,
            [(header::SET_COOKIE, sesion::cabecera_abrir(&cookie, e.minutos))],
            Json(Ok2 { ok: true }),
        )
            .into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// Cierra la sesión borrando la cookie. No necesita estar autenticado: pedir
/// salir sin sesión no es un error.
async fn salir() -> Response {
    (
        StatusCode::OK,
        [(header::SET_COOKIE, sesion::cabecera_cerrar())],
        Json(Ok2 { ok: true }),
    )
        .into_response()
}

#[derive(Serialize)]
struct Yo {
    id: i64,
    nombre: String,
    rol: Option<RolUsuario>,
    institucion_id: Option<i64>,
}

/// Quién soy. La interfaz la usa para pintar el menú según el rol.
async fn yo(Autenticado(u): Autenticado) -> Json<Yo> {
    let rol = u.rol();
    Json(Yo {
        id: u.id,
        nombre: u.nombre,
        rol,
        institucion_id: u.institucion_id,
    })
}

/// Pacientes, acotados al ámbito del usuario. El `ambito()` sale del rol y la
/// institución del usuario autenticado: nunca es un parámetro que el cliente
/// pueda mandar.
async fn listar_pacientes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<Vec<Paciente>>, StatusCode> {
    paciente::listar(&e.pool, u.ambito())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
