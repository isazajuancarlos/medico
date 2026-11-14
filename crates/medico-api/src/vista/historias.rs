// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! La historia clínica: la pantalla más grande del producto y la última del
//! port asistencial.
//!
//! Port de `frontend/src/app/pages/historias/historias.component.ts` (2 237
//! líneas, tres vistas en un `signal<'busqueda'|'lista'|'formulario'>` y un
//! `mat-stepper` de siete pasos).
//!
//! # El paso deja de ser un número en memoria y pasa a ser una URL
//!
//! `activeStep()` iba de 1 a 7 y no salía del navegador: recargar volvía al
//! principio, «atrás» se salía de la historia entera y no había forma de
//! mandarle a un colega el enlace del examen físico de una atención. Aquí cada
//! paso es una dirección —`/panel/historias/{id}/examen`— y las cuatro cosas
//! salen gratis.
//!
//! **Y cada paso GUARDA lo suyo**, en vez de un único botón al final. El
//! componente de Angular mantenía la historia entera en memoria y la mandaba
//! completa; una consulta se interrumpe —entra una urgencia, se cae la red— y
//! con el modelo anterior lo tecleado se perdía sin dejar rastro.
//!
//! # Guardar una sección no puede borrar las otras
//!
//! Y ese es el riesgo que trae este diseño, porque `PUT /api/historias/{id}`
//! REEMPLAZA las 61 columnas editables de una vez. La sección que se guarda
//! parte de lo que hay escrito ([`historia::como_nueva`]) y solo pisa sus
//! campos; lo demás viaja intacto. Que no falte una columna lo vigila
//! `como_nueva_lleva_todas_las_columnas_editables`, en `medico-datos`, porque el
//! fallo no da error: se nota cuando alguien vuelve a abrir la historia y el
//! examen físico está en blanco.
//!
//! # Los antecedentes ya no los define el navegador
//!
//! Las cuatro columnas `ant_*` son JSON sin esquema, y hasta hoy los 65 campos
//! que llevan dentro estaban declarados **solo** en el componente de Angular.
//! Con dos frontales eso es un formato con dos autores: basta que uno escriba
//! el bloque sin un campo para que ese campo desaparezca del expediente. Están
//! en [`medico_dominio::antecedentes`], y la paridad con Angular la mide
//! `antecedentes_paridad_con_angular.rs` mientras los dos convivan.
//!
//! # Lo que se conserva porque ya estaba bien
//!
//! - **Una historia firmada no se edita ni se borra**: los formularios no se
//!   dibujan y el servidor responde igual (`Resultado::Firmada` → 409 por
//!   `/api`, y aquí un aviso).
//! - **El CUPS de la consulta se valida contra el ámbito** con la misma función
//!   que la ruta JSON, y la **finalidad se anota contra ese CUPS** —no se
//!   filtra— porque la tabla del MUV cubre el 2,26 % de los procedimientos y
//!   filtrar haría indistinguible «vale» de «no sé».
//! - **Leer deja eslabón en la bitácora con los MISMOS argumentos** que
//!   `GET /api/historias/{id}`: `("ver", "historia", id, paciente_id)`. Migrar
//!   una pantalla sin anotar habría borrado rastro que hoy existe.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::{formula, historia, imagen, orden, paciente};
use medico_dominio::antecedentes::{self, Bloque, Tipo};
use medico_dominio::rol::Capacidad;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::auth::{Borrar, EscribirClinico, EscribirPaciente, Estado, Firmar};
use crate::vista::{aviso, averia, contexto, pagina, piezas, Contexto, EnSesion, EnSesionCon, Tono};

const RUTA: &str = "/panel/historias";

/// Las secciones de la ficha, en el orden en que se recorren. Es la misma
/// sucesión del `mat-stepper` de Angular, con una diferencia: la primera —la
/// consulta— era el encabezado del formulario y no un paso, así que no se podía
/// volver a ella sin salir del stepper.
const SECCIONES: &[(&str, &str)] = &[
    ("", "Consulta"),
    ("/antecedentes", "Antecedentes"),
    ("/examen", "Examen físico"),
    ("/diagnostico", "Diagnóstico"),
    ("/plan", "Plan"),
    ("/formulas", "Fórmula médica"),
    ("/ordenes", "Órdenes"),
    ("/imagenes", "Imágenes"),
];

/// Los quince aparatos del examen por sistemas, con su columna. Salen del
/// formulario de Angular y del esquema; no se inventa ninguno.
const SISTEMAS: &[(&str, &str)] = &[
    ("ef_cabeza", "Cabeza"),
    ("ef_ojos", "Ojos"),
    ("ef_nariz", "Nariz"),
    ("ef_oidos", "Oídos"),
    ("ef_boca", "Boca"),
    ("ef_faringe", "Faringe"),
    ("ef_orl", "Otorrinolaringológico"),
    ("ef_cuello", "Cuello"),
    ("ef_pulmones", "Pulmones"),
    ("ef_corazon", "Corazón"),
    ("ef_cardiopulmonar", "Cardiopulmonar"),
    ("ef_abdomen", "Abdomen"),
    ("ef_gu", "Genitourinario"),
    ("ef_neurologico", "Neurológico"),
    ("ef_extremidad", "Extremidades"),
    ("ef_osteomuscular", "Osteomuscular"),
    ("ef_piel", "Piel"),
];

/// Los tipos de orden del producto. **Son EXACTAMENTE los del `CHECK` de
/// `orden.tipo`** (migración `0027`), no una lista de pantalla: el vocabulario
/// se unificó ahí justo porque había tres escrituras distintas y dos pantallas
/// muertas comparando «Pendiente» contra «PENDIENTE».
///
/// Escribí «Procedimiento» de memoria en la primera versión y no está en el
/// `CHECK`: la orden se habría rechazado al insertar. Lo dice
/// `los_tipos_de_orden_son_los_del_esquema`, que los mide creando una de cada.
const TIPOS_ORDEN: &[&str] = &[
    "Laboratorio",
    "Imagenología",
    "Remisión Especialista",
    "Cirugía",
    "Terapia",
    "Otro",
];

// ── Avisos ───────────────────────────────────────────────────────────────────

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "creada" => Some((Tono::Ok, "Historia abierta.")),
        "guardada" => Some((Tono::Ok, "Cambios guardados.")),
        "antecedentes" => Some((Tono::Ok, "Antecedentes guardados. Son del paciente, así que valen para todas sus historias.")),
        "formula" => Some((Tono::Ok, "Medicamento añadido a la fórmula.")),
        "cantidad_invalida" => Some((
            Tono::Error,
            "La cantidad tiene que ser un número mayor que cero. No se guarda como \
             cero: una cantidad que nadie escribió viajaría al RIPS como si alguien \
             la hubiera puesto.",
        )),
        "dias_invalidos" => Some((
            Tono::Error,
            "Los días de tratamiento no pueden ser negativos. Déjelo vacío si no \
             aplica, o ponga 0 si es dosis única: el número viaja al RIPS como \
             `numDiasTratamiento` y se le factura a la EPS.",
        )),
        "formula_borrada" => Some((Tono::Ok, "Medicamento retirado de la fórmula.")),
        "orden" => Some((Tono::Ok, "Orden creada.")),
        "orden_borrada" => Some((Tono::Ok, "Orden eliminada.")),
        "imagen" => Some((Tono::Ok, "Archivo adjuntado.")),
        "imagen_borrada" => Some((Tono::Ok, "Archivo eliminado.")),
        "firmada" => Some((
            Tono::Ok,
            "Historia firmada. Queda cerrada: para corregir algo se emite otra.",
        )),
        "borrada" => Some((Tono::Ok, "Historia eliminada.")),
        "historia_firmada" => Some((
            Tono::Error,
            "Esta historia está firmada: no se le añade ni se le quita nada. Un documento \
             firmado no se edita por ninguna puerta — se corrige emitiendo otro.",
        )),
        "ya_dispensada" => Some((
            Tono::Error,
            "Ese medicamento ya se entregó al paciente y la prescripción no se retira: sin ella \
             quedaría una entrega que nada respalda. Si la entrega fue un error, se corrige en \
             farmacia.",
        )),
        "no_estaba" => Some((Tono::Error, "Eso ya no estaba: puede que alguien lo quitara antes.")),
        "orden_invalida" => Some((
            Tono::Error,
            "No se pudo crear la orden con ese tipo. Los tipos son los que ofrece el selector.",
        )),
        "imagen_rechazada" => Some((
            Tono::Error,
            "No se pudo adjuntar el archivo. Se admiten imágenes y PDF, y el tipo se decide por \
             los bytes: renombrar la extensión no cambia nada.",
        )),
        "no_se_firmo" => Some((
            Tono::Error,
            "No se firmó: puede que ya estuviera firmada.",
        )),
        "incompleta" => Some((
            Tono::Error,
            "No se firmó: a la historia le falta contenido clínico. Debajo, en «Cerrar la \
             atención», se dice cuál.",
        )),
        "cups_ajeno" => Some((
            Tono::Error,
            "Ese CUPS no se reporta en el archivo de CONSULTAS. No se guardó nada: un código \
             de otro archivo hace que el validador rechace el RIPS del mes entero.",
        )),
        "cerrada" => Some((
            Tono::Error,
            "Esta historia está firmada y no se edita. Sobre ella ya se decidió una conducta; \
             se corrige emitiendo otra.",
        )),
        _ => None,
    }
}

fn de_vuelta(a: &str, aviso: &str) -> Response {
    Redirect::to(&format!("{a}?aviso={aviso}")).into_response()
}

#[derive(Deserialize, Default)]
pub struct Filtro {
    #[serde(default)]
    pub aviso: String,
    #[serde(default)]
    pub q: String,
}

fn texto(c: &HashMap<String, String>, k: &str) -> Option<String> {
    c.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn entero(c: &HashMap<String, String>, k: &str) -> Option<i64> {
    texto(c, k).and_then(|s| s.parse().ok())
}

fn decimal(c: &HashMap<String, String>, k: &str) -> Option<f64> {
    texto(c, k).and_then(|s| s.replace(',', ".").parse().ok())
}

// ── Vista 1: a quién se le abre la historia ──────────────────────────────────

/// `GET /panel/historias`
///
/// El buscador de paciente. **No abre nada por sí solo**: una historia clínica
/// se abre sobre una persona, y entrar a la sección sin haber elegido a nadie no
/// tiene resultado que enseñar. Por eso aquí no hay listado del padrón entero —
/// eso es `/panel/pacientes`, y duplicarlo sería un segundo sitio donde mirar.
pub async fn buscar(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    // La búsqueda baja a SQL (2026-08-08): antes traía el padrón entero y
    // filtraba en memoria. `paciente::buscar` acota en la consulta, pliega
    // tildes y admite varias palabras. Sin aguja no se busca nada: esta pantalla
    // arranca vacía a propósito, porque abrir una historia es un acto dirigido.
    let aguja = f.q.trim();
    let encontrados = if aguja.is_empty() {
        Vec::new()
    } else {
        match paciente::buscar(
            &e.pool,
            u.ambito(),
            &paciente::Criterio { buscar: Some(aguja), limite: None },
        )
        .await
        {
            Ok(h) => h.filas,
            Err(_) => return averia(),
        }
    };

    dibujar(
        &cx,
        "Historias clínicas",
        html! {
            header class="titulo-seccion" {
                h1 { "Historias clínicas" }
                form method="get" action=(RUTA) class="buscador" {
                    label class="oculto" for="q" { "Buscar paciente" }
                    input #q type="search" name="q" value=(f.q)
                          placeholder="Nombre o documento del paciente";
                    button type="submit" { "Buscar" }
                }
            }
            @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }

            @if aguja.is_empty() {
                section class="tarjeta vacio" {
                    p { "Busque al paciente por nombre o documento para ver o abrir su historia." }
                    p class="sutil" {
                        "No se lista el padrón entero a propósito: una historia clínica se abre \
                         sobre una persona concreta, y ver la lista de quién es paciente de la \
                         institución es otra cosa — está en "
                        a href="/panel/pacientes" { "Pacientes" } "."
                    }
                }
            } @else if encontrados.is_empty() {
                section class="tarjeta vacio" {
                    p { "Ningún paciente coincide con «" (f.q) "»." }
                }
            } @else {
                section class="tarjeta" {
                    p class="sutil" {
                        (encontrados.len())
                        @if encontrados.len() == 1 { " paciente" } @else { " pacientes" }
                    }
                    (piezas::tabla(&["Documento", "Nombre", "Sexo", ""], html! {
                        @for p in &encontrados {
                            tr {
                                td class="mono" { (p.tipo_documento) " " (p.numero_documento) }
                                td {
                                    (p.nombre1) " " (p.nombre2.as_deref().unwrap_or(""))
                                    " " (p.apellido1) " " (p.apellido2.as_deref().unwrap_or(""))
                                }
                                td { (p.sexo) }
                                td {
                                    a href={ (RUTA) "/paciente/" (p.id) } { "Ver historias" }
                                }
                            }
                        }
                    }))
                }
            }
        },
    )
}

