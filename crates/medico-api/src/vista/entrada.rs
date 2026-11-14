// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! La pantalla de entrada del panel.
//!
//! **No decide nada.** Llama a [`crate::rutas::intentar_acceso`], que es la misma
//! función que atiende `POST /api/acceso`, y traduce sus cuatro finales a cuatro
//! pantallas. El freno al ensayo de contraseñas, el hash de descarte para un
//! correo que no existe y la decisión de qué se le anota a quién viven ahí y
//! **no se copian aquí** (directiva 1).
//!
//! # Los textos son los del dominio, y están redactados a propósito
//!
//! «Credenciales incorrectas» no distingue «ese correo no existe» de «la clave
//! está mal»: distinguirlos convertiría la pantalla en un listado de qué cuentas
//! hay en esta institución. Igual con el 429, que se responde con los segundos
//! que faltan y sin decir si la cuenta existe.

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use crate::auth::Estado;
use crate::rutas::{Acceso, Credenciales};
use crate::sesion;
use crate::vista::{aviso, pagina, Tono};

/// El formulario, con un aviso opcional encima.
///
/// El correo se devuelve para no obligar a reteclearlo tras un fallo; la clave
/// **no**, nunca, ni siquiera en un `value` — un formulario que devuelve la
/// contraseña la deja en el historial del navegador y en cualquier proxy que
/// registre cuerpos.
fn pantalla(correo: &str, mensaje: Option<Markup>) -> Markup {
    pagina(
        "Entrar",
        None,
        html! {
            section class="tarjeta estrecha entrada" {
                h1 { "medico" }
                p class="sutil" { "Historia clínica y facturación para prestadores de salud" }
                @if let Some(m) = mensaje { (m) }
                // POST normal, sin htmx: la entrada tiene que funcionar aunque
                // el JavaScript no cargue. Es la única pantalla de la que eso se
                // puede decir sin discutirlo — sin ella no hay panel.
                form method="post" action="/panel/entrada" class="apilado" {
                    label for="email" { "Correo" }
                    input #email type="email" name="email" value=(correo)
                          required autocomplete="username" autofocus;
                    label for="clave" { "Contraseña" }
                    input #clave type="password" name="clave"
                          required autocomplete="current-password";
                    button type="submit" class="principal" { "Entrar" }
                }
            }
        },
    )
}

pub async fn formulario() -> Markup {
    pantalla("", None)
}

#[derive(Deserialize)]
pub struct Enviado {
    email: String,
    clave: String,
}

/// El POST de la entrada. Cuatro finales, cuatro pantallas.
pub async fn enviar(
    State(e): State<Estado>,
    cabeceras: HeaderMap,
    extensiones: axum::http::Extensions,
    Form(f): Form<Enviado>,
) -> Response {
    let cred = Credenciales { email: f.email.clone(), clave: f.clave };
    match crate::rutas::intentar_acceso(&e, &cabeceras, &extensiones, &cred).await {
        Acceso::Entra(cookie) => (
            // 303: convierte el POST en un GET, así que recargar la pantalla de
            // destino no reenvía la contraseña.
            StatusCode::SEE_OTHER,
            [
                (
                    header::SET_COOKIE,
                    sesion::cabecera_abrir(&cookie, e.minutos, e.tras_proxy),
                ),
                (header::LOCATION, "/panel".to_string()),
            ],
        )
            .into_response(),

        // La contraseña valía; falta el código. Se deja el PENDIENTE y se manda
        // a la pantalla del segundo paso — con 303, igual que la entrada, para
        // que recargar no reenvíe la contraseña.
        Acceso::SegundoPaso(pendiente) => (
            StatusCode::SEE_OTHER,
            [
                (
                    header::SET_COOKIE,
                    sesion::cabecera_pendiente_abrir(&pendiente, e.tras_proxy),
                ),
                (header::LOCATION, "/panel/entrada/codigo".to_string()),
            ],
        )
            .into_response(),

        Acceso::Credenciales => (
            StatusCode::UNAUTHORIZED,
            pantalla(
                &f.email,
                Some(aviso(Tono::Error, "Credenciales incorrectas.")),
            ),
        )
            .into_response(),

        Acceso::Frenado(segundos) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, segundos.to_string())],
            pantalla(
                &f.email,
                Some(aviso(
                    Tono::Error,
                    &format!(
                        "Demasiados intentos. Espere {segundos} segundos y vuelva a probar. \
                         No hay nadie a quien pedirle que le desbloquee: la espera se agota sola."
                    ),
                )),
            ),
        )
            .into_response(),

        // NO se le echa la culpa al usuario: no ha fallado su contraseña, ha
        // fallado el servidor. Decirle «credenciales incorrectas» aquí lo manda
        // a cambiar una clave que estaba bien.
        Acceso::NoSePudo => (
            StatusCode::SERVICE_UNAVAILABLE,
            pantalla(
                &f.email,
                Some(aviso(
                    Tono::Averia,
                    "No se pudo comprobar el acceso en este momento. No es su contraseña: \
                     el servidor no pudo consultar la base. Inténtelo en un minuto.",
                )),
            ),
        )
            .into_response(),
    }
}

