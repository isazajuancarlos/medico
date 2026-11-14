// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! La pantalla del RIPS: **revisar el periodo y después descargarlo**.
//!
//! Port de `frontend/src/app/pages/rips/rips.component.ts`, que eran dos fechas
//! y un botón. Aquí son dos pasos, y esa es la única diferencia de fondo — el
//! resto es la misma llamada a la misma función.
//!
//! # Por qué dos pasos y no uno
//!
//! Porque la pantalla de Angular tenía un final que no servía: si al periodo le
//! faltaba un dato obligatorio, el servidor respondía 422 con la lista exacta de
//! qué falta, en qué archivo y en qué registro… y la pantalla enseñaba **una
//! línea de texto**, `err.error?.mensaje`, o sea «faltan 37 datos obligatorios».
//! Quien cierra el mes tiene entonces 37 huecos y ninguna pista de dónde están.
//!
//! Ese detalle ya existía y se tiraba a la basura. Enseñarlo es lo que convierte
//! la pantalla en lo que `backend/auditoria_rips.py` hace en el Python: mirar la
//! historia ANTES de generar y decir qué atenciones van completas, para corregir
//! doce consultas hoy en vez de descubrir en un mes que se reportaron mil.
//!
//! # Revisar NO exporta, y por eso NO deja eslabón
//!
//! La bitácora de la Ley 1581 anota la exportación **cuando el archivo llegó a
//! salir**: un 422 por datos faltantes no deja rastro, porque una bitácora que
//! registra lo que no ocurrió es peor que una que no registra. La revisión es
//! exactamente eso —una generación que no entrega nada— y además no enseña ni un
//! dato identificable: cuenta registros y nombra campos vacíos.
//!
//! Hay una segunda razón, práctica: se revisa una y otra vez mientras se corrige.
//! Un eslabón por pulsación ahogaría la cadena, que es el mismo freno por el que
//! los listados clínicos todavía no anotan.
//!
//! **La descarga sí anota**, y no puede no hacerlo: llama a [`crate::rips::exportar`],
//! que genera y anota en la misma función. La alternativa —dos llamadas y
//! acordarse de la segunda— es el olvido que la Ley 1581 no perdona.
//!
//! # Y no se reimplementa NADA del RIPS
//!
//! Las cinco consultas, el recolector compartido, el resumen de supuestos y la
//! lista de pacientes viven en [`crate::rips`], que es lo que sirve
//! `GET /api/rips`. Aquí solo se le da forma de pantalla.

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use chrono::NaiveDate;
use maud::{html, Markup};
use serde::Deserialize;

use crate::auth::{Estado, GenerarRips};
use crate::rips::{Fallo, Generacion, Periodo};
use crate::vista::{aviso, pagina, piezas, EnSesionCon, Tono};

/// Cuántas faltas se enseñan en la tabla. El resumen por campo va entero: es el
/// que dice **qué** hay que corregir, y son unas pocas etiquetas. El detalle es
/// el que crece con el mes, y mil filas en una pantalla no se leen — quien
/// corrige empieza por arriba y vuelve a revisar.
const FALTAS_QUE_SE_ENSEÑAN: usize = 100;

/// Lo que llega del formulario. Las fechas entran como TEXTO y se validan aquí.
///
/// Con `Query<Periodo>` —que ya sabe leer `NaiveDate`— una fecha vacía o rara la
/// rechazaría axum con su propio texto en crudo («Failed to deserialize query
/// string»), que en una pantalla no es un mensaje: es una avería aparente. El
/// `required` del navegador no cuenta como control (se rodea), así que la regla
/// vive aquí y dice qué pasa.
#[derive(Deserialize, Default)]
pub struct Enviado {
    #[serde(default)]
    inicio: String,
    #[serde(default)]
    fin: String,
    /// El consecutivo del RIPS sin factura. Obligatorio en esa ruta y **no se
    /// puede repetir entre envíos**: el manual del convertidor advierte que
    /// repetirlo genera rechazo. Solo lo usa la descarga del FEV-RIPS.
    #[serde(default)]
    num_nota: String,
    /// `"1"` = generar aunque falten datos, reportando lo supuesto. Es la puerta
    /// `permitir_supuestos` de `/api/rips`, y **la pide el llamante**: no hay
    /// forma de que se ponga sola.
    #[serde(default)]
    permitir_supuestos: String,
}

impl Enviado {
    /// El periodo, o la frase que explica qué falta. Ninguna fecha se inventa.
    fn periodo(&self) -> Result<Periodo, String> {
        let leer = |texto: &str, cual: &str| {
            let t = texto.trim();
            if t.is_empty() {
                return Err(format!("Falta la fecha {cual} del periodo."));
            }
            NaiveDate::parse_from_str(t, "%Y-%m-%d")
                .map_err(|_| format!("La fecha {cual} («{t}») no es una fecha."))
        };
        Ok(Periodo {
            inicio: leer(&self.inicio, "de inicio")?,
            fin: leer(&self.fin, "de fin")?,
            permitir_supuestos: self.permitir_supuestos == "1",
            // Se conserva en la API por compatibilidad y aquí no se ofrece: no
            // falta ningún documento desde el 2026-08-04, así que una casilla
            // para eso sería una bandera que no significa nada.
            parcial: false,
        })
    }
}

// ── La pantalla ──────────────────────────────────────────────────────────────

