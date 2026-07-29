//! Las rutas HTTP. Todo lo protegido pide `Autenticado`, así que una ruta sin
//! esa firma es, de un vistazo, una ruta pública — no hay que leerla entera para
//! saberlo.

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use medico_dominio::RolUsuario;
use serde::{Deserialize, Serialize};

use crate::auth::{autenticar, Autenticado, Estado};
use crate::{claves, sesion};
use medico_datos::institucion::{self, Institucion, InstitucionNueva};
use medico_datos::paciente::{self, Paciente, PacienteNuevo};
use medico_datos::usuario::{self, ResumenUsuario, Usuario, UsuarioNuevo};

pub fn router(estado: Estado) -> Router {
    Router::new()
        .route("/api/acceso", post(acceso))
        .route("/api/salir", post(salir))
        .route("/api/yo", get(yo))
        .route("/api/pacientes", get(listar_pacientes).post(crear_paciente))
        .route("/api/pacientes/{id}", put(actualizar_paciente))
        .route("/api/instituciones", get(listar_instituciones).post(crear_institucion))
        .route("/api/usuarios", get(listar_usuarios).post(crear_usuario))
        .with_state(estado)
}

/// Quién puede gestionar usuarios e instituciones: administración del sistema.
fn es_gestor(u: &Usuario) -> bool {
    matches!(u.rol(), Some(RolUsuario::Admin | RolUsuario::Superadmin))
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

#[derive(Serialize)]
struct Creado {
    id: i64,
}

/// Crea un paciente en la institución del usuario. 201 con su id; 403 si el
/// ámbito no puede crear (superadmin sin institución, o sin ámbito).
async fn crear_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(nuevo): Json<PacienteNuevo>,
) -> Response {
    match paciente::crear(&e.pool, u.ambito(), &nuevo).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Actualiza un paciente propio. 204 si se actualizó; **404 si no existe o no es
/// suyo** —nunca 403: un 403 confirmaría que el id existe en otra institución, y
/// eso ya enumera pacientes ajenos—.
async fn actualizar_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(datos): Json<PacienteNuevo>,
) -> StatusCode {
    match paciente::actualizar(&e.pool, u.ambito(), id, &datos).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Instituciones (las gestiona el superadmin) ------------------------------

async fn listar_instituciones(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<Vec<Institucion>>, StatusCode> {
    institucion::listar(&e.pool, u.ambito())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn crear_institucion(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(nueva): Json<InstitucionNueva>,
) -> Response {
    match institucion::crear(&e.pool, u.ambito(), &nueva).await {
        Ok(institucion::ResultadoCrear::Creada(id)) => {
            (StatusCode::CREATED, Json(Creado { id })).into_response()
        }
        Ok(institucion::ResultadoCrear::SinPermiso) => StatusCode::FORBIDDEN.into_response(),
        Ok(institucion::ResultadoCrear::NitEnUso) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// --- Usuarios (los gestiona admin o superadmin) ------------------------------

async fn listar_usuarios(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<Vec<ResumenUsuario>>, StatusCode> {
    if !es_gestor(&u) {
        return Err(StatusCode::FORBIDDEN);
    }
    usuario::listar(&e.pool, u.ambito())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize)]
struct CrearUsuario {
    nombre: String,
    email: String,
    clave: String,
    rol: String,
    institucion_id: Option<i64>,
}

async fn crear_usuario(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(req): Json<CrearUsuario>,
) -> Response {
    if !es_gestor(&u) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // El rol tiene que existir, y **solo un superadmin puede fabricar otro
    // superadmin**: si no, un admin se ascendería creando una cuenta.
    let Some(rol) = RolUsuario::desde_str(&req.rol) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if rol == RolUsuario::Superadmin && u.rol() != Some(RolUsuario::Superadmin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(hash) = claves::hashear(&req.clave) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let nuevo = UsuarioNuevo {
        nombre: req.nombre,
        email: req.email,
        password_hash: hash,
        rol: req.rol,
        institucion_id: req.institucion_id,
    };
    match usuario::crear(&e.pool, u.ambito(), &nuevo).await {
        Ok(usuario::ResultadoCrear::Creado(id)) => {
            (StatusCode::CREATED, Json(Creado { id })).into_response()
        }
        Ok(usuario::ResultadoCrear::SinPermiso) => StatusCode::FORBIDDEN.into_response(),
        Ok(usuario::ResultadoCrear::EmailEnUso) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
