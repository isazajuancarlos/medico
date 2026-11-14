//! Parque de ambulancias y traslados asistenciales.
//!
//! Módulo nuevo: no lo hereda del Visual FoxPro ni del sistema Python. Lo que
//! sostiene sus reglas es el marco real de un prestador colombiano — TAB y TAM
//! como las dos modalidades del transporte asistencial, traslado primario y
//! secundario, SOAT obligatorio — y no una traducción de código anterior.
//!
//! El parque es de la institución; los traslados cuelgan de la ambulancia, y por
//! ahí se aíslan. El paciente tiene que ser de la misma institución: un traslado
//! con la ambulancia de una y el paciente de otra no es un caso inusual, es un
//! registro que no puede ser cierto.
//!
//! # Las tres negativas al despachar
//!
//! Están aquí y no en el manejador HTTP porque despachar es la operación con
//! consecuencias del módulo, y la comprobación que se escribe en la ruta es la
//! que falta en la ruta siguiente. Cada una tiene su variante, con el dato que
//! el usuario necesita para resolverla: qué traslado la tiene ocupada, en qué
//! estado está, cuándo venció el SOAT.

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

pub const TIPOS: [&str; 2] = ["TAB", "TAM"];
pub const ESTADOS: [&str; 4] = ["Disponible", "En Traslado", "En Mantenimiento", "Fuera de Servicio"];
pub const TIPOS_TRASLADO: [&str; 2] = ["Primario", "Secundario"];