/// `GET /panel/rips`
///
/// Exige `GenerarRips`, la MISMA capacidad que `GET /api/rips`: una pantalla del
/// panel no puede abrir una puerta que su ruta JSON tiene cerrada.
pub async fn formulario(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GenerarRips>,
) -> Response {
    let cx = match crate::vista::contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    pagina(
        "RIPS",
        Some(crate::vista::panel::menu(&cx, "/panel/rips")),
        html! {
            header class="titulo-seccion" { h1 { "Generación de RIPS" } }
            section class="tarjeta" {
                p class="sutil" {
                    "Los cinco archivos de la Resolución 948 de 2026 —AC, AP, AM, AU y AH— "
                    "para el periodo que elija. Primero se revisa y después se descarga: "
                    "así los datos que falten se ven ANTES, con su registro y su campo, "
                    "en vez de aparecer como un rechazo del validador semanas después."
                }
                // `hx-get` y no `hx-post`: revisar no cambia nada ni deja rastro,
                // así que repetirlo no tiene consecuencias.
                form hx-get="/panel/rips/revisar"
                     hx-target="#resultado"
                     hx-swap="innerHTML"
                     class="apilado" {
                    label for="inicio" { "Fecha de inicio" }
                    input #inicio type="date" name="inicio" required;
                    label for="fin" { "Fecha de fin" }
                    input #fin type="date" name="fin" required;
                    button type="submit" class="principal" { "Revisar el periodo" }
                }
            }
            div #resultado {}
        },
    )
    .into_response()
}

/// `GET /panel/rips/revisar?inicio=&fin=` — el fragmento con lo que llevaría el
/// archivo, o con lo que le falta.
pub async fn revisar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GenerarRips>,
    Query(f): Query<Enviado>,
) -> Markup {
    let p = match f.periodo() {
        Ok(p) => p,
        Err(texto) => return aviso(Tono::Error, &texto),
    };
    // ESTRICTO siempre, sea lo que sea que traiga el formulario: la revisión
    // existe para ENSEÑAR lo que falta, y con supuestos no faltaría nada que
    // enseñar. Lo permisivo se decide después, viendo la lista.
    let estricto = Periodo {
        permitir_supuestos: false,
        ..p
    };
    // Lo ya enviado se enseña SIEMPRE, complete o no el periodo: si el mes
    // pasado se radicó y alguien vuelve a revisar, lo primero que necesita saber
    // es que ya hay un CUV — antes de volver a mandarlo y quemar otro consecutivo.
    let enviados = historial(&e, &u, &p).await;
    match crate::rips::generar_documentos(&e, &u, &estricto).await {
        Ok(g) => html! { (enviados) (completo(&g, &p)) },
        Err(Fallo::Incompleto(faltas)) => html! { (enviados) (incompleto(&faltas, &p)) },
        Err(Fallo::Averia) => aviso(
            Tono::Averia,
            "No se pudo leer la base de datos. No es un problema de las fechas \
             que eligió; vuelva a intentarlo en un momento.",
        ),
        Err(Fallo::Peticion { mensaje, .. }) | Err(Fallo::DatoCorrupto(mensaje)) => {
            aviso(Tono::Error, &mensaje)
        }
    }
}

/// Lo que ya se envió al MUV de este periodo. Vacío si nunca se envió, y
/// entonces no dibuja nada: una tarjeta que dice «no hay envíos» es ruido en la
/// pantalla de quien todavía está cuadrando el mes.
async fn historial(e: &Estado, u: &medico_datos::usuario::Usuario, p: &Periodo) -> Markup {
    // `usuario::ambito()` es la vía que ya existe, y falla cerrado ante un rol
    // corrupto: no se reconstruye a mano (directiva 1).
    let ambito = u.ambito();
    let envios = medico_datos::envio_muv::del_periodo(
        &e.pool,
        ambito,
        &p.inicio.to_string(),
        &p.fin.to_string(),
    )
    .await
    .unwrap_or_default();
    if envios.is_empty() {
        return html! {};
    }
    html! {
        section class="tarjeta" {
            h2 { "Ya enviado al validador" }
            table {
                thead {
                    tr { th { "Cuándo" } th { "Nota" } th { "Resultado" } th { "Ambiente" } }
                }
                tbody {
                    @for x in &envios {
                        tr {
                            // Se corta a la fecha: la hora exacta del envío no
                            // ayuda a decidir nada aquí, y ocuparía la columna.
                            td { (x.enviado.chars().take(10).collect::<String>()) }
                            td { code { (x.num_nota) } }
                            td {
                                @if let Some(cuv) = &x.cuv {
                                    strong { "CUV " } code { (cuv) }
                                } @else {
                                    "rechazado"
                                    @let n = cuantos_hallazgos(&x.hallazgos);
                                    @if n > 0 { " · " (n) " hallazgo(s)" }
                                }
                            }
                            // Un CUV de stage NO vale, así que el ambiente se
                            // dice: sin él, los dos se ven idénticos en la tabla.
                            td.tenue { (x.ambiente.clone().unwrap_or_else(|| "—".into())) }
                        }
                    }
                }
            }
        }
    }
}

/// Cuántos hallazgos trae el JSON guardado. Un JSON que no se pueda leer da 0 y
/// no rompe la pantalla: el envío ya ocurrió y su fila es historia.
fn cuantos_hallazgos(json: &str) -> usize {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .map(|v| v.len())
        .unwrap_or(0)
}

/// El periodo está completo: cuántos registros lleva cada archivo, y el botón.
fn completo(g: &Generacion, p: &Periodo) -> Markup {
    let total: usize = g.conteos.iter().map(|(_, n)| n).sum();
    html! {
        section class="tarjeta" {
            (aviso(Tono::Ok, "El periodo está completo: no falta ningún dato obligatorio."))
            (cabecera_periodo(p))
            div class="resumen" {
                @for (archivo, n) in &g.conteos {
                    div {
                        span class="cifra" { (n) }
                        span class="sutil" { (nombre_del_archivo(archivo)) }
                    }
                }
            }
            p class="sutil" { (total) " registros en total." }
            @if total == 0 {
                // CERO no es un archivo: es un periodo sin atenciones. Se dice,
                // porque descargar un RIPS vacío y subirlo al validador es
                // reportar que no se atendió a nadie.
                (aviso(
                    Tono::Error,
                    "No hay ninguna atención en ese periodo. Un archivo vacío reporta \
                     que no se atendió a nadie: revise las fechas antes de descargarlo.",
                ))
            }
            (descarga(p, false, "Descargar RIPS"))
            (descarga_fev(p, false))
        }
        (procedencia(&g.salida.procedencia))
    }
}

