//! Referencia y contrarreferencia. Port a Rust de `backend/routers/remisiones.py`
//! y `contrarreferencia.py`.
//!
//! La REMISIÓN es una `orden` de tipo ESPECIALISTA con máquina de estados. La
//! CONTRARREFERENCIA es la respuesta desde la institución de destino; al crearla
//! se cierra la remisión (orden→Completada) si seguía abierta.
//!
//! Flujo INTER-institucional: el scoping va por la institución del PACIENTE
//! (ajeno == inexistente), como el resto. La orden no lleva institucion propia;
//! se llega a la del paciente por su historia.

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// El tipo de orden que ES una remisión.
///
/// Era `"ESPECIALISTA"`, y por eso una remisión creada desde la historia clínica
/// —que la crea como «Remisión Especialista», ver `historias.component.ts:1228`—
/// no aparecía jamás en esta bandeja. Ahora es el mismo valor en los dos sitios,
/// y el CHECK de la 0027 impide que se vuelvan a separar.
const TIPO_REMISION: &str = "Remisión Especialista";

/// Estados a los que se puede pasar desde `estado`, en orden de la máquina (el
/// camino normal primero). Vacío desde un estado final.
pub fn destinos(estado: &str) -> Vec<String> {
    let posibles: &[&str] = match estado {
        "Pendiente" => &["En Proceso", "Completada", "Cancelada"],
        "En Proceso" => &["Completada", "Cancelada"],
        _ => &[], // Completada, Cancelada: finales
    };
    posibles.iter().map(|s| s.to_string()).collect()
}

/// Si el estado cuenta como carga pendiente (no es final).
pub fn esta_abierta(estado: &str) -> bool {
    matches!(estado, "Pendiente" | "En Proceso")
}

fn ambito_sql(ambito: Ambito) -> Option<(&'static str, Option<i64>)> {
    match ambito {
        Ambito::Superadmin => Some(("1 = 1", None)),
        Ambito::Institucion(i) => Some(("p.institucion_id = ?", Some(i))),
        Ambito::Ninguno => None,
    }
}

// --- Remisión (vista sobre orden) --------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Remision {
    pub id: i64,
    /// **De quién es.** Igual que en `incapacidad::Incapacidad`: la pantalla
    /// enseña el nombre y el documento, pero la bitácora de la Ley 1581 ata el
    /// eslabón al titular por su id. Sin él, abrir una remisión no se podía
    /// anotar sin inventarse el paciente (directiva 20).
    pub paciente_id: i64,
    pub fecha_orden: String,
    pub estado: String,
    pub paciente: String,
    pub identificacion: String,
    pub descripcion: String,
    pub justificacion: String,
    pub especialidad_destino: Option<String>,
    pub institucion_destino: Option<String>,
    pub medico_ordena: Option<String>,
    pub codigo_cups: Option<String>,
    pub estados_posibles: Vec<String>,
    pub abierta: bool,
}

#[derive(sqlx::FromRow)]
struct RemFila {
    id: i64,
    paciente_id: i64,
    fecha_orden: String,
    estado: String,
    paciente: String,
    identificacion: String,
    descripcion: String,
    justificacion: String,
    especialidad_destino: Option<String>,
    institucion_destino: Option<String>,
    medico_ordena: Option<String>,
    codigo_cups: Option<String>,
}

impl From<RemFila> for Remision {
    fn from(f: RemFila) -> Self {
        let estados_posibles = destinos(&f.estado);
        let abierta = esta_abierta(&f.estado);
        Remision {
            id: f.id,
            paciente_id: f.paciente_id,
            fecha_orden: f.fecha_orden,
            estado: f.estado,
            paciente: f.paciente,
            identificacion: f.identificacion,
            descripcion: f.descripcion,
            justificacion: f.justificacion,
            especialidad_destino: f.especialidad_destino,
            institucion_destino: f.institucion_destino,
            medico_ordena: f.medico_ordena,
            codigo_cups: f.codigo_cups,
            estados_posibles,
            abierta,
        }
    }
}

const REM_COLS: &str = "o.id, h.paciente_id, o.fecha_orden, o.estado, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente, \
    p.numero_documento AS identificacion, COALESCE(o.descripcion,'') AS descripcion, \
    COALESCE(o.justificacion,'') AS justificacion, esp.nombre AS especialidad_destino, \
    idest.nombre AS institucion_destino, um.nombre AS medico_ordena, o.codigo_cups AS codigo_cups";

