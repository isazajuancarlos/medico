// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El panel de medico, tejido EN EL SERVIDOR con `maud` (directiva 26).
//!
//! Sustituye —por partes, no de golpe— al SPA de Angular 22 + Material que vive
//! en `frontend/`. El HTML lo compila `rustc` con los tipos del dominio dentro,
//! así que una plantilla rota es un error de compilación y no una pantalla en
//! blanco a las tres semanas.
//!
//! # Los dos frontales CONVIVEN, y esa es la decisión de fondo
//!
//! medico tiene **28 pantallas vivas y ~19 000 líneas de Angular**: migrarlas en
//! una pasada es una rama larga que no se puede probar hasta el final, sobre un
//! producto que ya está en manos de un cliente. Así que el panel nuevo se monta
//! bajo **`/panel`** y Angular sigue sirviendo `/` mientras dure la migración.
//! Cada pantalla que llega aquí se puede abrir, probar y desplegar sola.
//!
//! El cambio de dueño de `/` es el ÚLTIMO paso, y hasta entonces todo esto es
//! reversible borrando una rama. informes lo hizo igual: su «Angular fuera» fue
//! el último commit de la migración, no el primero.
//!
//! # `/api` NO cambia
//!
//! Sigue siendo la superficie de integración, y este panel es un segundo
//! consumidor —server-rendered— de las MISMAS funciones. Lo que no se hace es
//! que el panel llame a `/api` por HTTP contra sí mismo: llama a `medico_datos`
//! directamente, igual que hacen los manejadores JSON.
//!
//! # Nada de este módulo decide sobre seguridad
//!
//! El acceso lo resuelve [`crate::rutas::intentar_acceso`], que es la MISMA
//! función que usa `/api/acceso` —con su freno, su hash de descarte y sus cuatro
//! finales—; aquí solo se le da forma de HTML. El aislamiento entre
//! instituciones lo sigue ejerciendo el `Ambito` de cada consulta, y la licencia
//! por módulo, la capa de [`crate::licencia`]. Copiar cualquiera de esos
//! razonamientos habría creado una segunda copia que se queda atrás en el primer
//! arreglo (directiva 1).

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use maud::{html, Markup, DOCTYPE};

use crate::auth::Estado;

pub mod admin;
pub mod agenda;
pub mod banco_sangre;
pub mod buscador_paciente;
pub mod ambulancias;
pub mod anestesia;
pub mod censo;
pub mod citas;
pub mod consentimiento;
pub mod contrarreferencia;
pub mod defunciones;
pub mod documentos;
pub mod habilitacion;
pub mod historias;
pub mod hospitalizacion;
pub mod enfermeria;
pub mod entrada;
pub mod facturacion;
pub mod farmacia;
pub mod estadisticas;
pub mod imagenes;
pub mod incapacidades;
pub mod laboratorio;
pub mod nacimientos;
pub mod nutricion;
pub mod pacientes;
pub mod piezas;
pub mod segundofactor;
pub mod pyp;
pub mod quirofano;
pub mod rda;
pub mod rehabilitacion;
pub mod remisiones;
pub mod rips;
pub mod urgencias;
pub mod superadmin;
pub mod tablero;
pub mod teleconsultas;
pub mod turnos;
pub mod usuarios;
pub mod panel;

// ── Lo vendorizado ───────────────────────────────────────────────────────────
//
// La MISMA versión de htmx y de `boi` que nexus, chasqui e informes (directiva
// 26), comprobada con `md5sum` y no de memoria:
//
//     htmx.min.js  19a573773be4ca22570ca2f8543120c5
//     boi.js       57ef52d66558e0959482b00e7f5a2360
//     boi.css      0525cc0173d3a098cb4465380aad00cd
//
// (El md5 de boi.js estaba stale —decía b1e6558…, de una versión anterior—; se
// corrigió al vendorizar boi.css el 2026-09-29. La copia real siempre fue la del
// repo boi; era el comentario el que no se había actualizado.)
//
// Van LOCALES, dentro del binario: medico se instala en el portátil de un
// consultorio que trabaja sin internet, y lo que no esté dentro del binario no
// existe. Un `<script src>` a un CDN es justo el vector que este stack cierra.
//
// Y viven en `panel/`, no en `estaticos/`: ese directorio es la salida del build
// de Angular y está en `.gitignore` entero, así que un archivo nuestro ahí no se
// versionaría y desaparecería en el primer clon limpio.
const HTMX_JS: &str = include_str!("../../panel/htmx.min.js");
const BOI_JS: &str = include_str!("../../panel/boi.js");
const PANEL_CSS: &str = include_str!("../../panel/panel.css");
/// La mitad estética de boi (tokens + tema + IBM Plex empotrada), byte a byte la
/// del repo boi (md5 arriba). Va en `panel/` por lo mismo que boi.js.
const BOI_CSS: &str = include_str!("../../panel/boi.css");
/// Un comportamiento de `boi`, **byte a byte el del repositorio compartido**
/// (`9c0bfb297627fab9fe31957a0ddad6a4`).
///
/// Se sirve bajo `/panel/comportamientos/` y no junto a `boi.js` por una razón
/// concreta: el archivo importa `from "../boi.js"`, y esa ruta relativa solo
/// resuelve si vive un nivel por debajo. La alternativa era editarle el import
/// al vendorizarlo —y entonces dejaría de ser el mismo archivo, que es
/// justamente lo que la directiva 26 pide poder comprobar con `md5sum`.
const CONFIRMAR_DOBLE_JS: &str = include_str!("../../panel/confirmar-doble.js");