/// **La auditoría de procedencia (#84), DIBUJADA.** Qué campos van al archivo con
/// un valor que nadie afirmó.
///
/// Existía desde que la captura escribe procedencia y **no se dibujaba en ninguna
/// parte**: `Salida.procedencia` viajaba solo dentro del JSON de `GET /api/rips`
/// y del archivo descargado. Su propia documentación decía «se muestra a quien
/// firma», y quien firma es exactamente quien mira esta pantalla. Una alarma que
/// no se dibuja no existe (directiva 28); lo cazó la revisión de la directiva 25
/// con un control positivo — la palabra «procedencia» no aparecía ni una vez en
/// esta carpeta, mientras «supuestos» aparecía doce veces.
///
/// No bloquea y no debe: son datos presentes, el archivo se puede generar. Lo que
/// hace es que quien pulsa «Descargar» lo sepa antes.
///
/// Vacío no dibuja nada, como el historial de envíos: una tarjeta que dice «no
/// hay nada que reportar» es ruido en la pantalla de quien está cuadrando el mes.
///
/// **Lo que esto NO cubre todavía, y se dice en vez de dejarlo creído**: cuando al
/// periodo le FALTAN datos obligatorios, `generar_documentos` sale por
/// `Err(Fallo::Incompleto(faltas))` y esa rama solo lleva las faltas — la lista de
/// procedencia se descarta antes de llegar aquí. O sea que hoy esto se ve en el
/// periodo completo, que es el estado en el que se firma y se descarga, y no en el
/// que todavía se está corrigiendo.
fn procedencia(marcados: &[crate::rips::Faltante]) -> Markup {
    if marcados.is_empty() {
        return html! {};
    }
    // Por campo, igual que las faltas: quien captura corrige «el CUM del
    // medicamento» de un tirón, no un registro suelto.
    let mut por_campo: std::collections::BTreeMap<&str, (usize, &str)> =
        std::collections::BTreeMap::new();
    for f in marcados {
        let motivo = f.motivo.as_deref().unwrap_or("");
        let e = por_campo.entry(f.etiqueta).or_insert((0, motivo));
        e.0 += 1;
    }
    html! {
        section class="tarjeta" {
            h2 { "Datos que nadie afirmó" }
            p class="sutil" {
                "Estos campos VAN en el archivo y tienen valor, pero nadie confirmó de "
                "dónde salió. No impide descargar: es lo que conviene saber antes de "
                "firmar el cierre, porque un valor por defecto tiene la misma forma que "
                "uno real."
            }
            table {
                thead { tr { th { "Campo" } th { "Veces" } th { "Motivo" } } }
                tbody {
                    @for (etiqueta, (n, motivo)) in &por_campo {
                        tr {
                            td { (etiqueta) }
                            td { (n) }
                            td class="sutil" { (motivo) }
                        }
                    }
                }
            }
        }
    }
}

/// Falta algo: qué campos, cuántas veces, y en qué registros.
fn incompleto(faltas: &[crate::rips::Faltante], p: &Periodo) -> Markup {
    // Por campo, que es la unidad en la que se corrige: quien captura arregla
    // «la finalidad de la consulta» de un tirón, no un registro suelto.
    let mut por_campo: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for f in faltas {
        *por_campo.entry(f.etiqueta).or_insert(0) += 1;
    }
    html! {
        section class="tarjeta" {
            (aviso(
                Tono::Error,
                &format!(
                    "Faltan {} datos obligatorios. El archivo no se puede afirmar tal como está.",
                    faltas.len()
                ),
            ))
            (cabecera_periodo(p))

            h2 { "Qué falta" }
            (piezas::tabla(
                &["Dato", "Veces"],
                html! {
                    @for (etiqueta, n) in &por_campo {
                        tr {
                            td { (etiqueta) }
                            td class="mono" { (n) }
                        }
                    }
                },
            ))

            h2 { "Dónde" }
            (piezas::tabla(
                &["Archivo", "Registro", "Campo", "Motivo"],
                html! {
                    @for f in faltas.iter().take(FALTAS_QUE_SE_ENSEÑAN) {
                        tr {
                            td class="mono" { (f.archivo) }
                            td class="mono" { (f.registro) }
                            td class="mono" { (f.campo) }
                            td {
                                @match &f.motivo {
                                    Some(m) => (m),
                                    // No se deja la celda en blanco: un hueco
                                    // silencioso se lee como «aquí no pasa nada».
                                    None => span class="falta" { "sin dato" },
                                }
                            }
                        }
                    }
                },
            ))
            @if faltas.len() > FALTAS_QUE_SE_ENSEÑAN {
                // Se DICE lo que no se enseña. Una tabla recortada en silencio se
                // lee como la lista entera, y entonces corregir las cien de
                // arriba parecería haber terminado el trabajo.
                p class="sutil" {
                    "Se enseñan las primeras " (FALTAS_QUE_SE_ENSEÑAN) " de "
                    (faltas.len()) ". Corrija estas y vuelva a revisar."
                }
            }

            h2 { "Descargar de todos modos" }
            p class="sutil" {
                "Se puede generar el archivo rellenando lo que falta con supuestos. "
                "Lo supuesto viaja DENTRO del archivo, en el resumen, para que quien "
                "lo firma vea qué lleva inventado antes de firmarlo. No es lo mismo "
                "que un archivo completo, y ante una glosa la diferencia se nota."
            }
            (descarga(p, true, "Descargar con supuestos"))
            (descarga_fev(p, true))
        }
    }
}

