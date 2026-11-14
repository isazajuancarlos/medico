// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Estadísticas e indicadores: los siete paneles del prestador.
//!
//! Port de `frontend/src/app/pages/estadisticas/estadisticas.component.ts` (712
//! líneas, siete pestañas y catorce gráficos de `chart.js`).
//!
//! # Los gráficos se dibujan en el SERVIDOR, y no viene ninguna librería
//!
//! `chart.js` es una dependencia de npm con su `node_modules` detrás, que es
//! justo lo que el cambio de stack quitó (directiva 26: el frontal de este
//! producto son htmx y código nuestro). Y no hacía falta: lo que estos paneles
//! enseñan son distribuciones —«citas por estado»— y series —«tendencia
//! diaria»—, y las dos se dibujan con rectángulos. Van en
//! [`crate::vista::piezas::barras`] y [`crate::vista::piezas::columnas`], en SVG
//! y con los números al lado.
//!
//! Lo que se pierde y conviene decirlo: los anillos y las tartas. Un anillo de
//! cinco porciones se lee peor que cinco barras con su porcentaje escrito, así
//! que tampoco es una pérdida que haya que compensar — pero es un cambio de
//! aspecto que quien usaba la pantalla va a notar.
//!
//! # Una URL por panel, donde había un `signal<Tab>`
//!
//! Siete direcciones en vez de siete estados de un componente. Cada panel se
//! puede enlazar, recargar y guardar con su periodo dentro.
//!
//! # El periodo lo resuelve la MISMA función que `/api/estadisticas`
//!
//! [`crate::rutas::RangoEstadistica`], con sus dos defectos —mes en curso para
//! los paneles de actividad, año en curso para los acumulados— y su rechazo de
//! una fecha ilegible. Copiar aquí esa resolución habría hecho que los mismos
//! filtros dieran periodos distintos según el frontal, que es la clase de
//! diferencia que nadie ve hasta que dos informes no cuadran.
//!
//! # Solo lectura, y solo sesión
//!
//! Igual que las ocho rutas JSON: mirar el resumen de la propia institución no
//! es una acción de gestión, y exigir una capacidad dejaría sin portada a los
//! roles que atienden. Lo que cada uno ve lo decide su ÁMBITO.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};
use medico_datos::estadisticas;

use crate::auth::Estado;
use crate::rutas::{Defecto, RangoEstadistica};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, Tono};

const RUTA: &str = "/panel/estadisticas";

/// Los siete paneles, con su ruta y su etiqueta. Una sola lista: el menú de
/// arriba y las rutas registradas salen de aquí, así que no pueden divergir.
const PANELES: &[(&str, &str)] = &[
    ("", "Resumen"),
    ("/citas", "Citas"),
    ("/pacientes", "Pacientes"),
    ("/diagnosticos", "Diagnósticos"),
    ("/produccion", "Producción"),
    ("/financiero", "Financiero"),
    ("/urgencias", "Urgencias"),
    ("/cobertura", "Cobertura"),
];

/// Pesos colombianos, con separador de miles y sin decimales.
///
/// **Sin decimales a propósito**: son cifras de facturación de un periodo, y los
/// centavos de un total de cien millones son ruido que hace más difícil comparar
/// dos números de un vistazo. El separador es el punto, que es el de Colombia.
fn pesos(v: f64) -> String {
    let entero = v.round().abs() as i64;
    let digitos = entero.to_string();
    let mut salida = String::new();
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            salida.push('.');
        }
        salida.push(c);
    }
    if v < 0.0 {
        format!("-$ {salida}")
    } else {
        format!("$ {salida}")
    }
}