fn recurso(cuerpo: &'static str, tipo: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, tipo),
            // Se sirven desde el binario y cambian solo cuando cambia el
            // binario, así que la caché larga es correcta y ahorra la petición
            // en cada pantalla. La invalidación va por el nombre del archivo el
            // día que se suba de versión.
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        cuerpo,
    )
        .into_response()
}

pub async fn htmx_js() -> Response {
    recurso(HTMX_JS, "application/javascript; charset=utf-8")
}
pub async fn boi_js() -> Response {
    recurso(BOI_JS, "application/javascript; charset=utf-8")
}
pub async fn panel_css() -> Response {
    recurso(PANEL_CSS, "text/css; charset=utf-8")
}
pub async fn boi_css() -> Response {
    recurso(BOI_CSS, "text/css; charset=utf-8")
}
pub async fn confirmar_doble_js() -> Response {
    recurso(CONFIRMAR_DOBLE_JS, "application/javascript; charset=utf-8")
}

// ── Quién pide, en el panel ──────────────────────────────────────────────────

/// El mismo usuario que exige `Autenticado`, pero **rechazando como se le
/// rechaza a una persona**: con una redirección a la entrada, no con un 401.
///
/// La AUTORIZACIÓN no cambia ni un ápice —por dentro es `auth::usuario_de`, con
/// su relectura del usuario y de `token_version` en cada petición—. Lo único
/// que cambia es la forma del rechazo: quien teclea `/panel/pacientes` sin
/// sesión tiene que ver la pantalla de entrada, no un 401 en crudo. informes
/// estrenó su panel con este mismo defecto y lo cazó su banco de navegador en la
/// primera corrida.
pub struct EnSesion(pub medico_datos::usuario::Usuario);

impl axum::extract::FromRequestParts<Estado> for EnSesion {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        estado: &Estado,
    ) -> Result<Self, Response> {
        match crate::auth::usuario_de(estado, &parts.headers).await {
            // **Gate de #538**: con la contraseña por restablecer, a cambiarla
            // antes de nada. Va en el EXTRACTOR y no en `contexto` porque no toda
            // pantalla de datos construye contexto —el fragmento del buscador de
            // paciente, por ejemplo, no— y ahí se colaba dato de paciente. La
            // pantalla de cambio y salir NO usan este extractor (cargan la sesión
            // a mano), así que no quedan encerrados.
            Some(u) if u.requiere_cambio_password => {
                Err(Redirect::to("/panel/cambiar-clave").into_response())
            }
            Some(u) => Ok(EnSesion(u)),
            // 303 y no 302: tras un POST fallido, un 302 haría que el navegador
            // repitiera el POST contra la entrada. El 303 lo convierte en GET,
            // que es lo que hay que hacer con una pantalla.
            None => Err(Redirect::to("/panel/entrada").into_response()),
        }
    }
}

/// Como [`EnSesion`], pero **exigiendo además una capacidad** — la misma que
/// exige la ruta JSON equivalente.
///
/// # Por qué existe, y es un agujero que casi abro yo
///
/// La primera versión de `/panel/usuarios` pedía solo `EnSesion`, mientras
/// `GET /api/usuarios` exige `Autorizado<GestionarUsuarios>`. O sea que migrar
/// esa pantalla a `maud` habría **quitado** una autorización que el producto ya
/// tenía: cualquier usuario autenticado —un laboratorista, un auxiliar— habría
/// listado el personal de la institución por la ruta nueva mientras la vieja se
/// lo negaba.
///
/// Es la forma exacta del fallo del que viene #534, cometida otra vez en la
/// migración que lo arregló: **una puerta que existe en un frontal y no en el
/// otro**. Por eso la regla de esta migración es que una pantalla del panel
/// exige lo MISMO que su ruta JSON, y cuando no, se escribe por qué.
///
/// Rechaza como se le rechaza a una persona: quien no tiene sesión va a la
/// entrada; quien la tiene pero no la capacidad ve una pantalla que lo explica,
/// no un 403 en crudo. La DECISIÓN es la misma que toma `Autorizado`, porque por
/// dentro es él.
pub struct EnSesionCon<C: crate::auth::Exige>(pub medico_datos::usuario::Usuario, std::marker::PhantomData<C>);

impl<C: crate::auth::Exige> axum::extract::FromRequestParts<Estado> for EnSesionCon<C> {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        estado: &Estado,
    ) -> Result<Self, Response> {
        // Sin sesión, a la entrada — antes de mirar capacidades, porque «no has
        // entrado» y «no te corresponde» se corrigen de forma distinta.
        let Some(u) = crate::auth::usuario_de(estado, &parts.headers).await else {
            return Err(Redirect::to("/panel/entrada").into_response());
        };
        // **Gate de #538 para el PANEL**: con la contraseña por restablecer, se
        // REBOTA a cambiarla —no un 403—. Va aquí y no delegando en `Autorizado`
        // porque ese extractor lo comparte la API, donde un usuario así recibe
        // 403; el panel, en cambio, tiene que MANDARLO a la pantalla de cambio.
        // Se comprueba antes que la capacidad: cambiar la contraseña no es algo
        // que se le pueda negar por permisos.
        if u.requiere_cambio_password {
            return Err(Redirect::to("/panel/cambiar-clave").into_response());
        }
        match crate::auth::Autorizado::<C>::from_request_parts(parts, estado).await {
            Ok(a) => Ok(EnSesionCon(a.0, std::marker::PhantomData)),
            Err(StatusCode::SERVICE_UNAVAILABLE) => Err(averia()),
            Err(_) => Err((
                StatusCode::FORBIDDEN,
                pagina(
                    "Sin permiso",
                    None,
                    html! {
                        section class="tarjeta estrecha" {
                            h1 { "Esta sección no le corresponde" }
                            p { "Su cuenta no tiene el permiso que esta pantalla necesita. \
                                 Pídaselo a un administrador de su institución." }
                            p { a href="/panel" { "Volver al panel" } }
                        }
                    },
                ),
            )
                .into_response()),
        }
    }
}

