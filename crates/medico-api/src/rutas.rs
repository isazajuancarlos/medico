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

use crate::auth::{
    autenticar, AtenderUrgencias, Autenticado, Autorizado, Borrar, EscribirClinico,
    EscribirImagenologia, EscribirLaboratorio, EscribirNutricion, EscribirPaciente,
    EscribirRehabilitacion, Estado, Firmar, GestionarAgenda, GestionarAmbulancias,
    GestionarFarmacia, GestionarRemision, GestionarUsuarios,
};
use crate::{claves, sesion};
use medico_datos::acceso;
use medico_datos::agenda::{self, AgendaNueva, BloqueNuevo, BloqueoNuevo};
use medico_datos::ambulancia;
use medico_datos::catalogo::{self, Especialidad, Medicamento, UnidadDispensacion};
use medico_datos::cita::{self, CitaNueva, CitaVista};
use medico_datos::diagnostico;
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
use medico_datos::inventario;
use medico_datos::licencia;
use medico_datos::orden::{self, Orden, OrdenNueva};
use medico_datos::paciente::{self, Antecedentes, InfoPaciente, Paciente, PacienteNuevo};
use medico_datos::proveedor;
use medico_datos::usuario::{self, ResumenUsuario, Usuario, UsuarioNuevo};

/// Tope de una subida de imagen. Una radiografía o un PDF de laboratorio caben
/// de sobra; por encima es casi seguro un error o un abuso.
const MAX_SUBIDA: usize = 25 * 1024 * 1024;

/// La política de contenido de la APLICACIÓN. Ver [`cabeceras_de_seguridad`].
const CSP: &str = "default-src 'self'; \
                   script-src 'self'; \
                   style-src 'self' 'unsafe-inline'; \
                   img-src 'self' data:; \
                   font-src 'self'; \
                   connect-src 'self'; \
                   object-src 'none'; \
                   base-uri 'self'; \
                   form-action 'self'; \
                   frame-ancestors 'none'";

/// Cabeceras que van en TODA respuesta.
///
/// Es el respaldo sistémico que no había: `grep nosniff` sobre el repositorio no
/// devolvía nada, así que la única defensa contra que el navegador reinterprete
/// un tipo era que el tipo estuviera bien — y no lo estaba.
///
/// La CSP máxima —`default-src 'none'; sandbox`— sigue yendo aparte, en la
/// respuesta que sirve archivos subidos: ahí están los bytes de un tercero y ahí
/// puede cerrarse todo sin romper nada, porque la interfaz nunca sale por ese
/// extremo. Lo de aquí es la política de la APLICACIÓN, que es otra cosa.
///
/// # La CSP: probada contra el Angular compilado, no copiada de una lista
///
/// - **`script-src 'self'`, sin `'unsafe-inline'` ni `'unsafe-eval'`.** Es la que
///   para un XSS. Para que se pudiera poner hubo que apagar `inlineCritical` en
///   `angular.json`: emitía `onload="this.media='all'"` en la hoja de estilos —un
///   manejador en línea, o sea script—, y con esta directiva el navegador no lo
///   ejecuta, la hoja se queda en `media="print"` y la aplicación se ve SIN
///   ESTILOS *sin dar un solo error*.
/// - **`style-src` lleva `'unsafe-inline'` y no es pereza.** Angular y PrimeNG
///   escriben estilos en tiempo de ejecución y usan atributos `style="…"`. Un
///   nonce no lo arreglaría: vale para elementos `<style>`, no para atributos
///   `style`. El riesgo que deja —inyectar CSS— es mucho menor que ejecutar
///   código.
/// - **`default-src 'self'` se pudo cerrar porque el `index.html` dejó de salir a
///   `fonts.googleapis.com`.** Inter viaja ahora dentro del paquete. Eso no era
///   solo un problema de CSP: cada carga le contaba a un tercero que alguien está
///   usando una historia clínica, desde qué IP y a qué hora.
/// - **Nada de HSTS aquí**: el TLS termina en el proxy, que es quien puede
///   prometerlo con conocimiento de causa.
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
    // La respuesta que sirve un archivo subido ya trae la suya, MÁS estricta
    // (`default-src 'none'; sandbox`). No se le pisa: quien la puso sabía que ahí
    // podía cerrar del todo, y esta es más permisiva por necesidad.
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(header::CONTENT_SECURITY_POLICY, header::HeaderValue::from_static(CSP));
    }
    r
}

pub fn router(estado: Estado) -> Router {
    Router::new()
        .route("/api/acceso", post(acceso))
        .route("/api/salir", post(salir))
        .route("/api/salir-todo", post(salir_todo))
        .route("/api/yo", get(yo))
        .route("/api/demo", get(demo))
        .route("/api/pacientes", get(listar_pacientes).post(crear_paciente))
        .route("/api/pacientes/{id}", get(obtener_paciente).put(actualizar_paciente))
        .route("/api/pacientes/{id}/antecedentes", put(actualizar_antecedentes_paciente))
        .route("/api/pacientes/{id}/historias", get(listar_historias_paciente))
        .route("/api/pacientes/{id}/accesos", get(accesos_paciente))
        // DIVIPOLA, para el selector de municipio (#16/#77).
        .route("/api/municipios", get(listar_municipios))
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
        // Laboratorio. Las órdenes las crea la historia clínica; aquí se
        // procesan. Leer es de cualquiera con sesión —el médico consulta el
        // resultado de su paciente—; escribir exige `EscribirLaboratorio`.
        .route("/api/laboratorio/ordenes", get(lab_ordenes))
        .route("/api/laboratorio/ordenes/{id}", get(lab_orden))
        .route("/api/laboratorio/ordenes/{id}/estado", put(lab_estado))
        .route(
            "/api/laboratorio/ordenes/{id}/resultado",
            get(lab_resultado).post(lab_crear_resultado).put(lab_actualizar_resultado),
        )
        .route("/api/laboratorio/ordenes/{id}/resultado/validar", post(lab_validar))
        .route("/api/laboratorio/paciente/{id}/resultados", get(lab_historial))
        .route("/api/laboratorio/perfiles", get(lab_perfiles).post(lab_crear_perfil))
        .route("/api/laboratorio/perfiles/{id}", axum::routing::delete(lab_eliminar_perfil))
        // Imágenes diagnósticas. Conviven con `/api/imagenes/{id}` y
        // `/api/imagenes/{id}/archivo`, que son los ADJUNTOS de la historia: los
        // segmentos literales (`ordenes`, `paciente`) tienen prioridad sobre el
        // parámetro, así que no se pisan. La prueba `las_rutas_de_imagenes_no_se_pisan`
        // lo comprueba en vez de confiarlo al enrutador.
        .route("/api/imagenes/ordenes", get(img_ordenes))
        .route("/api/imagenes/ordenes/{id}", get(img_orden))
        .route("/api/imagenes/ordenes/{id}/estado", put(img_estado))
        .route(
            "/api/imagenes/ordenes/{id}/informe",
            get(img_informe).post(img_crear_informe).put(img_actualizar_informe),
        )
        .route("/api/imagenes/ordenes/{id}/informe/firmar", post(img_firmar))
        .route(
            "/api/imagenes/ordenes/{id}/informe/imagenes",
            post(img_subir).layer(DefaultBodyLimit::max(MAX_SUBIDA)),
        )
        .route("/api/imagenes/paciente/{id}/informes", get(img_historial))
        .route("/api/instituciones", get(listar_instituciones).post(crear_institucion))
        .route("/api/instituciones/{id}/licencia", put(fijar_licencia))
        .route("/api/usuarios", get(listar_usuarios).post(crear_usuario))
        .route("/api/usuarios/{id}/documento", put(fijar_documento_usuario))
        // El menú que ve cada institución (#65). Leerlo basta con estar
        // autenticado; cambiar la licencia es del vendedor, no del cliente.
        .route("/api/modulos", get(crate::modulos::listar))
        // El RIPS del periodo (#250). Solo emite el AC todavía, y se niega a
        // hacerlo sin que se lo pidan: ver la cabecera de `crate::rips`.
        .route("/api/rips", get(crate::rips::generar))
        .route(
            "/api/instituciones/{id}/modulos",
            put(crate::modulos::fijar),
        )
        // La matriz de capacidades de la institución (#235). Misma capacidad
        // que gestionar usuarios: quién es quién y quién puede qué son la misma
        // responsabilidad.
        .route(
            "/api/capacidades",
            get(crate::capacidades::obtener).put(crate::capacidades::actualizar),
        )
        .route("/api/medicos", get(listar_medicos))
        .route("/api/citas", get(listar_citas).post(crear_cita))
        .route("/api/citas/{id}/estado", put(cambiar_estado_cita))
        // Agenda (#237). LEER es de cualquiera con sesión: quien agenda desde el
        // mostrador tiene que ver los turnos libres para poder darlos. ESCRIBIR
        // —definir las franjas de un profesional, bloquearle un rato— exige
        // `GestionarAgenda`: a un médico no se le cierra la consulta del jueves
        // sin que se entere.
        //
        // El orden importa: los segmentos literales (`medicos`, `turnos`,
        // `bloqueos`, `indicadores`) van ANTES que `/api/agenda/{id}`, y
        // `las_rutas_de_agenda_no_se_pisan` lo comprueba en vez de confiarlo.
        .route("/api/agenda/medicos", get(agenda_medicos))
        .route("/api/agenda/turnos", get(agenda_turnos))
        .route("/api/agenda/indicadores", get(agenda_indicadores))
        .route(
            "/api/agenda/bloqueos",
            get(agenda_listar_bloqueos).post(agenda_crear_bloqueo),
        )
        .route(
            "/api/agenda/bloqueos/{id}",
            axum::routing::delete(agenda_borrar_bloqueo),
        )
        .route("/api/agenda", get(agenda_listar).post(agenda_crear))
        .route(
            "/api/agenda/{id}",
            get(agenda_obtener)
                .put(agenda_actualizar)
                .delete(agenda_borrar),
        )
        .route("/api/agenda/{id}/bloques", post(agenda_anadir_bloque))
        .route(
            "/api/agenda/{id}/bloques/{bloque_id}",
            put(agenda_activar_bloque).delete(agenda_borrar_bloque),
        )
        // Farmacia. Leer no exige capacidad; escribir sí (`GestionarFarmacia`).
        .route("/api/farmacia/stats", get(farm_stats))
        .route("/api/farmacia/inventario", get(farm_listar).post(farm_crear))
        .route(
            "/api/farmacia/inventario/{id}",
            put(farm_actualizar).delete(farm_eliminar),
        )
        .route("/api/farmacia/inventario/{id}/entrada", post(farm_entrada))
        .route("/api/farmacia/formulas-pendientes", get(farm_formulas_pendientes))
        .route("/api/farmacia/dispensar", post(farm_dispensar))
        .route("/api/farmacia/dispensaciones", get(farm_dispensaciones))
        .route("/api/farmacia/movimientos", get(farm_movimientos))
        .route(
            "/api/farmacia/proveedores",
            get(prov_listar).post(prov_crear),
        )
        .route(
            "/api/farmacia/proveedores/{id}",
            get(prov_obtener).put(prov_actualizar).delete(prov_eliminar),
        )
        // Ambulancias. `traslados` es un segmento literal y va antes que `{id}`,
        // como en `/api/imagenes`.
        .route("/api/ambulancias", get(amb_listar).post(amb_crear))
        .route("/api/ambulancias/traslados", get(amb_traslados).post(amb_despachar))
        .route("/api/ambulancias/traslados/{id}", get(amb_traslado))
        .route("/api/ambulancias/traslados/{id}/finalizar", post(amb_finalizar))
        .route("/api/ambulancias/traslados/{id}/cancelar", post(amb_cancelar))
        .route(
            "/api/ambulancias/{id}",
            get(amb_obtener).put(amb_actualizar).delete(amb_eliminar),
        )
        // Catálogos de referencia: los lee cualquier usuario autenticado.
        .route("/api/especialidades", get(listar_especialidades))
        .route("/api/medicamentos/buscar", get(buscar_medicamentos))
        .route("/api/unidades-dispensacion", get(listar_unidades_dispensacion))
        // Las dos tablas de la 948 que la historia tiene que ofrecer para que
        // el médico ELIJA. Hasta el 2026-08-02 no se ofrecían y el sistema las
        // rellenaba solo, con códigos que además eran de la tabla anterior.
        .route("/api/finalidades", get(listar_finalidades))
        .route("/api/causas-motivo", get(listar_causas_motivo))
        .route(
            "/api/tipos-documento-profesional",
            get(listar_tipos_documento_profesional),
        )
        // Todo lo que no sea una ruta de API lo resuelve la interfaz embebida:
        // los archivos del build y, para las rutas del SPA, el index.html.
        .fallback(crate::estaticos::servir)
        // Después de `fallback` y antes de `with_state`: así cubre TODAS las
        // respuestas, incluidas las de la interfaz y las de una ruta que no existe.
        .layer(axum::middleware::from_fn(cabeceras_de_seguridad))
        .with_state(estado)
}