/// Días de antelación con que se avisa de un vencimiento. Un mes es lo que tarda
/// en tramitarse una renovación sin que el vehículo pare.
const DIAS_AVISO: i64 = 30;

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Ambulancia {
    pub id: i64,
    pub placa: String,
    pub tipo: String,
    pub numero_interno: Option<String>,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub estado: String,
    pub vence_soat: Option<String>,
    pub vence_tecnomecanica: Option<String>,
    pub observaciones: Option<String>,
    pub activa: bool,
    /// El SOAT venció. Es un hecho, no una sospecha: impide despachar.
    pub soat_vencido: bool,
    /// Vence dentro del mes. Avisa; no impide nada.
    pub soat_por_vencer: bool,
    /// **No está registrado.** No significa que esté al día — significa que el
    /// sistema no puede afirmar nada. Se muestra; no bloquea el despacho, porque
    /// un campo sin llenar no puede convertirse en una demora asistencial.
    pub soat_sin_registrar: bool,
    pub tecnomecanica_vencida: bool,
    /// El traslado que la tiene ocupada, si lo hay.
    pub traslado_en_curso: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AmbulanciaNueva {
    pub placa: String,
    pub tipo: Option<String>,
    pub numero_interno: Option<String>,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub vence_soat: Option<String>,
    pub vence_tecnomecanica: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AmbulanciaEdicion {
    pub placa: Option<String>,
    pub tipo: Option<String>,
    pub numero_interno: Option<String>,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub estado: Option<String>,
    pub vence_soat: Option<String>,
    pub vence_tecnomecanica: Option<String>,
    pub observaciones: Option<String>,
    pub activa: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Traslado {
    pub id: i64,
    pub ambulancia_id: i64,
    pub ambulancia_placa: Option<String>,
    pub ambulancia_tipo: Option<String>,
    pub paciente_id: i64,
    pub paciente_nombre: Option<String>,
    pub paciente_identificacion: Option<String>,
    pub remision_id: Option<i64>,
    pub tipo_traslado: String,
    pub origen: String,
    pub destino: String,
    pub motivo: String,
    pub estado: String,
    pub fecha_salida: String,
    pub fecha_llegada: Option<String>,
    pub conductor: Option<String>,
    pub personal_asistencial: Option<String>,
    pub registrado_por_id: i64,
    pub registrado_por_nombre: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrasladoNuevo {
    pub ambulancia_id: i64,
    pub paciente_id: i64,
    pub remision_id: Option<i64>,
    pub tipo_traslado: Option<String>,
    pub origen: String,
    pub destino: String,
    pub motivo: String,
    pub conductor: Option<String>,
    pub personal_asistencial: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Cierre {
    /// Cuándo llegó. Sin valor, ahora. No puede ser anterior a la salida.
    pub fecha_llegada: Option<String>,
    pub observaciones: Option<String>,
}

/// Qué pasó al dar de alta o editar una ambulancia.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoAlta {
    Hecho(Box<Ambulancia>),
    NoExiste,
    SinPlaca,
    TipoInvalido,
    EstadoInvalido,
    PlacaRepetida(String),
}

/// Por qué esta ambulancia **no puede salir ahora mismo**, o `None` si puede.
///
/// # Por qué es una función y no un `if` dentro de `despachar`
///
/// Porque la regla ya estaba escrita DOS veces. Una aquí, en `despachar`; otra
/// en el frontal, cuyo propio comentario lo admitía: *«Las que el servidor
/// dejaría salir ahora mismo. Es la misma regla que aplica `ambulancia::despachar`,
/// escrita aquí para no ofrecer lo que va a fallar»*. Ofrecer solo lo que puede
/// salir es correcto —hacer que el usuario descubra la regla chocándose con un
/// 409 es enseñársela dos veces—, pero la copia se queda atrás en el primer
/// arreglo, y entonces la pantalla ofrece lo que el servidor niega.
///
/// Con esto la pantalla pregunta en vez de deducir, y de paso puede DECIR por
/// qué no sale cada una en vez de esconderla.
///
/// **El orden es parte de la regla**: primero ocupada, después no disponible y
/// por último el SOAT. Una ambulancia dada de baja que además tuviera el SOAT
/// vencido debe decir lo primero, que es lo que se puede resolver.
///
/// **Que el SOAT no esté REGISTRADO no impide salir**, y ésa es la decisión que
/// más se presta a estropearse: no saber no es estar vencido, y un campo sin
/// llenar no puede convertirse en una demora asistencial.
#[derive(Debug, Clone, PartialEq)]
pub enum NoSale {
    Ocupada { traslado_id: i64 },
    NoDisponible { estado: String },
    SoatVencido { vencio: String },
}

pub fn motivo_para_no_despachar(a: &Ambulancia) -> Option<NoSale> {
    if let Some(traslado_id) = a.traslado_en_curso {
        return Some(NoSale::Ocupada { traslado_id });
    }
    if !a.activa || a.estado != "Disponible" {
        let estado = if a.activa { a.estado.clone() } else { "Dada de baja".to_string() };
        return Some(NoSale::NoDisponible { estado });
    }
    if a.soat_vencido {
        return Some(NoSale::SoatVencido {
            vencio: a.vence_soat.clone().unwrap_or_default(),
        });
    }
    None
}

/// Qué pasó al despachar. Cada negativa lleva el dato que hace falta para
/// resolverla; «no se pudo» no es una respuesta que sirva a quien coordina.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoDespacho {
    Hecho(Box<Traslado>),
    /// La ambulancia o el paciente no existen, o no son de esta institución.
    NoExiste,
    /// Ya está en un traslado sin cerrar.
    Ocupada { traslado_id: i64 },
    /// En mantenimiento, fuera de servicio o dada de baja.
    NoDisponible { estado: String },
    /// El SOAT venció. No sale.
    SoatVencido { vencio: String },
    TipoInvalido,
}

/// Qué pasó al cerrar un traslado.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoCierre {
    Hecho(Box<Traslado>),
    NoExiste,
    /// Ya está finalizado o cancelado.
    YaCerrado { estado: String },
    /// La llegada es anterior a la salida. Un traslado que termina antes de
    /// empezar no es un dato raro: es uno imposible.
    LlegadaAntesDeSalida { salida: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoBorrado {
    Hecho,
    NoExiste,
    /// Tiene traslados registrados: se da de baja, no se borra.
    TieneTraslados { traslados: i64 },
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltrosAmbulancia {
    pub q: Option<String>,
    pub estado: Option<String>,
    pub tipo: Option<String>,
    /// Sin decir nada se devuelven todas. La pantalla de despacho pide `true`.
    pub activa: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltrosTraslado {
    pub estado: Option<String>,
    pub ambulancia_id: Option<i64>,
    pub paciente_id: Option<i64>,
}

use crate::{ahora, filtro_ambito, hoy};



fn limite_aviso(hoy: &str) -> String {
    match chrono::NaiveDate::parse_from_str(hoy, "%Y-%m-%d") {
        Ok(d) => (d + chrono::Duration::days(DIAS_AVISO)).to_string(),
        Err(_) => hoy.to_string(),
    }
}

/// Columnas de la ambulancia más las derivadas. Los `?` toman, en orden:
/// `hoy` (vencido), `hoy` y `aviso` (por vencer), `hoy` (tecnomecánica).
const COLS_AMB: &str = "a.id, a.placa, a.tipo, a.numero_interno, a.marca, a.modelo, a.estado, \
    a.vence_soat, a.vence_tecnomecanica, a.observaciones, a.activa, \
    (a.vence_soat IS NOT NULL AND a.vence_soat < ?) AS soat_vencido, \
    (a.vence_soat IS NOT NULL AND a.vence_soat >= ? AND a.vence_soat <= ?) AS soat_por_vencer, \
    (a.vence_soat IS NULL) AS soat_sin_registrar, \
    (a.vence_tecnomecanica IS NOT NULL AND a.vence_tecnomecanica < ?) AS tecnomecanica_vencida, \
    (SELECT t.id FROM traslado t WHERE t.ambulancia_id = a.id AND t.estado = 'En Curso') AS traslado_en_curso";

fn enlazar_fechas<'a, T>(
    q: sqlx::query::QueryAs<'a, sqlx::Sqlite, T, sqlx::sqlite::SqliteArguments<'a>>,
    hoy: &str,
    aviso: &str,
) -> sqlx::query::QueryAs<'a, sqlx::Sqlite, T, sqlx::sqlite::SqliteArguments<'a>> {
    q.bind(hoy.to_owned())
        .bind(hoy.to_owned())
        .bind(aviso.to_owned())
        .bind(hoy.to_owned())
}

pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &FiltrosAmbulancia,
    hoy: &str,
) -> Result<Vec<Ambulancia>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "a.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!("SELECT {COLS_AMB} FROM ambulancia a WHERE {filtro}");
    if f.q.is_some() {
        sql.push_str(" AND (a.placa LIKE ? OR a.numero_interno LIKE ?)");
    }
    if f.estado.is_some() {
        sql.push_str(" AND a.estado = ?");
    }
    if f.tipo.is_some() {
        sql.push_str(" AND a.tipo = ?");
    }
    if f.activa.is_some() {
        sql.push_str(" AND a.activa = ?");
    }
    sql.push_str(" ORDER BY a.activa DESC, a.placa");

    let mut q = enlazar_fechas(sqlx::query_as::<_, Ambulancia>(&sql), hoy, &limite_aviso(hoy));
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(t) = &f.q {
        let patron = format!("%{t}%");
        q = q.bind(patron.clone()).bind(patron);
    }
    if let Some(e) = &f.estado {
        q = q.bind(e.clone());
    }
    if let Some(t) = &f.tipo {
        q = q.bind(t.clone());
    }
    if let Some(a) = f.activa {
        q = q.bind(a);
    }
    q.fetch_all(pool).await
}

pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    hoy: &str,
) -> Result<Option<Ambulancia>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "a.institucion_id") else {
        return Ok(None);
    };
    let sql = format!("SELECT {COLS_AMB} FROM ambulancia a WHERE a.id = ? AND {filtro}");
    let mut q = enlazar_fechas(sqlx::query_as::<_, Ambulancia>(&sql), hoy, &limite_aviso(hoy)).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    institucion_id: i64,
    a: &AmbulanciaNueva,
    hoy: &str,
) -> Result<ResultadoAlta, sqlx::Error> {
    if ambito.puede_ver(Some(institucion_id)) != Visibilidad::Si {
        return Ok(ResultadoAlta::NoExiste);
    }
    let placa = a.placa.trim().to_uppercase();
    if placa.is_empty() {
        return Ok(ResultadoAlta::SinPlaca);
    }
    let tipo = a.tipo.as_deref().unwrap_or("TAB");
    if !TIPOS.contains(&tipo) {
        return Ok(ResultadoAlta::TipoInvalido);
    }
    let r = sqlx::query(
        "INSERT INTO ambulancia \
         (institucion_id, placa, tipo, numero_interno, marca, modelo, vence_soat, \
          vence_tecnomecanica, observaciones) VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(&placa)
    .bind(tipo)
    .bind(&a.numero_interno)
    .bind(&a.marca)
    .bind(&a.modelo)
    .bind(&a.vence_soat)
    .bind(&a.vence_tecnomecanica)
    .bind(&a.observaciones)
    .execute(pool)
    .await;
    match r {
        Ok(r) => {
            let creada = obtener(pool, ambito, r.last_insert_rowid(), hoy).await?.expect("recién creada");
            Ok(ResultadoAlta::Hecho(Box::new(creada)))
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Ok(ResultadoAlta::PlacaRepetida(placa))
        }
        Err(e) => Err(e),
    }
}

/// Actualiza los campos provistos.
///
/// **`estado` se admite aquí, pero «En Traslado» no**: ese estado lo pone el
/// despacho y lo quita el cierre. Dejar que se fije a mano permitiría marcar
/// ocupada una ambulancia que no lo está —o libre una que sí—, y entonces el
/// estado dejaría de decir nada sobre dónde está el vehículo.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    e: &AmbulanciaEdicion,
    hoy: &str,
) -> Result<ResultadoAlta, sqlx::Error> {
    let Some(actual) = obtener(pool, ambito, id, hoy).await? else {
        return Ok(ResultadoAlta::NoExiste);
    };
    if let Some(t) = &e.tipo
        && !TIPOS.contains(&t.as_str())
    {
        return Ok(ResultadoAlta::TipoInvalido);
    }
    if let Some(nuevo) = &e.estado {
        if !ESTADOS.contains(&nuevo.as_str()) || nuevo == "En Traslado" {
            return Ok(ResultadoAlta::EstadoInvalido);
        }
        // Y tampoco se le quita el «En Traslado» a mano: se cierra el traslado.
        if actual.traslado_en_curso.is_some() {
            return Ok(ResultadoAlta::EstadoInvalido);
        }
    }
    let placa = match e.placa.as_deref().map(|p| p.trim().to_uppercase()) {
        Some(p) if p.is_empty() => return Ok(ResultadoAlta::SinPlaca),
        otra => otra,
    };
    let r = sqlx::query(
        "UPDATE ambulancia SET placa=COALESCE(?,placa), tipo=COALESCE(?,tipo), \
         numero_interno=COALESCE(?,numero_interno), marca=COALESCE(?,marca), \
         modelo=COALESCE(?,modelo), estado=COALESCE(?,estado), \
         vence_soat=COALESCE(?,vence_soat), vence_tecnomecanica=COALESCE(?,vence_tecnomecanica), \
         observaciones=COALESCE(?,observaciones), activa=COALESCE(?,activa) WHERE id = ?",
    )
    .bind(&placa)
    .bind(&e.tipo)
    .bind(&e.numero_interno)
    .bind(&e.marca)
    .bind(&e.modelo)
    .bind(&e.estado)
    .bind(&e.vence_soat)
    .bind(&e.vence_tecnomecanica)
    .bind(&e.observaciones)
    .bind(e.activa)
    .bind(id)
    .execute(pool)
    .await;
    match r {
        Ok(_) => {
            let a = obtener(pool, ambito, id, hoy).await?.expect("existe");
            Ok(ResultadoAlta::Hecho(Box::new(a)))
        }
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            Ok(ResultadoAlta::PlacaRepetida(placa.unwrap_or_default()))
        }
        Err(err) => Err(err),
    }
}