// ── El armazón ───────────────────────────────────────────────────────────────

/// La página entera: cabecera, hojas, y el cuerpo que le pase la pantalla.
///
/// `menu` es `Option` porque la entrada no lo lleva —quien no ha entrado no
/// tiene menú que ver, y pintarlo delataría qué módulos tiene contratada esta
/// instalación a quien todavía no es nadie—.
pub fn pagina(titulo: &str, menu: Option<Markup>, cuerpo: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="es" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                // El panel no sale a internet y no quiere que lo indexen: es la
                // consola de un prestador, no una página.
                meta name="robots" content="noindex, nofollow";
                title { (titulo) " · medico" }
                link rel="stylesheet" href="/panel/boi.css";
                link rel="stylesheet" href="/panel/panel.css";
                script src="/panel/htmx.min.js" {}
                script type="module" src="/panel/boi.js" {}
                // Los comportamientos de `boi` que este panel usa. Cada uno se
                // importa aparte: el núcleo no trae ninguno, y cargar los que no
                // se usan sería peso muerto en el portátil de un consultorio.
                script type="module" src="/panel/comportamientos/confirmar-doble.js" {}
            }
            body {
                @if let Some(m) = menu {
                    (m)
                }
                main id="principal" { (cuerpo) }
            }
        }
    }
}

/// Un aviso para la persona que está delante, con su tono.
///
/// Los tres tonos existen porque los tres significan cosas distintas y la
/// pantalla tiene que poder decirlas: lo que salió bien, lo que el usuario puede
/// corregir, y lo que no depende de él.
pub fn aviso(tono: Tono, texto: &str) -> Markup {
    html! {
        p class=(tono.clase()) role="status" { (texto) }
    }
}

#[derive(Clone, Copy)]
pub enum Tono {
    /// Lo que salió bien. Lo estrenó el 2026-08-10 el envío al MUV, que es la
    /// primera pantalla de este panel con un desenlace bueno que anunciar (el CUV).
    Ok,
    Error,
    /// No es culpa de quien mira: la base no respondió, el servicio está caído.
    Averia,
}

impl Tono {
    fn clase(self) -> &'static str {
        match self {
            Tono::Ok => "aviso aviso-ok",
            Tono::Error => "aviso aviso-error",
            Tono::Averia => "aviso aviso-averia",
        }
    }
}

/// Una avería de base de datos, con forma de pantalla y no de 500 pelado.
///
/// Va aquí y no en cada manejador para que todas digan lo mismo: quien está
/// delante no tiene nada que corregir, y decirle «error 500» lo manda a probar
/// su contraseña otra vez.
pub fn averia() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        pagina(
            "No disponible",
            None,
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se pudo consultar la información" }
                    (aviso(Tono::Averia, "El servidor no pudo leer la base de datos. \
                        No es un problema de su cuenta ni de lo que acaba de hacer."))
                    p { "Vuelva a intentarlo en un momento. Si sigue igual, avise a soporte." }
                }
            },
        ),
    )
        .into_response()
}

