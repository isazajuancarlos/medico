// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza

//! El enrolamiento del segundo factor, desde dentro de la sesión.
//!
//! # Tres pantallas y no una
//!
//! `ver` (en qué punto estoy) → `empezar` (el QR) → `confirmar` (los códigos de
//! recuperación, UNA vez). Son tres porque el estado es tres, y colapsarlas
//! dejaría a quien cierre el navegador en medio con un secreto guardado que
//! nunca confirmó — y sin manera de volver a empezar.
//!
//! # Los códigos de recuperación se enseñan UNA vez y no se pueden volver a ver
//!
//! Se guardan hasheados con Argon2, así que el servidor **no puede** volver a
//! mostrarlos ni aunque quisiera. Eso no es una limitación que haya que
//! disculpar: es la razón por la que un volcado de la base no es una lista de
//! diez llaves maestras. La pantalla lo dice antes de enseñarlos, no después.
//!
//! # Retirar el propio factor pide la CONTRASEÑA
//!
//! Estar dentro no basta, y aquí medico se aparta de informes a propósito. Una
//! sesión robada —una cookie copiada de un equipo desatendido— podría quitarle a
//! su dueño el segundo factor sin saber ninguna contraseña, y dejaría la cuenta
//! con una sola puerta justo cuando ya hay alguien dentro. Pedir la contraseña
//! cuesta un formulario y cierra ese camino.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup, PreEscaped};
use medico_datos::segundofactor as sf;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{aviso, pagina, Tono};

/// `GET /panel/segundo-factor`
pub async fn ver(State(e): State<Estado>, cabeceras: HeaderMap) -> Response {
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };
    let Ok(situacion) = sf::situacion(&e.pool, u.id).await else {
        return averia_pantalla();
    };
    let quedan = sf::codigos_sin_usar(&e.pool, u.id).await.unwrap_or(0);
    estado_pantalla(&situacion, quedan, e.clave_reposo.is_some(), None).into_response()
}

fn averia_pantalla() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        pagina(
            "Segundo factor",
            None,
            html! {
                section class="tarjeta estrecha" {
                    (aviso(Tono::Averia, "No se pudo consultar el segundo factor. \
                        No es su cuenta: el servidor no pudo leer la base."))
                }
            },
        ),
    )
        .into_response()
}