/// El formulario de descarga. **POST y no un enlace**: la descarga exporta dato
/// personal y deja eslabón en la bitácora, así que no puede dispararla nada que
/// precargue enlaces —un antivirus, el propio navegador—, que es el mismo
/// razonamiento por el que `/panel/salir` no es un `GET`.
fn descarga(p: &Periodo, con_supuestos: bool, etiqueta: &str) -> Markup {
    html! {
        // Sin htmx a propósito: lo que vuelve es un archivo, y htmx no lo
        // descarga — reemplazaría un trozo de la pantalla con el JSON dentro.
        form method="post" action="/panel/rips/archivo" {
            input type="hidden" name="inicio" value=(p.inicio.to_string());
            input type="hidden" name="fin" value=(p.fin.to_string());
            @if con_supuestos {
                input type="hidden" name="permitir_supuestos" value="1";
            }
            @if con_supuestos {
                // Dos pulsaciones (`boi`): firmar un RIPS con datos supuestos
                // tiene efectos ante la EPS y la Supersalud, y la primera
                // pulsación no debería bastar. Sin JavaScript el botón funciona
                // a la primera: la barrera de verdad es la capacidad, no esto.
                button type="submit" class="principal"
                       data-confirmar="Pulse otra vez para generarlo con supuestos" {
                    (etiqueta)
                }
            } @else {
                button type="submit" class="principal" { (etiqueta) }
            }
        }
    }
}

/// La descarga del FEV-RIPS, que es el documento que se sube al MUV.
///
/// Va aparte de [`descarga`] y no como un botón más porque pide un dato que los
/// cinco documentos no piden: el **número de nota**, que es obligatorio, tiene
/// que ser consecutivo y **no se puede repetir entre envíos** —repetirlo genera
/// rechazo—. Meterlo escondido en el otro formulario sería regalar un rechazo
/// del validador a quien pulsara sin leer.
fn descarga_fev(p: &Periodo, con_supuestos: bool) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Documento para el validador (FEV-RIPS)" }
            p class="sutil" {
                "Es el archivo ÚNICO que se sube al Mecanismo Único de Validación: los mismos "
                "datos de arriba, pero con cada paciente una sola vez y todos sus servicios "
                "juntos. Sustituye al paso manual por el conversor web de MinSalud."
            }
            form method="post" action="/panel/rips/fev" class="apilado" {
                input type="hidden" name="inicio" value=(p.inicio.to_string());
                input type="hidden" name="fin" value=(p.fin.to_string());
                @if con_supuestos {
                    input type="hidden" name="permitir_supuestos" value="1";
                }
                label for="num_nota" { "Número de nota" }
                input #num_nota type="text" name="num_nota" required
                      placeholder="RS-2026-001";
                p class="sutil" {
                    "Obligatorio, consecutivo y distinto en cada envío: repetirlo produce "
                    "rechazo en el validador."
                }
                button type="submit" class="principal" { "Descargar el FEV-RIPS" }
            }
            p class="sutil" {
                "Esta es la ruta de RIPS SIN FACTURA, que es la del profesional independiente "
                "y la de quien no factura electrónicamente. La FEV en salud necesita además el "
                "XML de la factura ya aprobado por la DIAN, y ese archivo todavía no vive en "
                "este sistema."
            }

            // El envío va en su PROPIO formulario y no como un segundo botón del
            // de arriba: descargar es reversible —el archivo se borra y ya—, y
            // enviar no. Un archivo aceptado obtiene un CUV con efectos ante la
            // EPS y la Supersalud, así que la separación visual es parte de la
            // salvaguarda, no decoración.
            form method="post" action="/panel/rips/muv" class="apilado" {
                input type="hidden" name="inicio" value=(p.inicio.to_string());
                input type="hidden" name="fin" value=(p.fin.to_string());
                @if con_supuestos {
                    input type="hidden" name="permitir_supuestos" value="1";
                }
                label for="num_nota_envio" { "Número de nota" }
                input #num_nota_envio type="text" name="num_nota" required
                      placeholder="RS-2026-001";
                button type="submit"
                       data-confirmar="Pulse otra vez para ENVIARLO al validador" {
                    "Enviar al validador y obtener el CUV"
                }
                p class="sutil" {
                    "Sube el archivo al contenedor del prestador y devuelve el CUV, o la lista "
                    "exacta de campos que hay que corregir. Es un envío REAL y no se deshace: "
                    "por eso pide dos pulsaciones. Si este servidor no tiene configurado el "
                    "validador, lo dirá sin haber leído la historia clínica del periodo."
                }
            }
        }
    }
}

fn cabecera_periodo(p: &Periodo) -> Markup {
    html! {
        p class="sutil" { "Periodo del " (p.inicio) " al " (p.fin) "." }
    }
}

/// El nombre largo de cada archivo. «AC» no le dice nada a quien cierra el mes;
/// «consultas» sí, y es lo que le permite ver que el número no cuadra.
fn nombre_del_archivo(codigo: &str) -> &'static str {
    match codigo {
        "AC" => "consultas (AC)",
        "AP" => "procedimientos (AP)",
        "AM" => "medicamentos (AM)",
        "AU" => "urgencias (AU)",
        "AH" => "hospitalizaciones (AH)",
        // No se inventa un nombre para un archivo que no conocemos: se enseña su
        // código, que es un dato cierto (directiva 20).
        _ => "otro archivo",
    }
}

// ── La descarga ──────────────────────────────────────────────────────────────

