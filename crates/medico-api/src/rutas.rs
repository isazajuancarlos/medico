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
use medico_datos::acceso;
use medico_datos::catalogo::{self, Especialidad, Medicamento, UnidadDispensacion};
use medico_datos::cita::{self, CitaNueva, CitaVista};
use medico_datos::documento::{self, Documento, DocumentoEdicion, DocumentoNuevo, DocumentoVista};
use medico_datos::formula::{self, Formula, FormulaNueva};
use medico_datos::urgencias::{self, TriageNuevo};
use medico_datos::hospitalizacion::{self, Egreso, Ingreso};
use medico_datos::nutricion::{self, PlanCrear, PlanEditar, ValoracionCrear, ValoracionEditar};
use medico_datos::rehabilitacion::{self, SesionCrear, SesionEditar};
use medico_datos::incapacidad::{self, Filtros};
use medico_datos::referencia::{self, ContrarreferenciaCrear, Recepcion};
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
        .route("/api/pacientes/{id}/accesos", get(accesos_paciente))
        .route("/api/acceso/verificar", get(verificar_acceso))
        .route("/api/historias", post(crear_historia))
        .route(
            "/api/historias/{id}",
            get(obtener_historia).put(actualizar_historia).delete(eliminar_historia),
        )
        .route("/api/historias/{id}/firmar", post(firmar_historia))
        .route("/api/historias/{id}/verificar-firma", get(verificar_firma_historia))
        .route("/api/historias/paciente/{id}/contexto", get(contexto_paciente))
        // Documentos clínicos: una sola familia de rutas para todos los tipos
        // (evolución, epicrisis, interconsulta, …); el tipo va como filtro/campo.
        .route("/api/documentos", get(listar_documentos).post(crear_documento))
        .route(
            "/api/documentos/{id}",
            get(obtener_documento).put(actualizar_documento).delete(eliminar_documento),
        )
        .route("/api/documentos/{id}/firmar", post(firmar_documento))
        .route("/api/urgencias/cola", get(urgencias_cola))
        .route("/api/urgencias/triage", post(urgencias_triage))
        .route("/api/urgencias/{id}/egreso", put(urgencias_egreso))
        .route("/api/hospitalizacion", get(hosp_listar))
        .route("/api/hospitalizacion/camas", get(hosp_camas))
        .route("/api/hospitalizacion/medicos", get(hosp_medicos))
        .route("/api/hospitalizacion/ingresar", post(hosp_ingresar))
        .route("/api/hospitalizacion/{id}/egreso", put(hosp_egreso))
        .route("/api/nutricion/valoraciones", get(nutri_listar).post(nutri_crear))
        .route(
            "/api/nutricion/valoraciones/{id}",
            get(nutri_obtener).put(nutri_actualizar).delete(nutri_eliminar),
        )
        .route(
            "/api/nutricion/valoraciones/{id}/plan",
            get(nutri_plan).post(nutri_crear_plan),
        )
        .route("/api/nutricion/planes/{id}", put(nutri_actualizar_plan))
        .route("/api/rehabilitacion/planes", get(rehab_listar).post(rehab_crear))
        .route(
            "/api/rehabilitacion/planes/{id}",
            get(rehab_obtener).put(rehab_actualizar),
        )
        .route(
            "/api/rehabilitacion/planes/{id}/sesiones",
            get(rehab_sesiones).post(rehab_crear_sesion),
        )
        .route("/api/rehabilitacion/sesiones/{id}", put(rehab_actualizar_sesion))
        .route("/api/incapacidades", get(incap_listar))
        .route("/api/incapacidades/resumen", get(incap_resumen))
        .route("/api/incapacidades/{historia_id}", get(incap_detalle))
        .route("/api/remisiones", get(rem_listar))
        .route("/api/remisiones/resumen", get(rem_resumen))
        .route("/api/remisiones/{id}", get(rem_obtener))
        .route("/api/remisiones/{id}/estado", put(rem_estado))
        .route("/api/contrarreferencia", get(cr_listar).post(cr_crear))
        .route("/api/contrarreferencia/remisiones-abiertas", get(cr_rem_abiertas))
        .route("/api/contrarreferencia/{id}", get(cr_obtener))
        .route("/api/contrarreferencia/{id}/recibir", post(cr_recibir))
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