const REM_FROM: &str = "FROM orden o \
    JOIN historia h ON h.id = o.historia_id \
    JOIN paciente p ON p.id = h.paciente_id \
    LEFT JOIN especialidad esp ON esp.id = o.especialidad_destino_id \
    LEFT JOIN institucion idest ON idest.id = o.institucion_destino_id \
    LEFT JOIN usuario um ON um.id = h.medico_id AND um.institucion_id = p.institucion_id";

/// Remisiones (ordenes ESPECIALISTA) del ámbito, con filtros. De la más reciente
/// a la más antigua.
pub async fn remisiones_listar(
    pool: &crate::Pool,
    ambito: Ambito,
    estado: Option<&str>,
    desde: Option<&str>,
    hasta: Option<&str>,
    buscar: Option<&str>,
) -> Result<Vec<Remision>, sqlx::Error> {
    let Some((filtro, inst)) = ambito_sql(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = format!("SELECT {REM_COLS} {REM_FROM} WHERE o.tipo = ? AND {filtro}");
    if estado.is_some() {
        sql.push_str(" AND o.estado = ?");
    }
    if desde.is_some() {
        sql.push_str(" AND date(o.fecha_orden) >= date(?)");
    }
    if hasta.is_some() {
        sql.push_str(" AND date(o.fecha_orden) <= date(?)");
    }
    if buscar.is_some() {
        sql.push_str(" AND (p.nombre1 LIKE ? OR p.apellido1 LIKE ? OR p.numero_documento LIKE ?)");
    }
    sql.push_str(" ORDER BY o.fecha_orden DESC");

    let mut q = sqlx::query_as::<_, RemFila>(&sql).bind(TIPO_REMISION);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(e) = estado {
        q = q.bind(e.to_string());
    }
    if let Some(d) = desde {
        q = q.bind(d.to_string());
    }
    if let Some(h) = hasta {
        q = q.bind(h.to_string());
    }
    if let Some(b) = buscar {
        let aguja = format!("%{b}%");
        q = q.bind(aguja.clone()).bind(aguja.clone()).bind(aguja);
    }
    Ok(q.fetch_all(pool).await?.into_iter().map(Remision::from).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct ResumenRemisiones {
    pub total: i64,
    pub abiertas: i64,
    pub por_estado: BTreeMap<String, i64>,
}

/// Cuenta total, abiertas y por estado sobre el MISMO conjunto que lista.
pub async fn remisiones_resumen(
    pool: &crate::Pool,
    ambito: Ambito,
    desde: Option<&str>,
    hasta: Option<&str>,
    buscar: Option<&str>,
) -> Result<ResumenRemisiones, sqlx::Error> {
    let lista = remisiones_listar(pool, ambito, None, desde, hasta, buscar).await?;
    let mut por_estado: BTreeMap<String, i64> = BTreeMap::new();
    for e in ["Pendiente", "En Proceso", "Completada", "Cancelada"] {
        por_estado.insert(e.to_string(), 0);
    }
    let mut abiertas = 0;
    for r in &lista {
        *por_estado.entry(r.estado.clone()).or_insert(0) += 1;
        if r.abierta {
            abiertas += 1;
        }
    }
    Ok(ResumenRemisiones { total: lista.len() as i64, abiertas, por_estado })
}

/// Obtiene una remisión del ámbito por id (o `None` si ajena / no es remisión).
pub async fn remision_obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Remision>, sqlx::Error> {
    let Some((filtro, inst)) = ambito_sql(ambito) else {
        return Ok(None);
    };
    let sql = format!("SELECT {REM_COLS} {REM_FROM} WHERE o.id = ? AND o.tipo = ? AND {filtro}");
    let mut q = sqlx::query_as::<_, RemFila>(&sql).bind(id).bind(TIPO_REMISION);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_optional(pool).await?.map(Remision::from))
}

/// Cambia el estado de una remisión del ámbito, validando la transición.
/// `None` si la remisión es ajena/no existe o la transición no es válida.
pub async fn remision_cambiar_estado(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    nuevo: &str,
) -> Result<Option<Remision>, sqlx::Error> {
    let Some(actual) = remision_obtener(pool, ambito, id).await? else {
        return Ok(None);
    };
    if !destinos(&actual.estado).iter().any(|d| d == nuevo) {
        return Ok(None); // transición no permitida desde el estado actual
    }
    sqlx::query("UPDATE orden SET estado = ? WHERE id = ? AND tipo = ?")
        .bind(nuevo)
        .bind(id)
        .bind(TIPO_REMISION)
        .execute(pool)
        .await?;
    remision_obtener(pool, ambito, id).await
}

// --- Contrarreferencia -------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Contrarreferencia {
    pub id: i64,
    pub orden_id: Option<i64>,
    pub paciente_id: i64,
    pub fecha_creacion: String,
    pub diagnostico_final: String,
    pub procedimientos_realizados: Option<String>,
    pub tratamiento_instaurado: Option<String>,
    pub recomendaciones: Option<String>,
    pub requiere_seguimiento: bool,
    pub fecha_control_sugerida: Option<String>,
    pub recibida: bool,
    pub fecha_recepcion: Option<String>,
    pub paciente: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContrarreferenciaCrear {
    pub paciente_id: i64,
    pub orden_id: Option<i64>,
    pub diagnostico_final: String,
    pub procedimientos_realizados: Option<String>,
    pub tratamiento_instaurado: Option<String>,
    pub recomendaciones: Option<String>,
    #[serde(default)]
    pub requiere_seguimiento: bool,
    pub fecha_control_sugerida: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemisionAbierta {
    pub id: i64,
    pub fecha_orden: String,
    pub paciente: String,
    pub descripcion: String,
    pub estado: String,
}

const CR_COLS: &str = "cr.id, cr.orden_id, cr.paciente_id, cr.fecha_creacion, cr.diagnostico_final, \
    cr.procedimientos_realizados, cr.tratamiento_instaurado, cr.recomendaciones, cr.requiere_seguimiento, \
    cr.fecha_control_sugerida, cr.recibida, cr.fecha_recepcion, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente";

/// Contrarreferencias del ámbito, opcionalmente solo las no recibidas.
pub async fn contrarref_listar(
    pool: &crate::Pool,
    ambito: Ambito,
    solo_pendientes: bool,
) -> Result<Vec<Contrarreferencia>, sqlx::Error> {
    let Some((filtro, inst)) = ambito_sql(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT {CR_COLS} FROM contrarreferencia cr JOIN paciente p ON p.id = cr.paciente_id WHERE {filtro}"
    );
    if solo_pendientes {
        sql.push_str(" AND cr.recibida = 0");
    }
    sql.push_str(" ORDER BY cr.fecha_creacion DESC");
    let mut q = sqlx::query_as::<_, Contrarreferencia>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Remisiones ABIERTAS del ámbito: contra qué se puede contrarreferir.
pub async fn remisiones_abiertas(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<RemisionAbierta>, sqlx::Error> {
    let Some((filtro, inst)) = ambito_sql(ambito) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT o.id, o.fecha_orden, \
         (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente, \
         COALESCE(o.descripcion,'') AS descripcion, o.estado \
         FROM orden o JOIN historia h ON h.id = o.historia_id JOIN paciente p ON p.id = h.paciente_id \
         WHERE o.tipo = ? AND o.estado IN ('Pendiente','En Proceso') AND {filtro} \
         ORDER BY o.fecha_orden DESC"
    );
    let mut q = sqlx::query_as::<_, (i64, String, String, String, String)>(&sql).bind(TIPO_REMISION);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id, fecha_orden, paciente, descripcion, estado)| RemisionAbierta {
            id,
            fecha_orden,
            paciente,
            descripcion,
            estado,
        })
        .collect())
}

/// El paciente de una orden ESPECIALISTA del ámbito (para validar al crear).
async fn orden_remision_del_ambito(pool: &crate::Pool, ambito: Ambito, orden_id: i64) -> Result<Option<i64>, sqlx::Error> {
    let Some((filtro, inst)) = ambito_sql(ambito) else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT h.paciente_id FROM orden o JOIN historia h ON h.id = o.historia_id \
         JOIN paciente p ON p.id = h.paciente_id WHERE o.id = ? AND o.tipo = ? AND {filtro}"
    );
    let mut q = sqlx::query_as::<_, (i64,)>(&sql).bind(orden_id).bind(TIPO_REMISION);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_optional(pool).await?.map(|(pid,)| pid))
}

