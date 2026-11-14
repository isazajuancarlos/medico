// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El armazón del panel: el menú y la pantalla de inicio.
//!
//! # El menú lo decide la LICENCIA **y la capacidad**, y las dos significan algo
//!
//! Se construye con `modulo::CATALOGO` y `modulos_habilitados`, igual que hacía
//! el menú de Angular. La diferencia es que desde el 2026-08-07 (#534) el
//! servidor **ejerce** esa licencia con la capa de [`crate::licencia`]: hasta
//! entonces el menú era la única barrera y vivía en el navegador.
//!
//! Así que este menú nace ya sin la mentira que arrastraba el anterior —cuyo
//! `moduloGuard` documentaba un 403 que nadie emitía—. Ocultar aquí un módulo es
//! comodidad: quien teclee la URL igualmente recibe el 403 de la capa.
//!
//! # Solo se enlaza lo que este panel YA sirve
//!
//! El catálogo tiene 46 módulos; el panel en `maud` va por la pantalla 27 de 29.
//! Un menú que enlazara los 46 llevaría a 404 en la mayoría, que es exactamente
//! el «módulo vacío frente a módulo sin construir» que este producto lleva
//! años distinguiendo a propósito. Los que todavía viven en Angular se enlazan
//! **a Angular**, con su marca, y los que no existen no se enlazan.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};
use medico_dominio::modulo;

use crate::auth::Estado;
use crate::vista::{contexto, pagina, Contexto, EnSesion};

/// Lo que este panel ya sirve sale de [`crate::licencia::PANTALLAS`], que es la
/// MISMA tabla con la que la capa cierra la puerta.
///
/// No hay una segunda lista aquí a propósito: dos listas —una para pintar y otra
/// para vigilar— es exactamente cómo el menú de Angular acabó prometiendo un 403
/// que nadie emitía (#534). Añadir una pantalla es añadir UNA fila.
use crate::licencia::PANTALLAS;


/// **Las pantallas del panel que NO son de un módulo del catálogo.**
///
/// Existen y son páginas, pero no se venden por separado: el personal de la
/// institución es administración, y la agenda no tiene módulo porque añadir uno
/// cambia lo que se vende —es decisión comercial de Juan, no de este archivo—.
/// Por eso están en `PANEL_FUERA_DEL_LICENCIAMIENTO` y no en `PANTALLAS`.
///
/// # Por qué hace falta esta segunda lista, y por qué NO es la segunda lista
/// de #534
///
/// Sin ella, una pantalla exenta de licencia queda **exenta de menú**: se sirve
/// y no se llega a ella más que tecleando la URL. Pasó con `/panel/usuarios`
/// desde el lote 3, y `/panel/agenda` iba a ser la segunda — dos huérfanas son
/// suficiente evidencia de que el patrón se repite.
///
/// No es la divergencia de #534 porque no compite con `PANTALLAS`: aquella era
/// una lista para PINTAR y otra para VIGILAR diciendo cosas distintas del mismo
/// módulo. Aquí cada ruta está en una lista o en la otra, nunca en las dos, y lo
/// vigila `las_pantallas_sin_modulo_estan_exentas_de_licencia` — que exige que
/// cada fila de aquí esté declarada exenta y que ninguna sea de un módulo.
pub(crate) const SIN_MODULO: &[(&str, &str, Option<medico_dominio::rol::Capacidad>)] = &[
    (
        "/panel/usuarios",
        "Personal",
        Some(medico_dominio::rol::Capacidad::GestionarUsuarios),
    ),
    (
        "/panel/admin",
        "Administración",
        Some(medico_dominio::rol::Capacidad::AdministrarInstitucion),
    ),
    // Leer la agenda no pide capacidad —quien agenda desde el mostrador tiene
    // que ver los turnos libres—; definirla exige `GestionarAgenda`, y eso lo
    // ejercen los manejadores de dentro.
    ("/panel/agenda", "Agenda médica", None),
];

