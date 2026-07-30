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

/// Cabeceras que van en TODA respuesta.
///
/// Es el respaldo sistémico que no había: `grep nosniff` sobre el repositorio no
/// devolvía nada, así que la única defensa contra que el navegador reinterprete
/// un tipo era que el tipo estuviera bien — y no lo estaba.
///
/// **Aquí van solo las que no pueden romper la interfaz.** Una CSP completa para
/// la aplicación necesita probarse contra el Angular compilado (estilos en línea,
/// trabajadores, fuentes) y no se pone a ciegas: una CSP que rompe la aplicación
/// es peor que ninguna, porque la siguiente reacción es quitarla entera. La CSP
/// estricta SÍ va, y máxima, en la respuesta que sirve archivos subidos — que es
/// donde están los bytes de un tercero.
async fn cabeceras_de_seguridad(
    peticion: axum::extract::Request,
    siguiente: axum::middleware::Next,
) -> Response {
    let mut r = siguiente.run(peticion).await;
    let h = r.headers_mut();
    // No reinterpretes el tipo que declaro.
    h.insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));
    // Nadie enmarca esta aplicación: sin esto, un sitio ajeno puede ponerla en un
    // iframe y engañar a quien la usa para que pulse donde no cree.
    h.insert(header::X_FRAME_OPTIONS, header::HeaderValue::from_static("DENY"));
    // Y la ruta de una historia clínica no viaja como `Referer` a terceros.
    h.insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    r
}

pub fn router(estado: Estado) -> Router {
    Router::new()
        .route("/api/acceso", post(acceso))
        .route("/api/salir", post(salir))
        .route("/api/salir-todo", post(salir_todo))
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
        // Después de `fallback` y antes de `with_state`: así cubre TODAS las
        // respuestas, incluidas las de la interfaz y las de una ruta que no existe.
        .layer(axum::middleware::from_fn(cabeceras_de_seguridad))
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

/// De dónde viene la petición, para el freno de `/api/acceso`.
///
/// Detrás de nginx la dirección del zócalo es siempre `127.0.0.1`, así que hace
/// falta `X-Forwarded-For` — pero esa cabecera la escribe **quien llama**, y
/// creerla sin más convierte el freno en un adorno: bastaría cambiar un número
/// en cada intento para no acumular ninguno. Por eso solo se lee cuando el
/// despliegue AFIRMA estar detrás de un proxy (`MEDICO_TRAS_PROXY`), que es
/// exactamente el caso en que esa cabecera la pone nuestro nginx y no el cliente.
///
/// Sin proxy declarado, el origen es el del zócalo. Si tampoco lo hay —una
/// prueba que llama al router sin red— queda `desconocido`, y entonces todos los
/// intentos comparten cubo: el freno degrada a «por cuenta», que es la forma
/// segura de degradar.
fn origen_de(tras_proxy: bool, cabeceras: &header::HeaderMap, zocalo: Option<std::net::SocketAddr>) -> String {
    if tras_proxy {
        // El primero de la lista es el cliente; los siguientes, los saltos.
        let primero = cabeceras
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|xff| xff.split(',').next())
            .map(str::trim)
            .filter(|p| !p.is_empty());
        if let Some(p) = primero {
            return p.to_string();
        }
    }
    zocalo.map(|s| s.ip().to_string()).unwrap_or_else(|| "desconocido".into())
}