/// Un número con un decimal y coma, para las tasas.
fn tasa(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

// ── El armazón de la sección ─────────────────────────────────────────────────

#[derive(serde::Deserialize, Default)]
pub struct Rango {
    #[serde(default)]
    pub fecha_inicio: Option<String>,
    #[serde(default)]
    pub fecha_fin: Option<String>,
}

impl Rango {
    fn como_rango(&self) -> RangoEstadistica {
        RangoEstadistica {
            fecha_inicio: self.fecha_inicio.clone(),
            fecha_fin: self.fecha_fin.clone(),
            top: None,
        }
    }

    /// La cola de la URL que conserva el periodo al saltar de panel. Sin esto,
    /// elegir un rango y cambiar de pestaña lo perdería en silencio.
    fn cola(&self) -> String {
        match (&self.fecha_inicio, &self.fecha_fin) {
            (None, None) => String::new(),
            (i, f) => format!(
                "?fecha_inicio={}&fecha_fin={}",
                i.clone().unwrap_or_default(),
                f.clone().unwrap_or_default()
            ),
        }
    }
}

fn cabecera(activo: &str, r: &Rango, periodo: Option<&estadisticas::Periodo>) -> Markup {
    let cola = r.cola();
    html! {
        header class="titulo-seccion" {
            h1 { "Estadísticas e indicadores" }
            nav class="buscador" {
                @for (sufijo, etiqueta) in PANELES {
                    a class=[(*sufijo == activo).then_some("enlace-activo")]
                      href={ (RUTA) (sufijo) (cola) } { (etiqueta) }
                }
            }
        }
        section class="tarjeta" {
            form method="get" action={ (RUTA) (activo) } class="buscador" {
                label for="desde" class="sutil" { "Desde" }
                input #desde type="date" name="fecha_inicio"
                      value=(r.fecha_inicio.clone().unwrap_or_default());
                label for="hasta" class="sutil" { "Hasta" }
                input #hasta type="date" name="fecha_fin"
                      value=(r.fecha_fin.clone().unwrap_or_default());
                button type="submit" { "Aplicar" }
                a class="enlace-sutil" href={ (RUTA) (activo) } { "Quitar el filtro" }
            }
            @if let Some(p) = periodo {
                // El periodo EFECTIVO se dice siempre, también cuando no se
                // eligió ninguno: sin esto, «del 1 al 8» y «del año entero» se
                // ven igual —una cifra sin contexto— y es la diferencia entre
                // una cifra del mes y una del año.
                p class="sutil" { "Periodo: del " (p.inicio) " al " (p.fin) "." }
            }
        }
    }
}

/// Lo que se pinta cuando el periodo viene mal. **Es un 400 con forma de
/// pantalla**, no una tabla vacía: un filtro roto que enseña «no hay datos» es
/// la peor respuesta posible (directiva 20), y es exactamente lo que la ruta
/// JSON ya evita.
fn periodo_invalido(cx: &Contexto, activo: &str, r: &Rango, motivo: &str) -> Response {
    (
        axum::http::StatusCode::BAD_REQUEST,
        pagina(
            "Estadísticas",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                (cabecera(activo, r, None))
                section class="tarjeta" { (aviso(Tono::Error, motivo)) }
            },
        ),
    )
        .into_response()
}

/// Resuelve el periodo, o dice qué le pasa.
///
/// Devuelve el MOTIVO y no la `Response` ya armada, siguiendo el precedente que
/// este panel fijó en `SinPanel`: una `Response` entera en la variante `Err`
/// pesa 128 bytes que se copian en cada retorno del camino feliz
/// (`clippy::result_large_err`). La pantalla la arma el llamante.
fn periodo(r: &Rango, defecto: Defecto) -> Result<(String, String), String> {
    r.como_rango().resolver(defecto).map_err(|(_, motivo)| motivo)
}

fn pantalla(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA)), cuerpo).into_response()
}

/// El preámbulo que repiten los siete manejadores.
macro_rules! contexto_o_salir {
    ($e:expr, $u:expr) => {
        match contexto(&$e, &$u).await {
            Ok(c) => c,
            Err(r) => return r,
        }
    };
}

// ── Resumen ──────────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas`
pub async fn resumen(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = contexto_o_salir!(e, u);
    let r = match estadisticas::resumen(&e.pool, u.ambito()).await {
        Ok(r) => r,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Estadísticas",
        html! {
            (cabecera("", &Rango::default(), None))
            // El resumen NO acepta periodo: son las cifras de HOY y del mes en
            // curso, calculadas en el dominio. Ofrecer aquí el filtro y que no
            // hiciera nada sería peor que no ofrecerlo.
            p class="sutil" { "Al " (r.fecha) ". Estas cifras son de hoy y del mes en curso: no dependen del filtro." }

            section class="tarjeta resumen" {
                div { span class="cifra" { (r.total_pacientes) } span class="sutil" { "pacientes" } }
                div { span class="cifra" { (r.citas_hoy) } span class="sutil" { "citas hoy" } }
                div { span class="cifra" { (r.citas_mes) } span class="sutil" { "citas del mes" } }
                div { span class="cifra" { (r.consultas_mes) } span class="sutil" { "consultas del mes" } }
                div { span class="cifra" { (r.urgencias_activas) } span class="sutil" { "urgencias activas" } }
                div { span class="cifra" { (r.hospitalizados) } span class="sutil" { "hospitalizados" } }
            }

            section class="tarjeta" {
                h2 { "Camas" }
                (piezas::barras("Estado", &[
                    ("Ocupadas".to_string(), r.camas_ocupadas),
                    ("Disponibles".to_string(), r.camas_disponibles),
                ]))
                p class="sutil" {
                    "Ocupación: " (tasa(r.tasa_ocupacion)) " % de " (r.total_camas) " camas. "
                    // La tasa viene calculada del dominio; recalcularla aquí
                    // dividiendo dos campos daría el mismo número hoy y otro el
                    // día que cambie la definición (directiva 32).
                }
            }

            section class="tarjeta" {
                h2 { "Asistencia y cobro" }
                (piezas::datos(&[
                    ("Tasa de asistencia", Some(format!("{} %", tasa(r.tasa_asistencia)))),
                    ("Citas atendidas del mes", Some(r.citas_atendidas_mes.to_string())),
                    ("Facturas pendientes", Some(r.facturas_pendientes.to_string())),
                    ("Valor pendiente de cobro", Some(pesos(r.valor_pendiente))),
                ]))
            }
        },
    )
}