/// `POST /panel/rips/archivo` — el archivo, como descarga.
///
/// Llama a [`crate::rips::exportar`], que genera **y anota la exportación** en la
/// bitácora de la Ley 1581 con los pacientes que cubrió. Es la misma función que
/// hay detrás de `GET /api/rips`.
pub async fn archivo(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GenerarRips>,
    Form(f): Form<Enviado>,
) -> Response {
    let p = match f.periodo() {
        Ok(p) => p,
        Err(texto) => return pantalla_de_fallo(&e, &u, StatusCode::BAD_REQUEST, Tono::Error, &texto).await,
    };
    let g = match crate::rips::exportar(&e, &u, &p).await {
        Ok(g) => g,
        Err(Fallo::Averia) => return crate::vista::averia(),
        Err(Fallo::Incompleto(faltas)) => {
            return pantalla_de_fallo(
                &e,
                &u,
                StatusCode::UNPROCESSABLE_ENTITY,
                Tono::Error,
                &format!(
                    "Faltan {} datos obligatorios, así que el archivo no se generó. \
                     Revise el periodo para ver cuáles.",
                    faltas.len()
                ),
            )
            .await
        }
        Err(Fallo::Peticion { codigo, mensaje }) => {
            return pantalla_de_fallo(&e, &u, codigo, Tono::Error, &mensaje).await
        }
        Err(Fallo::DatoCorrupto(mensaje)) => {
            return pantalla_de_fallo(
                &e,
                &u,
                StatusCode::UNPROCESSABLE_ENTITY,
                Tono::Error,
                &mensaje,
            )
            .await
        }
    };

    // `to_vec_pretty` y no compacto: es el mismo archivo que la pantalla de
    // Angular descargaba (`JSON.stringify(data, null, 2)`), y un RIPS se abre a
    // mano cuando el validador se queja de un campo.
    let cuerpo = match serde_json::to_vec_pretty(&g.salida) {
        Ok(v) => v,
        // Serializar lo que acabamos de construir no puede fallar por el dato:
        // si falla, es una avería nuestra y no algo que el usuario corrija.
        Err(_) => return crate::vista::averia(),
    };
    let nombre = format!("RIPS_{}_{}.json", p.inicio, p.fin);
    (
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nombre}\""),
            ),
            // Lleva datos de paciente: no se guarda en ninguna caché compartida.
            (
                header::CACHE_CONTROL,
                "no-store, private".to_string(),
            ),
        ],
        cuerpo,
    )
        .into_response()
}

/// La pantalla entera con un aviso: la descarga es una navegación normal, así
/// que lo que vuelve tiene que ser una página y no un fragmento suelto.
async fn pantalla_de_fallo(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    codigo: StatusCode,
    tono: Tono,
    texto: &str,
) -> Response {
    let menu = match crate::vista::contexto(e, u).await {
        Ok(cx) => Some(crate::vista::panel::menu(&cx, "/panel/rips")),
        // Sin menú antes que sin pantalla: el aviso es lo que hay que leer.
        Err(_) => None,
    };
    (
        codigo,
        pagina(
            "RIPS",
            menu,
            html! {
                header class="titulo-seccion" { h1 { "Generación de RIPS" } }
                section class="tarjeta" {
                    (aviso(tono, texto))
                    p { a href="/panel/rips" { "Volver a elegir el periodo" } }
                }
            },
        ),
    )
        .into_response()
}

/// `POST /panel/rips/fev` — el documento ÚNICO que se sube al MUV.
///
/// Es el hermano de [`archivo`] y comparte con él la puerta que importa:
/// `crate::rips::exportar`, que anota la salida en la bitácora con los pacientes
/// que cubrió. Dos descargas del mismo dato personal por dos rutas distintas, y
/// una sin eslabón, sería justo el rastro partido que este repositorio ya cerró
/// en imagenología.
///
/// Solo la ruta **sin factura**: la FEV en salud exige el AttachedDocument de la
/// factura electrónica ya aprobado por la DIAN, y ese archivo no está en este
/// sistema. Ofrecer un campo para pegarlo aquí sería una pantalla que promete lo
/// que no puede cumplir; cuando la factura electrónica entre, entra con él.
pub async fn fev(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GenerarRips>,
    Form(f): Form<Enviado>,
) -> Response {
    let p = match f.periodo() {
        Ok(p) => p,
        Err(texto) => {
            return pantalla_de_fallo(&e, &u, StatusCode::BAD_REQUEST, Tono::Error, &texto).await
        }
    };
    if f.num_nota.trim().is_empty() {
        return pantalla_de_fallo(
            &e,
            &u,
            StatusCode::BAD_REQUEST,
            Tono::Error,
            "Falta el número de nota. Es obligatorio en un RIPS sin factura, tiene que ser \
             consecutivo y no se puede repetir entre envíos: repetirlo produce rechazo en el \
             validador.",
        )
        .await;
    }
    let g = match crate::rips::exportar(&e, &u, &p).await {
        Ok(g) => g,
        Err(Fallo::Averia) => return crate::vista::averia(),
        Err(Fallo::Incompleto(faltas)) => {
            return pantalla_de_fallo(
                &e,
                &u,
                StatusCode::UNPROCESSABLE_ENTITY,
                Tono::Error,
                &format!(
                    "Faltan {} datos obligatorios, así que el archivo no se generó. \
                     Revise el periodo para ver cuáles.",
                    faltas.len()
                ),
            )
            .await
        }
        Err(Fallo::Peticion { codigo, mensaje }) => {
            return pantalla_de_fallo(&e, &u, codigo, Tono::Error, &mensaje).await
        }
        Err(Fallo::DatoCorrupto(mensaje)) => {
            return pantalla_de_fallo(&e, &u, StatusCode::UNPROCESSABLE_ENTITY, Tono::Error, &mensaje)
                .await
        }
    };
    let sobre = crate::rips::Sobre {
        periodo: p,
        num_nota: f.num_nota.trim().to_string(),
        num_factura: String::new(),
        xml_fev: String::new(),
    };
    let paquete = match g.fev(&sobre) {
        Ok(x) => x,
        Err(x) => {
            return pantalla_de_fallo(
                &e,
                &u,
                StatusCode::UNPROCESSABLE_ENTITY,
                Tono::Error,
                &x.mensaje(),
            )
            .await
        }
    };

    // Con BOM y con sangría de UN espacio: es lo que escribe el conversor
    // oficial de MinSalud, y un archivo real del prestador lo trae. No está
    // confirmado que el validador lo EXIJA; parecerse al que ya le funciona es
    // la apuesta segura.
    let cuerpo = medico_dominio::rips::fev::con_bom(&medico_dominio::rips::fev::archivo(&paquete));
    let nombre = format!("FEVRIPS_{}_{}.json", sobre.periodo.inicio, sobre.periodo.fin);
    (
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nombre}\""),
            ),
        ],
        cuerpo,
    )
        .into_response()
}