/// Borra una ambulancia **que no haya rodado**. La que tiene traslados se da de
/// baja (`activa = 0`): su historial es el respaldo de cada traslado facturado.
pub async fn eliminar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<ResultadoBorrado, sqlx::Error> {
    if obtener(pool, ambito, id, &hoy()).await?.is_none() {
        return Ok(ResultadoBorrado::NoExiste);
    }
    let traslados: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM traslado WHERE ambulancia_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if traslados > 0 {
        return Ok(ResultadoBorrado::TieneTraslados { traslados });
    }
    sqlx::query("DELETE FROM ambulancia WHERE id = ?").bind(id).execute(pool).await?;
    Ok(ResultadoBorrado::Hecho)
}

// --- Traslados ---------------------------------------------------------------

const COLS_TRA: &str = "t.id, t.ambulancia_id, a.placa AS ambulancia_placa, \
    a.tipo AS ambulancia_tipo, t.paciente_id, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente_nombre, \
    p.numero_documento AS paciente_identificacion, t.remision_id, t.tipo_traslado, t.origen, \
    t.destino, t.motivo, t.estado, t.fecha_salida, t.fecha_llegada, t.conductor, \
    t.personal_asistencial, t.registrado_por_id, u.nombre AS registrado_por_nombre, t.observaciones";