/// Las rutas del panel. Se montan bajo `/panel` mientras Angular tenga `/`.
pub fn rutas() -> axum::Router<Estado> {
    use axum::routing::{get, post};
    axum::Router::new()
        // Lo vendorizado. Sin sesión a propósito: la hoja de estilos y htmx
        // hacen falta para PINTAR la pantalla de entrada.
        .route("/panel/panel.css", get(panel_css))
        .route("/panel/boi.css", get(boi_css))
        .route("/panel/htmx.min.js", get(htmx_js))
        .route("/panel/boi.js", get(boi_js))
        .route("/panel/comportamientos/confirmar-doble.js", get(confirmar_doble_js))
        // La entrada.
        .route("/panel/entrada", get(entrada::formulario).post(entrada::enviar))
        // El segundo paso. Va en el grupo SIN sesión —quien llega aquí todavía
        // no la tiene— y lo que autoriza es el token pendiente, que el POST
        // comprueba.
        .route(
            "/panel/entrada/codigo",
            get(entrada::formulario_codigo).post(entrada::enviar_codigo),
        )
        .route("/panel/salir", post(entrada::salir))
        // El enrolamiento se hace DESDE DENTRO: quien llega aquí ya entró con su
        // contraseña, y cada manejador vuelve a leer la sesión.
        .route("/panel/segundo-factor", get(segundofactor::ver))
        .route("/panel/segundo-factor/empezar", post(segundofactor::empezar))
        .route("/panel/segundo-factor/confirmar", post(segundofactor::confirmar))
        .route("/panel/segundo-factor/retirar", post(segundofactor::retirar))
        .route("/panel/salir-todo", post(entrada::salir_todo))
        .route(
            "/panel/cambiar-clave",
            get(entrada::cambiar_clave).post(entrada::cambiar_clave_enviar),
        )
        // El panel.
        // **LA RAÍZ.** Hasta el 2026-08-08 la servía el `fallback` con el
        // `index.html` de Angular; retirado el SPA, `/` redirige al panel.
        // Basta una regla para los dos casos: quien no tenga sesión rebota de
        // `/panel` a `/panel/entrada`, porque de eso ya se encarga `EnSesion`.
        .route("/", get(|| async { Redirect::to("/panel") }))
        .route("/panel", get(panel::inicio))
        .route(buscador_paciente::RUTA, get(buscador_paciente::fragmento))
        .route("/panel/pacientes", get(pacientes::listar))
        // Ojo al orden con la ruta de `{id}`: `nuevo` es un segmento ESTÁTICO y
        // matchit lo prefiere sobre la captura, así que `/panel/pacientes/nuevo`
        // no cae en `ficha`.
        .route("/panel/pacientes/nuevo", get(pacientes::nuevo).post(pacientes::crear))
        .route("/panel/pacientes/{id}", get(pacientes::ficha))
        .route(
            "/panel/pacientes/{id}/editar",
            get(pacientes::editar).post(pacientes::guardar),
        )
        .route("/panel/citas", get(citas::listar))
        .route("/panel/incapacidades", get(incapacidades::listar))
        .route("/panel/incapacidades/{id}", get(incapacidades::detalle))
        .route("/panel/remisiones", get(remisiones::listar))
        .route("/panel/contrarreferencia", get(contrarreferencia::listar))
        .route("/panel/consentimiento", get(consentimiento::listar))
        .route("/panel/urgencias", get(urgencias::cola))
        .route("/panel/usuarios", get(usuarios::listar))
        .route("/panel/evoluciones", get(documentos::evoluciones))
        .route("/panel/epicrisis", get(documentos::epicrisis))
        .route("/panel/interconsultas", get(documentos::interconsultas))
        .route("/panel/notas-operatorias", get(documentos::notas_operatorias))
        .route("/panel/tablero", get(tablero::ver))
        .route("/panel/hospitalizacion", get(hospitalizacion::listar))
        // Dos verbos en la MISMA ruta: la lista y el registro. La captura no
        // tiene pantalla propia porque no tiene detalle que enseñar — 14 campos
        // y ninguno depende de otro—, y una ruta más sería una fila más en
        // `PANTALLAS` vigilando lo mismo.
        .route(nacimientos::RUTA, get(nacimientos::listar).post(nacimientos::registrar))
        // El turno de enfermería: una pantalla para los cuatro módulos, y cada
        // sección mira su propia licencia con `cx.tiene(…)`.
        .route(enfermeria::RUTA, get(enfermeria::ver).post(enfermeria::anotar))
        .route("/panel/enfermeria/{id}/anular", post(enfermeria::anular))
        .route(defunciones::RUTA, get(defunciones::listar).post(defunciones::certificar))
        .route("/panel/defunciones/{id}/anular", post(defunciones::anular))
        .route(censo::RUTA, get(censo::ver))
        .route(anestesia::RUTA, get(anestesia::ver))
        .route("/panel/anestesia/valorar", post(anestesia::valorar))
        .route("/panel/anestesia/recibir", post(anestesia::recibir))
        .route("/panel/anestesia/{id}/salida", post(anestesia::salida))
        .route(pyp::RUTA, get(pyp::ver))
        .route("/panel/pyp/inscribir", post(pyp::inscribir))
        .route("/panel/pyp/controlar", post(pyp::controlar))
        .route("/panel/pyp/aplicar", post(pyp::aplicar))
        .route("/panel/pyp/{id}/egresar", post(pyp::egresar))
        .route(habilitacion::RUTA, get(habilitacion::ver))
        .route("/panel/habilitacion/equipos", post(habilitacion::crear_equipo))
        .route("/panel/habilitacion/mantenimientos", post(habilitacion::crear_mantenimiento))
        .route("/panel/habilitacion/comites", post(habilitacion::crear_comite))
        .route("/panel/habilitacion/sesiones", post(habilitacion::crear_sesion))
        .route("/panel/habilitacion/infecciones", post(habilitacion::reportar_iaas))
        .route("/panel/habilitacion/{id}/notificar", post(habilitacion::notificar))
        .route(banco_sangre::RUTA, get(banco_sangre::ver))
        .route("/panel/banco-sangre/unidades", post(banco_sangre::crear_unidad))
        .route("/panel/banco-sangre/tamizaje", post(banco_sangre::tamizar))
        .route("/panel/banco-sangre/transfusiones", post(banco_sangre::transfundir))
        .route("/panel/banco-sangre/{id}/reaccion", post(banco_sangre::reaccion))
        .route(turnos::RUTA, get(turnos::ver).post(turnos::programar))
        .route("/panel/turnos/{id}/quitar", post(turnos::quitar))
        .route(teleconsultas::RUTA, get(teleconsultas::ver).post(teleconsultas::registrar))
        .route("/panel/rehabilitacion", get(rehabilitacion::listar))
        // El RIPS son TRES rutas y no una: elegir el periodo, revisarlo y
        // descargarlo. Las tres caen bajo `/panel/rips`, así que la capa de
        // licencia las cubre por el prefijo con una sola fila en `PANTALLAS`.
        .route("/panel/rips", get(rips::formulario))
        .route("/panel/rips/revisar", get(rips::revisar))
        .route("/panel/rips/archivo", post(rips::archivo))
        .route("/panel/rips/fev", post(rips::fev))
        .route("/panel/rips/muv", post(rips::enviar_al_muv))
        // Imágenes diagnósticas: la primera pantalla del panel que ESCRIBE.
        // Leer solo pide sesión —igual que `/api/imagenes/ordenes`—; las cuatro
        // escrituras exigen `EscribirImagenologia`, la misma capacidad que sus
        // rutas JSON. Todas cuelgan de `/panel/imagenes`, así que la capa de
        // licencia las cubre por el prefijo con una sola fila en `PANTALLAS`.
        .route("/panel/imagenes", get(imagenes::listar))
        .route("/panel/imagenes/{id}", get(imagenes::ver))
        .route("/panel/imagenes/{id}/informe", post(imagenes::guardar))
        .route("/panel/imagenes/{id}/firmar", post(imagenes::firmar))
        .route("/panel/imagenes/{id}/iniciar", post(imagenes::iniciar))
        .route("/panel/rda/{tipo}/{id}", get(rda::preparar))
        .route("/panel/rda/{tipo}/{id}/avalar", post(rda::avalar))
        .route("/panel/rda/{tipo}/{id}/emitir", post(rda::emitir))
        // **El tope alto vale SOLO para las rutas que reciben un archivo**, y
        // faltaba: sin él rige el defecto de axum —2 MB— mientras
        // `POST /api/imagenes/ordenes/{id}/adjuntar` admite 25. O sea que la
        // misma radiografía entraba por la API y el panel la rechazaba con un
        // 413 que no dice nada. Se vio al montar el adjunto de la historia.
        .route(
            "/panel/imagenes/{id}/adjuntar",
            post(imagenes::adjuntar).layer(DefaultBodyLimit::max(crate::rutas::MAX_SUBIDA)),
        )
        // ── Historia clínica ────────────────────────────────────────────────
        //
        // Una URL por sección, que en Angular era un `activeStep()` de 1 a 7 que
        // no salía del navegador. Los tramos estáticos (`/paciente/…`) conviven
        // con `{id}` sin ambigüedad: axum resuelve el estático con prioridad.
        .route("/panel/historias", get(historias::buscar))
        .route("/panel/historias/paciente/{id}", get(historias::del_paciente).post(historias::abrir))
        .route("/panel/historias/{id}", get(historias::consulta).post(historias::guardar_consulta))
        .route(
            "/panel/historias/{id}/antecedentes",
            get(historias::ver_antecedentes).post(historias::guardar_antecedentes),
        )
        .route("/panel/historias/{id}/examen", get(historias::examen).post(historias::guardar_examen))
        .route(
            "/panel/historias/{id}/diagnostico",
            get(historias::diagnostico).post(historias::guardar_diagnostico),
        )
        .route("/panel/historias/{id}/plan", get(historias::plan).post(historias::guardar_plan))
        .route(
            "/panel/historias/{id}/formulas",
            get(historias::formulas).post(historias::crear_formula),
        )
        .route("/panel/historias/{id}/formulas/{fid}/borrar", post(historias::borrar_formula))
        .route("/panel/historias/{id}/ordenes", get(historias::ordenes).post(historias::crear_orden))
        .route("/panel/historias/{id}/ordenes/{oid}/borrar", post(historias::borrar_orden))
        .route(
            "/panel/historias/{id}/imagenes",
            get(historias::imagenes)
                .post(historias::subir_imagen)
                .layer(DefaultBodyLimit::max(crate::rutas::MAX_SUBIDA)),
        )
        .route("/panel/historias/{id}/imagenes/{iid}/borrar", post(historias::borrar_imagen))
        .route("/panel/historias/{id}/firmar", post(historias::firmar))
        .route("/panel/historias/{id}/borrar", post(historias::borrar))
        // ── Superadministración ─────────────────────────────────────────────
        //
        // La única pantalla del panel que NO es de un prestador: no lleva
        // `Contexto` ni `panel::menu`, porque el superadmin no pertenece a
        // ninguna institución y `contexto()` le devuelve —con razón— la página
        // de «esta cuenta no tiene panel».
        .route("/panel/superadmin", get(superadmin::listar).post(superadmin::crear_ips))
        .route("/panel/superadmin/{id}", get(superadmin::ficha))
        .route("/panel/superadmin/{id}/licencia", post(superadmin::fijar_licencia))
        .route("/panel/superadmin/{id}/modulos", post(superadmin::fijar_modulos))
        .route("/panel/superadmin/{id}/admin", post(superadmin::crear_admin))
        // Ambulancias: cinco vistas —parque, traslados, alta, ficha y despacho—
        // donde Angular tenía dos pestañas y dos diálogos. Los tramos ESTÁTICOS
        // van antes de `{id}` en la lectura, pero no dependen de eso: axum
        // resuelve el estático con prioridad sobre el parámetro, que es lo mismo
        // que ya hace `/api/ambulancias/traslados` conviviendo con
        // `/api/ambulancias/{id}`.
        .route("/panel/ambulancias", get(ambulancias::listar))
        .route("/panel/ambulancias/traslados", get(ambulancias::traslados))
        .route(
            "/panel/ambulancias/traslados/{id}/finalizar",
            post(ambulancias::finalizar),
        )
        .route(
            "/panel/ambulancias/traslados/{id}/cancelar",
            post(ambulancias::cancelar),
        )
        .route(
            "/panel/ambulancias/despachar",
            get(ambulancias::despacho).post(ambulancias::despachar),
        )
        .route("/panel/ambulancias/nueva", get(ambulancias::nueva).post(ambulancias::crear))
        .route(
            "/panel/ambulancias/{id}",
            get(ambulancias::ficha).post(ambulancias::guardar),
        )
        .route("/panel/ambulancias/{id}/baja", post(ambulancias::baja))
        // Los siete paneles de estadísticas traen sus propias rutas, y las
        // construyen desde la MISMA lista con la que dibujan su menú: dos
        // listas dejarían un panel en el menú apuntando a un 404.
        .merge(estadisticas::rutas())
        // Laboratorio: la hermana de imagenología. Leer solo pide sesión; las
        // cinco escrituras exigen `EscribirLaboratorio`. Los tramos estáticos
        // —`/perfiles`— conviven con `{id}` igual que en `/api`.
        .route("/panel/laboratorio", get(laboratorio::listar))
        .route(
            "/panel/laboratorio/perfiles",
            get(laboratorio::perfiles).post(laboratorio::crear_perfil),
        )
        .route(
            "/panel/laboratorio/perfiles/{id}/retirar",
            post(laboratorio::retirar_perfil),
        )
        .route("/panel/laboratorio/{id}", get(laboratorio::ver))
        .route("/panel/laboratorio/{id}/resultado", post(laboratorio::guardar))
        .route("/panel/laboratorio/{id}/validar", post(laboratorio::validar))
        .route("/panel/laboratorio/{id}/iniciar", post(laboratorio::iniciar))
        // Agenda médica. NO es un módulo del catálogo —añadir uno cambia lo
        // que se vende, y eso es decisión comercial— así que sus rutas van
        // exentas de licencia, una por una y con su motivo, igual que las de
        // `/api/agenda`. Mirar no pide capacidad; las seis escrituras exigen
        // `GestionarAgenda`.
        .route("/panel/agenda", get(agenda::horarios).post(agenda::crear))
        .route(
            "/panel/agenda/bloqueos",
            get(agenda::bloqueos).post(agenda::crear_bloqueo),
        )
        .route(
            "/panel/agenda/bloqueos/{id}/borrar",
            post(agenda::borrar_bloqueo),
        )
        .route("/panel/agenda/indicadores", get(agenda::indicadores))
        .route("/panel/agenda/{id}", get(agenda::ficha).post(agenda::guardar))
        .route("/panel/agenda/{id}/borrar", post(agenda::borrar))
        .route("/panel/agenda/{id}/bloques", post(agenda::anadir_bloque))
        .route(
            "/panel/agenda/{id}/bloques/{bloque_id}/borrar",
            post(agenda::borrar_bloque),
        )
        // Nutrición: leer no pide capacidad; las cuatro escrituras exigen
        // `EscribirNutricion`, la misma que sus rutas JSON —incluido el
        // borrado, que en `/api` estuvo abierto y se cerró aparte—.
        .route("/panel/nutricion", get(nutricion::listar).post(nutricion::crear))
        .route(
            "/panel/nutricion/{id}",
            get(nutricion::ficha).post(nutricion::guardar),
        )
        .route("/panel/nutricion/{id}/borrar", post(nutricion::borrar))
        .route("/panel/nutricion/{id}/plan", post(nutricion::plan))
        // Quirófano. Programar exige `EscribirClinico` —es un acto de la
        // historia— y conducir la cirugía, `AtenderUrgencias`: las dos mismas
        // que exigen sus rutas JSON.
        .route("/panel/quirofano", get(quirofano::agenda))
        .route("/panel/quirofano/salas", get(quirofano::salas))
        .route(
            "/panel/quirofano/programar",
            get(quirofano::formulario).post(quirofano::programar),
        )
        .route("/panel/quirofano/{id}", get(quirofano::ficha))
        .route("/panel/quirofano/{id}/checklist", post(quirofano::checklist))
        .route("/panel/quirofano/{id}/iniciar", post(quirofano::iniciar))
        .route("/panel/quirofano/{id}/completar", post(quirofano::completar))
        .route("/panel/quirofano/{id}/cancelar", post(quirofano::cancelar))
        .route("/panel/quirofano/{id}/suspender", post(quirofano::suspender))
        // Farmacia: cinco vistas donde Angular tenía cinco pestañas. Leer solo
        // pide sesión —enfermería necesita saber si queda adrenalina— y las
        // seis escrituras exigen `GestionarFarmacia`.
        .route("/panel/farmacia", get(farmacia::inventario).post(farmacia::crear))
        .route("/panel/farmacia/formulas", get(farmacia::formulas))
        .route("/panel/farmacia/dispensar", post(farmacia::dispensar))
        .route("/panel/farmacia/dispensaciones", get(farmacia::dispensaciones))
        .route("/panel/farmacia/movimientos", get(farmacia::movimientos))
        .route(
            "/panel/farmacia/suministros",
            get(farmacia::suministros).post(farmacia::suministrar),
        )
        .route(
            "/panel/farmacia/proveedores",
            get(farmacia::proveedores).post(farmacia::crear_proveedor),
        )
        .route(
            "/panel/farmacia/proveedores/{id}/baja",
            post(farmacia::baja_proveedor),
        )
        .route("/panel/farmacia/{id}", get(farmacia::ficha).post(farmacia::guardar))
        .route("/panel/farmacia/{id}/entrada", post(farmacia::entrada))
        // Facturación. Leer solo pide sesión —quien responde en ventanilla por
        // el estado de una factura no tiene por qué poder radicarla— y las
        // ocho escrituras exigen `GestionarFacturacion`.
        .route("/panel/facturacion", get(facturacion::facturas).post(facturacion::crear))
        .route(
            "/panel/facturacion/glosas",
            get(facturacion::glosas),
        )
        .route(
            "/panel/facturacion/glosas/{id}/responder",
            post(facturacion::responder_glosa),
        )
        .route(
            "/panel/facturacion/contratos",
            get(facturacion::contratos).post(facturacion::crear_contrato),
        )
        .route("/panel/facturacion/{id}", get(facturacion::ficha))
        .route("/panel/facturacion/{id}/detalles", post(facturacion::agregar_detalle))
        .route(
            "/panel/facturacion/{id}/detalles/{detalle_id}/borrar",
            post(facturacion::borrar_detalle),
        )
        .route("/panel/facturacion/{id}/radicar", post(facturacion::radicar))
        .route("/panel/facturacion/{id}/estado", post(facturacion::cambiar_estado))
        .route("/panel/facturacion/{id}/glosas", post(facturacion::agregar_glosa))
        // Administración: ocho pestañas. Aquí LEER también exige capacidad
        // —`AdministrarInstitucion` en las veintiséis rutas de `/api/admin`— y
        // la matriz de capacidades exige la de quien nombra.
        .route("/panel/admin", get(admin::institucion).post(admin::guardar_institucion))
        .route(
            "/panel/admin/configuracion",
            get(admin::configuracion).post(admin::guardar_configuracion),
        )
        .route(
            "/panel/admin/seguridad",
            get(admin::seguridad).post(admin::guardar_seguridad),
        )
        .route("/panel/admin/campos", get(admin::campos).post(admin::crear_campo))
        .route("/panel/admin/campos/{id}/borrar", post(admin::borrar_campo))
        .route("/panel/admin/catalogos", get(admin::catalogos))
        .route("/panel/admin/especialidades", post(admin::crear_especialidad))
        .route("/panel/admin/entidades", post(admin::crear_entidad))
        .route("/panel/admin/usuarios", get(admin::usuarios).post(admin::crear_usuario))
        .route("/panel/admin/usuarios/{id}/activo", post(admin::alternar_activo))
        .route(
            "/panel/admin/usuarios/{id}/password",
            post(admin::restablecer_password),
        )
        .route(
            "/panel/admin/usuarios/{id}/nombre",
            post(admin::fijar_nombre),
        )
        .route(
            "/panel/admin/capacidades",
            get(admin::capacidades).post(admin::guardar_capacidades),
        )
        .route("/panel/admin/auditoria", get(admin::auditoria))
}