fn estado_pantalla(
    situacion: &sf::Situacion,
    codigos_restantes: i64,
    hay_reposo: bool,
    mensaje: Option<Markup>,
) -> Markup {
    pagina(
        "Segundo factor",
        None,
        html! {
            section class="tarjeta estrecha" {
                h1 { "Segundo factor" }
                @if let Some(m) = mensaje { (m) }

                // SIN frase de reposo no se puede empezar, y se dice AQUÍ en vez
                // de dejar que el botón falle: el secreto no se guarda en claro
                // «mientras tanto», así que la pantalla no puede ofrecerlo.
                @if !hay_reposo {
                    (aviso(Tono::Averia, "Este servidor no tiene configurada la frase de \
                        reposo (MEDICO_STORAGE_KEY), y sin ella el secreto del segundo \
                        factor se guardaría en claro. No se puede activar hasta que quien \
                        administra el sistema la configure."))
                } @else {
                    @match situacion {
                        sf::Situacion::Activo { desde } => {
                            p { "Activo desde " strong { (desde.split('T').next().unwrap_or(desde)) } "." }
                            p class="sutil" {
                                "Le quedan " strong { (codigos_restantes) }
                                " códigos de recuperación sin usar."
                            }
                            @if codigos_restantes == 0 {
                                (aviso(Tono::Error, "No le queda ningún código de \
                                    recuperación. Si pierde el teléfono no podrá entrar \
                                    sin que un administrador le retire el segundo factor. \
                                    Vuelva a activarlo para recibir diez nuevos."))
                            }
                            hr;
                            h2 { "Cambiar de teléfono" }
                            p class="sutil" {
                                "Al empezar de nuevo, el teléfono actual deja de servir en el "
                                "acto y su cuenta se queda solo con la contraseña hasta que "
                                "confirme el nuevo. Por eso hace falta que la teclee."
                            }
                            form method="post" action="/panel/segundo-factor/empezar" class="apilado" {
                                label for="clave-nuevo" { "Su contraseña" }
                                input #clave-nuevo type="password" name="clave" required
                                      autocomplete="current-password";
                                button type="submit" class="secundario" {
                                    "Volver a activar con otro teléfono"
                                }
                            }
                            hr;
                            h2 { "Retirar el segundo factor" }
                            p class="sutil" {
                                "Su cuenta volverá a entrar solo con la contraseña. \
                                 Hace falta que la teclee: estar dentro no basta."
                            }
                            form method="post" action="/panel/segundo-factor/retirar" class="apilado" {
                                label for="clave" { "Su contraseña" }
                                input #clave type="password" name="clave" required
                                      autocomplete="current-password";
                                button type="submit" class="peligro" { "Retirar" }
                            }
                        }
                        _ => {
                            p {
                                "Con el segundo factor, entrar pide además un código de seis \
                                 dígitos que cambia cada 30 segundos. Una contraseña robada \
                                 deja de bastar."
                            }
                            @if matches!(situacion, sf::Situacion::Enrolando) {
                                (aviso(Tono::Error, "Empezó a activarlo y no llegó a \
                                    confirmarlo, así que TODAVÍA NO está activo. Vuelva a \
                                    empezar: el código anterior deja de servir."))
                            }
                            form method="post" action="/panel/segundo-factor/empezar" class="apilado" {
                                // Sin factor activo no hay nada que debilitar, así
                                // que no se pide contraseña. El campo viaja vacío
                                // porque el manejador lo comparte con el caso de
                                // arriba, y allí sí decide.
                                input type="hidden" name="clave" value="";
                                button type="submit" class="principal" { "Activar" }
                            }
                        }
                    }
                }
            }
        },
    )
}