/// La pantalla del SEGUNDO paso: seis dígitos, o un código de recuperación.
///
/// No dice de quién es la cuenta ni si el enrolamiento fue reciente: quien llega
/// aquí trae el pendiente, pero esta pantalla se sirve sin comprobarlo a
/// propósito —comprobarlo aquí solo cambiaría el mensaje, y el POST sí lo
/// comprueba—. Lo que NO se hace es adelantar información sobre la cuenta.
fn pantalla_codigo(mensaje: Option<Markup>) -> Markup {
    pagina(
        "Código de verificación",
        None,
        html! {
            section class="tarjeta estrecha entrada" {
                h1 { "Código de verificación" }
                p class="sutil" {
                    "Abra su aplicación de autenticación y teclee el código de seis dígitos."
                }
                @if let Some(m) = mensaje { (m) }
                form method="post" action="/panel/entrada/codigo" class="apilado" {
                    label for="codigo" { "Código" }
                    // `inputmode=numeric` saca el teclado numérico en el móvil,
                    // que es donde está el autenticador. NO `type=number`: ése
                    // se come los ceros a la izquierda, y un código que empieza
                    // por cero es uno de cada diez.
                    input #codigo type="text" name="codigo" required autofocus
                          inputmode="numeric" autocomplete="one-time-code"
                          maxlength="17" placeholder="000000";
                    button type="submit" class="principal" { "Verificar" }
                }
                p class="sutil" {
                    "¿Perdió el teléfono? Teclee aquí uno de sus códigos de recuperación."
                }
                form method="post" action="/panel/entrada" class="apilado" {
                    button type="submit" class="secundario" formaction="/panel/entrada"
                           formmethod="get" { "Volver a empezar" }
                }
            }
        },
    )
}

/// `GET /panel/entrada/codigo`
pub async fn formulario_codigo() -> Markup {
    pantalla_codigo(None)
}

#[derive(Deserialize)]
pub struct CodigoEnviado {
    codigo: String,
}