// ── Lo que toda pantalla necesita antes de pintar ────────────────────────────

/// De qué institución es quien mira, qué módulos tiene su plan y **qué puede
/// hacer**.
///
/// # Por qué las tres cosas juntas y no cada una por su lado
///
/// Porque las catorce pantallas repetían las mismas ocho líneas —resolver la
/// institución, leer los módulos, y decidir entre `SinPanel` y `averia()`— y
/// porque el menú necesita la tercera. Sin las capacidades, el menú ofrece
/// entradas que llevan a «esta sección no le corresponde»: el sidebar de Angular
/// que se está migrando SÍ las escondía (`roles: ['admin','superadmin']` en
/// RIPS), así que pintarlas sería perder una conducta al migrar.
///
/// # La respuesta a «¿puede?» es la MISMA que da `Autorizado`
///
/// Sale de `estado.capacidades.ajustes(...)`, la matriz de la institución con su
/// caché — no de `rol.puede()` a secas. Eso importa: una institución puede
/// haberle concedido `GenerarRips` a otro rol, y un menú que consultara el
/// defecto nacional escondería una pantalla que el servidor sí abre. Sería la
/// segunda lista de #534 otra vez, con el menú diciendo una cosa y la puerta
/// haciendo otra.
pub struct Contexto {
    pub habilitados: Vec<String>,
    /// `None` = un rol que no se reconoce. **No puede nada**, igual que en
    /// `Autorizado`: un rol corrupto falla cerrado.
    rol: Option<medico_dominio::rol::RolUsuario>,
    ajustes: std::sync::Arc<medico_dominio::ajustes::Ajustes>,
}