/// La cabecera con quién es el paciente. Se repite en todas las pantallas de la
/// historia porque es lo que evita el error más caro de un sistema clínico:
/// escribir en el expediente de otra persona.
fn cabecera_paciente(p: &paciente::InfoPaciente) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { (p.nombre_completo) }
            (piezas::datos(&[
                ("Documento", Some(format!("{} {}", p.tipo_id, p.identificacion))),
                ("Sexo", Some(p.sexo.clone())),
                ("Edad", Some(format!("{} años", p.edad))),
                ("Nacimiento", Some(p.fecha_nacimiento.clone())),
                ("EPS", p.entidad_nombre.clone()),
                ("Teléfono", p.celular.clone()),
            ]))
        }
    }
}

// ── Vista 2: las historias de un paciente ────────────────────────────────────

/// `GET /panel/historias/paciente/{id}`
pub async fn del_paciente(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(paciente_id): Path<i64>,
    Query(f): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(Some(p)) = paciente::info(&e.pool, u.ambito(), paciente_id).await else {
        return no_existe(&cx, "Ese paciente no es de esta institución, o no existe");
    };
    let hs = match historia::listar_por_paciente(&e.pool, u.ambito(), paciente_id).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    // El MISMO eslabón que `GET /api/pacientes/{id}/historias`: un conjunto de
    // registros clínicos de un titular identificado en la petición.
    crate::rutas::registrar_coleccion_de_un_titular(
        &e, &u, "listar", "historias", paciente_id, None, hs.len(),
    )
    .await;

    let puede_escribir = cx.puede(Capacidad::EscribirClinico);
    dibujar(
        &cx,
        "Historias clínicas",
        html! {
            (piezas::migas(RUTA, "Historias clínicas"))
            @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }
            (cabecera_paciente(&p))

            section class="tarjeta" {
                h2 { "Atenciones" }
                @if hs.is_empty() {
                    p class="sutil" { "Este paciente no tiene historias clínicas registradas." }
                } @else {
                    (piezas::tabla(&["Código", "Fecha", "Motivo", "Diagnóstico", "Estado", ""], html! {
                        @for h in &hs {
                            tr {
                                td class="mono" { (h.codigo_historia) }
                                td { (h.fecha) }
                                td { (h.motivo_consulta) }
                                td { (h.diagnostico_principal) }
                                td {
                                    @if h.firmada {
                                        (piezas::estado("FIRMADA"))
                                    } @else {
                                        (piezas::estado("BORRADOR"))
                                    }
                                }
                                td { a href={ (RUTA) "/" (h.id) } { "Abrir" } }
                            }
                        }
                    }))
                }
            }

            @if puede_escribir {
                section class="tarjeta" {
                    h2 { "Abrir una atención nueva" }
                    p class="sutil" {
                        "El motivo y la enfermedad actual son lo mínimo para abrirla; lo demás \
                         se completa por secciones y se guarda a medida que se escribe."
                    }
                    form method="post" action={ (RUTA) "/paciente/" (paciente_id) } class="apilado" {
                        label for="motivo" { "Motivo de consulta" }
                        input #motivo type="text" name="motivo_consulta" required;
                        label for="enfermedad" { "Enfermedad actual" }
                        textarea #enfermedad name="enfermedad_actual" rows="3" required {}
                        button type="submit" class="principal" { "Abrir historia" }
                    }
                }
            }
        },
    )
}

/// `POST /panel/historias/paciente/{id}`
///
/// La atribución (`medico_id`) es **quien abre la historia**, que en el panel es
/// siempre quien tiene la sesión. `crear` deja aparte `creado_por`, así que un
/// auxiliar que transcribe se distingue del médico al que se atribuye el acto —
/// pero elegir a otro médico desde aquí sería afirmar en su nombre, y eso no lo
/// ofrece esta pantalla.
pub async fn abrir(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(paciente_id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let nueva = historia::HistoriaNueva {
        codigo_historia: None,
        paciente_id,
        medico_id: u.id,
        cita_id: None,
        acomp_nombre: None,
        acomp_parentesco: None,
        acomp_telefono: None,
        acomp_documento: None,
        motivo_consulta: texto(&c, "motivo_consulta").unwrap_or_default(),
        enfermedad_actual: texto(&c, "enfermedad_actual").unwrap_or_default(),
        tension_arterial: None,
        pam: None,
        frecuencia_cardiaca: None,
        frecuencia_respiratoria: None,
        pulso: None,
        temperatura: None,
        pabd: None,
        pc: None,
        peso: None,
        talla: None,
        imc: None,
        peso_ideal: None,
        talla_ideal: None,
        estado_nutricional: None,
        ef_cabeza: None,
        ef_ojos: None,
        ef_nariz: None,
        ef_oidos: None,
        ef_boca: None,
        ef_faringe: None,
        ef_cuello: None,
        ef_pulmones: None,
        ef_corazon: None,
        ef_abdomen: None,
        ef_gu: None,
        ef_neurologico: None,
        ef_extremidad: None,
        ef_osteomuscular: None,
        ef_piel: None,
        ef_hallazgos: None,
        ef_orl: None,
        ef_cardiopulmonar: None,
        finalidad: None,
        causa_externa: None,
        tipo_diagnostico: None,
        cod_consulta_cups: None,
        modalidad_grupo_servicio: None,
        grupo_servicios: None,
        cod_servicio: None,
        via_ingreso_servicio: None,
        causa_motivo: None,
        valor_servicio: None,
        concepto_recaudo: None,
        valor_pago_moderador: None,
        num_fev_pago_moderador: None,
        datos_dinamicos: None,
        // El diagnóstico se elige en su sección. Abrir la historia con uno
        // inventado sería el `890201` otra vez: un dato con la forma correcta
        // que nadie afirmó.
        diagnostico_principal: String::new(),
        diagnostico_relacionado1: None,
        diagnostico_relacionado2: None,
        diagnostico_relacionado3: None,
        impresion_diagnostica: None,
        diagnostico_principal_cie11: None,
        diagnostico_principal_uri: None,
        procedencias: None,
        genera_incapacidad: None,
        dias_incapacidad: None,
        plan_tratamiento: String::new(),
    };
    match historia::crear(&e.pool, u.ambito(), u.id, &nueva).await {
        Ok(Some(id)) => de_vuelta(&format!("{RUTA}/{id}"), "creada"),
        // `None` = el paciente no es de esta institución, o no existe. Los dos
        // se cuentan igual, que es la regla de siempre: distinguirlos deja
        // enumerar el padrón del vecino probando números.
        Ok(None) => match contexto(&e, &u).await {
            Ok(cx) => no_existe(&cx, "Ese paciente no es de esta institución, o no existe"),
            Err(r) => r,
        },
        Err(_) => averia(),
    }
}

// ── Vista 3: la ficha, una URL por sección ───────────────────────────────────

/// Lo que toda sección necesita antes de dibujar: la historia, su paciente y si
/// se puede escribir en ella.
struct Ficha {
    h: historia::Historia,
    p: paciente::InfoPaciente,
    /// Se puede editar: ni firmada ni sin capacidad. Son dos condiciones y se
    /// cuentan distinto —una firmada no la edita nadie; sin capacidad, otro sí—.
    editable: bool,
}

/// Carga la ficha y **deja el mismo eslabón que `GET /api/historias/{id}`**.
///
/// El contenido del eslabón está congelado byte a byte, así que un verbo
/// distinto partiría el rastro de un mismo hecho en dos según qué pantalla lo
/// abriera. Se anota UNA vez por sección abierta, que es exactamente lo que
/// hace la ruta JSON cada vez que el componente pedía la historia.
async fn cargar(e: &Estado, u: &medico_datos::usuario::Usuario, cx: &Contexto, id: i64)
    -> Result<Ficha, Response>
{
    let h = match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => h,
        Ok(None) => return Err(no_existe(cx, "No se encontró esa historia clínica")),
        Err(_) => return Err(averia()),
    };
    let p = match paciente::info(&e.pool, u.ambito(), h.paciente_id).await {
        Ok(Some(p)) => p,
        // La historia existe y su paciente no es visible: no puede pasar —el
        // ámbito de la historia se deriva del paciente— y si pasa es avería,
        // no un «no encontrado» que mandaría a buscar un enlace roto.
        Ok(None) | Err(_) => return Err(averia()),
    };
    crate::rutas::registrar_acceso(e, u, "ver", "historia", id, h.paciente_id).await;
    let editable = !h.firmada && cx.puede(Capacidad::EscribirClinico);
    Ok(Ficha { h, p, editable })
}

/// La cabecera común: quién, qué atención, en qué estado, y las ocho secciones.
fn encabezado(f: &Ficha, activa: &str, aviso_clave: &str) -> Markup {
    let id = f.h.id;
    html! {
        (piezas::migas(&format!("{RUTA}/paciente/{}", f.h.paciente_id), "Historias del paciente"))
        @if let Some((tono, t)) = traducir_aviso(aviso_clave) { (aviso(tono, t)) }
        header class="titulo-seccion" {
            h1 { (f.p.nombre_completo) }
            p class="sutil" {
                (f.p.tipo_id) " " (f.p.identificacion) " · " (f.p.edad) " años · "
                (f.h.codigo_historia) " · " (f.h.fecha)
            }
        }
        @if f.h.firmada {
            (aviso(Tono::Ok, "Historia FIRMADA: queda cerrada y no se edita. Para corregir \
                              algo se emite otra."))
        }
        nav class="buscador" {
            @for (sufijo, etiqueta) in SECCIONES {
                @if *sufijo == activa {
                    a class="enlace-activo" href={ (RUTA) "/" (id) (sufijo) } { (etiqueta) }
                } @else {
                    a href={ (RUTA) "/" (id) (sufijo) } { (etiqueta) }
                }
            }
        }
        // El RDA (IHCE) de esta atención: preparar y avalar sus secciones (#603).
        p class="sutil" {
            a href={ "/panel/rda/consulta/" (id) } { "Preparar RDA (Resumen Digital de Atención)" }
        }
    }
}

fn dibujar(cx: &Contexto, titulo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(crate::vista::panel::menu(cx, RUTA)), cuerpo).into_response()
}

/// Guarda la sección `cambiar` sobre lo que HAY escrito.
///
/// El orden importa: se lee, se aplica lo de la sección y se escribe entero. Y
/// **la comprobación del CUPS va antes del `UPDATE`**, con la misma función que
/// usa la ruta JSON, para que un código inválido no entre por la edición — que
/// es el camino que más se usa, porque la historia se abre y se completa varias
/// veces durante la atención.
async fn guardar_seccion(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    id: i64,
    destino: &str,
    cambiar: impl FnOnce(&mut historia::HistoriaNueva),
) -> Response {
    let h = match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => h,
        Ok(None) => return Redirect::to(RUTA).into_response(),
        Err(_) => return averia(),
    };
    if h.firmada {
        return de_vuelta(destino, "cerrada");
    }
    let mut n = historia::como_nueva(&h);
    cambiar(&mut n);
    if crate::rutas::cups_de_consulta_valido(e, n.cod_consulta_cups.as_deref())
        .await
        .is_err()
    {
        return de_vuelta(destino, "cups_ajeno");
    }
    match historia::actualizar(&e.pool, u.ambito(), id, &n).await {
        Ok(historia::Resultado::Hecho) => de_vuelta(destino, "guardada"),
        Ok(historia::Resultado::Firmada) => de_vuelta(destino, "cerrada"),
        Ok(historia::Resultado::NoExiste) => Redirect::to(RUTA).into_response(),
        Err(_) => averia(),
    }
}

/// Los campos RIPS que la sección de diagnóstico escribe y que llevan procedencia
/// (#84). Todos son de fricción DURA (efecto jurídico/económico), así que ninguno
/// se confirma solo: la captura tiene que mandar su `procedencia_<campo>`.
const CAMPOS_RIPS_DIAGNOSTICO: &[&str] = &[
    "cod_consulta_cups",
    "finalidad",
    "causa_externa",
    "diagnostico_principal",
    "diagnostico_relacionado1",
    "diagnostico_relacionado2",
    "diagnostico_relacionado3",
];