const DESDE_TRA: &str = "FROM traslado t \
    JOIN ambulancia a ON a.id = t.ambulancia_id \
    JOIN paciente p ON p.id = t.paciente_id \
    LEFT JOIN usuario u ON u.id = t.registrado_por_id";

pub async fn listar_traslados(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &FiltrosTraslado,
) -> Result<Vec<Traslado>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "a.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!("SELECT {COLS_TRA} {DESDE_TRA} WHERE {filtro}");
    if f.estado.is_some() {
        sql.push_str(" AND t.estado = ?");
    }
    if f.ambulancia_id.is_some() {
        sql.push_str(" AND t.ambulancia_id = ?");
    }
    if f.paciente_id.is_some() {
        sql.push_str(" AND t.paciente_id = ?");
    }
    // Lo que está rodando primero: es lo que hay que vigilar.
    sql.push_str(" ORDER BY CASE t.estado WHEN 'En Curso' THEN 0 ELSE 1 END, t.fecha_salida DESC, t.id DESC");

    let mut q = sqlx::query_as::<_, Traslado>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(e) = &f.estado {
        q = q.bind(e.clone());
    }
    if let Some(a) = f.ambulancia_id {
        q = q.bind(a);
    }
    if let Some(p) = f.paciente_id {
        q = q.bind(p);
    }
    q.fetch_all(pool).await
}

pub async fn obtener_traslado(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Traslado>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "a.institucion_id") else {
        return Ok(None);
    };
    let sql = format!("SELECT {COLS_TRA} {DESDE_TRA} WHERE t.id = ? AND {filtro}");
    let mut q = sqlx::query_as::<_, Traslado>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Despacha una ambulancia. Marca el vehículo «En Traslado» y abre el traslado,
/// en una transacción: un despacho a medias dejaría una ambulancia ocupada sin
/// nada que la libere, o un traslado sin vehículo asignado.
pub async fn despachar(
    pool: &crate::Pool,
    ambito: Ambito,
    usuario_id: i64,
    t: &TrasladoNuevo,
    hoy: &str,
) -> Result<ResultadoDespacho, sqlx::Error> {
    let tipo = t.tipo_traslado.as_deref().unwrap_or("Primario");
    if !TIPOS_TRASLADO.contains(&tipo) {
        return Ok(ResultadoDespacho::TipoInvalido);
    }
    let Some(amb) = obtener(pool, ambito, t.ambulancia_id, hoy).await? else {
        return Ok(ResultadoDespacho::NoExiste);
    };
    // El paciente, de la misma institución. Se resuelve por el ámbito, así que
    // uno ajeno es indistinguible de uno inexistente.
    let del_paciente: Option<(i64,)> =
        sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
            .bind(t.paciente_id)
            .fetch_optional(pool)
            .await?;
    if !del_paciente.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si) {
        return Ok(ResultadoDespacho::NoExiste);
    }
    // Ocupada, no disponible o con el SOAT vencido: la regla vive en
    // `motivo_para_no_despachar`, que es la MISMA que consulta la pantalla para
    // no ofrecer lo que va a fallar. Ver su cabecera.
    match motivo_para_no_despachar(&amb) {
        Some(NoSale::Ocupada { traslado_id }) => {
            return Ok(ResultadoDespacho::Ocupada { traslado_id })
        }
        Some(NoSale::NoDisponible { estado }) => {
            return Ok(ResultadoDespacho::NoDisponible { estado })
        }
        Some(NoSale::SoatVencido { vencio }) => {
            return Ok(ResultadoDespacho::SoatVencido { vencio })
        }
        None => {}
    }

    let mut tx = pool.begin().await?;
    // La condición vuelve a mirar el estado dentro de la transacción: entre la
    // lectura de arriba y esta escritura cabe otro despacho.
    let tomada = sqlx::query(
        "UPDATE ambulancia SET estado = 'En Traslado' WHERE id = ? AND estado = 'Disponible' AND activa = 1",
    )
    .bind(t.ambulancia_id)
    .execute(&mut *tx)
    .await?;
    if tomada.rows_affected() == 0 {
        tx.rollback().await?;
        let ahora_esta = obtener(pool, ambito, t.ambulancia_id, hoy).await?;
        return Ok(match ahora_esta.and_then(|a| a.traslado_en_curso) {
            Some(traslado_id) => ResultadoDespacho::Ocupada { traslado_id },
            None => ResultadoDespacho::NoDisponible { estado: "En Traslado".into() },
        });
    }
    let r = sqlx::query(
        "INSERT INTO traslado \
         (ambulancia_id, paciente_id, remision_id, tipo_traslado, origen, destino, motivo, \
          fecha_salida, conductor, personal_asistencial, registrado_por_id, observaciones) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(t.ambulancia_id)
    .bind(t.paciente_id)
    .bind(t.remision_id)
    .bind(tipo)
    .bind(&t.origen)
    .bind(&t.destino)
    .bind(&t.motivo)
    .bind(ahora())
    .bind(&t.conductor)
    .bind(&t.personal_asistencial)
    .bind(usuario_id)
    .bind(&t.observaciones)
    .execute(&mut *tx)
    .await;
    let id = match r {
        Ok(r) => r.last_insert_rowid(),
        // El índice único parcial: otra transacción abrió el traslado primero.
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            tx.rollback().await?;
            let en_curso = obtener(pool, ambito, t.ambulancia_id, hoy).await?
                .and_then(|a| a.traslado_en_curso)
                .unwrap_or_default();
            return Ok(ResultadoDespacho::Ocupada { traslado_id: en_curso });
        }
        Err(e) => {
            tx.rollback().await?;
            return Err(e);
        }
    };
    tx.commit().await?;
    let creado = obtener_traslado(pool, ambito, id).await?.expect("recién creado");
    Ok(ResultadoDespacho::Hecho(Box::new(creado)))
}