// ── Citas ────────────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas/citas`
pub async fn citas(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(rango): Query<Rango>,
) -> Response {
    let cx = contexto_o_salir!(e, u);
    let (i, f) = match periodo(&rango, Defecto::MesEnCurso) {
        Ok(v) => v,
        Err(motivo) => return periodo_invalido(&cx, "/citas", &rango, &motivo),
    };
    let d = match estadisticas::citas(&e.pool, u.ambito(), &i, &f).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Citas",
        html! {
            (cabecera("/citas", &rango, Some(&d.periodo)))
            section class="tarjeta resumen" {
                div { span class="cifra" { (d.total) } span class="sutil" { "citas en el periodo" } }
            }
            section class="tarjeta" {
                h2 { "Por estado" }
                (piezas::barras("Estado", d.by_estado.pares()))
            }
            section class="tarjeta" {
                h2 { "Por tipo" }
                (piezas::barras("Tipo", d.by_tipo.pares()))
            }
            section class="tarjeta" {
                h2 { "Tendencia" }
                (piezas::columnas(&d.tendencia.iter()
                    .map(|p| (p.fecha.clone(), p.cantidad))
                    .collect::<Vec<_>>()))
            }
            section class="tarjeta" {
                h2 { "Por médico" }
                (piezas::barras("Médico", &d.by_medico.iter()
                    .map(|m| (m.medico.clone(), m.cantidad))
                    .collect::<Vec<_>>()))
            }
        },
    )
}

// ── Pacientes ────────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas/pacientes`
pub async fn pacientes(State(e): State<Estado>, EnSesion(u): EnSesion) -> Response {
    let cx = contexto_o_salir!(e, u);
    let d = match estadisticas::pacientes(&e.pool, u.ambito()).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Pacientes",
        html! {
            (cabecera("/pacientes", &Rango::default(), None))
            // Igual que el resumen: es el padrón a día de hoy, no un periodo.
            p class="sutil" { "El padrón completo. No depende del filtro de fechas." }
            section class="tarjeta resumen" {
                div { span class="cifra" { (d.total) } span class="sutil" { "pacientes" } }
            }
            section class="tarjeta" { h2 { "Por sexo" } (piezas::barras("Sexo", d.by_sexo.pares())) }
            section class="tarjeta" {
                h2 { "Por grupo de edad" }
                (piezas::barras("Grupo", d.by_grupo_edad.pares()))
            }
            section class="tarjeta" {
                h2 { "Por régimen" }
                (piezas::barras("Régimen", d.by_tipo_usuario.pares()))
            }
            section class="tarjeta" { h2 { "Por zona" } (piezas::barras("Zona", d.by_zona.pares())) }
            section class="tarjeta" {
                h2 { "Por EPS" }
                (piezas::barras("EPS", &d.by_eps.iter()
                    .map(|x| (x.eps.clone(), x.cantidad))
                    .collect::<Vec<_>>()))
            }
        },
    )
}

