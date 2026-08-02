// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! La matriz de capacidades de una institución: consultarla y cambiarla (#235).
//!
//! Quien la toca necesita [`Capacidad::GestionarUsuarios`], que es la misma que
//! da de alta al personal — decidir quién puede hacer qué y decidir quién es
//! quién son la misma responsabilidad, y partirlas en dos permisos crearía la
//! figura del administrador que nombra pero no autoriza, que no existe en
//! ninguna IPS.
//!
//! **Un administrador solo ve y toca la matriz de SU institución.** El
//! `institucion_id` no se acepta por parámetro para quien pertenece a una: se
//! toma del usuario. El superadmin, que no pertenece a ninguna, sí tiene que
//! decir de cuál habla.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use medico_datos::capacidad;
use medico_dominio::ajustes::Ajustes;
use medico_dominio::rol::{Capacidad, RolUsuario};
use serde::{Deserialize, Serialize};

use crate::auth::{Autorizado, Estado, GestionarUsuarios};

/// Una casilla de la matriz, tal como la pinta la pantalla.
#[derive(Debug, Serialize)]
pub struct Casilla {
    pub rol: RolUsuario,
    /// La respuesta EFECTIVA en esta institución: lo que el servidor va a hacer.
    pub permitido: bool,
    /// Lo que diría el defecto nacional. La pantalla marca de otro color las
    /// casillas donde `permitido != defecto`, para que se vea qué se apartó.
    pub defecto: bool,
    /// Hay una fila guardada para esta casilla. **No es lo mismo que
    /// `permitido != defecto`**: se puede guardar una fila que coincide con el
    /// defecto de hoy, y entonces esa casilla deja de seguirlo si el defecto
    /// cambia mañana. La pantalla necesita poder ofrecer «devolver al defecto»
    /// justo ahí, y sin este campo no sabría que hay algo que borrar.
    pub apartado: bool,
    /// Esta casilla se puede tocar. `false` para el superadmin y el paciente.
    pub configurable: bool,
}

#[derive(Debug, Serialize)]
pub struct Fila {
    pub capacidad: Capacidad,
    /// Esta capacidad se puede configurar por institución.
    /// `gestionar_instituciones` no.
    pub configurable: bool,
    pub roles: Vec<Casilla>,
}

#[derive(Debug, Serialize)]
pub struct Matriz {
    pub institucion_id: i64,
    pub capacidades: Vec<Fila>,
}

#[derive(Debug, Deserialize)]
pub struct DeQuien {
    /// Solo lo usa el superadmin. Para todos los demás se ignora: su matriz es
    /// la de su institución y punto.
    pub institucion_id: Option<i64>,
}

/// De qué institución habla esta petición.
///
/// Para quien pertenece a una, la suya — el parámetro se **ignora**, no se
/// rechaza: rechazarlo convertiría un intento inocuo en un error, y aceptarlo
/// sería el agujero. Para el superadmin, la que diga; sin decirlo, 400.
///
/// Devuelve el error como par pequeño y no como `Response`: una `Response`
/// entera en la variante `Err` de un ayudante pesa 128 bytes que se copian en
/// cada retorno del camino feliz (`clippy::result_large_err`). El llamante la
/// arma.
fn institucion_de(
    u: &medico_datos::usuario::Usuario,
    pedida: Option<i64>,
) -> Result<i64, (StatusCode, &'static str)> {
    match u.institucion_id {
        Some(propia) => Ok(propia),
        None => pedida.ok_or((
            StatusCode::BAD_REQUEST,
            "sin institución propia: indique institucion_id",
        )),
    }
}

fn construir(institucion_id: i64, ajustes: &Ajustes) -> Matriz {
    let guardadas: std::collections::HashSet<(Capacidad, RolUsuario)> =
        ajustes.listar().into_iter().map(|(c, r, _)| (c, r)).collect();

    let capacidades = Capacidad::TODAS
        .iter()
        .map(|&c| Fila {
            capacidad: c,
            configurable: c.configurable_por_institucion(),
            roles: RolUsuario::TODOS
                .iter()
                .map(|&r| Casilla {
                    rol: r,
                    permitido: ajustes.puede(r, c),
                    defecto: r.puede(c),
                    apartado: guardadas.contains(&(c, r)),
                    configurable: c.configurable_por_institucion()
                        && r.configurable_por_institucion(),
                })
                .collect(),
        })
        .collect();

    Matriz {
        institucion_id,
        capacidades,
    }
}

/// `GET /api/capacidades` — la matriz entera, resuelta.
///
/// Se devuelve COMPLETA y no solo las excepciones: la pantalla tiene que pintar
/// las casillas que siguen el defecto igual que las apartadas, y hacer que el
/// navegador recalcule el defecto por su cuenta sería tener la regla escrita en
/// dos sitios — que es cómo se desincronizan.
pub async fn obtener(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarUsuarios>,
    Query(q): Query<DeQuien>,
) -> Result<Json<Matriz>, Response> {
    let u = autorizado.0;
    let institucion_id = institucion_de(&u, q.institucion_id).map_err(IntoResponse::into_response)?;
    let ajustes = e
        .capacidades
        .ajustes(&e.pool, institucion_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Json(construir(institucion_id, &ajustes)))
}