/// El menú lateral, acotado a lo que esta institución compró **y a lo que quien
/// mira puede hacer**.
///
/// Los dos filtros son distintos y hacen falta los dos: el módulo dice qué
/// contrató el prestador, y la capacidad, qué le corresponde a esta persona
/// dentro de él. Un admin y un médico de la misma IPS ven menús distintos, que
/// es lo que hacía el sidebar de Angular con `roles: [...]`.
///
/// La capacidad se pregunta al [`Contexto`], o sea a la matriz de la
/// institución: preguntársela al defecto nacional escondería una pantalla que el
/// servidor sí abre para un rol al que esa IPS se la concedió.
pub fn menu(cx: &Contexto, activo: &str) -> Markup {
    let tiene = |n: &str| cx.tiene(n);
    html! {
        nav class="menu" aria-label="Secciones" {
            a class="marca" href="/panel" { "medico" }
            ul {
                // El tablero va a mano y no por `PANTALLAS`: no es un módulo del
                // catálogo —es la portada— y por eso está exento de licencia.
                li {
                    a href="/panel/tablero"
                      aria-current=[("/panel/tablero" == activo).then_some("page")] {
                        "Tablero"
                    }
                }
                @for p in PANTALLAS {
                    // Los DOS filtros: el módulo está en el plan Y quien mira
                    // tiene la capacidad que la pantalla exige, si exige alguna.
                    @if tiene(p.modulo) && p.exige.is_none_or(|c| cx.puede(c)) {
                        li {
                            a href=(p.ruta)
                              aria-current=[(p.ruta == activo).then_some("page")] {
                                (p.etiqueta)
                            }
                        }
                    }
                }
            }
            // Las que no son de un módulo: administración del prestador. Van en
            // su propio grupo porque no dependen de lo que se contrató, sino de
            // lo que esta persona puede hacer.
            @let propias: Vec<_> = SIN_MODULO
                .iter()
                .filter(|(_, _, exige)| exige.is_none_or(|c| cx.puede(c)))
                .collect();
            @if !propias.is_empty() {
                p class="menu-titulo" { "Administración" }
                ul {
                    @for (ruta, etiqueta, _) in propias {
                        li {
                            a href=(ruta) aria-current=[(*ruta == activo).then_some("page")] {
                                (etiqueta)
                            }
                        }
                    }
                }
            }
            // El segundo factor, en el menú de TODOS y no en el de
            // administración: cada cual activa el suyo. Sin esta entrada la
            // pantalla existiría y no habría manera de llegar, que es el defecto
            // que ya tuvo `/panel/usuarios` — una pantalla huérfana es una
            // pantalla que no está.
            div class="menu-grupo" {
                ul {
                    li {
                        a href="/panel/segundo-factor"
                          aria-current=[(activo == "/panel/segundo-factor").then_some("page")] {
                            "Segundo factor"
                        }
                    }
                }
            }
            form method="post" action="/panel/salir" class="menu-salir" {
                button type="submit" class="enlace" { "Salir" }
            }
            // Cerrar en TODOS los dispositivos no se deshace: quien lo pulsa
            // echa también a la tableta de consultorio y al portátil de casa. Va
            // con `data-confirmar`, el comportamiento de `boi` — dos pulsaciones
            // y no un `confirm()` del navegador, que se acepta sin leerlo.
            //
            // **Y sin JavaScript sigue funcionando**: es un `<form>` normal, así
            // que si `boi` no cargara la acción se ejecuta a la primera. Es
            // mejora progresiva (regla de boi): el comportamiento AÑADE una
            // barrera, no la sostiene. La barrera de verdad es que exige sesión.
            form method="post" action="/panel/salir-todo" class="menu-salir" {
                button type="submit" class="enlace"
                        data-confirmar="¿Seguro? Pulse otra vez para cerrar todas" {
                    "Cerrar todas las sesiones"
                }
            }
        }
    }
}