/// `POST /panel/entrada/codigo` — el segundo paso, en el panel.
pub async fn enviar_codigo(
    State(e): State<Estado>,
    cabeceras: HeaderMap,
    extensiones: axum::http::Extensions,
    Form(f): Form<CodigoEnviado>,
) -> Response {
    match crate::rutas::verificar_segundo_paso(&e, &cabeceras, &extensiones, &f.codigo).await {
        // `AppendHeaders`, por lo mismo que en `/api/2fa/verificar`: con un array
        // de pares axum INSERTA, y el segundo `Set-Cookie` pisaría al primero
        // dejando la respuesta sin sesión.
        crate::rutas::SegundoPaso::Entra(cookie) => (
            StatusCode::SEE_OTHER,
            axum::response::AppendHeaders([
                (
                    header::SET_COOKIE,
                    sesion::cabecera_abrir(&cookie, e.minutos, e.tras_proxy),
                ),
                (
                    header::SET_COOKIE,
                    sesion::cabecera_pendiente_cerrar(e.tras_proxy),
                ),
                (header::LOCATION, "/panel".to_string()),
            ]),
        )
            .into_response(),

        // El pendiente se QUEMA también aquí: si sobreviviera a un código
        // equivocado, valdría para seguir probando con la contraseña ya dada por
        // buena, y el ensayo de códigos no tendría más freno que la ventana.
        crate::rutas::SegundoPaso::Incorrecto => (
            StatusCode::UNAUTHORIZED,
            [(
                header::SET_COOKIE,
                sesion::cabecera_pendiente_cerrar(e.tras_proxy),
            )],
            pantalla(
                "",
                Some(aviso(
                    Tono::Error,
                    "Código incorrecto. Vuelva a entrar con su correo y su contraseña.",
                )),
            ),
        )
            .into_response(),

        crate::rutas::SegundoPaso::SinPendiente => (
            StatusCode::SEE_OTHER,
            [(header::LOCATION, "/panel/entrada".to_string())],
        )
            .into_response(),

        // **El pendiente YA se quemó**, y este comentario decía lo contrario hasta
        // el 2026-08-12: `verificar_segundo_paso` lo consume ANTES de mirar el
        // freno, así que llegar aquí significa que el token está gastado y hay
        // que rehacer la contraseña. Se corrige el texto y no el orden: gastarlo
        // siempre que se presenta es la postura conservadora —un token que
        // sobrevive a una petición es un token reutilizable— y el precio, tener
        // que volver a entrar tras esperar, lo paga quien ya lleva cinco fallos.
        //
        // Un comentario que describe un comportamiento que el código no tiene es
        // exactamente el defecto que la revisión de esta misma rama encontró dos
        // veces (el `DELETE` sin `usado_en` y el `SinReposo` que tapaba errores
        // de base). No se deja pasar por ser «solo un comentario».
        crate::rutas::SegundoPaso::Frenado(segundos) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, segundos.to_string())],
            pantalla_codigo(Some(aviso(
                Tono::Error,
                &format!(
                    "Demasiados intentos. Espere {segundos} segundos y vuelva a probar. \
                     La espera se agota sola; no hay a quien pedirle que le desbloquee."
                ),
            ))),
        )
            .into_response(),

        // Ni «código incorrecto» ni «vuelva a entrar»: el servidor no puede leer
        // su propio secreto, y mandar a esta persona a probar otra vez la deja
        // intentándolo contra una avería hasta que el freno la pare.
        crate::rutas::SegundoPaso::NoSePudo => (
            StatusCode::SERVICE_UNAVAILABLE,
            pantalla_codigo(Some(aviso(
                Tono::Averia,
                "No se pudo comprobar el código en este momento. No es su código: el \
                 servidor no pudo leer la configuración del segundo factor. Avise a quien \
                 administra el sistema.",
            ))),
        )
            .into_response(),
    }
}

/// Salir: borra la cookie y devuelve a la entrada.
///
/// Va por POST y no por un enlace a propósito: un `GET /panel/salir` lo dispara
/// cualquier cosa que precargue enlaces —un antivirus, el propio navegador— y
/// echaría de la sesión sin que nadie lo pidiera.
pub async fn salir(State(e): State<Estado>) -> Response {
    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, sesion::cabecera_cerrar(e.tras_proxy)),
            (header::LOCATION, "/panel/entrada".to_string()),
        ],
    )
        .into_response()
}

/// **Cerrar la sesión en TODOS los dispositivos.**
///
/// Sube `token_version`, con lo que toda cookie ya emitida deja de valer en la
/// siguiente petición — incluida la de aquí, que por eso se borra también.
///
/// Llama a `medico_datos::usuario::revocar_sesiones`, la MISMA función que
/// `POST /api/salir-todo`, y actúa sobre el usuario AUTENTICADO y nunca sobre un
/// id que venga del cliente: así nadie revoca las sesiones de otro.
///
/// **Si la revocación falla, NO se finge el cierre.** Decirle a alguien que
/// cerró sus sesiones cuando siguen vivas es peor que no ofrecer el botón: es la
/// acción que se usa justo cuando se sospecha que alguien más entró.
pub async fn salir_todo(State(e): State<Estado>, cabeceras: HeaderMap) -> Response {
    // La sesión a mano, no por `EnSesion`: un usuario con la contraseña por
    // restablecer también tiene que poder cerrar sus sesiones, y el gate de
    // `EnSesion` lo mandaría a cambiarla en vez de dejarlo salir.
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };
    match medico_datos::usuario::revocar_sesiones(&e.pool, u.id).await {
        Ok(true) => (
            StatusCode::SEE_OTHER,
            [
                (header::SET_COOKIE, sesion::cabecera_cerrar(e.tras_proxy)),
                (header::LOCATION, "/panel/entrada".to_string()),
            ],
        )
            .into_response(),
        // `false` = la sentencia no tocó ninguna fila: el usuario que acabamos
        // de autenticar ya no está. No hubo revocación, así que tampoco hay
        // redirección con cara de éxito.
        Ok(false) | Err(_) => crate::vista::averia(),
    }
}

