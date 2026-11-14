// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Imágenes diagnósticas: la bandeja del servicio y el informe radiológico.
//!
//! Port de `frontend/src/app/pages/imagenes/imagenes.component.ts` (505 líneas,
//! dos vistas en un componente con un `signal<'lista'|'informe'>`). Aquí son dos
//! URL, que es lo que ese estado estaba imitando: `/panel/imagenes` y
//! `/panel/imagenes/{id}`. Con dos direcciones de verdad, el botón «atrás» del
//! navegador funciona, un informe se puede enlazar y recargar no pierde el sitio.
//!
//! # Es la PRIMERA pantalla del panel que escribe
//!
//! Las diecisiete anteriores leían. Aquí se guarda un informe, se firma, se
//! cambia el estado de la orden y se adjunta un archivo. Tres decisiones que van
//! con eso:
//!
//! 1. **POST normal y redirección (303), no htmx.** Escribir y volver a pintar la
//!    pantalla entera es lo correcto cuando el resultado cambia media pantalla
//!    —al firmar, el formulario se convierte en un documento de solo lectura—, y
//!    además funciona sin JavaScript. La recarga tras un POST no repite la
//!    escritura porque el 303 la convierte en un GET.
//! 2. **La protección contra CSRF ya existe y no se reinventa**: la cookie de
//!    sesión es `SameSite=Strict`, así que un formulario de otro sitio no la
//!    lleva y el POST llega sin sesión — a la entrada. No hay token que añadir.
//! 3. **Las acciones se dibujan solo para quien puede hacerlas.** Leer no exige
//!    capacidad —el médico que pidió el examen tiene que poder ver su resultado—
//!    pero escribir exige `EscribirImagenologia`, la misma que `/api/imagenes/…`.
//!    Pintarle «Guardar» a quien recibirá un 403 es la forma pequeña del mismo
//!    fallo que el menú tenía.
//!
//! # Lo que esta pantalla NO puede exigir, y por qué se dice
//!
//! Los tres campos obligatorios del informe —técnica, hallazgos e impresión
//! diagnóstica— **solo se exigen en el navegador**, con `required`, que es
//! exactamente lo que hacía Angular deshabilitando el botón. `crear_informe` y
//! `actualizar_informe` aceptan los tres vacíos, así que por `/api` se puede
//! guardar un informe que no dice nada. No se arregla aquí a propósito: una
//! regla que viviera solo en esta pantalla sería la segunda lista de #534 otra
//! vez, y la que hay que escribir está en el dominio — donde además hay que
//! decidir antes si un borrador a medias tiene que poder guardarse, que es lo
//! que el comentario de `actualizar_informe` deja abierto. Queda como tarea.
//!
//! # La bitácora: se anota lo MISMO que anota la ruta JSON, o sea nada
//!
//! `GET /api/imagenes/ordenes` y `/ordenes/{id}` no dejan eslabón hoy — es el
//! hueco que `CLAUDE.md` ya declara para los listados clínicos que atraviesan
//! pacientes—. Esta pantalla lo respeta en vez de mejorarlo por su cuenta: si el
//! panel anotara y la API no, el rastro de un mismo hecho quedaría partido según
//! qué frontal lo abriera, que es justo lo que la migración de `pacientes` evitó
//! usando los mismos argumentos. Se cierra en los dos sitios a la vez, con la
//! medición de volumen que esa tarea pide.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::diagnostico::{self, Escritura, Servicio};
use medico_dominio::rol::Capacidad;
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, Tono};

const SERVICIO: Servicio = Servicio::Imagenologia;
const RUTA: &str = "/panel/imagenes";

/// El mismo texto que responde `/api/imagenes/…` cuando el informe está cerrado.
const CERRADO: &str =
    "Este informe ya está firmado y no se modifica. Emita uno nuevo que lo corrija.";

// ── Los avisos, que viajan en la URL ─────────────────────────────────────────