// `es_gestor` ya no existe. Comprobaba el rol A MANO —`Admin | Superadmin`— y
// por tanto NO consultaba la matriz de capacidades por institución (#235): una
// institución podía quitarle `gestionar_usuarios` a `admin`, la matriz lo
// aceptaba, la pantalla lo pintaba revocado… y `/api/usuarios` seguía dejándole
// entrar. Peor: la prueba `un_admin_no_se_quita_a_si_mismo_la_administracion`
// BENDICE ese estado, así que había una prueba en verde sobre una promesa que
// esa ruta no cumplía.
//
// Las dos rutas que lo usaban piden ahora `Autorizado<GestionarUsuarios>`, que
// es el único sitio donde esa pregunta se responde. Quitar la copia débil es el
// arreglo; dejarla y «acordarse» de sincronizarla, no (directiva 1).

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
/// falta una cabecera. **Cuál, decide si el freno funciona.**
///
/// # Por qué `X-Real-IP` y no `X-Forwarded-For`
///
/// Esto leía el primer salto de `X-Forwarded-For`, razonando que con
/// `MEDICO_TRAS_PROXY` esa cabecera «la pone nuestro nginx y no el cliente». La
/// premisa era falsa. `frontend/nginx.conf` la escribe con
/// `$proxy_add_x_forwarded_for`, que **añade**, no sobrescribe: es
/// `$http_x_forwarded_for, $remote_addr`. Un cliente que mande
/// `X-Forwarded-For: 1.2.3.4` produce `1.2.3.4, <su IP real>`, y el primer salto
/// —el que se leía— es justo el valor que él eligió.
///
/// Es decir: en la única postura para la que se escribió (`tras_proxy` activo,
/// detrás de nuestro nginx), bastaba cambiar un número por petición para no
/// acumular ninguno. El control existía, estaba probado y documentado, y **no
/// frenaba**. La prueba que lo cubría pasaba porque simulaba el valor que pone el
/// proxy, no uno falsificado por el cliente.
///
/// `X-Real-IP` sí se escribe con `$remote_addr` (misma plantilla, línea de
/// arriba): es un valor, no una lista, y nginx **pisa** lo que traiga el cliente.
/// Es la misma elección que hace `informes` en su módulo `origen`.
///
/// # Y por qué ya no hay `desconocido`
///
/// Antes, si no había cabecera ni zócalo se devolvía `"desconocido"` y todos los
/// intentos compartían cubo. Eso no es «degradar con seguridad»: es el freno por
/// cuenta sola, que es la variante que este módulo rechazó al diseñarse porque
/// deja que un desconocido eche al dueño de la cuenta. Ante el dato ausente se
/// devuelve `None` y quien llama decide — y decide no dejar pasar.
fn origen_de(
    tras_proxy: bool,
    cabeceras: &header::HeaderMap,
    zocalo: Option<std::net::SocketAddr>,
) -> Option<String> {
    if tras_proxy {
        return cabeceras
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_owned);
    }
    // Sin proxy declarado, la dirección del zócalo: el cliente no la elige.
    zocalo.map(|s| s.ip().to_string())
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
    let Some(origen) = origen_de(e.tras_proxy, &cabeceras, zocalo) else {
        // Mismo criterio que un fallo al leer el freno: si no se puede aplicar,
        // no se deja pasar. Rellenarlo con un valor común pondría a todo el
        // mundo en el mismo cubo, y entonces cinco peticiones de un desconocido
        // dejan fuera a la doctora — el bloqueo que este freno existe para no
        // tener.
        eprintln!(
            "⚠️  no se pudo determinar el origen de la petición. Con MEDICO_TRAS_PROXY \
             activo hace falta que el proxy escriba X-Real-IP (proxy_set_header \
             X-Real-IP $remote_addr)."
        );
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let ahora = chrono::Utc::now();

    match medico_datos::freno::estado(&e.pool, &cred.email, &origen, ahora).await {
        Ok(f) if f.frenado() => {
            // EL INTENTO FRENADO NO SE ANOTA, y esto es lo contrario de lo que
            // hacía antes. La razón que lo justificaba —«si no, insistir durante
            // la espera saldría gratis»— suena bien y convierte el freno en lo
            // único que el diseño dice no ser: un bloqueo.
            //
            // Con el intento anotado, cada reintento durante la espera suma un
            // fallo, y como la espera dobla por fallo, quien insiste llega al
            // techo de cinco minutos y SE QUEDA ahí mientras siga insistiendo: en
            // cuanto vence, un clic lo devuelve arriba. Eso no le pasa a un
            // atacante paciente —él espera—, le pasa a la doctora que pulsa tres
            // veces porque cree que la página se colgó. El freno existe para
            // limitar el RITMO de ensayo, no para castigar la impaciencia, y
            // «nadie tiene que desbloquear a nadie» deja de ser cierto si la
            // víctima puede encerrarse sola.
            //
            // Insistir tampoco sale gratis: se responde 429 y la espera no se
            // acorta por pedirla más veces. Y hay una segunda razón, de la misma
            // familia que el corte del correo largo: anotar aquí es UNA ESCRITURA
            // EN LA BASE POR PETICIÓN NO AUTENTICADA. Justo lo que este camino
            // debe evitar.
            //
            // informes hace esto mismo (`rutas.rs`, rama `freno.frenado()`), y las
            // dos aplicaciones comparten la regla en `guaca::freno`. Diferir en
            // quién la alimenta hacía que la misma regla se comportara distinto.
            eprintln!("acceso frenado para «{}» desde {origen}", cred.email);
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
            [(header::SET_COOKIE, sesion::cabecera_abrir(&cookie, e.minutos, e.tras_proxy))],
            Json(Ok2 { ok: true }),
        )
            .into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// Cierra la sesión borrando la cookie. No necesita estar autenticado: pedir
/// salir sin sesión no es un error.
///
/// Sí necesita el `Estado`, y solo por `tras_proxy`: la cookie de cierre lleva
/// los mismos atributos que la de apertura para que de verdad la sustituya.
async fn salir(State(e): State<Estado>) -> Response {
    (
        StatusCode::OK,
        [(header::SET_COOKIE, sesion::cabecera_cerrar(e.tras_proxy))],
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
            [(header::SET_COOKIE, sesion::cabecera_cerrar(e.tras_proxy))],
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

#[derive(Serialize)]
struct CredencialesDemo {
    correo: String,
    clave: String,
}

/// Las credenciales de la DEMOSTRACIÓN, para que la pantalla de acceso las
/// muestre. **Sin sesión**, porque quien la necesita todavía no la tiene.
///
/// Responde `204 No Content` cuando el despliegue no pidió ser una demostración,
/// que es el defecto y lo que le pasa a la instalación de un cliente: no hay
/// nada que enseñar y no hay que acordarse de apagar nada. Publicar unas
/// credenciales exige poner DOS variables de entorno a propósito
/// (`MEDICO_DEMO_CORREO` y `MEDICO_DEMO_CLAVE`); media configuración no arranca.
///
/// No se registra en la bitácora de accesos: no hay titular ni dato de paciente
/// detrás, solo la nota de un cartel que el propio despliegue mandó colgar.
async fn demo(State(e): State<Estado>) -> Response {
    match &e.demo {
        Some(d) => Json(CredencialesDemo {
            correo: d.correo.clone(),
            clave: d.clave.clone(),
        })
        .into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// Pacientes, acotados al ámbito del usuario. El `ambito()` sale del rol y la
/// institución del usuario autenticado: nunca es un parámetro que el cliente
/// pueda mandar.
async fn listar_pacientes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<Vec<Paciente>>, StatusCode> {
    let lista = paciente::listar(&e.pool, u.ambito())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // El directorio de pacientes es dato personal: quién lo consultó y cuántos
    // vio va a la bitácora. Sin los ids —eso es lo que separa un listado de una
    // exportación—, y también cuando devuelve cero: «no hay pacientes» también
    // se consultó, y un rastro que solo anota lo que dio resultado deja fuera
    // justo el barrido de quien va probando.
    registrar_coleccion(&e, &u, "listar", "pacientes", None, lista.len(), vec![]).await;
    Ok(Json(lista))
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
        Ok(Some(p)) => {
            // La ficha del paciente ES el dato personal, y hasta el 2026-08-02
            // abrirla no dejaba rastro: se anotaba su historia y su documento,
            // pero no él. Un recurso único, que es lo que es.
            registrar_acceso(&e, &u, "ver", "paciente", id, id).await;
            Ok(Json(p))
        }
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
    // Los eventos vienen ya proyectados por la capa de datos: de una exportación
    // sale cuántos pacientes cubrió, nunca quiénes eran los otros.
    match acceso::por_paciente(&e.pool, u.ambito(), id).await {
        Ok(eventos) => Ok(Json(eventos)),
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
/// Comprueba que el municipio, si viene, EXISTE en DIVIPOLA.
///
/// `None` es válido: un paciente puede no tener el dato todavía, y eso se ve.
/// Lo que no se acepta es un código inventado, que pasa la validación de
/// estructura del RIPS y sale mal en el envío, lejos de donde se tecleó. **Y no
/// se sustituye por ninguno**: el sistema viejo rellenaba con `"05001"` y
/// reportaba a todo el país como residente en Medellín — un dato falso no se
/// nota nunca, y eso es peor que un rechazo (directiva 20).
async fn municipio_valido(e: &Estado, cod: &Option<String>) -> Result<(), Response> {
    let Some(cod) = cod.as_deref() else { return Ok(()) };
    if cod.trim().is_empty() {
        return Ok(());
    }
    match medico_datos::divipola::existe(&e.pool, cod).await {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("el código de municipio «{cod}» no está en DIVIPOLA (van cinco dígitos, con el cero de la izquierda)"),
        )
            .into_response()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

async fn crear_paciente(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirPaciente>,
    Json(nuevo): Json<PacienteNuevo>,
) -> Response {
    let u = autorizado.0;
    if let Err(r) = municipio_valido(&e, &nuevo.cod_municipio).await {
        return r;
    }
    match paciente::crear(&e.pool, u.ambito(), &nuevo).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /api/municipios?q=…&limite=…` — el selector de la pantalla.
///
/// Solo pide estar autenticado: la lista de municipios de Colombia es pública y
/// no dice nada de ningún paciente. Exigirle una capacidad sería una regla que
/// no protege nada y que habría que recordar para cada pantalla nueva.
async fn listar_municipios(
    State(e): State<Estado>,
    _u: Autenticado,
    Query(q): Query<BuscarMunicipio>,
) -> Result<Json<Vec<medico_datos::divipola::Municipio>>, StatusCode> {
    // 20 por defecto: lo que cabe en un desplegable sin que haya que
    // desplazarse. El llamante puede pedir más; se acota a 500 para que un
    // parámetro suelto no arrastre el país entero por la red en cada tecla.
    let limite = q.limite.unwrap_or(20).clamp(1, 500);
    medico_datos::divipola::buscar(&e.pool, q.q.as_deref().unwrap_or(""), limite)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(serde::Deserialize)]
struct BuscarMunicipio {
    q: Option<String>,
    limite: Option<i64>,
}

/// Actualiza un paciente propio. 204 si se actualizó; **404 si no existe o no es
/// suyo** —nunca 403: un 403 confirmaría que el id existe en otra institución, y
/// eso ya enumera pacientes ajenos—.
async fn actualizar_paciente(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirPaciente>,
    Path(id): Path<i64>,
    Json(datos): Json<PacienteNuevo>,
) -> Response {
    let u = autorizado.0;
    // La misma comprobación que al crear. Sin esto, el municipio inválido entra
    // por la puerta de la edición: cerrar solo una de las dos deja el agujero
    // igual de abierto y encima parece cerrado.
    if let Err(r) = municipio_valido(&e, &datos.cod_municipio).await {
        return r;
    }
    match paciente::actualizar(&e.pool, u.ambito(), id, &datos).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Actualiza los antecedentes (JSON) de un paciente propio. 204 si se actualizó;
/// **404 si no existe o no es suyo** —nunca 403, la misma regla que el resto—.
/// Solo se escriben los bloques presentes: guardar unos no borra los otros.
async fn actualizar_antecedentes_paciente(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirPaciente>,
    Path(id): Path<i64>,
    Json(ant): Json<Antecedentes>,
) -> StatusCode {
    let u = autorizado.0;
    match paciente::actualizar_antecedentes(&e.pool, u.ambito(), id, &ant).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// --- Historia clínica --------------------------------------------------------

async fn crear_historia(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Json(h): Json<HistoriaNueva>,
) -> Response {
    let u = autorizado.0;
    // Quién registra sale de la SESIÓN. El cuerpo dice a quién se ATRIBUYE
    // (`medico_id`, ya validado contra la institución); quién teclea, no.
    match historia::crear(&e.pool, u.ambito(), u.id, &h).await {
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
        objeto: acceso::Objeto::Recurso {
            paciente_id,
            recurso_id,
        },
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    if let Err(err) = acceso::registrar(&e.pool, &evento, e.clave_firma.as_deref()).await {
        eprintln!("⚠️  bitácora de acceso: no se registró {recurso} {recurso_id} (paciente {paciente_id}): {err}");
    }
}

/// El registro de un acceso a un CONJUNTO, best-effort como [`registrar_acceso`].
/// No se llama directamente: entra por [`registrar_coleccion`] o por
/// [`registrar_coleccion_de_un_titular`], que son las que eligen la forma.
async fn registrar_conjunto(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    accion: &str,
    recurso: &str,
    objeto: acceso::Objeto,
) {
    let evento = acceso::Evento {
        usuario_id: u.id,
        rol: u.rol.clone(),
        institucion_id: u.institucion_id,
        accion: accion.to_string(),
        recurso: recurso.to_string(),
        objeto,
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    if let Err(err) = acceso::registrar(&e.pool, &evento, e.clave_firma.as_deref()).await {
        eprintln!("⚠️  bitácora de acceso: no se registró {accion} de {recurso}: {err}");
    }
}

/// El alcance de un conjunto visto por `u`. El ámbito sale del usuario, nunca de
/// un parámetro: es la misma regla que el RIPS aprendió a la mala.
fn alcance_visto_por(
    u: &medico_datos::usuario::Usuario,
    filtro: Option<String>,
    n: usize,
    pacientes: Vec<i64>,
) -> acceso::Alcance {
    acceso::Alcance {
        ambito: acceso::describir_ambito(u.ambito()),
        filtro,
        n: n as i64,
        pacientes,
    }
}

/// Un conjunto que **atraviesa titulares**: el directorio de pacientes, una
/// exportación de RIPS.
///
/// La diferencia con un recurso único no es cosmética (ver `acceso::Alcance`):
/// un **listado** anota a qué ámbito se consultó, con qué filtro y cuántos
/// salieron, y **no guarda ids** —ver un nombre en una lista no es abrir el
/// expediente, y un eslabón por fila ahogaría la cadena—; una **exportación**
/// sí guarda los pacientes que sacó, porque el dato salió del sistema y el
/// titular tiene derecho a saber que el suyo iba dentro (Ley 1581).
pub(crate) async fn registrar_coleccion(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    accion: &str,
    recurso: &str,
    filtro: Option<String>,
    n: usize,
    pacientes: Vec<i64>,
) {
    let alcance = alcance_visto_por(u, filtro, n, pacientes);
    registrar_conjunto(e, u, accion, recurso, acceso::Objeto::Coleccion { alcance }).await;
}

/// Un conjunto de registros de **UN titular**: sus historias, sus documentos,
/// sus resultados de laboratorio.
///
/// No lleva `pacientes` y no es negociable: esa lista significa «el dato salió
/// del sistema», y aquí no salió — el titular ya va nombrado aparte, que es lo
/// que hace que el acceso aparezca en SU rastro. En el rastro se lee
/// `pacientes_n: 0`, y esa es justo la diferencia que él necesita ver entre
/// alguien que consultó su expediente y alguien que se lo llevó.
pub(crate) async fn registrar_coleccion_de_un_titular(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    accion: &str,
    recurso: &str,
    paciente_id: i64,
    filtro: Option<String>,
    n: usize,
) {
    // EL TITULAR TIENE QUE SER DEL ÁMBITO DE QUIEN PREGUNTA, y se comprueba aquí
    // y no en cada ruta por lo mismo que `obtener()` existe en vez de arreglar
    // cada `db.get`: un control que hay que acordarse de poner en cuatro sitios
    // se olvida en el quinto.
    //
    // Sin esto, el `paciente_id` entraba tal cual de la petición. Las consultas
    // SÍ están aisladas —devuelven la lista vacía para un paciente ajeno, así que
    // no sale ni un dato—, pero el evento se sellaba igual: bastaba recorrer
    // identificadores contra `/api/pacientes/{id}/historias` para meter eslabones
    // firmados en el rastro de los pacientes de OTRA institución, donde su
    // titular los lee como accesos que nunca ocurrieron. Y no se pueden quitar:
    // la cadena está encadenada por hash, así que borrar el eslabón falso deja la
    // bitácora entera en `Rota`.
    //
    // Es la misma regla que ya seguían `obtener_paciente` y `contexto_paciente`,
    // que registran dentro del `Ok(Some(...))` — y que una ficha ajena no deje
    // rastro es justo lo que aquellas prueban.
    match acceso::paciente_es_del_ambito(&e.pool, u.ambito(), paciente_id).await {
        Ok(true) => {}
        // No es suyo: no hubo acceso a ese titular, así que no hay nada que anotar.
        Ok(false) => return,
        // Si no se puede saber, no se sella. Un eslabón que quizá sea falso vale
        // menos que ninguno: el rastro se lee para responder «¿quién miró mi
        // historia?», y ahí una duda sellada es una respuesta equivocada.
        Err(err) => {
            eprintln!(
                "⚠️  bitácora de acceso: no se registró {accion} de {recurso} (paciente {paciente_id}): no se pudo comprobar el ámbito: {err}"
            );
            return;
        }
    }
    let alcance = alcance_visto_por(u, filtro, n, vec![]);
    registrar_conjunto(
        e,
        u,
        accion,
        recurso,
        acceso::Objeto::DeUnTitular {
            paciente_id,
            alcance,
        },
    )
    .await;
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
    autorizado: Autorizado<EscribirClinico>,
    Path(id): Path<i64>,
    Json(h): Json<HistoriaNueva>,
) -> Response {
    let u = autorizado.0;
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
    // `Autorizado<Firmar>`: la firma afirma autoría clínica, así que la exige la
    // FIRMA del manejador y no un `if` que la próxima ruta olvidará.
    autorizado: Autorizado<Firmar>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
    match historia::firmar(&e.pool, u.ambito(), id, u.id, e.clave_firma.as_deref()).await {
        Ok(historia::ResultadoFirma::Firmada) => StatusCode::NO_CONTENT,
        Ok(historia::ResultadoFirma::NoSePudo) => StatusCode::NOT_FOUND,
        // Hay clave configurada y no sirve: es un fallo del DESPLIEGUE, no de la
        // petición, y no se disfraza de 404. Antes esto marcaba la historia como
        // firmada con `firma = NULL` y devolvía 204: una historia que afirmaba
        // estar firmada sin estarlo.
        Ok(historia::ResultadoFirma::ClaveInservible) => {
            eprintln!("⚠️  MEDICO_SIGNING_KEY no sirve: la historia {id} NO se firmó");
            StatusCode::INTERNAL_SERVER_ERROR
        }
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
    autorizado: Autorizado<Borrar>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
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
        Ok(Some(paciente)) => {
            registrar_acceso(&e, &u, "ver", "paciente", paciente_id, paciente_id).await;
            Ok(Json(Contexto {
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
            }))
        }
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
    let hs = historia::listar_por_paciente(&e.pool, u.ambito(), paciente_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    registrar_coleccion_de_un_titular(&e, &u, "listar", "historias", paciente_id, None, hs.len())
        .await;
    Ok(Json(hs))
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
    let ds = documento::listar_ambito(&e.pool, u.ambito(), f.paciente_id, f.tipo.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // El mismo endpoint cae en DOS formas según lo que se pida, y es la regla la
    // que decide, no el caso: con `paciente_id` es el expediente de un titular y
    // entra en SU rastro; sin él atraviesa titulares y es una colección.
    let filtro = f.tipo.as_ref().map(|t| format!("tipo={t}"));
    match f.paciente_id {
        Some(p) => {
            registrar_coleccion_de_un_titular(&e, &u, "listar", "documentos", p, filtro, ds.len())
                .await
        }
        None => registrar_coleccion(&e, &u, "listar", "documentos", filtro, ds.len(), vec![]).await,
    }
    Ok(Json(ds))
}

/// Crea un documento. El autor es el usuario autenticado —no un campo del
/// cliente—. 201 con su id; 403 si el paciente no es del ámbito.
async fn crear_documento(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Json(d): Json<DocumentoNuevo>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<AtenderUrgencias>,
    Json(n): Json<TriageNuevo>,
) -> Response {
    let u = autorizado.0;
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
    /// Los tres diagnósticos del RIPS. Van aquí y no en el triage porque los
    /// pone el MÉDICO, y el egreso es el único momento en que un médico toca la
    /// fila en este módulo. **Sin ellos el AU no se puede completar nunca**:
    /// `codDiagnosticoPrincipal` es obligatorio y la 948 exige la causa básica
    /// de muerte cuando el paciente muere. Lo que se omite no borra lo que ya
    /// hubiera.
    diagnostico_ingreso: Option<String>,
    diagnostico_egreso: Option<String>,
    diagnostico_causa_muerte: Option<String>,
}

async fn urgencias_egreso(
    State(e): State<Estado>,
    autorizado: Autorizado<AtenderUrgencias>,
    Path(id): Path<i64>,
    Json(b): Json<EgresoBody>,
) -> StatusCode {
    let u = autorizado.0;
    let datos = urgencias::Egreso {
        destino: &b.destino,
        observaciones: b.observaciones.as_deref(),
        diagnostico_ingreso: b.diagnostico_ingreso.as_deref(),
        diagnostico_egreso: b.diagnostico_egreso.as_deref(),
        diagnostico_causa_muerte: b.diagnostico_causa_muerte.as_deref(),
    };
    match urgencias::egreso(&e.pool, u.ambito(), id, &datos).await {
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
    autorizado: Autorizado<AtenderUrgencias>,
    Json(i): Json<Ingreso>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<AtenderUrgencias>,
    Path(id): Path<i64>,
    Json(eg): Json<Egreso>,
) -> StatusCode {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirNutricion>,
    Json(v): Json<ValoracionCrear>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirNutricion>,
    Path(id): Path<i64>,
    Json(v): Json<ValoracionEditar>,
) -> Response {
    let u = autorizado.0;
    match nutricion::actualizar_valoracion(&e.pool, u.ambito(), id, &v).await {
        Ok(true) => match nutricion::obtener_valoracion(&e.pool, u.ambito(), id).await {
            Ok(Some(val)) => Json(val).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Borra una valoración nutricional. **Exige la misma capacidad que escribirla.**
///
/// Llevaba `Autenticado` pelado, y era el ÚNICO de los diez `delete(...)` del
/// router con extractor sin capacidad: el commit que puso
/// `Autorizado<EscribirNutricion>` en las cuatro escrituras hermanas se dejó el
/// borrado. El resultado era que un laboratorista, un terapeuta o un regente de
/// farmacia recibían 403 al crear o editar la valoración y **204 al
/// destruirla** — recorriendo ids, la institución entera, y con cada valoración
/// su plan detrás (`0018_nutricion.sql`: `ON DELETE CASCADE`). Irreversible.
///
/// Se exige `EscribirNutricion` y no `Borrar`: el criterio del resto del router
/// es que borrar un recurso pide la capacidad de ESE recurso —laboratorio pide
/// `EscribirLaboratorio`, farmacia `GestionarFarmacia`—, y `Borrar` está
/// reservada a la historia y los documentos clínicos. Además así lo puede hacer
/// el nutricionista, que es quien la escribió.
async fn nutri_eliminar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirNutricion>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirNutricion>,
    Path(id): Path<i64>,
    Json(p): Json<PlanCrear>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirNutricion>,
    Path(id): Path<i64>,
    Json(p): Json<PlanEditar>,
) -> StatusCode {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirRehabilitacion>,
    Json(p): Json<rehabilitacion::PlanCrear>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirRehabilitacion>,
    Path(id): Path<i64>,
    Json(p): Json<rehabilitacion::PlanEditar>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirRehabilitacion>,
    Path(id): Path<i64>,
    Json(s): Json<SesionCrear>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirRehabilitacion>,
    Path(id): Path<i64>,
    Json(s): Json<SesionEditar>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<GestionarRemision>,
    Path(id): Path<i64>,
    Json(b): Json<EstadoBody>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<GestionarRemision>,
    Json(c): Json<ContrarreferenciaCrear>,
) -> Response {
    let u = autorizado.0;
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

async fn cr_recibir(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarRemision>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirClinico>,
    Path(id): Path<i64>,
    Json(d): Json<DocumentoEdicion>,
) -> Response {
    let u = autorizado.0;
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
    autorizado: Autorizado<Firmar>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
    match documento::firmar(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn eliminar_documento(
    State(e): State<Estado>,
    autorizado: Autorizado<Borrar>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
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
    autorizado: Autorizado<EscribirClinico>,
    Path(historia_id): Path<i64>,
    Json(f): Json<FormulaNueva>,
) -> Response {
    let u = autorizado.0;
    match formula::crear(&e.pool, u.ambito(), historia_id, &f).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn actualizar_formula(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Path(id): Path<i64>,
    Json(f): Json<FormulaNueva>,
) -> StatusCode {
    let u = autorizado.0;
    match formula::actualizar(&e.pool, u.ambito(), id, &f).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn eliminar_formula(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    match formula::eliminar(&e.pool, u.ambito(), id).await {
        Ok(formula::ResultadoBorrado::Hecho) => StatusCode::NO_CONTENT.into_response(),
        Ok(formula::ResultadoBorrado::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(formula::ResultadoBorrado::YaDispensada { entregas }) => detalle(
            StatusCode::CONFLICT,
            format!(
                "Esta fórmula ya tiene {entregas} entrega(s) registradas en farmacia: borrarla \
                 dejaría la dispensación sin prescripción que la respalde. Corríjala con una \
                 fórmula nueva."
            ),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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
    autorizado: Autorizado<EscribirClinico>,
    Path(historia_id): Path<i64>,
    Json(o): Json<OrdenNueva>,
) -> Response {
    let u = autorizado.0;
    match orden::crear(&e.pool, u.ambito(), historia_id, &o).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cambiar_estado_orden(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Path(id): Path<i64>,
    Json(req): Json<CambioEstado>,
) -> StatusCode {
    let u = autorizado.0;
    match orden::cambiar_estado(&e.pool, u.ambito(), id, &req.estado).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn eliminar_orden(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirClinico>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
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
    multipart: Multipart,
) -> Response {
    match guardar_imagen_de(&e, &u, historia_id, multipart, "").await {
        Ok(id) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Err(codigo) => codigo.into_response(),
    }
}

/// Guarda el archivo de un multipart como imagen de `historia_id` y devuelve su
/// id. **Es el único camino por el que entran bytes al sistema**, y por eso lo
/// comparten el adjunto de la historia clínica y la imagen de un informe
/// radiológico: un segundo almacén con sus propias reglas sería el sitio por
/// donde entrara el SVG que `tipos.rs` existe para no admitir.
///
/// Acepta el archivo venga en el campo que venga —se toma el primero que traiga
/// nombre de archivo—. La pantalla de la historia lo llama `archivo` y la de
/// imágenes `file`; exigir uno de los dos habría hecho que la otra fallara con
/// un 400 que no dice nada, y el nombre del campo no es una comprobación de
/// seguridad: la que cuenta es la de los bytes, dos párrafos más abajo.
async fn guardar_imagen_de(
    e: &Estado,
    u: &Usuario,
    historia_id: i64,
    mut multipart: Multipart,
    descripcion_por_defecto: &str,
) -> Result<i64, StatusCode> {
    let mut archivo: Option<(String, Vec<u8>)> = None; // nombre, bytes
    let mut descripcion = descripcion_por_defecto.to_owned();
    loop {
        match multipart.next_field().await {
            Ok(Some(campo)) => {
                if campo.name() == Some("descripcion") {
                    descripcion = campo.text().await.unwrap_or_default();
                    continue;
                }
                let Some(nombre) = campo.file_name().map(str::to_owned) else {
                    continue;
                };
                if archivo.is_some() {
                    continue;
                }
                match campo.bytes().await {
                    Ok(b) => archivo = Some((nombre, b.to_vec())),
                    // El único fallo esperable al leer el campo es que exceda el
                    // tope del `DefaultBodyLimit`.
                    Err(_) => return Err(StatusCode::PAYLOAD_TOO_LARGE),
                }
            }
            Ok(None) => break,
            Err(_) => return Err(StatusCode::BAD_REQUEST),
        }
    }

    let Some((nombre, bytes)) = archivo else {
        return Err(StatusCode::BAD_REQUEST);
    };
    if bytes.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    // El tipo, de los BYTES. Lo que el cliente declaró se descarta aquí mismo: no
    // se compara siquiera con lo olfateado, porque no aporta nada — si coincide
    // no añade garantía y si discrepa la buena es la nuestra.
    let Some(tipo) = crate::tipos::tipo_real(&bytes) else {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    };

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
        Ok(None) => return Err(StatusCode::FORBIDDEN),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    // Los bytes van al disco DESPUÉS de la fila: si la escritura falla, se
    // deshace la fila para no dejar una referencia a un archivo que no existe.
    let destino = e.dir_imagenes.join(&img.ruta_archivo);
    if tokio::fs::write(&destino, &para_disco).await.is_err() {
        let _ = imagen::eliminar(&e.pool, u.ambito(), img.id).await;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(img.id)
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
    // `Borrar`, igual que `eliminar_documento`. Escapó a la primera pasada: el
    // producto ya había decidido que borrar un registro clínico es de
    // superadmin, admin o médico, y por esta puerta un laboratorista sí podía
    // borrar la radiografía de un paciente aunque no pudiera borrar un
    // documento. Misma decisión, un camino que se había quedado fuera.
    //
    // SUBIR una imagen sigue pidiendo solo autenticación, y a propósito: el
    // radiólogo es justo quien las sube y no está en esos tres roles. Borrar y
    // subir no son el mismo acto, y `/imagenes` no lleva `roleGuard` en
    // `app.routes.ts`, así que el producto no ha declarado quién sube.
    autorizado: Autorizado<Borrar>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
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
    autorizado: Autorizado<GestionarUsuarios>,
) -> Result<Json<Vec<ResumenUsuario>>, StatusCode> {
    let u = autorizado.0;
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
    /// P11/P12: el documento de quien atiende, que el RIPS exige en los cinco
    /// archivos. Opcionales los dos, y **o los dos o ninguno** — ver abajo.
    tipo_documento: Option<String>,
    numero_documento: Option<String>,
}

/// El documento del profesional (P11/P12), validado, o la respuesta que lo
/// rechaza.
///
/// UNA sola escritura de la regla, compartida por el alta y por la corrección.
/// Dos copias es lo que dejó dos pantallas muertas con `orden.estado`: la
/// segunda se queda atrás en cuanto una cambia.
///
/// Las dos reglas, y las dos son del documento y no del gusto:
///
/// - **o los dos campos o ninguno**. Medio documento no identifica a nadie, y
///   aceptar solo el número dejaría una fila que PARECE traer el dato
///   (directiva 20). `None` en los dos es válido y significa «no lo tengo»:
///   sin él, el recolector anota la falta y el cierre del mes la enseña.
/// - **el tipo sale del maestro del validador**, no de una lista escrita aquí.
///   `RC` y `TI` están en el catálogo —un paciente puede tenerlos— pero un
///   menor de edad no firma un acto médico. Es el campo que ya se equivocó una
///   vez mandando el documento del PACIENTE: el MUV lo destapó rechazando a los
///   menores, y las atenciones de adultos pasaban.
///
/// Devuelve el MOTIVO del rechazo y no una `Response` ya pintada: así el tipo de
/// error es pequeño (clippy se queja, con razón, de un `Err` del tamaño de una
/// respuesta HTTP) y quien llama decide cómo la presenta.
fn documento_del_profesional(
    tipo: &Option<String>,
    numero: &Option<String>,
) -> Result<Option<(String, String)>, (StatusCode, String)> {
    match (tipo, numero) {
        (None, None) => Ok(None),
        (Some(t), Some(n)) if t.trim().is_empty() && n.trim().is_empty() => Ok(None),
        (Some(t), Some(n)) if !t.trim().is_empty() && !n.trim().is_empty() => {
            if !medico_dominio::muv::admite_como_profesional(t.trim()) {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!(
                        "«{}» no es un tipo de documento válido para quien realiza la atención. \
                         La Resolución 948 admite: {}.",
                        t.trim(),
                        medico_dominio::muv::tipos_documento_profesional_listados()
                    ),
                ));
            }
            Ok(Some((t.trim().to_string(), n.trim().to_string())))
        }
        _ => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "el documento del profesional va completo —tipo y número— o no va: medio \
             documento no identifica a nadie."
                .to_string(),
        )),
    }
}

async fn crear_usuario(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarUsuarios>,
    Json(req): Json<CrearUsuario>,
) -> Response {
    let u = autorizado.0;
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
        // El día del prestador, no el del servidor: una licencia que vence hoy
        // no puede darse por vencida porque el reloj de la máquina ya pasó la
        // medianoche en otro huso.
        let hoy = medico_dominio::zona::hoy().to_string();
        match licencia::puerta_para_usuario(&e.pool, id, &hoy).await {
            Ok(licencia::Puerta::Abierta) => {}
            Ok(bloqueo) => {
                return (StatusCode::PAYMENT_REQUIRED, Json(Bloqueo { motivo: motivo(bloqueo) }))
                    .into_response();
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
    // EL DOCUMENTO DEL PROFESIONAL (P11/P12), que hasta hoy no tenía por dónde
    // entrar: las columnas existen desde `0037` y ninguna ruta las escribía, así
    // que el RIPS las anotaba como ausentes en TODA atención y nadie podía
    // arreglarlo. Es el mismo patrón que `finalidad` y `causa_externa` —campo
    // obligatorio que ninguna pantalla ofrecía—, con la diferencia de que aquí
    // no había un defecto tapando el hueco.
    //
    // Van los dos o ninguno. Medio documento no identifica a nadie, y aceptar
    // solo el número dejaría una fila que PARECE tener el dato (directiva 20).
    let documento = match documento_del_profesional(&req.tipo_documento, &req.numero_documento) {
        Ok(d) => d,
        Err((c, m)) => return detalle(c, m),
    };
    let Some(hash) = claves::hashear(&req.clave) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let nuevo = UsuarioNuevo {
        nombre: req.nombre,
        email: req.email,
        password_hash: hash,
        rol: req.rol,
        institucion_id: req.institucion_id,
        documento,
    };
    match usuario::crear(&e.pool, u.ambito(), &nuevo).await {
        Ok(usuario::ResultadoCrear::Creado(id)) => {
            (StatusCode::CREATED, Json(Creado { id })).into_response()
        }
        Ok(usuario::ResultadoCrear::SinPermiso) => StatusCode::FORBIDDEN.into_response(),
        Ok(usuario::ResultadoCrear::EmailEnUso) => StatusCode::CONFLICT.into_response(),
        Ok(usuario::ResultadoCrear::DocumentoEnUso) => detalle(
            StatusCode::CONFLICT,
            "ese documento ya es de otro profesional.".to_string(),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct DocumentoProfesional {
    tipo_documento: Option<String>,
    numero_documento: Option<String>,
}

/// Corrige el documento de identidad de un profesional que YA existe (P11/P12).
///
/// # Por qué una ruta que hace SOLO esto
///
/// Las columnas existen desde `0037` y el alta las escribe desde hoy, pero
/// **todos los usuarios dados de alta antes no tenían por dónde recibirlas**: el
/// RIPS los anotaba como ausentes en toda atención y no había forma de
/// arreglarlo desde el producto. Eso es lo que cierra esta ruta.
///
/// Podría haber sido un `PATCH /api/usuarios/{id}` general, y sería la puerta
/// por la que se cambia el rol o la clave de otro: `crear_usuario` tiene la
/// regla de que solo un superadmin fabrica otro superadmin, y un actualizador
/// general necesitaría esa y varias más, cada una una decisión aparte. Esta no
/// puede hacer nada más que lo que dice su nombre.
///
/// Enviar los dos campos vacíos **borra** el documento, y es deliberado:
/// corregir un dedazo dejando la fila sin documento es mejor que dejarla con
/// uno falso, que es justo lo que P11/P12 enseñó a no hacer.
///
/// **404 y no 403** para un usuario de otra institución: un 403 confirma que el
/// id existe, y con eso se enumera la plantilla del vecino.
async fn fijar_documento_usuario(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarUsuarios>,
    Path(id): Path<i64>,
    Json(req): Json<DocumentoProfesional>,
) -> Response {
    let u = autorizado.0;
    let documento = match documento_del_profesional(&req.tipo_documento, &req.numero_documento) {
        Ok(d) => d,
        Err((c, m)) => return detalle(c, m),
    };
    let par = documento.as_ref().map(|(t, n)| (t.as_str(), n.as_str()));
    match usuario::fijar_documento(&e.pool, u.ambito(), id, par).await {
        Ok(usuario::ResultadoDocumento::Actualizado) => StatusCode::NO_CONTENT.into_response(),
        Ok(usuario::ResultadoDocumento::NoEncontrado) => StatusCode::NOT_FOUND.into_response(),
        Ok(usuario::ResultadoDocumento::EnUso) => detalle(
            StatusCode::CONFLICT,
            "ese documento ya es de otro profesional.".to_string(),
        ),
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

// ─── Agenda (#237) ──────────────────────────────────────────────────────────
//
// Sin esto, «Agenda Médica» del menú no cargaba nada contra el binario y
// `getSlots()` de `cita.service.ts` devolvía `of([])`: la hora de la cita se
// tecleaba a mano, sin que nada comprobara que el profesional atiende ese día ni
// que la hora estuviera libre.

/// Los profesionales a los que se puede agendar. Cualquiera con sesión: es la
/// lista del desplegable de «agendar cita».
async fn agenda_medicos(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match agenda::medicos(&e.pool, u.ambito()).await {
        Ok(ms) => Json(ms).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct FiltroTurnos {
    medico_id: i64,
    fecha: String,
}

/// Los turnos de un profesional un día. Lista vacía = **no atiende ese día**,
/// que no es lo mismo que «está lleno»; el cliente lo distingue porque en el
/// segundo caso vienen turnos con `disponible: false` y su motivo.
async fn agenda_turnos(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroTurnos>,
) -> Response {
    match agenda::turnos(&e.pool, u.ambito(), f.medico_id, &f.fecha).await {
        Ok(ts) => Json(ts).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct FiltroIndicadores {
    medico_id: Option<i64>,
    desde: Option<String>,
    hasta: Option<String>,
}

/// Indicadores del periodo. Sin fechas, el mes en curso hasta hoy.
async fn agenda_indicadores(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroIndicadores>,
) -> Response {
    let hoy = medico_dominio::zona::hoy();
    let desde = f.desde.unwrap_or_else(|| hoy.format("%Y-%m-01").to_string());
    let hasta = f.hasta.unwrap_or_else(|| hoy.to_string());
    match agenda::indicadores(&e.pool, u.ambito(), f.medico_id, &desde, &hasta).await {
        Ok(i) => Json(i).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct FiltroAgendas {
    medico_id: Option<i64>,
    anio: Option<i64>,
    mes: Option<i64>,
}

async fn agenda_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroAgendas>,
) -> Response {
    match agenda::listar(&e.pool, u.ambito(), f.medico_id, f.anio, f.mes).await {
        Ok(a) => Json(a).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn agenda_obtener(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Response {
    match agenda::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(a)) => Json(a).into_response(),
        // 404 y no 403: un 403 confirmaría que el id existe en otra institución.
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn agenda_crear(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Json(nueva): Json<AgendaNueva>,
) -> Response {
    let u = autorizado.0;
    match agenda::crear(&e.pool, u.ambito(), u.id, &nueva).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        // Médico ajeno, mes repetido, u hora ilegible. No se distinguen: las tres
        // son «esta agenda no se puede crear» y separarlas diría qué ids existen.
        Ok(None) => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct AgendaEdicion {
    activa: Option<bool>,
    observaciones: Option<String>,
}

async fn agenda_actualizar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Path(id): Path<i64>,
    Json(req): Json<AgendaEdicion>,
) -> StatusCode {
    match agenda::actualizar(
        &e.pool,
        autorizado.0.ambito(),
        id,
        req.activa,
        req.observaciones.as_deref(),
    )
    .await
    {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn agenda_borrar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Path(id): Path<i64>,
) -> StatusCode {
    match agenda::borrar(&e.pool, autorizado.0.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn agenda_anadir_bloque(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Path(id): Path<i64>,
    Json(b): Json<BloqueNuevo>,
) -> Response {
    match agenda::anadir_bloque(&e.pool, autorizado.0.ambito(), id, &b).await {
        Ok(Some(id)) => (StatusCode::CREATED, Json(Creado { id })).into_response(),
        Ok(None) => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct BloqueEdicion {
    activo: bool,
}

async fn agenda_activar_bloque(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Path((id, bloque_id)): Path<(i64, i64)>,
    Json(req): Json<BloqueEdicion>,
) -> StatusCode {
    match agenda::activar_bloque(&e.pool, autorizado.0.ambito(), id, bloque_id, req.activo).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn agenda_borrar_bloque(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Path((id, bloque_id)): Path<(i64, i64)>,
) -> StatusCode {
    match agenda::borrar_bloque(&e.pool, autorizado.0.ambito(), id, bloque_id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Deserialize)]
struct FiltroBloqueos {
    medico_id: Option<i64>,
    desde: Option<String>,
    hasta: Option<String>,
}

async fn agenda_listar_bloqueos(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroBloqueos>,
) -> Response {
    match agenda::listar_bloqueos(
        &e.pool,
        u.ambito(),
        f.medico_id,
        f.desde.as_deref(),
        f.hasta.as_deref(),
    )
    .await
    {
        Ok(bs) => Json(bs).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn agenda_crear_bloqueo(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Json(b): Json<BloqueoNuevo>,
) -> Response {
    let u = autorizado.0;
    match agenda::crear_bloqueo(&e.pool, u.ambito(), u.id, &b).await {
        // Devuelve además cuántas citas quedan dentro: el bloqueo NO las
        // cancela, y quien coordina tiene que reasignarlas.
        Ok(Some(r)) => (StatusCode::CREATED, Json(r)).into_response(),
        Ok(None) => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn agenda_borrar_bloqueo(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAgenda>,
    Path(id): Path<i64>,
) -> StatusCode {
    match agenda::borrar_bloqueo(&e.pool, autorizado.0.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

/// `GET /api/finalidades` — las 34 finalidades de la Resolución 948.
///
/// Van empotradas en el binario (`medico_dominio::muv`), no en la base: son la
/// tabla del validador y no cambian por institución. Y se sirven completas, con
/// `aplica_ac` y `aplica_ap`, para que la pantalla ofrezca las que valen en cada
/// documento en vez de las 34 siempre — unas cuantas solo aplican a
/// procedimientos.
///
/// Basta con estar autenticado: es un catálogo público de MinSalud y no dice
/// nada de ningún paciente.
async fn listar_finalidades(_: Autenticado) -> Json<&'static [medico_dominio::muv::Finalidad]> {
    Json(medico_dominio::muv::finalidades())
}

/// `GET /api/causas-motivo` — las 29 causas o motivos de atención de la 948.
async fn listar_causas_motivo(_: Autenticado) -> Json<&'static [medico_dominio::muv::CausaMotivo]> {
    Json(medico_dominio::muv::causas_motivo())
}

/// Un tipo de documento admitido para quien REALIZA la atención (P11/P12).
#[derive(Serialize)]
struct TipoDocumentoProfesional {
    codigo: &'static str,
    nombre: &'static str,
}

/// `GET /api/tipos-documento-profesional` — los ocho de la 948.
///
/// Existe para que la pantalla que corrige el documento OFREZCA la lista en vez
/// de escribirla. Ningún código de catálogo se escribe a mano en este proyecto:
/// una lista en el frontend y otra en `muv.rs` divergen en cuanto el maestro
/// cambie, y la del navegador sería la que ve el médico.
///
/// Son ocho y no los trece del maestro: `RC` y `TI` valen para un PACIENTE pero
/// no para quien firma el acto médico. Ese filtro vive en el dominio.
async fn listar_tipos_documento_profesional(
    _: Autenticado,
) -> Json<Vec<TipoDocumentoProfesional>> {
    Json(
        medico_dominio::muv::tipos_documento_profesional()
            .iter()
            .map(|(c, n)| TipoDocumentoProfesional {
                codigo: c.as_str(),
                nombre: n.as_str(),
            })
            .collect(),
    )
}

// --- Laboratorio e imágenes diagnósticas -------------------------------------
//
// Las dos bandejas comparten forma: listar órdenes del servicio, abrir una,
// mover su estado, y escribir la respuesta. Lo que cambia es la respuesta —
// parámetros con referencias contra informe redactado— y la capacidad que se
// exige. LEER no exige capacidad: el médico que pidió el examen tiene que poder
// ver el resultado de su paciente.

#[derive(Deserialize)]
struct FiltroOrdenesDx {
    estado: Option<String>,
    paciente_id: Option<i64>,
}

/// Traduce el resultado de una escritura que puede chocar con algo cerrado.
fn responder_escritura<T: Serialize>(
    r: diagnostico::Escritura<T>,
    creado: bool,
    cerrado: &str,
) -> Response {
    match r {
        diagnostico::Escritura::Hecho(v) => {
            let codigo = if creado { StatusCode::CREATED } else { StatusCode::OK };
            (codigo, Json(v)).into_response()
        }
        diagnostico::Escritura::NoExiste => StatusCode::NOT_FOUND.into_response(),
        diagnostico::Escritura::Cerrado => detalle(StatusCode::CONFLICT, cerrado.to_owned()),
        diagnostico::Escritura::EstadoInvalido => detalle(
            StatusCode::BAD_REQUEST,
            format!(
                "Estado no válido. Los del producto son: {}.",
                diagnostico::ESTADOS.join(", ")
            ),
        ),
    }
}

const LAB_CERRADO: &str = "Este resultado ya está validado y no se modifica: sobre él \
    puede haberse decidido una conducta. Emita uno nuevo que lo corrija.";
const INFORME_CERRADO: &str = "Este informe ya está firmado y no se modifica. Emita uno \
    nuevo que lo corrija.";

async fn lab_ordenes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroOrdenesDx>,
) -> Response {
    match diagnostico::listar_ordenes(
        &e.pool, u.ambito(), diagnostico::Servicio::Laboratorio,
        f.estado.as_deref(), f.paciente_id,
    ).await {
        Ok(os) => Json(os).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_orden(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match diagnostico::obtener_orden(&e.pool, u.ambito(), diagnostico::Servicio::Laboratorio, id).await {
        Ok(Some(o)) => Json(o).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_estado(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirLaboratorio>,
    Path(id): Path<i64>,
    Json(c): Json<CambioEstado>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::cambiar_estado_orden(
        &e.pool, u.ambito(), diagnostico::Servicio::Laboratorio, id, &c.estado,
    ).await {
        Ok(r) => responder_escritura(r, false, LAB_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_resultado(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match diagnostico::resultado_de_orden(&e.pool, u.ambito(), id).await {
        Ok(Some(r)) => Json(r).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_crear_resultado(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirLaboratorio>,
    Path(id): Path<i64>,
    Json(r): Json<diagnostico::ResultadoNuevo>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::crear_resultado(&e.pool, u.ambito(), id, u.id, &r).await {
        Ok(res) => responder_escritura(res, true, LAB_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_actualizar_resultado(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirLaboratorio>,
    Path(id): Path<i64>,
    Json(r): Json<diagnostico::ResultadoNuevo>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::actualizar_resultado(&e.pool, u.ambito(), id, &r).await {
        Ok(res) => responder_escritura(res, false, LAB_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_validar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirLaboratorio>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::validar_resultado(&e.pool, u.ambito(), id, u.id).await {
        Ok(res) => responder_escritura(res, false, LAB_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_historial(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match diagnostico::resultados_de_paciente(&e.pool, u.ambito(), id).await {
        Ok(rs) => {
            registrar_coleccion_de_un_titular(
                &e,
                &u,
                "listar",
                "resultados_laboratorio",
                id,
                None,
                rs.len(),
            )
            .await;
            Json(rs).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_perfiles(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match diagnostico::listar_perfiles(&e.pool, u.ambito()).await {
        Ok(ps) => Json(ps).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_crear_perfil(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirLaboratorio>,
    Json(p): Json<diagnostico::PerfilNuevo>,
) -> Response {
    let u = autorizado.0;
    let Some(institucion_id) = u.institucion_id else {
        return detalle(
            StatusCode::BAD_REQUEST,
            "Los perfiles de examen son de una institución —sus referencias dependen del método y \
             del equipo— y esta cuenta no está adscrita a ninguna."
                .into(),
        );
    };
    match diagnostico::crear_perfil(&e.pool, u.ambito(), institucion_id, &p).await {
        Ok(diagnostico::ResultadoPerfil::Creado(perfil)) => {
            (StatusCode::CREATED, Json(*perfil)).into_response()
        }
        Ok(diagnostico::ResultadoPerfil::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(diagnostico::ResultadoPerfil::SinNombre) => detalle(
            StatusCode::BAD_REQUEST,
            "El perfil necesita un nombre.".into(),
        ),
        Ok(diagnostico::ResultadoPerfil::NombreRepetido(n)) => detalle(
            StatusCode::CONFLICT,
            format!("Ya hay un perfil llamado «{n}» en esta institución."),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn lab_eliminar_perfil(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirLaboratorio>,
    Path(id): Path<i64>,
) -> StatusCode {
    let u = autorizado.0;
    match diagnostico::eliminar_perfil(&e.pool, u.ambito(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn img_ordenes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroOrdenesDx>,
) -> Response {
    match diagnostico::listar_ordenes(
        &e.pool, u.ambito(), diagnostico::Servicio::Imagenologia,
        f.estado.as_deref(), f.paciente_id,
    ).await {
        Ok(os) => Json(os).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_orden(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match diagnostico::obtener_orden(&e.pool, u.ambito(), diagnostico::Servicio::Imagenologia, id).await {
        Ok(Some(o)) => Json(o).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_estado(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirImagenologia>,
    Path(id): Path<i64>,
    Json(c): Json<CambioEstado>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::cambiar_estado_orden(
        &e.pool, u.ambito(), diagnostico::Servicio::Imagenologia, id, &c.estado,
    ).await {
        Ok(r) => responder_escritura(r, false, INFORME_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_informe(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match diagnostico::informe_de_orden(&e.pool, u.ambito(), id).await {
        Ok(Some(i)) => Json(i).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_crear_informe(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirImagenologia>,
    Path(id): Path<i64>,
    Json(i): Json<diagnostico::InformeNuevo>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::crear_informe(&e.pool, u.ambito(), id, u.id, &i).await {
        Ok(r) => responder_escritura(r, true, INFORME_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_actualizar_informe(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirImagenologia>,
    Path(id): Path<i64>,
    Json(i): Json<diagnostico::InformeNuevo>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::actualizar_informe(&e.pool, u.ambito(), id, &i).await {
        Ok(r) => responder_escritura(r, false, INFORME_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_firmar(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirImagenologia>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    match diagnostico::firmar_informe(&e.pool, u.ambito(), id, u.id).await {
        Ok(r) => responder_escritura(r, false, INFORME_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn img_historial(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match diagnostico::informes_de_paciente(&e.pool, u.ambito(), id).await {
        Ok(is) => {
            registrar_coleccion_de_un_titular(
                &e,
                &u,
                "listar",
                "informes_imagenologia",
                id,
                None,
                is.len(),
            )
            .await;
            Json(is).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Adjunta una imagen al informe de una orden de imagenología.
///
/// Los bytes NO se guardan aquí de otra manera: se registran por el mismo camino
/// que los adjuntos de la historia clínica —tipo deducido de la firma de los
/// bytes, tope de tamaño, cifrado en reposo— y al informe solo llega la ruta que
/// los sirve. Un segundo almacén, con sus propias reglas, habría sido el sitio
/// por donde entrase el SVG que `tipos.rs` existe para no admitir.
async fn img_subir(
    State(e): State<Estado>,
    autorizado: Autorizado<EscribirImagenologia>,
    Path(orden_id): Path<i64>,
    multipart: Multipart,
) -> Response {
    let u = autorizado.0;
    // La orden tiene que ser suya y de imagenología, y su informe estar abierto,
    // ANTES de escribir nada en el disco.
    let Ok(Some(orden)) =
        diagnostico::obtener_orden(&e.pool, u.ambito(), diagnostico::Servicio::Imagenologia, orden_id).await
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match diagnostico::informe_de_orden(&e.pool, u.ambito(), orden_id).await {
        Ok(Some(i)) if i.firmado => return detalle(StatusCode::CONFLICT, INFORME_CERRADO.into()),
        Ok(Some(_)) => {}
        // Sin informe todavía no hay dónde colgar la imagen. Crearlo vacío aquí
        // dejaría un informe sin técnica ni hallazgos que parece redactado.
        Ok(None) => {
            return detalle(
                StatusCode::CONFLICT,
                "Redacte el informe antes de adjuntarle imágenes: una imagen suelta no dice qué \
                 se vio en ella."
                    .into(),
            )
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let id_imagen = match guardar_imagen_de(&e, &u, orden.historia_id, multipart, "Imagen diagnóstica").await {
        Ok(id) => id,
        Err(codigo) => return codigo.into_response(),
    };
    match diagnostico::adjuntar_imagen(&e.pool, u.ambito(), orden_id, &format!("/api/imagenes/{id_imagen}/archivo")).await {
        Ok(r) => responder_escritura(r, false, INFORME_CERRADO),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// --- Ambulancias y traslados -------------------------------------------------
//
// Ver el parque solo pide sesión: saber si hay una ambulancia disponible es lo
// primero que necesita quien atiende una urgencia. Darlas de alta y despachar
// exigen `GestionarAmbulancias`.

fn respuesta_ambulancia(r: ambulancia::ResultadoAlta, creada: bool) -> Response {
    match r {
        ambulancia::ResultadoAlta::Hecho(a) => {
            let codigo = if creada { StatusCode::CREATED } else { StatusCode::OK };
            (codigo, Json(*a)).into_response()
        }
        ambulancia::ResultadoAlta::NoExiste => StatusCode::NOT_FOUND.into_response(),
        ambulancia::ResultadoAlta::SinPlaca => detalle(
            StatusCode::BAD_REQUEST,
            "La ambulancia necesita placa: es lo que la identifica ante el tránsito y ante el \
             asegurador."
                .into(),
        ),
        ambulancia::ResultadoAlta::TipoInvalido => detalle(
            StatusCode::BAD_REQUEST,
            format!(
                "El tipo tiene que ser {} — las dos modalidades del transporte asistencial.",
                ambulancia::TIPOS.join(" o ")
            ),
        ),
        ambulancia::ResultadoAlta::EstadoInvalido => detalle(
            StatusCode::CONFLICT,
            "«En Traslado» lo pone el despacho y lo quita el cierre del traslado, no la edición: \
             si se pudiera fijar a mano, el estado dejaría de decir dónde está el vehículo."
                .into(),
        ),
        ambulancia::ResultadoAlta::PlacaRepetida(p) => detalle(
            StatusCode::CONFLICT,
            format!("Ya hay una ambulancia con placa {p} en esta institución."),
        ),
    }
}

async fn amb_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<ambulancia::FiltrosAmbulancia>,
) -> Response {
    match ambulancia::listar(&e.pool, u.ambito(), &f, &ambulancia::hoy()).await {
        Ok(a) => Json(a).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_obtener(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match ambulancia::obtener(&e.pool, u.ambito(), id, &ambulancia::hoy()).await {
        Ok(Some(a)) => Json(a).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_crear(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAmbulancias>,
    Json(a): Json<ambulancia::AmbulanciaNueva>,
) -> Response {
    let u = autorizado.0;
    let Some(institucion_id) = u.institucion_id else {
        return detalle(
            StatusCode::BAD_REQUEST,
            "El parque de ambulancias es de una institución y esta cuenta no está adscrita a \
             ninguna."
                .into(),
        );
    };
    match ambulancia::crear(&e.pool, u.ambito(), institucion_id, &a, &ambulancia::hoy()).await {
        Ok(r) => respuesta_ambulancia(r, true),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_actualizar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAmbulancias>,
    Path(id): Path<i64>,
    Json(cambios): Json<ambulancia::AmbulanciaEdicion>,
) -> Response {
    let u = autorizado.0;
    match ambulancia::actualizar(&e.pool, u.ambito(), id, &cambios, &ambulancia::hoy()).await {
        Ok(r) => respuesta_ambulancia(r, false),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_eliminar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAmbulancias>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    match ambulancia::eliminar(&e.pool, u.ambito(), id).await {
        Ok(ambulancia::ResultadoBorrado::Hecho) => StatusCode::NO_CONTENT.into_response(),
        Ok(ambulancia::ResultadoBorrado::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(ambulancia::ResultadoBorrado::TieneTraslados { traslados }) => detalle(
            StatusCode::CONFLICT,
            format!(
                "Esta ambulancia tiene {traslados} traslado(s) registrados y su historial respalda \
                 cada uno. Désela de baja: dejará de poder despacharse y seguirá explicando los \
                 que ya hizo."
            ),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_traslados(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<ambulancia::FiltrosTraslado>,
) -> Response {
    match ambulancia::listar_traslados(&e.pool, u.ambito(), &f).await {
        Ok(t) => Json(t).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_traslado(State(e): State<Estado>, Autenticado(u): Autenticado, Path(id): Path<i64>) -> Response {
    match ambulancia::obtener_traslado(&e.pool, u.ambito(), id).await {
        Ok(Some(t)) => Json(t).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Despacha. Cada negativa dice qué hacer: qué traslado la tiene ocupada, en qué
/// estado está, cuándo venció el SOAT. Quien coordina una urgencia no puede
/// resolver nada con un «no se pudo».
async fn amb_despachar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAmbulancias>,
    Json(t): Json<ambulancia::TrasladoNuevo>,
) -> Response {
    use ambulancia::ResultadoDespacho as R;
    let u = autorizado.0;
    match ambulancia::despachar(&e.pool, u.ambito(), u.id, &t, &ambulancia::hoy()).await {
        Ok(R::Hecho(t)) => (StatusCode::CREATED, Json(*t)).into_response(),
        Ok(R::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(R::Ocupada { traslado_id }) => detalle(
            StatusCode::CONFLICT,
            format!("Esa ambulancia está en el traslado #{traslado_id}. Ciérrelo o despache otra."),
        ),
        Ok(R::NoDisponible { estado }) => detalle(
            StatusCode::CONFLICT,
            format!("Esa ambulancia está «{estado}» y no se puede despachar."),
        ),
        Ok(R::SoatVencido { vencio }) => detalle(
            StatusCode::CONFLICT,
            format!(
                "El SOAT de esa ambulancia venció el {vencio}: no puede circular. Despache otra \
                 mientras se renueva."
            ),
        ),
        Ok(R::TipoInvalido) => detalle(
            StatusCode::BAD_REQUEST,
            format!(
                "El traslado es {} — primario desde el sitio del evento, secundario entre \
                 prestadores.",
                ambulancia::TIPOS_TRASLADO.join(" o ")
            ),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn cerrar_traslado(
    e: &Estado,
    u: &Usuario,
    id: i64,
    c: ambulancia::Cierre,
    cancelado: bool,
) -> Response {
    use ambulancia::ResultadoCierre as R;
    match ambulancia::cerrar(&e.pool, u.ambito(), id, &c, cancelado).await {
        Ok(R::Hecho(t)) => Json(*t).into_response(),
        Ok(R::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(R::YaCerrado { estado }) => detalle(
            StatusCode::CONFLICT,
            format!("Ese traslado ya está «{estado}»."),
        ),
        Ok(R::LlegadaAntesDeSalida { salida }) => detalle(
            StatusCode::BAD_REQUEST,
            format!("La llegada no puede ser anterior a la salida, que fue el {salida}."),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn amb_finalizar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAmbulancias>,
    Path(id): Path<i64>,
    Json(c): Json<ambulancia::Cierre>,
) -> Response {
    cerrar_traslado(&e, &autorizado.0, id, c, false).await
}

async fn amb_cancelar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarAmbulancias>,
    Path(id): Path<i64>,
    Json(c): Json<ambulancia::Cierre>,
) -> Response {
    cerrar_traslado(&e, &autorizado.0, id, c, true).await
}

// --- Farmacia ----------------------------------------------------------------
//
// LEER el inventario solo pide sesión: enfermería necesita saber si queda
// adrenalina antes de pedirla, y un 403 ahí no protege nada que el listado de
// medicamentos no diga ya. ESCRIBIR —alta, edición, entrada, dispensación—
// exige `GestionarFarmacia`, que en la firma del manejador se lee de un vistazo.

/// El cuerpo con el que se rechaza una operación de farmacia, en el mismo
/// formato (`detail`) que ya espera la interfaz.
fn detalle(codigo: StatusCode, texto: String) -> Response {
    (codigo, Json(serde_json::json!({ "detail": texto }))).into_response()
}

async fn farm_stats(State(e): State<Estado>, Autenticado(u): Autenticado) -> Response {
    match inventario::stats(&e.pool, u.ambito(), &inventario::hoy()).await {
        Ok(s) => Json(s).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn farm_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<inventario::Filtros>,
) -> Response {
    match inventario::listar(&e.pool, u.ambito(), &f, &inventario::hoy()).await {
        Ok(items) => Json(items).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// La institución del item sale de la del USUARIO, nunca del cuerpo.
///
/// Un superadmin no tiene institución (`institucion_id = NULL`), así que no
/// puede dar de alta inventario: no hay respuesta correcta a «¿de quién es este
/// medicamento?» y no se inventa una. Responde 400 diciéndolo, en vez de crear
/// una fila huérfana que después no ve nadie (directiva 20).
async fn farm_crear(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Json(i): Json<inventario::ItemNuevo>,
) -> Response {
    let u = autorizado.0;
    let Some(institucion_id) = u.institucion_id else {
        return detalle(
            StatusCode::BAD_REQUEST,
            "El inventario pertenece a una institución y esta cuenta no está adscrita a ninguna."
                .into(),
        );
    };
    match inventario::crear(&e.pool, u.ambito(), institucion_id, &i).await {
        Ok(Some(id)) => match inventario::obtener(&e.pool, u.ambito(), id, &inventario::hoy()).await {
            Ok(Some(item)) => (StatusCode::CREATED, Json(item)).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        // El único conflicto posible es el UNIQUE (institución, nombre, lote):
        // dar de alta dos veces el mismo lote partiría el stock en dos filas.
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => detalle(
            StatusCode::CONFLICT,
            "Ya existe un item con ese nombre y ese lote. Registre la remesa como ENTRADA sobre él."
                .into(),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn farm_actualizar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Path(id): Path<i64>,
    Json(cambios): Json<inventario::ItemEdicion>,
) -> Response {
    let u = autorizado.0;
    match inventario::actualizar(&e.pool, u.ambito(), id, &cambios).await {
        Ok(true) => match inventario::obtener(&e.pool, u.ambito(), id, &inventario::hoy()).await {
            Ok(Some(item)) => Json(item).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn farm_eliminar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    match inventario::eliminar(&e.pool, u.ambito(), id).await {
        Ok(inventario::ResultadoBorrado::Hecho) => StatusCode::NO_CONTENT.into_response(),
        Ok(inventario::ResultadoBorrado::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(inventario::ResultadoBorrado::TieneHistorial { movimientos, dispensaciones }) => detalle(
            StatusCode::CONFLICT,
            format!(
                "Este item tiene {movimientos} movimiento(s) y {dispensaciones} dispensación(es) \
                 registradas: borrarlo borraría el rastro de a quién se le entregó. \
                 Para retirarlo del uso, déjelo en cero."
            ),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn farm_entrada(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Path(id): Path<i64>,
    Json(entrada): Json<inventario::Entrada>,
) -> Response {
    let u = autorizado.0;
    match inventario::entrada(&e.pool, u.ambito(), id, u.id, &entrada, &inventario::hoy()).await {
        Ok(inventario::ResultadoEntrada::Hecha(item)) => Json(*item).into_response(),
        Ok(inventario::ResultadoEntrada::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(inventario::ResultadoEntrada::CantidadInvalida) => detalle(
            StatusCode::BAD_REQUEST,
            "La cantidad que entra tiene que ser mayor que cero.".into(),
        ),
        Ok(inventario::ResultadoEntrada::LoteDistinto { actual, quedan }) => detalle(
            StatusCode::CONFLICT,
            format!(
                "Quedan {quedan} unidades del lote {actual}. Un lote nuevo no se suma al anterior: \
                 vencen en fechas distintas y dejarían de poder rastrearse por separado. \
                 Dé de alta el lote nuevo como item."
            ),
        ),
        Ok(inventario::ResultadoEntrada::ProveedorNoValido) => detalle(
            StatusCode::CONFLICT,
            "Ese proveedor no es de esta institución o está inactivo. La entrada no se registró: \
             guardarla sin procedencia habría dejado una remesa que usted cree tener rastreada."
                .into(),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// --- Proveedores -------------------------------------------------------------

async fn prov_listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<proveedor::FiltrosProveedor>,
) -> Response {
    match proveedor::listar(&e.pool, u.ambito(), &f).await {
        Ok(ps) => Json(ps).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn prov_obtener(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Path(id): Path<i64>,
) -> Response {
    match proveedor::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Traduce el resultado del alta o la edición, y devuelve el proveedor ya
/// releído para que la interfaz no tenga que pedirlo otra vez.
async fn respuesta_proveedor(
    e: &Estado,
    ambito: Ambito,
    r: proveedor::ResultadoAlta,
    creado: bool,
) -> Response {
    match r {
        proveedor::ResultadoAlta::Creado(id) => match proveedor::obtener(&e.pool, ambito, id).await {
            Ok(Some(p)) => {
                let codigo = if creado { StatusCode::CREATED } else { StatusCode::OK };
                (codigo, Json(p)).into_response()
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        proveedor::ResultadoAlta::NoExiste => StatusCode::NOT_FOUND.into_response(),
        proveedor::ResultadoAlta::SinRazonSocial => detalle(
            StatusCode::BAD_REQUEST,
            "El proveedor necesita una razón social: sin ella la fila no identifica a nadie.".into(),
        ),
        proveedor::ResultadoAlta::NitRepetido(nit) => detalle(
            StatusCode::CONFLICT,
            format!("Ya hay un proveedor con el NIT {nit} en esta institución."),
        ),
    }
}

async fn prov_crear(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Json(p): Json<proveedor::ProveedorNuevo>,
) -> Response {
    let u = autorizado.0;
    let Some(institucion_id) = u.institucion_id else {
        return detalle(
            StatusCode::BAD_REQUEST,
            "Los proveedores son de una institución y esta cuenta no está adscrita a ninguna."
                .into(),
        );
    };
    match proveedor::crear(&e.pool, u.ambito(), institucion_id, &p).await {
        Ok(r) => respuesta_proveedor(&e, u.ambito(), r, true).await,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn prov_actualizar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Path(id): Path<i64>,
    Json(cambios): Json<proveedor::ProveedorEdicion>,
) -> Response {
    let u = autorizado.0;
    match proveedor::actualizar(&e.pool, u.ambito(), id, &cambios).await {
        Ok(r) => respuesta_proveedor(&e, u.ambito(), r, false).await,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn prov_eliminar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Path(id): Path<i64>,
) -> Response {
    let u = autorizado.0;
    match proveedor::eliminar(&e.pool, u.ambito(), id).await {
        Ok(proveedor::ResultadoBorrado::Hecho) => StatusCode::NO_CONTENT.into_response(),
        Ok(proveedor::ResultadoBorrado::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(proveedor::ResultadoBorrado::TieneEntradas { entradas }) => detalle(
            StatusCode::CONFLICT,
            format!(
                "Este proveedor ya trajo {entradas} remesa(s): borrarlo dejaría esas entradas sin \
                 procedencia. Desactívelo y dejará de aparecer al registrar una entrada."
            ),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct BusquedaFormulas {
    q: Option<String>,
}

async fn farm_formulas_pendientes(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(b): Query<BusquedaFormulas>,
) -> Response {
    match inventario::formulas_pendientes(&e.pool, u.ambito(), b.q.as_deref()).await {
        Ok(fs) => Json(fs).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// La entrega. Cada negativa lleva su motivo real: quien atiende el mostrador
/// tiene que poder decirle al paciente por qué no se le entrega, y «error» no es
/// una respuesta que se pueda dar en una farmacia.
async fn farm_dispensar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarFarmacia>,
    Json(d): Json<inventario::DispensarNueva>,
) -> Response {
    use inventario::ResultadoDispensa as R;
    let u = autorizado.0;
    match inventario::dispensar(&e.pool, u.ambito(), u.id, &d, &inventario::hoy()).await {
        Ok(R::Hecha(disp)) => (StatusCode::CREATED, Json(*disp)).into_response(),
        Ok(R::NoExiste) => StatusCode::NOT_FOUND.into_response(),
        Ok(R::CantidadInvalida) => detalle(
            StatusCode::BAD_REQUEST,
            "La cantidad a dispensar tiene que ser mayor que cero.".into(),
        ),
        Ok(R::YaCompleta) => detalle(
            StatusCode::CONFLICT,
            "Esta fórmula ya se entregó completa.".into(),
        ),
        Ok(R::ExcedePrescrito { pendiente }) => detalle(
            StatusCode::CONFLICT,
            format!("De esta fórmula quedan {pendiente} por entregar; no se puede dispensar más de lo prescrito."),
        ),
        Ok(R::SinExistencias { disponible }) => detalle(
            StatusCode::CONFLICT,
            format!("Solo quedan {disponible} unidades de ese item."),
        ),
        Ok(R::LoteVencido { vencio }) => detalle(
            StatusCode::CONFLICT,
            format!("Ese lote venció el {vencio}. No se dispensa: hay que darlo de baja por vencimiento."),
        ),
        Ok(R::LoteNoCoincide { del_item }) => detalle(
            StatusCode::CONFLICT,
            format!("El lote indicado no es el del item seleccionado, que es el {del_item}."),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct Pagina {
    page: Option<i64>,
}

async fn farm_dispensaciones(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(p): Query<Pagina>,
) -> Response {
    match inventario::dispensaciones(&e.pool, u.ambito(), p.page.unwrap_or(1)).await {
        Ok(ds) => Json(ds).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct FiltroMovimientos {
    item_id: Option<i64>,
    tipo: Option<String>,
    page: Option<i64>,
}

async fn farm_movimientos(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
    Query(f): Query<FiltroMovimientos>,
) -> Response {
    match inventario::movimientos(&e.pool, u.ambito(), f.item_id, f.tipo.as_deref(), f.page.unwrap_or(1)).await {
        Ok(ms) => Json(ms).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
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
        // Igual que el arranque de verdad (`main.rs`). Sin esto la tabla de
        // municipios queda vacía y CUALQUIER código sería inválido: el banco
        // dejaría de parecerse a producción justo en lo que se quiere probar.
        medico_datos::divipola::sembrar(&pool).await.unwrap();
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
            demo: None,
            capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()),
        }
    }

    /// Las capacidades por rol, ejercidas por el CABLEADO y no por el dominio.
    ///
    /// `rol.rs` ya prueba que un laboratorista no puede firmar. Eso no dice nada
    /// sobre si la ruta se lo impide: el fallo que esto cierra era justamente que
    /// `/api/demo` publica credenciales, así que lo que hay que probar es que
    /// NO las publica cuando nadie las pidió — y que el que las pide, las
    /// recibe sin sesión, porque quien va a entrar todavía no tiene ninguna.
    mod demostracion {
        use super::*;

        async fn pedir_demo(e: &Estado) -> (StatusCode, String) {
            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/api/demo")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let estado = r.status();
            let cuerpo = axum::body::to_bytes(r.into_body(), 64 * 1024).await.unwrap();
            (estado, String::from_utf8_lossy(&cuerpo).into_owned())
        }

        /// El defecto, o sea lo que le pasa a la instalación de un cliente:
        /// nadie tiene que acordarse de apagar nada.
        #[tokio::test]
        async fn sin_modo_demo_no_publica_nada() {
            let e = estado_de_prueba(false).await;
            let (estado, cuerpo) = pedir_demo(&e).await;
            assert_eq!(estado, StatusCode::NO_CONTENT);
            assert!(cuerpo.is_empty(), "no debería haber cuerpo: {cuerpo}");
        }

        /// Y con el modo pedido a propósito, se publican SIN sesión: la pantalla
        /// de acceso las necesita antes de que exista ninguna.
        #[tokio::test]
        async fn con_modo_demo_las_publica_sin_sesion() {
            let mut e = estado_de_prueba(false).await;
            e.demo = Some(crate::config::Demo {
                correo: "demo@ips.test".into(),
                clave: "demo-1234".into(),
            });
            let (estado, cuerpo) = pedir_demo(&e).await;
            assert_eq!(estado, StatusCode::OK);
            assert!(cuerpo.contains("demo@ips.test"), "{cuerpo}");
            assert!(cuerpo.contains("demo-1234"), "{cuerpo}");
        }
    }

    /// la regla no estaba cableada en ninguna parte. Aquí se pide con una cookie
    /// de verdad, por la ruta de verdad.
    mod capacidades {
        use super::*;

        /// Una institución con un médico, un laboratorista y un paciente, todos
        /// activos y de la misma institución. La historia 7 existe y no está
        /// firmada, que es el único caso en que borrarla o firmarla es posible.
        async fn con_tres_roles() -> Estado {
            let e = estado_de_prueba(false).await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (5,'Lab','lab@x.co',?,'laboratorista',1,1),
                        (6,'Paciente','pac@x.co',?,'paciente',1,1)",
            )
            .bind(&h)
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();
            // La historia NO lleva `institucion_id`: cuelga del paciente, y de ahí
            // sale el aislamiento (ver 0005_historia.sql).
            sqlx::query(
                "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                       fecha_nacimiento, sexo, institucion_id)
                 VALUES (1,'CC','123','Quien','Sea','1990-01-01','F',1)",
            )
            .execute(&e.pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO historia (id, codigo_historia, paciente_id, medico_id, creado_por,
                                       motivo_consulta, enfermedad_actual,
                                       diagnostico_principal, plan_tratamiento)
                 VALUES (7,'H-7',1,2,2,'dolor','desde ayer','J00','reposo')",
            )
            .execute(&e.pool)
            .await
            .unwrap();
            e
        }

        /// La cookie de un usuario, sin pasar por el login (que aquí no se prueba).
        fn cookie_de(id: i64) -> String {
            format!("medico_sesion={}", sesion::emitir(SECRETO, id, 0, 60))
        }

        async fn pedir(e: &Estado, metodo: &str, uri: &str, cookie: &str) -> StatusCode {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method(metodo)
                        .uri(uri)
                        .header(header::COOKIE, cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
        }

        /// Con cuerpo, para las escrituras.
        async fn pedir_con(e: &Estado, metodo: &str, uri: &str, cookie: &str, json: &str) -> StatusCode {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method(metodo)
                        .uri(uri)
                        .header(header::COOKIE, cookie)
                        .header("content-type", "application/json")
                        .body(Body::from(json.to_owned()))
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
        }

        /// EL PEOR DE TODOS: un laboratorista RECETANDO.
        ///
        /// Firmar y borrar ya estaban cerrados, pero escribir no, y el agujero
        /// ancho era ese: `POST /api/historias/{id}/formulas` solo pedía estar
        /// autenticado. Cualquier usuario de la institución —laboratorista,
        /// trabajo social, terapeuta— podía prescribir medicamentos a un paciente
        /// y crear la nota clínica que los justifica. La firma protegía la AUTORÍA
        /// del documento y dejaba abierto su CONTENIDO.
        #[tokio::test]
        async fn un_laboratorista_no_receta_ni_escribe_en_la_historia() {
            let e = con_tres_roles().await;
            let lab = cookie_de(5);

            assert_eq!(
                pedir_con(&e, "POST", "/api/historias/7/formulas", &lab,
                          r#"{"medicamento":"Morfina","dosis":"10mg","frecuencia":"c/4h","duracion":"3d","cantidad":18}"#).await,
                StatusCode::FORBIDDEN,
                "UN LABORATORISTA RECETÓ MEDICAMENTOS"
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/historias", &lab,
                          r#"{"paciente_id":1,"motivo_consulta":"x","enfermedad_actual":"y","diagnostico_principal":"J00","plan_tratamiento":"z"}"#).await,
                StatusCode::FORBIDDEN,
                "un laboratorista creó una nota clínica"
            );
            assert_eq!(
                pedir_con(&e, "PUT", "/api/historias/7", &lab,
                          r#"{"motivo_consulta":"otro","enfermedad_actual":"y","diagnostico_principal":"J00","plan_tratamiento":"z"}"#).await,
                StatusCode::FORBIDDEN,
                "un laboratorista modificó la nota de un médico"
            );

            // Y no basta el 403: nada tuvo que escribirse. Un rechazo que llega
            // después de la escritura no protege nada.
            let formulas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM formula")
                .fetch_one(&e.pool).await.unwrap();
            assert_eq!(formulas, 0, "se guardó la fórmula pese al 403");
            let historias: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia")
                .fetch_one(&e.pool).await.unwrap();
            assert_eq!(historias, 1, "se creó una historia pese al 403");
            let motivo: String =
                sqlx::query_scalar("SELECT motivo_consulta FROM historia WHERE id = 7")
                    .fetch_one(&e.pool).await.unwrap();
            assert_eq!(motivo, "dolor", "la nota del médico se modificó pese al 403");
        }

        /// Farmacia: el que PRESCRIBE no es el que DISPENSA.
        ///
        /// La separación es la razón de ser del control: si el mismo médico que
        /// firma la fórmula puede además marcarla entregada y bajar el stock, la
        /// dispensación deja de ser una comprobación sobre la prescripción y pasa
        /// a ser un segundo clic del mismo acto. Por eso `Medico` NO tiene
        /// `GestionarFarmacia`, aunque sí tenga casi todo lo demás.
        ///
        /// Y la otra mitad, que es la que hace que esto no sea una regla que
        /// cierra por cerrar: LEER el inventario sí puede cualquiera con sesión.
        #[tokio::test]
        async fn el_medico_que_receta_no_dispensa_pero_si_puede_mirar_el_stock() {
            let e = con_tres_roles().await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (8,'Regente','reg@x.co',?,'regente_farmacia',1,1)",
            )
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();
            let medico = cookie_de(2);
            let regente = cookie_de(8);
            let alta = r#"{"nombre":"Acetaminofén 500mg","lote":"L-1","cantidad_actual":100}"#;

            // El médico: puede mirar, no puede tocar.
            assert_eq!(pedir(&e, "GET", "/api/farmacia/inventario", &medico).await, StatusCode::OK);
            assert_eq!(pedir(&e, "GET", "/api/farmacia/stats", &medico).await, StatusCode::OK);
            assert_eq!(
                pedir_con(&e, "POST", "/api/farmacia/inventario", &medico, alta).await,
                StatusCode::FORBIDDEN,
                "un médico dio de alta inventario"
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/farmacia/dispensar", &medico,
                          r#"{"formula_id":1,"item_inventario_id":1,"cantidad_dispensada":1}"#).await,
                StatusCode::FORBIDDEN,
                "EL MISMO QUE RECETA DISPENSÓ"
            );
            let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_inventario")
                .fetch_one(&e.pool).await.unwrap();
            assert_eq!(items, 0, "se creó el item pese al 403");

            // El regente sí: si esto fallara, la regla estaría cerrando de más y
            // la prueba de arriba pasaría por el motivo equivocado.
            assert_eq!(
                pedir_con(&e, "POST", "/api/farmacia/inventario", &regente, alta).await,
                StatusCode::CREATED
            );
        }

        /// `/api/imagenes/…` sirve dos cosas distintas y NO se pisan.
        ///
        /// `/api/imagenes/{id}` y `/api/imagenes/{id}/archivo` son los ADJUNTOS
        /// de la historia clínica; `/api/imagenes/ordenes/…` y
        /// `/api/imagenes/paciente/…` son la bandeja de imagenología. El
        /// enrutador da prioridad al segmento literal sobre el parámetro, así que
        /// `ordenes` no se interpreta como un id — pero eso es una propiedad del
        /// enrutador, no una decisión de este código, y una versión que la
        /// cambiara rompería la bandeja en silencio.
        ///
        /// Se comprueba pidiendo por las dos formas: la bandeja responde 200 y el
        /// adjunto inexistente 404. Si `ordenes` se estuviera leyendo como id, la
        /// primera daría 404 —o el router habría entrado en pánico al montarse,
        /// que es el otro final posible y que esta prueba también caza—.
        #[tokio::test]
        async fn las_rutas_de_imagenes_no_se_pisan() {
            let e = con_tres_roles().await;
            let medico = cookie_de(2);
            assert_eq!(pedir(&e, "GET", "/api/imagenes/ordenes", &medico).await, StatusCode::OK);
            assert_eq!(
                pedir(&e, "GET", "/api/imagenes/paciente/1/informes", &medico).await,
                StatusCode::OK
            );
            // El adjunto por id sigue existiendo como ruta (404 porque no hay
            // imagen 99, no porque la ruta se haya perdido).
            assert_eq!(
                pedir(&e, "GET", "/api/imagenes/99/archivo", &medico).await,
                StatusCode::NOT_FOUND
            );
        }

        /// `/api/agenda/…` mezcla segmentos literales con `{id}` y NO se pisan.
        ///
        /// `medicos`, `turnos`, `indicadores` y `bloqueos` van al lado de
        /// `/api/agenda/{id}`. Que el literal gane al parámetro es una propiedad
        /// del enrutador, no una decisión de este código: una versión que la
        /// cambiara dejaría «Agenda Médica» buscando un médico con id `medicos`.
        #[tokio::test]
        async fn las_rutas_de_agenda_no_se_pisan() {
            let e = con_tres_roles().await;
            let medico = cookie_de(2);
            assert_eq!(pedir(&e, "GET", "/api/agenda/medicos", &medico).await, StatusCode::OK);
            assert_eq!(
                pedir(&e, "GET", "/api/agenda/turnos?medico_id=2&fecha=2026-08-03", &medico).await,
                StatusCode::OK
            );
            assert_eq!(pedir(&e, "GET", "/api/agenda/indicadores", &medico).await, StatusCode::OK);
            assert_eq!(pedir(&e, "GET", "/api/agenda/bloqueos", &medico).await, StatusCode::OK);
            assert_eq!(pedir(&e, "GET", "/api/agenda", &medico).await, StatusCode::OK);
            // La agenda por id sigue siendo una ruta viva (404 porque no hay
            // agenda 99, no porque se haya perdido).
            assert_eq!(pedir(&e, "GET", "/api/agenda/99", &medico).await, StatusCode::NOT_FOUND);
        }

        /// El control es el cableado: definir la agenda exige `GestionarAgenda`,
        /// y **mirarla no**.
        ///
        /// Las dos mitades importan. Si solo se probara el 403, una regla que
        /// cerrara TODO pasaría igual — y dejaría a la recepcionista sin poder
        /// ver los turnos libres, que es el mismo fallo con el signo cambiado.
        #[tokio::test]
        async fn la_agenda_la_define_quien_debe_y_la_mira_cualquiera() {
            let e = con_tres_roles().await;
            let lab = cookie_de(5);
            let medico = cookie_de(2);
            let nueva = r#"{"medico_id":2,"anio":2026,"mes":8,
                "bloques":[{"dia_semana":0,"hora_inicio":"08:00","hora_fin":"12:00"}]}"#;

            assert_eq!(
                pedir_con(&e, "POST", "/api/agenda", &lab, nueva).await,
                StatusCode::FORBIDDEN,
                "un laboratorista definió la agenda de un médico"
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/agenda", &medico, nueva).await,
                StatusCode::CREATED
            );
            // Y el laboratorista SÍ puede consultarla: agendar una cita es
            // trabajo de mostrador, no una decisión clínica.
            assert_eq!(
                pedir(&e, "GET", "/api/agenda/turnos?medico_id=2&fecha=2026-08-03", &lab).await,
                StatusCode::OK
            );
            assert_eq!(pedir(&e, "GET", "/api/agenda", &lab).await, StatusCode::OK);
        }

        /// Los turnos salen de la agenda recién creada, y la cita los descuenta.
        #[tokio::test]
        async fn los_turnos_reflejan_la_agenda_y_las_citas() {
            let e = con_tres_roles().await;
            let medico = cookie_de(2);
            let nueva = r#"{"medico_id":2,"anio":2026,"mes":8,
                "bloques":[{"dia_semana":0,"hora_inicio":"08:00","hora_fin":"10:00",
                            "duracion_slot_min":30}]}"#;
            assert_eq!(
                pedir_con(&e, "POST", "/api/agenda", &medico, nueva).await,
                StatusCode::CREATED
            );

            let turnos = |cookie: String| {
                let e = e.clone();
                async move {
                    let r = router(e)
                        .oneshot(
                            Request::builder()
                                .method("GET")
                                .uri("/api/agenda/turnos?medico_id=2&fecha=2026-08-03")
                                .header(header::COOKIE, cookie)
                                .body(Body::empty())
                                .unwrap(),
                        )
                        .await
                        .unwrap();
                    let cuerpo = axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap();
                    serde_json::from_slice::<serde_json::Value>(&cuerpo).unwrap()
                }
            };

            let antes = turnos(medico.clone()).await;
            assert_eq!(antes.as_array().unwrap().len(), 4);
            assert!(antes.as_array().unwrap().iter().all(|t| t["disponible"] == true));

            // Una cita de 45 minutos a las 08:30 tapa DOS turnos: el suyo y el de
            // las 09:00, que todavía está ocurriendo.
            let cita = r#"{"paciente_id":1,"fecha":"2026-08-03","hora":"08:30",
                "medico_id":2,"tipo":"CONTROL","duracion_minutos":45}"#;
            assert_eq!(
                pedir_con(&e, "POST", "/api/citas", &medico, cita).await,
                StatusCode::CREATED
            );
            let despues = turnos(medico).await;
            let libres: Vec<&str> = despues
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["disponible"] == true)
                .map(|t| t["hora"].as_str().unwrap())
                .collect();
            assert_eq!(
                libres,
                ["08:00", "09:30"],
                "una cita de 45 minutos dejó libre un turno que ya estaba ocupado"
            );
        }

        /// Laboratorio e imágenes: quién escribe el resultado y quién no.
        ///
        /// Enfermería atiende al paciente y registra en la historia, pero no
        /// valida un hemograma ni firma una radiografía. Y al revés, la prueba
        /// tiene que mostrar que la regla DISCRIMINA: el bacteriólogo entra al
        /// laboratorio y no a imagenología, y la radióloga al revés.
        #[tokio::test]
        async fn el_resultado_lo_escribe_quien_lo_procesa() {
            let e = con_tres_roles().await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (10,'Bacterióloga','bac@x.co',?,'bacteriologo',1,1),
                        (11,'Radióloga','rad@x.co',?,'radiologo',1,1),
                        (12,'Enfermera','enf@x.co',?,'enfermeria',1,1)",
            )
            .bind(&h).bind(&h).bind(&h)
            .execute(&e.pool).await.unwrap();
            sqlx::query(
                "INSERT INTO orden (id, historia_id, tipo, descripcion, justificacion)
                 VALUES (1,7,'Laboratorio','Hemograma','Control'),
                        (2,7,'Imagenología','Rx de tórax','Tos')",
            )
            .execute(&e.pool).await.unwrap();

            let resultado = r#"{"resultados_json":{"hb":{"valor":"13","critico":false}},"valores_criticos":false}"#;
            let informe = r#"{"tecnica":"PA","hallazgos":"Sin consolidaciones","impresion_diagnostica":"Normal"}"#;

            // Enfermería no escribe ninguno de los dos.
            assert_eq!(
                pedir_con(&e, "POST", "/api/laboratorio/ordenes/1/resultado", &cookie_de(12), resultado).await,
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/imagenes/ordenes/2/informe", &cookie_de(12), informe).await,
                StatusCode::FORBIDDEN
            );
            // La bacterióloga, el laboratorio sí y el informe radiológico no.
            assert_eq!(
                pedir_con(&e, "POST", "/api/laboratorio/ordenes/1/resultado", &cookie_de(10), resultado).await,
                StatusCode::CREATED
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/imagenes/ordenes/2/informe", &cookie_de(10), informe).await,
                StatusCode::FORBIDDEN
            );
            // La radióloga, al revés.
            assert_eq!(
                pedir_con(&e, "POST", "/api/imagenes/ordenes/2/informe", &cookie_de(11), informe).await,
                StatusCode::CREATED
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/laboratorio/ordenes/1/resultado/validar", &cookie_de(11), "{}").await,
                StatusCode::FORBIDDEN
            );
            // Y enfermería SÍ puede leer el resultado de su paciente: la regla
            // cierra la escritura, no la consulta.
            assert_eq!(
                pedir(&e, "GET", "/api/laboratorio/ordenes/1/resultado", &cookie_de(12)).await,
                StatusCode::OK
            );

            // Firmado el informe, ni la propia radióloga lo reescribe.
            assert_eq!(
                pedir_con(&e, "POST", "/api/imagenes/ordenes/2/informe/firmar", &cookie_de(11), "{}").await,
                StatusCode::OK
            );
            assert_eq!(
                pedir_con(&e, "PUT", "/api/imagenes/ordenes/2/informe", &cookie_de(11), informe).await,
                StatusCode::CONFLICT
            );
        }

        /// Borrar una IMAGEN clínica es borrar un registro clínico.
        ///
        /// Este camino se había quedado fuera de la primera pasada: un
        /// laboratorista no podía borrar un documento —eso ya exigía `Borrar`—
        /// pero sí la radiografía del paciente. Misma decisión del producto, una
        /// puerta que no la aplicaba.
        #[tokio::test]
        async fn un_laboratorista_no_borra_una_imagen_clinica() {
            let e = con_tres_roles().await;
            assert_eq!(
                pedir(&e, "DELETE", "/api/imagenes/1", &cookie_de(5)).await,
                StatusCode::FORBIDDEN,
                "un laboratorista borró una imagen clínica"
            );
            // Y la médica no se queda fuera de lo suyo: aquí no hay imagen 1, así
            // que lo que importa es que NO sea 403 —el rol la deja pasar y el 404
            // lo pone el recurso—.
            assert_ne!(
                pedir(&e, "DELETE", "/api/imagenes/1", &cookie_de(2)).await,
                StatusCode::FORBIDDEN,
                "se le cerró a la médica el borrado de imágenes"
            );
        }

        /// La otra mitad, sin la cual lo de arriba pasaría con un extractor que
        /// devolviera 403 siempre: la MÉDICA sí receta y sí escribe.
        #[tokio::test]
        async fn la_medica_si_receta_y_si_escribe() {
            let e = con_tres_roles().await;
            let med = cookie_de(2);
            assert_ne!(
                pedir_con(&e, "POST", "/api/historias/7/formulas", &med,
                          r#"{"medicamento":"Acetaminofén","dosis":"500mg","frecuencia":"c/8h","duracion":"3d","cantidad":9}"#).await,
                StatusCode::FORBIDDEN,
                "se le cerró la puerta a la médica: el arreglo sería peor que el fallo"
            );
            assert_ne!(
                pedir_con(&e, "POST", "/api/historias", &med,
                          r#"{"paciente_id":1,"motivo_consulta":"x","enfermedad_actual":"y","diagnostico_principal":"J00","plan_tratamiento":"z"}"#).await,
                StatusCode::FORBIDDEN,
                "la médica no pudo crear una historia"
            );
        }

        /// Escribir PACIENTE es un conjunto más ancho que escribir HISTORIA, y esa
        /// diferencia es el sitio donde una regla mal puesta hace daño en silencio:
        /// enfermería registra pacientes todos los días sin escribir la nota
        /// médica. Se comprueban los dos lados con el mismo caso.
        #[tokio::test]
        async fn enfermeria_registra_pacientes_pero_no_escribe_la_nota() {
            let e = con_tres_roles().await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (8,'Enf','enf@x.co',?,'enfermeria',1,1)",
            )
            .bind(&h).execute(&e.pool).await.unwrap();
            let enf = cookie_de(8);

            assert_ne!(
                pedir_con(&e, "POST", "/api/pacientes", &enf,
                          r#"{"tipo_documento":"CC","numero_documento":"999","nombre1":"Nuevo","apellido1":"Paciente","fecha_nacimiento":"1990-01-01","sexo":"M"}"#).await,
                StatusCode::FORBIDDEN,
                "se le cerró a enfermería el registro de pacientes, que sí es suyo"
            );
            assert_eq!(
                pedir_con(&e, "POST", "/api/historias", &enf,
                          r#"{"paciente_id":1,"motivo_consulta":"x","enfermedad_actual":"y","diagnostico_principal":"J00","plan_tratamiento":"z"}"#).await,
                StatusCode::FORBIDDEN,
                "enfermería escribió una nota médica"
            );
        }

        /// La matriz de los módulos, con la respuesta de Juan del 2026-07-30.
        ///
        /// Cada caso comprueba LOS DOS LADOS: quien no debe entrar recibe 403 y
        /// quien sí debe no lo recibe. Sin la segunda mitad, un extractor que
        /// devolviera 403 siempre pasaría la prueba entera y habríamos cambiado un
        /// agujero por una puerta tapiada — que en una historia clínica es igual
        /// de grave, solo que se descubre antes.
        ///
        /// SUSTITUYE a `los_modulos_sin_rol_declarado_siguen_abiertos_a_proposito`,
        /// que fijaba el hueco mientras la matriz no estuviera decidida. Se puso
        /// roja al cerrar hospitalización, que es exactamente para lo que estaba,
        /// y se borra aquí a conciencia.
        #[tokio::test]
        async fn la_matriz_de_los_modulos_deja_entrar_a_quien_atiende_y_no_a_quien_no() {
            let e = con_tres_roles().await;
            let h = claves::hashear("clave123").unwrap();
            // Un profesional de cada oficio que la matriz nombra.
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (10,'Enf','enf2@x.co',?,'enfermeria',1,1),
                        (11,'Nutri','nut@x.co',?,'nutricionista',1,1),
                        (12,'Fisio','fis@x.co',?,'terapeuta',1,1)",
            )
            .bind(&h).bind(&h).bind(&h).execute(&e.pool).await.unwrap();

            let lab = cookie_de(5);
            let enf = cookie_de(10);
            let nutri = cookie_de(11);
            let fisio = cookie_de(12);

            // URGENCIAS Y HOSPITALIZACIÓN: «el médico, el especialista, los
            // enfermeros, todos los profesionales que interactúan con el
            // paciente». El laboratorista interactúa con la MUESTRA.
            let ingreso = r#"{"paciente_id":1,"cama":"101","motivo":"observación"}"#;
            assert_eq!(
                pedir_con(&e, "POST", "/api/hospitalizacion/ingresar", &lab, ingreso).await,
                StatusCode::FORBIDDEN,
                "un laboratorista dio ingreso de hospitalización"
            );
            assert_ne!(
                pedir_con(&e, "POST", "/api/hospitalizacion/ingresar", &enf, ingreso).await,
                StatusCode::FORBIDDEN,
                "se le cerró a enfermería el ingreso de hospitalización, que sí es suyo"
            );

            // NUTRICIÓN: «el nutriólogo o nutricionista».
            let val = r#"{"paciente_id":1,"peso":70.0,"talla":1.70,"diagnostico":"normal"}"#;
            assert_eq!(
                pedir_con(&e, "POST", "/api/nutricion/valoraciones", &lab, val).await,
                StatusCode::FORBIDDEN,
                "un laboratorista hizo una valoración nutricional"
            );
            assert_ne!(
                pedir_con(&e, "POST", "/api/nutricion/valoraciones", &nutri, val).await,
                StatusCode::FORBIDDEN,
                "se le cerró a la nutricionista el módulo de nutrición — el fallo con el signo cambiado"
            );

            // REHABILITACIÓN: «el fisioterapeuta», que en esta enumeración es
            // `Terapeuta`.
            let plan = r#"{"paciente_id":1,"diagnostico":"lumbalgia","objetivo":"movilidad","sesiones_totales":10}"#;
            assert_eq!(
                pedir_con(&e, "POST", "/api/rehabilitacion/planes", &lab, plan).await,
                StatusCode::FORBIDDEN,
                "un laboratorista abrió un plan de rehabilitación"
            );
            assert_ne!(
                pedir_con(&e, "POST", "/api/rehabilitacion/planes", &fisio, plan).await,
                StatusCode::FORBIDDEN,
                "se le cerró al fisioterapeuta el módulo de rehabilitación"
            );

            // REMISIONES: «enfermeros o personal administrativo».
            let cr = r#"{"remision_id":1,"resumen":"x","recomendaciones":"y"}"#;
            assert_eq!(
                pedir_con(&e, "POST", "/api/contrarreferencia", &lab, cr).await,
                StatusCode::FORBIDDEN,
                "un laboratorista creó una contrarreferencia"
            );
            assert_ne!(
                pedir_con(&e, "POST", "/api/contrarreferencia", &enf, cr).await,
                StatusCode::FORBIDDEN,
                "se le cerró a enfermería la contrarreferencia"
            );
        }

        /// **Borrar una valoración de nutrición pide lo mismo que escribirla.**
        ///
        /// Hallado por `security-review` el 2026-07-31: era el único borrado del
        /// router con `Autenticado` pelado. Un laboratorista recibía 403 al
        /// crear la valoración y **204 al destruirla**, y con ella el plan
        /// (`ON DELETE CASCADE`). La prueba de autorización que había solo
        /// ejercitaba el POST, así que el hueco quedaba fuera de su alcance.
        ///
        /// Discrimina en los dos sentidos: al que no le toca se le cierra, y al
        /// que sí, se le deja. Sin la segunda mitad, un `403` para todo el mundo
        /// pasaría igual.
        #[tokio::test]
        async fn borrar_nutricion_exige_la_misma_capacidad_que_escribirla() {
            let e = con_tres_roles().await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (11,'Nutri','nut3@x.co',?,'nutricionista',1,1)",
            )
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();

            // Una valoración real, creada por quien puede.
            let val = r#"{"paciente_id":1,"peso":70.0,"talla":1.7,"imc":24.2,
                          "diagnostico_nutricional":"eutrófico"}"#;
            let creado =
                pedir_con(&e, "POST", "/api/nutricion/valoraciones", &cookie_de(11), val).await;
            assert!(
                creado.is_success(),
                "el nutricionista debería poder crearla, y devolvió {creado}"
            );
            let id: (i64,) =
                sqlx::query_as("SELECT id FROM valoracion_nutricional ORDER BY id DESC LIMIT 1")
                    .fetch_one(&e.pool)
                    .await
                    .unwrap();
            let uri = format!("/api/nutricion/valoraciones/{}", id.0);

            // Al laboratorista se le cierra…
            assert_eq!(
                pedir_con(&e, "DELETE", &uri, &cookie_de(5), "").await,
                StatusCode::FORBIDDEN,
                "un laboratorista borró una valoración nutricional"
            );
            // …y sigue ahí.
            let quedan: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valoracion_nutricional")
                .fetch_one(&e.pool)
                .await
                .unwrap();
            assert_eq!(quedan.0, 1, "el 403 no impidió el borrado");

            // Y a quien le toca, se le deja.
            assert_ne!(
                pedir_con(&e, "DELETE", &uri, &cookie_de(11), "").await,
                StatusCode::FORBIDDEN,
                "al nutricionista no se le puede cerrar su propio recurso"
            );
        }

        /// SUBIR una imagen sigue abierto, y esta prueba lo fija igual que hacía
        /// la anterior con hospitalización: el radiólogo es quien las sube y no
        /// está en ninguna lista, así que cerrarlo a ojo lo dejaría sin trabajar.
        /// El día que se decida, esto se pondrá rojo y habrá que borrarlo a
        /// conciencia. BORRAR una imagen sí exige `BorrarClinico`.
        #[tokio::test]
        async fn subir_imagen_sigue_sin_exigir_rol_a_proposito() {
            let e = con_tres_roles().await;
            // Contenido ASCII: lo que se comprueba es que el ROL no cierra la
            // puerta, no que la subida termine bien. Cualquier código que no sea
            // 403 —incluido un 400 por imagen inválida— responde a la pregunta.
            let cuerpo = "--x\r\nContent-Disposition: form-data; name=\"archivo\"; filename=\"r.png\"\r\nContent-Type: image/png\r\n\r\nno-es-una-imagen\r\n--x--\r\n";
            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/historias/7/imagenes")
                        .header("content-type", "multipart/form-data; boundary=x")
                        .header(header::COOKIE, cookie_de(5))
                        .body(Body::from(cuerpo))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_ne!(
                r.status(),
                StatusCode::FORBIDDEN,
                "subir imagen ya exige rol: decídase quién sube y BÓRRESE esta prueba a conciencia"
            );
        }

        /// El laboratorista está autenticado y es de la institución: el
        /// aislamiento lo deja pasar. Lo que lo detiene es el ROL, que es lo que
        /// no existía.
        #[tokio::test]
        async fn un_laboratorista_no_firma_ni_borra_una_historia() {
            let e = con_tres_roles().await;
            let lab = cookie_de(5);
            assert_eq!(
                pedir(&e, "POST", "/api/historias/7/firmar", &lab).await,
                StatusCode::FORBIDDEN,
                "un laboratorista firmó una historia clínica como si fuera el médico"
            );
            assert_eq!(
                pedir(&e, "DELETE", "/api/historias/7", &lab).await,
                StatusCode::FORBIDDEN,
                "un laboratorista borró una historia clínica"
            );
            // Y la historia sigue ahí, que es lo que de verdad importa: un 403 que
            // llegara después del borrado no serviría de nada.
            let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia WHERE id = 7")
                .fetch_one(&e.pool)
                .await
                .unwrap();
            assert_eq!(n, 1, "la historia se borró pese al 403");
        }

        /// El reverso, y la mitad que evita el fallo con el signo cambiado: la
        /// médica SÍ puede. Sin esto, un extractor que devolviera 403 siempre
        /// pasaría la prueba de arriba.
        #[tokio::test]
        async fn la_medica_si_firma_su_historia() {
            let e = con_tres_roles().await;
            assert_eq!(
                pedir(&e, "POST", "/api/historias/7/firmar", &cookie_de(2)).await,
                StatusCode::NO_CONTENT,
                "la médica no pudo firmar: el control dejó fuera a quien debe entrar"
            );
        }

        /// Un paciente no ve la institución. Hoy su cuenta no tiene ninguna
        /// pantalla —el portal no existe—, y antes tenía toda la historia clínica
        /// de la clínica entera.
        #[tokio::test]
        async fn un_paciente_no_ve_la_clinica() {
            let e = con_tres_roles().await;
            let pac = cookie_de(6);
            // 404 y no 403: para lo que no le corresponde, «no existe» — es el
            // mismo criterio que ya usa el aislamiento entre instituciones.
            assert_eq!(pedir(&e, "GET", "/api/historias/7", &pac).await, StatusCode::NOT_FOUND);
            assert_eq!(
                pedir(&e, "POST", "/api/historias/7/firmar", &pac).await,
                StatusCode::FORBIDDEN,
                "un paciente firmó una historia clínica"
            );
        }
    }

    /// Un intento de acceso, como lo hace un cliente. `origen` simula el
    /// `X-Real-IP` que escribe el proxy.
    async fn intentar(estado: &Estado, email: &str, clave: &str, origen: Option<&str>) -> StatusCode {
        intentar_con(estado, email, clave, origen, None).await
    }

    /// El mismo intento, pudiendo además falsificar `X-Forwarded-For` como haría
    /// un cliente: nginx la **añade** (`$proxy_add_x_forwarded_for`), así que lo
    /// que el cliente mande queda por delante de su IP real.
    async fn intentar_con(
        estado: &Estado,
        email: &str,
        clave: &str,
        origen: Option<&str>,
        xff: Option<&str>,
    ) -> StatusCode {
        let mut peticion = Request::builder()
            .method("POST")
            .uri("/api/acceso")
            .header("content-type", "application/json")
            // Lo que pone el servidor de verdad con
            // `into_make_service_with_connect_info`. Sin esto no hay origen y el
            // acceso se niega — a propósito.
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:5555".parse::<std::net::SocketAddr>().unwrap(),
            ));
        if let Some(v) = origen {
            peticion = peticion.header("x-real-ip", v);
        }
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

    /// Insistir durante la espera NO la alarga.
    ///
    /// Es la prueba del cambio, y la que distingue los dos comportamientos
    /// posibles. Antes, el intento frenado se anotaba: cada reintento sumaba un
    /// fallo, y como la espera dobla por fallo, tres clics de impaciencia
    /// multiplicaban por ocho lo que había que esperar — y al vencer, otro clic
    /// devolvía al techo. El freno dejaba de limitar el ritmo de ensayo para
    /// convertirse en un bloqueo que la propia víctima se renueva.
    ///
    /// Se compara el `Retry-After` de la primera respuesta frenada con el de
    /// después de insistir. No se exige que sean iguales —el tiempo corre y el
    /// resto puede haber bajado un segundo—, se exige que NO HAYA CRECIDO.
    #[tokio::test]
    async fn insistir_durante_la_espera_no_alarga_la_espera() {
        let e = estado_de_prueba(false).await;
        for _ in 0..=freno::UMBRAL {
            intentar(&e, "ruiz@x.co", "mal", None).await;
        }
        let espera_inicial = segundos_de_espera(&e, "ruiz@x.co").await;
        assert!(espera_inicial >= 1, "no llegó a frenar: {espera_inicial}");

        // Tres golpes más, ya frenado. Con el comportamiento anterior cada uno
        // doblaba la espera.
        for _ in 0..3 {
            assert_eq!(
                intentar(&e, "ruiz@x.co", "mal", None).await,
                StatusCode::TOO_MANY_REQUESTS
            );
        }
        let espera_despues = segundos_de_espera(&e, "ruiz@x.co").await;
        assert!(
            espera_despues <= espera_inicial,
            "insistir alargó la espera de {espera_inicial}s a {espera_despues}s: \
             el intento frenado se está anotando, y eso convierte el freno en un bloqueo"
        );
    }

    /// El `Retry-After` de un intento frenado, en segundos.
    async fn segundos_de_espera(estado: &Estado, email: &str) -> u64 {
        let r = router(estado.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/acceso")
                    .header("content-type", "application/json")
                    .extension(axum::extract::ConnectInfo(
                        "127.0.0.1:5555".parse::<std::net::SocketAddr>().unwrap(),
                    ))
                    .body(Body::from(format!(r#"{{"email":"{email}","clave":"mal"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS, "se esperaba estar frenado");
        r.headers()
            .get(header::RETRY_AFTER)
            .expect("falta Retry-After")
            .to_str()
            .unwrap()
            .parse()
            .expect("Retry-After no es un número de segundos")
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
                    // El zócalo que pone el servidor real: sin él no hay origen
                    // y el acceso se niega, a propósito.
                    .extension(axum::extract::ConnectInfo(
                        "127.0.0.1:5555".parse::<std::net::SocketAddr>().unwrap(),
                    ))
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

    /// **Sin proxy declarado, la cabecera no se cree.** Es la prueba que
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

    /// **La prueba que faltaba, y por la que el freno no frenaba.**
    ///
    /// Con el proxy declarado —la postura de producción— esto leía el primer
    /// salto de `X-Forwarded-For`. Nuestro nginx la escribe con
    /// `$proxy_add_x_forwarded_for`, que AÑADE: lo que manda el cliente queda
    /// delante de su IP real. Así que el atacante escogía su propio cubo en cada
    /// petición y no acumulaba ni un fallo.
    ///
    /// La prueba anterior no lo veía porque simulaba el valor que pone el proxy,
    /// nunca uno falsificado por el cliente: pasaba por la razón equivocada.
    #[tokio::test]
    async fn con_proxy_una_cabecera_falsificada_por_el_cliente_no_cambia_de_cubo() {
        let e = estado_de_prueba(true).await;
        // Lo que llega de verdad: X-Real-IP la pisa nginx con la IP real, y
        // X-Forwarded-For trae delante lo que el cliente quiso poner.
        for i in 0..=freno::UMBRAL {
            intentar_con(
                &e,
                "ruiz@x.co",
                "mal",
                Some("203.0.113.9"),
                Some(&format!("10.0.0.{i}, 203.0.113.9")),
            )
            .await;
        }
        assert_eq!(
            intentar_con(
                &e,
                "ruiz@x.co",
                "mal",
                Some("203.0.113.9"),
                Some("10.0.0.250, 203.0.113.9"),
            )
            .await,
            StatusCode::TOO_MANY_REQUESTS,
            "el cliente eligió su propio cubo cambiando X-Forwarded-For: el freno no frena"
        );
    }

    /// Con el proxy declarado y sin `X-Real-IP`, el origen NO SE SABE — y no se
    /// rellena con un valor común, que pondría a todos en el mismo cubo y
    /// dejaría que un desconocido echase al dueño de la cuenta.
    #[tokio::test]
    async fn con_proxy_y_sin_cabecera_no_se_deja_pasar() {
        let e = estado_de_prueba(true).await;
        assert_eq!(
            intentar(&e, "ruiz@x.co", "clave123", None).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "sin saber de dónde viene, el acceso siguió adelante"
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
                        .extension(axum::extract::ConnectInfo(
                            "127.0.0.1:5555".parse::<std::net::SocketAddr>().unwrap(),
                        ))
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
        c.insert("x-real-ip", "203.0.113.1".parse().unwrap());
        // La que el cliente puede escribir, y que nginx solo AÑADE.
        c.insert("x-forwarded-for", "9.9.9.9, 203.0.113.1".parse().unwrap());

        // Sin proxy manda el zócalo, y ninguna cabecera lo cambia. El puerto no
        // entra: con él, cada conexión sería un cubo nuevo y el freno no llegaría
        // al umbral ni siendo aporreado.
        assert_eq!(origen_de(false, &c, Some(zocalo)).as_deref(), Some("198.51.100.7"));
        // Con proxy manda `X-Real-IP`, NUNCA el primer salto de X-Forwarded-For:
        // ese lo elige el cliente (`9.9.9.9` de arriba).
        assert_eq!(origen_de(true, &c, Some(zocalo)).as_deref(), Some("203.0.113.1"));

        // Con proxy declarado y sin la cabecera, el dato NO ESTÁ. No se vuelve al
        // zócalo —que sería el del proxy, o sea el mismo para todos— ni se
        // inventa un valor: quien llama decide, y decide no dejar pasar.
        let vacia = header::HeaderMap::new();
        assert_eq!(origen_de(true, &vacia, Some(zocalo)), None);
        assert_eq!(origen_de(true, &vacia, None), None);
        // Y la otra mitad, que es la que dejaba pasar el relleno viejo: SIN proxy
        // y sin zócalo tampoco se inventa un `desconocido`. Faltaba este caso, y
        // por eso una mutación que devolvía el relleno seguía verde.
        assert_eq!(origen_de(false, &vacia, None), None);
        assert_eq!(origen_de(false, &c, None), None);
        // Vacía es lo mismo que ausente.
        let mut blanca = header::HeaderMap::new();
        blanca.insert("x-real-ip", "   ".parse().unwrap());
        assert_eq!(origen_de(true, &blanca, Some(zocalo)), None);
    }

    /// **Ningún borrado se autoriza solo con estar autenticado.**
    ///
    /// Esta prueba mira el CÓDIGO FUENTE, no el comportamiento, y es a
    /// propósito: la alternativa —probar ruta por ruta— es lo que ya falló dos
    /// veces. `eliminar_imagen` lleva un comentario diciendo que «escapó a la
    /// primera pasada», y el 2026-07-31 un `security-review` encontró que
    /// `nutri_eliminar` había escapado a la segunda, porque el commit que puso
    /// la capacidad en sus cuatro escrituras hermanas se dejó el borrado.
    ///
    /// Una prueba por caso habría hecho falta escribirla para el caso que nadie
    /// vio. Ésta falla sola con el borrado número once (directiva 23: se
    /// corrige la regla, no el caso).
    ///
    /// Qué exige exactamente: que todo manejador registrado con `delete(...)`
    /// tome un `Autorizado<…>`. No dice CUÁL capacidad —eso es criterio de cada
    /// recurso— sino que haya alguna.
    #[test]
    fn ningun_borrado_se_conforma_con_estar_autenticado() {
        let fuente = include_str!("rutas.rs");

        // Los nombres de manejador que aparecen dentro de un `delete(...)`.
        let mut borradores: Vec<&str> = Vec::new();
        for (i, _) in fuente.match_indices("delete(") {
            let resto = &fuente[i + "delete(".len()..];
            if let Some(fin) = resto.find(')') {
                let nombre = resto[..fin].trim();
                // `delete(...)` con una expresión dentro (rutas de otro módulo)
                // se salta: aquí solo se comprueban los de este archivo.
                if !nombre.is_empty() && nombre.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    borradores.push(nombre);
                }
            }
        }
        assert!(
            borradores.len() >= 10,
            "se esperaban al menos 10 borradores y se encontraron {}: ¿cambió la \
             forma de registrar rutas y esta prueba dejó de mirar nada?",
            borradores.len()
        );

        let mut pelados = Vec::new();
        for nombre in &borradores {
            let firma = format!("async fn {nombre}(");
            let Some(i) = fuente.find(&firma) else {
                panic!("no se encontró el manejador `{nombre}` que registra un delete(...)");
            };
            // El cuerpo empieza en el primer `{` tras la lista de parámetros;
            // basta con mirar hasta ahí.
            let resto = &fuente[i..];
            let fin = resto.find(" {").unwrap_or(resto.len());
            let parametros = &resto[..fin];
            if !parametros.contains("Autorizado<") {
                pelados.push(*nombre);
            }
        }

        assert!(
            pelados.is_empty(),
            "estos borradores no exigen ninguna capacidad, solo estar autenticado: {pelados:?}.\n\
             Un usuario cualquiera de la institución puede destruir el recurso. \
             Ponles el `Autorizado<…>` que corresponda a SU recurso.",
        );
    }

    /// El menú por institución (#65, punto 6), por la ruta de verdad.
    ///
    /// `modulo.rs` prueba el catálogo y `modulos.rs` que no mienta sobre lo que
    /// el binario sirve. Lo que aquí importa es **quién puede cambiar la
    /// licencia**: es la decisión del vendedor, y dejársela al administrador del
    /// cliente sería regalarle el producto entero.
    mod menu_por_institucion {
        use super::*;

        fn cookie_de(id: i64) -> String {
            format!("medico_sesion={}", sesion::emitir(SECRETO, id, 0, 60))
        }

        /// Un admin de la institución 1 y un superadmin sin institución.
        async fn con_admin_y_superadmin() -> Estado {
            let e = estado_de_prueba(false).await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (30,'Admin','adm4@x.co',?,'admin',1,1),
                        (31,'Root','root@x.co',?,'superadmin',NULL,1)",
            )
            .bind(&h)
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();
            e
        }

        async fn enviar(e: &Estado, metodo: &str, uri: &str, quien: i64, cuerpo: &str) -> Response {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method(metodo)
                        .uri(uri)
                        .header(header::COOKIE, cookie_de(quien))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(cuerpo.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        /// Una institución nueva arranca con los tres que todo prestador usa, y
        /// no con todo ni con nada: sin pacientes no hay historia, sin historia
        /// no hay RIPS y sin RIPS no hay pago.
        #[tokio::test]
        async fn una_institucion_nueva_arranca_con_lo_minimo_para_trabajar() {
            let e = con_admin_y_superadmin().await;
            let r = enviar(&e, "GET", "/api/modulos", 30, "").await;
            assert_eq!(r.status(), StatusCode::OK);
            let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(
                v["habilitados"],
                serde_json::json!(["CITAS", "HISTORIA", "PACIENTES"])
            );
        }

        /// **La licencia la cambia el vendedor, no el cliente.**
        #[tokio::test]
        async fn un_admin_de_institucion_no_se_amplia_su_propia_licencia() {
            let e = con_admin_y_superadmin().await;
            let cuerpo = r#"{"modulos":["CITAS","PACIENTES","HISTORIA","FARMACIA"]}"#;
            assert_eq!(
                enviar(&e, "PUT", "/api/instituciones/1/modulos", 30, cuerpo).await.status(),
                StatusCode::FORBIDDEN,
                "un admin se compró un módulo solo"
            );
            // Y el superadmin sí.
            let r = enviar(&e, "PUT", "/api/instituciones/1/modulos", 31, cuerpo).await;
            assert_eq!(r.status(), StatusCode::OK);
        }

        /// El cambio se ve en el menú de la institución en la siguiente petición.
        #[tokio::test]
        async fn lo_que_el_vendedor_licencia_aparece_en_el_menu() {
            let e = con_admin_y_superadmin().await;
            let cuerpo = r#"{"modulos":["CITAS","PACIENTES","HISTORIA","FARMACIA","KARDEX"]}"#;
            assert_eq!(
                enviar(&e, "PUT", "/api/instituciones/1/modulos", 31, cuerpo).await.status(),
                StatusCode::OK
            );

            let r = enviar(&e, "GET", "/api/modulos", 30, "").await;
            let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            let estado = |nombre: &str| -> String {
                v["grupos"]
                    .as_array().unwrap().iter()
                    .flat_map(|g| g["modulos"].as_array().unwrap())
                    .find(|m| m["modulo"] == nombre)
                    .unwrap()["estado"].as_str().unwrap().to_string()
            };
            // Los TRES estados a la vez, que es lo que antes era indistinguible.
            assert_eq!(estado("FARMACIA"), "habilitado", "licenciado y construido");
            assert_eq!(estado("KARDEX"), "pendiente", "licenciado y sin construir");
            assert_eq!(estado("QUIROFANO"), "no_incluido", "no está en su plan");
        }

        /// Un módulo inventado se RECHAZA. Filtrarlo en silencio dejaría la
        /// licencia inerte y nadie sabría por qué ese cliente no ve lo que cree
        /// haber comprado.
        #[tokio::test]
        async fn un_modulo_que_no_existe_se_rechaza_en_vez_de_ignorarse() {
            let e = con_admin_y_superadmin().await;
            let r = enviar(
                &e,
                "PUT",
                "/api/instituciones/1/modulos",
                31,
                r#"{"modulos":["CITAS","TELETRANSPORTE"]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let b = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
            let texto = String::from_utf8(b.to_vec()).unwrap();
            assert!(texto.contains("TELETRANSPORTE"), "debe decir cuál: {texto}");

            // Y no se aplicó NADA: ni siquiera la parte válida.
            let r = enviar(&e, "GET", "/api/modulos", 30, "").await;
            let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(
                v["habilitados"],
                serde_json::json!(["CITAS", "HISTORIA", "PACIENTES"]),
                "un lote rechazado no puede aplicarse a medias"
            );
        }

        #[tokio::test]
        async fn sin_cookie_no_hay_menu() {
            let e = con_admin_y_superadmin().await;
            let r = router(e.clone())
                .oneshot(Request::builder().uri("/api/modulos").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
    }

    /// DIVIPOLA por la ruta de verdad (#16/#77).
    ///
    /// `divipola.rs` ya prueba el catálogo. Lo que aquí se comprueba es que la
    /// API lo EJERZA: hasta ahora `cod_municipio` era TEXT sin validación, así
    /// que un código inventado entraba y el envío del RIPS se rompía después,
    /// lejos de donde se tecleó.
    mod municipios {
        use super::*;

        fn cookie_de(id: i64) -> String {
            format!("medico_sesion={}", sesion::emitir(SECRETO, id, 0, 60))
        }

        async fn crear_con_municipio(e: &Estado, cod: &str) -> Response {
            let municipio = if cod.is_empty() {
                "null".to_string()
            } else {
                format!("\"{cod}\"")
            };
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/pacientes")
                        .header(header::COOKIE, cookie_de(2))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(format!(
                            r#"{{"tipo_documento":"CC","numero_documento":"{}",
                                 "nombre1":"Ana","apellido1":"Ruiz",
                                 "fecha_nacimiento":"1990-01-01","sexo":"F",
                                 "cod_municipio":{municipio}}}"#,
                            // Documento distinto por caso: el número es único.
                            cod.len() * 7 + 1000
                        )))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        #[tokio::test]
        async fn un_municipio_real_entra() {
            let e = estado_de_prueba(false).await;
            assert_eq!(crear_con_municipio(&e, "05001").await.status(), StatusCode::CREATED);
        }

        /// El caso que justifica el cambio: antes esto entraba y salía mal en el
        /// envío, lejos de donde se tecleó.
        #[tokio::test]
        async fn un_municipio_inventado_se_rechaza_aqui_y_no_en_el_envio() {
            let e = estado_de_prueba(false).await;
            let r = crear_con_municipio(&e, "99999").await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let b = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
            let texto = String::from_utf8(b.to_vec()).unwrap();
            assert!(texto.contains("DIVIPOLA"), "el mensaje debe decir qué pasa: {texto}");
        }

        /// **La forma de cuatro dígitos que publica MinSalud no cuela.** Es el
        /// error que más se comete y el que menos se ve: `5001` es Medellín leído
        /// como número, y aceptarlo «siendo amables» haría imposible distinguir
        /// después los que se arreglaron de los que se adivinaron.
        #[tokio::test]
        async fn el_codigo_sin_el_cero_de_la_izquierda_se_rechaza() {
            let e = estado_de_prueba(false).await;
            assert_eq!(
                crear_con_municipio(&e, "5001").await.status(),
                StatusCode::UNPROCESSABLE_ENTITY
            );
        }

        /// Sin municipio SÍ se puede: un paciente puede no tener el dato todavía,
        /// y eso se ve. Lo que no se acepta es uno falso — y no se sustituye por
        /// ninguno (el sistema viejo rellenaba con «05001»).
        #[tokio::test]
        async fn sin_municipio_se_admite_y_no_se_rellena() {
            let e = estado_de_prueba(false).await;
            assert_eq!(crear_con_municipio(&e, "").await.status(), StatusCode::CREATED);
            let cod: (Option<String>,) =
                sqlx::query_as("SELECT cod_municipio FROM paciente ORDER BY id DESC LIMIT 1")
                    .fetch_one(&e.pool).await.unwrap();
            assert_eq!(cod.0, None, "un dato ausente se queda ausente");
        }

        /// La edición cierra por la misma puerta. Cerrar solo el alta deja el
        /// agujero igual de abierto y encima parece cerrado.
        #[tokio::test]
        async fn la_edicion_valida_igual_que_el_alta() {
            let e = estado_de_prueba(false).await;
            assert_eq!(crear_con_municipio(&e, "05001").await.status(), StatusCode::CREATED);
            let id: (i64,) = sqlx::query_as("SELECT id FROM paciente ORDER BY id DESC LIMIT 1")
                .fetch_one(&e.pool).await.unwrap();

            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("PUT")
                        .uri(format!("/api/pacientes/{}", id.0))
                        .header(header::COOKIE, cookie_de(2))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            r#"{"tipo_documento":"CC","numero_documento":"1000",
                                "nombre1":"Ana","apellido1":"Ruiz",
                                "fecha_nacimiento":"1990-01-01","sexo":"F",
                                "cod_municipio":"99999"}"#,
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
        }

        #[tokio::test]
        async fn el_selector_busca_y_respeta_el_limite() {
            let e = estado_de_prueba(false).await;
            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .uri("/api/municipios?q=MEDELL&limite=5")
                        .header(header::COOKIE, cookie_de(2))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::OK);
            let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            let lista = v.as_array().unwrap();
            assert!(!lista.is_empty() && lista.len() <= 5);
            assert!(lista.iter().any(|m| m["codigo"] == "05001"));
            assert_eq!(lista[0]["nombre_departamento"], "ANTIOQUIA");
        }

        /// Un `limite` disparatado no arrastra el país entero por la red.
        #[tokio::test]
        async fn el_limite_se_acota() {
            let e = estado_de_prueba(false).await;
            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .uri("/api/municipios?limite=999999")
                        .header(header::COOKIE, cookie_de(2))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(v.as_array().unwrap().len(), 500);
        }

        #[tokio::test]
        async fn sin_cookie_no_hay_municipios() {
            let e = estado_de_prueba(false).await;
            let r = router(e.clone())
                .oneshot(Request::builder().uri("/api/municipios").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
    }

    /// La matriz de capacidades por institución (#235), por la ruta de verdad.
    ///
    /// `ajustes.rs` ya prueba la regla y `capacidad.rs` la caché. Lo que aquí se
    /// comprueba es lo que ninguno de los dos puede: que el cambio pedido por
    /// HTTP **cambie de verdad lo que la siguiente petición puede hacer**. Ese
    /// es el cableado, y es donde se cae una configuración que se guarda y no se
    /// aplica.
    mod matriz_por_institucion {
        use super::*;

        const ADMIN: i64 = 20;
        const LAB: i64 = 21;

        /// Institución 1 con un admin y un laboratorista; un paciente y una
        /// historia sin firmar, para tener sobre qué ejercer `EscribirClinico`.
        async fn con_admin_y_lab() -> Estado {
            let e = estado_de_prueba(false).await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
                 VALUES (?,'Admin','adm@x.co',?,'admin',1,1),
                        (?,'Lab','lab2@x.co',?,'laboratorista',1,1)",
            )
            .bind(ADMIN)
            .bind(&h)
            .bind(LAB)
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                       fecha_nacimiento, sexo, institucion_id)
                 VALUES (1,'CC','123','Quien','Sea','1990-01-01','F',1)",
            )
            .execute(&e.pool)
            .await
            .unwrap();
            e
        }

        fn cookie_de(id: i64) -> String {
            format!("medico_sesion={}", sesion::emitir(SECRETO, id, 0, 60))
        }

        async fn enviar(e: &Estado, metodo: &str, uri: &str, quien: i64, cuerpo: &str) -> Response {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method(metodo)
                        .uri(uri)
                        .header(header::COOKIE, cookie_de(quien))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(cuerpo.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        async fn cuerpo_de(r: Response) -> String {
            let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
            String::from_utf8(b.to_vec()).unwrap()
        }

        /// Crear una historia: la acción que exige `EscribirClinico`, y que el
        /// laboratorista NO puede por defecto.
        async fn intenta_escribir(e: &Estado, quien: i64) -> StatusCode {
            enviar(
                e,
                "POST",
                "/api/historias",
                quien,
                r#"{"paciente_id":1,"medico_id":2,"motivo_consulta":"m","enfermedad_actual":"e",
                    "diagnostico_principal":"J00","plan_tratamiento":"p"}"#,
            )
            .await
            .status()
        }

        // === LA PRUEBA QUE JUSTIFICA TODO EL CAMBIO =======================

        /// Antes de configurar, el laboratorista no escribe. Después de que el
        /// administrador se lo conceda **por la API**, escribe. Sin esta prueba,
        /// todo lo demás podría estar guardándose sin aplicarse nunca.
        #[tokio::test]
        async fn conceder_por_la_api_cambia_lo_que_la_siguiente_peticion_puede_hacer() {
            let e = con_admin_y_lab().await;
            assert_eq!(
                intenta_escribir(&e, LAB).await,
                StatusCode::FORBIDDEN,
                "de partida el defecto no le deja"
            );

            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"laboratorista","permitido":true}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK, "{}", cuerpo_de(r).await);

            assert_eq!(
                intenta_escribir(&e, LAB).await,
                StatusCode::CREATED,
                "tras concederlo, la MISMA ruta le deja: la caché se invalidó"
            );
        }

        /// Y al revés, que es la mitad que se olvida: quitar una capacidad la
        /// quita de verdad. El médico escribe por defecto; tras recortarlo, no.
        #[tokio::test]
        async fn recortar_por_la_api_tambien_surte_efecto() {
            let e = con_admin_y_lab().await;
            assert_eq!(intenta_escribir(&e, 2).await, StatusCode::CREATED);

            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"medico","permitido":false}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);

            assert_eq!(intenta_escribir(&e, 2).await, StatusCode::FORBIDDEN);
        }

        /// Devolver al defecto es una tercera operación, no «poner el valor del
        /// defecto»: borra la excepción y la casilla vuelve a seguir la regla.
        #[tokio::test]
        async fn devolver_al_defecto_restituye() {
            let e = con_admin_y_lab().await;
            enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"medico","permitido":false}]}"#,
            )
            .await;
            assert_eq!(intenta_escribir(&e, 2).await, StatusCode::FORBIDDEN);

            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"medico","permitido":null}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);
            assert_eq!(intenta_escribir(&e, 2).await, StatusCode::CREATED);
        }

        // === QUIÉN PUEDE TOCARLA ==========================================

        #[tokio::test]
        async fn un_laboratorista_no_ve_ni_toca_la_matriz() {
            let e = con_admin_y_lab().await;
            assert_eq!(
                enviar(&e, "GET", "/api/capacidades", LAB, "").await.status(),
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                enviar(
                    &e,
                    "PUT",
                    "/api/capacidades",
                    LAB,
                    r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"laboratorista","permitido":true}]}"#
                )
                .await
                .status(),
                StatusCode::FORBIDDEN,
                "y menos concedérsela a sí mismo"
            );
        }

        #[tokio::test]
        async fn sin_cookie_no_hay_matriz() {
            let e = con_admin_y_lab().await;
            let r = router(e.clone())
                .oneshot(
                    Request::builder()
                        .uri("/api/capacidades")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }

        // === GUARDARRAÍLES ================================================

        #[tokio::test]
        async fn no_se_puede_tapiar_la_gestion_de_usuarios() {
            let e = con_admin_y_lab().await;
            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"gestionar_usuarios","rol":"admin","permitido":false}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let texto = cuerpo_de(r).await;
            assert!(texto.contains("sin ningún rol"), "mensaje poco útil: {texto}");

            // Y no se aplicó nada: el admin sigue entrando.
            assert_eq!(
                enviar(&e, "GET", "/api/capacidades", ADMIN, "").await.status(),
                StatusCode::OK
            );
        }

        #[tokio::test]
        async fn un_admin_no_se_quita_a_si_mismo_la_administracion() {
            let e = con_admin_y_lab().await;
            // Primero da relevo al laboratorista, así que el guardarraíl 1 no salta.
            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"gestionar_usuarios","rol":"laboratorista","permitido":true}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);

            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"gestionar_usuarios","rol":"admin","permitido":false}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);

            // Pero el relevo SÍ puede hacérselo: el guardarraíl protege del
            // despiste propio, no vuelve al rol intocable.
            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                LAB,
                r#"{"cambios":[{"capacidad":"gestionar_usuarios","rol":"admin","permitido":false}]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK, "{}", cuerpo_de(r).await);
        }

        /// El caso que la ficha original bloqueaba y la directiva 21 rescata:
        /// una IPS sin ambulancias puede dejar esa capacidad sin nadie.
        #[tokio::test]
        async fn una_capacidad_que_la_ips_no_usa_puede_quedarse_sin_nadie() {
            let e = con_admin_y_lab().await;
            let r = enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[
                    {"capacidad":"gestionar_ambulancias","rol":"admin","permitido":false},
                    {"capacidad":"gestionar_ambulancias","rol":"medico","permitido":false},
                    {"capacidad":"gestionar_ambulancias","rol":"enfermeria","permitido":false},
                    {"capacidad":"gestionar_ambulancias","rol":"auxiliar_enfermeria","permitido":false}
                ]}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK, "{}", cuerpo_de(r).await);
        }

        #[tokio::test]
        async fn no_se_concede_la_gestion_de_instituciones_ni_se_toca_al_superadmin() {
            let e = con_admin_y_lab().await;
            for cuerpo in [
                r#"{"cambios":[{"capacidad":"gestionar_instituciones","rol":"admin","permitido":true}]}"#,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"superadmin","permitido":false}]}"#,
            ] {
                let r = enviar(&e, "PUT", "/api/capacidades", ADMIN, cuerpo).await;
                assert_eq!(
                    r.status(),
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "debería rechazarse: {cuerpo}"
                );
            }
        }

        // === LO QUE SE DEVUELVE Y LO QUE SE ANOTA ==========================

        #[tokio::test]
        async fn la_matriz_dice_lo_efectivo_y_lo_que_decia_el_defecto() {
            let e = con_admin_y_lab().await;
            enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"laboratorista","permitido":true}]}"#,
            )
            .await;

            let texto = cuerpo_de(enviar(&e, "GET", "/api/capacidades", ADMIN, "").await).await;
            let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
            let fila = v["capacidades"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["capacidad"] == "escribir_clinico")
                .unwrap();
            let casilla = fila["roles"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["rol"] == "laboratorista")
                .unwrap();
            assert_eq!(casilla["permitido"], true, "lo efectivo");
            assert_eq!(casilla["defecto"], false, "lo que decía el defecto");
            assert_eq!(casilla["apartado"], true, "hay fila guardada");

            // Y el superadmin sale marcado como no configurable, para que la
            // pantalla no ofrezca una casilla que el servidor va a rechazar.
            let sa = fila["roles"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["rol"] == "superadmin")
                .unwrap();
            assert_eq!(sa["configurable"], false);
        }

        /// Punto 7 de la ficha: el cambio queda en la bitácora encadenada, y la
        /// cadena sigue verificando después.
        #[tokio::test]
        async fn el_cambio_queda_en_la_bitacora_y_la_cadena_verifica() {
            let e = con_admin_y_lab().await;
            enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"escribir_clinico","rol":"laboratorista","permitido":true}]}"#,
            )
            .await;

            let filas: Vec<(String,)> = sqlx::query_as(
                "SELECT contenido FROM acceso WHERE contenido LIKE '%capacidad_rol%'",
            )
            .fetch_all(&e.pool)
            .await
            .unwrap();
            assert_eq!(filas.len(), 1, "un eslabón por cambio");
            assert!(
                filas[0].0.contains("escribir_clinico:laboratorista=si"),
                "el eslabón debe decir QUÉ se amplió, no solo que algo cambió: {}",
                filas[0].0
            );

            assert_eq!(
                medico_datos::acceso::verificar(&e.pool, None).await.unwrap(),
                guaca::auditoria::Auditoria::Intacta
            );
        }

        /// Y si el cambio se rechaza, NO se anota: una bitácora que registra lo
        /// que no ocurrió es peor que una que no registra.
        #[tokio::test]
        async fn un_cambio_rechazado_no_deja_rastro() {
            let e = con_admin_y_lab().await;
            enviar(
                &e,
                "PUT",
                "/api/capacidades",
                ADMIN,
                r#"{"cambios":[{"capacidad":"gestionar_usuarios","rol":"admin","permitido":false}]}"#,
            )
            .await;
            let n: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM acceso WHERE contenido LIKE '%capacidad_rol%'",
            )
            .fetch_one(&e.pool)
            .await
            .unwrap();
            assert_eq!(n.0, 0);
        }
    }

    /// El alta de usuario con el documento del profesional (P11/P12).
    ///
    /// Las columnas existen desde `0037` y hasta hoy **ninguna ruta las
    /// escribía**: el RIPS las anotaba como ausentes en toda atención y no
    /// había forma de arreglarlo desde el producto. Es el mismo patrón que
    /// `finalidad` y `causa_externa`, con la diferencia de que allí un defecto
    /// tapaba el hueco y aquí simplemente no había puerta.
    ///
    /// Las tres puertas que se prueban: que el dato ENTRA, que un tipo que la
    /// 948 no admite para quien atiende se rechaza, y que medio documento no
    /// pasa por entero.
    mod documento_del_profesional {
        use super::*;

        const ADMIN: i64 = 30;

        async fn con_admin() -> Estado {
            let e = estado_de_prueba(false).await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
                 VALUES (?,'Admin','adm-doc@x.co',?,'admin',1,1)",
            )
            .bind(ADMIN)
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();
            // Sin licencia vigente la puerta comercial responde 402 y no se
            // llegaría a mirar el documento: se siembra perpetua para que lo
            // que falle sea lo que se está probando y no otra cosa.
            sqlx::query("INSERT INTO licencia (institucion_id, max_usuarios, vence) VALUES (1, 50, NULL)")
                .execute(&e.pool)
                .await
                .unwrap();
            e
        }

        fn cookie_de(id: i64) -> String {
            format!("medico_sesion={}", sesion::emitir(SECRETO, id, 0, 60))
        }

        async fn alta(e: &Estado, cuerpo: &str) -> Response {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/usuarios")
                        .header(header::COOKIE, cookie_de(ADMIN))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(cuerpo.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        #[tokio::test]
        async fn el_documento_entra_y_queda_guardado() {
            let e = con_admin().await;
            let r = alta(
                &e,
                r#"{"nombre":"Dra Ruiz","email":"ruiz-doc@x.co","clave":"clave123456",
                    "rol":"medico","tipo_documento":"CC","numero_documento":"71234567"}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::CREATED);
            let g: (Option<String>, Option<String>) = sqlx::query_as(
                "SELECT tipo_documento, numero_documento FROM usuario WHERE email='ruiz-doc@x.co'",
            )
            .fetch_one(&e.pool)
            .await
            .unwrap();
            assert_eq!(g, (Some("CC".into()), Some("71234567".into())));
        }

        #[tokio::test]
        async fn sin_documento_se_sigue_pudiendo_crear() {
            // La regla es «o los dos o ninguno», no «obligatorio»: los usuarios
            // que ya existen no lo traen y el alta no puede exigir un dato que
            // el alta de ayer no pedía. Sin él, el recolector anota la falta.
            let e = con_admin().await;
            let r = alta(
                &e,
                r#"{"nombre":"Enfermería","email":"enf-doc@x.co","clave":"clave123456","rol":"enfermeria"}"#,
            )
            .await;
            assert_eq!(r.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn el_tipo_que_la_948_no_admite_para_quien_atiende_se_rechaza() {
            // RC (registro civil) y TI (tarjeta de identidad) SÍ están en el
            // maestro —un paciente puede tenerlos— pero un menor de edad no
            // firma un acto médico. Es exactamente el caso con el que el MUV
            // destapó que se estaba mandando el documento del PACIENTE.
            let e = con_admin().await;
            for tipo in ["RC", "TI", "XX"] {
                let r = alta(
                    &e,
                    &format!(
                        r#"{{"nombre":"Quien Sea","email":"{tipo}@x.co","clave":"clave123456",
                            "rol":"medico","tipo_documento":"{tipo}","numero_documento":"1"}}"#
                    ),
                )
                .await;
                assert_eq!(
                    r.status(),
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "«{tipo}» no puede identificar a quien realiza la atención"
                );
            }
            let n: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM usuario WHERE email IN ('RC@x.co','TI@x.co','XX@x.co')",
            )
            .fetch_one(&e.pool)
            .await
            .unwrap();
            assert_eq!(n.0, 0, "y no se creó ninguno a medias");
        }

        #[tokio::test]
        async fn medio_documento_no_pasa() {
            let e = con_admin().await;
            for cuerpo in [
                r#"{"nombre":"A","email":"a-doc@x.co","clave":"clave123456","rol":"medico","tipo_documento":"CC"}"#,
                r#"{"nombre":"B","email":"b-doc@x.co","clave":"clave123456","rol":"medico","numero_documento":"9"}"#,
                r#"{"nombre":"C","email":"c-doc@x.co","clave":"clave123456","rol":"medico","tipo_documento":"CC","numero_documento":"  "}"#,
            ] {
                assert_eq!(
                    alta(&e, cuerpo).await.status(),
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "medio documento no identifica a nadie: {cuerpo}"
                );
            }
        }

        /// La ruta de CORRECCIÓN, que es la que faltaba: todos los
        /// profesionales dados de alta antes de hoy no tienen documento y no
        /// había por dónde ponérselo.
        mod corregir_el_de_uno_que_ya_existe {
            use super::*;

            async fn poner(e: &Estado, id: i64, quien: i64, cuerpo: &str) -> Response {
                router(e.clone())
                    .oneshot(
                        Request::builder()
                            .method("PUT")
                            .uri(format!("/api/usuarios/{id}/documento"))
                            .header(header::COOKIE, cookie_de(quien))
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(cuerpo.to_string()))
                            .unwrap(),
                    )
                    .await
                    .unwrap()
            }

            async fn doc_de(e: &Estado, id: i64) -> (Option<String>, Option<String>) {
                sqlx::query_as("SELECT tipo_documento, numero_documento FROM usuario WHERE id = ?")
                    .bind(id)
                    .fetch_one(&e.pool)
                    .await
                    .unwrap()
            }

            #[tokio::test]
            async fn se_lo_pone_a_uno_que_no_lo_tenia_y_se_lo_puede_quitar() {
                let e = con_admin().await;
                // El propio admin sembrado por `con_admin` no trae documento:
                // es exactamente el caso que esta ruta existe para arreglar.
                assert_eq!(doc_de(&e, ADMIN).await, (None, None));

                let r = poner(&e, ADMIN, ADMIN, r#"{"tipo_documento":"CC","numero_documento":"71234567"}"#).await;
                assert_eq!(r.status(), StatusCode::NO_CONTENT);
                assert_eq!(doc_de(&e, ADMIN).await, (Some("CC".into()), Some("71234567".into())));

                // Y se puede BORRAR: corregir un dedazo dejando la fila sin
                // documento es mejor que dejarla con uno falso.
                let r = poner(&e, ADMIN, ADMIN, r#"{"tipo_documento":null,"numero_documento":null}"#).await;
                assert_eq!(r.status(), StatusCode::NO_CONTENT);
                assert_eq!(doc_de(&e, ADMIN).await, (None, None));
            }

            #[tokio::test]
            async fn un_usuario_de_otra_institucion_da_404_y_no_se_toca() {
                let e = con_admin().await;
                sqlx::query("INSERT OR IGNORE INTO institucion (id,nombre,nit) VALUES (2,'Otra','901')")
                    .execute(&e.pool)
                    .await
                    .unwrap();
                sqlx::query(
                    "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
                     VALUES (77,'Ajeno','ajeno@x.co','h','medico',2,1)",
                )
                .execute(&e.pool)
                .await
                .unwrap();

                let r = poner(&e, 77, ADMIN, r#"{"tipo_documento":"CC","numero_documento":"99"}"#).await;
                assert_eq!(
                    r.status(),
                    StatusCode::NOT_FOUND,
                    "404 y NUNCA 403: un 403 confirma que el id existe, y con eso se enumera \
                     la plantilla del vecino"
                );
                assert_eq!(doc_de(&e, 77).await, (None, None), "y no lo tocó");
            }

            #[tokio::test]
            async fn el_tipo_se_valida_igual_que_en_el_alta() {
                // La regla vive en UNA función compartida; esto comprueba que la
                // ruta nueva la usa y no una copia que se quedará atrás.
                let e = con_admin().await;
                for cuerpo in [
                    r#"{"tipo_documento":"RC","numero_documento":"1"}"#,
                    r#"{"tipo_documento":"CC"}"#,
                ] {
                    assert_eq!(
                        poner(&e, ADMIN, ADMIN, cuerpo).await.status(),
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "{cuerpo}"
                    );
                }
                assert_eq!(doc_de(&e, ADMIN).await, (None, None));
            }

            #[tokio::test]
            async fn no_se_puede_robar_el_documento_de_otro() {
                let e = con_admin().await;
                sqlx::query(
                    "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                     tipo_documento,numero_documento) \
                     VALUES (78,'Dra','dra@x.co','h','medico',1,1,'CC','71234567')",
                )
                .execute(&e.pool)
                .await
                .unwrap();
                let r = poner(&e, ADMIN, ADMIN, r#"{"tipo_documento":"CC","numero_documento":"71234567"}"#).await;
                assert_eq!(r.status(), StatusCode::CONFLICT);
                assert_eq!(doc_de(&e, 78).await, (Some("CC".into()), Some("71234567".into())), "el dueño lo conserva");
            }

            #[tokio::test]
            async fn hace_falta_la_capacidad_de_gestionar_usuarios() {
                let e = con_admin().await;
                sqlx::query(
                    "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
                     VALUES (79,'Dr','dr@x.co','h','medico',1,1)",
                )
                .execute(&e.pool)
                .await
                .unwrap();
                let r = poner(&e, ADMIN, 79, r#"{"tipo_documento":"CC","numero_documento":"5"}"#).await;
                assert_eq!(
                    r.status(),
                    StatusCode::FORBIDDEN,
                    "un médico no edita la ficha de identidad de la planta"
                );
            }
        }

        #[tokio::test]
        async fn dos_profesionales_no_comparten_documento() {
            let e = con_admin().await;
            let cuerpo = |correo: &str| {
                format!(
                    r#"{{"nombre":"Quien","email":"{correo}","clave":"clave123456",
                        "rol":"medico","tipo_documento":"CC","numero_documento":"71234567"}}"#
                )
            };
            assert_eq!(alta(&e, &cuerpo("uno@x.co")).await.status(), StatusCode::CREATED);
            assert_eq!(
                alta(&e, &cuerpo("dos@x.co")).await.status(),
                StatusCode::CONFLICT,
                "lo para `ux_usuario_documento`, no una consulta previa"
            );
        }
    }

    /// `GET /api/rips` por la ruta de verdad (#250).
    ///
    /// Lo que aquí importa no es que el JSON salga —eso lo prueba
    /// `medico_dominio::rips::ac`— sino las tres puertas del endpoint: quién
    /// puede pedirlo, que NO emita un archivo a medias sin que se lo pidan, y
    /// que un dato que falta salga como 422 con la lista y no como un 500.
    mod generacion_del_rips {
        use super::*;
        use chrono::TimeZone;

        /// La hora a la que ocurrió la atención de la prueba, en la zona DEL
        /// PRESTADOR.
        ///
        /// Todo lo demás se deriva de aquí: la base guarda su equivalente en UTC
        /// y el RIPS tiene que devolver otra vez esta misma hora. Escrita así,
        /// la prueba dice lo mismo en Medellín que en el CI, que corre en UTC.
        ///
        /// Usa `zona::zona()` y no `chrono::Local` **a propósito**: `Local` es el
        /// reloj de la máquina, que es justo lo que el producto dejó de mirar el
        /// 2026-08-03. Con `Local` aquí, el arnés y el código medirían husos
        /// distintos y esta prueba volvería a caerse solo en el CI.
        fn atencion_local() -> chrono::DateTime<chrono_tz::Tz> {
            medico_dominio::zona::zona()
                .with_ymd_and_hms(2026, 3, 15, 9, 30, 0)
                .single()
                .expect("9:30 del 15 de marzo existe en cualquier huso sensato")
        }

        fn cookie_de(id: i64) -> String {
            format!("medico_sesion={}", sesion::emitir(SECRETO, id, 0, 60))
        }

        const ADMIN: i64 = 40;
        const MEDICO: i64 = 2;

        async fn con_admin_y_una_consulta() -> Estado {
            let e = estado_de_prueba(false).await;
            let h = claves::hashear("clave123").unwrap();
            sqlx::query(
                "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                 tipo_documento,numero_documento) \
                 VALUES (40,'Admin','adm-rips@x.co',?,'admin',1,1,'CC','71234567')",
            )
            .bind(&h)
            .execute(&e.pool)
            .await
            .unwrap();
            sqlx::query("UPDATE institucion SET codigo_habilitacion='050010123401' WHERE id=1")
                .execute(&e.pool)
                .await
                .unwrap();
            sqlx::query(
                "UPDATE usuario SET tipo_documento='CC', numero_documento='80000000' WHERE id=2",
            )
            .execute(&e.pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
                 fecha_nacimiento,sexo,institucion_id,tipo_usuario,cod_pais,cod_municipio,zona,\
                 cod_pais_origen) \
                 VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F',1,'01','170','05001','01','170')",
            )
            .execute(&e.pool)
            .await
            .unwrap();
            // `finalidad='11'` y `causa_externa='38'` son de la tabla de la
            // Resolución 948. Antes iban «11» y «13»; el 13 era «enfermedad
            // general» en la tabla ANTERIOR y no existe en la nueva, donde ese
            // concepto es el 38 — sembrarlo hacía fallar la prueba con una falta
            // de más, que es exactamente lo que el generador tiene que hacer.
            // LA FECHA SE DERIVA DE LA HORA LOCAL, no se escribe literal.
            //
            // La base guarda UTC y el RIPS reporta la hora a la que ocurrió la
            // atención, o sea la local del prestador. Escribir aquí un
            // `'2026-03-15 14:30:00'` fijo y esperar `09:30` en la salida ataba
            // la prueba a un huso de −05: pasaba en Medellín y fallaba en el CI,
            // que corre en UTC y veía `14:30`. Es la misma forma que ya usa
            // `el_periodo_se_pide_en_local_y_la_base_guarda_utc`.
            let fecha_utc = atencion_local()
                .with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            sqlx::query(
                "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,creado_por,\
                 motivo_consulta,enfermedad_actual,plan_tratamiento,diagnostico_principal,\
                 cod_consulta_cups,modalidad_grupo_servicio,grupo_servicios,cod_servicio,\
                 valor_servicio,concepto_recaudo,finalidad,tipo_diagnostico,causa_externa) \
                 VALUES (1,?,'H1',10,2,2,'dolor','desde ayer','reposo','A000',\
                 '890201','01','01',328,45000,'05','11','01','38')",
            )
            .bind(&fecha_utc)
            .execute(&e.pool)
            .await
            .unwrap();
            e
        }

        /// Una petición con cuerpo, para las rutas que escriben. El módulo de
        /// capacidades tiene una igual, pero es privada de aquel: duplicar seis
        /// líneas cuesta menos que abrir el suyo al resto del archivo.
        async fn escribir(e: &Estado, metodo: &str, uri: &str, quien: i64, json: &str) -> Response {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method(metodo)
                        .uri(uri)
                        .header(header::COOKIE, cookie_de(quien))
                        .header("content-type", "application/json")
                        .body(Body::from(json.to_owned()))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        async fn enviar(e: &Estado, uri: &str, quien: i64) -> Response {
            router(e.clone())
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(uri)
                        .header(header::COOKIE, cookie_de(quien))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        async fn cuerpo(r: Response) -> serde_json::Value {
            let b = axum::body::to_bytes(r.into_body(), 1 << 22).await.unwrap();
            serde_json::from_slice(&b).unwrap()
        }

        #[tokio::test]
        async fn ya_no_hay_que_pedir_nada_para_generarlo() {
            // Hasta el 2026-08-04 esto daba 409: faltaban documentos por portar y
            // el binario se negaba a entregar un archivo que PARECÍA completo.
            // Con los cinco dentro, la puerta se quitó — y esta prueba es la que
            // impide que vuelva por inercia.
            let e = con_admin_y_una_consulta().await;
            let r = enviar(&e, "/api/rips?inicio=2026-03-01&fin=2026-03-31", ADMIN).await;
            assert_eq!(r.status(), StatusCode::OK);
            let v = cuerpo(r).await;
            assert_eq!(v["sin_portar"], serde_json::json!([]), "no falta ninguno");
            for archivo in ["AC", "AP", "AM", "AU", "AH"] {
                assert!(
                    v["archivos"][archivo].is_object(),
                    "falta el {archivo} en la respuesta"
                );
            }
        }

        #[tokio::test]
        async fn la_puerta_que_sigue_en_pie_es_la_de_los_supuestos() {
            // La pareja de la de arriba: quitar el 409 no relajó lo que de verdad
            // separa «este archivo dice lo que hay» de «este archivo rellena lo
            // que no hay». Sin datos, sigue negándose.
            let e = con_admin_y_una_consulta().await;
            sqlx::query("UPDATE historia SET valor_servicio = NULL")
                .execute(&e.pool)
                .await
                .unwrap();
            let r = enviar(&e, "/api/rips?inicio=2026-03-01&fin=2026-03-31", ADMIN).await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&permitir_supuestos=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK, "el camino permisivo lo pide el llamante");
        }

        #[tokio::test]
        async fn el_medico_no_genera_el_rips_por_defecto() {
            // El archivo lleva las atenciones de TODOS los pacientes y de él
            // depende el pago: el defecto son los dos roles que ya exigía el
            // Python. Un prestador de una sola persona se lo concede en su
            // matriz de capacidades.
            let e = con_admin_y_una_consulta().await;
            let r = enviar(&e, "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true", MEDICO)
                .await;
            assert_eq!(r.status(), StatusCode::FORBIDDEN);
        }

        #[tokio::test]
        async fn con_parcial_sale_el_ac_con_la_consulta_dentro() {
            let e = con_admin_y_una_consulta().await;
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);
            let v = cuerpo(r).await;
            assert_eq!(v["archivos"]["AC"]["numDocumentoIdObligado"], "900");
            let u = &v["archivos"]["AC"]["usuarios"][0];
            assert_eq!(u["numDocumentoIdentificacion"], "111");
            let c = &u["servicios"]["consultas"][0];
            assert_eq!(c["codPrestador"], "050010123401");
            assert_eq!(
                c["fechaInicioAtencion"],
                atencion_local().format("%Y-%m-%d %H:%M").to_string(),
                "se reporta la hora LOCAL a la que ocurrió, no la que guarda la base"
            );
            // El documento del MÉDICO, no el del paciente. Es el fallo que el
            // MUV destapó en el Python y la razón de que el SELECT aliase las
            // columnas homónimas.
            assert_eq!(c["numDocumentoIdentificacion"], "80000000");
            assert_eq!(c["codDiagnosticoPrincipalCIE11"], "1A00&XN8P1&XN8P1");
            // Y la respuesta declara SIEMPRE lo que no lleva.
            assert_eq!(v["sin_portar"], serde_json::json!([]));
        }

        /// Una orden y una fórmula colgadas de la MISMA historia de la siembra,
        /// con todo lo que el RIPS pide. Los campos que ninguna pantalla captura
        /// se rellenan aquí a mano —es lo que hará el tarifador— para poder
        /// probar el camino en que el archivo sale completo.
        async fn con_procedimiento_y_medicamento(e: &Estado) {
            sqlx::query(
                "INSERT INTO orden (id,historia_id,tipo,codigo_cups,descripcion,justificacion,\
                 via_ingreso_servicio,grupo_servicios,cod_servicio,valor_servicio,\
                 concepto_recaudo) \
                 VALUES (77,1,'Laboratorio','902210','Hemograma','control','01','02',105,32000,'05')",
            )
            .execute(&e.pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO formula (id,historia_id,medicamento,codigo_cum,dosis,cantidad,\
                 unidad,dias_tratamiento,tipo_medicamento,valor_unitario,valor_servicio,\
                 concepto_recaudo) \
                 VALUES (88,1,'Acetaminofén 500 mg','20048021-41','1 c/8h',30,'tableta',10,\
                 '01',300,9000,'05')",
            )
            .execute(&e.pool)
            .await
            .unwrap();
        }

        #[tokio::test]
        async fn el_ap_y_el_am_salen_con_su_procedimiento_y_su_medicamento_dentro() {
            let e = con_admin_y_una_consulta().await;
            con_procedimiento_y_medicamento(&e).await;
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK, "no debería faltar nada");
            let v = cuerpo(r).await;

            let p = &v["archivos"]["AP"]["usuarios"][0]["servicios"]["procedimientos"][0];
            assert_eq!(p["codProcedimiento"], "902210");
            assert_eq!(p["modalidadGrupoServicioTecSal"], "02", "laboratorio es apoyo dx");
            assert_eq!(
                p["fechaInicioAtencion"],
                atencion_local().format("%Y-%m-%d %H:%M").to_string(),
                "la hora LOCAL de la atención, igual que el AC"
            );
            // El documento del MÉDICO, no el del paciente.
            assert_eq!(p["numDocumentoIdentificacion"], "80000000");
            // El CIE-11 sale del mismo diagnóstico que el CIE-10 de al lado. En
            // el Python iba vacío SIEMPRE, por un `getattr` sobre `Orden`.
            assert_eq!(p["codDiagnosticoPrincipal"], "A000");
            assert_eq!(p["codDiagnosticoPrincipalCIE11"], "1A00&XN8P1&XN8P1");

            let m = &v["archivos"]["AM"]["usuarios"][0]["servicios"]["medicamentos"][0];
            assert_eq!(m["codTecnologiaSalud"], "20048021-41");
            assert_eq!(m["nomTecnologiaSalud"], "Acetaminofén 500 mg");
            assert_eq!(m["cantidadMedicamento"], 30, "entero, no 30.0");
            assert_eq!(m["diasTratamiento"], 10);
            assert_eq!(
                m["fechaDispensAdmon"],
                atencion_local().format("%Y-%m-%d").to_string()
            );
            // El código del catálogo REFs_UPR, no un número inventado.
            assert_eq!(
                m["unidadMinDispensa"],
                medico_dominio::muv::codigo_presentacion(Some("tableta")).unwrap()
            );
            assert_ne!(m["unidadMinDispensa"], 1, "el 1 del catálogo es AMPOLLA");
        }

        #[tokio::test]
        async fn el_au_reporta_una_muerte_como_muerte_y_no_como_un_traslado() {
            // El fallo que este port destapó en el sistema vivo: el mapa escrito
            // a mano del Python emitía «04» —REFERIDO A OTRA INSTITUCION— para un
            // fallecimiento, porque la 948 no trae esa tabla y nadie la había
            // sacado del validador. El «02» es PACIENTE MUERTO.
            let e = con_admin_y_una_consulta().await;
            let ingreso = atencion_local()
                .with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            sqlx::query(
                "INSERT INTO atencion_urgencias (id,paciente_id,triage,motivo_consulta,\
                 enfermera_triage_id,fecha_ingreso,fecha_egreso,causa_motivo,\
                 diagnostico_ingreso,diagnostico_egreso,destino_salida,\
                 diagnostico_causa_muerte) \
                 VALUES (55,10,'TRIAGE_1','paro',2,?,?,'38','A000','A000','FALLECIDO','I219')",
            )
            .bind(&ingreso)
            .bind(&ingreso)
            .execute(&e.pool)
            .await
            .unwrap();

            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK, "no debería faltar nada");
            let v = cuerpo(r).await;
            let u = &v["archivos"]["AU"]["usuarios"][0]["servicios"]["urgencias"][0];
            assert_eq!(u["condicionDestinoUsuarioEgreso"], "02");
            assert_eq!(u["codDiagnosticoCausaMuerte"], "I219");
            // Y el AU no puede llevar los campos del profesional ni los de plata:
            // el MUV rechaza el archivo entero por una propiedad que sobra.
            for prohibido in ["tipoDocumentoIdentificacion", "vrServicio"] {
                assert!(
                    u.get(prohibido).is_none(),
                    "«{prohibido}» no va en el AU: {u}"
                );
            }
            assert_eq!(v["sin_portar"], serde_json::json!([]));
        }

        #[tokio::test]
        async fn lo_que_se_captura_en_urgencias_llega_entero_al_au() {
            // Cruza la frontera del programa de punta a punta: se registra un
            // triage y un egreso **por la API**, y se pide el RIPS. Es la prueba
            // que faltaba, y no es de adorno: las cuatro columnas del AU
            // (`causa_motivo` y los tres diagnósticos) existían en la base y
            // **ninguna ruta las escribía**, así que el archivo las reportaba
            // como ausentes en toda atención y no había forma de arreglarlo
            // desde el producto. Es el mismo patrón que dejó P11/P12 vacío
            // durante meses.
            let e = con_admin_y_una_consulta().await;

            let creado = escribir(
                &e,
                "POST",
                "/api/urgencias/triage",
                ADMIN,
                r#"{"paciente_id":10,"triage":"TRIAGE_1",
                    "motivo_consulta":"dolor torácico","causa_motivo":"38"}"#,
            )
            .await;
            assert_eq!(creado.status(), StatusCode::CREATED);
            let id = cuerpo(creado).await["id"].as_i64().unwrap();

            assert_eq!(
                escribir(
                    &e,
                    "PUT",
                    &format!("/api/urgencias/{id}/egreso"),
                    ADMIN,
                    r#"{"destino":"FALLECIDO","observaciones":"paro irreversible",
                        "diagnostico_ingreso":"A000","diagnostico_egreso":"A000",
                        "diagnostico_causa_muerte":"I219"}"#,
                )
                .await
                .status(),
                StatusCode::NO_CONTENT
            );

            let v = cuerpo(
                enviar(
                    &e,
                    "/api/rips?inicio=2026-01-01&fin=2026-12-31&parcial=true",
                    ADMIN,
                )
                .await,
            )
            .await;
            let u = &v["archivos"]["AU"]["usuarios"][0]["servicios"]["urgencias"][0];
            assert_eq!(u["causaMotivoAtencion"], "38");
            assert_eq!(u["codDiagnosticoPrincipal"], "A000");
            assert_eq!(u["codDiagnosticoPrincipalE"], "A000");
            assert_eq!(u["condicionDestinoUsuarioEgreso"], "02", "paciente muerto");
            assert_eq!(u["codDiagnosticoCausaMuerte"], "I219");
        }

        #[tokio::test]
        async fn un_egreso_sin_diagnosticos_no_borra_los_que_hubiera() {
            // La pareja de la de arriba. `COALESCE` y no asignación directa: si
            // el cuerpo omite un diagnóstico, se queda el que estuviera. Con
            // asignación directa, un egreso corregido a mano habría vaciado en
            // silencio lo que ya se había registrado.
            let e = con_admin_y_una_consulta().await;
            sqlx::query(
                "INSERT INTO atencion_urgencias (id,paciente_id,triage,motivo_consulta,\
                 enfermera_triage_id,fecha_ingreso,diagnostico_ingreso) \
                 VALUES (91,10,'TRIAGE_3','fiebre',2,'2026-03-15T10:00:00Z','J039')",
            )
            .execute(&e.pool)
            .await
            .unwrap();

            assert_eq!(
                escribir(&e, "PUT", "/api/urgencias/91/egreso", ADMIN, r#"{"destino":"DOMICILIO"}"#)
                    .await
                    .status(),
                StatusCode::NO_CONTENT
            );
            let (dx,): (Option<String>,) =
                sqlx::query_as("SELECT diagnostico_ingreso FROM atencion_urgencias WHERE id = 91")
                    .fetch_one(&e.pool)
                    .await
                    .unwrap();
            assert_eq!(dx.as_deref(), Some("J039"), "no se borró al no mandarlo");
        }

        #[tokio::test]
        async fn una_defuncion_hospitalaria_llega_entera_al_ah() {
            // De punta a punta y por la API: ingreso, egreso y RIPS. Cubre de una
            // vez los dos mapas que estaban mal en el sistema vivo —la vía de
            // ingreso INTERCAMBIADA y la defunción reportada como traslado— y los
            // dos campos que ninguna ruta escribía.
            let e = con_admin_y_una_consulta().await;
            sqlx::query(
                "INSERT INTO cama (id,institucion_id,numero,piso,servicio) \
                 VALUES (3,1,'201','2','GENERAL')",
            )
            .execute(&e.pool)
            .await
            .unwrap();

            let creado = escribir(
                &e,
                "POST",
                "/api/hospitalizacion/ingresar",
                ADMIN,
                r#"{"paciente_id":10,"cama_id":3,"medico_id":2,"via_ingreso":"URGENCIAS",
                    "diagnostico_ingreso":"A000","causa_motivo":"38"}"#,
            )
            .await;
            assert_eq!(creado.status(), StatusCode::CREATED);
            let id = cuerpo(creado).await["id"].as_i64().unwrap();

            assert_eq!(
                escribir(
                    &e,
                    "PUT",
                    &format!("/api/hospitalizacion/{id}/egreso"),
                    ADMIN,
                    r#"{"tipo_egreso":"DEFUNCION","diagnostico_egreso":"A000",
                        "diagnostico_causa_muerte":"I219"}"#,
                )
                .await
                .status(),
                StatusCode::NO_CONTENT
            );

            let v = cuerpo(enviar(&e, "/api/rips?inicio=2026-01-01&fin=2026-12-31", ADMIN).await)
                .await;
            let h = &v["archivos"]["AH"]["usuarios"][0]["servicios"]["hospitalizacion"][0];
            assert_eq!(
                h["viaIngresoServicioSalud"], "03",
                "urgencias es DERIVADO DE URGENCIAS, no el 02 de consulta externa"
            );
            assert_eq!(h["causaMotivoAtencion"], "38");
            assert_eq!(h["codDiagnosticoPrincipal"], "A000");
            assert_eq!(
                h["condicionDestinoUsuarioEgreso"], "02",
                "PACIENTE MUERTO, no el 04 de referido a otra institución"
            );
            assert_eq!(h["codDiagnosticoCausaMuerte"], "I219");
            // Y el AH tampoco admite los campos del profesional ni los de plata.
            for prohibido in ["tipoDocumentoIdentificacion", "vrServicio", "diasEstancia"] {
                assert!(h.get(prohibido).is_none(), "«{prohibido}» no va en el AH");
            }
        }

        #[tokio::test]
        async fn el_cum_que_no_esta_en_el_catalogo_del_invima_para_el_archivo() {
            // La pareja de la de arriba: el mismo camino, con el CUM cambiado por
            // uno que tiene la forma correcta y no existe. Sin esta comprobación
            // el archivo sale, el MUV lo acepta o lo rechaza tres semanas después
            // y la factura entera se cae.
            let e = con_admin_y_una_consulta().await;
            con_procedimiento_y_medicamento(&e).await;
            sqlx::query("UPDATE formula SET codigo_cum = '00000000' WHERE id = 88")
                .execute(&e.pool)
                .await
                .unwrap();
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let v = cuerpo(r).await;
            let f = v["faltantes"].as_array().unwrap();
            assert_eq!(f.len(), 1);
            assert_eq!(f[0]["archivo"], "AM");
            assert_eq!(f[0]["registro"], 88, "dice CUÁL fórmula");
            assert!(f[0]["motivo"]
                .as_str()
                .unwrap()
                .contains("CatalogoCUMs"));
        }

        #[tokio::test]
        async fn el_paciente_que_solo_tuvo_una_formula_entra_en_el_rastro() {
            // La bitácora de la Ley 1581 sacaba los pacientes SOLO del AC. Con el
            // AP y el AM dentro, alguien puede aparecer en el archivo por una
            // fórmula o un laboratorio de una atención que no es suya en el AC —
            // y su dato salió igual del sistema.
            let e = con_admin_y_una_consulta().await;
            con_procedimiento_y_medicamento(&e).await;
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);
            let filas: Vec<(String,)> =
                sqlx::query_as("SELECT contenido FROM acceso ORDER BY secuencia ASC")
                    .fetch_all(&e.pool)
                    .await
                    .unwrap();
            let eslabon = filas
                .iter()
                .map(|(c,)| serde_json::from_str::<serde_json::Value>(c).unwrap())
                .find(|v| v["recurso"] == "rips")
                .expect("la exportación deja su eslabón");
            assert_eq!(eslabon["alcance"]["pacientes"], serde_json::json!([10]));
            assert_eq!(
                eslabon["alcance"]["n"], 3,
                "una consulta, un procedimiento y un medicamento"
            );
        }

        #[tokio::test]
        async fn lo_que_falta_sale_como_422_con_la_lista_y_no_como_500() {
            let e = con_admin_y_una_consulta().await;
            sqlx::query("UPDATE historia SET valor_servicio = NULL, concepto_recaudo = NULL")
                .execute(&e.pool)
                .await
                .unwrap();
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let v = cuerpo(r).await;
            let faltantes = v["faltantes"].as_array().unwrap();
            assert_eq!(faltantes.len(), 2);
            let campos: Vec<&str> = faltantes
                .iter()
                .map(|f| f["campo"].as_str().unwrap())
                .collect();
            assert!(campos.contains(&"valor_servicio"), "{campos:?}");
            assert!(faltantes[0]["registro"].as_i64().is_some(), "dice CUÁL historia");
        }

        #[tokio::test]
        async fn con_supuestos_permitidos_sale_y_la_respuesta_los_lleva() {
            // Lo supuesto viaja EN el archivo, no en un log aparte: quien lo
            // firma tiene que verlo antes de firmarlo.
            let e = con_admin_y_una_consulta().await;
            sqlx::query("UPDATE historia SET valor_servicio = NULL")
                .execute(&e.pool)
                .await
                .unwrap();
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true&permitir_supuestos=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);
            let v = cuerpo(r).await;
            assert_eq!(v["supuestos"]["valor del servicio"], 1);
            assert_eq!(
                v["archivos"]["AC"]["usuarios"][0]["servicios"]["consultas"][0]["vrServicio"],
                0
            );
        }

        #[tokio::test]
        async fn un_periodo_al_reves_se_rechaza() {
            let e = con_admin_y_una_consulta().await;
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-03-31&fin=2026-03-01&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn los_catalogos_de_la_948_se_sirven_para_que_la_pantalla_los_ofrezca() {
            // Existen porque hasta el 2026-08-02 la pantalla NO podía ofrecer
            // estos dos campos, así que el sistema los rellenaba solo — y con
            // códigos de la tabla anterior, que el validador rechaza.
            let e = con_admin_y_una_consulta().await;

            let r = enviar(&e, "/api/finalidades", ADMIN).await;
            assert_eq!(r.status(), StatusCode::OK);
            let v = cuerpo(r).await;
            let fs = v.as_array().unwrap();
            assert_eq!(fs.len(), 34);
            assert_eq!(fs[0]["codigo"], "11", "la tabla de la 948 empieza en 11");
            assert!(fs.iter().all(|f| f["codigo"] != "10"), "el «10» ya no está");
            // Con `aplica_ac`, para que la consulta no ofrezca las de solo AP.
            assert!(fs.iter().any(|f| f["aplica_ac"] == false));

            let r = enviar(&e, "/api/causas-motivo", ADMIN).await;
            assert_eq!(r.status(), StatusCode::OK);
            let v = cuerpo(r).await;
            let cs = v.as_array().unwrap();
            assert_eq!(cs.len(), 29);
            assert!(cs.iter().all(|c| c["codigo"] != "13"), "el «13» ya no está");
            let g = cs.iter().find(|c| c["codigo"] == "38").unwrap();
            assert_eq!(g["nombre"], "ENFERMEDAD GENERAL");
        }

        #[tokio::test]
        async fn fuera_del_periodo_no_sale_la_consulta() {
            let e = con_admin_y_una_consulta().await;
            let r = enviar(
                &e,
                "/api/rips?inicio=2026-04-01&fin=2026-04-30&parcial=true",
                ADMIN,
            )
            .await;
            assert_eq!(r.status(), StatusCode::OK);
            let v = cuerpo(r).await;
            assert_eq!(v["archivos"]["AC"]["usuarios"].as_array().unwrap().len(), 0);
        }

        /// #186 — los hooks de **listado** y **exportación** de la bitácora, por la
        /// ruta de verdad. Hasta hoy la bitácora solo anotaba el acceso a UN
        /// recurso: quien listaba el directorio entero de pacientes o se llevaba
        /// el RIPS del mes no dejaba rastro, que es justo el acceso masivo que la
        /// Ley 1581 obliga a poder reconstruir.
        mod bitacora_de_colecciones {
            use super::*;

            /// Los eslabones que hay en la base para un recurso, ya parseados.
            async fn eslabones_de(e: &Estado, recurso: &str) -> Vec<serde_json::Value> {
                let filas: Vec<(String,)> =
                    sqlx::query_as("SELECT contenido FROM acceso ORDER BY secuencia ASC")
                        .fetch_all(&e.pool)
                        .await
                        .unwrap();
                filas
                    .into_iter()
                    .map(|(c,)| serde_json::from_str::<serde_json::Value>(&c).unwrap())
                    .filter(|v| v["recurso"] == recurso)
                    .collect()
            }

            /// Un segundo paciente de la MISMA institución, con su consulta en el
            /// periodo: sin él la exportación cubre a uno solo y no se puede
            /// distinguir «trae los ids» de «trae el id».
            async fn segundo_paciente(e: &Estado) {
                sqlx::query(
                    "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
                     fecha_nacimiento,sexo,institucion_id,tipo_usuario,cod_pais,cod_municipio,zona,\
                     cod_pais_origen) \
                     VALUES (11,'CC','222','Beto','Páez','1985-05-05','M',1,'01','170','05001','01','170')",
                )
                .execute(&e.pool)
                .await
                .unwrap();
                sqlx::query(
                    "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,creado_por,\
                     motivo_consulta,enfermedad_actual,plan_tratamiento,diagnostico_principal,\
                     cod_consulta_cups,modalidad_grupo_servicio,grupo_servicios,cod_servicio,\
                     valor_servicio,concepto_recaudo,finalidad,tipo_diagnostico,causa_externa) \
                     VALUES (2,'2026-03-20 10:00:00','H2',11,2,2,'tos','tres días','reposo','A000',\
                     '890201','01','01',328,45000,'05','11','01','38')",
                )
                .execute(&e.pool)
                .await
                .unwrap();
            }

            /// Una exportación anota QUÉ pacientes salieron —el dato dejó el
            /// sistema— y con qué puertas permisivas se pidió.
            #[tokio::test]
            async fn una_exportacion_deja_su_eslabon_con_los_pacientes() {
                let e = con_admin_y_una_consulta().await;
                segundo_paciente(&e).await;
                let r = enviar(
                    &e,
                    "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                    ADMIN,
                )
                .await;
                assert_eq!(r.status(), StatusCode::OK);

                let evs = eslabones_de(&e, "rips").await;
                assert_eq!(evs.len(), 1, "un eslabón por exportación");
                let v = &evs[0];
                assert_eq!(v["accion"], "exportar");
                assert_eq!(v["usuario_id"], ADMIN);
                // Recurso único y colección no se mezclan (directiva #20).
                assert_eq!(v["paciente_id"], serde_json::Value::Null);
                assert_eq!(v["recurso_id"], serde_json::Value::Null);
                assert_eq!(v["alcance"]["pacientes"], serde_json::json!([10, 11]));
                assert_eq!(v["alcance"]["n"], 2, "dos consultas");
                assert_eq!(v["alcance"]["ambito"], "institucion:1");
                let filtro = v["alcance"]["filtro"].as_str().unwrap();
                assert!(filtro.contains("2026-03-01..2026-03-31"), "{filtro}");
                assert!(filtro.contains("parcial"), "el archivo iba incompleto: {filtro}");

                // Y la cadena sigue verificando con el eslabón nuevo dentro.
                assert_eq!(
                    medico_datos::acceso::verificar(&e.pool, None).await.unwrap(),
                    guaca::auditoria::Auditoria::Intacta
                );
            }

            /// El `permitir_supuestos` queda escrito: dentro de un año, «quién
            /// exportó qué» no se responde sin saber si aquel archivo llevaba
            /// valores que nadie afirmó.
            #[tokio::test]
            async fn el_eslabon_dice_si_el_archivo_iba_con_supuestos() {
                let e = con_admin_y_una_consulta().await;
                sqlx::query("UPDATE historia SET valor_servicio = NULL")
                    .execute(&e.pool)
                    .await
                    .unwrap();
                enviar(
                    &e,
                    "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true&permitir_supuestos=true",
                    ADMIN,
                )
                .await;
                let evs = eslabones_de(&e, "rips").await;
                assert_eq!(evs.len(), 1);
                assert!(
                    evs[0]["alcance"]["filtro"]
                        .as_str()
                        .unwrap()
                        .contains("con supuestos"),
                    "{}",
                    evs[0]["alcance"]["filtro"]
                );
            }

            /// Lo que NO ocurrió no se anota. Prueba que discrimina: si el hook se
            /// moviera al principio del handler, los tres casos dejarían rastro de
            /// una exportación que nunca salió.
            #[tokio::test]
            async fn un_rips_que_no_llego_a_salir_no_deja_rastro() {
                // (a) 400: el periodo va al revés y no se generó nada. (Antes
                // este caso era el 409 de `parcial`, que dejó de existir cuando
                // entró el quinto documento.)
                let e = con_admin_y_una_consulta().await;
                let r = enviar(&e, "/api/rips?inicio=2026-03-31&fin=2026-03-01", ADMIN).await;
                assert_eq!(r.status(), StatusCode::BAD_REQUEST);
                assert!(eslabones_de(&e, "rips").await.is_empty(), "el 400 no exportó nada");

                // (b) 422: faltaban datos obligatorios.
                let e = con_admin_y_una_consulta().await;
                sqlx::query("UPDATE historia SET valor_servicio = NULL, concepto_recaudo = NULL")
                    .execute(&e.pool)
                    .await
                    .unwrap();
                let r = enviar(
                    &e,
                    "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                    ADMIN,
                )
                .await;
                assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
                assert!(eslabones_de(&e, "rips").await.is_empty(), "el 422 no exportó nada");

                // (c) 403: el rol no podía generarlo.
                let e = con_admin_y_una_consulta().await;
                let r = enviar(
                    &e,
                    "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                    MEDICO,
                )
                .await;
                assert_eq!(r.status(), StatusCode::FORBIDDEN);
                assert!(eslabones_de(&e, "rips").await.is_empty(), "el 403 no exportó nada");
            }

            /// Un listado anota ámbito y cuántos, **nunca los ids**: ver un nombre
            /// en una lista no es abrir el expediente, y un eslabón por fila
            /// ahogaría la cadena.
            #[tokio::test]
            async fn el_listado_de_pacientes_deja_su_eslabon_sin_ids() {
                let e = con_admin_y_una_consulta().await;
                segundo_paciente(&e).await;
                let r = enviar(&e, "/api/pacientes", ADMIN).await;
                assert_eq!(r.status(), StatusCode::OK);

                let evs = eslabones_de(&e, "pacientes").await;
                assert_eq!(evs.len(), 1);
                assert_eq!(evs[0]["accion"], "listar");
                assert_eq!(evs[0]["alcance"]["n"], 2);
                assert_eq!(
                    evs[0]["alcance"]["pacientes"],
                    serde_json::json!([]),
                    "un listado no guarda ids"
                );
            }

            /// Y un listado que no devolvió a nadie se anota igual: el barrido de
            /// quien va probando ámbitos es exactamente lo que no puede quedar
            /// fuera del rastro.
            #[tokio::test]
            async fn un_listado_vacio_tambien_se_anota() {
                let e = con_admin_y_una_consulta().await;
                sqlx::query("DELETE FROM historia").execute(&e.pool).await.unwrap();
                sqlx::query("DELETE FROM paciente").execute(&e.pool).await.unwrap();
                enviar(&e, "/api/pacientes", ADMIN).await;
                let evs = eslabones_de(&e, "pacientes").await;
                assert_eq!(evs.len(), 1);
                assert_eq!(evs[0]["alcance"]["n"], 0);
            }

            /// El otro lado del mismo hook: el titular ve en SU rastro que el dato
            /// salió, y no se lleva de paso los ids de los demás que iban en el
            /// mismo archivo (el hueco de lectura que la ficha #186 dejó anotado).
            #[tokio::test]
            async fn el_titular_ve_la_exportacion_sin_los_ids_de_los_demas() {
                let e = con_admin_y_una_consulta().await;
                segundo_paciente(&e).await;
                enviar(
                    &e,
                    "/api/rips?inicio=2026-03-01&fin=2026-03-31&parcial=true",
                    ADMIN,
                )
                .await;

                let r = enviar(&e, "/api/pacientes/10/accesos", ADMIN).await;
                assert_eq!(r.status(), StatusCode::OK);
                let v = cuerpo(r).await;
                let evs = v.as_array().unwrap();
                assert_eq!(evs.len(), 1, "la exportación aparece en el rastro del titular");
                let alcance = &evs[0]["alcance"];
                assert!(
                    alcance.get("pacientes").is_none(),
                    "la lista de exportados no puede salir por aquí: {alcance}"
                );
                assert_eq!(alcance["pacientes_n"], 2, "cuántos, sin decir quiénes");
                // Y por ninguna otra clave del alcance. (Se mira el alcance y no
                // el evento entero porque el `ts` real puede traer un «11» en la
                // hora, y eso haría intermitente una prueba de fuga.)
                let crudo = serde_json::to_string(alcance).unwrap();
                assert!(!crudo.contains("11"), "el id del otro paciente se coló: {crudo}");
            }

            // === LA TERCERA FORMA (#186.b) ============================

            /// Listar el expediente de un paciente entra en SU rastro. Antes no
            /// entraba en el de nadie: como colección no guarda ids, así que
            /// quien abría las historias de alguien por esta vía no dejaba
            /// huella — el agujero que abrió la ficha #186.b.
            #[tokio::test]
            async fn listar_el_expediente_de_un_paciente_entra_en_su_rastro() {
                let e = con_admin_y_una_consulta().await;
                segundo_paciente(&e).await;
                assert_eq!(
                    enviar(&e, "/api/pacientes/10/historias", ADMIN).await.status(),
                    StatusCode::OK
                );

                let v = cuerpo(enviar(&e, "/api/pacientes/10/accesos", ADMIN).await).await;
                let evs = v.as_array().unwrap();
                assert_eq!(evs.len(), 1);
                assert_eq!(evs[0]["accion"], "listar");
                assert_eq!(evs[0]["recurso"], "historias");
                assert_eq!(evs[0]["alcance"]["n"], 1, "una historia suya");
                assert_eq!(evs[0]["recurso_id"], serde_json::Value::Null, "no hay UN id");
                // Y dice que NADA salió del sistema: es una consulta, no una
                // exportación. Esa distinción es el motivo de la tercera forma.
                assert_eq!(evs[0]["alcance"]["pacientes_n"], 0);

                // No ensucia el rastro del otro paciente de la misma institución.
                let v = cuerpo(enviar(&e, "/api/pacientes/11/accesos", ADMIN).await).await;
                assert!(v.as_array().unwrap().is_empty());
            }

            /// **Es la regla la que decide, no el caso** (directiva 23): el mismo
            /// endpoint cae en una forma o en otra según lo que se le pida.
            #[tokio::test]
            async fn el_mismo_endpoint_cae_en_dos_formas_segun_lo_que_se_pida() {
                let e = con_admin_y_una_consulta().await;

                // Con paciente: expediente de un titular → entra en su rastro.
                enviar(&e, "/api/documentos?paciente_id=10&tipo=epicrisis", ADMIN).await;
                let evs = eslabones_de(&e, "documentos").await;
                assert_eq!(evs.len(), 1);
                assert_eq!(evs[0]["paciente_id"], 10);
                assert_eq!(evs[0]["alcance"]["filtro"], "tipo=epicrisis");
                assert_eq!(evs[0]["alcance"]["pacientes"], serde_json::json!([]));

                // Sin paciente: atraviesa titulares → colección, sin titular.
                enviar(&e, "/api/documentos", ADMIN).await;
                let evs = eslabones_de(&e, "documentos").await;
                assert_eq!(evs.len(), 2);
                assert_eq!(evs[1]["paciente_id"], serde_json::Value::Null);
                assert_eq!(evs[1]["alcance"]["filtro"], serde_json::Value::Null);

                // Y ese segundo no aparece en el rastro de nadie.
                let v = cuerpo(enviar(&e, "/api/pacientes/10/accesos", ADMIN).await).await;
                assert_eq!(v.as_array().unwrap().len(), 1, "solo el acotado a él");
            }

            /// El hueco que la tercera forma dejó a la vista: abrir la ficha de un
            /// paciente no dejaba rastro ninguno. Se anotaba su historia y su
            /// documento, pero no él.
            #[tokio::test]
            async fn abrir_la_ficha_de_un_paciente_deja_rastro() {
                let e = con_admin_y_una_consulta().await;
                assert_eq!(
                    enviar(&e, "/api/pacientes/10", ADMIN).await.status(),
                    StatusCode::OK
                );
                let v = cuerpo(enviar(&e, "/api/pacientes/10/accesos", ADMIN).await).await;
                let evs = v.as_array().unwrap();
                assert_eq!(evs.len(), 1);
                assert_eq!(evs[0]["accion"], "ver");
                assert_eq!(evs[0]["recurso"], "paciente");
                assert_eq!(evs[0]["recurso_id"], 10);
                assert!(evs[0].get("alcance").is_none(), "un recurso único no lleva alcance");
            }

            /// Y una ficha de otra institución no deja rastro, porque no hubo
            /// acceso: el 404 es anterior. Discrimina el hook de `obtener_paciente`
            /// contra el error de anotar antes de comprobar el ámbito.
            #[tokio::test]
            async fn una_ficha_ajena_no_deja_rastro() {
                let e = con_admin_y_una_consulta().await;
                sqlx::query("INSERT OR IGNORE INTO institucion (id,nombre,nit) VALUES (2,'Otra','901')")
                    .execute(&e.pool)
                    .await
                    .unwrap();
                sqlx::query(
                    "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
                     fecha_nacimiento,sexo,institucion_id) \
                     VALUES (99,'CC','999','Ajena','Pérez','1990-01-01','F',2)",
                )
                .execute(&e.pool)
                .await
                .unwrap();
                assert_eq!(
                    enviar(&e, "/api/pacientes/99", ADMIN).await.status(),
                    StatusCode::NOT_FOUND
                );
                let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM acceso")
                    .fetch_one(&e.pool)
                    .await
                    .unwrap();
                assert_eq!(n.0, 0, "no hubo acceso que anotar");
            }

            /// Y lo mismo por la vía de los LISTADOS, que es por donde se coló.
            ///
            /// Las cuatro rutas por titular tomaban el `paciente_id` de la
            /// petición y sellaban el eslabón sin mirar el ámbito. No sale ni un
            /// dato —las consultas sí filtran, y devuelven la lista vacía—, pero
            /// el rastro del paciente de la OTRA institución se llenaba de
            /// accesos que nunca ocurrieron, firmados y encadenados, o sea
            /// imposibles de quitar sin dejar la bitácora en `Rota`.
            ///
            /// Discrimina: quitando la comprobación de
            /// `registrar_coleccion_de_un_titular`, este contador da 4.
            #[tokio::test]
            async fn los_listados_de_un_titular_ajeno_no_dejan_rastro() {
                let e = con_admin_y_una_consulta().await;
                sqlx::query("INSERT OR IGNORE INTO institucion (id,nombre,nit) VALUES (2,'Otra','901')")
                    .execute(&e.pool)
                    .await
                    .unwrap();
                sqlx::query(
                    "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
                     fecha_nacimiento,sexo,institucion_id) \
                     VALUES (99,'CC','999','Ajena','Pérez','1990-01-01','F',2)",
                )
                .execute(&e.pool)
                .await
                .unwrap();

                for ruta in [
                    "/api/pacientes/99/historias",
                    "/api/documentos?paciente_id=99",
                    "/api/laboratorio/paciente/99/resultados",
                    "/api/imagenes/paciente/99/informes",
                ] {
                    let r = enviar(&e, ruta, ADMIN).await;
                    assert_eq!(r.status(), StatusCode::OK, "{ruta}: la lista sale vacía, no 403");
                    assert_eq!(
                        cuerpo(r).await.as_array().map(|a| a.len()),
                        Some(0),
                        "{ruta}: y vacía de verdad, que es lo que impide la fuga"
                    );
                }

                let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM acceso")
                    .fetch_one(&e.pool)
                    .await
                    .unwrap();
                assert_eq!(n.0, 0, "cuatro listados de un titular ajeno, cero eslabones");

                // Y con el titular PROPIO sí anota: una salvaguarda que solo
                // cierra no se distingue de una que nunca abre.
                enviar(&e, "/api/pacientes/10/historias", ADMIN).await;
                let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM acceso")
                    .fetch_one(&e.pool)
                    .await
                    .unwrap();
                assert_eq!(n.0, 1, "el listado del paciente propio sí deja rastro");
            }
        }
    }
}