/// Redirección de cortesía: `/panel/entrada` con sesión ya abierta va al panel.
#[allow(dead_code)]
pub fn ya_dentro() -> Response {
    Redirect::to("/panel").into_response()
}

// ── Cambiar la propia contraseña (#538) ──────────────────────────────────────
//
// La contraseña que un administrador restablece deja `requiere_cambio_password`
// en `true`, y `crate::vista::contexto` rebota TODA pantalla del panel aquí
// mientras siga puesta. Esta es la única que NO llama a `contexto` —por eso no
// entra en el bucle—, y usa `EnSesion` a secas: sirve tanto para el cambio
// OBLIGADO (tras un restablecimiento) como para el voluntario.

/// El formulario. La clave nunca se devuelve en un `value`, ni la actual ni la
/// nueva: un formulario que la devolviera la dejaría en el historial.
fn pantalla_cambio(obligado: bool, mensaje: Option<Markup>) -> Markup {
    pagina(
        "Cambiar contraseña",
        None,
        html! {
            section class="tarjeta estrecha" {
                h1 { "Cambiar contraseña" }
                @if obligado {
                    (aviso(
                        Tono::Error,
                        "Un administrador restableció su contraseña. Debe cambiarla antes de \
                         continuar; hasta entonces no se puede usar el resto del panel.",
                    ))
                }
                @if let Some(m) = mensaje { (m) }
                form method="post" action="/panel/cambiar-clave" class="apilado" {
                    label for="actual" { "Contraseña actual" }
                    input #actual type="password" name="actual" required
                          autocomplete="current-password" autofocus;
                    label for="nueva" { "Contraseña nueva" }
                    input #nueva type="password" name="nueva" required
                          autocomplete="new-password";
                    label for="confirmar" { "Repetir la nueva" }
                    input #confirmar type="password" name="confirmar" required
                          autocomplete="new-password";
                    button type="submit" class="principal" { "Cambiar" }
                }
            }
        },
    )
}

pub async fn cambiar_clave(State(e): State<Estado>, cabeceras: HeaderMap) -> Response {
    // La sesión se carga a MANO, no por `EnSesion`: ese extractor rebota aquí
    // cuando la bandera está puesta, y usarlo encerraría esta pantalla en un
    // bucle contra sí misma.
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };
    pantalla_cambio(u.requiere_cambio_password, None).into_response()
}

#[derive(Deserialize)]
pub struct CambioForm {
    actual: String,
    nueva: String,
    confirmar: String,
}

/// El POST. Reusa `crate::rutas::cambiar_clave_propia` —la MISMA lógica que
/// `POST /api/mi-clave`: exige la actual, aplica la política de la institución,
/// sube `token_version` y limpia la bandera—, y como eso echa a las sesiones
/// viejas (incluida la de aquí), reemite la cookie antes de mandar al panel.
pub async fn cambiar_clave_enviar(
    State(e): State<Estado>,
    cabeceras: HeaderMap,
    Form(f): Form<CambioForm>,
) -> Response {
    // La sesión a mano, por lo mismo que el GET: `EnSesion` rebotaría aquí.
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };
    // La repetición se comprueba aquí, en la pantalla: es una ayuda de captura,
    // no una regla de seguridad, y el núcleo compartido no la conoce.
    if f.nueva != f.confirmar {
        return pantalla_cambio(
            u.requiere_cambio_password,
            Some(aviso(Tono::Error, "La nueva contraseña y su repetición no coinciden.")),
        )
        .into_response();
    }
    match crate::rutas::cambiar_clave_propia(&e, &u, &f.actual, &f.nueva).await {
        Ok(cookie) => (
            // 303 + cookie nueva: la recarga no reenvía la contraseña y la sesión
            // sigue viva con la versión nueva.
            StatusCode::SEE_OTHER,
            [
                (
                    header::SET_COOKIE,
                    sesion::cabecera_abrir(&cookie, e.minutos, e.tras_proxy),
                ),
                (header::LOCATION, "/panel".to_string()),
            ],
        )
            .into_response(),
        Err((codigo, motivo)) => (
            codigo,
            pantalla_cambio(u.requiere_cambio_password, Some(aviso(Tono::Error, &motivo))),
        )
            .into_response(),
    }
}