// ── Diagnósticos ─────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas/diagnosticos`
pub async fn diagnosticos(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(rango): Query<Rango>,
) -> Response {
    let cx = contexto_o_salir!(e, u);
    let (i, f) = match periodo(&rango, Defecto::AnioEnCurso) {
        Ok(v) => v,
        Err(motivo) => return periodo_invalido(&cx, "/diagnosticos", &rango, &motivo),
    };
    let d = match estadisticas::diagnosticos(&e.pool, u.ambito(), &i, &f, 10).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Diagnósticos",
        html! {
            (cabecera("/diagnosticos", &rango, Some(&d.periodo)))
            section class="tarjeta" {
                h2 { "Los diez más frecuentes (CIE-10)" }
                // Se DICE que son diez, porque el recorte es del servidor y una
                // lista recortada en silencio se lee como la lista entera.
                p class="sutil" { "Los diez con más casos en el periodo, no todos." }
                @if d.diagnosticos.is_empty() {
                    p class="sutil" { "Sin diagnósticos registrados en este periodo." }
                } @else {
                    (piezas::tabla(
                        &["Código", "Descripción", "Casos", ""],
                        html! {
                            @let maximo = d.diagnosticos.iter().map(|x| x.cantidad).max().unwrap_or(0);
                            @for dx in &d.diagnosticos {
                                tr {
                                    td class="mono" { (dx.codigo) }
                                    td { (dx.descripcion) }
                                    td class="mono" { (dx.cantidad) }
                                    // Aquí la barra se compara con el MÁXIMO y no
                                    // con el total: es un top-10, así que los
                                    // porcentajes sobre la suma de diez mentirían
                                    // —no suman el total de casos del periodo—.
                                    td { (piezas::barras("", &[(String::new(), dx.cantidad), (String::new(), maximo - dx.cantidad)])) }
                                }
                            }
                        },
                    ))
                }
            }
        },
    )
}

// ── Producción ───────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas/produccion`
pub async fn produccion(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(rango): Query<Rango>,
) -> Response {
    let cx = contexto_o_salir!(e, u);
    let (i, f) = match periodo(&rango, Defecto::MesEnCurso) {
        Ok(v) => v,
        Err(motivo) => return periodo_invalido(&cx, "/produccion", &rango, &motivo),
    };
    let d = match estadisticas::produccion(&e.pool, u.ambito(), &i, &f).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Producción",
        html! {
            (cabecera("/produccion", &rango, Some(&d.periodo)))
            section class="tarjeta" {
                h2 { "Por profesional" }
                @if d.medicos.is_empty() {
                    p class="sutil" { "Sin producción registrada en este periodo." }
                } @else {
                    (piezas::tabla(
                        &["Profesional", "Especialidad", "Citas atendidas", "Historias"],
                        html! {
                            @for m in &d.medicos {
                                tr {
                                    td { (m.nombre) }
                                    td {
                                        // La especialidad NO se inventa: no hay
                                        // columna en este esquema, llega con el
                                        // maestro de talento humano.
                                        @match &m.especialidad {
                                            Some(s) => (s),
                                            None => span class="falta" { "sin registrar" },
                                        }
                                    }
                                    td class="mono" { (m.citas_atendidas) }
                                    td class="mono" { (m.historias_creadas) }
                                }
                            }
                        },
                    ))
                }
            }
            section class="tarjeta" {
                h2 { "Citas atendidas" }
                (piezas::barras("Profesional", &d.medicos.iter()
                    .map(|m| (m.nombre.clone(), m.citas_atendidas))
                    .collect::<Vec<_>>()))
            }
        },
    )
}