/// Los campos de la FÓRMULA que viajan al AM y llevan procedencia (#84): los
/// cuatro que esta pantalla captura de verdad. El resto de lo que el AM exige de
/// `formula` —`tipo_medicamento`, `valor_unitario`, `valor_servicio`,
/// `concepto_recaudo`— no lo captura ninguna pantalla todavía (lo dice la
/// migración 0041), así que llega `NULL`, y sin valor no hay nada que respaldar.
///
/// **La lista tiene que ser COMPLETA respecto de lo que la pantalla captura, y esa
/// es la parte que se paga cara si se hace a ojo.** En cuanto una fórmula lleva
/// procedencia, el recolector audita TODOS sus campos con valor: uno capturado que
/// falte aquí sale marcado en el cierre de mes sin que nadie pueda confirmarlo, y
/// es la alarma que a la tercera nadie mira (directiva 23). Se midió con el
/// mutante, y el resultado corrigió lo que yo daba por hecho: con la lista corta
/// a `codigo_cum` y `dias_tratamiento`, la que salía marcada en toda fórmula
/// confirmada era **`unidad`**.
///
/// `cantidad` entró en la lista cuando todavía no se auditaba —`cantidad_entera`
/// salía por el camino corto sin pasar por el recolector—, y **ya se audita**:
/// lo arregla `Recolector::presente` en el mismo cambio. La lista queda igual;
/// lo que cambia es que ahora hace falta de verdad.
///
/// `dosis` y `observaciones` NO están, y es correcto: no viajan al AM.
///
/// **`medicamento` tampoco está, y la razón que aquí decía era FALSA.** Decía que
/// «no viajan al AM como código —lleva el CUM, no el nombre— así que el recolector
/// nunca los emite y nunca los audita»: la premisa es cierta y la conclusión no.
/// `medicamento` SÍ se emite, como `nomTecnologiaSalud` (`rips/am.rs`), sale con
/// valor en cada fórmula y lo captura esta misma pantalla. Que no se audite es una
/// decisión pendiente, no un hecho del modelo — y escrita así era un falso negativo
/// CON COARTADA, que es la peor clase: el próximo lector tenía aquí la razón para
/// no mirar. Lo cazó la revisión de la directiva 25 sobre `9f8c147d`. Queda como
/// ficha aparte junto con `valor_pago_moderador` y `num_autorizacion`, que salen
/// con `unwrap_or` sin pasar por el recolector.
const CAMPOS_RIPS_FORMULA: &[&str] =
    &["codigo_cum", "dias_tratamiento", "unidad", "cantidad"];

/// Fusiona la procedencia (#84) de los campos que ESTA sección guarda con la que ya
/// tenía la historia. Fricción graduada (opción 3): la señal explícita del
/// formulario (`procedencia_<campo>`) manda; un campo suave con valor se confirma
/// implícito; un campo DURO sin señal se OMITE, y así la auditoría lo marca como
/// sin respaldo.
///
/// **Los campos de ESTA sección se reescriben enteros, y el aval NO se hereda**:
/// lo que no venga afirmado en ESTE guardado pierde su procedencia anterior. «Lo
/// quité» no es «lo confirmé», y «lo cambié sin marcar la casilla» tampoco. Del
/// mapa previo solo sobreviven las claves de OTRAS secciones. `None` si no queda
/// ninguna (la historia no se audita hasta que la captura afirme algo).
fn fusionar_procedencias(
    existente: Option<&str>,
    form: &HashMap<String, String>,
    campos: &[&str],
) -> Option<String> {
    use medico_dominio::procedencia::{procedencias_de_captura, Procedencia};
    // EL MISMO SILENCIADOR, POR EL LADO QUE SÍ DESTRUYE. La lectura de la
    // pantalla dejó de tragarse un JSON ilegible el 2026-08-16; aquí seguía, y
    // aquí es peor: al guardar se REESCRIBE la columna con lo que quedó, así que
    // un mapa que no se pudo leer se convierte en un mapa vacío en disco. Lo vio
    // la segunda ronda de la directiva 25 sobre `b42033aa`.
    //
    // Se sigue degradando a «nada avalado» —parar el guardado clínico por una
    // columna corrupta sería el sesgo a cerrar— pero se GRITA, y el aviso dice
    // qué se pierde, que es lo que permite ir a mirarlo antes del cierre.
    let mut mapa: std::collections::BTreeMap<String, Procedencia> = match existente {
        Some(s) => serde_json::from_str(s).unwrap_or_else(|err| {
            eprintln!(
                "la columna `procedencias` de esta historia no se pudo leer ({err:?}). \
                 Se reescribe con lo que esta captura afirme; los avales anteriores \
                 que hubiera ahí dentro se pierden."
            );
            Default::default()
        }),
        None => Default::default(),
    };

    let tiene_valor = |c: &str| form.get(c).map(|v| !v.trim().is_empty()).unwrap_or(false);
    let con_valor: Vec<&str> = campos.iter().copied().filter(|c| tiene_valor(c)).collect();

    // FILTRADO A `confirmado`, igual que en `procedencias_del_formulario`. Aquí NO
    // estaba, y ésa era la asimetría: dos capturas leyendo la misma señal con dos
    // políticas, y una sola escrita. Lo cazó la revisión de la directiva 25 sobre
    // `9f8c147d`.
    //
    // Sin el filtro, un `POST /panel/historias/{id}/diagnostico` a mano —misma
    // sesión, sin navegador— con `procedencia_diagnostico_principal=no_aplica`
    // sobre un campo que SÍ tiene valor pasa la auditoría limpio: la fricción
    // `Dura` se satisface con `avalado_por_humano()`, que es `!matches!(Derivado)`,
    // así que `NoAplica` y `Propuesto` la satisfacen igual que `Confirmado`.
    //
    // No es evasión de privilegio —el mismo usuario puede mandar `confirmado` con
    // un clic— pero sí escribe en el rastro clínico un estado que NINGUNA pantalla
    // puede producir: «se preguntó y no procede» sobre un diagnóstico que está
    // puesto. Y se propaga: `avalado` vuelve a dibujar la casilla PRE-MARCADA, así
    // que el siguiente guardado normal de otra persona blanquea la etiqueta falsa
    // en una confirmación auténtica. Todo ello dentro de lo que firma la historia.
    let nuevos = procedencias_de_captura(con_valor, |campo| {
        form.get(&format!("procedencia_{campo}"))
            .and_then(|s| Procedencia::desde_str(s))
            .filter(|p| *p == Procedencia::Confirmado)
    });
    // ESTA SECCIÓN SE REESCRIBE ENTERA: quien guarda responde por lo que deja, y
    // el aval NO se hereda. Antes se borraba solo lo que quedaba VACÍO, y eso
    // dejaba el defecto exacto que este mismo archivo declara inaceptable tres
    // funciones más arriba: un campo DURO con valor y sin señal lo OMITE
    // `procedencias_de_captura` (`Friccion::Dura → None`), así que el `extend` no
    // lo tocaba y el `confirmado` anterior sobrevivía **pegado a un valor nuevo
    // que nadie miró**.
    //
    // Lo cazaron los DOS revisores de la directiva 25 sobre `1c570350`, por
    // separado y con confianza 8 los dos. La consecuencia que lo hace hallazgo no
    // es escalada de privilegio —el mismo usuario puede marcar la casilla— sino
    // que **desmarcarla no hacía nada**: el único gesto del producto para decir
    // «no respondo por esto» era un no-op, y no había forma de retirar un aval de
    // un campo que sigue teniendo valor. El filtro a `confirmado` del commit
    // anterior lo ensanchó: antes un `derivado` explícito al menos degradaba el
    // aval; con el filtro se descarta y el viejo se quedaba.
    for c in campos {
        mapa.remove(*c);
    }
    mapa.extend(nuevos);
    // `{}` y no `None`, por lo mismo que en `formula::json_de`: escribir la
    // columna es la afirmación «aquí hubo captura», y con `NULL` el recolector se
    // salta la historia entera. Los siete campos de esta sección son DUROS, así
    // que guardar sin marcar ninguna casilla dejaba el mapa vacío y la consulta
    // quedaba exenta de la auditoría — el caso normal, no el raro.
    serde_json::to_string(&mapa).ok()
}

/// La procedencia (#84) de un registro NUEVO que se afirma DE UNA VEZ —la fórmula—,
/// con una sola señal (`señal_del_registro`) que vale para todos sus campos RIPS.
///
/// Es la misma fricción graduada de [`fusionar_procedencias`] con la unidad movida
/// del campo al registro: sin la señal, los campos duros se OMITEN y el cierre del
/// mes los anota como sin respaldo; con ella, todos los que traen valor quedan
/// afirmados. Los que llegan vacíos no se registran — no se afirma lo que no hay.
fn procedencias_del_formulario(
    campos: &[&str],
    hay_valor: impl Fn(&str) -> bool,
    form: &HashMap<String, String>,
    señal_del_registro: &str,
) -> std::collections::BTreeMap<String, medico_dominio::procedencia::Procedencia> {
    use medico_dominio::procedencia::{procedencias_de_captura, Procedencia};
    // **Solo `confirmado`**, y no lo que venga. La casilla ofrece exactamente ese
    // valor; `Procedencia::desde_str` admite los cuatro, así que una petición
    // directa podía grabar `no_aplica` sobre un campo que SÍ tiene valor, o
    // `propuesto` sobre uno tecleado a mano. Los tres avalados por humano pasan la
    // auditoría igual —no es evasión— pero dejan una etiqueta falsa en el rastro
    // clínico, que es lo contrario de lo que #84 persigue.
    let afirmada = form
        .get(señal_del_registro)
        .and_then(|s| Procedencia::desde_str(s))
        .filter(|p| *p == Procedencia::Confirmado);
    let con_valor: Vec<&str> = campos.iter().copied().filter(|c| hay_valor(c)).collect();
    procedencias_de_captura(con_valor, |_| afirmada)
}

fn campo(nombre: &str, etiqueta: &str, valor: Option<&str>, editable: bool) -> Markup {
    html! {
        label for=(nombre) { (etiqueta) }
        @if editable {
            input id=(nombre) type="text" name=(nombre) value=(valor.unwrap_or(""));
        } @else {
            p class="mono" { (valor.unwrap_or("—")) }
        }
    }
}

/// Un campo RIPS de fricción DURA con su toque de confirmación (#84, opción 3).
/// El checkbox manda `procedencia_<campo>=confirmado` solo si el humano lo marca;
/// llega pre-marcado si ya estaba avalado, para no re-confirmar lo ya afirmado.
/// Sin marcar, la auditoría del cierre anota el campo como sin respaldo.
fn campo_confirmable(nombre: &str, etiqueta: &str, valor: Option<&str>, editable: bool, avalado: bool) -> Markup {
    html! {
        (campo(nombre, etiqueta, valor, editable))
        @if editable { (confirmar(nombre, avalado)) }
    }
}

/// El toque de confirmación de un campo duro (#84, opción 3). Manda
/// `procedencia_<campo>=confirmado` solo si se marca; pre-marcado si ya estaba avalado.
fn confirmar(nombre: &str, avalado: bool) -> Markup {
    html! {
        label class="confirmar sutil" {
            input type="checkbox" name=(format!("procedencia_{nombre}")) value="confirmado" checked[avalado];
            " Confirmo este valor — va a facturación"
        }
    }
}

fn area(nombre: &str, etiqueta: &str, valor: &str, editable: bool) -> Markup {
    html! {
        label for=(nombre) { (etiqueta) }
        @if editable {
            textarea id=(nombre) name=(nombre) rows="4" { (valor) }
        } @else {
            p { @if valor.is_empty() { "—" } @else { (valor) } }
        }
    }
}

fn numero(nombre: &str, etiqueta: &str, valor: Option<String>, editable: bool) -> Markup {
    html! {
        label for=(nombre) { (etiqueta) }
        @if editable {
            input id=(nombre) type="number" step="any" name=(nombre)
                  value=(valor.unwrap_or_default());
        } @else {
            p class="mono" { (valor.unwrap_or_else(|| "—".into())) }
        }
    }
}

// ── Sección: la consulta ─────────────────────────────────────────────────────