/// Un cambio pedido por la pantalla.
#[derive(Debug, Deserialize)]
pub struct Cambio {
    pub capacidad: Capacidad,
    pub rol: RolUsuario,
    /// `Some(true/false)` fija la casilla; `None` la devuelve al defecto
    /// borrando la excepción.
    ///
    /// Son operaciones distintas y no un booleano con tres valores por pereza:
    /// marcar una casilla con el valor que hoy tiene el defecto la CONGELA ahí,
    /// y devolverla al defecto la deja moverse si el defecto cambia. Un cliente
    /// que quiera lo primero debe poder pedirlo.
    pub permitido: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Peticion {
    pub cambios: Vec<Cambio>,
    /// De qué institución, para el superadmin.
    pub institucion_id: Option<i64>,
}

/// `PUT /api/capacidades` — aplica un lote de cambios.
///
/// **Todo o nada.** Se valida el lote ENTERO contra el estado resultante antes
/// de escribir nada: quitar `GestionarUsuarios` a `Admin` es inocuo si el mismo
/// lote se lo da a `Medico`, y fatal si no. Validar cambio a cambio haría que el
/// orden de las casillas decidiera si la petición pasa, y ese orden no lo elige
/// el administrador.
pub async fn actualizar(
    State(e): State<Estado>,
    autorizado: Autorizado<GestionarUsuarios>,
    Json(p): Json<Peticion>,
) -> Result<Json<Matriz>, Response> {
    let u = autorizado.0;
    let institucion_id = institucion_de(&u, p.institucion_id).map_err(IntoResponse::into_response)?;
    let Some(quien) = u.rol() else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };

    let ajustes = e
        .capacidades
        .ajustes(&e.pool, institucion_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    // Los que fijan un valor y los que vuelven al defecto se separan aquí, pero
    // se VALIDAN juntos: un lote que quita a Admin y a la vez devuelve al
    // defecto la casilla de Medico tiene que juzgarse por lo que queda al final.
    let mut fijar: Vec<(Capacidad, RolUsuario, bool)> = Vec::new();
    let mut al_defecto: Vec<(Capacidad, RolUsuario)> = Vec::new();
    for c in &p.cambios {
        match c.permitido {
            Some(v) => fijar.push((c.capacidad, c.rol, v)),
            None => al_defecto.push((c.capacidad, c.rol)),
        }
    }

    // El estado resultante: primero se sueltan las que vuelven al defecto (lo
    // que equivale a fijarlas al valor del defecto SOLO a efectos de validar),
    // luego se aplican las fijadas.
    let mut propuesta: Vec<(Capacidad, RolUsuario, bool)> = al_defecto
        .iter()
        .map(|&(c, r)| (c, r, r.puede(c)))
        .collect();
    propuesta.extend(fijar.iter().copied());

    // Se valida por el rechazo, no por el resultado: lo que se guarda son los
    // cambios, y el estado se relee al final. Quedarse con el `Ajustes` de aquí
    // sería una tercera copia de la verdad que podría diferir de la base.
    let _ = ajustes
        .validar(&propuesta, quien)
        .map_err(|r| (StatusCode::UNPROCESSABLE_ENTITY, r.to_string()).into_response())?;

    // La bitácora ANTES de aplicar y **de forma obligatoria**, al revés que el
    // acceso a dato de paciente, que es best-effort porque allí manda la
    // atención (decisión de Juan, 2026-07-29). Aquí no hay paciente esperando:
    // esto es un acto administrativo que amplía o recorta quién puede tocar
    // datos clínicos, y un cambio de autorización sin rastro es justo lo que la
    // bitácora existe para impedir. Si no se puede registrar, no se aplica.
    registrar_cambio(&e, &u, institucion_id, &p.cambios)
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "no se pudo registrar el cambio en la bitácora: no se aplicó",
            )
                .into_response()
        })?;

    if !fijar.is_empty() {
        capacidad::guardar(&e.pool, institucion_id, &fijar, u.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    }
    for (c, r) in al_defecto {
        capacidad::devolver_al_defecto(&e.pool, institucion_id, c, r)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    }

    // Sin esto, el cambio queda escrito y NO se aplica hasta reiniciar.
    e.capacidades.invalidar(institucion_id);

    // Se devuelve lo que hay ahora, releído: que la pantalla no tenga que
    // adivinar el resultado de su propio envío.
    let ahora = e
        .capacidades
        .ajustes(&e.pool, institucion_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Json(construir(institucion_id, &ahora)))
}

/// Anota el cambio en la bitácora encadenada y firmada.
///
/// Va como COLECCIÓN y no como recurso único porque no hay UN paciente: lo que
/// cambia es quién puede llegar a los datos de todos. El `alcance` lleva el
/// detalle de las casillas para que el eslabón diga qué se amplió, no solo que
/// algo se tocó.
async fn registrar_cambio(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    institucion_id: i64,
    cambios: &[Cambio],
) -> Result<(), sqlx::Error> {
    let detalle = cambios
        .iter()
        .map(|c| {
            format!(
                "{}:{}={}",
                c.capacidad.como_str(),
                c.rol.como_str(),
                match c.permitido {
                    Some(true) => "si",
                    Some(false) => "no",
                    None => "defecto",
                }
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let evento = medico_datos::acceso::Evento {
        usuario_id: u.id,
        rol: u.rol.clone(),
        institucion_id: u.institucion_id,
        accion: "configurar".into(),
        recurso: "capacidad_rol".into(),
        objeto: medico_datos::acceso::Objeto::Coleccion {
            alcance: medico_datos::acceso::Alcance {
                ambito: medico_datos::acceso::describir_ambito(
                    medico_dominio::Ambito::institucion(institucion_id),
                ),
                filtro: Some(detalle),
                n: cambios.len() as i64,
                pacientes: vec![],
            },
        },
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    medico_datos::acceso::registrar(&e.pool, &evento, e.clave_firma.as_deref()).await
}
