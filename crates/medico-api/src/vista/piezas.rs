// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Las piezas que repiten las pantallas del panel: tabla, estado vacío, ficha de
//! datos, migas y etiqueta de estado.
//!
//! # Por qué existen ya, con tres pantallas hechas
//!
//! Porque quedan 27 y la directiva 9 dice que lo que se repita en la segunda
//! pantalla de un proyecto sale a pieza común. `pacientes` ya tenía tabla,
//! estado vacío y ficha; `citas` los pedía iguales. Extraerlos ahora hace barata
//! cada pantalla siguiente; extraerlos con quince hechas es reescribir quince.
//!
//! # Lo que NO es
//!
//! No es un framework de componentes ni el sustituto de `@angular/material`. Son
//! funciones que devuelven `Markup` y toman datos: sin estado, sin ciclo de
//! vida, sin registro. Si una necesita una opción más, se le añade un parámetro;
//! el día que necesite tres, probablemente eran dos piezas.
//!
//! # Y el vacío distingue «no hay» de «tu filtro no encontró»
//!
//! Es la única de estas piezas que tiene una regla dentro, y viene de un fallo
//! real: confundir las dos frases hace pensar que se perdieron los datos. Por
//! eso [`vacio`] pide las dos y no acepta una sola.

use maud::{html, Markup};

/// Una tabla con su cabecera, envuelta para que desborde EN HORIZONTAL dentro de
/// su caja y no arrastre la página.
///
/// Un padrón con documentos largos en el portátil de un consultorio desborda; si
/// el desbordamiento lo toma el `body`, la pantalla entera se mueve al tocar la
/// tabla y el menú se va de sitio.
pub fn tabla(cabeceras: &[&str], filas: Markup) -> Markup {
    html! {
        div class="tabla-envoltura" {
            table class="tabla" {
                thead {
                    tr {
                        @for c in cabeceras {
                            th { (c) }
                        }
                    }
                }
                tbody { (filas) }
            }
        }
    }
}

/// El estado vacío, con **las dos frases obligatorias**.
///
/// `sin_nada` es lo que se dice cuando de verdad no hay ninguno; `sin_coincidir`
/// cuando hay pero el filtro no encontró. Se piden las dos porque son cosas
/// distintas y confundirlas hace pensar que se perdieron los datos — pasó, y por
/// eso esta función no acepta un solo texto.
///
/// `volver` es la ruta para «ver todos»; solo se pinta cuando hay filtro.
pub fn vacio(hay_filtro: bool, sin_nada: &str, sin_coincidir: &str, volver: &str) -> Markup {
    html! {
        section class="tarjeta vacio" {
            @if hay_filtro {
                p { (sin_coincidir) }
                p { a href=(volver) { "Ver todos" } }
            } @else {
                p { (sin_nada) }
            }
        }
    }
}

/// Una ficha de pares etiqueta/valor. Los `None` **no se pintan**: una fila
/// «Celular: —» ocupa sitio para decir que no hay dato.
pub fn datos(pares: &[(&str, Option<String>)]) -> Markup {
    html! {
        dl class="datos" {
            @for (etiqueta, valor) in pares {
                @if let Some(v) = valor {
                    @if !v.trim().is_empty() {
                        dt { (etiqueta) }
                        dd { (v) }
                    }
                }
            }
        }
    }
}

/// La miga de vuelta al listado.
pub fn migas(a: &str, texto: &str) -> Markup {
    html! {
        nav class="migas" { a href=(a) { "← " (texto) } }
    }
}

/// Una etiqueta de estado, coloreada por familia.
///
/// El color sale del propio texto y no de un parámetro, para que el mismo estado
/// se vea igual en las seis pantallas que lo muestran. La familia se decide por
/// lo que el estado SIGNIFICA —terminado, en curso, cancelado— y lo que no
/// encaje sale neutro: inventarle un color a un estado nuevo lo pintaría de
/// «bien» o de «mal» sin que nadie lo haya decidido (directiva 20).
pub fn estado(texto: &str) -> Markup {
    let clase = match texto.to_uppercase().as_str() {
        "COMPLETADA" | "ATENDIDA" | "ACEPTADA" | "PAGADA" | "FIRMADA" | "VALIDADO" | "CERRADA" => {
            "pastilla pastilla-ok"
        }
        "CANCELADA" | "ANULADA" | "RECHAZADA" | "GLOSADA" | "SUSPENDIDA" | "FUGA" => {
            "pastilla pastilla-mal"
        }
        // `EN_CURSO` es el de quirófano, que se guarda con guion bajo. Va con
        // los demás «en marcha» y no aparte: el color dice en qué punto está
        // algo, y una cirugía en curso está en el mismo punto que una orden en
        // proceso.
        "PENDIENTE" | "EN PROCESO" | "EN_CURSO" | "BORRADOR" | "PROGRAMADA" | "ACTIVA"
        | "RADICADA" => {
            "pastilla pastilla-curso"
        }
        _ => "pastilla",
    };
    html! { span class=(clase) { (texto) } }
}