impl Contexto {
    /// ¿Está este módulo en el plan de la institución?
    pub fn tiene(&self, modulo: &str) -> bool {
        self.habilitados.iter().any(|h| h == modulo)
    }

    /// ¿Puede quien mira esta capacidad, según la matriz de SU institución?
    pub fn puede(&self, capacidad: medico_dominio::rol::Capacidad) -> bool {
        self.rol.is_some_and(|r| self.ajustes.puede(r, capacidad))
    }
}

/// El contexto de la pantalla, o la respuesta que hay que devolver en su lugar.
///
/// Los dos finales que no son un contexto ya estaban en las catorce pantallas y
/// no cambian: una cuenta sin institución ve la pantalla que lo explica, y una
/// base que no responde ve la avería. Lo que sí se corrige es que la matriz de
/// capacidades **no se puede saltar**: si no se puede leer, es avería y no un
/// menú recortado (directiva 20 — un menú sin entradas diría que no compró nada).
pub async fn contexto(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
) -> Result<Contexto, Response> {
    // El gate de #538 (contraseña por restablecer → a cambiarla antes de nada)
    // NO vive aquí: vive en los extractores de sesión `EnSesion` y `EnSesionCon`,
    // que toda pantalla del panel atraviesa —también las que no construyen
    // `Contexto`, como el fragmento del buscador de paciente—. Ponerlo solo aquí
    // dejaba esos fragmentos fuera del gate.
    let institucion = institucion_de(u).map_err(|sin| sin.into_response())?;
    let habilitados = match medico_datos::institucion::modulos_habilitados(&e.pool, institucion)
        .await
    {
        Ok(Some(h)) => h,
        // La institución dejó de existir bajo una sesión viva, o la base no
        // respondió: las dos son avería, no «usted no tiene módulos».
        Ok(None) | Err(_) => return Err(averia()),
    };
    let ajustes = match e.capacidades.ajustes(&e.pool, institucion).await {
        Ok(a) => a,
        Err(_) => return Err(averia()),
    };
    Ok(Contexto {
        habilitados,
        rol: u.rol(),
        ajustes,
    })
}