/// `GET /panel/historias/{id}`
pub async fn consulta(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    dibujar(&cx, "Consulta", html! {
        (encabezado(&f, "", q.aviso.trim()))
        section class="tarjeta" {
            h2 { "Motivo y enfermedad actual" }
            form method="post" action={ (RUTA) "/" (id) } class="apilado" {
                (area("motivo_consulta", "Motivo de consulta", &f.h.motivo_consulta, f.editable))
                (area("enfermedad_actual", "Enfermedad actual", &f.h.enfermedad_actual, f.editable))
                h3 { "Acompañante" }
                p class="sutil" {
                    "Se registra cuando alguien acompaña al paciente; en un menor de edad o en \
                     alguien que no puede consentir por sí mismo, es quien firma."
                }
                (campo("acomp_nombre", "Nombre", f.h.acomp_nombre.as_deref(), f.editable))
                (campo("acomp_documento", "Documento", f.h.acomp_documento.as_deref(), f.editable))
                (campo("acomp_parentesco", "Parentesco", f.h.acomp_parentesco.as_deref(), f.editable))
                (campo("acomp_telefono", "Teléfono", f.h.acomp_telefono.as_deref(), f.editable))
                @if f.editable { button type="submit" class="principal" { "Guardar" } }
            }
        }

        @if !f.h.firmada && cx.puede(Capacidad::FirmarClinico) {
            // Qué le falta lo decide `faltan_para_firmar`, la MISMA función que
            // usa `historia::firmar`. No se repite aquí la lista: con dos copias,
            // una se queda atrás y la pantalla ofrece un botón que lleva a un
            // rechazo. Ojo a las SECCIONES —el motivo vive en «Consulta», el
            // diagnóstico y el plan en las suyas—, así que lo que falta se dice
            // por su nombre y no «revise el formulario de arriba».
            @let faltan = f.h.faltan_para_firmar();
            section class="tarjeta" {
                h2 { "Cerrar la atención" }
                p class="sutil" {
                    "Firmar cierra la historia: deja de poder editarse y de poder borrarse. La \
                     firma acredita quién la hizo y sella el contenido, así que una alteración \
                     posterior se detecta."
                }
                @if faltan.is_empty() {
                    form method="post" action={ (RUTA) "/" (id) "/firmar" } {
                        button type="submit" class="principal"
                               data-confirmar="Pulse otra vez para firmar y cerrar" { "Firmar historia" }
                    }
                } @else {
                    // Se dice QUÉ falta, no «no se puede»: un botón que
                    // desaparece sin explicación se lee como un fallo del
                    // sistema. Y el borrador se sigue guardando — lo que se
                    // frena es la firma, que es lo que no se deshace.
                    (aviso(Tono::Error, &format!(
                        "Todavía no se puede firmar: falta {}. El borrador se guarda igual \
                         —la historia se escribe a lo largo de la atención—; lo que no puede \
                         existir es una historia FIRMADA que no diga nada.",
                        faltan.join(", ")
                    )))
                }
            }
        }
        @if !f.h.firmada && cx.puede(Capacidad::BorrarClinico) {
            section class="tarjeta" {
                h2 { "Eliminar" }
                p class="sutil" {
                    "Solo un borrador se elimina, y con él se van sus fórmulas, sus órdenes y \
                     sus adjuntos."
                }
                form method="post" action={ (RUTA) "/" (id) "/borrar" } {
                    button type="submit" class="peligro"
                           data-confirmar="Pulse otra vez para eliminarla" { "Eliminar historia" }
                }
            }
        }
    })
}

/// `POST /panel/historias/{id}`
pub async fn guardar_consulta(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}");
    guardar_seccion(&e, &u, id, &destino, |n| {
        n.motivo_consulta = texto(&c, "motivo_consulta").unwrap_or_default();
        n.enfermedad_actual = texto(&c, "enfermedad_actual").unwrap_or_default();
        n.acomp_nombre = texto(&c, "acomp_nombre");
        n.acomp_documento = texto(&c, "acomp_documento");
        n.acomp_parentesco = texto(&c, "acomp_parentesco");
        n.acomp_telefono = texto(&c, "acomp_telefono");
    })
    .await
}

// ── Sección: antecedentes (son del PACIENTE) ─────────────────────────────────

/// Lee del JSON guardado el valor de un campo, en la forma que espera su
/// control. Un bloque que no sea un objeto —o que no esté— devuelve vacío en vez
/// de romper: es historia clínica ya escrita y no se puede dejar sin abrir por
/// una fila con forma rara. Es la misma tolerancia que `parametros_de` en
/// laboratorio.
fn valor_guardado(bloque: &Value, campo: &antecedentes::Campo) -> String {
    let Some(v) = bloque.as_object().and_then(|o| o.get(campo.clave)) else {
        return String::new();
    };
    match v {
        Value::Bool(b) => (if *b { "1" } else { "" }).to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn marcado(bloque: &Value, clave: &str) -> bool {
    bloque
        .as_object()
        .and_then(|o| o.get(clave))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn dibujar_bloque(b: &Bloque, guardado: &Value, editable: bool) -> Markup {
    html! {
        section class="tarjeta" {
            h2 { (b.titulo) }
            p class="sutil" { (b.nota) }
            @for c in b.campos {
                @let nombre = format!("{}__{}", b.columna, c.clave);
                @match c.tipo {
                    Tipo::Casilla => {
                        @if editable {
                            label {
                                input type="checkbox" name=(nombre) value="1"
                                      checked[marcado(guardado, c.clave)];
                                " " (c.etiqueta)
                            }
                        } @else {
                            p { (c.etiqueta) ": " @if marcado(guardado, c.clave) { "sí" } @else { "no" } }
                        }
                    }
                    Tipo::Texto => (campo(&nombre, c.etiqueta, Some(&valor_guardado(guardado, c)), editable)),
                    Tipo::Fecha => {
                        label for=(nombre) { (c.etiqueta) }
                        @if editable {
                            input id=(nombre) type="date" name=(nombre)
                                  value=(valor_guardado(guardado, c));
                        } @else {
                            p class="mono" {
                                @let v = valor_guardado(guardado, c);
                                @if v.is_empty() { "—" } @else { (v) }
                            }
                        }
                    }
                    Tipo::Entero | Tipo::Decimal => {
                        @let v = valor_guardado(guardado, c);
                        (numero(&nombre, c.etiqueta, (!v.is_empty()).then_some(v), editable))
                    }
                }
            }
        }
    }
}

/// `GET /panel/historias/{id}/antecedentes`
pub async fn ver_antecedentes(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    // Los antecedentes son del PACIENTE, no de la atención: los edita quien
    // puede escribir su ficha, y una historia firmada no los cierra —el
    // antecedente sigue siendo el mismo mañana—.
    let editable = cx.puede(Capacidad::EscribirPaciente);
    let guardados: HashMap<&str, &Value> = HashMap::from([
        ("ant_familiares", &f.p.ant_familiares),
        ("ant_personales", &f.p.ant_personales),
        ("ant_ginecologicos", &f.p.ant_ginecologicos),
        ("ant_perinatales", &f.p.ant_perinatales),
    ]);
    let vacio = Value::Null;

    dibujar(&cx, "Antecedentes", html! {
        (encabezado(&f, "/antecedentes", q.aviso.trim()))
        section class="tarjeta" {
            p {
                "Los antecedentes son del PACIENTE y no de esta atención: se escriben una vez y "
                "valen para todas sus historias. Por eso siguen editándose aunque la historia "
                "esté firmada."
            }
        }
        form method="post" action={ (RUTA) "/" (id) "/antecedentes" } class="apilado" {
            @for b in antecedentes::BLOQUES {
                (dibujar_bloque(b, guardados.get(b.columna).copied().unwrap_or(&vacio), editable))
            }
            @if editable { button type="submit" class="principal" { "Guardar antecedentes" } }
        }
    })
}

/// `POST /panel/historias/{id}/antecedentes`
///
/// **Se escriben los cuatro bloques enteros**, con todos sus campos, también los
/// que quedan vacíos: una casilla desmarcada no se envía, así que guardar solo
/// lo que llega convertiría «desmarcó el asma» en «no dijo nada del asma», y en
/// un antecedente eso es lo contrario. Lo que hubiera de más en la columna —de
/// una versión anterior del formulario— **se conserva**: no se sabe qué es, y
/// borrarlo sería tirar historia clínica por no reconocerla.
pub async fn guardar_antecedentes(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirPaciente>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}/antecedentes");
    let h = match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => h,
        Ok(None) => return Redirect::to(RUTA).into_response(),
        Err(_) => return averia(),
    };
    let previos = match paciente::info(&e.pool, u.ambito(), h.paciente_id).await {
        Ok(Some(p)) => p,
        Ok(None) | Err(_) => return averia(),
    };

    let mut nuevos = paciente::Antecedentes {
        ant_familiares: None,
        ant_personales: None,
        ant_ginecologicos: None,
        ant_perinatales: None,
    };
    for b in antecedentes::BLOQUES {
        let anterior = match b.columna {
            "ant_familiares" => &previos.ant_familiares,
            "ant_personales" => &previos.ant_personales,
            "ant_ginecologicos" => &previos.ant_ginecologicos,
            _ => &previos.ant_perinatales,
        };
        // Se parte de lo que había: lo que este formulario no conoce se queda.
        let mut o: Map<String, Value> =
            anterior.as_object().cloned().unwrap_or_default();
        for campo in b.campos {
            let nombre = format!("{}__{}", b.columna, campo.clave);
            let v = match campo.tipo {
                Tipo::Casilla => Value::Bool(c.contains_key(&nombre)),
                Tipo::Texto | Tipo::Fecha => {
                    Value::String(texto(&c, &nombre).unwrap_or_default())
                }
                Tipo::Entero => entero(&c, &nombre).map_or(Value::Null, Value::from),
                Tipo::Decimal => decimal(&c, &nombre).map_or(Value::Null, Value::from),
            };
            o.insert(campo.clave.to_string(), v);
        }
        let valor = Some(Value::Object(o));
        match b.columna {
            "ant_familiares" => nuevos.ant_familiares = valor,
            "ant_personales" => nuevos.ant_personales = valor,
            "ant_ginecologicos" => nuevos.ant_ginecologicos = valor,
            _ => nuevos.ant_perinatales = valor,
        }
    }

    match paciente::actualizar_antecedentes(&e.pool, u.ambito(), h.paciente_id, &nuevos).await {
        Ok(true) => de_vuelta(&destino, "antecedentes"),
        Ok(false) => Redirect::to(RUTA).into_response(),
        Err(_) => averia(),
    }
}

// ── Sección: examen físico ───────────────────────────────────────────────────

/// `GET /panel/historias/{id}/examen`
///
/// Signos vitales y examen por aparatos, que en Angular eran dos pestañas
/// dentro del mismo paso. Aquí van en la misma página y en el mismo formulario:
/// son una sola exploración, y separarlas obligaba a guardar dos veces.
pub async fn examen(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let h = &f.h;
    dibujar(&cx, "Examen físico", html! {
        (encabezado(&f, "/examen", q.aviso.trim()))
        form method="post" action={ (RUTA) "/" (id) "/examen" } class="apilado" {
            section class="tarjeta" {
                h2 { "Signos vitales" }
                (campo("tension_arterial", "Tensión arterial", h.tension_arterial.as_deref(), f.editable))
                (campo("pam", "Presión arterial media", h.pam.as_deref(), f.editable))
                (numero("frecuencia_cardiaca", "Frecuencia cardiaca", h.frecuencia_cardiaca.map(|v| v.to_string()), f.editable))
                (numero("frecuencia_respiratoria", "Frecuencia respiratoria", h.frecuencia_respiratoria.map(|v| v.to_string()), f.editable))
                (numero("pulso", "Pulso", h.pulso.map(|v| v.to_string()), f.editable))
                (numero("temperatura", "Temperatura (°C)", h.temperatura.map(|v| v.to_string()), f.editable))
            }
            section class="tarjeta" {
                h2 { "Antropometría" }
                (numero("peso", "Peso (kg)", h.peso.map(|v| v.to_string()), f.editable))
                (numero("talla", "Talla (m)", h.talla.map(|v| v.to_string()), f.editable))
                (numero("pabd", "Perímetro abdominal (cm)", h.pabd.map(|v| v.to_string()), f.editable))
                (numero("pc", "Perímetro cefálico (cm)", h.pc.map(|v| v.to_string()), f.editable))
                // El índice se DERIVA del peso y la talla, y aquí solo se
                // enseña: es la misma decisión que se tomó en nutrición, donde
                // lo calculaba el navegador y el servidor lo guardaba tal cual
                // —un derivado firmado por el cliente, que podía contradecir su
                // propio peso—. La fórmula es la del sistema vivo.
                @if let Some(imc) = h.imc {
                    p { "Índice de masa corporal: " strong { (format!("{imc:.1}")) }
                        " — " (medico_dominio::antropometria::categoria_imc(imc)) }
                } @else {
                    p class="sutil" {
                        "El índice de masa corporal se calcula al guardar, a partir del peso y \
                         la talla. No se escribe a mano."
                    }
                }
                // Las dos referencias del original, que hasta hoy se guardaban
                // y NO se veían: el peso ideal se derivaba desde el 2026-08-11 y
                // la talla ideal ni existía. Se enseñan juntas y solo cuando hay
                // valor —«—» en un campo de referencia clínica se lee como si el
                // sistema hubiera calculado algo—, y se dice de dónde salen.
                @if h.peso_ideal.is_some() || h.talla_ideal.is_some() {
                    p {
                        "Referencia del sistema original: "
                        @if let Some(pi) = h.peso_ideal {
                            "peso ideal " strong { (format!("{pi:.1}")) " kg" }
                        }
                        @if h.peso_ideal.is_some() && h.talla_ideal.is_some() { ", " }
                        @if let Some(ti) = h.talla_ideal {
                            "talla ideal " strong { (format!("{ti:.2}")) " m" }
                        }
                        "."
                    }
                }
                (campo("estado_nutricional", "Estado nutricional", h.estado_nutricional.as_deref(), f.editable))
            }
            section class="tarjeta" {
                h2 { "Examen por aparatos" }
                p class="sutil" { "Lo que se deje en blanco no se explora: vacío es «no se examinó», no «normal»." }
                @for (col, etiqueta) in SISTEMAS {
                    (campo(col, etiqueta, valor_ef(h, col), f.editable))
                }
                (area("ef_hallazgos", "Otros hallazgos", h.ef_hallazgos.as_deref().unwrap_or(""), f.editable))
            }
            @if f.editable { button type="submit" class="principal" { "Guardar examen" } }
        }
    })
}

/// El valor de una columna `ef_*`. Es un `match` y no un mapa dinámico porque
/// Rust no da acceso a un campo por nombre, y una lista paralela sería lo mismo
/// con más sitios donde equivocarse: si falta una fila, el guardián
/// `el_examen_dibuja_los_diecisiete_aparatos` lo dice.
fn valor_ef<'a>(h: &'a historia::Historia, columna: &str) -> Option<&'a str> {
    let v = match columna {
        "ef_cabeza" => &h.ef_cabeza,
        "ef_ojos" => &h.ef_ojos,
        "ef_nariz" => &h.ef_nariz,
        "ef_oidos" => &h.ef_oidos,
        "ef_boca" => &h.ef_boca,
        "ef_faringe" => &h.ef_faringe,
        "ef_orl" => &h.ef_orl,
        "ef_cuello" => &h.ef_cuello,
        "ef_pulmones" => &h.ef_pulmones,
        "ef_corazon" => &h.ef_corazon,
        "ef_cardiopulmonar" => &h.ef_cardiopulmonar,
        "ef_abdomen" => &h.ef_abdomen,
        "ef_gu" => &h.ef_gu,
        "ef_neurologico" => &h.ef_neurologico,
        "ef_extremidad" => &h.ef_extremidad,
        "ef_osteomuscular" => &h.ef_osteomuscular,
        "ef_piel" => &h.ef_piel,
        _ => return None,
    };
    v.as_deref()
}