/// El recuento de un listado, diciendo si está filtrado.
pub fn recuento(mostrados: usize, total: usize, singular: &str, plural: &str) -> Markup {
    html! {
        p class="sutil" {
            (mostrados)
            " "
            @if mostrados == 1 { (singular) } @else { (plural) }
            @if mostrados != total { " de " (total) }
        }
    }
}

// ── Gráficos ─────────────────────────────────────────────────────────────────
//
// # Por qué se dibujan aquí y no con una librería
//
// La pantalla de estadísticas de Angular usaba `chart.js`. La directiva 26 deja
// el frontal de este producto en UNA dependencia —htmx—, así que traer una
// librería de gráficos con su `node_modules` detrás es exactamente lo que el
// cambio de stack quitó. Y no hacía falta: lo que esos paneles enseñan son
// distribuciones y series, que se dibujan con rectángulos.
//
// # Y se dibujan en SVG, NO con `style="width:X%"`
//
// La CSP de este binario llevaba `style-src 'self' 'unsafe-inline'`, y ese
// `'unsafe-inline'` estaba ahí **por Angular**: su comentario lo decía, y el día
// que Angular saliera había que poder cerrarlo. **Se cerró el 2026-08-08**, y
// esta decisión es la que lo permitió: una barra con estilo en línea habría sido
// una razón nueva para dejarlo abierto, escrita por el frontal que vino a
// sustituirlo. En SVG el ancho es un ATRIBUTO de geometría y no CSS, así que
// pasa bajo cualquier `style-src` — y hoy un estilo en línea ya no se vería
// «raro»: no se aplicaría.
//
// # Los números van SIEMPRE al lado
//
// Un dibujo no se lee con un lector de pantalla ni se copia a un informe. La
// barra acompaña a la cifra; no la sustituye — la misma regla que sigue
// `rehabilitacion`, que desde #613 también dibuja su avance con esta `barra`.

/// Una distribución: etiqueta, cantidad, porcentaje y su barra.
///
/// El orden de entrada se RESPETA, y en varios paneles ese orden es información
/// —el triage va de Rojo a Azul, que es la gravedad—. Por eso toma una lista y
/// no un mapa.
pub fn barras(cabecera: &str, datos: &[(String, i64)]) -> Markup {
    let total: i64 = datos.iter().map(|(_, n)| n).sum();
    html! {
        @if datos.is_empty() {
            p class="sutil" { "Sin datos en este periodo." }
        } @else {
            (tabla(
                &[cabecera, "Cantidad", "%", ""],
                html! {
                    @for (etiqueta, n) in datos {
                        tr {
                            td { (etiqueta) }
                            td class="mono" { (n) }
                            td class="mono" { (porcentaje(*n, total)) " %" }
                            td { (barra(*n, total)) }
                        }
                    }
                },
            ))
            p class="sutil" { "Total: " (total) }
        }
    }
}

/// El porcentaje con un decimal. **Un total de cero da cero**, no un `NaN`: la
/// división entra en el HTML y `NaN %` se lee como un fallo del servidor.
fn porcentaje(n: i64, total: i64) -> String {
    if total == 0 {
        return "0,0".into();
    }
    format!("{:.1}", n as f64 * 100.0 / total as f64).replace('.', ",")
}