// ── Financiero ───────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas/financiero`
pub async fn financiero(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(rango): Query<Rango>,
) -> Response {
    let cx = contexto_o_salir!(e, u);
    let (i, f) = match periodo(&rango, Defecto::AnioEnCurso) {
        Ok(v) => v,
        Err(motivo) => return periodo_invalido(&cx, "/financiero", &rango, &motivo),
    };
    let d = match estadisticas::financiero(&e.pool, u.ambito(), &i, &f).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Financiero",
        html! {
            (cabecera("/financiero", &rango, Some(&d.periodo)))
            section class="tarjeta resumen" {
                div {
                    span class="cifra" { (pesos(d.valor_facturado)) }
                    span class="sutil" { "facturado en el periodo (emitido, sin borradores ni anuladas)" }
                }
                div {
                    span class="cifra" { (pesos(d.valor_glosado_pendiente)) }
                    span class="sutil" { "glosado pendiente" }
                }
            }
            section class="tarjeta" {
                h2 { "Facturas por estado" }
                @if d.by_estado.is_empty() {
                    p class="sutil" { "Sin facturas en este periodo." }
                } @else {
                    (piezas::tabla(
                        &["Estado", "Cantidad", "Valor"],
                        html! {
                            @for g in &d.by_estado {
                                tr {
                                    td { (piezas::estado(&g.estado)) }
                                    td class="mono" { (g.cantidad) }
                                    td class="mono" { (pesos(g.valor)) }
                                }
                            }
                        },
                    ))
                }
            }
            section class="tarjeta" {
                h2 { "Tendencia mensual" }
                // La serie es de VALOR, que es lo que enseñaba el gráfico de
                // líneas. Se redondea a pesos enteros para dibujarla; la cifra
                // exacta va en la tabla de al lado.
                (piezas::columnas(&d.tendencia_mensual.iter()
                    .map(|p| (p.periodo.clone(), p.valor.round() as i64))
                    .collect::<Vec<_>>()))
                @if !d.tendencia_mensual.is_empty() {
                    (piezas::tabla(
                        &["Mes", "Facturas", "Valor"],
                        html! {
                            @for p in &d.tendencia_mensual {
                                tr {
                                    td class="mono" { (p.periodo) }
                                    td class="mono" { (p.cantidad) }
                                    td class="mono" { (pesos(p.valor)) }
                                }
                            }
                        },
                    ))
                }
            }
            section class="tarjeta" {
                h2 { "Glosas por estado" }
                @if d.glosas.is_empty() {
                    p class="sutil" { "Sin glosas en este periodo." }
                } @else {
                    (piezas::tabla(
                        &["Estado", "Cantidad", "Valor"],
                        html! {
                            @for g in &d.glosas {
                                tr {
                                    td { (piezas::estado(&g.estado)) }
                                    td class="mono" { (g.cantidad) }
                                    td class="mono" { (pesos(g.valor)) }
                                }
                            }
                        },
                    ))
                }
            }
        },
    )
}

// ── Urgencias ────────────────────────────────────────────────────────────────

/// `GET /panel/estadisticas/urgencias`
pub async fn urgencias(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(rango): Query<Rango>,
) -> Response {
    let cx = contexto_o_salir!(e, u);
    let (i, f) = match periodo(&rango, Defecto::MesEnCurso) {
        Ok(v) => v,
        Err(motivo) => return periodo_invalido(&cx, "/urgencias", &rango, &motivo),
    };
    let d = match estadisticas::urgencias(&e.pool, u.ambito(), &i, &f).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Urgencias",
        html! {
            (cabecera("/urgencias", &rango, Some(&d.periodo)))
            section class="tarjeta resumen" {
                div { span class="cifra" { (d.total) } span class="sutil" { "atenciones" } }
                div {
                    span class="cifra" { (tasa(d.estancia_promedio_min)) }
                    span class="sutil" { "minutos de estancia media" }
                }
            }
            section class="tarjeta" {
                // **Estancia, no espera**, y el nombre lo dice: son dos
                // magnitudes distintas y este esquema no guarda la segunda. La
                // pantalla anterior la llamaba «tiempo de espera», que es un
                // indicador de calidad que aquí nadie mide.
                p class="sutil" {
                    "La estancia es lo que va del ingreso al egreso. NO es el tiempo de espera "
                    "hasta la atención: ese indicador necesita una marca que este sistema "
                    "todavía no registra, y por eso no se afirma."
                }
            }
            section class="tarjeta" {
                h2 { "Por triage" }
                // El orden lo pone el dominio y es la GRAVEDAD, de Rojo a Azul.
                // Reordenar aquí —alfabéticamente, por cantidad— convertiría una
                // escala clínica en una lista cualquiera.
                (piezas::barras("Triage", d.by_triage.pares()))
            }
            section class="tarjeta" {
                h2 { "Por destino al egreso" }
                (piezas::barras("Destino", d.by_destino.pares()))
            }
        },
    )
}