/// `POST /panel/historias/{id}/examen`
pub async fn guardar_examen(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}/examen");

    // El peso ideal necesita SEXO y EDAD, que no están en la historia sino en el
    // paciente, y la cocina de `guardar_seccion` es síncrona. Se resuelven aquí
    // antes de entrar, y se lee la historia una segunda vez a propósito: son dos
    // consultas por id sobre SQLite, y la alternativa —abrir el cierre a `async`
    // por un solo campo— le cambia la forma a las doce secciones que lo usan.
    //
    // Si algo de esto FALLA se para (`averia`) en vez de seguir con el peso ideal
    // en blanco: un `None` aquí significa «no se sabe» y borraría el valor
    // anterior, que es un dato clínico desapareciendo por un error de lectura
    // (directiva 20).
    let paciente = match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => match medico_datos::paciente::obtener(&e.pool, u.ambito(), h.paciente_id)
            .await
        {
            Ok(p) => p,
            Err(_) => return averia(),
        },
        // Que no exista o no sea suya lo resuelve `guardar_seccion` con su
        // redirección; aquí no se adelanta la respuesta.
        Ok(None) => None,
        Err(_) => return averia(),
    };
    let edad_y_sexo = paciente
        .as_ref()
        .and_then(|p| super::pacientes::edad(&p.fecha_nacimiento).map(|a| (a, p.sexo.clone())));
    // En MESES para la talla ideal, que se busca en una tabla con una fila por
    // cada mes del primer año. Ver `pacientes::edad_meses`.
    let meses_y_sexo = paciente
        .as_ref()
        .and_then(|p| super::pacientes::edad_meses(&p.fecha_nacimiento).map(|m| (m, p.sexo.clone())));

    guardar_seccion(&e, &u, id, &destino, |n| {
        n.tension_arterial = texto(&c, "tension_arterial");
        n.pam = texto(&c, "pam");
        n.frecuencia_cardiaca = entero(&c, "frecuencia_cardiaca");
        n.frecuencia_respiratoria = entero(&c, "frecuencia_respiratoria");
        n.pulso = entero(&c, "pulso");
        n.temperatura = decimal(&c, "temperatura");
        n.peso = decimal(&c, "peso");
        n.talla = decimal(&c, "talla");
        n.pabd = decimal(&c, "pabd");
        n.pc = decimal(&c, "pc");
        n.estado_nutricional = texto(&c, "estado_nutricional");
        // El índice lo DERIVA el servidor de lo que quede escrito, nunca el
        // cuerpo de la petición. Sin peso o sin talla no hay índice y se
        // borra el anterior: dejar el de la medición pasada al lado de un peso
        // nuevo es peor que no tener ninguno.
        n.imc = match (n.peso, n.talla) {
            (Some(p), Some(t)) => medico_dominio::antropometria::imc(p, t),
            _ => None,
        };
        // El peso ideal, por lo mismo y con la misma regla: lo deriva el
        // servidor de la talla escrita más el sexo y la edad del paciente, y sin
        // talla no hay peso ideal. Hasta hoy esta columna existía y no la
        // llenaba NADIE —la fórmula solo estaba en `backend/antropometria.py`,
        // que dejó de tener API el 2026-08-09—, así que el campo llevaba desde
        // el port siempre en blanco (#104).
        n.peso_ideal = match (n.talla, &edad_y_sexo) {
            (Some(t), Some((anios, sexo))) => {
                medico_dominio::antropometria::peso_ideal(t, sexo, *anios)
            }
            _ => None,
        };
        // La talla ideal, con la misma regla que las otras dos derivadas: la
        // pone el SERVIDOR y sin edad no hay talla ideal. A diferencia del peso
        // ideal esto no es una fórmula sino una BÚSQUEDA en la tabla
        // `PESOTALLA1` del original — que en el sistema vivo nunca llegó a
        // consultarse, porque el formulario abría la tabla equivocada (#104).
        n.talla_ideal = match &meses_y_sexo {
            Some((meses, sexo)) => medico_dominio::antropometria::talla_ideal(*meses, sexo),
            None => None,
        };
        n.ef_cabeza = texto(&c, "ef_cabeza");
        n.ef_ojos = texto(&c, "ef_ojos");
        n.ef_nariz = texto(&c, "ef_nariz");
        n.ef_oidos = texto(&c, "ef_oidos");
        n.ef_boca = texto(&c, "ef_boca");
        n.ef_faringe = texto(&c, "ef_faringe");
        n.ef_orl = texto(&c, "ef_orl");
        n.ef_cuello = texto(&c, "ef_cuello");
        n.ef_pulmones = texto(&c, "ef_pulmones");
        n.ef_corazon = texto(&c, "ef_corazon");
        n.ef_cardiopulmonar = texto(&c, "ef_cardiopulmonar");
        n.ef_abdomen = texto(&c, "ef_abdomen");
        n.ef_gu = texto(&c, "ef_gu");
        n.ef_neurologico = texto(&c, "ef_neurologico");
        n.ef_extremidad = texto(&c, "ef_extremidad");
        n.ef_osteomuscular = texto(&c, "ef_osteomuscular");
        n.ef_piel = texto(&c, "ef_piel");
        n.ef_hallazgos = texto(&c, "ef_hallazgos");
    })
    .await
}

// ── Sección: diagnóstico ─────────────────────────────────────────────────────

/// `GET /panel/historias/{id}/diagnostico`
///
/// Los tres campos que viajan al RIPS y que hasta el 2026-08-02 **nadie podía
/// elegir**: finalidad, causa o motivo, y el CUPS de la consulta. El sistema los
/// rellenaba con `10`, `13` y `890201`, códigos de la tabla ANTERIOR a la
/// Resolución 948 que el validador rechaza — puestos por el sistema en nombre
/// del médico.
///
/// **La finalidad se acota por el CUPS, pero solo se descarta lo que la tabla
/// RECHAZA.** `finalidad_admitida_para` tiene tres respuestas y `None` es la
/// mayoritaria: la tabla del MUV cubre 308 de 13 640 procedimientos (2,26 %).
/// Tratar «no sé» como «no vale» dejaría un selector vacío en el 97,74 % de las
/// consultas.
pub async fn diagnostico(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let h = &f.h;
    // #84: qué campos duros ya tienen aval humano, para pre-marcar su confirmación.
    //
    // `desde_json` está escrita a propósito para NO tragarse un JSON ilegible
    // (directiva 20, con su caso rojo en `procedencia.rs`), y aquí el llamante lo
    // silenciaba con `unwrap_or_default()`: una columna corrupta se dibujaba como
    // «ningún campo avalado», con todas las casillas desmarcadas y sin decir nada
    // — el mismo silenciador que `en_registro` acaba de quitar en el recolector.
    // Se sigue degradando a «nada avalado», que es la dirección segura, pero se
    // GRITA. Lo trajo la revisión de la directiva 25 sobre `1c570350`.
    let respaldos = match medico_dominio::procedencia::Respaldos::desde_json(
        h.procedencias.as_deref(),
    ) {
        Ok(r) => r,
        Err(err) => {
            eprintln!(
                "historia {id}: la columna `procedencias` no se pudo leer ({err:?}). \
                 La pantalla dibuja las casillas SIN marcar; el aval guardado sigue \
                 en la base y no se ha perdido."
            );
            medico_dominio::procedencia::Respaldos::default()
        }
    };
    // `== Confirmado`, y no `avalado_por_humano()`, que es `!matches!(Derivado)`.
    // Con el filtro del commit anterior la pantalla ya solo puede ESCRIBIR
    // `Confirmado`, pero una fila anterior con `no_aplica` o `propuesto` llegaba
    // aquí PRE-MARCADA, y el siguiente guardado la convertía en un `confirmado`
    // auténtico — el blanqueo que ese mismo commit describe por escrito y dejó
    // vivo en la lectura.
    let avalado =
        |campo: &str| respaldos.de(campo) == Some(medico_dominio::procedencia::Procedencia::Confirmado);
    let cups = h.cod_consulta_cups.clone().unwrap_or_default();
    let todas = medico_dominio::muv::finalidades();
    let ofrecidas: Vec<_> = todas
        .iter()
        .filter(|x| {
            cups.is_empty()
                || medico_dominio::muv::finalidad_admitida_para(&x.codigo, &cups) != Some(false)
        })
        .collect();
    let descartadas = todas.len() - ofrecidas.len();

    dibujar(&cx, "Diagnóstico", html! {
        (encabezado(&f, "/diagnostico", q.aviso.trim()))
        form method="post" action={ (RUTA) "/" (id) "/diagnostico" } class="apilado" {
            section class="tarjeta" {
                h2 { "Diagnósticos" }
                (campo_confirmable("diagnostico_principal", "CIE-10 principal", Some(&h.diagnostico_principal), f.editable, avalado("diagnostico_principal")))
                (campo_confirmable("diagnostico_relacionado1", "CIE-10 relacionado", h.diagnostico_relacionado1.as_deref(), f.editable, avalado("diagnostico_relacionado1")))
                (campo_confirmable("diagnostico_relacionado2", "CIE-10 relacionado (2)", h.diagnostico_relacionado2.as_deref(), f.editable, avalado("diagnostico_relacionado2")))
                (campo_confirmable("diagnostico_relacionado3", "CIE-10 relacionado (3)", h.diagnostico_relacionado3.as_deref(), f.editable, avalado("diagnostico_relacionado3")))
                (area("impresion_diagnostica", "Impresión diagnóstica", h.impresion_diagnostica.as_deref().unwrap_or(""), f.editable))
                @if let Some(c11) = &h.diagnostico_principal_cie11 {
                    p class="sutil" { "CIE-11 derivado: " span class="mono" { (c11) } }
                }
            }
            section class="tarjeta" {
                h2 { "Lo que viaja al RIPS" }
                p class="sutil" {
                    "Los tres van al archivo de consultas de la Resolución 948. Dejarlos sin \
                     elegir es legítimo —el cierre del mes los pide y los anota como faltantes—; \
                     lo que no vale es un código que nadie afirmó."
                }
                (campo_confirmable("cod_consulta_cups", "CUPS del procedimiento de la consulta", h.cod_consulta_cups.as_deref(), f.editable, avalado("cod_consulta_cups")))
                @if !cups.is_empty() {
                    p class="sutil" {
                        @match medico_dominio::muv::finalidades_de_cups(&cups) {
                            Some(_) => {
                                "La tabla del MUV cubre este procedimiento: se ofrecen solo las \
                                 finalidades que admite."
                            }
                            None => {
                                "La tabla del MUV no cubre este procedimiento —cubre 308 de \
                                 13 640—, así que no se descarta ninguna finalidad. Que se \
                                 ofrezcan todas no significa que todas valgan."
                            }
                        }
                    }
                }

                label for="finalidad" { "Finalidad de la atención" }
                @if f.editable {
                    select #finalidad name="finalidad" {
                        option value="" selected[h.finalidad.is_empty()] { "— Sin elegir —" }
                        @for x in &ofrecidas {
                            option value=(x.codigo) selected[h.finalidad == x.codigo] {
                                (x.codigo) " — " (x.nombre)
                            }
                        }
                    }
                    @if descartadas > 0 {
                        p class="sutil" {
                            (descartadas) " finalidades no aplican a ese procedimiento y no se \
                             ofrecen. Una lista que encoge sin explicación se lee como un fallo."
                        }
                    }
                    (confirmar("finalidad", avalado("finalidad")))
                } @else {
                    p class="mono" { @if h.finalidad.is_empty() { "—" } @else { (h.finalidad) } }
                }

                label for="causa" { "Causa o motivo de la atención" }
                @if f.editable {
                    select #causa name="causa_externa" {
                        option value="" selected[h.causa_externa.is_empty()] { "— Sin elegir —" }
                        @for x in medico_dominio::muv::causas_motivo() {
                            option value=(x.codigo) selected[h.causa_externa == x.codigo] {
                                (x.codigo) " — " (x.nombre)
                            }
                        }
                    }
                    (confirmar("causa_externa", avalado("causa_externa")))
                } @else {
                    p class="mono" { @if h.causa_externa.is_empty() { "—" } @else { (h.causa_externa) } }
                }
            }
            @if f.editable { button type="submit" class="principal" { "Guardar diagnóstico" } }
        }
    })
}