/// El rastro de acceso de un paciente (derecho del titular, Ley 1581). Restringido
/// al ámbito: un paciente ajeno no existe (404), igual que en el resto.
async fn accesos_paciente(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    match paciente::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(_)) => {}
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
    match acceso::por_paciente(&e.pool, u.ambito(), id).await {
        Ok(eventos) => Ok(Json(
            eventos.iter().filter_map(|s| serde_json::from_str(s).ok()).collect(),
        )),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Verifica la integridad de TODA la bitácora (encadenamiento + firmas). Solo
/// superadmin: es un control global, no de un recurso. Devuelve el veredicto.
async fn verificar_acceso(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if u.ambito() != Ambito::Superadmin {
        return Err(StatusCode::FORBIDDEN);
    }
    match acceso::verificar(&e.pool, e.clave_verificacion.as_deref()).await {
        Ok(veredicto) => Ok(Json(serde_json::json!({ "veredicto": format!("{veredicto:?}") }))),
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

/// Registra un acceso a dato de paciente en la bitácora (Ley 1581), **best-effort**:
/// si el registro falla NO se bloquea la atención — se grita en el log. El registro
/// es casi infalible (BEGIN IMMEDIATE + reintentos); un fallo es raro y queda
/// avisado. Decisión de Juan 2026-07-29: la seguridad del paciente por encima de la
/// formalidad del rastro.
async fn registrar_acceso(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    accion: &str,
    recurso: &str,
    recurso_id: i64,
    paciente_id: i64,
) {
    let evento = acceso::Evento {
        usuario_id: u.id,
        rol: u.rol.clone(),
        institucion_id: u.institucion_id,
        accion: accion.to_string(),
        recurso: recurso.to_string(),
        recurso_id: Some(recurso_id),
        paciente_id: Some(paciente_id),
        alcance: None,
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    if let Err(err) = acceso::registrar(&e.pool, &evento, e.clave_firma.as_deref()).await {
        eprintln!("⚠️  bitácora de acceso: no se registró {recurso} {recurso_id} (paciente {paciente_id}): {err}");
    }
}

async fn obtener_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Result<Json<Historia>, StatusCode> {
    match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => {
            registrar_acceso(&e, &u, "ver", "historia", id, h.paciente_id).await;
            Ok(Json(h))
        }
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
    match historia::firmar(&e.pool, u.ambito(), id, e.clave_firma.as_deref()).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Serialize)]
struct EstadoFirma {
    estado: &'static str,
}

/// Verifica la firma criptográfica de una historia contra su contenido actual.
/// `valida` / `alterada` / `invalida` / `sin_firma` (cerrada sin clave) /
/// `sin_clave` (el servidor no tiene clave pública configurada). 404 si la
/// historia no existe o no es del ámbito.
async fn verificar_firma_historia(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Response {
    let Some(vk) = e.clave_verificacion.as_deref() else {
        return (StatusCode::OK, Json(EstadoFirma { estado: "sin_clave" })).into_response();
    };
    match historia::verificar(&e.pool, u.ambito(), id, vk).await {
        Ok(Some(v)) => {
            let estado = match v {
                guaca::firma::Verificacion::Valida => "valida",
                guaca::firma::Verificacion::Alterada => "alterada",
                guaca::firma::Verificacion::Invalida => "invalida",
            };
            (StatusCode::OK, Json(EstadoFirma { estado })).into_response()
        }
        // La historia existe (se pudo consultar) pero no tiene firma, o no existe.
        Ok(None) => match historia::obtener(&e.pool, u.ambito(), id).await {
            Ok(Some(_)) => (StatusCode::OK, Json(EstadoFirma { estado: "sin_firma" })).into_response(),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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

// --- Urgencias ---------------------------------------------------------------

async fn urgencias_cola(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match urgencias::cola(&e.pool, u.ambito()).await {
        Ok(cola) => {
            // Se enriquece con la etiqueta humana del triage; el orden ya viene por
            // prioridad desde la consulta.
            let filas: Vec<serde_json::Value> = cola
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "paciente_id": a.paciente_id,
                        "triage": a.triage,
                        "triage_label": urgencias::etiqueta_triage(&a.triage),
                        "motivo_consulta": a.motivo_consulta,
                        "fecha_ingreso": a.fecha_ingreso,
                        "estado": "EN_ESPERA",
                    })
                })
                .collect();
            Json(filas).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn urgencias_triage(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(n): Json<TriageNuevo>,
) -> Response {
    match urgencias::registrar(&e.pool, u.ambito(), u.id, &n).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        // Paciente ajeno o triage inválido: no se distingue de «no se puede».
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct EgresoBody {
    destino: String,
    observaciones: Option<String>,
}

async fn urgencias_egreso(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(b): Json<EgresoBody>,
) -> StatusCode {
    match urgencias::egreso(&e.pool, u.ambito(), id, &b.destino, b.observaciones.as_deref()).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND, // no existe, ajena, cerrada o destino inválido
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Hospitalización ---------------------------------------------------------

#[derive(serde::Deserialize)]
struct FiltroHosp {
    estado: Option<String>,
}

#[derive(serde::Deserialize)]
struct FiltroCamas {
    estado: Option<String>,
    servicio: Option<String>,
}

async fn hosp_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroHosp>,
) -> Response {
    match hospitalizacion::listar(&e.pool, u.ambito(), f.estado.as_deref()).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn hosp_camas(
    State(e): State<Estado>,
    Autenticado(_u): Autenticado,
    Query(f): Query<FiltroCamas>,
) -> Response {
    match hospitalizacion::listar_camas(&e.pool, f.estado.as_deref(), f.servicio.as_deref()).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn hosp_medicos(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match hospitalizacion::listar_medicos(&e.pool, u.ambito()).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn hosp_ingresar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(i): Json<Ingreso>,
) -> Response {
    match hospitalizacion::ingresar(&e.pool, u.ambito(), &i).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        // Cama no disponible, paciente ajeno, vía inválida o FK inexistente.
        Ok(None) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "detail": "No se pudo ingresar: cama no disponible o datos inválidos" })),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn hosp_egreso(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(eg): Json<Egreso>,
) -> StatusCode {
    match hospitalizacion::egreso(&e.pool, u.ambito(), id, &eg).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Nutrición ---------------------------------------------------------------

#[derive(serde::Deserialize)]
struct FiltroNutri {
    paciente_id: Option<i64>,
}

async fn nutri_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroNutri>,
) -> Response {
    match nutricion::listar_valoraciones(&e.pool, u.ambito(), f.paciente_id).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn nutri_obtener(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match nutricion::obtener_valoracion(&e.pool, u.ambito(), id).await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn nutri_crear(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(v): Json<ValoracionCrear>,
) -> Response {
    match nutricion::crear_valoracion(&e.pool, u.ambito(), u.id, &v).await {
        Ok(Some(id)) => match nutricion::obtener_valoracion(&e.pool, u.ambito(), id).await {
            Ok(Some(val)) => (StatusCode::CREATED, Json(val)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn nutri_actualizar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(v): Json<ValoracionEditar>,
) -> Response {
    match nutricion::actualizar_valoracion(&e.pool, u.ambito(), id, &v).await {
        Ok(true) => match nutricion::obtener_valoracion(&e.pool, u.ambito(), id).await {
            Ok(Some(val)) => Json(val).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn nutri_eliminar(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> StatusCode {
    match nutricion::eliminar_valoracion(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn nutri_plan(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match nutricion::obtener_plan(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn nutri_crear_plan(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(p): Json<PlanCrear>,
) -> Response {
    match nutricion::crear_plan(&e.pool, u.ambito(), id, u.id, &p).await {
        Ok(Some(_)) => match nutricion::obtener_plan(&e.pool, u.ambito(), id).await {
            Ok(Some(plan)) => (StatusCode::CREATED, Json(plan)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn nutri_actualizar_plan(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(p): Json<PlanEditar>,
) -> StatusCode {
    match nutricion::actualizar_plan(&e.pool, u.ambito(), id, &p).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Rehabilitación ----------------------------------------------------------

async fn rehab_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroNutri>,
) -> Response {
    match rehabilitacion::listar_planes(&e.pool, u.ambito(), f.paciente_id).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rehab_obtener(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match rehabilitacion::obtener_plan(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rehab_crear(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(p): Json<rehabilitacion::PlanCrear>,
) -> Response {
    match rehabilitacion::crear_plan(&e.pool, u.ambito(), u.id, &p).await {
        Ok(Some(id)) => match rehabilitacion::obtener_plan(&e.pool, u.ambito(), id).await {
            Ok(Some(plan)) => (StatusCode::CREATED, Json(plan)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rehab_actualizar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(p): Json<rehabilitacion::PlanEditar>,
) -> Response {
    match rehabilitacion::actualizar_plan(&e.pool, u.ambito(), id, &p).await {
        Ok(true) => match rehabilitacion::obtener_plan(&e.pool, u.ambito(), id).await {
            Ok(Some(plan)) => Json(plan).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rehab_sesiones(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match rehabilitacion::listar_sesiones(&e.pool, u.ambito(), id).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rehab_crear_sesion(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(s): Json<SesionCrear>,
) -> Response {
    match rehabilitacion::crear_sesion(&e.pool, u.ambito(), id, u.id, &s).await {
        Ok(Some(sid)) => match rehabilitacion::obtener_sesion(&e.pool, u.ambito(), sid).await {
            Ok(Some(ses)) => (StatusCode::CREATED, Json(ses)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rehab_actualizar_sesion(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(s): Json<SesionEditar>,
) -> Response {
    match rehabilitacion::actualizar_sesion(&e.pool, u.ambito(), id, &s).await {
        Ok(true) => match rehabilitacion::obtener_sesion(&e.pool, u.ambito(), id).await {
            Ok(Some(ses)) => Json(ses).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// --- Incapacidades (solo lectura sobre historia) -----------------------------

#[derive(serde::Deserialize)]
struct FiltroIncap {
    desde: Option<String>,
    hasta: Option<String>,
    medico_id: Option<i64>,
    buscar: Option<String>,
}

impl FiltroIncap {
    fn como_filtros(&self) -> Filtros<'_> {
        Filtros {
            desde: self.desde.as_deref(),
            hasta: self.hasta.as_deref(),
            medico_id: self.medico_id,
            buscar: self.buscar.as_deref(),
        }
    }
}

async fn incap_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroIncap>,
) -> Response {
    match incapacidad::listar(&e.pool, u.ambito(), &f.como_filtros()).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn incap_resumen(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroIncap>,
) -> Response {
    match incapacidad::resumen(&e.pool, u.ambito(), &f.como_filtros()).await {
        Ok(r) => Json(r).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn incap_detalle(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match incapacidad::detalle(&e.pool, u.ambito(), id).await {
        Ok(Some(d)) => Json(d).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// --- Remisiones y contrarreferencia ------------------------------------------

#[derive(serde::Deserialize)]
struct FiltroRem {
    estado: Option<String>,
    desde: Option<String>,
    hasta: Option<String>,
    buscar: Option<String>,
}

async fn rem_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroRem>,
) -> Response {
    match referencia::remisiones_listar(
        &e.pool, u.ambito(), f.estado.as_deref(), f.desde.as_deref(), f.hasta.as_deref(), f.buscar.as_deref(),
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rem_resumen(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroRem>,
) -> Response {
    match referencia::remisiones_resumen(&e.pool, u.ambito(), f.desde.as_deref(), f.hasta.as_deref(), f.buscar.as_deref()).await {
        Ok(r) => Json(r).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn rem_obtener(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match referencia::remision_obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(r)) => Json(r).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct EstadoBody {
    estado: String,
}

async fn rem_estado(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
    Json(b): Json<EstadoBody>,
) -> Response {
    match referencia::remision_cambiar_estado(&e.pool, u.ambito(), id, &b.estado).await {
        Ok(Some(r)) => Json(r).into_response(),
        // No existe / ajena / transición no válida desde el estado actual.
        Ok(None) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "detail": "No se pudo cambiar el estado: remisión no encontrada o transición no válida" })),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct FiltroCr {
    solo_pendientes: Option<bool>,
}

async fn cr_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroCr>,
) -> Response {
    match referencia::contrarref_listar(&e.pool, u.ambito(), f.solo_pendientes.unwrap_or(false)).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cr_rem_abiertas(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match referencia::remisiones_abiertas(&e.pool, u.ambito()).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cr_crear(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Json(c): Json<ContrarreferenciaCrear>,
) -> Response {
    match referencia::contrarref_crear(&e.pool, u.ambito(), u.id, u.institucion_id, &c).await {
        Ok(Some(id)) => match referencia::contrarref_obtener(&e.pool, u.ambito(), id).await {
            Ok(Some(cr)) => (StatusCode::CREATED, Json(cr)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        // Paciente ajeno o remisión que no cuadra.
        Ok(None) => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cr_obtener(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match referencia::contrarref_obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(cr)) => Json(cr).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cr_recibir(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match referencia::contrarref_recibir(&e.pool, u.ambito(), id, u.id).await {
        Ok(Recepcion::Recibida) => match referencia::contrarref_obtener(&e.pool, u.ambito(), id).await {
            Ok(Some(cr)) => Json(cr).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(Recepcion::YaRecibida) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "detail": "Esta contrarreferencia ya se registró como recibida. El primer acuse es el que cuenta." })),
        )
            .into_response(),
        Ok(Recepcion::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn obtener_documento(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Result<Json<Documento>, StatusCode> {
    match documento::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(d)) => {
            registrar_acceso(&e, &u, "ver", "documento", id, d.paciente_id).await;
            Ok(Json(d))
        }
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

    // `tamano_bytes` es el tamaño del ORIGINAL (lo que ve el usuario), no del
    // blob cifrado. Con clave de reposo el archivo va cifrado al disco.
    let meta = ImagenNueva {
        nombre_original: nombre,
        tipo_mime: mime,
        tamano_bytes: bytes.len() as i64,
        descripcion,
    };
    let (para_disco, cifrada) = match e.clave_reposo.as_deref() {
        Some(frase) => (guaca::reposo::cifrar(&bytes, frase), true),
        None => (bytes, false),
    };
    let img = match imagen::registrar(&e.pool, u.ambito(), historia_id, &meta, cifrada).await {
        Ok(Some(img)) => img,
        Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    // Los bytes van al disco DESPUÉS de la fila: si la escritura falla, se
    // deshace la fila para no dejar una referencia a un archivo que no existe.
    let destino = e.dir_imagenes.join(&img.ruta_archivo);
    if tokio::fs::write(&destino, &para_disco).await.is_err() {
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
    // Bitácora (Ley 1581): acceso a imagen diagnóstica. El paciente se resuelve
    // por la historia de la imagen; si no se puede resolver NO se inventa un id
    // (directiva 20): se avisa y se omite, sin bloquear la entrega.
    match historia::obtener(&e.pool, u.ambito(), img.historia_id).await {
        Ok(Some(h)) => registrar_acceso(&e, &u, "ver", "imagen", id, h.paciente_id).await,
        _ => eprintln!(
            "⚠️  bitácora de acceso: no se resolvió el paciente de la imagen {id} (historia {})",
            img.historia_id
        ),
    }
    let crudo = match tokio::fs::read(e.dir_imagenes.join(&img.ruta_archivo)).await {
        Ok(b) => b,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    // Si el archivo está cifrado, se descifra con la clave de reposo. Sin clave
    // (o si el blob fue alterado) no se puede servir: 500, no datos corruptos.
    let bytes = if img.cifrada {
        match e.clave_reposo.as_deref().and_then(|f| guaca::reposo::descifrar(&crudo, f).ok()) {
            Some(claro) => claro,
            None => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    } else {
        crudo
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