/// Crea una contrarreferencia. Sella institución de origen desde el ámbito y el
/// médico dado. Si trae `orden_id`, la orden debe ser una remisión del ámbito y
/// del mismo paciente; al crear, se cierra la remisión (orden→Completada) si
/// seguía abierta. `None` si el paciente es ajeno o la remisión no cuadra.
pub async fn contrarref_crear(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: i64,
    institucion_origen: Option<i64>,
    c: &ContrarreferenciaCrear,
) -> Result<Option<i64>, sqlx::Error> {
    // Paciente del ámbito.
    let pac: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(c.paciente_id)
        .fetch_optional(pool)
        .await?;
    if !pac.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si) {
        return Ok(None);
    }
    // Si hay orden, debe ser remisión del ámbito y del mismo paciente.
    if let Some(orden_id) = c.orden_id {
        match orden_remision_del_ambito(pool, ambito, orden_id).await? {
            Some(pid) if pid == c.paciente_id => {}
            _ => return Ok(None),
        }
    }

    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut tx = pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO contrarreferencia \
         (orden_id, paciente_id, institucion_origen_id, medico_contrarrefiere_id, fecha_creacion, \
          diagnostico_final, procedimientos_realizados, tratamiento_instaurado, recomendaciones, \
          requiere_seguimiento, fecha_control_sugerida) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(c.orden_id)
    .bind(c.paciente_id)
    .bind(institucion_origen)
    .bind(medico_id)
    .bind(&fecha)
    .bind(&c.diagnostico_final)
    .bind(&c.procedimientos_realizados)
    .bind(&c.tratamiento_instaurado)
    .bind(&c.recomendaciones)
    .bind(c.requiere_seguimiento)
    .bind(&c.fecha_control_sugerida)
    .execute(&mut *tx)
    .await?;
    // Cierra la remisión si seguía abierta.
    if let Some(orden_id) = c.orden_id {
        sqlx::query("UPDATE orden SET estado = 'Completada' WHERE id = ? AND estado IN ('Pendiente','En Proceso')")
            .bind(orden_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Some(r.last_insert_rowid()))
}