// ═════════════════════════════════════════════════════════════════════════════
// ENVIAR AL MUV
// ═════════════════════════════════════════════════════════════════════════════

/// `POST /panel/rips/muv` — genera el FEV-RIPS del periodo y lo **sube** al
/// Mecanismo Único de Validación del prestador, devolviendo el CUV o los
/// hallazgos que hay que corregir.
///
/// # Por qué esto es un POST con doble confirmación y no un enlace
///
/// No es una descarga: **sale del sistema hacia un tercero y tiene efectos**. En
/// el ambiente de producción del MUV un archivo aceptado obtiene un CUV
/// verdadero, con consecuencias jurídicas y financieras ante la EPS y la
/// Supersalud. Nada que precargue enlaces puede dispararlo, y el botón pide dos
/// pulsaciones — el mismo criterio que `permitir_supuestos`.
///
/// # La bitácora se anota DESPUÉS del envío, y solo si llegó a salir
///
/// Es la regla que este repositorio ya tiene escrita para la exportación: un
/// fallo de conexión no exportó nada, y una bitácora que registra lo que no
/// ocurrió es peor que una que no registra. Por eso aquí se llama a
/// `generar_documentos` —que NO anota— y el eslabón lo pone este manejador, con
/// la acción `enviar_muv`: decir «exportar» sería cierto a medias, porque el dato
/// no se descargó, se entregó a MinSalud.
pub async fn enviar_al_muv(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GenerarRips>,
    Form(f): Form<Enviado>,
) -> Response {
    let p = match f.periodo() {
        Ok(p) => p,
        Err(texto) => {
            return pantalla_de_fallo(&e, &u, StatusCode::BAD_REQUEST, Tono::Error, &texto).await
        }
    };
    if f.num_nota.trim().is_empty() {
        return pantalla_de_fallo(
            &e,
            &u,
            StatusCode::BAD_REQUEST,
            Tono::Error,
            "Falta el número de nota. Es obligatorio, tiene que ser consecutivo y no se puede \
             repetir entre envíos: repetirlo produce rechazo en el validador.",
        )
        .await;
    }

    // La configuración va ANTES de generar nada: si no hay a dónde enviar, no
    // tiene sentido leer un mes entero de historia clínica para tirarlo.
    let Some(cfg) = e.muv.clone() else {
        return pantalla_de_fallo(
            &e,
            &u,
            StatusCode::SERVICE_UNAVAILABLE,
            Tono::Error,
            "El envío al MUV no está configurado en este servidor. Hacen falta \
             MEDICO_MUV_URL, MEDICO_MUV_TIPO_DOCUMENTO, MEDICO_MUV_DOCUMENTO, \
             MEDICO_MUV_CLAVE y MEDICO_MUV_NIT en el entorno del proceso. Mientras tanto, \
             descargue el archivo y súbalo a mano.",
        )
        .await;
    };

    // El NIT de las credenciales tiene que ser el de esta institución. Con una
    // sola configuración por proceso, un despliegue multi-institucional podría
    // facturarle a la EPS con el NIT del vecino sin que nada fallara — y eso, una
    // vez radicado, no se deshace con un `git revert`.
    match nit_de_la_institucion(&e, &u).await {
        Some(nit) if nit.trim() == cfg.nit.trim() => {}
        Some(nit) => {
            return pantalla_de_fallo(
                &e, &u, StatusCode::CONFLICT, Tono::Error,
                &format!(
                    "Este servidor tiene credenciales del MUV del NIT {} y esta institución es \
                     el NIT {nit}. No se envía: radicar con el NIT equivocado no se deshace.",
                    cfg.nit
                ),
            ).await
        }
        None => {
            return pantalla_de_fallo(
                &e, &u, StatusCode::CONFLICT, Tono::Error,
                "No se pudo determinar el NIT de esta institución, así que no se envía \
                 (no se supone cuál es).",
            ).await
        }
    }

    // El consecutivo que YA obtuvo un CUV no se puede reutilizar jamás: el
    // validador lo rechazaría, y aquí se sabe sin gastar el viaje. La puerta es
    // solo sobre los ACEPTADOS — ver `envio_muv::nota_ya_aceptada`.
    if let Some(id_inst) = u.institucion_id {
        match medico_datos::envio_muv::nota_ya_aceptada(&e.pool, id_inst, f.num_nota.trim()).await {
            Ok(Some(cuando)) => {
                return pantalla_de_fallo(
                    &e, &u, StatusCode::CONFLICT, Tono::Error,
                    &format!(
                        "El número de nota «{}» ya obtuvo un CUV en el envío del {}. Un \
                         consecutivo aceptado no se puede repetir: use el siguiente.",
                        f.num_nota.trim(),
                        &cuando[..10.min(cuando.len())]
                    ),
                ).await
            }
            Ok(None) => {}
            // No se puede comprobar → no se envía. Dejar pasar «por si acaso»
            // gastaría un consecutivo en un rechazo seguro (directiva 20).
            Err(x) => {
                eprintln!("envio_muv: no se pudo comprobar el consecutivo: {x}");
                return crate::vista::averia();
            }
        }
    }

    let g = match crate::rips::generar_documentos(&e, &u, &p).await {
        Ok(g) => g,
        Err(Fallo::Averia) => return crate::vista::averia(),
        Err(Fallo::Incompleto(faltas)) => {
            return pantalla_de_fallo(
                &e, &u, StatusCode::UNPROCESSABLE_ENTITY, Tono::Error,
                &format!(
                    "Faltan {} datos obligatorios, así que no se envió nada. Revise el periodo \
                     para ver cuáles.",
                    faltas.len()
                ),
            ).await
        }
        Err(Fallo::Peticion { codigo, mensaje }) => {
            return pantalla_de_fallo(&e, &u, codigo, Tono::Error, &mensaje).await
        }
        Err(Fallo::DatoCorrupto(mensaje)) => {
            return pantalla_de_fallo(&e, &u, StatusCode::UNPROCESSABLE_ENTITY, Tono::Error, &mensaje)
                .await
        }
    };

    let (inicio, fin) = (p.inicio, p.fin);
    let (registros, pacientes) = g.para_bitacora();
    let sobre = crate::rips::Sobre {
        periodo: p,
        num_nota: f.num_nota.trim().to_string(),
        num_factura: String::new(),
        xml_fev: String::new(),
    };
    let paquete = match g.fev(&sobre) {
        Ok(x) => x,
        Err(x) => {
            return pantalla_de_fallo(&e, &u, StatusCode::UNPROCESSABLE_ENTITY, Tono::Error, &x.mensaje())
                .await
        }
    };

    let cliente = match crate::muv::entrar(&cfg).await {
        Ok(c) => c,
        Err(medico_muv::ErrorMuv::CredencialesInvalidas(_)) => {
            return pantalla_de_fallo(
                &e, &u, StatusCode::BAD_GATEWAY, Tono::Error,
                "SISPRO rechazó las credenciales del prestador. No es un problema del archivo: \
                 hay que revisar el registro en Mi Seguridad Social y la vinculación a la entidad.",
            ).await
        }
        Err(x) => {
            return pantalla_de_fallo(
                &e, &u, StatusCode::BAD_GATEWAY, Tono::Error,
                &format!(
                    "No se pudo contactar el validador, así que NO se envió nada. Puede \
                     reintentarlo: {x}"
                ),
            ).await
        }
    };

    let cuerpo = serde_json::to_value(&paquete).unwrap_or(serde_json::Value::Null);
    let resultado = match cliente.cargar(&cuerpo, cfg.destino).await {
        Ok(r) => r,
        Err(x) => {
            return pantalla_de_fallo(
                &e, &u, StatusCode::BAD_GATEWAY, Tono::Error,
                &format!("El envío no llegó a completarse, así que puede reintentarlo: {x}"),
            ).await
        }
    };

    // Aquí sí: el archivo SALIÓ hacia MinSalud, lo hayan aceptado o no.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "enviar_muv",
        "rips",
        Some(format!("{inicio} a {fin}")),
        registros,
        pacientes,
    )
    .await;

    // Y se GUARDA el comprobante. Es *best-effort*, igual que la bitácora: el
    // envío ya ocurrió y no se puede deshacer, así que un fallo al escribir no
    // puede tragarse el CUV que el usuario tiene delante. Lo que sí hace es
    // DECIRLO en la pantalla — si no, quien lo ve creería que quedó guardado y
    // cerraría la ventana (directiva 20).
    let guardado = guardar_envio(&e, &u, &inicio, &fin, f.num_nota.trim(), &cfg, &resultado, registros)
        .await;

    pantalla_del_resultado(&e, &u, &inicio, &fin, &resultado, guardado).await
}