/// La pantalla de inicio: quién eres, de qué institución, y qué tienes.
///
/// No es un tablero de indicadores. El de Angular (`dashboard`) son 81 líneas
/// que pintan contadores; portarlo antes que las pantallas de trabajo sería
/// migrar primero lo que menos se usa.
pub async fn inicio(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    let pendientes: Vec<&str> = modulo::CATALOGO
        .iter()
        .filter(|m| cx.tiene(m.nombre))
        .filter(|m| m.prefijo.is_none())
        .map(|m| m.nombre)
        .collect();

    pagina(
        "Panel",
        Some(menu(&cx, "/panel")),
        html! {
            section class="tarjeta" {
                h1 { "Hola, " (u.nombre) }
                p class="sutil" { (u.email) }
                p {
                    "Su institución tiene " strong { (cx.habilitados.len()) }
                    " módulos en su plan."
                }
                @if !pendientes.is_empty() {
                    p class="sutil" {
                        "De ellos, " (pendientes.len()) " están licenciados y todavía "
                        "no construidos: " (pendientes.join(", ")) ". "
                        "Aparecen aquí para que sepa que los compró y aún no están, "
                        "en vez de encontrarse una pantalla vacía."
                    }
                }
            }
        },
    )
    .into_response()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Ninguna entrada del menú se sale del panel.**
    ///
    /// Sustituye a `ninguna_pantalla_migrada_sigue_anunciandose_como_de_angular`,
    /// que se retiró con `TODAVIA_EN_ANGULAR` el 2026-08-08: aquel vigilaba que
    /// un módulo migrado no siguiera anunciándose en la aplicación vieja, y sin
    /// aplicación vieja no hay colisión posible. Un guardián que no puede fallar
    /// se borra con lo que vigilaba (directiva 7).
    ///
    /// Lo que SÍ puede volver a pasar es lo de fondo: una entrada de menú que
    /// lleva fuera del panel. Antes eran los enlaces con flecha a Angular;
    /// mañana sería una ruta que alguien escribe sin el prefijo. Se mide sobre
    /// las DOS tablas del menú.
    #[test]
    fn ninguna_entrada_del_menu_sale_del_panel() {
        let fuera = fuera_del_panel(
            &PANTALLAS.iter().map(|p| p.ruta).collect::<Vec<_>>(),
            &SIN_MODULO.iter().map(|(r, _, _)| *r).collect::<Vec<_>>(),
        );
        assert!(fuera.is_empty(), "estas entradas del menú salen del panel: {fuera:?}");
        // CONTROL: las dos tablas tienen contenido. Con las dos vacías, lo de
        // arriba pasa sin comparar nada.
        assert!(!PANTALLAS.is_empty() && !SIN_MODULO.is_empty());
    }

    /// Su caso rojo, contra fuente sintética. Sin él, la de arriba sale verde
    /// con la regla buena y con una regla ciega: hoy todas las rutas reales
    /// empiezan por `/panel`, así que el conjunto vacío no distingue (directiva
    /// 33).
    #[test]
    fn el_guardian_ve_la_entrada_que_se_sale() {
        assert_eq!(fuera_del_panel(&["/panel/citas", "/historias"], &[]), vec!["/historias"]);
        assert_eq!(fuera_del_panel(&[], &["/panel/admin", "/admin"]), vec!["/admin"]);
        // La pareja: sin fugas no inventa una.
        assert!(fuera_del_panel(&["/panel/citas"], &["/panel/admin"]).is_empty());
        // Y `/paneles` no es `/panel`: la frontera es de SEGMENTO, no de prefijo.
        assert_eq!(fuera_del_panel(&["/paneles/x"], &[]), vec!["/paneles/x"]);
    }

    /// Las rutas que no cuelgan de `/panel`. Recibe listas y no las constantes,
    /// que es lo que la hace probable por la MISMA costura que usa el camino
    /// real.
    fn fuera_del_panel<'a>(con_modulo: &[&'a str], sin_modulo: &[&'a str]) -> Vec<&'a str> {
        con_modulo
            .iter()
            .chain(sin_modulo.iter())
            .copied()
            .filter(|r| *r != "/panel" && !r.starts_with("/panel/"))
            .collect()
    }
}