/// `POST /panel/segundo-factor/empezar` — genera el secreto y dibuja el QR.
///
/// # Si YA hay un factor activo, pide la contraseña. Y no es simetría: es un
/// agujero que existía
///
/// Empezar de nuevo SUSTITUYE el secreto y deja la cuenta en `Enrolando`, que no
/// exige código. O sea que entre pulsar el botón y confirmar el teléfono nuevo,
/// la cuenta se queda con una sola puerta. Sin contraseña, eso convierte este
/// botón en la vuelta a la esquina de `retirar`: una sesión robada apagaría el
/// segundo factor sin saber ninguna contraseña, que es exactamente lo que
/// `retirar` impide pidiéndola.
///
/// **La primera vez NO la pide**, y la asimetría es la correcta (directiva 21):
/// cuando no hay factor activo no hay nada que debilitar, y exigir la contraseña
/// ahí solo pondría fricción en el camino que queremos que la gente recorra.
pub async fn empezar(
    State(e): State<Estado>,
    cabeceras: HeaderMap,
    Form(f): Form<Retiro>,
) -> Response {
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };

    let Ok(situacion) = sf::situacion(&e.pool, u.id).await else {
        return averia_pantalla();
    };
    if situacion.exige_codigo()
        && crate::auth::autenticar_usuario(&e.pool, &u.email, &f.clave).await.is_none()
    {
        let quedan = sf::codigos_sin_usar(&e.pool, u.id).await.unwrap_or(0);
        return (
            StatusCode::UNAUTHORIZED,
            estado_pantalla(
                &situacion,
                quedan,
                e.clave_reposo.is_some(),
                Some(aviso(
                    Tono::Error,
                    "Esa no es su contraseña. Su segundo factor sigue como estaba.",
                )),
            ),
        )
            .into_response();
    }

    let secreto = sf::generar_secreto();
    if sf::iniciar_enrolamiento(&e.pool, u.id, &secreto, e.clave_reposo.as_deref())
        .await
        .is_err()
    {
        return averia_pantalla();
    }

    let uri = sf::uri_otpauth("medico", &u.email, &secreto);
    // Si el QR no cabe —no puede pasar con un `otpauth://`, pero el tipo lo
    // admite— se sigue SIN dibujo en vez de reventar: el secreto en texto basta
    // para enrolar a mano, que es la salida que la pantalla ya ofrece.
    let svg = guaca::qr::codificar(uri.as_bytes()).ok().map(|c| c.a_svg());

    pagina(
        "Activar el segundo factor",
        None,
        html! {
            section class="tarjeta estrecha" {
                h1 { "Activar el segundo factor" }
                ol {
                    li { "Abra su aplicación de autenticación y escanee este código." }
                    li { "Teclee abajo el código de seis dígitos que le muestre." }
                }
                @if let Some(svg) = &svg {
                    // El SVG lo produce `guaca::qr` a partir de bytes propios y
                    // no lleva nada del exterior salvo el correo, que viaja
                    // porcentaje-codificado dentro del URI.
                    div class="qr" { (PreEscaped(svg.clone())) }
                } @else {
                    (aviso(Tono::Error, "No se pudo dibujar el código. Use la clave \
                        manual de abajo."))
                }
                details {
                    summary { "¿No puede escanear? Teclee la clave a mano" }
                    p class="sutil" { "Cuenta: " code { (u.email) } }
                    p { code class="secreto" { (secreto) } }
                }
                form method="post" action="/panel/segundo-factor/confirmar" class="apilado" {
                    label for="codigo" { "Código de seis dígitos" }
                    input #codigo type="text" name="codigo" required autofocus
                          inputmode="numeric" autocomplete="one-time-code"
                          maxlength="6" placeholder="000000";
                    button type="submit" class="principal" { "Confirmar" }
                }
                p class="sutil" {
                    "Hasta que confirme, su cuenta sigue entrando solo con la contraseña."
                }
            }
        },
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct Codigo {
    codigo: String,
}

/// `POST /panel/segundo-factor/confirmar` — comprueba el código y ACTIVA.
///
/// Es el único sitio donde se ven los códigos de recuperación.
pub async fn confirmar(
    State(e): State<Estado>,
    cabeceras: HeaderMap,
    Form(f): Form<Codigo>,
) -> Response {
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };
    let Ok(g) = sf::guardado(&e.pool, u.id, e.clave_reposo.as_deref()).await else {
        return averia_pantalla();
    };

    // **SOLO DESDE `Enrolando`.** Sin esta guarda, un POST aquí con un TOTP de
    // una ventana FRESCA sobre una cuenta ya `Activo` re-sellaba
    // `totp_confirmado_en` —perdiendo la fecha real de activación que la
    // pantalla enseña— y, peor, **borraba los códigos de recuperación** para
    // entregar diez nuevos. Quien tuviera la pantalla del QR abierta en otra
    // pestaña y la enviara al rato se quedaba sin los códigos que ya había
    // guardado, creyendo que seguían valiendo.
    //
    // El antirreplay no lo tapaba: con el MISMO código sale `Reutilizado` y no
    // llega aquí, pero una ventana nueva aparece cada 30 segundos. Lo encontró
    // la revisión en la nube del 2026-08-12, y es el hueco de la máquina de
    // estados que este mismo módulo declara en su cabecera — `empezar` y
    // `retirar` ya cierran su transición desde `Activo` pidiendo la contraseña;
    // a `confirmar` le faltaba la suya.
    if !matches!(g.situacion, sf::Situacion::Enrolando) {
        let quedan = sf::codigos_sin_usar(&e.pool, u.id).await.unwrap_or(0);
        return (
            StatusCode::CONFLICT,
            estado_pantalla(
                &g.situacion,
                quedan,
                e.clave_reposo.is_some(),
                Some(aviso(
                    Tono::Error,
                    "No hay ningún enrolamiento a medias que confirmar. Si quiere                      cambiar de teléfono, empiece por «Volver a activar»: sus códigos                      de recuperación NO se han tocado.",
                )),
            ),
        )
            .into_response();
    }

    let Some(secreto) = g.secreto else {
        return averia_pantalla();
    };

    let ahora = chrono::Utc::now();
    let contador = match sf::verificar(
        &secreto,
        f.codigo.trim(),
        ahora.timestamp().max(0) as u64,
        g.ultimo_contador.map(|c| c as u64),
    ) {
        sf::Veredicto::Correcto { contador } => contador,
        sf::Veredicto::SecretoIlegible => return averia_pantalla(),
        // NO se activa con un código que no cuadra: activar aquí dejaría a la
        // persona fuera de su cuenta en el siguiente acceso, que es el peor
        // momento para descubrir que el teléfono no estaba bien enrolado.
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                estado_pantalla(
                    &sf::Situacion::Enrolando,
                    0,
                    e.clave_reposo.is_some(),
                    Some(aviso(
                        Tono::Error,
                        "Ese código no coincide. Vuelva a activar y escanee otra vez: \
                         el segundo factor NO quedó activado.",
                    )),
                ),
            )
                .into_response();
        }
    };

    if sf::confirmar(&e.pool, u.id, contador, ahora).await.is_err() {
        return averia_pantalla();
    }

    // Los diez códigos, generados y guardados HASHEADOS. Se enseñan aquí y no
    // vuelven a verse nunca: el servidor no los tiene en claro.
    let claros = sf::generar_codigos_recuperacion(sf::CODIGOS_DE_RECUPERACION);
    let hashes: Vec<String> = claros.iter().filter_map(|c| guaca::claves::hashear(c)).collect();
    if hashes.len() != claros.len() || sf::guardar_codigos(&e.pool, u.id, &hashes, ahora).await.is_err()
    {
        return averia_pantalla();
    }

    pagina(
        "Segundo factor activado",
        None,
        html! {
            section class="tarjeta estrecha" {
                h1 { "Segundo factor activado" }
                (aviso(Tono::Ok, "A partir de ahora, entrar le pedirá el código."))
                h2 { "Sus códigos de recuperación" }
                p {
                    strong { "Guárdelos ahora." }
                    " Son la única forma de entrar si pierde el teléfono, cada uno sirve \
                     UNA vez, y "
                    strong { "esta pantalla no se puede volver a abrir" }
                    ": el servidor los guarda cifrados y no puede enseñárselos otra vez."
                }
                ul class="codigos" {
                    @for c in &claros { li { code { (c) } } }
                }
                p class="sutil" {
                    "Imprímalos o cópielos a donde guarde sus contraseñas. No los deje en \
                     el mismo sitio que su contraseña de medico."
                }
                a href="/panel/segundo-factor" class="boton principal" { "Ya los guardé" }
            }
        },
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct Retiro {
    clave: String,
}

/// `POST /panel/segundo-factor/retirar` — el propio, y pidiendo la contraseña.
pub async fn retirar(
    State(e): State<Estado>,
    cabeceras: HeaderMap,
    Form(f): Form<Retiro>,
) -> Response {
    let Some(u) = crate::auth::usuario_de(&e, &cabeceras).await else {
        return Redirect::to("/panel/entrada").into_response();
    };

    // La contraseña, otra vez. Ver la cabecera del módulo: una sesión robada no
    // puede quitarle a su dueño el segundo factor.
    if crate::auth::autenticar_usuario(&e.pool, &u.email, &f.clave).await.is_none() {
        let situacion = sf::situacion(&e.pool, u.id).await.unwrap_or(sf::Situacion::SinSecreto);
        let quedan = sf::codigos_sin_usar(&e.pool, u.id).await.unwrap_or(0);
        return (
            StatusCode::UNAUTHORIZED,
            estado_pantalla(
                &situacion,
                quedan,
                e.clave_reposo.is_some(),
                Some(aviso(Tono::Error, "Esa no es su contraseña. No se retiró nada.")),
            ),
        )
            .into_response();
    }

    if sf::desactivar(&e.pool, u.id).await.is_err() {
        return averia_pantalla();
    }
    estado_pantalla(
        &sf::Situacion::SinSecreto,
        0,
        e.clave_reposo.is_some(),
        Some(aviso(Tono::Ok, "Segundo factor retirado. Su cuenta vuelve a entrar solo \
            con la contraseña.")),
    )
    .into_response()
}