/// Qué pasó en la escritura anterior. Viaja como `?aviso=` porque después de un
/// POST se redirige (303) y la pantalla siguiente es un GET limpio.
///
/// **Se traduce por una lista cerrada y lo que no esté no dice nada**: el valor
/// llega del cliente, así que aceptar texto libre sería dejar que cualquiera
/// eligiera lo que la pantalla afirma —«Informe firmado» sin haber firmado
/// nada—. Es la directiva 23: se enumera lo que SÍ significa algo y el resto va
/// a «no se puede saber», que aquí es callar.
fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "guardado" => Some((Tono::Ok, "Informe guardado.")),
        "firmado" => Some((
            Tono::Ok,
            "Informe firmado. La orden queda completada y el informe, de solo lectura.",
        )),
        "iniciado" => Some((Tono::Ok, "La orden pasa a «En Proceso».")),
        "adjuntado" => Some((Tono::Ok, "Archivo adjuntado al informe.")),
        "cerrado" => Some((Tono::Error, CERRADO)),
        "incompleto" => Some((
            Tono::Error,
            "No se firmó: al informe le falta alguno de los tres campos obligatorios. Debajo \
             se dice cuál.",
        )),
        "sin_informe" => Some((
            Tono::Error,
            "Redacte el informe antes de adjuntarle imágenes: una imagen suelta no dice qué \
             se vio en ella.",
        )),
        "archivo" => Some((
            Tono::Error,
            "No se pudo guardar el archivo: revise que sea una imagen o un PDF y que no \
             pase del tamaño permitido.",
        )),
        _ => None,
    }
}

/// A dónde se vuelve tras escribir, con lo que haya que decir.
fn de_vuelta(id: i64, aviso: &str) -> Response {
    Redirect::to(&format!("{RUTA}/{id}?aviso={aviso}")).into_response()
}

// ── La bandeja ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub estado: String,
}

/// `GET /panel/imagenes`
///
/// Solo sesión, igual que `GET /api/imagenes/ordenes`: leer no exige capacidad
/// en ninguno de los módulos de diagnóstico.
///
/// **Se enseña a todo el que tenga el módulo, y eso ensancha lo que hacía el
/// sidebar de Angular** —que la ocultaba salvo a radiólogo, admin, médico y
/// superadmin—. Es deliberado: aquella lista de roles vivía en el navegador y no
/// se correspondía con lo que el servidor permite, y un menú que esconde una
/// puerta abierta es la misma mentira de #534 mirada por el otro lado. Si esas
/// órdenes no deben verlas todos, la regla va en la API y entonces el menú la
/// sigue solo.
pub async fn listar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let estado = f.estado.trim();
    // Un estado que no es del producto NO se pasa a la consulta como si nada:
    // devolvería la lista vacía, que se lee como «no hay órdenes» (directiva 20).
    if !estado.is_empty() && !diagnostico::ESTADOS.contains(&estado) {
        return pantalla(
            &cx,
            "Imágenes diagnósticas",
            html! {
                (encabezado(""))
                section class="tarjeta" {
                    (aviso(
                        Tono::Error,
                        &format!(
                            "«{estado}» no es un estado de este producto. Los que hay son: {}.",
                            diagnostico::ESTADOS.join(", ")
                        ),
                    ))
                }
            },
        );
    }

    let ordenes = match diagnostico::listar_ordenes(
        &e.pool,
        u.ambito(),
        SERVICIO,
        (!estado.is_empty()).then_some(estado),
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return averia(),
    };

    // **El MISMO eslabón que `GET /api/imagenes/ordenes`**, con los mismos
    // argumentos: la bandeja atraviesa pacientes y enseña nombre y documento, así
    // que es `Coleccion` —sin ids—. Si el panel anotara distinto que la API, el
    // rastro de un mismo hecho quedaría partido según qué frontal lo abriera.
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_ORDENES_IMAGEN,
        (!estado.is_empty()).then(|| estado.to_string()),
        ordenes.len(),
        vec![],
    )
    .await;

    pantalla(
        &cx,
        "Imágenes diagnósticas",
        html! {
            (encabezado(estado))
            @if ordenes.is_empty() {
                (piezas::vacio(
                    !estado.is_empty(),
                    "No hay órdenes de imagenología.",
                    "Ninguna orden está en ese estado.",
                    RUTA,
                ))
            } @else {
                (piezas::recuento(ordenes.len(), ordenes.len(), "orden", "órdenes"))
                (piezas::tabla(
                    &["Fecha", "Paciente", "Documento", "Estudio", "Solicitante", "Estado", "Informe"],
                    html! {
                        @for o in &ordenes {
                            tr {
                                td class="mono" { (o.fecha_orden) }
                                td {
                                    // El enlace va en el nombre y lleva al
                                    // informe: es lo que hacía el botón de la
                                    // columna «Acción», y una fila entera que
                                    // se pulsa esconde a dónde lleva.
                                    a href={ (RUTA) "/" (o.id) } {
                                        (o.paciente_nombre.clone().unwrap_or_else(|| "—".into()))
                                    }
                                }
                                td class="mono" {
                                    (o.paciente_identificacion.clone().unwrap_or_default())
                                }
                                td {
                                    (o.descripcion)
                                    @if let Some(c) = &o.codigo_cups {
                                        span class="sutil" { " · CUPS " (c) }
                                    }
                                }
                                td { (o.medico_nombre.clone().unwrap_or_else(|| "—".into())) }
                                td { (piezas::estado(&o.estado)) }
                                td {
                                    @if o.tiene_resultado {
                                        (piezas::estado("EMITIDO"))
                                    } @else {
                                        span class="falta" { "pendiente" }
                                    }
                                }
                            }
                        }
                    },
                ))
            }
        },
    )
}