/// Anota el envío en `envio_muv`. `false` = no se pudo, y la pantalla lo dice.
#[allow(clippy::too_many_arguments)]
async fn guardar_envio(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    inicio: &chrono::NaiveDate,
    fin: &chrono::NaiveDate,
    num_nota: &str,
    cfg: &crate::muv::Config,
    r: &medico_muv::ResultadoCarga,
    registros: usize,
) -> bool {
    let Some(institucion_id) = u.institucion_id else {
        return false;
    };
    // Los hallazgos se guardan tal como vinieron: es lo que permite volver a
    // mirar QUÉ se rechazó sin reenviar nada.
    let hallazgos = serde_json::to_string(&r.hallazgos).unwrap_or_else(|_| "[]".into());
    let n = medico_datos::envio_muv::Nuevo {
        institucion_id,
        usuario_id: u.id,
        periodo_inicio: &inicio.to_string(),
        periodo_fin: &fin.to_string(),
        num_nota,
        destino: match cfg.destino {
            medico_muv::Destino::SinFactura => "sin_factura",
            medico_muv::Destino::Fev => "fev",
        },
        aceptado: r.aceptado,
        cuv: r.cuv.as_deref(),
        proceso_id: r.proceso_id,
        fecha_radicacion: r.fecha_radicacion.as_deref(),
        ambiente: r.ambiente.as_deref(),
        hallazgos: &hallazgos,
        registros: registros as i64,
    };
    match medico_datos::envio_muv::registrar(&e.pool, n).await {
        Ok(_) => true,
        Err(x) => {
            eprintln!("envio_muv: el envío SALIÓ y no se pudo guardar el comprobante: {x}");
            false
        }
    }
}