/// Cierra un traslado y libera la ambulancia. Las dos cosas, o ninguna.
pub async fn cerrar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &Cierre,
    cancelado: bool,
) -> Result<ResultadoCierre, sqlx::Error> {
    let Some(actual) = obtener_traslado(pool, ambito, id).await? else {
        return Ok(ResultadoCierre::NoExiste);
    };
    if actual.estado != "En Curso" {
        return Ok(ResultadoCierre::YaCerrado { estado: actual.estado });
    }
    let llegada = c.fecha_llegada.clone().unwrap_or_else(ahora);
    // Comparación de cadenas: las dos son RFC 3339 en UTC, donde el orden
    // lexicográfico es el cronológico. Un traslado que llega antes de salir no es
    // un dato raro, es uno imposible, y no se guarda «tal como lo mandaron».
    if llegada < actual.fecha_salida {
        return Ok(ResultadoCierre::LlegadaAntesDeSalida { salida: actual.fecha_salida });
    }

    let mut tx = pool.begin().await?;
    let estado = if cancelado { "Cancelado" } else { "Finalizado" };
    let cerrado = sqlx::query(
        "UPDATE traslado SET estado = ?, fecha_llegada = ?, \
         observaciones = COALESCE(?, observaciones) WHERE id = ? AND estado = 'En Curso'",
    )
    .bind(estado)
    // Un traslado CANCELADO no llegó a ninguna parte: dejarle hora de llegada
    // diría que el paciente fue trasladado, que es justo lo que no pasó.
    .bind(if cancelado { None } else { Some(&llegada) })
    .bind(&c.observaciones)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if cerrado.rows_affected() == 0 {
        tx.rollback().await?;
        let ahora_esta = obtener_traslado(pool, ambito, id).await?;
        return Ok(ResultadoCierre::YaCerrado {
            estado: ahora_esta.map(|t| t.estado).unwrap_or_else(|| "Finalizado".into()),
        });
    }
    // La ambulancia vuelve a estar disponible. `AND estado = 'En Traslado'` para
    // no sacar de mantenimiento a una que alguien movió mientras tanto.
    sqlx::query("UPDATE ambulancia SET estado = 'Disponible' WHERE id = ? AND estado = 'En Traslado'")
        .bind(actual.ambulancia_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let final_ = obtener_traslado(pool, ambito, id).await?.expect("existe");
    Ok(ResultadoCierre::Hecho(Box::new(final_)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const HOY: &str = "2026-08-01";

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES \
             (5,'Coordinadora','c@x.co','h','enfermeria',1,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1017','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2028','Beto','Paz','1985-01-01','M',2)",
        ).execute(&pool).await.unwrap();
        pool
    }

    fn nueva(placa: &str) -> AmbulanciaNueva {
        AmbulanciaNueva {
            placa: placa.into(),
            tipo: Some("TAB".into()),
            numero_interno: Some("A-01".into()),
            marca: Some("Renault".into()),
            modelo: Some("2022".into()),
            vence_soat: Some("2027-01-31".into()),
            vence_tecnomecanica: Some("2027-01-31".into()),
            observaciones: None,
        }
    }

    fn traslado(ambulancia_id: i64) -> TrasladoNuevo {
        TrasladoNuevo {
            ambulancia_id,
            paciente_id: 10,
            remision_id: None,
            tipo_traslado: Some("Primario".into()),
            origen: "Vía Las Palmas km 12".into(),
            destino: "Urgencias sede norte".into(),
            motivo: "Accidente de tránsito".into(),
            conductor: Some("J. Muñoz".into()),
            personal_asistencial: Some("Aux. Torres".into()),
            observaciones: None,
        }
    }

    async fn crear_amb(pool: &crate::Pool, placa: &str) -> i64 {
        let ResultadoAlta::Hecho(a) = crear(pool, Ambito::institucion(1), 1, &nueva(placa), HOY).await.unwrap()
        else { panic!("debía crearse") };
        a.id
    }

    #[tokio::test]
    async fn crear_y_listar_normaliza_la_placa_y_calcula_los_avisos() {
        let pool = base().await;
        let ResultadoAlta::Hecho(a) = crear(&pool, Ambito::institucion(1), 1, &nueva(" wxy123 "), HOY).await.unwrap()
        else { panic!() };
        assert_eq!(a.placa, "WXY123", "la placa se normaliza: sin espacios y en mayúsculas");
        assert!(!a.soat_vencido);
        assert!(!a.soat_por_vencer);
        assert!(!a.soat_sin_registrar);
        assert_eq!(a.estado, "Disponible");
        assert_eq!(a.traslado_en_curso, None);
        assert_eq!(listar(&pool, Ambito::institucion(1), &FiltrosAmbulancia::default(), HOY).await.unwrap().len(), 1);
    }

    /// **`motivo_para_no_despachar` decide igual que `despachar`, y en el mismo
    /// ORDEN.**
    ///
    /// La regla se extrajo el 2026-08-08 porque estaba escrita dos veces —aquí y
    /// en el frontal—, y la pantalla ahora la consulta en vez de repetirla. Esta
    /// prueba mira la función a solas, con **los cuatro finales**: si solo
    /// comprobara los tres motivos, una versión que dijera «no sale» a todas
    /// pasaría igual.
    ///
    /// Se construye la `Ambulancia` a mano a propósito: es un valor, y la regla
    /// no toca la base. La prueba de que esa estructura la produce el SQL es
    /// `el_soat_distingue_vencido_por_vencer_y_sin_registrar`.
    #[test]
    fn el_motivo_para_no_despachar_decide_igual_y_en_el_mismo_orden() {
        let limpia = Ambulancia {
            id: 1,
            placa: "AAA111".into(),
            tipo: "TAB".into(),
            numero_interno: None,
            marca: None,
            modelo: None,
            estado: "Disponible".into(),
            vence_soat: Some("2027-01-01".into()),
            vence_tecnomecanica: None,
            observaciones: None,
            activa: true,
            soat_vencido: false,
            soat_por_vencer: false,
            soat_sin_registrar: false,
            tecnomecanica_vencida: false,
            traslado_en_curso: None,
        };

        // El positivo: una ambulancia en regla SALE.
        assert_eq!(motivo_para_no_despachar(&limpia), None);

        // **Sin SOAT registrado también sale**, que es la decisión que más se
        // presta a estropearse: no saber no es estar vencido, y un campo sin
        // llenar no puede volverse una demora asistencial.
        let sin_soat = Ambulancia {
            vence_soat: None,
            soat_sin_registrar: true,
            ..limpia.clone()
        };
        assert_eq!(
            motivo_para_no_despachar(&sin_soat),
            None,
            "un campo sin llenar dejó en tierra una ambulancia"
        );

        let ocupada = Ambulancia { traslado_en_curso: Some(7), ..limpia.clone() };
        assert_eq!(
            motivo_para_no_despachar(&ocupada),
            Some(NoSale::Ocupada { traslado_id: 7 })
        );

        let de_baja = Ambulancia { activa: false, ..limpia.clone() };
        assert_eq!(
            motivo_para_no_despachar(&de_baja),
            Some(NoSale::NoDisponible { estado: "Dada de baja".into() }),
            "una dada de baja no dice su estado anterior, dice que está de baja"
        );

        let en_mantenimiento =
            Ambulancia { estado: "En Mantenimiento".into(), ..limpia.clone() };
        assert_eq!(
            motivo_para_no_despachar(&en_mantenimiento),
            Some(NoSale::NoDisponible { estado: "En Mantenimiento".into() })
        );

        let caducada = Ambulancia {
            vence_soat: Some("2020-01-01".into()),
            soat_vencido: true,
            ..limpia.clone()
        };
        assert_eq!(
            motivo_para_no_despachar(&caducada),
            Some(NoSale::SoatVencido { vencio: "2020-01-01".into() })
        );

        // **El ORDEN es parte de la regla**: la que está ocupada Y además tiene
        // el SOAT vencido dice lo primero, que es lo que se puede resolver
        // cerrando un traslado.
        let las_dos = Ambulancia {
            traslado_en_curso: Some(7),
            soat_vencido: true,
            activa: false,
            ..limpia
        };
        assert_eq!(
            motivo_para_no_despachar(&las_dos),
            Some(NoSale::Ocupada { traslado_id: 7 }),
            "se contestó el motivo que no se puede resolver ahora"
        );
    }

    /// «Sin registrar» no es «al día», y las tres respuestas son distintas.
    #[tokio::test]
    async fn el_soat_distingue_vencido_por_vencer_y_sin_registrar() {
        let pool = base().await;
        let mut sin = nueva("AAA111");
        sin.vence_soat = None;
        let ResultadoAlta::Hecho(a) = crear(&pool, Ambito::institucion(1), 1, &sin, HOY).await.unwrap() else { panic!() };
        assert!(a.soat_sin_registrar);
        assert!(!a.soat_vencido, "no saber no es lo mismo que estar vencido");
        assert!(!a.soat_por_vencer);

        let mut pronto = nueva("BBB222");
        pronto.vence_soat = Some("2026-08-20".into()); // a 19 días
        let ResultadoAlta::Hecho(b) = crear(&pool, Ambito::institucion(1), 1, &pronto, HOY).await.unwrap() else { panic!() };
        assert!(b.soat_por_vencer);
        assert!(!b.soat_vencido);

        let mut caducado = nueva("CCC333");
        caducado.vence_soat = Some("2026-07-31".into()); // ayer
        let ResultadoAlta::Hecho(c) = crear(&pool, Ambito::institucion(1), 1, &caducado, HOY).await.unwrap() else { panic!() };
        assert!(c.soat_vencido);
        assert!(!c.soat_por_vencer, "lo vencido no cuenta además como por vencer");
    }

    #[tokio::test]
    async fn el_parque_ajeno_es_indistinguible_del_inexistente() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;
        assert!(listar(&pool, Ambito::institucion(2), &FiltrosAmbulancia::default(), HOY).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::institucion(2), id, HOY).await.unwrap().is_none());
        assert_eq!(eliminar(&pool, Ambito::institucion(2), id).await.unwrap(), ResultadoBorrado::NoExiste);
        assert_eq!(
            despachar(&pool, Ambito::institucion(2), 5, &traslado(id), HOY).await.unwrap(),
            ResultadoDespacho::NoExiste,
            "no se despacha la ambulancia de otra institución"
        );
        assert_eq!(crear(&pool, Ambito::institucion(1), 2, &nueva("ZZZ999"), HOY).await.unwrap(), ResultadoAlta::NoExiste);
        // Y la misma placa en dos instituciones no choca; en la misma, sí.
        assert!(matches!(crear(&pool, Ambito::institucion(2), 2, &nueva("WXY123"), HOY).await.unwrap(), ResultadoAlta::Hecho(_)));
        assert_eq!(
            crear(&pool, Ambito::institucion(1), 1, &nueva("wxy123"), HOY).await.unwrap(),
            ResultadoAlta::PlacaRepetida("WXY123".into())
        );
    }

    #[tokio::test]
    async fn despachar_ocupa_la_ambulancia_y_cerrar_la_libera() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;
        let ResultadoDespacho::Hecho(t) = despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap()
        else { panic!("debía despacharse") };
        assert_eq!(t.estado, "En Curso");
        assert_eq!(t.ambulancia_placa.as_deref(), Some("WXY123"));
        assert_eq!(t.paciente_nombre.as_deref(), Some("Ana Ruiz"));
        assert_eq!(t.registrado_por_nombre.as_deref(), Some("Coordinadora"));
        assert!(t.fecha_llegada.is_none(), "en curso no tiene hora de llegada");

        let a = obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap();
        assert_eq!(a.estado, "En Traslado");
        assert_eq!(a.traslado_en_curso, Some(t.id));

        // Con una en curso, no se despacha otra vez.
        assert_eq!(
            despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap(),
            ResultadoDespacho::Ocupada { traslado_id: t.id }
        );

        let ResultadoCierre::Hecho(cerrado) = cerrar(&pool, Ambito::institucion(1), t.id, &Cierre::default(), false).await.unwrap()
        else { panic!("debía cerrarse") };
        assert_eq!(cerrado.estado, "Finalizado");
        assert!(cerrado.fecha_llegada.is_some());
        let a = obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap();
        assert_eq!(a.estado, "Disponible");
        assert_eq!(a.traslado_en_curso, None);
        // Y ya no se cierra dos veces.
        assert_eq!(
            cerrar(&pool, Ambito::institucion(1), t.id, &Cierre::default(), false).await.unwrap(),
            ResultadoCierre::YaCerrado { estado: "Finalizado".into() }
        );
    }

    /// Las cuatro negativas del despacho, cada una con su motivo, y ninguna deja
    /// rastro. Y DISCRIMINA: la misma llamada con la ambulancia en orden sale.
    #[tokio::test]
    async fn despachar_discrimina_los_motivos_de_rechazo() {
        let pool = base().await;
        // SOAT vencido.
        let mut caducada = nueva("CCC333");
        caducada.vence_soat = Some("2026-07-31".into());
        let ResultadoAlta::Hecho(c) = crear(&pool, Ambito::institucion(1), 1, &caducada, HOY).await.unwrap() else { panic!() };
        assert_eq!(
            despachar(&pool, Ambito::institucion(1), 5, &traslado(c.id), HOY).await.unwrap(),
            ResultadoDespacho::SoatVencido { vencio: "2026-07-31".into() }
        );
        // ...y el día del vencimiento todavía sale: la póliza vale ese día.
        assert!(matches!(
            despachar(&pool, Ambito::institucion(1), 5, &traslado(c.id), "2026-07-31").await.unwrap(),
            ResultadoDespacho::Hecho(_)
        ));

        // En mantenimiento.
        let m = crear_amb(&pool, "MMM111").await;
        actualizar(&pool, Ambito::institucion(1), m, &AmbulanciaEdicion {
            estado: Some("En Mantenimiento".into()), ..Default::default()
        }, HOY).await.unwrap();
        assert_eq!(
            despachar(&pool, Ambito::institucion(1), 5, &traslado(m), HOY).await.unwrap(),
            ResultadoDespacho::NoDisponible { estado: "En Mantenimiento".into() }
        );

        // Paciente de otra institución.
        let b = crear_amb(&pool, "BBB222").await;
        let mut ajeno = traslado(b);
        ajeno.paciente_id = 20;
        assert_eq!(
            despachar(&pool, Ambito::institucion(1), 5, &ajeno, HOY).await.unwrap(),
            ResultadoDespacho::NoExiste
        );
        // Tipo de traslado inventado.
        let mut raro = traslado(b);
        raro.tipo_traslado = Some("Terciario".into());
        assert_eq!(
            despachar(&pool, Ambito::institucion(1), 5, &raro, HOY).await.unwrap(),
            ResultadoDespacho::TipoInvalido
        );

        // Nada de eso escribió un traslado con esa ambulancia…
        let de_b = listar_traslados(&pool, Ambito::institucion(1), &FiltrosTraslado {
            ambulancia_id: Some(b), ..Default::default()
        }).await.unwrap();
        assert!(de_b.is_empty());
        // …y la misma llamada, ya correcta, sí sale.
        assert!(matches!(
            despachar(&pool, Ambito::institucion(1), 5, &traslado(b), HOY).await.unwrap(),
            ResultadoDespacho::Hecho(_)
        ));
    }

    #[tokio::test]
    async fn un_traslado_no_puede_llegar_antes_de_salir() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;
        let ResultadoDespacho::Hecho(t) = despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap()
        else { panic!() };
        let imposible = Cierre { fecha_llegada: Some("2020-01-01T00:00:00Z".into()), observaciones: None };
        assert_eq!(
            cerrar(&pool, Ambito::institucion(1), t.id, &imposible, false).await.unwrap(),
            ResultadoCierre::LlegadaAntesDeSalida { salida: t.fecha_salida.clone() }
        );
        // Sigue en curso y la ambulancia sigue ocupada: el rechazo no cerró nada.
        assert_eq!(obtener_traslado(&pool, Ambito::institucion(1), t.id).await.unwrap().unwrap().estado, "En Curso");
        assert_eq!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().estado, "En Traslado");
    }

    /// Un traslado CANCELADO libera la ambulancia pero **no** anota llegada: el
    /// paciente no fue trasladado a ninguna parte.
    #[tokio::test]
    async fn cancelar_libera_la_ambulancia_y_no_inventa_una_llegada() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;
        let ResultadoDespacho::Hecho(t) = despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap()
        else { panic!() };
        let ResultadoCierre::Hecho(c) = cerrar(&pool, Ambito::institucion(1), t.id, &Cierre {
            fecha_llegada: None, observaciones: Some("El paciente fue trasladado por un particular".into()),
        }, true).await.unwrap() else { panic!() };
        assert_eq!(c.estado, "Cancelado");
        assert!(c.fecha_llegada.is_none(), "un traslado cancelado no llegó a ninguna parte");
        assert_eq!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().estado, "Disponible");
    }

    /// El estado «En Traslado» lo pone el despacho, no la edición. Ni se pone ni
    /// se quita a mano: si se pudiera, el estado dejaría de decir dónde está el
    /// vehículo.
    #[tokio::test]
    async fn el_estado_en_traslado_no_se_edita_a_mano() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;
        assert_eq!(
            actualizar(&pool, Ambito::institucion(1), id, &AmbulanciaEdicion {
                estado: Some("En Traslado".into()), ..Default::default()
            }, HOY).await.unwrap(),
            ResultadoAlta::EstadoInvalido
        );
        despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap();
        // Y con una en curso tampoco se le pone otro estado.
        assert_eq!(
            actualizar(&pool, Ambito::institucion(1), id, &AmbulanciaEdicion {
                estado: Some("Disponible".into()), ..Default::default()
            }, HOY).await.unwrap(),
            ResultadoAlta::EstadoInvalido
        );
        // Pero editar OTRA cosa sigue funcionando.
        assert!(matches!(
            actualizar(&pool, Ambito::institucion(1), id, &AmbulanciaEdicion {
                observaciones: Some("Cambio de llantas pendiente".into()), ..Default::default()
            }, HOY).await.unwrap(),
            ResultadoAlta::Hecho(_)
        ));
    }

    #[tokio::test]
    async fn la_ambulancia_que_ya_rodo_se_da_de_baja_pero_no_se_borra() {
        let pool = base().await;
        let sin_uso = crear_amb(&pool, "AAA111").await;
        assert_eq!(eliminar(&pool, Ambito::institucion(1), sin_uso).await.unwrap(), ResultadoBorrado::Hecho);

        let id = crear_amb(&pool, "WXY123").await;
        let ResultadoDespacho::Hecho(t) = despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap()
        else { panic!() };
        cerrar(&pool, Ambito::institucion(1), t.id, &Cierre::default(), false).await.unwrap();
        assert_eq!(
            eliminar(&pool, Ambito::institucion(1), id).await.unwrap(),
            ResultadoBorrado::TieneTraslados { traslados: 1 }
        );
        // Darla de baja sí, y entonces no se despacha.
        actualizar(&pool, Ambito::institucion(1), id, &AmbulanciaEdicion {
            activa: Some(false), ..Default::default()
        }, HOY).await.unwrap();
        assert_eq!(
            despachar(&pool, Ambito::institucion(1), 5, &traslado(id), HOY).await.unwrap(),
            ResultadoDespacho::NoDisponible { estado: "Dada de baja".into() }
        );
    }

    #[tokio::test]
    async fn sin_ambito_no_se_ve_ni_se_despacha() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;
        assert!(listar(&pool, Ambito::Ninguno, &FiltrosAmbulancia::default(), HOY).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::Ninguno, id, HOY).await.unwrap().is_none());
        assert!(listar_traslados(&pool, Ambito::Ninguno, &FiltrosTraslado::default()).await.unwrap().is_empty());
        assert_eq!(
            despachar(&pool, Ambito::Ninguno, 5, &traslado(id), HOY).await.unwrap(),
            ResultadoDespacho::NoExiste
        );
        assert_eq!(crear(&pool, Ambito::Ninguno, 1, &nueva("ZZZ111"), HOY).await.unwrap(), ResultadoAlta::NoExiste);
    }

    /// Directiva 8: 100 despachos simultáneos de LA MISMA ambulancia. Solo uno
    /// puede salir; los otros 99 tienen que ver «ocupada», y no puede quedar más
    /// de un traslado en curso. Con timeout, para que un interbloqueo no se
    /// confunda con lentitud.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_despachos_concurrentes_no_parten_la_ambulancia_en_dos() {
        let pool = base().await;
        let id = crear_amb(&pool, "WXY123").await;

        let mut tareas = Vec::new();
        for _ in 0..100 {
            let p = pool.clone();
            tareas.push(tokio::spawn(async move {
                despachar(&p, Ambito::institucion(1), 5, &traslado(id), HOY).await
            }));
        }
        let plazo = std::time::Duration::from_secs(120);
        let (mut salieron, mut ocupadas, mut otras) = (0, 0, 0);
        for t in tareas {
            match tokio::time::timeout(plazo, t).await
                .expect("un despacho no respondió: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic")
                .expect("ninguno debía fallar por SQL")
            {
                ResultadoDespacho::Hecho(_) => salieron += 1,
                ResultadoDespacho::Ocupada { .. } | ResultadoDespacho::NoDisponible { .. } => ocupadas += 1,
                _ => otras += 1,
            }
        }
        assert_eq!(salieron, 1, "salieron {salieron} traslados con la misma ambulancia");
        assert_eq!(ocupadas, 99);
        assert_eq!(otras, 0);
        let en_curso: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM traslado WHERE ambulancia_id = ? AND estado = 'En Curso'")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(en_curso, 1);
        assert_eq!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().estado, "En Traslado");
    }
}