/// `POST /panel/historias/{id}/diagnostico`
pub async fn guardar_diagnostico(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}/diagnostico");
    guardar_seccion(&e, &u, id, &destino, |n| {
        n.diagnostico_principal = texto(&c, "diagnostico_principal").unwrap_or_default();
        n.diagnostico_relacionado1 = texto(&c, "diagnostico_relacionado1");
        n.diagnostico_relacionado2 = texto(&c, "diagnostico_relacionado2");
        n.diagnostico_relacionado3 = texto(&c, "diagnostico_relacionado3");
        n.impresion_diagnostica = texto(&c, "impresion_diagnostica");
        n.cod_consulta_cups = texto(&c, "cod_consulta_cups");
        // Vacío se guarda como vacío, no se conserva lo anterior: «lo quité»
        // y «no lo toqué» son cosas distintas, y el RIPS anota la ausencia.
        n.finalidad = Some(texto(&c, "finalidad").unwrap_or_default());
        n.causa_externa = Some(texto(&c, "causa_externa").unwrap_or_default());
        // #84: la procedencia de estos campos RIPS (todos duros) — la señal la
        // manda la captura; sin ella se omiten y la auditoría los marca.
        n.procedencias = fusionar_procedencias(n.procedencias.as_deref(), &c, CAMPOS_RIPS_DIAGNOSTICO);
    })
    .await
}

// ── Sección: plan ────────────────────────────────────────────────────────────

/// `GET /panel/historias/{id}/plan`
pub async fn plan(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let h = &f.h;
    dibujar(&cx, "Plan", html! {
        (encabezado(&f, "/plan", q.aviso.trim()))
        section class="tarjeta" {
            h2 { "Plan de tratamiento" }
            form method="post" action={ (RUTA) "/" (id) "/plan" } class="apilado" {
                (area("plan_tratamiento", "Conducta", &h.plan_tratamiento, f.editable))
                @if f.editable {
                    label {
                        input type="checkbox" name="genera_incapacidad" value="1"
                              checked[h.genera_incapacidad];
                        " Genera incapacidad"
                    }
                    (numero("dias_incapacidad", "Días de incapacidad", Some(h.dias_incapacidad.to_string()), true))
                    p class="sutil" {
                        "Los días solo cuentan si se marca que genera incapacidad. Esta casilla \
                         ANOTA la incapacidad en la historia; el certificado se emite en su \
                         propia pantalla, que es la que lleva las fechas y el diagnóstico."
                    }
                    button type="submit" class="principal" { "Guardar plan" }
                } @else {
                    p {
                        "Incapacidad: "
                        @if h.genera_incapacidad { (h.dias_incapacidad) " días" } @else { "no" }
                    }
                }
            }
        }
    })
}

/// `POST /panel/historias/{id}/plan`
pub async fn guardar_plan(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}/plan");
    guardar_seccion(&e, &u, id, &destino, |n| {
        n.plan_tratamiento = texto(&c, "plan_tratamiento").unwrap_or_default();
        let genera = c.contains_key("genera_incapacidad");
        n.genera_incapacidad = Some(genera);
        // Sin incapacidad no hay días. Guardar «14 días» junto a «no genera
        // incapacidad» deja una historia que se contradice a sí misma, y es el
        // par de campos que se desmarca sin borrar el número.
        n.dias_incapacidad = Some(if genera { entero(&c, "dias_incapacidad").unwrap_or(0) } else { 0 });
    })
    .await
}

// ── Sección: fórmula médica ──────────────────────────────────────────────────

/// `GET /panel/historias/{id}/formulas`
pub async fn formulas(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let items = match formula::listar_por_historia(&e.pool, u.ambito(), id).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let unidades = medico_datos::catalogo::unidades_dispensacion(&e.pool).await.unwrap_or_default();

    dibujar(&cx, "Fórmula médica", html! {
        (encabezado(&f, "/formulas", q.aviso.trim()))
        section class="tarjeta" {
            h2 { "Medicamentos prescritos" }
            @if items.is_empty() {
                p class="sutil" { "Esta atención no tiene fórmula." }
            } @else {
                (piezas::tabla(&["Medicamento", "CUM", "Dosis", "Cantidad", "Días", "Estado", ""], html! {
                    @for x in &items {
                        tr {
                            td { (x.medicamento) }
                            td class="mono sutil" { (x.codigo_cum.as_deref().unwrap_or("—")) }
                            td { (x.dosis) }
                            td {
                                (x.cantidad) " " (x.unidad.as_deref().unwrap_or(""))
                            }
                            td { (x.dias_tratamiento.map(|d| d.to_string()).unwrap_or_else(|| "—".into())) }
                            td {
                                @if x.dispensada { (piezas::estado("DISPENSADA")) }
                                @else { (piezas::estado("PENDIENTE")) }
                            }
                            td {
                                // Una fórmula DISPENSADA no se retira: la
                                // farmacia ya entregó, y borrar la prescripción
                                // dejaría una entrega de medicamento sin nada
                                // que la respalde. Lo decide `formula::eliminar`;
                                // aquí solo se deja de ofrecer.
                                @if f.editable && !x.dispensada {
                                    form method="post"
                                         action={ (RUTA) "/" (id) "/formulas/" (x.id) "/borrar" } {
                                        button type="submit" class="enlace"
                                               data-confirmar="Pulse otra vez para retirarlo" { "Retirar" }
                                    }
                                } @else if x.dispensada {
                                    span class="sutil" { "entregada" }
                                }
                            }
                        }
                    }
                }))
            }
        }
        @if f.editable {
            section class="tarjeta" {
                h2 { "Añadir un medicamento" }
                p class="sutil" {
                    "El CUM se valida contra el catálogo del INVIMA al generar el RIPS. Sin él \
                     la fórmula sirve igual para dispensar, pero el archivo del mes sale con \
                     una falta anotada."
                }
                form method="post" action={ (RUTA) "/" (id) "/formulas" } class="apilado" {
                    label for="med" { "Medicamento" }
                    input #med type="text" name="medicamento" required;
                    label for="cum" { "Código CUM" }
                    input #cum type="text" name="codigo_cum";
                    label for="dosis" { "Dosis" }
                    input #dosis type="text" name="dosis" required
                          placeholder="Ej: 1 tableta cada 8 horas";
                    label for="cant" { "Cantidad" }
                    // `min` es INCLUSIVO en HTML, así que `min="0"` dejaba enviar el
                    // cero que el servidor rechaza: el control decía una cosa y la
                    // regla otra. No sustituye a la guarda del servidor —un POST a
                    // mano se salta el navegador— pero un control que contradice a
                    // su regla enseña a desconfiar del formulario.
                    input #cant type="number" step="any" min="0.0001" name="cantidad" required;
                    label for="unidad" { "Unidad de dispensación" }
                    select #unidad name="unidad" {
                        option value="" { "— Sin elegir —" }
                        @for x in &unidades { option value=(x.nombre) { (x.nombre) } }
                    }
                    label for="dias" { "Días de tratamiento" }
                    input #dias type="number" min="0" name="dias_tratamiento";
                    label for="obs" { "Observaciones" }
                    input #obs type="text" name="observaciones";
                    // #84: UN toque por fórmula, no cuatro por campo. La unidad de
                    // afirmación aquí es el registro —se crea de una vez, en una
                    // pantalla, y los cuatro campos que van al AM se escriben
                    // juntos—, así que cuatro casillas serían cuatro veces la misma
                    // pregunta. En la sección de diagnóstico es al revés y por eso
                    // allí van por campo: se editan por separado y en visitas
                    // distintas.
                    label class="confirmar sutil" {
                        input type="checkbox" name="procedencia_formula" value="confirmado";
                        " Confirmo el CUM, la cantidad, la unidad y los días — van a facturación"
                    }
                    button type="submit" class="principal" { "Añadir" }
                }
            }
        }
    })
}

/// `POST /panel/historias/{id}/formulas`
pub async fn crear_formula(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}/formulas");
    // Una historia FIRMADA está cerrada, y eso alcanza a lo que cuelga de ella:
    // añadir un medicamento a una atención ya firmada sería modificarla por la
    // puerta de al lado.
    match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) if h.firmada => return de_vuelta(&destino, "cerrada"),
        Ok(Some(_)) => {}
        Ok(None) => return Redirect::to(RUTA).into_response(),
        Err(_) => return averia(),
    }
    // La cantidad que NO es un número se rechaza; no se convierte en 0.
    //
    // Estaba `decimal(...).unwrap_or(0.0)`, que es relleno de manual (directiva
    // 20): `cantidad=abc` guardaba una cantidad CERO, la mandaba al AM y —con la
    // casilla marcada— la certificaba como afirmada por un humano. El navegador
    // lo impide con `type=number required`; una petición desde la misma sesión,
    // no, y ese es el camino que hay que cerrar aquí y no en la pantalla.
    let cantidad = match c.get("cantidad").map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(_) => match decimal(&c, "cantidad") {
            // `is_finite` además de `> 0.0`: `str::parse::<f64>()` acepta `"inf"`,
            // `"+infinity"` y cualquier literal que desborde (`1e309`), y para
            // todos ellos `n > 0.0` es CIERTO. La fórmula se creaba con cantidad
            // infinita y encima `cantidad: confirmado`. Aguas abajo no se colaba
            // al archivo —`cantidad_entera` ve `fract()` = NaN y cae en `no_vale`—
            // pero el aviso promete «un número mayor que cero» y eso no era lo que
            // se comprobaba. Lo vio la revisión de la directiva 25 sobre `9f8c147d`.
            Some(n) if n.is_finite() && n > 0.0 => n,
            _ => return de_vuelta(&destino, "cantidad_invalida"),
        },
        None => return de_vuelta(&destino, "cantidad_invalida"),
    };
    // Los días de tratamiento, por la misma razón y por la misma puerta. Esta
    // ruta NO pasa por `serde`, así que la guarda de `FormulaNueva` no la
    // alcanza: hay que escribirla aquí también, y NO con un `.filter()`, que
    // silenciaría el negativo en vez de rechazarlo.
    //
    // El cero es legítimo (dosis única). Lo que se rechaza es el negativo, que
    // viajaba al AM como `numDiasTratamiento: -30` y —con la casilla marcada—
    // certificado. Lo vio la segunda ronda de la directiva 25 sobre `b42033aa`.
    let dias_tratamiento = match c.get("dias_tratamiento").map(|s| s.trim()) {
        Some(t) if !t.is_empty() => match entero(&c, "dias_tratamiento") {
            Some(d) if d >= 0 => Some(d),
            _ => return de_vuelta(&destino, "dias_invalidos"),
        },
        _ => None,
    };
    let nueva = formula::FormulaNueva {
        medicamento: texto(&c, "medicamento").unwrap_or_default(),
        codigo_cum: texto(&c, "codigo_cum"),
        dosis: texto(&c, "dosis").unwrap_or_default(),
        cantidad,
        unidad: texto(&c, "unidad"),
        dias_tratamiento,
        observaciones: texto(&c, "observaciones"),
    };
    // #84: la procedencia de los campos RIPS de esta fórmula. Fricción graduada:
    // sin el toque explícito, el campo duro se OMITE, la columna queda `{}` y el
    // cierre del mes lo anota como sin respaldo — no se finge que alguien lo
    // afirmó.
    //
    // Se decide sobre el valor YA GUARDADO (`nueva`), no sobre el texto del
    // formulario: eran dos cosas distintas y por eso `dias_tratamiento=abc`
    // dejaba una clave afirmando un valor que la base no llegó a tener.
    let procedencias = procedencias_del_formulario(
        CAMPOS_RIPS_FORMULA,
        |campo| match campo {
            "codigo_cum" => nueva.codigo_cum.is_some(),
            "dias_tratamiento" => nueva.dias_tratamiento.is_some(),
            "unidad" => nueva.unidad.is_some(),
            "cantidad" => true, // ya validada arriba: si llegó aquí, es > 0
            // Un campo añadido a la lista y no a este `match` cae en «no hay
            // valor»: no se afirma, y el cierre lo marca. Ruidoso en vez de
            // ciego, que es la dirección segura (directiva 21).
            _ => false,
        },
        &c,
        "procedencia_formula",
    );
    match formula::crear(&e.pool, u.ambito(), id, &nueva, &procedencias).await {
        Ok(medico_datos::Creacion::Creada(_)) => de_vuelta(&destino, "formula"),
        Ok(medico_datos::Creacion::NoExiste) => Redirect::to(RUTA).into_response(),
        // La comprobación de `h.firmada` que este manejador ya hacía sigue
        // arriba y es la que responde en el camino normal; esto es la segunda
        // vuelta de llave, ahora en la capa de datos, por si alguien llega aquí
        // por otra puerta.
        Ok(medico_datos::Creacion::Firmada) => de_vuelta(&destino, "historia_firmada"),
        Err(_) => averia(),
    }
}

