// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Agendas, bloques, bloqueos y los turnos que salen de cruzarlos.
//!
//! Aquí solo hay acceso a datos y ámbito. Las reglas —partir una franja en
//! turnos, descontar bloqueos y citas— viven en [`medico_dominio::agenda`],
//! donde se prueban sin abrir una base.
//!
//! # El dueño es la institución, y no es opcional
//!
//! En el Python (`models.py:2159`, `2189`) `institucion_id` es `Optional` en
//! `AgendaMedico` y en `BloqueoAgenda`: el mismo agujero que tenía
//! `ItemInventario`. Aquí toda consulta lleva el ámbito y toda fila nace con
//! dueño. Los bloques cuelgan de su agenda y heredan el suyo por ahí.

use medico_dominio::agenda::{es_instante, minutos, Bloqueo, Franja, Ocupacion, Turno};
use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

/// Una agenda mensual con el nombre del médico ya resuelto.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AgendaVista {
    pub id: i64,
    pub medico_id: i64,
    pub medico_nombre: String,
    pub anio: i64,
    pub mes: i64,
    pub activa: bool,
    pub observaciones: Option<String>,
    pub institucion_id: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BloqueVista {
    pub id: i64,
    pub agenda_id: i64,
    pub dia_semana: i64,
    pub hora_inicio: String,
    pub hora_fin: String,
    pub duracion_slot_min: i64,
    pub activo: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgendaConBloques {
    #[serde(flatten)]
    pub agenda: AgendaVista,
    pub bloques: Vec<BloqueVista>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BloqueoVista {
    pub id: i64,
    pub medico_id: i64,
    pub medico_nombre: String,
    pub inicio: String,
    pub fin: String,
    pub motivo: String,
    pub tipo: String,
}

/// Quién puede tener agenda. Los roles que ATIENDEN a un paciente citado.
///
/// No es la lista de quien trabaja en la IPS: al laboratorista y al regente de
/// farmacia no se les agenda una cita, se les manda una orden. Sale de los roles
/// clínicos del Python (`agenda_medica.py:149`), que ya tenía esta pregunta
/// resuelta, y no de inventar una nueva.
const ROLES_QUE_ATIENDEN: &[&str] = &[
    "medico",
    "anestesiologo",
    "psicologo",
    "nutricionista",
    "terapeuta",
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MedicoAgendable {
    pub id: i64,
    pub nombre: String,
    pub rol: String,
}

/// Los profesionales del ámbito a los que se puede agendar.
pub async fn medicos(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<MedicoAgendable>, sqlx::Error> {
    let (cond, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(id) => ("institucion_id = ?", Some(id)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    // Un `?` por rol en vez de interpolar las cadenas. Hoy los cinco son
    // constantes de este archivo, así que interpolarlas era seguro — pero es el
    // patrón que deja de serlo el día que la lista salga de la base o traiga un
    // apóstrofo, y un marcador cuesta lo mismo que una comilla.
    let huecos = vec!["?"; ROLES_QUE_ATIENDEN.len()].join(",");
    let sql = format!(
        "SELECT id, nombre, rol FROM usuario \
         WHERE activo = 1 AND rol IN ({huecos}) AND {cond} ORDER BY nombre"
    );
    let mut q = sqlx::query_as::<_, MedicoAgendable>(&sql);
    for r in ROLES_QUE_ATIENDEN {
        q = q.bind(*r);
    }
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

const SELECT_AGENDA: &str = "SELECT a.id, a.medico_id, u.nombre AS medico_nombre, \
    a.anio, a.mes, a.activa, a.observaciones, a.institucion_id \
    FROM agenda_medico a JOIN usuario u ON u.id = a.medico_id";

/// Agendas del ámbito, con filtros opcionales.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: Option<i64>,
    anio: Option<i64>,
    mes: Option<i64>,
) -> Result<Vec<AgendaVista>, sqlx::Error> {
    let (cond, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(id) => ("a.institucion_id = ?", Some(id)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let mut sql = format!("{SELECT_AGENDA} WHERE {cond}");
    if medico_id.is_some() {
        sql.push_str(" AND a.medico_id = ?");
    }
    if anio.is_some() {
        sql.push_str(" AND a.anio = ?");
    }
    if mes.is_some() {
        sql.push_str(" AND a.mes = ?");
    }
    sql.push_str(" ORDER BY a.anio DESC, a.mes DESC, u.nombre");
    let mut q = sqlx::query_as::<_, AgendaVista>(&sql);
    for v in [inst, medico_id, anio, mes].into_iter().flatten() {
        q = q.bind(v);
    }
    q.fetch_all(pool).await
}

#[derive(Debug, Clone, Deserialize)]
pub struct BloqueNuevo {
    pub dia_semana: i64,
    pub hora_inicio: String,
    pub hora_fin: String,
    /// Cuánto dura cada turno. Si no viene, la tabla pone 30 — es la
    /// configuración convencional, declarada, no un dato clínico inventado.
    pub duracion_slot_min: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgendaNueva {
    pub medico_id: i64,
    pub anio: i64,
    pub mes: i64,
    pub observaciones: Option<String>,
    #[serde(default)]
    pub bloques: Vec<BloqueNuevo>,
}

/// Crea la agenda y sus bloques **en una sola transacción**: una agenda a medio
/// crear —sin los bloques que la definen— ofrecería cero turnos y parecería que
/// el médico no atiende ese mes.
///
/// `None` si el ámbito no puede crear, si el médico no es suyo, si alguna hora
/// es ilegible o si ya hay agenda de ese médico y mes.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    quien: i64,
    nueva: &AgendaNueva,
) -> Result<Option<i64>, sqlx::Error> {
    let institucion_id = match ambito {
        Ambito::Institucion(id) => id,
        Ambito::Superadmin | Ambito::Ninguno => return Ok(None),
    };
    if !crate::usuario::es_del_ambito(pool, ambito, nueva.medico_id).await? {
        return Ok(None);
    }
    // Las horas se validan ANTES de abrir la transacción: un bloque ilegible es
    // un error del que pide, no un fallo a mitad de escritura.
    for b in &nueva.bloques {
        let (Some(i), Some(f)) = (minutos(&b.hora_inicio), minutos(&b.hora_fin)) else {
            return Ok(None);
        };
        if i >= f {
            return Ok(None);
        }
    }
    let mut tx = pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO agenda_medico (institucion_id, medico_id, anio, mes, observaciones, creada_por_id) \
         VALUES (?,?,?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(nueva.medico_id)
    .bind(nueva.anio)
    .bind(nueva.mes)
    .bind(&nueva.observaciones)
    .bind(quien)
    .execute(&mut *tx)
    .await;
    let id = match r {
        Ok(r) => r.last_insert_rowid(),
        // Choca con el UNIQUE (ya hay agenda de ese mes) o con un CHECK (mes 13).
        Err(sqlx::Error::Database(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    for b in &nueva.bloques {
        let r = sqlx::query(
            "INSERT INTO bloque_horario (agenda_id, dia_semana, hora_inicio, hora_fin, duracion_slot_min) \
             VALUES (?,?,?,?,COALESCE(?,30))",
        )
        .bind(id)
        .bind(b.dia_semana)
        .bind(&b.hora_inicio)
        .bind(&b.hora_fin)
        .bind(b.duracion_slot_min)
        .execute(&mut *tx)
        .await;
        if let Err(sqlx::Error::Database(_)) = r {
            // La transacción se descarta al salir: no queda agenda coja.
            return Ok(None);
        }
        r?;
    }
    tx.commit().await?;
    Ok(Some(id))
}

/// Una agenda con sus bloques, si es del ámbito.
pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<AgendaConBloques>, sqlx::Error> {
    let Some(agenda) = una(pool, ambito, id).await? else {
        return Ok(None);
    };
    let bloques = sqlx::query_as::<_, BloqueVista>(
        "SELECT id, agenda_id, dia_semana, hora_inicio, hora_fin, duracion_slot_min, activo \
         FROM bloque_horario WHERE agenda_id = ? ORDER BY dia_semana, hora_inicio",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(Some(AgendaConBloques { agenda, bloques }))
}

async fn una(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<AgendaVista>, sqlx::Error> {
    let a = sqlx::query_as::<_, AgendaVista>(&format!("{SELECT_AGENDA} WHERE a.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(a.filter(|a| ambito.puede_ver(Some(a.institucion_id)) == medico_dominio::Visibilidad::Si))
}

/// Activa/desactiva o reetiqueta una agenda. `false` si no existe o no es suya.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    activa: Option<bool>,
    observaciones: Option<&str>,
) -> Result<bool, sqlx::Error> {
    if una(pool, ambito, id).await?.is_none() {
        return Ok(false);
    }
    let r = sqlx::query(
        "UPDATE agenda_medico SET activa = COALESCE(?, activa), \
         observaciones = COALESCE(?, observaciones) WHERE id = ?",
    )
    .bind(activa)
    .bind(observaciones)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

/// Borra una agenda (y sus bloques, por la cascada). `false` si no es suya.
pub async fn borrar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    if una(pool, ambito, id).await?.is_none() {
        return Ok(false);
    }
    let r = sqlx::query("DELETE FROM agenda_medico WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// Añade un bloque a una agenda del ámbito. `None` si la agenda no es suya o el
/// bloque es ilegible.
pub async fn anadir_bloque(
    pool: &crate::Pool,
    ambito: Ambito,
    agenda_id: i64,
    b: &BloqueNuevo,
) -> Result<Option<i64>, sqlx::Error> {
    if una(pool, ambito, agenda_id).await?.is_none() {
        return Ok(None);
    }
    let (Some(i), Some(f)) = (minutos(&b.hora_inicio), minutos(&b.hora_fin)) else {
        return Ok(None);
    };
    if i >= f {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO bloque_horario (agenda_id, dia_semana, hora_inicio, hora_fin, duracion_slot_min) \
         VALUES (?,?,?,?,COALESCE(?,30))",
    )
    .bind(agenda_id)
    .bind(b.dia_semana)
    .bind(&b.hora_inicio)
    .bind(&b.hora_fin)
    .bind(b.duracion_slot_min)
    .execute(pool)
    .await;
    match r {
        Ok(r) => Ok(Some(r.last_insert_rowid())),
        Err(sqlx::Error::Database(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Activa o desactiva un bloque de una agenda del ámbito.
pub async fn activar_bloque(
    pool: &crate::Pool,
    ambito: Ambito,
    agenda_id: i64,
    bloque_id: i64,
    activo: bool,
) -> Result<bool, sqlx::Error> {
    if una(pool, ambito, agenda_id).await?.is_none() {
        return Ok(false);
    }
    let r = sqlx::query("UPDATE bloque_horario SET activo = ? WHERE id = ? AND agenda_id = ?")
        .bind(activo)
        .bind(bloque_id)
        .bind(agenda_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// Borra un bloque. El `agenda_id` va en el `WHERE` además del ámbito: sin él,
/// bastaría acertar un id de bloque para borrar el de otra agenda.
pub async fn borrar_bloque(
    pool: &crate::Pool,
    ambito: Ambito,
    agenda_id: i64,
    bloque_id: i64,
) -> Result<bool, sqlx::Error> {
    if una(pool, ambito, agenda_id).await?.is_none() {
        return Ok(false);
    }
    let r = sqlx::query("DELETE FROM bloque_horario WHERE id = ? AND agenda_id = ?")
        .bind(bloque_id)
        .bind(agenda_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

#[derive(Debug, Clone, Deserialize)]
pub struct BloqueoNuevo {
    pub medico_id: i64,
    /// `YYYY-MM-DD HH:MM`.
    pub inicio: String,
    pub fin: String,
    pub motivo: String,
    pub tipo: Option<String>,
}

const SELECT_BLOQUEO: &str = "SELECT b.id, b.medico_id, u.nombre AS medico_nombre, \
    b.inicio, b.fin, b.motivo, b.tipo \
    FROM bloqueo_agenda b JOIN usuario u ON u.id = b.medico_id";

/// Bloqueos del ámbito que se solapan con `[desde, hasta]`, si se dan.
pub async fn listar_bloqueos(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: Option<i64>,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Vec<BloqueoVista>, sqlx::Error> {
    let (cond, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(id) => ("b.institucion_id = ?", Some(id)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let mut sql = format!("{SELECT_BLOQUEO} WHERE {cond}");
    if medico_id.is_some() {
        sql.push_str(" AND b.medico_id = ?");
    }
    // Solapamiento, no contención: un bloqueo que empezó en julio y termina en
    // agosto tiene que salir al preguntar por agosto.
    if desde.is_some() {
        sql.push_str(" AND b.fin >= ?");
    }
    if hasta.is_some() {
        sql.push_str(" AND b.inicio <= ?");
    }
    sql.push_str(" ORDER BY b.inicio");
    let mut q = sqlx::query_as::<_, BloqueoVista>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(m) = medico_id {
        q = q.bind(m);
    }
    if let Some(d) = desde {
        q = q.bind(format!("{d} 00:00"));
    }
    if let Some(h) = hasta {
        q = q.bind(format!("{h} 23:59"));
    }
    q.fetch_all(pool).await
}

/// Lo que devuelve crear un bloqueo: su id y **cuántas citas quedan dentro**.
///
/// El aviso se conserva del Python y no es adorno: bloquear al médico no cancela
/// las citas que ya tenía, así que sin decirlo el paciente se presenta un día en
/// que no hay quien lo atienda. Lo que NO se hace es cancelarlas solas —eso es
/// una decisión asistencial de quien coordina, no del sistema— ni impedir el
/// bloqueo: una incapacidad no se puede rechazar porque haya agenda.
#[derive(Debug, Clone, Serialize)]
pub struct BloqueoCreado {
    pub id: i64,
    pub citas_en_conflicto: i64,
}

pub async fn crear_bloqueo(
    pool: &crate::Pool,
    ambito: Ambito,
    quien: i64,
    b: &BloqueoNuevo,
) -> Result<Option<BloqueoCreado>, sqlx::Error> {
    let institucion_id = match ambito {
        Ambito::Institucion(id) => id,
        Ambito::Superadmin | Ambito::Ninguno => return Ok(None),
    };
    if !crate::usuario::es_del_ambito(pool, ambito, b.medico_id).await? {
        return Ok(None);
    }
    // El formato se comprueba AQUÍ y no solo en la base: el `CHECK (inicio <
    // fin)` de la migración compara TEXTO, así que `{"inicio":"0000",
    // "fin":"9999"}` lo pasaba y dejaba al profesional con cero turnos libres
    // cualquier día, para siempre. Y con el formato garantizado, `recortar` deja
    // de necesitar una rama para instantes sin hora que la consulta que la
    // alimenta nunca le entregaba.
    if !es_instante(&b.inicio) || !es_instante(&b.fin) {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO bloqueo_agenda (institucion_id, medico_id, inicio, fin, motivo, tipo, creado_por_id) \
         VALUES (?,?,?,?,?,COALESCE(?,'OTRO'),?)",
    )
    .bind(institucion_id)
    .bind(b.medico_id)
    .bind(&b.inicio)
    .bind(&b.fin)
    .bind(&b.motivo)
    .bind(&b.tipo)
    .bind(quien)
    .execute(pool)
    .await;
    let id = match r {
        Ok(r) => r.last_insert_rowid(),
        // CHECK del tipo, o `inicio >= fin`.
        Err(sqlx::Error::Database(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    // Las citas que caen dentro. Se comparan `fecha || ' ' || hora` contra los
    // instantes del bloqueo: el bloqueo es de medio día, no de días enteros, y
    // comparar solo la fecha avisaría de citas de la mañana por una reunión de
    // la tarde. Las canceladas no cuentan: no hay a quién reasignar.
    let (citas_en_conflicto,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM cita \
         WHERE medico_id = ? AND estado <> 'CANCELADA' \
           AND (fecha || ' ' || hora) >= ? AND (fecha || ' ' || hora) <= ?",
    )
    .bind(b.medico_id)
    .bind(&b.inicio)
    .bind(&b.fin)
    .fetch_one(pool)
    .await?;
    Ok(Some(BloqueoCreado { id, citas_en_conflicto }))
}

pub async fn borrar_bloqueo(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<bool, sqlx::Error> {
    let cond = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return Ok(false),
    };
    let sql = if cond.is_some() {
        "DELETE FROM bloqueo_agenda WHERE id = ? AND institucion_id = ?"
    } else {
        "DELETE FROM bloqueo_agenda WHERE id = ?"
    };
    let mut q = sqlx::query(sql).bind(id);
    if let Some(i) = cond {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

/// Los turnos de un médico un día concreto.
///
/// `fecha` es `YYYY-MM-DD`. Devuelve lista vacía si el médico no es del ámbito o
/// no tiene agenda activa ese mes — «no atiende», que es distinto de «está
/// lleno», y por eso NO se fabrica una jornada por defecto.
pub async fn turnos(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: i64,
    fecha: &str,
) -> Result<Vec<Turno>, sqlx::Error> {
    if !crate::usuario::es_del_ambito(pool, ambito, medico_id).await? {
        return Ok(Vec::new());
    }
    let Some(dia) = chrono::NaiveDate::parse_from_str(fecha, "%Y-%m-%d").ok() else {
        return Ok(Vec::new());
    };
    use chrono::Datelike;
    // 0 = lunes, como el `weekday()` del Python y como el CHECK de la tabla.
    let dia_semana = dia.weekday().num_days_from_monday() as i64;

    let franjas: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT b.hora_inicio, b.hora_fin, b.duracion_slot_min \
         FROM bloque_horario b JOIN agenda_medico a ON a.id = b.agenda_id \
         WHERE a.medico_id = ? AND a.anio = ? AND a.mes = ? AND a.activa = 1 \
           AND b.dia_semana = ? AND b.activo = 1 \
         ORDER BY b.hora_inicio",
    )
    .bind(medico_id)
    .bind(dia.year() as i64)
    .bind(dia.month() as i64)
    .bind(dia_semana)
    .fetch_all(pool)
    .await?;
    if franjas.is_empty() {
        return Ok(Vec::new());
    }

    let bloqueos: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT inicio, fin, tipo FROM bloqueo_agenda \
         WHERE medico_id = ? AND fin >= ? AND inicio <= ?",
    )
    .bind(medico_id)
    .bind(format!("{fecha} 00:00"))
    .bind(format!("{fecha} 23:59"))
    .fetch_all(pool)
    .await?;

    let ocupadas: Vec<(String, Option<i64>)> = sqlx::query_as(
        "SELECT hora, duracion_minutos FROM cita \
         WHERE medico_id = ? AND fecha = ? AND estado <> 'CANCELADA'",
    )
    .bind(medico_id)
    .bind(fecha)
    .fetch_all(pool)
    .await?;

    Ok(medico_dominio::agenda::turnos(
        &franjas
            .iter()
            .filter_map(|(i, f, d)| {
                Some(Franja {
                    inicio: minutos(i)?,
                    fin: minutos(f)?,
                    // Una duración de cero o negativa la impide el CHECK de la
                    // migración, pero aquí se traduce a «esta franja no cuenta»
                    // en vez de a un valor de relleno: si algún día una fila la
                    // trajera, la franja desaparece —y se nota— en lugar de
                    // colgar el servidor en un bucle infinito.
                    duracion: std::num::NonZeroU16::new(u16::try_from(*d).ok()?)?,
                })
            })
            .collect::<Vec<_>>(),
        &bloqueos
            .iter()
            .map(|(i, f, t)| Bloqueo {
                // Un bloqueo que empezó ayer se recorta a las 00:00 de hoy, y el
                // que termina mañana, a las 23:59: dentro del día, tapa entero.
                inicio: recortar(i, fecha, 0),
                fin: recortar(f, fecha, 1439),
                tipo: t.clone(),
            })
            .collect::<Vec<_>>(),
        &ocupadas
            .iter()
            .filter_map(|(h, d)| {
                Some(Ocupacion {
                    inicio: minutos(h)?,
                    duracion: d.and_then(|d| u16::try_from(d).ok()),
                })
            })
            .collect::<Vec<_>>(),
    ))
}

/// El minuto del día que le corresponde a un instante `YYYY-MM-DD HH:MM` visto
/// desde `fecha`. Antes del día, el minuto 0; después, el 1439.
///
/// Solo entran instantes bien formados: `crear_bloqueo` los valida con
/// [`es_instante`]. Antes había aquí una rama para instantes SIN hora, comentada
/// «el día entero», y era una promesa que la consulta de arriba no podía
/// cumplir: filtra `fin >= '{fecha} 00:00'` como texto, y `"2026-08-10"` es
/// prefijo de `"2026-08-10 00:00"`, o sea MENOR — el bloqueo ni siquiera se
/// devolvía, y su último día salía entero libre. Se quitó la rama en vez de
/// arreglar la consulta: si la entrada no puede tener esa forma, el código que
/// la contempla es un mecanismo que sostiene a otro.
fn recortar(instante: &str, fecha: &str, si_fuera: u16) -> u16 {
    match instante.split_once(' ') {
        Some((d, h)) if d == fecha => minutos(h).unwrap_or(si_fuera),
        Some((d, _)) if d < fecha => 0,
        Some(_) => 1439,
        // Imposible con `es_instante` delante; se trata como «fuera del día».
        None => si_fuera,
    }
}

/// Indicadores de la agenda en un periodo.
///
/// # Por qué la oportunidad va aparte y con su propia cuenta
///
/// El extremo del Python se llama `/indicadores/oportunidad` y NO calcula
/// oportunidad: cuenta atendidas, canceladas e inasistencias y devuelve las dos
/// metas de la Resolución 256/2016 como constantes al lado. Quien lee el tablero
/// entiende que su IPS está midiendo lo que la norma exige, y no lo está.
///
/// La oportunidad son los días entre la SOLICITUD y la atención, así que sale de
/// `cita.solicitada_en` (migración 0035). Las citas anteriores a esa columna no
/// lo traen y no se rellenan: el indicador dice sobre cuántas pudo medir, y con
/// `medidas = 0` el promedio es `None` — no cero, que es el mejor número posible
/// y sería el más engañoso.
#[derive(Debug, Clone, Serialize)]
pub struct Indicadores {
    pub desde: String,
    pub hasta: String,
    pub atendidas: i64,
    pub canceladas: i64,
    pub no_asistio: i64,
    pub total: i64,
    pub tasa_inasistencia_pct: Option<f64>,
    pub tasa_cancelacion_pct: Option<f64>,
    /// Sobre cuántas citas del periodo se pudo calcular la oportunidad.
    pub oportunidad_medidas: i64,
    /// De cuántas NO se sabe, porque no tienen fecha de solicitud registrada.
    pub oportunidad_sin_dato: i64,
    /// Cuántas traen una solicitud POSTERIOR a la cita. Es imposible —nadie pide
    /// una cita después de que ocurra—, así que no se promedian: darían días
    /// negativos que MEJORAN el indicador, o sea que un dato roto se leería como
    /// buena gestión. Se cuentan aparte para que se vean y se corrijan.
    pub oportunidad_incoherentes: i64,
    /// Promedio de días entre solicitud y cita. `None` si no hubo ninguna que
    /// medir; no se sustituye por 0.
    pub oportunidad_dias_promedio: Option<f64>,
    pub meta_medicina_general_dias: i64,
    pub meta_especialista_dias: i64,
    pub normativa: &'static str,
}

pub async fn indicadores(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: Option<i64>,
    desde: &str,
    hasta: &str,
) -> Result<Indicadores, sqlx::Error> {
    let vacio = Indicadores {
        desde: desde.into(),
        hasta: hasta.into(),
        atendidas: 0,
        canceladas: 0,
        no_asistio: 0,
        total: 0,
        tasa_inasistencia_pct: None,
        tasa_cancelacion_pct: None,
        oportunidad_medidas: 0,
        oportunidad_sin_dato: 0,
        oportunidad_incoherentes: 0,
        oportunidad_dias_promedio: None,
        meta_medicina_general_dias: 3,
        meta_especialista_dias: 5,
        normativa: "Resolución 256 de 2016",
    };
    let inst = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return Ok(vacio),
    };
    let mut sql = String::from(
        "SELECT estado, solicitada_en, fecha FROM cita WHERE fecha >= ? AND fecha <= ?",
    );
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    if medico_id.is_some() {
        sql.push_str(" AND medico_id = ?");
    }
    let mut q = sqlx::query_as::<_, (String, Option<String>, String)>(&sql)
        .bind(desde)
        .bind(hasta);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(m) = medico_id {
        q = q.bind(m);
    }
    let filas = q.fetch_all(pool).await?;

    let mut r = vacio;
    let mut suma_dias = 0i64;
    for (estado, solicitada, fecha) in &filas {
        match estado.as_str() {
            "ATENDIDA" => r.atendidas += 1,
            "CANCELADA" => r.canceladas += 1,
            "NO_ASISTIO" => r.no_asistio += 1,
            _ => {}
        }
        match solicitada.as_deref().and_then(|s| dias_entre(s, fecha)) {
            Some(d) if d >= 0 => {
                r.oportunidad_medidas += 1;
                suma_dias += d;
            }
            Some(_) => r.oportunidad_incoherentes += 1,
            None => r.oportunidad_sin_dato += 1,
        }
    }
    r.total = r.atendidas + r.canceladas + r.no_asistio;
    if r.total > 0 {
        let pct = |n: i64| Some((n as f64 * 10_000.0 / r.total as f64).round() / 100.0);
        r.tasa_inasistencia_pct = pct(r.no_asistio);
        r.tasa_cancelacion_pct = pct(r.canceladas);
    }
    if r.oportunidad_medidas > 0 {
        let p = suma_dias as f64 / r.oportunidad_medidas as f64;
        r.oportunidad_dias_promedio = Some((p * 100.0).round() / 100.0);
    }
    Ok(r)
}

/// Días entre dos fechas `YYYY-MM-DD`. `None` si alguna es ilegible.
fn dias_entre(desde: &str, hasta: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(desde, "%Y-%m-%d").ok()?;
    let h = chrono::NaiveDate::parse_from_str(hasta, "%Y-%m-%d").ok()?;
    Some((h - d).num_days())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos instituciones con un médico cada una, y un paciente en la 1.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (5,'Dra Uno','u1@x.co','h','medico',1,1),
                    (6,'Dr Dos','u2@x.co','h','medico',2,1),
                    (7,'Lab Uno','l1@x.co','h','laboratorista',1,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,
                                   fecha_nacimiento,sexo,institucion_id)
             VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F',1)",
        ).execute(&pool).await.unwrap();
        pool
    }

    fn agenda_de_agosto(medico_id: i64) -> AgendaNueva {
        AgendaNueva {
            medico_id,
            anio: 2026,
            mes: 8,
            observaciones: None,
            // 2026-08-03 es lunes.
            bloques: vec![BloqueNuevo {
                dia_semana: 0,
                hora_inicio: "08:00".into(),
                hora_fin: "10:00".into(),
                duracion_slot_min: Some(30),
            }],
        }
    }

    #[tokio::test]
    async fn solo_salen_los_roles_que_atienden() {
        let pool = base().await;
        let ms = medicos(&pool, Ambito::institucion(1)).await.unwrap();
        assert_eq!(ms.len(), 1, "el laboratorista no se agenda, se le manda una orden");
        assert_eq!(ms[0].nombre, "Dra Uno");
        // Y no se ve al médico de la otra institución.
        assert!(!ms.iter().any(|m| m.nombre == "Dr Dos"));
    }

    #[tokio::test]
    async fn crear_y_leer_la_agenda_propia() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5))
            .await.unwrap().expect("no creó la agenda");
        let a = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(a.agenda.medico_nombre, "Dra Uno");
        assert_eq!(a.bloques.len(), 1);
        // La institución 2 no la ve.
        assert!(obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn no_se_le_hace_agenda_a_un_medico_ajeno() {
        let pool = base().await;
        // El médico 6 es de la institución 2.
        assert!(crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(6))
            .await.unwrap().is_none());
    }

    #[tokio::test]
    async fn dos_agendas_del_mismo_mes_no_caben() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().is_some());
        assert!(
            crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().is_none(),
            "dos agendas del mismo mes dan dos respuestas a «¿a qué hora atiende?»"
        );
    }

    /// Una hora ilegible NO crea una agenda a medias: ni agenda, ni bloques.
    #[tokio::test]
    async fn un_bloque_ilegible_no_deja_agenda_coja() {
        let pool = base().await;
        let mut nueva = agenda_de_agosto(5);
        nueva.bloques[0].hora_inicio = "25:00".into();
        assert!(crear(&pool, Ambito::institucion(1), 5, &nueva).await.unwrap().is_none());
        assert!(listar(&pool, Ambito::institucion(1), None, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn el_bloque_al_reves_no_pasa() {
        let pool = base().await;
        let mut nueva = agenda_de_agosto(5);
        nueva.bloques[0].hora_inicio = "12:00".into();
        nueva.bloques[0].hora_fin = "08:00".into();
        assert!(crear(&pool, Ambito::institucion(1), 5, &nueva).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn los_turnos_del_lunes_salen_y_los_del_martes_no() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().unwrap();
        // Lunes 2026-08-03.
        let ts = turnos(&pool, Ambito::institucion(1), 5, "2026-08-03").await.unwrap();
        assert_eq!(ts.len(), 4);
        assert!(ts.iter().all(|t| t.disponible));
        // Martes: el bloque es solo de lunes, así que no atiende.
        assert!(turnos(&pool, Ambito::institucion(1), 5, "2026-08-04").await.unwrap().is_empty());
        // Septiembre: no hay agenda de ese mes. NO se fabrica una jornada.
        assert!(turnos(&pool, Ambito::institucion(1), 5, "2026-09-07").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn un_medico_ajeno_no_tiene_turnos_para_mi() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(2), 6, &agenda_de_agosto(6)).await.unwrap().unwrap();
        assert!(turnos(&pool, Ambito::institucion(1), 6, "2026-08-03").await.unwrap().is_empty());
    }

    /// La cita ocupa su turno; la cancelada, no.
    #[tokio::test]
    async fn la_cita_tapa_su_turno_y_la_cancelada_lo_libera() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,medico_id,fecha,hora,estado,duracion_minutos)
             VALUES (1,10,5,'2026-08-03','08:30','PROGRAMADA',30)",
        ).execute(&pool).await.unwrap();
        let ts = turnos(&pool, Ambito::institucion(1), 5, "2026-08-03").await.unwrap();
        let ocupado = ts.iter().find(|t| t.hora == "08:30").unwrap();
        assert!(!ocupado.disponible);

        sqlx::query("UPDATE cita SET estado = 'CANCELADA' WHERE hora = '08:30'")
            .execute(&pool).await.unwrap();
        let ts = turnos(&pool, Ambito::institucion(1), 5, "2026-08-03").await.unwrap();
        assert!(ts.iter().find(|t| t.hora == "08:30").unwrap().disponible);
    }

    /// El bloqueo que cruza la medianoche tapa el día entero visto desde hoy.
    #[tokio::test]
    async fn un_bloqueo_de_varios_dias_tapa_el_dia_de_enmedio() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().unwrap();
        // Lunes 3 y lunes 10 caen dentro; se prueba el 10, que está en medio.
        crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
            medico_id: 5,
            inicio: "2026-08-08 00:00".into(),
            fin: "2026-08-15 23:59".into(),
            motivo: "Vacaciones".into(),
            tipo: Some("VACACIONES".into()),
        }).await.unwrap().expect("no creó el bloqueo");
        let ts = turnos(&pool, Ambito::institucion(1), 5, "2026-08-10").await.unwrap();
        assert_eq!(ts.len(), 4);
        assert!(ts.iter().all(|t| !t.disponible), "el bloqueo de varios días no tapó el día de enmedio");
    }

    /// Bloquear al médico NO cancela sus citas, así que se dice cuántas quedan
    /// dentro. Sin ese aviso, el paciente se presenta un día en que no hay quien
    /// lo atienda.
    #[tokio::test]
    async fn el_bloqueo_avisa_de_las_citas_que_deja_dentro() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,medico_id,fecha,hora,estado)
             VALUES (1,10,5,'2026-08-10','09:00','PROGRAMADA'),
                    (1,10,5,'2026-08-10','16:00','PROGRAMADA'),
                    (1,10,5,'2026-08-10','10:00','CANCELADA'),
                    (1,10,5,'2026-08-20','09:00','PROGRAMADA')",
        ).execute(&pool).await.unwrap();
        // Solo la mañana del 10.
        let r = crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
            medico_id: 5,
            inicio: "2026-08-10 08:00".into(),
            fin: "2026-08-10 12:00".into(),
            motivo: "Reunión".into(),
            tipo: Some("REUNION".into()),
        }).await.unwrap().unwrap();
        assert_eq!(
            r.citas_en_conflicto, 1,
            "contó citas de la tarde, de otro día, o la cancelada"
        );
    }

    /// Los seis tipos que la pantalla ofrece se guardan; uno inventado, no.
    #[tokio::test]
    async fn el_vocabulario_de_bloqueo_es_el_que_la_pantalla_ofrece() {
        let pool = base().await;
        for (i, t) in ["VACACIONES", "INCAPACIDAD", "REUNION", "CAPACITACION", "DIA_LIBRE", "OTRO"]
            .iter()
            .enumerate()
        {
            let r = crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
                medico_id: 5,
                inicio: format!("2026-09-{:02} 08:00", i + 1),
                fin: format!("2026-09-{:02} 12:00", i + 1),
                motivo: "x".into(),
                tipo: Some((*t).into()),
            }).await.unwrap();
            assert!(r.is_some(), "el tipo {t} no se pudo guardar");
        }
        assert!(crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
            medico_id: 5,
            inicio: "2026-09-20 08:00".into(),
            fin: "2026-09-20 12:00".into(),
            motivo: "x".into(),
            tipo: Some("Día Libre".into()),
        }).await.unwrap().is_none(), "entró el texto de la pantalla en la columna");
    }

    #[tokio::test]
    async fn el_bloqueo_al_reves_no_se_guarda() {
        let pool = base().await;
        assert!(crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
            medico_id: 5,
            inicio: "2026-08-10 12:00".into(),
            fin: "2026-08-10 08:00".into(),
            motivo: "Al revés".into(),
            tipo: None,
        }).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn los_bloqueos_se_listan_por_solapamiento_no_por_contencion() {
        let pool = base().await;
        crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
            medico_id: 5,
            inicio: "2026-07-28 00:00".into(),
            fin: "2026-08-05 23:59".into(),
            motivo: "A caballo entre dos meses".into(),
            tipo: None,
        }).await.unwrap().unwrap();
        let bs = listar_bloqueos(&pool, Ambito::institucion(1), None, Some("2026-08-01"), Some("2026-08-31"))
            .await.unwrap();
        assert_eq!(bs.len(), 1, "un bloqueo que empezó el mes pasado no salió al preguntar por agosto");
        // Y la institución 2 no lo ve.
        assert!(listar_bloqueos(&pool, Ambito::institucion(2), None, None, None).await.unwrap().is_empty());
    }

    /// El indicador dice sobre cuántas pudo medir y NO rellena las que no traen
    /// fecha de solicitud.
    #[tokio::test]
    async fn la_oportunidad_no_se_inventa_para_las_citas_sin_solicitud() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,medico_id,fecha,hora,estado,solicitada_en)
             VALUES (1,10,5,'2026-08-10','08:00','ATENDIDA','2026-08-05'),
                    (1,10,5,'2026-08-11','08:00','ATENDIDA','2026-08-10'),
                    (1,10,5,'2026-08-12','08:00','NO_ASISTIO',NULL)",
        ).execute(&pool).await.unwrap();
        let i = indicadores(&pool, Ambito::institucion(1), None, "2026-08-01", "2026-08-31")
            .await.unwrap();
        assert_eq!(i.total, 3);
        assert_eq!(i.atendidas, 2);
        assert_eq!(i.no_asistio, 1);
        assert_eq!(i.oportunidad_medidas, 2, "midió citas sin fecha de solicitud");
        assert_eq!(i.oportunidad_sin_dato, 1);
        // (5 + 1) / 2 = 3 días.
        assert_eq!(i.oportunidad_dias_promedio, Some(3.0));
        assert_eq!(i.tasa_inasistencia_pct, Some(33.33));
    }

    /// Una solicitud POSTERIOR a la cita es imposible. No se promedia: daría
    /// días negativos que MEJORAN el indicador, o sea que un dato roto se leería
    /// como buena gestión.
    #[tokio::test]
    async fn una_solicitud_posterior_a_la_cita_no_mejora_el_indicador() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,medico_id,fecha,hora,estado,solicitada_en)
             VALUES (1,10,5,'2026-08-10','08:00','ATENDIDA','2026-08-04'),
                    (1,10,5,'2026-08-11','08:00','ATENDIDA','2026-09-30')",
        ).execute(&pool).await.unwrap();
        let i = indicadores(&pool, Ambito::institucion(1), None, "2026-08-01", "2026-08-31")
            .await.unwrap();
        assert_eq!(i.oportunidad_medidas, 1);
        assert_eq!(i.oportunidad_incoherentes, 1, "la fecha imposible se promedió");
        assert_eq!(
            i.oportunidad_dias_promedio, Some(6.0),
            "el promedio se movió: entraron días negativos"
        );
    }

    /// Sin nada que medir, el promedio es «no se sabe» y NO cero — que sería el
    /// mejor resultado posible, inventado.
    #[tokio::test]
    async fn sin_datos_la_oportunidad_es_desconocida_no_cero() {
        let pool = base().await;
        let i = indicadores(&pool, Ambito::institucion(1), None, "2026-08-01", "2026-08-31")
            .await.unwrap();
        assert_eq!(i.total, 0);
        assert_eq!(i.oportunidad_dias_promedio, None);
        assert_eq!(i.tasa_inasistencia_pct, None);
    }

    /// EL SUPERADMIN NO CREA AGENDAS, y sí puede borrarlas. Queda fijado aquí
    /// para que sea una decisión y no un accidente.
    ///
    /// Crear exige elegir institución —la agenda es de una— y el superadmin no
    /// tiene ninguna; ninguna pantalla le ofrece escogerla, así que aceptarlo
    /// pediría API nueva para un caso que nadie pide. Borrar y corregir sí
    /// funcionan porque `puede_ver` le concede todo, y eso es lo que hace falta
    /// para dar soporte a un cliente. Si algún día hace falta que cree, la
    /// institución va en el cuerpo y de forma explícita.
    #[tokio::test]
    async fn el_superadmin_repara_agendas_pero_no_las_crea() {
        let pool = base().await;
        assert!(
            crear(&pool, Ambito::Superadmin, 5, &agenda_de_agosto(5)).await.unwrap().is_none(),
            "crear sin institución elegiría una por su cuenta"
        );
        let id = crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::Superadmin, id).await.unwrap().is_some());
        assert!(actualizar(&pool, Ambito::Superadmin, id, Some(false), None).await.unwrap());
        assert!(borrar(&pool, Ambito::Superadmin, id).await.unwrap());
    }

    /// Al superadmin no se le hace agenda: no atiende pacientes en ninguna
    /// institución. Antes esto salía bien por casualidad —el NULL de su
    /// `institucion_id` se decodificaba como 0—, y ahora se dice.
    #[tokio::test]
    async fn al_superadmin_no_se_le_hace_agenda() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (99,'Dueño','root@x.co','h','superadmin',NULL,1)",
        ).execute(&pool).await.unwrap();
        assert!(crear(&pool, Ambito::institucion(1), 5, &AgendaNueva {
            medico_id: 99, anio: 2026, mes: 8, observaciones: None, bloques: vec![],
        }).await.unwrap().is_none());
        assert!(turnos(&pool, Ambito::institucion(1), 99, "2026-08-03").await.unwrap().is_empty());
    }

    /// El bloqueo con formato basura no entra. `{"inicio":"0000","fin":"9999"}`
    /// pasaba el `CHECK (inicio < fin)` —que compara TEXTO— y dejaba al
    /// profesional con cero turnos libres cualquier día, para siempre.
    #[tokio::test]
    async fn un_bloqueo_con_formato_basura_no_entra() {
        let pool = base().await;
        for (ini, fin) in [
            ("0000", "9999"),
            ("2026-08-10", "2026-08-11"),          // sin hora
            ("2026-13-01 08:00", "2026-13-02 08:00"), // mes 13
            ("2026-02-31 08:00", "2026-02-31 12:00"), // 31 de febrero
            ("2026-08-10T08:00", "2026-08-10T12:00"), // ISO con T
        ] {
            assert!(
                crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
                    medico_id: 5, inicio: ini.into(), fin: fin.into(),
                    motivo: "x".into(), tipo: None,
                }).await.unwrap().is_none(),
                "se aceptó el bloqueo «{ini}» → «{fin}»"
            );
        }
        // Y el bien formado sigue entrando: la regla no cerró la operación.
        assert!(crear_bloqueo(&pool, Ambito::institucion(1), 5, &BloqueoNuevo {
            medico_id: 5, inicio: "2026-08-10 08:00".into(), fin: "2026-08-10 12:00".into(),
            motivo: "Reunión".into(), tipo: Some("REUNION".into()),
        }).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn el_ambito_ninguno_no_ve_ni_crea_nada() {
        let pool = base().await;
        assert!(listar(&pool, Ambito::Ninguno, None, None, None).await.unwrap().is_empty());
        assert!(medicos(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert!(crear(&pool, Ambito::Ninguno, 5, &agenda_de_agosto(5)).await.unwrap().is_none());
        assert!(turnos(&pool, Ambito::Ninguno, 5, "2026-08-03").await.unwrap().is_empty());
        assert!(listar_bloqueos(&pool, Ambito::Ninguno, None, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn el_bloque_ajeno_no_se_borra_acertando_el_id() {
        let pool = base().await;
        let mia = crear(&pool, Ambito::institucion(1), 5, &agenda_de_agosto(5)).await.unwrap().unwrap();
        let suya = crear(&pool, Ambito::institucion(2), 6, &agenda_de_agosto(6)).await.unwrap().unwrap();
        let bloques = obtener(&pool, Ambito::institucion(2), suya).await.unwrap().unwrap().bloques;
        let id_ajeno = bloques[0].id;
        // Desde la institución 1, con MI agenda y SU bloque.
        assert!(!borrar_bloque(&pool, Ambito::institucion(1), mia, id_ajeno).await.unwrap());
        assert_eq!(
            obtener(&pool, Ambito::institucion(2), suya).await.unwrap().unwrap().bloques.len(),
            1,
            "se borró el bloque de otra institución acertando su id"
        );
    }
}