/// El NIT de la institución de quien envía.
async fn nit_de_la_institucion(e: &Estado, u: &medico_datos::usuario::Usuario) -> Option<String> {
    let id = u.institucion_id?;
    // Por `listar` con el ámbito de la institución, que es la vía que ya respeta
    // el aislamiento; no hace falta un `por_id` nuevo (directiva 1).
    medico_datos::institucion::listar(
        &e.pool,
        medico_dominio::aislamiento::Ambito::Institucion(id),
    )
    .await
    .ok()?
    .into_iter()
    .next()
    .map(|i| i.nit)
}

/// Dibuja lo que dijo el validador: el CUV, o los campos que hay que corregir.
async fn pantalla_del_resultado(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    inicio: &chrono::NaiveDate,
    fin: &chrono::NaiveDate,
    r: &medico_muv::ResultadoCarga,
    guardado: bool,
) -> Response {
    let menu = match crate::vista::contexto(e, u).await {
        Ok(cx) => Some(crate::vista::panel::menu(&cx, "/panel/rips")),
        Err(respuesta) => return respuesta,
    };
    let campos = r.campos_a_corregir();
    let sueltos: Vec<_> = r
        .hallazgos
        .iter()
        .filter(|h| h.path_fuente.is_empty())
        .collect();

    pagina(
        "RIPS · envío al MUV",
        menu,
        html! {
            h1 { "Envío al MUV" }
            p.tenue { "Periodo del " (inicio.to_string()) " al " (fin.to_string()) "." }

            @if r.aceptado {
                @let cuv = r.cuv.clone().unwrap_or_default();
                (aviso(Tono::Ok, &format!("Aceptado. CUV: {cuv}")))
                div.tarjeta {
                    h2 { "Código Único de Validación" }
                    p.cuv { code { (cuv) } }
                    @if guardado {
                        p.tenue {
                            "Queda guardado con este periodo: es el comprobante de que se "
                            "radicó, y se puede volver a consultar desde la pantalla del RIPS."
                        }
                    } @else {
                        p.tenue {
                            strong { "NO se pudo guardar el comprobante en este servidor. " }
                            "El envío SÍ se hizo y el CUV de arriba es válido — cópielo antes "
                            "de cerrar esta página."
                        }
                    }
                    @if let Some(a) = &r.ambiente { p.tenue { "Ambiente: " (a) "." } }
                }
            } @else {
                (aviso(Tono::Error, "El validador NO aceptó el archivo: no hay CUV."))
            }

            @if !campos.is_empty() {
                div.tarjeta {
                    h2 { "Qué hay que corregir" }
                    p.tenue {
                        "Agrupado por el campo exacto que lo produjo, tal como lo señala el "
                        "validador."
                    }
                    @for (campo, lista) in &campos {
                        div.hallazgo {
                            h3 { code { (campo) } }
                            ul {
                                @for h in lista {
                                    li {
                                        strong { (h.codigo) } " · "
                                        @if h.es_notificacion() { span.tenue { "(aviso) " } }
                                        (h.descripcion)
                                        @if !h.observaciones.is_empty() { " — " (h.observaciones) }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            @if !sueltos.is_empty() {
                div.tarjeta {
                    h2 { "Otros hallazgos" }
                    p.tenue { "El validador no los ató a un campo concreto." }
                    ul {
                        @for h in &sueltos {
                            li {
                                strong { (h.codigo) } " · "
                                @if h.es_notificacion() { span.tenue { "(aviso) " } }
                                (h.descripcion)
                            }
                        }
                    }
                }
            }

            @if r.aceptado && !r.notificaciones().is_empty() {
                p.tenue {
                    "Aceptado con " (r.notificaciones().len()) " aviso(s). Conviene revisarlos: "
                    "desde el 1 de junio de 2026 varias reglas de este grupo pasaron a ser "
                    "causal de rechazo."
                }
            }

            p { a href="/panel/rips" { "Volver" } }
        },
    )
    .into_response()
}

#[cfg(test)]
mod pruebas_procedencia {
    use super::*;
    use crate::rips::Faltante;

    fn falta(etiqueta: &'static str, motivo: &str) -> Faltante {
        Faltante {
            archivo: "AM",
            registro: 1,
            campo: "codigo_cum",
            etiqueta,
            motivo: Some(motivo.to_string()),
        }
    }

    /// La tarjeta se dibuja con lo que hay, **y su pareja**: vacío no dibuja nada.
    ///
    /// Las dos mitades hacen falta. Solo con la primera pasaría un fragmento que
    /// pintara la tarjeta siempre —y una tarjeta que dice «no hay nada» en el
    /// cierre de todos los meses es la alarma que a la tercera nadie mira
    /// (directiva 23)—; solo con la segunda, uno que no pintara nunca, que es
    /// justo el estado del que venimos.
    #[test]
    fn la_auditoria_de_procedencia_se_dibuja_y_lo_vacio_no_pinta_nada() {
        let html = procedencia(&[
            falta("CUM del medicamento", "valor sin procedencia registrada (posible valor por defecto)"),
            falta("CUM del medicamento", "valor sin procedencia registrada (posible valor por defecto)"),
            falta("cantidad del medicamento", "valor sin procedencia registrada (posible valor por defecto)"),
        ])
        .into_string();

        assert!(html.contains("Datos que nadie afirmó"), "{html}");
        assert!(html.contains("CUM del medicamento"), "{html}");
        assert!(html.contains("cantidad del medicamento"), "{html}");
        // Agrupado por campo, con su cuenta: dos del CUM son una fila que dice 2.
        assert!(html.contains("<td>2</td>"), "las dos del CUM se agrupan: {html}");
        assert!(html.contains("posible valor por defecto"), "el motivo se enseña: {html}");

        assert_eq!(procedencia(&[]).into_string(), "", "vacío no dibuja tarjeta");
    }
}