/// Login. 200 con la cookie de sesión, 401 si no, **429 si se está probando**.
///
/// Un solo mensaje para todos los fallos: no se dice si el correo existe. El
/// freno se consulta ANTES de verificar la contraseña —si no, cada petición
/// frenada seguiría costando su Argon2 y el ensayo seguiría avanzando— y el
/// intento se anota SIEMPRE, acierte o falle, porque la cuenta de fallos se hace
/// desde el último acierto.
async fn acceso(
    State(e): State<Estado>,
    cabeceras: header::HeaderMap,
    extensiones: axum::http::Extensions,
    Json(cred): Json<Credenciales>,
) -> Response {
    // Se lee de las extensiones y no con el extractor `ConnectInfo` porque este
    // último RECHAZA la petición cuando el servidor no se montó con
    // `with_connect_info` — y así lo montan las pruebas, que llaman al router sin
    // red. Un login que devuelve 500 en las pruebas por no tener zócalo sería un
    // freno que impide probar el freno.
    let zocalo = extensiones
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0);
    let origen = origen_de(e.tras_proxy, &cabeceras, zocalo);
    let ahora = chrono::Utc::now();

    match medico_datos::freno::estado(&e.pool, &cred.email, &origen, ahora).await {
        Ok(f) if f.frenado() => {
            // El intento frenado también se anota: si no, insistir durante la
            // espera saldría gratis y la cuenta de fallos no crecería.
            let _ = medico_datos::freno::anotar(&e.pool, &cred.email, &origen, false, ahora).await;
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, f.segundos_restantes().to_string())],
            )
                .into_response();
        }
        Ok(_) => {}
        Err(err) => {
            // No se puede leer el freno: NO se deja pasar. Un fallo de la base en
            // el camino de autenticación que abriera la puerta sería el peor
            // defecto posible, y «degradar con gracia» aquí significa exactamente
            // eso. La historia clínica espera; la puerta abierta no se arregla.
            eprintln!("⚠️  no se pudo consultar el freno de acceso: {err}");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    }

    let resultado = autenticar(&e.pool, &e.secreto, e.minutos, &cred.email, &cred.clave).await;
    let _ = medico_datos::freno::anotar(
        &e.pool,
        &cred.email,
        &origen,
        resultado.is_some(),
        ahora,
    )
    .await;

    match resultado {
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

/// Cierra la sesión en TODOS los dispositivos (M3): sube el `token_version` del
/// usuario, con lo que toda cookie ya emitida —incluida la de este dispositivo—
/// deja de valer en la siguiente petición. Borra además la cookie de aquí. Actúa
/// sobre el usuario AUTENTICADO (nunca sobre un id que venga del cliente), así que
/// nadie revoca las sesiones de otro. Si la revocación falla, responde 500: no se
/// finge un cierre que no ocurrió (directiva de fallar en vez de suponer).
async fn salir_todo(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match medico_datos::usuario::revocar_sesiones(&e.pool, u.id).await {
        Ok(true) => (
            StatusCode::OK,
            [(header::SET_COOKIE, sesion::cabecera_cerrar())],
            Json(Ok2 { ok: true }),
        )
            .into_response(),
        // `false` = la sentencia no tocó ninguna fila: el usuario que acabamos de
        // autenticar ya no está. No hubo revocación, así que tampoco hay 200 —
        // descartar este booleano era justo fingir el cierre que el doc promete no fingir.
        Ok(false) => {
            eprintln!("⚠️  revocación sin efecto: el usuario {} ya no existe", u.id);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(err) => {
            eprintln!("⚠️  no se pudieron revocar las sesiones de {}: {err}", u.id);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
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
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroCamas>,
) -> Response {
    match hospitalizacion::listar_camas(&e.pool, u.ambito(), f.estado.as_deref(), f.servicio.as_deref()).await {
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
/// `archivo` (el binario) y, opcional, `descripcion`.
///
/// **El tipo lo decide el SERVIDOR mirando los bytes** (`tipos::tipo_real`), no
/// el `Content-Type` que declara el cliente. Antes esta frase estaba escrita y no
/// era cierta: el mime venía del cliente y se admitía cualquier cosa que empezara
/// por `image/` — incluido `image/svg+xml`, que es un documento con guiones y se
/// devolvía «en línea» desde el mismo origen que la aplicación. El nombre y el
/// tipo que declara el cliente no deciden nada: ni el contenido, ni dónde se
/// guarda —la ruta la fija el id de la fila—.
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
    // El tipo, de los BYTES. Lo que el cliente declaró se descarta aquí mismo: no
    // se compara siquiera con lo olfateado, porque no aporta nada — si coincide
    // no añade garantía y si discrepa la buena es la nuestra.
    let Some(tipo) = crate::tipos::tipo_real(&bytes) else {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    };
    let _ = mime;

    // `tamano_bytes` es el tamaño del ORIGINAL (lo que ve el usuario), no del
    // blob cifrado. Con clave de reposo el archivo va cifrado al disco.
    let meta = ImagenNueva {
        nombre_original: nombre,
        tipo_mime: tipo.to_string(),
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
    // El tipo se vuelve a olfatear sobre los bytes que se van a servir, y NO se
    // usa `img.tipo_mime`. Dos razones: las filas guardadas antes de esta
    // corrección llevan el tipo que declaró el cliente, y confiar en un valor de
    // la base porque «lo validamos al entrar» es confiar en la validación de una
    // versión anterior del código.
    let tipo = crate::tipos::tipo_real(&bytes).unwrap_or("application/octet-stream");
    // Lo que el navegador no pinta se descarga. `inline` sobre algo que no
    // reconoce es invitarle a adivinar qué es.
    let modo = if crate::tipos::se_puede_mostrar(tipo) { "inline" } else { "attachment" };
    // El nombre para descargar se limpia: un nombre de archivo del usuario podría
    // llevar comillas o saltos de línea y romper la cabecera.
    let disp = format!("{modo}; filename=\"{}\"", nombre_seguro(&img.nombre_original));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, tipo.to_string()),
            (header::CONTENT_DISPOSITION, disp),
            // `nosniff` para que el navegador no reinterprete el tipo que acabamos
            // de decidir, y una CSP que apaga TODO en esta respuesta: si algún día
            // algo con guiones consiguiera llegar hasta aquí, no tendría de dónde
            // cargar ni a dónde hablar. Va en esta respuesta y no en el router
            // porque aquí puede ser máxima sin romper nada — la interfaz nunca
            // sale por este extremo.
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox; base-uri 'none'; form-action 'none'".to_string(),
            ),
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use axum::body::Body;
    // Solo lo usan las pruebas: el handler llama a `medico_datos::freno::…` por
    // ruta completa, así que importarlo arriba sería un `use` muerto en el binario.
    use medico_datos::freno;
    use axum::http::Request;
    use tower::ServiceExt;

    const SECRETO: &[u8] = b"un secreto de prueba de mas de treinta y dos bytes";

    async fn estado_de_prueba(tras_proxy: bool) -> Estado {
        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool)
            .await
            .unwrap();
        let h = claves::hashear("clave123").unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
             VALUES (2,'Dra Ruiz','ruiz@x.co',?,'medico',1,1)",
        )
        .bind(&h)
        .execute(&pool)
        .await
        .unwrap();
        Estado {
            pool,
            secreto: SECRETO.to_vec(),
            minutos: 60,
            dir_imagenes: std::env::temp_dir(),
            clave_firma: None,
            clave_verificacion: None,
            clave_reposo: None,
            tras_proxy,
        }
    }

    /// Un intento de acceso, como lo hace un cliente. `xff` simula la cabecera
    /// que pondría un proxy — o que pondría un atacante.
    async fn intentar(estado: &Estado, email: &str, clave: &str, xff: Option<&str>) -> StatusCode {
        let mut peticion = Request::builder()
            .method("POST")
            .uri("/api/acceso")
            .header("content-type", "application/json");
        if let Some(v) = xff {
            peticion = peticion.header("x-forwarded-for", v);
        }
        let cuerpo = format!(r#"{{"email":"{email}","clave":"{clave}"}}"#);
        router(estado.clone())
            .oneshot(peticion.body(Body::from(cuerpo)).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn la_clave_correcta_entra_y_la_incorrecta_da_401() {
        let e = estado_de_prueba(false).await;
        assert_eq!(intentar(&e, "ruiz@x.co", "clave123", None).await, StatusCode::OK);
        assert_eq!(intentar(&e, "ruiz@x.co", "otra", None).await, StatusCode::UNAUTHORIZED);
    }

    /// El freno, ejercido por la ruta y no por el módulo: seis fallos seguidos y
    /// el séptimo intento recibe 429. Es la prueba que faltaba en tunjo —la
    /// lógica probada y el cableado no—, y aquí el cableado ES el control.
    #[tokio::test]
    async fn tras_pasarse_del_umbral_la_ruta_responde_429() {
        let e = estado_de_prueba(false).await;
        for i in 1..=freno::UMBRAL {
            assert_eq!(
                intentar(&e, "ruiz@x.co", "mal", None).await,
                StatusCode::UNAUTHORIZED,
                "el intento {i} ya frenaba: eso castiga a quien teclea mal"
            );
        }
        assert_eq!(
            intentar(&e, "ruiz@x.co", "mal", None).await,
            StatusCode::TOO_MANY_REQUESTS,
            "pasado el umbral la ruta no frenó"
        );
    }

    /// Y frena AUNQUE la clave sea correcta: si dejara pasar la buena, el freno
    /// no limitaría el ritmo de ensayo, que es lo único que hace.
    #[tokio::test]
    async fn el_freno_no_hace_excepcion_con_la_clave_buena() {
        let e = estado_de_prueba(false).await;
        for _ in 0..=freno::UMBRAL {
            intentar(&e, "ruiz@x.co", "mal", None).await;
        }
        assert_eq!(
            intentar(&e, "ruiz@x.co", "clave123", None).await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    /// La respuesta frenada lleva `Retry-After` con segundos, y nunca cero: un
    /// cero invita a reintentar ya mismo, que es lo contrario de lo que se pide.
    #[tokio::test]
    async fn el_429_dice_cuanto_hay_que_esperar() {
        let e = estado_de_prueba(false).await;
        for _ in 0..=freno::UMBRAL {
            intentar(&e, "ruiz@x.co", "mal", None).await;
        }
        let r = router(e.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/acceso")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"email":"ruiz@x.co","clave":"mal"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
        let segundos: u64 = r
            .headers()
            .get(header::RETRY_AFTER)
            .expect("falta Retry-After")
            .to_str()
            .unwrap()
            .parse()
            .expect("Retry-After no es un número de segundos");
        assert!(segundos >= 1, "Retry-After: {segundos}");
    }

    /// Un correo que NO existe se frena igual. Si solo se frenaran las cuentas
    /// reales, recibir el 429 diría que el correo existe — el mismo oráculo que
    /// el 401 único evita.
    #[tokio::test]
    async fn un_correo_inexistente_tambien_se_frena() {
        let e = estado_de_prueba(false).await;
        for _ in 0..=freno::UMBRAL {
            intentar(&e, "nadie@x.co", "mal", None).await;
        }
        assert_eq!(
            intentar(&e, "nadie@x.co", "mal", None).await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    /// **Sin proxy declarado, `X-Forwarded-For` no se cree.** Es la prueba que
    /// justifica la bandera: si se creyera siempre, bastaría cambiar el número de
    /// esa cabecera en cada intento para no acumular ninguno y el freno sería un
    /// adorno.
    #[tokio::test]
    async fn sin_proxy_declarado_la_cabecera_no_permite_esquivar_el_freno() {
        let e = estado_de_prueba(false).await;
        for i in 0..=freno::UMBRAL {
            intentar(&e, "ruiz@x.co", "mal", Some(&format!("203.0.113.{i}"))).await;
        }
        assert_eq!(
            intentar(&e, "ruiz@x.co", "mal", Some("203.0.113.99")).await,
            StatusCode::TOO_MANY_REQUESTS,
            "una cabecera que pone el propio cliente esquivó el freno"
        );
    }

    /// Con proxy declarado sí se lee, que es para lo que existe: detrás de nginx
    /// todos los intentos comparten zócalo y sin la cabecera el freno degradaría
    /// a «por cuenta», que es justo lo que deja fuera al dueño de la cuenta.
    #[tokio::test]
    async fn con_proxy_declarado_cada_origen_lleva_su_propia_cuenta() {
        let e = estado_de_prueba(true).await;
        for _ in 0..=freno::UMBRAL {
            intentar(&e, "ruiz@x.co", "mal", Some("203.0.113.9")).await;
        }
        assert_eq!(
            intentar(&e, "ruiz@x.co", "mal", Some("203.0.113.9")).await,
            StatusCode::TOO_MANY_REQUESTS,
            "el origen que insiste no quedó frenado"
        );
        // Y la doctora, desde su estación, entra con su clave correcta.
        assert_eq!(
            intentar(&e, "ruiz@x.co", "clave123", Some("10.0.0.5")).await,
            StatusCode::OK,
            "la dueña de la cuenta quedó fuera por culpa de otro"
        );
    }

    /// Un multipart mínimo con un archivo, hecho a mano para poder mentir en el
    /// `Content-Type` de la parte — que es exactamente lo que hace el atacante.
    fn multipart(nombre: &str, tipo: &str, bytes: &[u8]) -> (String, Vec<u8>) {
        let sep = "----frontera-de-prueba";
        let mut cuerpo = Vec::new();
        cuerpo.extend_from_slice(
            format!(
                "--{sep}\r\nContent-Disposition: form-data; name=\"archivo\"; \
                 filename=\"{nombre}\"\r\nContent-Type: {tipo}\r\n\r\n"
            )
            .as_bytes(),
        );
        cuerpo.extend_from_slice(bytes);
        cuerpo.extend_from_slice(format!("\r\n--{sep}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={sep}"), cuerpo)
    }

    async fn subir(estado: &Estado, cookie: &str, tipo: &str, bytes: &[u8]) -> StatusCode {
        let (ct, cuerpo) = multipart("radiografia.svg", tipo, bytes);
        router(estado.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/historias/1/imagenes")
                    .header("content-type", ct)
                    .header(header::COOKIE, cookie)
                    .body(Body::from(cuerpo))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    /// **El hallazgo HIGH de la revisión del 2026-07-30.** Un SVG declarado como
    /// `image/svg+xml` pasaba la comprobación —empieza por `image/`— y se
    /// devolvía «en línea» desde el mismo origen que la aplicación: guion
    /// ejecutándose con la sesión de quien abriera el adjunto.
    ///
    /// La prueba va en la RUTA y no en el olfateador: el control es el cableado,
    /// y el módulo puede estar perfecto mientras el handler no lo llama.
    #[tokio::test]
    async fn un_svg_no_entra_por_mucho_que_el_cliente_lo_llame_imagen() {
        let e = estado_de_prueba(false).await;
        let cookie = {
            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/acceso")
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"email":"ruiz@x.co","clave":"clave123"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            let c = r.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
            c.split(';').next().unwrap().to_string()
        };

        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        assert_eq!(
            subir(&e, &cookie, "image/svg+xml", svg).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "un SVG entró declarándose imagen"
        );
        // Y tampoco disfrazado de PNG: el tipo sale de los bytes, no del nombre
        // ni de lo declarado.
        assert_eq!(
            subir(&e, &cookie, "image/png", svg).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "un SVG entró disfrazado de PNG"
        );
        assert_eq!(
            subir(&e, &cookie, "image/png", b"<html><script>alert(1)</script>").await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "un HTML entró disfrazado de PNG"
        );
    }

    /// Toda respuesta lleva las cabeceras que impiden reinterpretarla. Antes
    /// `grep nosniff` sobre el repositorio no devolvía nada.
    #[tokio::test]
    async fn toda_respuesta_lleva_las_cabeceras_de_seguridad() {
        let e = estado_de_prueba(false).await;
        // Incluso una ruta que falla y una que no existe: si las cabeceras
        // dependieran del camino feliz, no serían un respaldo sistémico.
        for uri in ["/api/acceso", "/api/no-existe", "/"] {
            let r = router(e.clone())
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let h = r.headers();
            assert_eq!(
                h.get(header::X_CONTENT_TYPE_OPTIONS).map(|v| v.to_str().unwrap()),
                Some("nosniff"),
                "«{uri}» salió sin nosniff"
            );
            assert_eq!(
                h.get(header::X_FRAME_OPTIONS).map(|v| v.to_str().unwrap()),
                Some("DENY"),
                "«{uri}» se puede enmarcar"
            );
        }
    }

    #[test]
    fn el_origen_sale_del_zocalo_salvo_que_haya_proxy() {
        let zocalo: std::net::SocketAddr = "198.51.100.7:44444".parse().unwrap();
        let mut c = header::HeaderMap::new();
        c.insert("x-forwarded-for", "203.0.113.1, 10.0.0.1".parse().unwrap());

        assert_eq!(origen_de(false, &c, Some(zocalo)), "198.51.100.7");
        assert_eq!(origen_de(true, &c, Some(zocalo)), "203.0.113.1");

        // Con proxy declarado pero cabecera vacía o ausente, se vuelve al zócalo
        // en vez de inventarse un origen.
        let vacia = header::HeaderMap::new();
        assert_eq!(origen_de(true, &vacia, Some(zocalo)), "198.51.100.7");
        // Y sin zócalo —una prueba que llama al router sin red— queda un valor
        // único y explícito, no una cadena vacía que se confunda con un origen.
        assert_eq!(origen_de(true, &vacia, None), "desconocido");
    }
}
