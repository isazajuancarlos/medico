// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Superadministración: dar de alta una IPS, fijarle el plan, decidir qué
//! módulos compró y crearle su primer administrador. Es el **onboarding de un
//! cliente**, y la última pantalla del port.
//!
//! Port de `frontend/src/app/pages/superadmin/superadmin.component.ts` (406
//! líneas, una tabla y tres `MatDialog`).
//!
//! # El superadmin no tenía panel, literalmente
//!
//! `vista::contexto` resuelve la institución de quien mira y el superadmin no
//! pertenece a ninguna, así que devolvía `SinPanel::Superadmin` —«esta cuenta no
//! tiene panel»— para TODAS las pantallas. Correcto para las clínicas: el panel
//! es la consola de UN prestador, y enseñarle a un superadmin el padrón de
//! pacientes de todos los clientes no es lo que nadie pidió.
//!
//! Por eso esta pantalla **no usa `panel::menu` ni `Contexto`**: lleva su propio
//! menú, de tres entradas, y su puerta es `EnSesionCon<GestionarInstituciones>`,
//! que no depende de ninguna institución (esa capacidad no se configura desde la
//! matriz de una: un administrador que se la concediera se estaría regalando el
//! producto entero). Así el superadmin gana su pantalla sin que las clínicas
//! ganen un espectador.
//!
//! # Dos cosas que ninguna pantalla podía hacer, y ahora sí
//!
//! 1. **Los módulos licenciados.** `PUT /api/instituciones/{id}/modulos` existe
//!    en el binario desde hace tiempo y **ningún frontal lo llamaba**: medido,
//!    `modulos_habilitados` solo se podía cambiar tocando la base. Es la tarea
//!    **#65.6**, y bloqueaba dar de alta un cliente nuevo — un alta que no puede
//!    decidir qué se vendió no es un alta.
//! 2. **El cupo de MÉDICOS.** `superadmin.service.ts` manda
//!    `{max_usuarios, vence}` y nada más, así que `max_medicos` —la segunda cota,
//!    que nació el 2026-08-06 porque una IPS con plan de 3 médicos podía dar de
//!    alta 20— no tenía por dónde fijarse. La columna existía, la puerta la
//!    miraba y nadie podía escribirla.
//!
//! # Lo que se conserva
//!
//! - **El NIT es único y su choque se cuenta como tal** (`NitEnUso` → 409 por
//!   `/api`): dos IPS con el mismo NIT facturan a la misma EPS con la misma
//!   identidad.
//! - **Una licencia sin fecha es PERPETUA, no vencida.** Vacío = sin
//!   vencimiento, que es lo que dice el formulario de Angular y lo que hace
//!   `licencia::fijar`.
//! - **El primer administrador se crea sin pasar por el cupo**: el alta de una
//!   IPS la hace el superadmin, y `crear_usuario` lo exime a propósito — si no,
//!   una licencia recién fijada bloquearía al administrador que va a usarla.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};
use medico_datos::{institucion, licencia, usuario};
use medico_dominio::modulo;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::{Estado, GestionarInstituciones};
use crate::vista::{aviso, averia, pagina, piezas, EnSesionCon, Tono};

const RUTA: &str = "/panel/superadmin";