/// La licencia del módulo en el que ESTE manejador va a escribir.
///
/// Las pantallas compartidas —`/panel/enfermeria`, `/panel/anestesia`,
/// `/panel/pyp`, `/panel/habilitacion`— las abre cualquiera de sus licencias,
/// porque son un solo momento de trabajo y para LEER eso es lo correcto: cada
/// sección se dibuja solo si `cx.tiene(…)`. Para ESCRIBIR no vale, y la capa de
/// licencia no puede suplirlo — ve la ruta, no la tabla.
///
/// Se lee la licencia otra vez en vez de recibir el `Contexto` ya construido
/// porque estos manejadores no dibujan nada y no lo tienen; son dos consultas
/// sobre una petición que va a escribir en la historia clínica.
pub async fn exige_licencia(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    modulo: &'static str,
) -> Result<(), Response> {
    let cx = contexto(e, u).await?;
    if cx.tiene(modulo) {
        Ok(())
    } else {
        Err(crate::licencia::no_licenciado(modulo))
    }
}

/// De qué institución es quien mira, o la pantalla que lo explica.
///
/// El superadmin no pertenece a ninguna, y el panel es la consola de UN
/// prestador: se le dice, en vez de enseñarle una pantalla vacía que parece
/// rota.
pub fn institucion_de(u: &medico_datos::usuario::Usuario) -> Result<i64, SinPanel> {
    match u.ambito() {
        medico_dominio::Ambito::Institucion(id) => Ok(id),
        medico_dominio::Ambito::Superadmin => Err(SinPanel::Superadmin),
        medico_dominio::Ambito::Ninguno => Err(SinPanel::Ninguna),
    }
}

