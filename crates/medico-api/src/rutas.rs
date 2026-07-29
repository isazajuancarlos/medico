//! Las rutas HTTP. Todo lo protegido pide `Autenticado`, así que una ruta sin
//! esa firma es, de un vistazo, una ruta pública — no hay que leerla entera para
//! saberlo.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use medico_dominio::{Ambito, RolUsuario};
use serde::{Deserialize, Serialize};

use crate::auth::{autenticar, Autenticado, Estado};
use crate::{claves, sesion};
use medico_datos::catalogo::{self, Especialidad, Medicamento, UnidadDispensacion};
use medico_datos::cita::{self, CitaNueva, CitaVista};
use medico_datos::documento::{self, Documento, DocumentoEdicion, DocumentoNuevo, DocumentoVista};
use medico_datos::formula::{self, Formula, FormulaNueva};
use medico_datos::historia::{self, Historia, HistoriaNueva};
use medico_datos::imagen::{self, Imagen, ImagenNueva};
use medico_datos::institucion::{self, Institucion, InstitucionNueva};
use medico_datos::licencia;
use medico_datos::orden::{self, Orden, OrdenNueva};
use medico_datos::paciente::{self, Antecedentes, InfoPaciente, Paciente, PacienteNuevo};
use medico_datos::usuario::{self, ResumenUsuario, Usuario, UsuarioNuevo};

/// Tope de una subida de imagen. Una radiografía o un PDF de laboratorio caben
/// de sobra; por encima es casi seguro un error o un abuso.
const MAX_SUBIDA: usize = 25 * 1024 * 1024;

pub fn router(estado: Estado) -> Router {
    Router::new()
        .route("/api/acceso", post(acceso))
        .route("/api/salir", post(salir))
        .route("/api/yo", get(yo))
        .route("/api/pacientes", get(listar_pacientes).post(crear_paciente))
        .route("/api/pacientes/{id}", get(obtener_paciente).put(actualizar_paciente))
        .route("/api/pacientes/{id}/antecedentes", put(actualizar_antecedentes_paciente))
        .route("/api/pacientes/{id}/historias", get(listar_historias_paciente))
        .route("/api/historias", post(crear_historia))
        .route(
            "/api/historias/{id}",
            get(obtener_historia).put(actualizar_historia).delete(eliminar_historia),
        )
        .route("/api/historias/{id}/firmar", post(firmar_historia))
        .route("/api/historias/paciente/{id}/contexto", get(contexto_paciente))
        // Documentos clínicos: una sola familia de rutas para todos los tipos
        // (evolución, epicrisis, interconsulta, …); el tipo va como filtro/campo.
        .route("/api/documentos", get(listar_documentos).post(crear_documento))
        .route(
            "/api/documentos/{id}",
            get(obtener_documento).put(actualizar_documento).delete(eliminar_documento),
        )
        .route("/api/documentos/{id}/firmar", post(firmar_documento))
        .route("/api/historias/{id}/formulas", get(listar_formulas).post(crear_formula))
        .route("/api/formulas/{id}", put(actualizar_formula).delete(eliminar_formula))
        .route("/api/historias/{id}/ordenes", get(listar_ordenes).post(crear_orden))
        .route("/api/ordenes/{id}/estado", put(cambiar_estado_orden))
        .route("/api/ordenes/{id}", axum::routing::delete(eliminar_orden))
        .route(
            "/api/historias/{id}/imagenes",
            get(listar_imagenes)
                .post(subir_imagen)
                // El tope alto vale SOLO para esta ruta: las demás siguen con el
                // límite modesto de axum, que basta para JSON.
                .layer(DefaultBodyLimit::max(MAX_SUBIDA)),
        )
        .route("/api/imagenes/{id}/archivo", get(servir_imagen))
        .route("/api/imagenes/{id}", axum::routing::delete(eliminar_imagen))
        .route("/api/instituciones", get(listar_instituciones).post(crear_institucion))
        .route("/api/instituciones/{id}/licencia", put(fijar_licencia))
        .route("/api/usuarios", get(listar_usuarios).post(crear_usuario))
        .route("/api/medicos", get(listar_medicos))
        .route("/api/citas", get(listar_citas).post(crear_cita))
        .route("/api/citas/{id}/estado", put(cambiar_estado_cita))
        // Catálogos de referencia: los lee cualquier usuario autenticado.
        .route("/api/especialidades", get(listar_especialidades))
        .route("/api/medicamentos/buscar", get(buscar_medicamentos))
        .route("/api/unidades-dispensacion", get(listar_unidades_dispensacion))
        // Todo lo que no sea una ruta de API lo resuelve la interfaz embebida:
        // los archivos del build y, para las rutas del SPA, el index.html.
        .fallback(crate::estaticos::servir)
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
    email: String,
    rol: Option<RolUsuario>,
    institucion_id: Option<i64>,
}