/// `POST /panel/historias/{id}/formulas/{fid}/borrar`
pub async fn borrar_formula(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path((id, fid)): Path<(i64, i64)>,
) -> Response {
    let destino = format!("{RUTA}/{id}/formulas");
    match formula::eliminar(&e.pool, u.ambito(), fid).await {
        Ok(formula::ResultadoBorrado::Hecho) => de_vuelta(&destino, "formula_borrada"),
        // Se dice CUÁNTAS entregas la respaldan: «no se puede» sin el dato
        // manda a buscar el motivo a la farmacia.
        Ok(formula::ResultadoBorrado::YaDispensada { entregas }) => {
            let _ = entregas;
            de_vuelta(&destino, "ya_dispensada")
        }
        Ok(formula::ResultadoBorrado::NoExiste) => de_vuelta(&destino, "no_estaba"),
        Ok(formula::ResultadoBorrado::HistoriaFirmada) => {
            de_vuelta(&destino, "historia_firmada")
        }
        Err(_) => averia(),
    }
}

// ── Sección: órdenes ─────────────────────────────────────────────────────────

/// `GET /panel/historias/{id}/ordenes`
pub async fn ordenes(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let items = match orden::listar_por_historia(&e.pool, u.ambito(), id).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let especialidades = medico_datos::catalogo::especialidades(&e.pool).await.unwrap_or_default();

    dibujar(&cx, "Órdenes", html! {
        (encabezado(&f, "/ordenes", q.aviso.trim()))
        section class="tarjeta" {
            h2 { "Órdenes de esta atención" }
            p class="sutil" {
                "Una orden de laboratorio o de imágenes aparece en la bandeja de su servicio en \
                 cuanto se crea; una remisión a especialista, en la de remisiones."
            }
            @if items.is_empty() {
                p class="sutil" { "Esta atención no tiene órdenes." }
            } @else {
                (piezas::tabla(&["Tipo", "Descripción", "CUPS", "Estado", ""], html! {
                    @for x in &items {
                        tr {
                            td { (x.tipo) }
                            td { (x.descripcion) }
                            td class="mono sutil" { (x.codigo_cups.as_deref().unwrap_or("—")) }
                            td { (piezas::estado(&x.estado)) }
                            td {
                                // Solo se retira lo que nadie ha empezado: una
                                // orden en proceso ya movió trabajo en otro
                                // servicio, y borrarla dejaría un resultado sin
                                // orden que lo pidiera.
                                @if f.editable && x.estado == "Pendiente" {
                                    form method="post"
                                         action={ (RUTA) "/" (id) "/ordenes/" (x.id) "/borrar" } {
                                        button type="submit" class="enlace"
                                               data-confirmar="Pulse otra vez para eliminarla" { "Eliminar" }
                                    }
                                } @else if x.estado != "Pendiente" {
                                    span class="sutil" { "en curso" }
                                }
                            }
                        }
                    }
                }))
            }
        }
        @if f.editable {
            section class="tarjeta" {
                h2 { "Nueva orden" }
                form method="post" action={ (RUTA) "/" (id) "/ordenes" } class="apilado" {
                    label for="tipo" { "Tipo" }
                    select #tipo name="tipo" required {
                        @for x in TIPOS_ORDEN { option value=(x) { (x) } }
                    }
                    label for="desc" { "Descripción" }
                    input #desc type="text" name="descripcion" required;
                    label for="just" { "Justificación" }
                    textarea #just name="justificacion" rows="2" required {}
                    label for="cups_o" { "CUPS" }
                    input #cups_o type="text" name="codigo_cups";
                    label for="esp" { "Especialidad de destino" }
                    select #esp name="especialidad_destino_id" {
                        option value="" { "— No aplica —" }
                        @for x in &especialidades { option value=(x.id) { (x.nombre) } }
                    }
                    button type="submit" class="principal" { "Crear orden" }
                }
            }
        }
    })
}

/// `POST /panel/historias/{id}/ordenes`
pub async fn crear_orden(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}/ordenes");
    match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) if h.firmada => return de_vuelta(&destino, "cerrada"),
        Ok(Some(_)) => {}
        Ok(None) => return Redirect::to(RUTA).into_response(),
        Err(_) => return averia(),
    }
    let nueva = orden::OrdenNueva {
        tipo: texto(&c, "tipo").unwrap_or_default(),
        descripcion: texto(&c, "descripcion").unwrap_or_default(),
        justificacion: texto(&c, "justificacion").unwrap_or_default(),
        codigo_cups: texto(&c, "codigo_cups"),
        especialidad_destino_id: entero(&c, "especialidad_destino_id"),
        institucion_destino_id: None,
    };
    match orden::crear(&e.pool, u.ambito(), id, &nueva).await {
        Ok(medico_datos::Creacion::Creada(_)) => de_vuelta(&destino, "orden"),
        Ok(medico_datos::Creacion::NoExiste) => Redirect::to(RUTA).into_response(),
        Ok(medico_datos::Creacion::Firmada) => de_vuelta(&destino, "historia_firmada"),
        // Un tipo fuera del `CHECK` llega aquí como error de la base. Se cuenta
        // como rechazo y no como avería: lo que falla es lo que se pidió.
        Err(_) => de_vuelta(&destino, "orden_invalida"),
    }
}

/// `POST /panel/historias/{id}/ordenes/{oid}/borrar`
pub async fn borrar_orden(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<EscribirClinico>,
    Path((id, oid)): Path<(i64, i64)>,
) -> Response {
    let destino = format!("{RUTA}/{id}/ordenes");
    match orden::eliminar(&e.pool, u.ambito(), oid).await {
        Ok(medico_datos::Resultado::Hecho) => de_vuelta(&destino, "orden_borrada"),
        Ok(medico_datos::Resultado::NoExiste) => de_vuelta(&destino, "no_estaba"),
        Ok(medico_datos::Resultado::Firmada) => de_vuelta(&destino, "historia_firmada"),
        Err(_) => averia(),
    }
}

// ── Sección: imágenes adjuntas ───────────────────────────────────────────────

/// `GET /panel/historias/{id}/imagenes`
///
/// Son los ADJUNTOS de la historia —una radiografía traída por el paciente, un
/// resultado en papel escaneado—, no el módulo de imagenología: aquellos son
/// órdenes con su informe y viven en `/panel/imagenes`. La licencia lo distingue
/// igual (`LA_RUTA_MIENTE_SOBRE_SU_MODULO`), y por la misma razón: un
/// consultorio que no compró imagenología tiene que poder abrir los adjuntos de
/// sus propias historias.
pub async fn imagenes(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    Query(q): Query<Filtro>,
) -> Response {
    let cx = match contexto(&e, &u).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let f = match cargar(&e, &u, &cx, id).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let items = match imagen::listar_por_historia(&e.pool, u.ambito(), id).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let puede_borrar = cx.puede(Capacidad::BorrarClinico);

    dibujar(&cx, "Imágenes", html! {
        (encabezado(&f, "/imagenes", q.aviso.trim()))
        section class="tarjeta" {
            h2 { "Archivos de la atención" }
            // Las dos clases se DICEN, porque no son lo mismo ante la firma: el
            // adjunto que pone el médico es contenido del documento y entra en
            // lo que ésta sella; la placa que sube imagenología llega después y
            // no. Hasta el 2026-08-10 se pintaban idénticas, así que ni el
            // clínico ni un auditor podían saber qué cubre la firma.
            p class="sutil" {
                "Los adjuntos de la historia forman parte del documento y quedan dentro de "
                "su firma. Los resultados de imagenología llegan después y no la alteran."
            }
            @if items.is_empty() {
                p class="sutil" { "Esta atención no tiene archivos adjuntos." }
            } @else {
                (piezas::tabla(&["Archivo", "Origen", "Descripción", "Tipo", "Tamaño", "Fecha", ""], html! {
                    @for x in &items {
                        tr {
                            td {
                                // Una placa DICOM el navegador no la pinta: se muestra su
                                // vista PREVIA renderizada (el módulo de píxeles). Si el
                                // códec no se soporta, `/previa` da 404 y queda el enlace de
                                // descarga —nunca una imagen inventada (D20)—.
                                @if x.tipo_mime == "application/dicom" {
                                    a href={ "/api/imagenes/" (x.id) "/archivo" } {
                                        img src={ "/api/imagenes/" (x.id) "/previa" }
                                            alt="vista previa DICOM (descargue el original si no se muestra)"
                                            class="miniatura-dicom" loading="lazy";
                                    }
                                    br;
                                }
                                a href={ "/api/imagenes/" (x.id) "/archivo" } { (x.nombre_original) }
                            }
                            td class="sutil" {
                                @if x.sellado_con_la_firma() {
                                    "Adjunto de la historia"
                                } @else {
                                    "Resultado de imagenología"
                                }
                            }
                            td { (x.descripcion) }
                            td class="mono sutil" { (x.tipo_mime) }
                            td { (x.tamano_bytes / 1024) " KB" }
                            td { (x.fecha_subida) }
                            td {
                                @if puede_borrar {
                                    form method="post"
                                         action={ (RUTA) "/" (id) "/imagenes/" (x.id) "/borrar" } {
                                        button type="submit" class="enlace"
                                               data-confirmar="Pulse otra vez para eliminarlo" { "Eliminar" }
                                    }
                                }
                            }
                        }
                    }
                }))
            }
        }
        // SUBIR exige `EscribirClinico` desde el 2026-08-10 —igual que
        // `POST /api/historias/{id}/imagenes`— y BORRAR exige `BorrarClinico`.
        // El comentario anterior decía que subir pedía solo sesión «porque el
        // radiólogo no está entre los roles que borran»; era falso y la placa
        // del radiólogo entra por su ruta propia.
        @if !f.h.firmada {
            section class="tarjeta" {
                h2 { "Adjuntar un archivo" }
                p class="sutil" {
                    "El tipo se deduce de los BYTES, no de la extensión ni de lo que diga el \
                     navegador, y el archivo se guarda cifrado."
                }
                form method="post" action={ (RUTA) "/" (id) "/imagenes" }
                     enctype="multipart/form-data" class="apilado" {
                    label for="archivo" { "Archivo" }
                    input #archivo type="file" name="archivo" required;
                    label for="descripcion" { "Descripción" }
                    input #descripcion type="text" name="descripcion";
                    button type="submit" class="principal" { "Adjuntar" }
                }
            }
        }
    })
}

/// `POST /panel/historias/{id}/imagenes`
pub async fn subir_imagen(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Response {
    let destino = format!("{RUTA}/{id}/imagenes");
    match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) if h.firmada => return de_vuelta(&destino, "cerrada"),
        Ok(Some(_)) => {}
        Ok(None) => return Redirect::to(RUTA).into_response(),
        Err(_) => return averia(),
    }
    // El MISMO camino que la ruta JSON: es el único por el que entran bytes al
    // sistema, con el tipo deducido de la firma de los bytes y el cifrado en
    // reposo. Un segundo almacén sería la puerta por donde entrase el SVG que
    // `tipos.rs` existe para no admitir.
    match crate::rutas::guardar_imagen_de(
        &e,
        &u,
        id,
        multipart,
        "",
        medico_datos::imagen::Origen::AdjuntoDeLaHistoria,
    )
    .await
    {
        Ok(_) => de_vuelta(&destino, "imagen"),
        Err(_) => de_vuelta(&destino, "imagen_rechazada"),
    }
}

/// `POST /panel/historias/{id}/imagenes/{iid}/borrar`
pub async fn borrar_imagen(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<Borrar>,
    Path((id, iid)): Path<(i64, i64)>,
) -> Response {
    let destino = format!("{RUTA}/{id}/imagenes");
    match imagen::eliminar(&e.pool, u.ambito(), iid).await {
        Ok(Some(ruta)) => {
            let _ = tokio::fs::remove_file(e.dir_imagenes.join(&ruta)).await;
            de_vuelta(&destino, "imagen_borrada")
        }
        Ok(None) => de_vuelta(&destino, "no_estaba"),
        Err(_) => averia(),
    }
}

// ── Firmar y eliminar ────────────────────────────────────────────────────────