/// El filtro por estado, como enlaces y no como un `<select>`: cada estado tiene
/// su URL, así que se puede guardar y compartir, y el que está puesto se VE.
fn encabezado(activo: &str) -> Markup {
    html! {
        header class="titulo-seccion" {
            h1 { "Imágenes diagnósticas" }
            nav class="buscador" {
                a class=[activo.is_empty().then_some("enlace-activo")] href=(RUTA) { "Todas" }
                @for e in diagnostico::ESTADOS {
                    a class=[(activo == e).then_some("enlace-activo")]
                      href={ (RUTA) "?estado=" (e) } { (e) }
                }
            }
        }
    }
}

// ── El informe ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Visto {
    #[serde(default)]
    pub aviso: String,
}

/// `GET /panel/imagenes/{id}`
pub async fn ver(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(v): Query<Visto>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let orden = match diagnostico::obtener_orden(&e.pool, u.ambito(), SERVICIO, id).await {
        Ok(Some(o)) => o,
        // No existe, es de otra institución o es una orden de laboratorio: las
        // tres dan 404, y distinguirlas diría qué hay en la clínica de al lado.
        Ok(None) => return no_existe(&cx),
        Err(_) => return averia(),
    };
    let informe = match diagnostico::informe_de_orden(&e.pool, u.ambito(), id).await {
        Ok(i) => i,
        Err(_) => return averia(),
    };

    // Los MISMOS dos eslabones que dejarían las dos rutas JSON que esta pantalla
    // sustituye: abrir la ficha LEE la orden y su informe, así que anota las dos
    // cosas. Dos filas por apertura no es un problema medido: 232 bytes cada una
    // y 0,055 ms —comparadas 200 peticiones con eslabón contra 200 sin él, la
    // diferencia no se distingue del ruido—.
    match orden.paciente_id {
        Some(pid) => {
            crate::rutas::registrar_acceso(&e, &u, "ver", crate::rutas::RECURSO_ORDEN_IMAGEN, id, pid).await;
            if informe.is_some() {
                crate::rutas::registrar_acceso(&e, &u, "ver", crate::rutas::RECURSO_INFORME_IMAGEN, id, pid).await;
            }
        }
        // No se rellena con nada (directiva 20): la consulta lo saca de un JOIN
        // con `paciente`, así que faltar es una avería, no un caso.
        None => eprintln!(
            "⚠️  bitácora: la orden de imagenología {id} salió sin paciente_id; \
             no se anotó el acceso"
        ),
    }

    let puede_escribir = cx.puede(Capacidad::EscribirImagenologia);
    let firmado = informe.as_ref().is_some_and(|i| i.firmado);
    // Editable = puede escribir Y el informe no está cerrado. Las dos cosas por
    // separado: a quien no puede se le enseña el informe como documento, y a
    // quien puede pero el informe está firmado, también — por motivos distintos
    // que la pantalla dice cada uno con su frase.
    let editable = puede_escribir && !firmado;

    pantalla(
        &cx,
        "Informe radiológico",
        html! {
            (piezas::migas(RUTA, "Imágenes diagnósticas"))
            @if let Some((tono, texto)) = traducir_aviso(v.aviso.trim()) {
                (aviso(tono, texto))
            }

            header class="titulo-seccion" {
                h1 { "Informe radiológico" }
                (piezas::estado(&orden.estado))
            }

            section class="tarjeta" {
                (piezas::datos(&[
                    ("Paciente", orden.paciente_nombre.clone()),
                    ("Documento", orden.paciente_identificacion.clone()),
                    ("Estudio solicitado", Some(orden.descripcion.clone())),
                    ("CUPS", orden.codigo_cups.clone()),
                    ("Solicitante", orden.medico_nombre.clone()),
                    ("Fecha de la orden", Some(orden.fecha_orden.clone())),
                    ("Indicación clínica", Some(orden.justificacion.clone())),
                ]))
                @if orden.estado == "Pendiente" && puede_escribir {
                    form method="post" action={ (RUTA) "/" (orden.id) "/iniciar" } {
                        button type="submit" { "Iniciar el estudio" }
                    }
                }
            }

            @if editable {
                (formulario(&orden.id, informe.as_ref()))
            } @else {
                (documento(informe.as_ref(), firmado, puede_escribir))
            }

            (adjuntos(orden.id, informe.as_ref(), editable))
        },
    )
}