/// Una barra proporcional, en SVG. El ancho es un atributo, no CSS.
/// Una barra `n de total` dibujada en **SVG**, no con `style="width:X%"`: el
/// ancho es un atributo de geometría y pasa bajo `style-src 'self'` (ver la
/// cabecera del módulo). Es `pub` porque la reutiliza `rehabilitacion` para la
/// barra de sesiones, en vez de escribir su propio estilo en línea (directiva 9).
pub fn barra(n: i64, total: i64) -> Markup {
    let ancho = if total == 0 { 0.0 } else { n as f64 * 120.0 / total as f64 };
    html! {
        svg width="120" height="10" role="presentation" {
            rect x="0" y="0" width="120" height="10" rx="5" fill="var(--fondo-2)" {}
            @if ancho > 0.0 {
                rect x="0" y="0" width=(format!("{ancho:.2}")) height="10" rx="5"
                     fill="var(--acento)" {}
            }
        }
    }
}

/// Una serie en el tiempo, como columnas.
///
/// Es el único sitio donde la forma dice algo que la tabla no: una tendencia de
/// treinta días son treinta filas que nadie compara de un vistazo. Cada columna
/// lleva su `<title>`, que es lo que el navegador enseña al pasar por encima y
/// lo que lee un lector de pantalla.
///
/// **El máximo se escribe**, porque sin él una columna alta no significa nada:
/// dos gráficos con la misma forma pueden ser 3 y 3 000.
pub fn columnas(datos: &[(String, i64)]) -> Markup {
    let maximo = datos.iter().map(|(_, n)| *n).max().unwrap_or(0);
    let total: i64 = datos.iter().map(|(_, n)| n).sum();
    // Ancho fijo por columna: el SVG crece con los datos y su caja hace scroll
    // horizontal, en vez de apretar cincuenta columnas hasta que no se ven.
    let paso = 14.0_f64;
    let alto = 80.0_f64;
    let ancho = (datos.len() as f64 * paso).max(paso);
    html! {
        @if datos.is_empty() || maximo == 0 {
            p class="sutil" { "Sin datos en este periodo." }
        } @else {
            div class="tabla-envoltura" {
                svg width=(format!("{ancho:.0}")) height=(format!("{alto:.0}"))
                    role="img" aria-label="Tendencia del periodo" {
                    @for (i, (etiqueta, n)) in datos.iter().enumerate() {
                        @let h = (*n as f64 * (alto - 2.0) / maximo as f64).max(1.0);
                        rect x=(format!("{:.2}", i as f64 * paso))
                             y=(format!("{:.2}", alto - h))
                             width=(format!("{:.2}", paso - 3.0))
                             height=(format!("{h:.2}"))
                             fill="var(--acento)" {
                            title { (etiqueta) ": " (n) }
                        }
                    }
                }
            }
            p class="sutil" {
                "Máximo: " (maximo) " · total: " (total)
                @if let (Some(p), Some(u)) = (datos.first(), datos.last()) {
                    " · del " (p.0) " al " (u.0)
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **El vacío distingue las dos frases**, que es la única regla de este
    /// módulo y viene de un fallo real.
    #[test]
    fn el_vacio_no_dice_lo_mismo_con_filtro_que_sin_el() {
        let sin = vacio(false, "No hay pacientes.", "Nada coincide.", "/x").into_string();
        let con = vacio(true, "No hay pacientes.", "Nada coincide.", "/x").into_string();
        assert!(sin.contains("No hay pacientes."));
        assert!(!sin.contains("Nada coincide."), "sin filtro no puede hablar de coincidencias");
        assert!(con.contains("Nada coincide."));
        assert!(!con.contains("No hay pacientes."), "con filtro no puede decir que no hay ninguno");
        // Y «ver todos» solo tiene sentido cuando hay algo que quitar.
        assert!(con.contains("Ver todos"));
        assert!(!sin.contains("Ver todos"));
    }

    /// Un dato ausente NO se pinta. Una fila «Celular: —» ocupa sitio para decir
    /// que no hay dato, y en una ficha clínica el ruido esconde lo que sí está.
    #[test]
    fn la_ficha_se_salta_lo_que_no_hay() {
        let m = datos(&[
            ("Documento", Some("CC 123".into())),
            ("Celular", None),
            ("Correo", Some("   ".into())),
        ])
        .into_string();
        assert!(m.contains("Documento"));
        assert!(!m.contains("Celular"), "se pintó una etiqueta sin valor");
        assert!(!m.contains("Correo"), "un valor en blanco no es un valor");
    }

    /// **Lo que no se sabe clasificar sale NEUTRO**, no de un color cualquiera.
    /// Pintar de verde un estado nuevo diría que va bien sin que nadie lo haya
    /// decidido.
    #[test]
    fn un_estado_desconocido_no_se_pinta_de_bien_ni_de_mal() {
        let d = estado("ESTADO_QUE_NADIE_HA_VISTO").into_string();
        assert!(d.contains(r#"class="pastilla""#), "se le inventó una familia: {d}");
        // Y la pareja: los que SÍ se conocen se distinguen entre sí.
        assert!(estado("COMPLETADA").into_string().contains("pastilla-ok"));
        assert!(estado("CANCELADA").into_string().contains("pastilla-mal"));
        assert!(estado("PENDIENTE").into_string().contains("pastilla-curso"));
    }

    /// **Un total de cero no produce `NaN`.** La división entra en el HTML, y
    /// «NaN %» se lee como un fallo del servidor — que es peor que un cero.
    #[test]
    fn una_distribucion_vacia_no_divide_por_cero() {
        assert_eq!(porcentaje(0, 0), "0,0");
        let m = barras("Estado", &[("Pendiente".into(), 0), ("Atendida".into(), 0)]).into_string();
        assert!(!m.contains("NaN"), "salió un NaN a la pantalla: {m}");
        // Y la pareja: con datos, el porcentaje es el que es.
        assert_eq!(porcentaje(1, 4), "25,0");
        assert_eq!(porcentaje(1, 3), "33,3");
    }

    /// **La barra va en SVG y su ancho es un ATRIBUTO, no un `style`.**
    ///
    /// El `'unsafe-inline'` de `style-src` estuvo en la CSP por Angular, y el día
    /// que Angular salga hay que poder cerrarlo. Una barra con estilo en línea
    /// sería una razón nueva para dejarlo abierto, puesta por el frontal que
    /// vino a sustituirlo.
    #[test]
    fn los_graficos_no_escriben_estilos_en_linea() {
        let b = barras("Estado", &[("Atendida".into(), 3), ("Cancelada".into(), 1)]).into_string();
        assert!(!b.contains("style="), "la barra escribió un estilo en línea: {b}");
        assert!(b.contains("<svg"), "no se dibujó nada");
        // El control de que la barra es PROPORCIONAL y no decorativa: 3 de 4
        // ocupa el 75 % de los 120 px.
        assert!(b.contains(r#"width="90.00""#), "la barra no es proporcional: {b}");

        let c = columnas(&[("2026-08-01".into(), 2), ("2026-08-02".into(), 4)]).into_string();
        assert!(!c.contains("style="), "la columna escribió un estilo en línea: {c}");
        // Cada columna dice su dato: un dibujo sin cifras no se lee ni se copia.
        assert!(c.contains("2026-08-02: 4"));
        assert!(c.contains("Máximo: 4"), "sin el máximo, la altura no significa nada");
    }

    /// Una serie vacía **no se dibuja como una serie plana**: se dice que no hay
    /// datos. Un gráfico en cero se lee como «todo fue cero», que es otra cosa.
    #[test]
    fn una_serie_sin_datos_lo_dice_en_vez_de_dibujar_una_linea_en_cero() {
        assert!(columnas(&[]).into_string().contains("Sin datos"));
        assert!(columnas(&[("2026-08-01".into(), 0)]).into_string().contains("Sin datos"));
        // Pareja: con un solo dato distinto de cero, sí se dibuja.
        let c = columnas(&[("2026-08-01".into(), 1)]).into_string();
        assert!(c.contains("<rect"), "un punto sí es una serie: {c}");
    }

    /// El recuento solo dice «de N» cuando de verdad hay filtro: «5 de 5» es
    /// ruido y hace dudar de si falta algo.
    #[test]
    fn el_recuento_solo_menciona_el_total_si_filtra() {
        assert!(!recuento(5, 5, "cita", "citas").into_string().contains(" de "));
        assert!(recuento(2, 9, "cita", "citas").into_string().contains("de 9"));
        // Y la concordancia, que se lee en cada pantalla.
        assert!(recuento(1, 1, "cita", "citas").into_string().contains("1 cita"));
        assert!(recuento(3, 3, "cita", "citas").into_string().contains("3 citas"));
    }
}