pub async fn contrarref_obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Contrarreferencia>, sqlx::Error> {
    let Some((filtro, inst)) = ambito_sql(ambito) else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {CR_COLS} FROM contrarreferencia cr JOIN paciente p ON p.id = cr.paciente_id \
         WHERE cr.id = ? AND {filtro}"
    );
    let mut q = sqlx::query_as::<_, Contrarreferencia>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Resultado de un acuse de recibo.
#[derive(Debug, PartialEq, Eq)]
pub enum Recepcion {
    Recibida,
    YaRecibida,
    NoExiste,
}

/// Acusa recibo de una contrarreferencia del ámbito. El primer acuse es el que
/// cuenta: un segundo no se sobrescribe.
pub async fn contrarref_recibir(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    usuario_id: i64,
) -> Result<Recepcion, sqlx::Error> {
    let Some(cr) = contrarref_obtener(pool, ambito, id).await? else {
        return Ok(Recepcion::NoExiste);
    };
    if cr.recibida {
        return Ok(Recepcion::YaRecibida);
    }
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    // Guarda de carrera: solo si sigue sin recibir.
    let filas = sqlx::query(
        "UPDATE contrarreferencia SET recibida = 1, fecha_recepcion = ?, recibida_por_id = ? \
         WHERE id = ? AND recibida = 0",
    )
    .bind(&fecha)
    .bind(usuario_id)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(if filas > 0 { Recepcion::Recibida } else { Recepcion::YaRecibida })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let url = format!(
            "sqlite:file:refer_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dr House','h@x.co','x','medico',1,1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F',1),(20,'CC','222','Beto','Paz','1985-01-01','M',2)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento,fecha) VALUES (100,'HC1',10,5,'m','e','J00','p','2026-06-10T09:00:00Z'),(200,'HC2',20,5,'m','e','J00','p','2026-06-11T09:00:00Z')").execute(&pool).await.unwrap();
        pool
    }

    async fn remision(pool: &crate::Pool, id: i64, historia: i64, estado: &str) {
        sqlx::query("INSERT INTO orden (id,historia_id,fecha_orden,estado,tipo,descripcion,justificacion) VALUES (?,?,?,?,'Remisión Especialista','ver cardiología','soplo')")
            .bind(id).bind(historia).bind("2026-06-10T10:00:00Z").bind(estado)
            .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn lista_remisiones_scoped_con_estados_posibles() {
        let pool = base().await;
        remision(&pool, 1, 100, "Pendiente").await;
        remision(&pool, 2, 200, "Pendiente").await; // institución 2
        let lista = remisiones_listar(&pool, Ambito::institucion(1), None, None, None, None).await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].paciente, "Ana Ruiz");
        assert!(lista[0].abierta);
        assert_eq!(lista[0].estados_posibles, ["En Proceso", "Completada", "Cancelada"]);
        assert!(remisiones_listar(&pool, Ambito::institucion(2), None, None, None, None).await.unwrap().len() == 1);
        assert!(remisiones_listar(&pool, Ambito::Ninguno, None, None, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cambiar_estado_valida_la_transicion() {
        let pool = base().await;
        remision(&pool, 1, 100, "Pendiente").await;
        // Transición inválida (Pendiente no salta a un estado inexistente).
        assert!(remision_cambiar_estado(&pool, Ambito::institucion(1), 1, "ARCHIVADA").await.unwrap().is_none());
        // Ajeno no cambia.
        assert!(remision_cambiar_estado(&pool, Ambito::institucion(2), 1, "En Proceso").await.unwrap().is_none());
        // Válida.
        let r = remision_cambiar_estado(&pool, Ambito::institucion(1), 1, "En Proceso").await.unwrap().unwrap();
        assert_eq!(r.estado, "En Proceso");
        assert_eq!(r.estados_posibles, ["Completada", "Cancelada"]);
        // Desde Completada (final) no hay a dónde ir.
        remision_cambiar_estado(&pool, Ambito::institucion(1), 1, "Completada").await.unwrap().unwrap();
        assert!(remision_cambiar_estado(&pool, Ambito::institucion(1), 1, "En Proceso").await.unwrap().is_none());
    }

    fn contrarref(paciente_id: i64, orden_id: Option<i64>) -> ContrarreferenciaCrear {
        ContrarreferenciaCrear {
            paciente_id,
            orden_id,
            diagnostico_final: "I35.0".into(),
            procedimientos_realizados: Some("ecocardiograma".into()),
            tratamiento_instaurado: None,
            recomendaciones: None,
            requiere_seguimiento: true,
            fecha_control_sugerida: Some("2026-07-01".into()),
        }
    }

    #[tokio::test]
    async fn crear_contrarreferencia_cierra_la_remision_abierta() {
        let pool = base().await;
        remision(&pool, 1, 100, "Pendiente").await;
        // Paciente ajeno: no se crea.
        assert!(contrarref_crear(&pool, Ambito::institucion(1), 5, Some(1), &contrarref(20, Some(1))).await.unwrap().is_none());
        // Contra la remisión 1 (paciente 10): se crea y la remisión pasa a Completada.
        let id = contrarref_crear(&pool, Ambito::institucion(1), 5, Some(1), &contrarref(10, Some(1))).await.unwrap().unwrap();
        assert_eq!(remision_obtener(&pool, Ambito::institucion(1), 1).await.unwrap().unwrap().estado, "Completada");
        // Ya no está entre las abiertas.
        assert!(remisiones_abiertas(&pool, Ambito::institucion(1)).await.unwrap().is_empty());
        // Se lista para su institución, enriquecida.
        let cr = contrarref_obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(cr.paciente, "Ana Ruiz");
        assert!(cr.requiere_seguimiento);
        assert!(contrarref_obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn recibir_es_idempotente() {
        let pool = base().await;
        let id = contrarref_crear(&pool, Ambito::institucion(1), 5, None, &contrarref(10, None)).await.unwrap().unwrap();
        assert_eq!(contrarref_recibir(&pool, Ambito::institucion(2), id, 5).await.unwrap(), Recepcion::NoExiste); // ajeno
        assert_eq!(contrarref_recibir(&pool, Ambito::institucion(1), id, 5).await.unwrap(), Recepcion::Recibida);
        // El segundo acuse no sobrescribe.
        assert_eq!(contrarref_recibir(&pool, Ambito::institucion(1), id, 5).await.unwrap(), Recepcion::YaRecibida);
        assert!(contrarref_obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap().recibida);
    }
}