/// Por qué esta cuenta no tiene panel.
///
/// Es un enum pequeño y **no una `Response` ya armada**, siguiendo el precedente
/// que este repositorio fijó en `capacidades::institucion_de`: una `Response`
/// entera en la variante `Err` pesa 128 bytes que se copian en cada retorno del
/// camino feliz (`clippy::result_large_err`). La pantalla la arma el llamante.
#[derive(Clone, Copy)]
pub enum SinPanel {
    /// El dueño del sistema, que no es cliente de ninguna institución.
    Superadmin,
    /// Una cuenta a la que nadie le asignó institución.
    Ninguna,
}

impl IntoResponse for SinPanel {
    fn into_response(self) -> Response {
        // 200 y no 403: la cuenta es válida y no ha hecho nada mal; lo que pasa
        // es que este panel es el de UN prestador. Un 403 mandaría a buscar un
        // permiso que no existe.
        let (titulo, encabezado, texto) = match self {
            SinPanel::Superadmin => (
                "Cuenta de sistema",
                "Esta cuenta no tiene panel clínico",
                "Su usuario administra el sistema entero, no un prestador concreto. \
                 El panel clínico es de una institución; lo suyo es la consola del \
                 sistema.",
            ),
            SinPanel::Ninguna => (
                "Sin institución",
                "Su cuenta no pertenece a ninguna institución",
                "Pídale a un administrador que le asigne una. Hasta entonces no hay \
                 información que mostrarle.",
            ),
        };
        (
            StatusCode::OK,
            pagina(
                titulo,
                None,
                html! {
                    section class="tarjeta estrecha" {
                        h1 { (encabezado) }
                        p { (texto) }
                        // **El enlace es la mitad que faltaba.** Hasta el
                        // 2026-08-08 el superadmin entraba, aterrizaba aquí y no
                        // tenía a dónde ir: su pantalla no existía. Ahora existe,
                        // y una página que explica por qué no hay nada sin decir
                        // dónde SÍ lo hay es un callejón.
                        @if matches!(self, SinPanel::Superadmin) {
                            p { a href="/panel/superadmin" { "Ir a la consola del sistema" } }
                        }
                    }
                },
            ),
        )
            .into_response()
    }
}

/// El `State` no se usa en los recursos estáticos, pero axum exige que el
/// router entero comparta uno. Se declara aquí para que las pruebas puedan
/// montar solo el panel.
#[allow(dead_code)]
fn _estado_usado(_: State<Estado>) {}