/// El formulario del informe. Los tres primeros campos llevan `required`, que es
/// lo mismo que hacía Angular deshabilitando el botón — y lo mismo de flojo: ver
/// la cabecera de este módulo.
fn formulario(orden_id: &i64, i: Option<&diagnostico::InformeImagen>) -> Markup {
    let v = |f: fn(&diagnostico::InformeImagen) -> String| i.map(f).unwrap_or_default();
    html! {
        section class="tarjeta" {
            h2 { "Informe" }
            form method="post" action={ (RUTA) "/" (orden_id) "/informe" } class="apilado" {
                label for="tecnica" { "Técnica utilizada" }
                input #tecnica type="text" name="tecnica" required
                      value=(v(|i| i.tecnica.clone()))
                      placeholder="Proyecciones PA y lateral de tórax con equipo digital…";

                label for="hallazgos" { "Hallazgos" }
                textarea #hallazgos name="hallazgos" rows="6" required
                    placeholder="Descripción detallada de lo observado en el estudio…" {
                    (v(|i| i.hallazgos.clone()))
                }

                label for="impresion" { "Impresión diagnóstica" }
                textarea #impresion name="impresion_diagnostica" rows="4" required
                    placeholder="Conclusión e impresión diagnóstica del estudio…" {
                    (v(|i| i.impresion_diagnostica.clone()))
                }

                label for="recomendaciones" { "Recomendaciones" }
                textarea #recomendaciones name="recomendaciones" rows="3"
                    placeholder="Estudios complementarios sugeridos, seguimiento…" {
                    (i.and_then(|i| i.recomendaciones.clone()).unwrap_or_default())
                }

                label for="clasificacion" { "Clasificación (BIRADS, ACR, Lung-RADS…)" }
                input #clasificacion type="text" name="clasificacion"
                      value=(i.and_then(|i| i.clasificacion.clone()).unwrap_or_default())
                      placeholder="BIRADS 3, ACR C, Lung-RADS 2…";

                button type="submit" class="principal" { "Guardar informe" }
            }
        }

        // Firmar va en su PROPIO formulario y no como un segundo botón del de
        // arriba: son dos acciones y una de ellas no se deshace. Con los dos en
        // el mismo formulario, un «Enter» en cualquier campo dispararía el
        // primero, y cuál es el primero depende del orden del HTML.
        @if let Some(inf) = i {
            // Qué le falta lo decide `faltan_para_firmar`, la MISMA función que
            // usa `firmar_informe`. No se repite aquí la lista de los tres
            // campos: con dos copias, una se queda atrás y la pantalla ofrece un
            // botón que lleva a un rechazo — enseñarle la regla al usuario
            // chocándose con ella es enseñársela dos veces.
            @let faltan = inf.faltan_para_firmar();
            section class="tarjeta" {
                h2 { "Firmar" }
                p class="sutil" {
                    "Al firmar, el informe queda de SOLO LECTURA y la orden pasa a "
                    "«Completada». Un informe firmado no se corrige: se emite otro. "
                    "Guarde antes los cambios que quiera dejar dentro."
                }
                @if faltan.is_empty() {
                    form method="post" action={ (RUTA) "/" (orden_id) "/firmar" } {
                        // Dos pulsaciones (`boi`). No se deshace, y lo que firma
                        // es un acto médico con efectos jurídicos (Ley 527/1999).
                        // Sin JavaScript el botón funciona a la primera: la
                        // barrera de verdad es la capacidad, esto es una red de más.
                        button type="submit" class="principal"
                               data-confirmar="Pulse otra vez para firmar y cerrar" {
                            "Firmar y completar"
                        }
                    }
                } @else {
                    // Se dice QUÉ falta, no «no se puede». Un botón que
                    // desaparece sin explicación se lee como un fallo, y el
                    // borrador se guarda igual: lo que se frena es la firma.
                    (aviso(Tono::Error, &format!(
                        "Todavía no se puede firmar: falta {}. El borrador se guarda igual \
                         —se escribe a ratos, entre las placas—; lo que no puede existir es \
                         un informe FIRMADO que no diga nada.",
                        faltan.join(", ")
                    )))
                }
            }
        }
    }
}