/// `POST /panel/historias/{id}/firmar`
///
/// La firma **afirma autoría clínica**, así que la exige el extractor y no un
/// `if` que la ruta siguiente olvidará. Con clave configurada sella el contenido
/// entero —quién firmó y cuándo van DENTRO de lo firmado—, así que una
/// alteración posterior se detecta con `/api/historias/{id}/verificar-firma`.
pub async fn firmar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<Firmar>,
    Path(id): Path<i64>,
) -> Response {
    let destino = format!("{RUTA}/{id}");
    match historia::firmar(&e.pool, u.ambito(), id, u.id, e.clave_firma.as_deref()).await {
        Ok(historia::ResultadoFirma::Firmada) => de_vuelta(&destino, "firmada"),
        // No existe, es ajena o ya estaba firmada: los tres se cuentan igual,
        // que es lo que hace `/api`.
        Ok(historia::ResultadoFirma::NoSePudo) => de_vuelta(&destino, "no_se_firmo"),
        // La pantalla ya deja de ofrecer el botón cuando falta algo, así que
        // llegar aquí es un formulario reconstruido a mano o una carrera con
        // otro que vació un campo. Se vuelve a la ficha, donde la tarjeta de
        // firma dice exactamente cuál falta. **Va explícito y no bajo el `_`**:
        // con el comodín, «le falta el diagnóstico» se contestaba con la avería
        // del despliegue, que es el mismo error que este arreglo viene a cerrar.
        Ok(historia::ResultadoFirma::Incompleto { .. }) => de_vuelta(&destino, "incompleta"),
        // Hay clave y no sirve: es un fallo del DESPLIEGUE, no de quien firma.
        _ => averia(),
    }
}

/// `POST /panel/historias/{id}/borrar`
pub async fn borrar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<Borrar>,
    Path(id): Path<i64>,
) -> Response {
    let paciente_id = match historia::obtener(&e.pool, u.ambito(), id).await {
        Ok(Some(h)) => h.paciente_id,
        Ok(None) => return Redirect::to(RUTA).into_response(),
        Err(_) => return averia(),
    };
    match historia::eliminar(&e.pool, u.ambito(), id).await {
        Ok(historia::Resultado::Hecho) => {
            de_vuelta(&format!("{RUTA}/paciente/{paciente_id}"), "borrada")
        }
        Ok(historia::Resultado::Firmada) => de_vuelta(&format!("{RUTA}/{id}"), "cerrada"),
        Ok(historia::Resultado::NoExiste) => Redirect::to(RUTA).into_response(),
        Err(_) => averia(),
    }
}

/// La página de «no está», con el menú puesto para no dejar a nadie sin salida.
///
/// **Un registro ajeno y uno inexistente se cuentan igual**, que es la regla que
/// este repositorio tiene escrita desde el principio: un 403 confirmaría que el
/// id existe, y con eso se enumera el padrón del vecino probando números — y en
/// una IPS, saber que alguien es paciente de la clínica de al lado ya es un dato
/// de salud.
fn no_existe(cx: &Contexto, que: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrada",
            Some(crate::vista::panel::menu(cx, RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { (que) }
                    p { "Puede que el enlace esté mal o que ya no exista." }
                    p { a href=(RUTA) { "Volver a historias clínicas" } }
                }
            },
        ),
    )
        .into_response()
}

#[cfg(test)]
mod pruebas_procedencia {
    use super::*;

    fn form(pares: &[(&str, &str)]) -> HashMap<String, String> {
        pares.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// #84 write-side y su pareja (directiva 33): la señal explícita del formulario
    /// escribe la procedencia del campo duro; sin señal, el campo duro se OMITE (el
    /// cierre lo marcará sin respaldo); un campo vaciado pierde su respaldo previo.
    #[test]
    fn fusionar_procedencias_grada_la_friccion() {
        // CUPS confirmado por el humano; diagnóstico con valor pero SIN confirmar.
        let f = form(&[
            ("cod_consulta_cups", "890201"),
            ("procedencia_cod_consulta_cups", "confirmado"),
            ("diagnostico_principal", "R69"),
        ]);
        let json = fusionar_procedencias(None, &f, CAMPOS_RIPS_DIAGNOSTICO).unwrap();
        assert!(json.contains("\"cod_consulta_cups\":\"confirmado\""), "el confirmado se escribe: {json}");
        assert!(!json.contains("diagnostico_principal"), "el duro sin toque se omite → sin respaldo: {json}");

        // Un campo que se vacía pierde su respaldo previo: «lo quité» no es «lo confirmé».
        // Y el resultado es `{}`, NO `None`: la historia sigue DENTRO de la
        // auditoría. Esta línea decía `None`, y con eso vaciar el único campo
        // confirmado sacaba la consulta entera del control — más silencio, no más
        // ruido, que es la dirección equivocada.
        let previo = r#"{"diagnostico_principal":"confirmado"}"#;
        let f2 = form(&[("diagnostico_principal", "")]);
        assert_eq!(
            fusionar_procedencias(Some(previo), &f2, CAMPOS_RIPS_DIAGNOSTICO).as_deref(),
            Some("{}"),
            "vaciar el último confirmado no puede EXIMIR la historia de auditarse"
        );
    }

    /// **EL AVAL NO SE HEREDA**, y su pareja: reenviar la casilla marcada sí lo
    /// conserva.
    ///
    /// Lo cazaron los DOS revisores de la directiva 25 sobre `1c570350`, por
    /// separado y con confianza 8 los dos. `fusionar_procedencias` partía del mapa
    /// existente, hacía `extend` con lo afirmado y borraba solo lo VACÍO; un campo
    /// DURO con valor y sin señal lo OMITE `procedencias_de_captura`
    /// (`Friccion::Dura → None`), así que el `confirmado` anterior sobrevivía
    /// **pegado a un valor nuevo que nadie miró**.
    ///
    /// Por qué es defecto y no gusto: **desmarcar la casilla no hacía nada**. Era
    /// el único gesto del producto para decir «no respondo por esto», y no había
    /// forma de retirar un aval de un campo que sigue teniendo valor. La prueba
    /// anterior no lo veía porque los cinco casos pasan `existente = None`, que es
    /// justo por donde se coló.
    ///
    /// El caso rojo va por la vía REAL —la misma función que llama el manejador
    /// del panel, con el mapa previo que devuelve la base— y no por un arnés.
    #[test]
    fn el_aval_anterior_no_se_hereda_sobre_un_valor_nuevo() {
        let previo = r#"{"diagnostico_principal":"confirmado"}"#;

        // ROJO: valor NUEVO, casilla DESMARCADA (el navegador no manda la clave).
        let sin_senal = form(&[("diagnostico_principal", "C50")]);
        assert_eq!(
            fusionar_procedencias(Some(previo), &sin_senal, CAMPOS_RIPS_DIAGNOSTICO).as_deref(),
            Some("{}"),
            "el aval del valor ANTERIOR no puede quedarse pegado al nuevo"
        );

        // VERDE, la pareja: con la casilla marcada el aval se renueva de verdad.
        let con_senal = form(&[
            ("diagnostico_principal", "C50"),
            ("procedencia_diagnostico_principal", "confirmado"),
        ]);
        assert_eq!(
            fusionar_procedencias(Some(previo), &con_senal, CAMPOS_RIPS_DIAGNOSTICO).as_deref(),
            Some(r#"{"diagnostico_principal":"confirmado"}"#),
            "reenviar la casilla marcada SÍ conserva el aval"
        );

        // Y lo que NO es de esta sección no se toca: la poda es de `campos`, no
        // del mapa entero.
        //
        // **La clave del control es REAL y alcanzable**, y la primera versión de
        // esta prueba usaba `codigo_cum`, que vive en `formula.procedencias` —
        // otra tabla— así que el control habría pasado igual con una regla que no
        // podara nada (directiva 33: el control no se fabrica). La cazó la
        // segunda ronda de la directiva 25 sobre `b42033aa`.
        //
        // `diagnostico_principal_cie11` sí llega a esta columna: lo escribe el
        // sistema Python heredado como `DERIVADO` (`backend/cie11.py:189-190`),
        // y ninguna pantalla lo alcanza. Es exactamente la clase de clave que la
        // poda NO debe tocar.
        let ajeno =
            r#"{"diagnostico_principal_cie11":"derivado","diagnostico_principal":"confirmado"}"#;
        let json = fusionar_procedencias(Some(ajeno), &sin_senal, CAMPOS_RIPS_DIAGNOSTICO).unwrap();
        assert!(
            json.contains("\"diagnostico_principal_cie11\":\"derivado\""),
            "la poda es de los campos de la sección, no del mapa entero: {json}"
        );
        // La clave EXACTA con sus dos puntos: `contains("diagnostico_principal")`
        // a secas casa también con `diagnostico_principal_cie11`, que es
        // justamente la que tiene que sobrevivir.
        assert!(
            !json.contains("\"diagnostico_principal\":"),
            "y el suyo sí se retira: {json}"
        );
    }

    /// **UNA SEÑAL QUE NO ES `confirmado` NO SE GUARDA**, y su pareja: `confirmado`
    /// sí.
    ///
    /// Lo cazó la revisión de la directiva 25 sobre `9f8c147d`: la captura de la
    /// FÓRMULA filtraba la señal a `Confirmado` y la del DIAGNÓSTICO no — dos
    /// lecturas de la misma señal con dos políticas, y una sola escrita.
    ///
    /// Por qué importa aunque no sea evasión de privilegio: la fricción `Dura` se
    /// satisface con `avalado_por_humano()`, que es `!matches!(Derivado)`, así que
    /// `NoAplica` y `Propuesto` la pasan igual que `Confirmado`. Un
    /// `POST /panel/historias/{id}/diagnostico` a mano con `no_aplica` sobre un
    /// campo que SÍ tiene valor escribe en el rastro clínico un estado que NINGUNA
    /// pantalla puede producir —«se preguntó y no procede» sobre un diagnóstico
    /// puesto—, y encima se propaga: `avalado` vuelve a dibujar la casilla
    /// PRE-MARCADA, de modo que el siguiente guardado de otra persona blanquea la
    /// etiqueta falsa en una confirmación auténtica. Y todo eso entra en la firma.
    #[test]
    fn una_senal_que_no_es_confirmado_no_se_guarda() {
        let con_valor = |p: &str| {
            form(&[("diagnostico_principal", "J00"), ("procedencia_diagnostico_principal", p)])
        };
        for falsa in ["no_aplica", "propuesto", "derivado", "cualquier_cosa"] {
            let json = fusionar_procedencias(None, &con_valor(falsa), CAMPOS_RIPS_DIAGNOSTICO)
                .expect("la columna se escribe siempre");
            assert_eq!(
                json, "{}",
                "`{falsa}` no puede quedar como aval del diagnóstico: {json}"
            );
        }
        // LA PAREJA: `confirmado` sí se guarda. Sin ella pasaría igual un filtro
        // que tirara TODAS las señales, que es el vigilante ciego.
        let json = fusionar_procedencias(None, &con_valor("confirmado"), CAMPOS_RIPS_DIAGNOSTICO)
            .expect("la columna se escribe siempre");
        assert!(
            json.contains("\"diagnostico_principal\":\"confirmado\""),
            "el aval de verdad tiene que entrar: {json}"
        );
    }

    /// El rechazo de la cantidad tiene que **VERSE**, y su pareja: una clave que
    /// nadie tradujo no pinta nada.
    ///
    /// Es el caso rojo que le faltaba a las seis líneas de `traducir_aviso` para
    /// `cantidad_invalida`, y su ausencia la midió `sin-caso-rojo` el 2026-08-15:
    /// se podían borrar enteras y el banco no se enteraba. El E2E
    /// `la_cantidad_que_no_es_numero_no_se_guarda_como_cero` cubre la GUARDA —que
    /// la fórmula no se cree—, pero no que quien la escribió llegue a saber por
    /// qué: sin este brazo, `de_vuelta` redirige con `?aviso=cantidad_invalida`,
    /// `traducir_aviso` devuelve `None` y la pantalla sale **muda**. El
    /// medicamento no se añade y nadie dice nada, que es el fallo silencioso de
    /// la directiva 20 en la única capa donde lo ve una persona.
    #[test]
    fn el_rechazo_de_la_cantidad_se_traduce_a_un_error_visible() {
        let (tono, texto) = traducir_aviso("cantidad_invalida")
            .expect("`cantidad_invalida` tiene que traducirse: si no, la pantalla calla");
        // `Tono` no deriva `Debug` —es de producción y no se toca por una
        // prueba—, así que el mensaje nombra lo esperado, no lo recibido.
        assert!(
            matches!(tono, Tono::Error),
            "un rechazo se pinta con `Tono::Error`, no como aviso amable"
        );
        assert!(
            texto.contains("mayor que cero"),
            "el texto tiene que decir POR QUÉ se rechazó: {texto}"
        );

        // LA PAREJA (directiva 33): una clave que nadie tradujo no inventa
        // mensaje. Sin esto pasaría igual un `traducir_aviso` que devolviera
        // `Some` para todo, que es un instrumento que dice que sí siempre.
        assert!(
            traducir_aviso("una_clave_que_nadie_escribio").is_none(),
            "lo no traducido no pinta nada"
        );
    }
}