fn traducir_aviso(clave: &str) -> Option<(Tono, &'static str)> {
    match clave {
        "ips" => Some((Tono::Ok, "Institución creada. Fíjele el plan y los módulos.")),
        "licencia" => Some((Tono::Ok, "Plan fijado.")),
        "modulos" => Some((Tono::Ok, "Módulos guardados.")),
        "admin" => Some((Tono::Ok, "Administrador creado. Ya puede entrar y configurar su institución.")),
        "nit_en_uso" => Some((
            Tono::Error,
            "Ese NIT ya está registrado. Dos instituciones con el mismo NIT facturan a la EPS \
             con la misma identidad: no se crea la segunda.",
        )),
        "faltan_datos" => Some((Tono::Error, "El nombre y el NIT son obligatorios.")),
        "cupo_invalido" => Some((
            Tono::Error,
            "El cupo de usuarios tiene que ser al menos 1, y el de médicos —si se pacta— también. \
             Cero no es un plan: es una institución que no puede tener a nadie dentro.",
        )),
        "modulo_desconocido" => Some((
            Tono::Error,
            "Se pidió un módulo que no está en el catálogo. No se guardó nada: una licencia con \
             un nombre inventado dentro queda inerte y nadie sabe por qué ese cliente no ve lo \
             que cree haber comprado.",
        )),
        "admin_rechazado" => Some((
            Tono::Error,
            "No se pudo crear el administrador. Puede que ese correo ya esté en uso.",
        )),
        "no_existe" => Some((Tono::Error, "Esa institución ya no está.")),
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
}

fn texto(c: &HashMap<String, String>, k: &str) -> Option<String> {
    c.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn entero(c: &HashMap<String, String>, k: &str) -> Option<i64> {
    texto(c, k).and_then(|s| s.parse().ok())
}

/// Las entradas del menú del superadmin. **Una lista y no HTML suelto** para
/// que el guardián pueda recorrerlas y comprobar que ninguna es un callejón.
const ENTRADAS: &[(&str, &str)] = &[(RUTA, "Instituciones")];

/// El menú del superadmin, que **no es el del prestador**.
///
/// Una entrada y ninguna clínica: quien administra el sistema no pertenece a
/// ninguna institución, así que no hay módulos que ofrecerle ni pacientes que
/// enseñarle.
///
/// **`/panel/usuarios` NO entra, y estuvo a punto.** La pantalla de Angular
/// repetía debajo la tabla de todos los usuarios del sistema, y lo obvio era
/// enlazarla; pero `vista/usuarios.rs` llama a `contexto()`, que a un superadmin
/// le devuelve «esta cuenta no tiene panel». Habría sido una entrada de menú que
/// lleva a una página que dice que no hay nada — el mismo callejón que ya dejó
/// `/panel/usuarios` huérfano durante tres lotes, visto por el otro lado. El
/// personal se enseña **por institución**, dentro de su ficha, que además es lo
/// que hace falta para dar de alta a un cliente.
fn menu(activo: &str) -> Markup {
    html! {
        nav class="menu" aria-label="Secciones" {
            a class="marca" href=(RUTA) { "medico" }
            p class="menu-titulo" { "Sistema" }
            ul {
                @for (ruta, etiqueta) in ENTRADAS {
                    li {
                        a href=(ruta) aria-current=[(*ruta == activo).then_some("page")] {
                            (etiqueta)
                        }
                    }
                }
            }
            form method="post" action="/panel/salir" class="menu-salir" {
                button type="submit" class="enlace" { "Salir" }
            }
        }
    }
}

fn dibujar(titulo: &str, activo: &str, cuerpo: Markup) -> Response {
    pagina(titulo, Some(menu(activo)), cuerpo).into_response()
}

// ── El listado de instituciones ──────────────────────────────────────────────

/// `GET /panel/superadmin`
pub async fn listar(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarInstituciones>,
    Query(f): Query<Filtro>,
) -> Response {
    let ips = match institucion::listar(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    // El plan de cada una, para que la tabla diga de un vistazo cuál está sin
    // configurar. Sin esto hay que abrir las fichas una por una, que es lo que
    // hacía la pantalla anterior.
    let hoy = medico_dominio::zona::hoy().to_string();
    let mut planes: HashMap<i64, licencia::Estado> = HashMap::new();
    for i in &ips {
        match licencia::estado(&e.pool, i.id, &hoy).await {
            Ok(s) => {
                planes.insert(i.id, s);
            }
            Err(_) => return averia(),
        }
    }

    dibujar("Instituciones", RUTA, html! {
        header class="titulo-seccion" {
            h1 { "Instituciones" }
            p class="sutil" {
                "Dé de alta una IPS, fíjele el plan, decida qué módulos compró y créele su \
                 primer administrador."
            }
        }
        @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }

        section class="tarjeta" {
            @if ips.is_empty() {
                p class="sutil" { "Todavía no hay instituciones. Cree la primera." }
            } @else {
                (piezas::tabla(&["IPS", "NIT", "Estado", "Plan", ""], html! {
                    @for i in &ips {
                        tr {
                            td { (i.nombre) }
                            td class="mono" { (i.nit) }
                            td {
                                @if i.activa { (piezas::estado("ACTIVA")) }
                                @else { (piezas::estado("INACTIVA")) }
                            }
                            td { (frase_del_plan(planes.get(&i.id))) }
                            td { a href={ (RUTA) "/" (i.id) } { "Abrir" } }
                        }
                    }
                }))
            }
        }

        section class="tarjeta" {
            h2 { "Nueva IPS" }
            form method="post" action=(RUTA) class="apilado" {
                label for="nombre" { "Nombre" }
                input #nombre type="text" name="nombre" required;
                label for="nit" { "NIT" }
                input #nit type="text" name="nit" required;
                p class="sutil" {
                    "Nace SIN licencia y SIN módulos: los dos se fijan después, en su ficha. \
                     Una institución recién creada no puede dar de alta a nadie hasta que tenga \
                     plan, y eso es a propósito."
                }
                button type="submit" class="principal" { "Crear" }
            }
        }
    })
}

/// Qué dice la tabla del plan de cada IPS. **«Sin licencia» y «vencida» se
/// cuentan distinto**: la primera es una institución a medio dar de alta y la
/// segunda es un cliente que dejó de pagar, y se atienden de forma distinta.
fn frase_del_plan(estado: Option<&licencia::Estado>) -> Markup {
    html! {
        @match estado {
            Some(licencia::Estado::SinLicencia) => span class="falta" { "sin licencia" },
            Some(licencia::Estado::Vencida { vence }) => {
                span class="falta" { "vencida el " (vence) }
            }
            Some(licencia::Estado::Vigente { max_usuarios, max_medicos, .. }) => {
                (max_usuarios) " usuarios"
                @if let Some(m) = max_medicos { ", " (m) " médicos" }
            }
            None => span class="sutil" { "—" },
        }
    }
}

/// `POST /panel/superadmin`
pub async fn crear_ips(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarInstituciones>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let (Some(nombre), Some(nit)) = (texto(&c, "nombre"), texto(&c, "nit")) else {
        return de_vuelta(RUTA, "faltan_datos");
    };
    let nueva = institucion::InstitucionNueva { nombre, nit };
    match institucion::crear(&e.pool, u.ambito(), &nueva).await {
        // Se va derecho a su ficha: lo que sigue es fijarle el plan y los
        // módulos, y volver al listado obligaría a buscarla entre las demás.
        Ok(institucion::ResultadoCrear::Creada(id)) => {
            de_vuelta(&format!("{RUTA}/{id}"), "ips")
        }
        Ok(institucion::ResultadoCrear::NitEnUso) => de_vuelta(RUTA, "nit_en_uso"),
        // El extractor ya exigió la capacidad, así que aquí `SinPermiso` sería
        // un desacuerdo entre las dos puertas: es avería nuestra, no del cliente.
        Ok(institucion::ResultadoCrear::SinPermiso) | Err(_) => averia(),
    }
}

// ── La ficha de una IPS ──────────────────────────────────────────────────────

/// `GET /panel/superadmin/{id}`
pub async fn ficha(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarInstituciones>,
    Path(id): Path<i64>,
    Query(f): Query<Filtro>,
) -> Response {
    let ips = match institucion::listar(&e.pool, u.ambito()).await {
        Ok(v) => v,
        Err(_) => return averia(),
    };
    let Some(i) = ips.iter().find(|x| x.id == id) else {
        return no_existe();
    };
    let hoy = medico_dominio::zona::hoy().to_string();
    let plan = match licencia::estado(&e.pool, id, &hoy).await {
        Ok(s) => s,
        Err(_) => return averia(),
    };
    let habilitados = match institucion::modulos_habilitados(&e.pool, id).await {
        Ok(Some(h)) => h,
        Ok(None) => return no_existe(),
        Err(_) => return averia(),
    };
    let gente = match usuario::listar(&e.pool, u.ambito()).await {
        Ok(v) => v.into_iter().filter(|x| x.institucion_id == Some(id)).collect::<Vec<_>>(),
        Err(_) => return averia(),
    };

    // El formulario sale con lo que HAY, incluida la fecha —también si está
    // vencida—: un campo en blanco convertiría «subir el cupo» en «además,
    // hacerla perpetua», y eso no lo pidió nadie.
    let (max_usuarios, max_medicos, vence) = match &plan {
        licencia::Estado::Vigente { max_usuarios, max_medicos, vence } => {
            (Some(*max_usuarios), *max_medicos, vence.clone())
        }
        licencia::Estado::Vencida { vence } => (None, None, Some(vence.clone())),
        licencia::Estado::SinLicencia => (None, None, None),
    };

    dibujar(&i.nombre, RUTA, html! {
        (piezas::migas(RUTA, "Instituciones"))
        @if let Some((tono, t)) = traducir_aviso(f.aviso.trim()) { (aviso(tono, t)) }
        header class="titulo-seccion" {
            h1 { (i.nombre) }
            p class="sutil" { "NIT " (i.nit) }
        }

        section class="tarjeta" {
            h2 { "Plan" }
            @match &plan {
                licencia::Estado::SinLicencia => (aviso(Tono::Error,
                    "Esta institución no tiene licencia: no puede dar de alta a nadie. Es lo \
                     normal en una IPS recién creada; fíjele el plan aquí abajo.")),
                licencia::Estado::Vencida { vence } => (aviso(Tono::Error,
                    &format!("La licencia venció el {vence}. Se sigue atendiendo y consultando \
                              con normalidad; lo que se frena es crecer."))),
                licencia::Estado::Vigente { .. } => {}
            }
            form method="post" action={ (RUTA) "/" (id) "/licencia" } class="apilado" {
                label for="max_usuarios" { "Cupo de usuarios" }
                input #max_usuarios type="number" min="1" name="max_usuarios" required
                      value=(max_usuarios.map(|v| v.to_string()).unwrap_or_default());
                label for="max_medicos" { "Cupo de médicos (opcional)" }
                input #max_medicos type="number" min="1" name="max_medicos"
                      value=(max_medicos.map(|v| v.to_string()).unwrap_or_default());
                p class="sutil" {
                    "Son DOS cotas y no una. Vacío = este plan no pactó cupo de médicos, que no \
                     es lo mismo que cero: las licencias firmadas antes de que existiera esta \
                     columna no lo pactaron, y ponerles un número sería inventarles una \
                     cláusula. Ojo: sin esta pantalla no había forma de fijarlo — el frontal \
                     anterior no mandaba el campo."
                }
                label for="vence" { "Vence (opcional)" }
                input #vence type="date" name="vence" value=(vence.unwrap_or_default());
                p class="sutil" {
                    "Vacío = perpetua. Una licencia vencida bloquea CRECER —dar de alta, \
                     reactivar, ascender a médico—, nunca la lectura: dejar a un hospital sin \
                     sus registros por un asunto comercial lo pone a incumplir ante el paciente, \
                     la EPS y la Supersalud."
                }
                button type="submit" class="principal" { "Fijar plan" }
            }
        }

        section class="tarjeta" {
            h2 { "Módulos contratados" }
            p class="sutil" {
                "Lo que esta IPS compró. Un módulo sin marcar responde 403 en el servidor, no \
                 solo se esconde del menú."
            }
            form method="post" action={ (RUTA) "/" (id) "/modulos" } class="apilado" {
                @for grupo in modulo::grupos() {
                    h3 { (grupo) }
                    @for m in modulo::CATALOGO.iter().filter(|m| m.grupo == grupo) {
                        label {
                            input type="checkbox" name="modulo" value=(m.nombre)
                                  checked[habilitados.iter().any(|h| h == m.nombre)];
                            " " (m.nombre)
                            // Se DICE cuál está construido y cuál no. Vender uno
                            // pendiente es legítimo —el cliente lo compra por
                            // adelantado— pero venderlo sin saberlo no.
                            @if m.prefijo.is_none() {
                                " " span class="sutil" { "(aún no construido)" }
                            }
                        }
                    }
                }
                button type="submit" class="principal" { "Guardar módulos" }
            }
        }

        section class="tarjeta" {
            h2 { "Personal" }
            @if gente.is_empty() {
                p class="sutil" {
                    "Esta institución no tiene a nadie dentro. Créele un administrador: es quien \
                     configura lo demás y da de alta al resto."
                }
            } @else {
                (piezas::tabla(&["Nombre", "Correo", "Rol", "Estado"], html! {
                    @for x in &gente {
                        tr {
                            td { (x.nombre) }
                            td class="mono sutil" { (x.email) }
                            td { (x.rol) }
                            td {
                                @if x.activo { (piezas::estado("ACTIVA")) }
                                @else { (piezas::estado("INACTIVA")) }
                            }
                        }
                    }
                }))
            }
        }

        section class="tarjeta" {
            h2 { "Crear un administrador" }
            p class="sutil" {
                "El alta que hace el superadmin NO consume asiento de la licencia, a propósito: \
                 si lo consumiera, una licencia recién fijada podría bloquear al administrador \
                 que va a usarla."
            }
            form method="post" action={ (RUTA) "/" (id) "/admin" } class="apilado" {
                label for="nombre_a" { "Nombre" }
                input #nombre_a type="text" name="nombre" required;
                label for="email_a" { "Correo de acceso" }
                input #email_a type="email" name="email" required autocomplete="off";
                label for="clave_a" { "Contraseña" }
                input #clave_a type="password" name="clave" required autocomplete="new-password";
                button type="submit" class="principal" { "Crear administrador" }
            }
        }
    })
}

/// `POST /panel/superadmin/{id}/licencia`
pub async fn fijar_licencia(
    State(e): State<Estado>,
    EnSesionCon(_u, _): EnSesionCon<GestionarInstituciones>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}");
    let Some(max_usuarios) = entero(&c, "max_usuarios").filter(|v| *v >= 1) else {
        return de_vuelta(&destino, "cupo_invalido");
    };
    // El de médicos es opcional; lo que NO se acepta es cero, que el `CHECK` del
    // esquema rechaza y que además no significa nada como plan.
    let max_medicos = entero(&c, "max_medicos");
    if max_medicos.is_some_and(|m| m < 1) {
        return de_vuelta(&destino, "cupo_invalido");
    }
    match licencia::fijar(&e.pool, id, max_usuarios, max_medicos, texto(&c, "vence").as_deref())
        .await
    {
        Ok(()) => de_vuelta(&destino, "licencia"),
        Err(_) => averia(),
    }
}

/// `POST /panel/superadmin/{id}/modulos`
///
/// **Cierra la tarea #65.6**: hasta hoy `modulos_habilitados` solo se podía
/// cambiar tocando la base, porque `PUT /api/instituciones/{id}/modulos` existía
/// y ningún frontal lo llamaba.
///
/// Una casilla sin marcar no se envía, así que la lista que llega **es** la
/// licencia entera: no se mezcla con lo que había. Es lo correcto aquí —quitar
/// un módulo es una operación real, y con la mezcla no se podría— y por eso el
/// formulario dibuja el catálogo completo y no solo lo contratado.
pub async fn fijar_modulos(
    State(e): State<Estado>,
    EnSesionCon(_u, _): EnSesionCon<GestionarInstituciones>,
    Path(id): Path<i64>,
    Form(c): Form<Vec<(String, String)>>,
) -> Response {
    let destino = format!("{RUTA}/{id}");
    let pedidos: Vec<String> =
        c.iter().filter(|(k, _)| k == "modulo").map(|(_, v)| v.clone()).collect();
    // Un nombre que no está en el catálogo se RECHAZA, no se filtra en silencio:
    // guardar una licencia con `TELETRANSPORTE` dentro la dejaría inerte y nadie
    // sabría por qué ese cliente no ve el módulo que cree haber comprado. Es la
    // misma regla que la ruta JSON.
    if pedidos.iter().any(|m| !modulo::existe(m)) {
        return de_vuelta(&destino, "modulo_desconocido");
    }
    match institucion::fijar_modulos(&e.pool, id, &pedidos).await {
        Ok(true) => de_vuelta(&destino, "modulos"),
        Ok(false) => de_vuelta(RUTA, "no_existe"),
        Err(_) => averia(),
    }
}

/// `POST /panel/superadmin/{id}/admin`
pub async fn crear_admin(
    State(e): State<Estado>,
    EnSesionCon(u, _): EnSesionCon<GestionarInstituciones>,
    Path(id): Path<i64>,
    Form(c): Form<HashMap<String, String>>,
) -> Response {
    let destino = format!("{RUTA}/{id}");
    let nuevo = medico_datos::administracion::UsuarioAltaAdmin {
        username: None,
        email: texto(&c, "email"),
        password: c.get("clave").cloned().unwrap_or_default(),
        nombre_completo: texto(&c, "nombre").unwrap_or_default(),
        // El rol NO viaja en el formulario: esta pantalla crea ADMINISTRADORES
        // de la institución y nada más. Ofrecer el selector entero aquí sería
        // fabricar superadministradores desde el alta de un cliente, y el resto
        // del personal lo da de alta el administrador que se crea aquí — que es
        // quien conoce a su gente.
        rol: "admin".into(),
        especialidad_id: None,
        registro_medico: None,
        registro_rethus: None,
        // El alta de un administrador de institución no captura el nombre partido
        // del profesional (#603): eso es dato clínico y lo llena la pantalla de
        // personal cuando la persona atiende.
        nombre1: None,
        nombre2: None,
        apellido1: None,
        apellido2: None,
    };
    // La MISMA función que `/api/admin/usuarios` y que `/panel/admin/usuarios`:
    // las cinco puertas —ascenso, ámbito, licencia, política de contraseña y
    // bitácora obligatoria— viven en un sitio. Aquí el ámbito es el del
    // superadmin, así que la institución se dice; y la licencia lo exime.
    match crate::rutas::dar_de_alta_personal(&e, &u, Some(id), &nuevo).await {
        crate::rutas::EscrituraPersonal::Creado(_) => de_vuelta(&destino, "admin"),
        crate::rutas::EscrituraPersonal::Rechazado { .. } => de_vuelta(&destino, "admin_rechazado"),
        crate::rutas::EscrituraPersonal::NoExiste => de_vuelta(RUTA, "no_existe"),
        // Un superadmin está exento del cupo: llegar aquí sería un desacuerdo
        // entre `motivo_de_licencia` y esta pantalla.
        crate::rutas::EscrituraPersonal::SinCupo(_)
        | crate::rutas::EscrituraPersonal::Hecho
        | crate::rutas::EscrituraPersonal::Averia => averia(),
    }
}

fn no_existe() -> Response {
    (
        StatusCode::NOT_FOUND,
        pagina(
            "No encontrada",
            Some(menu(RUTA)),
            html! {
                section class="tarjeta estrecha" {
                    h1 { "No se encontró esa institución" }
                    p { "Puede que el enlace esté mal o que ya no exista." }
                    p { a href=(RUTA) { "Volver a instituciones" } }
                }
            },
        ),
    )
        .into_response()
}