/// El informe como DOCUMENTO: lo que ve quien no puede escribir, y lo que queda
/// cuando ya está firmado.
///
/// Las dos razones se dicen aparte porque se corrigen distinto: una se arregla
/// pidiendo un permiso y la otra no se arregla — se emite otro informe.
fn documento(
    i: Option<&diagnostico::InformeImagen>,
    firmado: bool,
    puede_escribir: bool,
) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Informe" }
            @if firmado {
                (aviso(Tono::Ok, "Informe firmado: es de solo lectura. Para corregirlo se emite uno nuevo."))
            } @else if !puede_escribir {
                p class="sutil" {
                    "Su cuenta no tiene el permiso para redactar informes de imagenología. "
                    "Puede leerlo."
                }
            }
            @match i {
                Some(i) => {
                    (piezas::datos(&[
                        ("Técnica", Some(i.tecnica.clone())),
                        ("Hallazgos", Some(i.hallazgos.clone())),
                        ("Impresión diagnóstica", Some(i.impresion_diagnostica.clone())),
                        ("Recomendaciones", i.recomendaciones.clone()),
                        ("Clasificación", i.clasificacion.clone()),
                        ("Radiólogo", i.radiologo_nombre.clone()),
                        ("Fecha del resultado", Some(i.fecha_resultado.clone())),
                        ("Firmado el", i.fecha_firma.clone()),
                    ]))
                }
                // «Todavía no hay informe» y «el informe está vacío» son cosas
                // distintas: la primera es lo normal en una orden recién puesta.
                None => p class="sutil" { "Esta orden todavía no tiene informe." },
            }
        }
    }
}

/// Los archivos del informe. Cuelgan del informe y no de la orden, que es la
/// regla que ya ejerce `/api/imagenes/ordenes/{id}/informe/imagenes`: una imagen
/// suelta no dice qué se vio en ella.
fn adjuntos(
    orden_id: i64,
    i: Option<&diagnostico::InformeImagen>,
    editable: bool,
) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { "Archivos adjuntos" }
            @match i {
                Some(i) if !i.imagenes_paths.is_empty() => {
                    ul {
                        @for p in &i.imagenes_paths {
                            li {
                                a href=(p) {
                                    (p.rsplit('/').next().unwrap_or(p.as_str()))
                                }
                            }
                        }
                    }
                }
                Some(_) => p class="sutil" { "Sin archivos adjuntos." },
                None => p class="sutil" {
                    "Redacte el informe antes de adjuntarle imágenes: una imagen suelta no \
                     dice qué se vio en ella."
                },
            }
            @if editable && i.is_some() {
                form method="post" action={ (RUTA) "/" (orden_id) "/adjuntar" }
                     enctype="multipart/form-data" class="apilado" {
                    label for="archivo" { "Adjuntar una imagen o un PDF" }
                    // El nombre del campo es `archivo`, uno de los dos que
                    // `guardar_imagen_de` acepta. El tipo NO lo decide este
                    // `accept`: lo deciden los bytes, en el servidor.
                    // `.dcm`/DICOM en el selector: una placa es lo normal aquí. El
                    // tipo NO lo decide este `accept` —lo deciden los bytes, en el
                    // servidor—, pero sin él el selector del sistema oculta los DICOM.
                    input #archivo type="file" name="archivo"
                          accept="image/*,.pdf,.dcm,application/dicom" required;
                    button type="submit" { "Adjuntar" }
                }
            }
        }
    }
}

