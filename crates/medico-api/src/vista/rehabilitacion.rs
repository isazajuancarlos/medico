// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Rehabilitación: los planes y cuántas sesiones llevan.
//!
//! Sale de `medico_datos::rehabilitacion::listar_planes`, la MISMA función que
//! sirve `GET /api/rehabilitacion/planes`.
//!
//! # El avance se muestra como «realizadas de autorizadas», no como porcentaje
//!
//! Son dos magnitudes distintas y el porcentaje las colapsa (directiva 32): «80
//! %» no dice si faltan dos sesiones de diez o veinte de cien, y lo que decide
//! si hay que pedir más autorización es el número que falta, no la proporción.
//! El porcentaje se pinta como barra —que es donde sí sirve, para ver de un
//! vistazo quién va cerca del tope— y los dos números van al lado.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};
use medico_datos::rehabilitacion::Plan;

use crate::auth::Estado;
use crate::vista::{averia, contexto, pagina, piezas, EnSesion};

/// `GET /panel/rehabilitacion`
pub async fn listar(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let planes = match medico_datos::rehabilitacion::listar_planes(&e.pool, u.ambito(), None).await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    let activos = planes.iter().filter(|p| p.activo).count();
    // Los que ya agotaron lo autorizado y siguen abiertos: es a quien hay que
    // pedirle autorización nueva antes de la siguiente sesión, y sin este número
    // se descubre cuando la EPS glosa.
    let agotados = planes
        .iter()
        .filter(|p| p.activo && p.sesiones_realizadas >= p.sesiones_autorizadas)
        .count();

    pagina(
        "Rehabilitación",
        Some(crate::vista::panel::menu(&cx, "/panel/rehabilitacion")),
        html! {
            header class="titulo-seccion" { h1 { "Rehabilitación" } }

            @if planes.is_empty() {
                (piezas::vacio(false, "No hay planes de rehabilitación.", "", "/panel/rehabilitacion"))
            } @else {
                section class="tarjeta resumen" {
                    div { span class="cifra" { (activos) } span class="sutil" { "planes activos" } }
                    div {
                        span class="cifra" { (planes.len() - activos) }
                        span class="sutil" { "cerrados" }
                    }
                    @if agotados > 0 {
                        div {
                            span class="cifra" { (agotados) }
                            span class="sutil" { "sin sesiones autorizadas" }
                        }
                    }
                }
                (piezas::tabla(
                    &["Paciente", "Documento", "Terapia", "Frecuencia", "Sesiones", "Terapeuta", "Estado"],
                    html! { @for p in &planes { (fila(p)) } },
                ))
            }
        },
    )
    .into_response()
}

/// Una fila del listado. Se extrae de [`listar`] para poder probar en aislamiento
/// que **no escribe un estilo en línea** (#613): la barra de avance va en SVG
/// (`piezas::barra`), no con `style="width"` —bajo la CSP actual un estilo en
/// línea no da error, simplemente no se aplica—.
fn fila(p: &Plan) -> Markup {
    html! {
        tr {
            td {
                a href={ "/panel/pacientes/" (p.paciente_id) } { (p.paciente_nombre) }
            }
            td class="mono" { (p.paciente_identificacion) }
            td { (p.tipo_terapia) }
            td { (p.frecuencia) }
            td {
                // Los DOS números, no el porcentaje: lo que decide si hay que pedir
                // más autorización es cuántas faltan, no la proporción.
                span class="mono" {
                    (p.sesiones_realizadas) " / " (p.sesiones_autorizadas)
                }
                @if p.sesiones_autorizadas > 0 {
                    // La barra en SVG (`piezas::barra`), NO con `style="width:X%"`:
                    // la CSP cerró `style-src 'unsafe-inline'` el 2026-08-08, y un
                    // estilo en línea no daría error —se dibujaría SIN ancho—.
                    // Capada a las autorizadas: realizar de más no pasa del tope.
                    (piezas::barra(
                        p.sesiones_realizadas.min(p.sesiones_autorizadas),
                        p.sesiones_autorizadas,
                    ))
                }
            }
            td { (p.terapeuta_nombre) }
            td {
                @if p.activo { (piezas::estado("ACTIVA")) } @else { (piezas::estado("CERRADA")) }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn plan(realizadas: i64, autorizadas: i64) -> Plan {
        Plan {
            id: 1,
            paciente_id: 7,
            terapeuta_id: 3,
            fecha_creacion: "2026-01-01T10:00:00Z".into(),
            diagnostico_funcional: "lumbalgia".into(),
            objetivos: "andar".into(),
            sesiones_autorizadas: autorizadas,
            sesiones_realizadas: realizadas,
            tipo_terapia: "FISICA".into(),
            frecuencia: "diaria".into(),
            duracion_sesion_min: 30,
            activo: true,
            fecha_cierre: None,
            resultado_final: None,
            paciente_nombre: "Ana".into(),
            paciente_identificacion: "CC-1".into(),
            terapeuta_nombre: "Ter".into(),
        }
    }

    /// **La barra de avance NO escribe un estilo en línea (#613).**
    ///
    /// La CSP cerró `style-src 'unsafe-inline'` el 2026-08-08. Esta fila escribía
    /// `style="width:X%"`, que bajo esa CSP no se aplica —la barra se dibuja SIN
    /// ancho— y ningún test server-side lo veía: el 200 trae el `style=` igual. El
    /// guardián `ningun_panel_escribe_estilos_en_linea` solo miraba los paneles de
    /// estadísticas, que ya usan `piezas::barras` en SVG, así que un artefacto
    /// limpio no lo delataba (directiva 33). Se dibuja con un plan que SÍ tiene
    /// sesiones autorizadas, para que la barra aparezca.
    #[test]
    fn la_fila_no_escribe_estilo_en_linea() {
        let html = fila(&plan(3, 10)).into_string();
        assert!(
            !html.contains("style=\""),
            "la fila escribió un estilo en línea: bajo la CSP no se aplicaría.\n{html}"
        );
        // La pareja del negativo: la barra SÍ se dibujó. Sin esto, la línea de
        // arriba pasaría con una fila sin barra y no mediría nada.
        assert!(html.contains("<svg"), "no se dibujó la barra de avance:\n{html}");
    }

    /// La pareja del `@if`: sin sesiones autorizadas no hay barra —ni SVG— y
    /// tampoco se cuela un estilo en línea por otro lado.
    #[test]
    fn sin_sesiones_autorizadas_no_hay_barra_ni_estilo() {
        let html = fila(&plan(0, 0)).into_string();
        assert!(!html.contains("<svg"), "dibujó una barra sin sesiones autorizadas:\n{html}");
        assert!(!html.contains("style=\""), "escribió un estilo en línea:\n{html}");
    }
}