/// `GET /panel/estadisticas/cobertura` — el **Reporte de Cobertura** del sistema
/// anterior (`soportecobertura`, #752).
///
/// No era un módulo de captura sino un reporte, así que se porta como panel y no
/// como pantalla propia. Los seis tramos son los de salud pública —`<1`, `1-4`,
/// `5-14`, `15-44`, `45-59`, `60+`— y no los del panel de pacientes: el primer
/// año va solo porque la mortalidad y la cobertura del menor de un año se
/// reportan aparte.
pub async fn cobertura(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(r): Query<Rango>,
) -> Response {
    let cx = contexto_o_salir!(e, u);
    let (i, f) = match periodo(&r, Defecto::MesEnCurso) {
        Ok(v) => v,
        Err(motivo) => return periodo_invalido(&cx, "/cobertura", &r, &motivo),
    };
    let d = match estadisticas::cobertura(&e.pool, u.ambito(), &i, &f).await {
        Ok(d) => d,
        Err(_) => return averia(),
    };
    pantalla(
        &cx,
        "Cobertura",
        html! {
            (cabecera("/cobertura", &r, Some(&d.periodo)))
            p class="sutil" {
                "Pacientes DISTINTOS atendidos en el periodo, por grupo de edad. "
                "Quien vino tres veces cuenta una: eso es cobertura, no producción. "
                "La edad es la del día de la atención, no la de hoy, para que el "
                "mismo periodo dé siempre la misma cifra."
            }
            section class="tarjeta resumen" {
                div { span class="cifra" { (d.total) } span class="sutil" { "personas atendidas" } }
            }
            // LO QUE NO ENTRÓ SE DICE. Sin esto, «personas atendidas» se lee como
            // el total de atendidos, y no lo es: las filas con fecha ilegible o
            // con nacimiento posterior a la atención se caen. Un cero no se
            // dibuja —sería ruido en el caso normal—; cualquier otra cifra sí.
            @if d.descartadas > 0 {
                (aviso(Tono::Error, &format!(
                    "{} atención(es) quedaron fuera de este recuento: su fecha de nacimiento \
                     no se pudo leer, o es posterior a la atención. Búsquelas en el listado de \
                     pacientes y corríjalas — mientras tanto, esta cifra no es el total.",
                    d.descartadas
                )))
            }
            section class="tarjeta" {
                h2 { "Por grupo de edad" }
                (piezas::barras("Grupo", d.by_grupo_edad.pares()))
            }
        },
    )
}

/// Las rutas de la sección, construidas desde [`PANELES`] para que la lista del
/// menú y la del router no puedan separarse.
pub fn rutas() -> axum::Router<Estado> {
    use axum::routing::get;
    axum::Router::new()
        .route(RUTA, get(resumen))
        .route(&format!("{RUTA}/citas"), get(citas))
        .route(&format!("{RUTA}/pacientes"), get(pacientes))
        .route(&format!("{RUTA}/diagnosticos"), get(diagnosticos))
        .route(&format!("{RUTA}/produccion"), get(produccion))
        .route(&format!("{RUTA}/financiero"), get(financiero))
        .route(&format!("{RUTA}/urgencias"), get(urgencias))
        .route(&format!("{RUTA}/cobertura"), get(cobertura))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Los pesos se escriben con separador de miles**, que es lo que hace
    /// comparables dos cifras de un vistazo. Sin él, `12345678` y `1234567` se
    /// distinguen contando dígitos.
    #[test]
    fn los_pesos_se_agrupan_de_tres_en_tres() {
        assert_eq!(pesos(0.0), "$ 0");
        assert_eq!(pesos(999.0), "$ 999");
        assert_eq!(pesos(1000.0), "$ 1.000");
        assert_eq!(pesos(45_000.0), "$ 45.000");
        assert_eq!(pesos(1_234_567.0), "$ 1.234.567");
        assert_eq!(pesos(12_345_678.9), "$ 12.345.679", "se redondea, no se trunca");
        // Un negativo lleva el signo DELANTE del símbolo: «$ -100» se lee como
        // un importe con un guion suelto.
        assert_eq!(pesos(-1000.0), "-$ 1.000");
    }

    /// La cola de la URL conserva el periodo al saltar de panel; sin filtro no
    /// ensucia el enlace con dos parámetros vacíos.
    #[test]
    fn el_periodo_viaja_al_cambiar_de_panel_y_no_ensucia_la_url_sin_filtro() {
        assert_eq!(Rango::default().cola(), "");
        let r = Rango {
            fecha_inicio: Some("2026-01-01".into()),
            fecha_fin: Some("2026-03-31".into()),
        };
        assert_eq!(r.cola(), "?fecha_inicio=2026-01-01&fecha_fin=2026-03-31");
    }

    /// Los OCHO paneles de la lista son los ocho que registra el router: con
    /// dos listas, un panel nuevo saldría en el menú y daría 404.
    #[test]
    fn la_lista_del_menu_y_la_del_router_son_la_misma() {
        assert_eq!(PANELES.len(), 8);
        let rutas = format!("{:?}", rutas());
        for (sufijo, etiqueta) in PANELES {
            let esperada = format!("{RUTA}{sufijo}");
            assert!(
                rutas.contains(&esperada),
                "«{etiqueta}» está en el menú y no en el router: iría a 404"
            );
        }
    }
}