// ── Las escrituras ───────────────────────────────────────────────────────────

/// Lo que llega del formulario del informe. Es el mismo `InformeNuevo` que come
/// `/api/imagenes/…`, con los `Option` en los dos campos que de verdad lo son.
#[derive(Deserialize)]
pub struct InformeEnviado {
    #[serde(default)]
    tecnica: String,
    #[serde(default)]
    hallazgos: String,
    #[serde(default)]
    impresion_diagnostica: String,
    #[serde(default)]
    recomendaciones: String,
    #[serde(default)]
    clasificacion: String,
}

impl From<InformeEnviado> for diagnostico::InformeNuevo {
    fn from(f: InformeEnviado) -> Self {
        // Un campo opcional que llega en blanco es AUSENTE, no la cadena vacía:
        // el formulario manda siempre los cinco, así que sin esto un informe sin
        // clasificación guardaría `""` donde la API guarda `null` y las dos
        // pilas dejarían de poder compararse.
        let opcional = |s: String| (!s.trim().is_empty()).then_some(s);
        diagnostico::InformeNuevo {
            tecnica: f.tecnica,
            hallazgos: f.hallazgos,
            impresion_diagnostica: f.impresion_diagnostica,
            recomendaciones: opcional(f.recomendaciones),
            clasificacion: opcional(f.clasificacion),
        }
    }
}

/// Traduce el resultado de una escritura a la pantalla siguiente.
///
/// `Hecho` y `Cerrado` vuelven al informe —el segundo con su motivo— porque en
/// los dos casos hay algo que mirar. `NoExiste` es 404 y `EstadoInvalido` no
/// puede ocurrir por aquí: el estado lo pone una constante de este módulo, no el
/// cliente.
fn tras_escribir<T>(cx: &Contexto, id: i64, r: Escritura<T>, bien: &str) -> Response {
    match r {
        Escritura::Hecho(_) => de_vuelta(id, bien),
        Escritura::Cerrado => de_vuelta(id, "cerrado"),
        Escritura::NoExiste => no_existe(cx),
        Escritura::EstadoInvalido => averia(),
        // La pantalla ya deja de ofrecer el botón cuando falta algo, así que
        // llegar aquí es un formulario reconstruido a mano o una carrera con
        // otro que vació un campo. Se vuelve al informe, donde el aviso dice
        // exactamente qué falta.
        Escritura::Incompleto { .. } => de_vuelta(id, "incompleto"),
    }
}

/// `POST /panel/imagenes/{id}/informe` — crear o actualizar.
///
/// Llama a `crear_informe`, que actualiza si ya existe: la misma función que hay
/// detrás de `POST /api/imagenes/ordenes/{id}/informe`. Decidir aquí entre crear
/// y actualizar habría sido una segunda copia de esa decisión.
pub async fn guardar(
    State(e): State<Estado>,
    crate::vista::EnSesionCon(u, _): crate::vista::EnSesionCon<crate::auth::EscribirImagenologia>,
    Path(id): Path<i64>,
    Form(f): Form<InformeEnviado>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let nuevo: diagnostico::InformeNuevo = f.into();
    match diagnostico::crear_informe(&e.pool, u.ambito(), id, u.id, &nuevo).await {
        Ok(r) => tras_escribir(&cx, id, r, "guardado"),
        Err(_) => averia(),
    }
}