/// Quién soy. La interfaz la usa para restaurar la sesión y pintar el menú.
async fn yo(Autenticado(u): Autenticado) -> Json<Yo> {
    let rol = u.rol();
    Json(Yo {
        id: u.id,
        nombre: u.nombre,
        email: u.email,
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

/// Un paciente por id, si es visible para el ámbito. **404 si no existe o no es
/// suyo** —nunca 403: un 403 confirmaría que el id existe en otra institución—.
async fn obtener_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Result<Json<Paciente>, StatusCode> {
    match paciente::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => Ok(Json(p)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
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

/// Actualiza los antecedentes (JSON) de un paciente propio. 204 si se actualizó;
/// **404 si no existe o no es suyo** —nunca 403, la misma regla que el resto—.
/// Solo se escriben los bloques presentes: guardar unos no borra los otros.
async fn actualizar_antecedentes_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(ant): Json<Antecedentes>,
) -> StatusCode {
    match paciente::actualizar_antecedentes(&e.pool, u.ambito(), id, &ant).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Historia clínica --------------------------------------------------------

async fn crear_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(h): Json<HistoriaNueva>,
) -> Response {
    match historia::crear(&e.pool, u.ambito(), &h).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn obtener_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Result<Json<Historia>, StatusCode> {
    match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => Ok(Json(h)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Edita una historia de borrador. 200 con la historia ya actualizada; **404 si
/// no existe o es ajena; 409 si está firmada** —cerrada, no editable—.
async fn actualizar_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(h): Json<HistoriaNueva>,
) -> Response {
    match historia::actualizar(&e.pool, u.ambito(), id, &h).await {
        Ok(historia::Resultado::Hecho) => match historia::obtener(&e.pool, u.ambito(), id).await {
            Ok(Some(actualizada)) => (StatusCode::OK, Json(actualizada)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(historia::Resultado::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(historia::Resultado::Firmada) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Firma una historia, que queda cerrada. 204 si se firmó; 404 si no existe, es
/// ajena o ya estaba firmada.
async fn firmar_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match historia::firmar(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Borra una historia de borrador. 204 si se borró; **404 si no existe o es
/// ajena; 409 si está firmada** —un documento legal cerrado no se elimina—.
async fn eliminar_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match historia::eliminar(&e.pool, u.ambito(), id).await {
        Ok(historia::Resultado::Hecho) => StatusCode::NO_CONTENT,
        Ok(historia::Resultado::NoExiste) => StatusCode::NOT_FOUND,
        Ok(historia::Resultado::Firmada) => StatusCode::CONFLICT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// El contexto del paciente para el encabezado del formulario: sus datos ya
/// compuestos y el médico que atiende (el usuario autenticado). **404 si el
/// paciente no existe o no es del ámbito**.
async fn contexto_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(paciente_id): Path<i64>,
) -> Result<Json<Contexto>, StatusCode> {
    match paciente::info(&e.pool, u.ambito(), paciente_id).await {
        Ok(Some(paciente)) => Ok(Json(Contexto {
            paciente,
            medico: InfoMedico {
                id: u.id,
                nombre_completo: u.nombre.clone(),
                // La especialidad y el registro médico del profesional aún no
                // viven en `usuario` (llegan con el maestro de talento humano);
                // hasta entonces, ausentes y no inventados.
                especialidad: None,
                registro_medico: None,
            },
        })),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
struct Contexto {
    paciente: InfoPaciente,
    medico: InfoMedico,
}

#[derive(Serialize)]
struct InfoMedico {
    id: i64,
    nombre_completo: String,
    especialidad: Option<String>,
    registro_medico: Option<String>,
}

async fn listar_historias_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(paciente_id): Path<i64>,
) -> Result<Json<Vec<Historia>>, StatusCode> {
    historia::listar_por_paciente(&e.pool, u.ambito(), paciente_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// --- Documentos clínicos (evolución, epicrisis, interconsulta, …) ------------

#[derive(Deserialize)]
struct FiltroDocumentos {
    paciente_id: Option<i64>,
    tipo: Option<String>,
}

/// La worklist de documentos visibles para el ámbito, con los nombres unidos.
/// Ambos filtros son opcionales: por tipo (una worklist de evoluciones) y por
/// paciente (los documentos de un paciente). Devuelve la vista ligera, sin
/// `contenido`; el editor pide el documento completo con `GET /{id}`.
async fn listar_documentos(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroDocumentos>,
) -> Result<Json<Vec<DocumentoVista>>, StatusCode> {
    documento::listar_ambito(&e.pool, u.ambito(), f.paciente_id, f.tipo.as_deref())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Crea un documento. El autor es el usuario autenticado —no un campo del
/// cliente—. 201 con su id; 403 si el paciente no es del ámbito.
async fn crear_documento(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(d): Json<DocumentoNuevo>,
) -> Response {
    match documento::crear(&e.pool, u.ambito(), u.id, &d).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn obtener_documento(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Result<Json<Documento>, StatusCode> {
    match documento::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(d)) => Ok(Json(d)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Edita un documento de borrador. 200 con el documento ya actualizado; **404 si
/// no existe o es ajeno; 409 si está firmado**.
async fn actualizar_documento(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(d): Json<DocumentoEdicion>,
) -> Response {
    match documento::actualizar(&e.pool, u.ambito(), id, &d).await {
        Ok(documento::Resultado::Hecho) => match documento::obtener(&e.pool, u.ambito(), id).await {
            Ok(Some(actualizado)) => (StatusCode::OK, Json(actualizado)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(documento::Resultado::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(documento::Resultado::Firmada) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn firmar_documento(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match documento::firmar(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn eliminar_documento(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match documento::eliminar(&e.pool, u.ambito(), id).await {
        Ok(documento::Resultado::Hecho) => StatusCode::NO_CONTENT,
        Ok(documento::Resultado::NoExiste) => StatusCode::NOT_FOUND,
        Ok(documento::Resultado::Firmada) => StatusCode::CONFLICT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Fórmulas (prescripciones de una historia) -------------------------------

async fn listar_formulas(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(historia_id): Path<i64>,
) -> Result<Json<Vec<Formula>>, StatusCode> {
    formula::listar_por_historia(&e.pool, u.ambito(), historia_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn crear_formula(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(historia_id): Path<i64>,
    Json(f): Json<FormulaNueva>,
) -> Response {
    match formula::crear(&e.pool, u.ambito(), historia_id, &f).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn actualizar_formula(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(f): Json<FormulaNueva>,
) -> StatusCode {
    match formula::actualizar(&e.pool, u.ambito(), id, &f).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn eliminar_formula(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match formula::eliminar(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Órdenes médicas ---------------------------------------------------------

async fn listar_ordenes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(historia_id): Path<i64>,
) -> Result<Json<Vec<Orden>>, StatusCode> {
    orden::listar_por_historia(&e.pool, u.ambito(), historia_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn crear_orden(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(historia_id): Path<i64>,
    Json(o): Json<OrdenNueva>,
) -> Response {
    match orden::crear(&e.pool, u.ambito(), historia_id, &o).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cambiar_estado_orden(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(req): Json<CambioEstado>,
) -> StatusCode {
    match orden::cambiar_estado(&e.pool, u.ambito(), id, &req.estado).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn eliminar_orden(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match orden::eliminar(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Imágenes diagnósticas ---------------------------------------------------

async fn listar_imagenes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(historia_id): Path<i64>,
) -> Result<Json<Vec<Imagen>>, StatusCode> {
    imagen::listar_por_historia(&e.pool, u.ambito(), historia_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Sube un archivo a una historia. Espera un `multipart/form-data` con el campo
/// `archivo` (el binario) y, opcional, `descripcion`. El mime y el tamaño los
/// mide el servidor sobre los bytes recibidos; el nombre y el tipo que declara
/// el cliente no deciden dónde se guarda —la ruta la fija el id de la fila—.
async fn subir_imagen(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(historia_id): Path<i64>,
    mut multipart: Multipart,
) -> Response {
    let mut archivo: Option<(String, String, Vec<u8>)> = None; // nombre, mime, bytes
    let mut descripcion = String::new();
    loop {
        match multipart.next_field().await {
            Ok(Some(campo)) => match campo.name() {
                Some("archivo") => {
                    let nombre = campo.file_name().unwrap_or("archivo").to_string();
                    let mime = campo
                        .content_type()
                        .unwrap_or("application/octet-stream")
                        .to_string();
                    match campo.bytes().await {
                        Ok(b) => archivo = Some((nombre, mime, b.to_vec())),
                        // El único fallo esperable al leer el campo es que exceda
                        // el tope del `DefaultBodyLimit`.
                        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
                    }
                }
                Some("descripcion") => descripcion = campo.text().await.unwrap_or_default(),
                _ => {}
            },
            Ok(None) => break,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    }

    let Some((nombre, mime, bytes)) = archivo else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if bytes.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    // Solo imágenes y PDF: es una imagen diagnóstica, no un buzón de archivos.
    if !(mime.starts_with("image/") || mime == "application/pdf") {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }

    let meta = ImagenNueva {
        nombre_original: nombre,
        tipo_mime: mime,
        tamano_bytes: bytes.len() as i64,
        descripcion,
    };
    let img = match imagen::registrar(&e.pool, u.ambito(), historia_id, &meta).await {
        Ok(Some(img)) => img,
        Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    // Los bytes van al disco DESPUÉS de la fila: si la escritura falla, se
    // deshace la fila para no dejar una referencia a un archivo que no existe.
    let destino = e.dir_imagenes.join(&img.ruta_archivo);
    if tokio::fs::write(&destino, &bytes).await.is_err() {
        let _ = imagen::eliminar(&e.pool, u.ambito(), img.id).await;
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (StatusCode::CREATED, Json(Creado { id: img.id })).into_response()
}

/// Devuelve los bytes de una imagen visible, con su mime. **404 si no existe o
/// no es del ámbito** —nunca 403—, la misma regla que el resto.
async fn servir_imagen(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Response {
    let img = match imagen::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(img)) => img,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let bytes = match tokio::fs::read(e.dir_imagenes.join(&img.ruta_archivo)).await {
        Ok(b) => b,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    // El nombre para descargar se limpia: un nombre de archivo del usuario podría
    // llevar comillas o saltos de línea y romper la cabecera.
    let disp = format!("inline; filename=\"{}\"", nombre_seguro(&img.nombre_original));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, img.tipo_mime),
            (header::CONTENT_DISPOSITION, disp),
        ],
        bytes,
    )
        .into_response()
}

async fn eliminar_imagen(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> StatusCode {
    match imagen::eliminar(&e.pool, u.ambito(), id).await {
        Ok(Some(ruta)) => {
            // La fila ya no está; borrar el archivo es limpieza. Si falla (por
            // ejemplo, ya no estaba), no cambia el resultado para el cliente.
            let _ = tokio::fs::remove_file(e.dir_imagenes.join(&ruta)).await;
            StatusCode::NO_CONTENT
        }
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Deja solo caracteres seguros para un nombre de archivo en una cabecera: el
/// resto pasa a `_`. No se usa para la ruta en disco (esa la fija el id), solo
/// para la etiqueta de descarga.
fn nombre_seguro(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_') { c } else { '_' })
        .collect()
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
    // Gate comercial: limita SOLO la expansión, y solo para un admin de
    // institución. El superadmin (onboarding) queda exento; el login y los
    // listados nunca pasan por aquí. Una licencia vencida no ciega nada: solo
    // impide agregar un usuario más.
    if let Ambito::Institucion(id) = u.ambito() {
        let hoy = chrono::Local::now().date_naive().to_string();
        match licencia::puerta_para_usuario(&e.pool, id, &hoy).await {
            Ok(licencia::Puerta::Abierta) => {}
            Ok(bloqueo) => {
                return (StatusCode::PAYMENT_REQUIRED, Json(Bloqueo { motivo: motivo(bloqueo) }))
                    .into_response();
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
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

/// Por qué se bloqueó una expansión: un código que la interfaz traduce a un
/// aviso accionable («renueve el plan», «amplíe el cupo»), no un 403 mudo.
#[derive(Serialize)]
struct Bloqueo {
    motivo: &'static str,
}

fn motivo(p: licencia::Puerta) -> &'static str {
    match p {
        licencia::Puerta::SinLicencia => "sin_licencia",
        licencia::Puerta::Vencida => "licencia_vencida",
        licencia::Puerta::CupoLleno => "cupo_lleno",
        licencia::Puerta::Abierta => "abierta",
    }
}

#[derive(Deserialize)]
struct FijarLicencia {
    max_usuarios: i64,
    vence: Option<String>,
}

/// Fija o renueva la licencia de una institución. **Solo el superadmin**: es
/// quien vende el plan. 204 al fijarla; 403 a cualquier otro.
async fn fijar_licencia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(req): Json<FijarLicencia>,
) -> StatusCode {
    if u.rol() != Some(RolUsuario::Superadmin) {
        return StatusCode::FORBIDDEN;
    }
    match licencia::fijar(&e.pool, id, req.max_usuarios, req.vence.as_deref()).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Los médicos de la institución, para poblar el selector de agendar. Cualquier
/// usuario autenticado puede pedirla (agendar no es una acción de gestión).
async fn listar_medicos(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<Vec<usuario::MedicoResumen>>, StatusCode> {
    usuario::medicos(&e.pool, u.ambito())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// --- Citas -------------------------------------------------------------------

#[derive(Deserialize)]
struct FiltroCitas {
    fecha: Option<String>,
}

async fn listar_citas(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroCitas>,
) -> Result<Json<Vec<CitaVista>>, StatusCode> {
    cita::listar(&e.pool, u.ambito(), f.fecha.as_deref())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Agenda una cita. 201 con su id; 403 si el ámbito no puede agendar o si el
/// paciente no es de su institución.
async fn crear_cita(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(nueva): Json<CitaNueva>,
) -> Response {
    match cita::crear(&e.pool, u.ambito(), &nueva).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct CambioEstado {
    estado: String,
}

async fn cambiar_estado_cita(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(req): Json<CambioEstado>,
) -> StatusCode {
    match cita::cambiar_estado(&e.pool, u.ambito(), id, &req.estado).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Catálogos de referencia -------------------------------------------------
// Datos compartidos por todas las instituciones: no llevan ámbito. Basta con
// estar autenticado (la firma `Autenticado`); no se filtra por institución
// porque una especialidad o un medicamento son los mismos para todas.

async fn listar_especialidades(
    State(e): State<Estado>,
    _: Autenticado,
) -> Result<Json<Vec<Especialidad>>, StatusCode> {
    catalogo::especialidades(&e.pool)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize)]
struct BusquedaMedicamento {
    #[serde(default)]
    q: String,
}

async fn buscar_medicamentos(
    State(e): State<Estado>,
    _: Autenticado,
    Query(b): Query<BusquedaMedicamento>,
) -> Result<Json<Vec<Medicamento>>, StatusCode> {
    catalogo::buscar_medicamentos(&e.pool, &b.q)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn listar_unidades_dispensacion(
    State(e): State<Estado>,
    _: Autenticado,
) -> Result<Json<Vec<UnidadDispensacion>>, StatusCode> {
    catalogo::unidades_dispensacion(&e.pool)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