/// `POST /panel/imagenes/{id}/firmar`
pub async fn firmar(
    State(e): State<Estado>,
    crate::vista::EnSesionCon(u, _): crate::vista::EnSesionCon<crate::auth::EscribirImagenologia>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match diagnostico::firmar_informe(&e.pool, u.ambito(), id, u.id).await {
        Ok(r) => tras_escribir(&cx, id, r, "firmado"),
        Err(_) => averia(),
    }
}

/// `POST /panel/imagenes/{id}/iniciar` — la orden pasa a «En Proceso».
///
/// El estado va como CONSTANTE y no como un campo del formulario: es el único
/// que este botón significa, y aceptarlo del cliente convertiría un botón en un
/// editor de estados con el que se podría dar por «Completada» una orden sin
/// informe.
pub async fn iniciar(
    State(e): State<Estado>,
    crate::vista::EnSesionCon(u, _): crate::vista::EnSesionCon<crate::auth::EscribirImagenologia>,
    Path(id): Path<i64>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    match diagnostico::cambiar_estado_orden(&e.pool, u.ambito(), SERVICIO, id, "En Proceso").await {
        Ok(r) => tras_escribir(&cx, id, r, "iniciado"),
        Err(_) => averia(),
    }
}

/// `POST /panel/imagenes/{id}/adjuntar`
///
/// Los bytes entran por `rutas::guardar_imagen_de`, **el único camino** por el
/// que entran bytes al sistema: tipo deducido de la firma de los bytes, tope de
/// tamaño y cifrado en reposo. Un segundo almacén con sus propias reglas sería
/// el sitio por donde entrase el SVG que `tipos.rs` existe para no admitir.
///
/// Las tres puertas de antes de escribir en disco son las mismas que las de
/// `/api`: que la orden sea suya y de imagenología, y que su informe exista y no
/// esté firmado.
pub async fn adjuntar(
    State(e): State<Estado>,
    crate::vista::EnSesionCon(u, _): crate::vista::EnSesionCon<crate::auth::EscribirImagenologia>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(Some(orden)) = diagnostico::obtener_orden(&e.pool, u.ambito(), SERVICIO, id).await else {
        return no_existe(&cx);
    };
    match diagnostico::informe_de_orden(&e.pool, u.ambito(), id).await {
        Ok(Some(i)) if i.firmado => return de_vuelta(id, "cerrado"),
        Ok(Some(_)) => {}
        // Sin informe todavía no hay dónde colgar la imagen. Crearlo vacío aquí
        // dejaría un informe sin técnica ni hallazgos que parece redactado.
        Ok(None) => return de_vuelta(id, "sin_informe"),
        Err(_) => return averia(),
    }

    let id_imagen =
        match crate::rutas::guardar_imagen_de(
            &e,
            &u,
            orden.historia_id,
            multipart,
            "Imagen diagnóstica",
            // La placa llega DESPUÉS de la consulta: no la frena una historia
            // firmada (migración 0056).
            medico_datos::imagen::Origen::ResultadoDeImagenologia,
        )
        .await
        {
            Ok(v) => v,
            // El motivo exacto —tipo no admitido, demasiado grande, sin
            // archivo— lo decide `guardar_imagen_de` con su código; aquí se
            // traduce a una frase, porque un 415 en pantalla no es un mensaje.
            Err(_) => return de_vuelta(id, "archivo"),
        };
    match diagnostico::adjuntar_imagen(
        &e.pool,
        u.ambito(),
        id,
        &format!("/api/imagenes/{id_imagen}/archivo"),
    )
    .await
    {
        Ok(r) => tras_escribir(&cx, id, r, "adjuntado"),
        Err(_) => averia(),
    }
}

// ── Piezas de esta pantalla ──────────────────────────────────────────────────

fn pantalla(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA)), cuerpo).into_response()
}

/// El 404, dibujado **igual** para «no existe», «es de otra institución» y «es
/// una orden de laboratorio»: distinguirlos diría qué hay al otro lado.
fn no_existe(cx: &Contexto) -> Response {
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrado",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró esa orden" }
                    p { "Puede que el enlace esté mal o que ya no exista." }
                    p { a href=(RUTA) { "Volver a Imágenes diagnósticas" } }
                }
            },
        ),
    )
        .into_response()
}
