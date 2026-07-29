//! Bitácora de acceso a dato personal (Ley 1581 / OWASP A09).
//!
//! Cada acceso a un dato de paciente se registra como un eslabón **encadenado
//! por hash y firmado** (sobre `guaca::auditoria`): borrar o reordenar una
//! entrada rompe la cadena, no solo alterarla. Es el rastro que la ley de datos
//! personales exige y que no se puede maquillar por debajo.
//!
//! **Dos niveles de garantía, según haya clave (`MEDICO_SIGNING_KEY`):**
//! - Sin clave: la cadena se encadena por hash igual. Detecta manipulación
//!   ACCIDENTAL o parcial, pero alguien con escritura en la BD podría rehacer
//!   toda la cadena (los hashes son públicos). Es lo mínimo.
//! - Con clave: cada eslabón va firmado. Ya no se puede forjar ni rehacer sin la
//!   clave privada. Es lo que se debe usar en producción — configúrala desde el
//!   primer día, porque un eslabón sin firma no verifica bajo una clave.

use std::collections::BTreeMap;

use guaca::auditoria::{self, Auditoria, Entrada, GENESIS};
use serde_json::Value;

/// Un acceso que se va a registrar. `ts` en UTC (RFC 3339); el llamante lo pone
/// para no atar el módulo al reloj.
#[derive(Debug, Clone)]
pub struct Evento {
    pub usuario_id: i64,
    pub rol: String,
    /// Institución del usuario, o `None` para el superadmin.
    pub institucion_id: Option<i64>,
    /// `ver`, `editar`, `exportar`, `imprimir`…
    pub accion: String,
    /// `historia`, `documento`, `imagen`, `paciente`…
    pub recurso: String,
    pub recurso_id: i64,
    /// El paciente cuyo dato se tocó — la clave del rastro de la Ley 1581.
    pub paciente_id: i64,
    pub ts: String,
}

/// El contenido canónico que se hashea y firma. Única fuente: registrar y
/// verificar lo construyen igual, o el hash no cuadraría.
fn contenido_de(e: &Evento) -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    m.insert("v".into(), Value::from(1));
    m.insert("usuario_id".into(), Value::from(e.usuario_id));
    m.insert("rol".into(), Value::from(e.rol.clone()));
    m.insert(
        "institucion_id".into(),
        e.institucion_id.map_or(Value::Null, Value::from),
    );
    m.insert("accion".into(), Value::from(e.accion.clone()));
    m.insert("recurso".into(), Value::from(e.recurso.clone()));
    m.insert("recurso_id".into(), Value::from(e.recurso_id));
    m.insert("paciente_id".into(), Value::from(e.paciente_id));
    m.insert("ts".into(), Value::from(e.ts.clone()));
    m
}

fn a_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn de_hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// `true` si el error es una carrera de escritura recuperable (colisión de
/// secuencia o base bloqueada): dos accesos concurrentes leyeron el mismo último
/// eslabón. Se reintenta; NO se pierde el registro.
fn es_conflicto(e: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db) = e {
        let m = db.message().to_ascii_lowercase();
        return m.contains("unique") || m.contains("locked") || m.contains("busy");
    }
    false
}

/// Inserta el eslabón sobre una conexión que YA está en una transacción de
/// escritura (BEGIN IMMEDIATE). Separada para poder hacer COMMIT/ROLLBACK limpio.
async fn insertar(
    conn: &mut sqlx::SqliteConnection,
    evento: &Evento,
    sk_hex: Option<&str>,
) -> Result<(), sqlx::Error> {
    let ultimo: Option<(i64, String)> =
        sqlx::query_as("SELECT secuencia, hash FROM acceso ORDER BY secuencia DESC LIMIT 1")
            .fetch_optional(&mut *conn)
            .await?;
    let (secuencia, anterior) = match ultimo {
        Some((s, h)) => (
            s + 1,
            de_hex32(&h).expect("un hash almacenado por nosotros siempre es hex de 32 bytes"),
        ),
        None => (0, GENESIS),
    };

    let cont = contenido_de(evento);
    // La firma: real si hay clave válida, "" si no. El hash lo calcula sellar_con
    // siempre, así que la cadena existe con o sin clave.
    let firmar = |pre: &[u8]| Some(sk_hex.and_then(|s| guaca::firma::firmar(pre, s)).unwrap_or_default());
    let sello = auditoria::sellar_con(secuencia as u64, anterior, &cont, firmar)
        .expect("la clausura de firma siempre devuelve Some");

    sqlx::query("INSERT INTO acceso (secuencia, hash_anterior, contenido, hash, firma) VALUES (?,?,?,?,?)")
        .bind(secuencia)
        .bind(a_hex(&anterior))
        .bind(serde_json::to_string(&cont).expect("un BTreeMap<String,Value> siempre serializa"))
        .bind(a_hex(&sello.hash))
        .bind(&sello.firma)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn intentar(pool: &crate::Pool, evento: &Evento, sk_hex: Option<&str>) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;
    // busy_timeout: ante contención, ESPERA en vez de fallar al instante.
    sqlx::query("PRAGMA busy_timeout = 5000").execute(&mut *conn).await?;
    // BEGIN IMMEDIATE toma el lock de escritura AL INICIO, antes del SELECT. Sin
    // esto, leer-el-último (lock de lectura) y luego insertar (subir a escritura)
    // se interbloquea con otra conexión que hace lo mismo: ninguna puede subir.
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;
    match insertar(&mut conn, evento, sk_hex).await {
        Ok(()) => {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            Ok(())
        }
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(e)
        }
    }
}

/// Registra un acceso, encadenado al último y firmado si hay clave. Reintenta
/// ante una carrera de escritura (SQLite es de escritor único): dos accesos
/// concurrentes nunca bifurcan la cadena ni pierden un registro.
pub async fn registrar(pool: &crate::Pool, evento: &Evento, sk_hex: Option<&str>) -> Result<(), sqlx::Error> {
    for _ in 0..16 {
        match intentar(pool, evento, sk_hex).await {
            Ok(()) => return Ok(()),
            Err(e) if es_conflicto(&e) => continue,
            Err(e) => return Err(e),
        }
    }
    // Un último intento: si vuelve a fallar, que el error suba.
    intentar(pool, evento, sk_hex).await
}

/// Verifica la cadena entera: eslabones, hashes, firmas y contigüidad.
///
/// Con `vk_hex = Some`, EXIGE que cada eslabón esté firmado y valide — un evento
/// sin firma (downgrade) se reporta como roto. Con `None`, comprueba solo el
/// encadenamiento por hash (no hay con qué verificar firmas).
pub async fn verificar(pool: &crate::Pool, vk_hex: Option<&str>) -> Result<Auditoria, sqlx::Error> {
    let filas: Vec<(i64, String, String, String, String)> = sqlx::query_as(
        "SELECT secuencia, hash_anterior, contenido, hash, firma FROM acceso ORDER BY secuencia ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut entradas = Vec::with_capacity(filas.len());
    for (sec, ha, cont, h, firma) in &filas {
        entradas.push(Entrada {
            secuencia: *sec as u64,
            hash_anterior: de_hex32(ha).unwrap_or([1u8; 32]), // valor imposible → rompe eslabón si está corrupto
            contenido: serde_json::from_str(cont).unwrap_or_default(),
            hash: de_hex32(h).unwrap_or([1u8; 32]),
            firma: firma.clone(),
        });
    }

    let verif = |pre: &[u8], tok: &str| match vk_hex {
        // Con clave: firma obligatoria y válida. Un eslabón sin firmar no cuela.
        Some(vk) => !tok.is_empty() && guaca::firma::verificar(pre, tok, vk) == guaca::firma::Verificacion::Valida,
        // Sin clave: solo el hash-encadenado (ya comprobado por verificar_con).
        None => true,
    };
    Ok(auditoria::verificar_con(&entradas, verif))
}

/// El rastro de un paciente: sus accesos en orden. Para responder «¿quién vio mi
/// historia?» (derecho del titular, Ley 1581). Devuelve el JSON de cada evento.
pub async fn por_paciente(pool: &crate::Pool, paciente_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let filas: Vec<(String,)> = sqlx::query_as(
        "SELECT contenido FROM acceso WHERE json_extract(contenido, '$.paciente_id') = ? ORDER BY secuencia ASC",
    )
    .bind(paciente_id)
    .fetch_all(pool)
    .await?;
    Ok(filas.into_iter().map(|(c,)| c).collect())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Cada test, su propia BD en memoria con caché COMPARTIDA — así las 8
    /// conexiones del pool ven la misma base (a diferencia de `sqlite::memory:`,
    /// que da una por conexión y rompería la prueba de concurrencia).
    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        let url = format!(
            "sqlite:file:acceso_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        crate::abrir(&url).await.unwrap()
    }

    fn evento(usuario: i64, paciente: i64, accion: &str) -> Evento {
        Evento {
            usuario_id: usuario,
            rol: "medico".into(),
            institucion_id: Some(1),
            accion: accion.into(),
            recurso: "historia".into(),
            recurso_id: paciente,
            paciente_id: paciente,
            ts: "2026-07-29T17:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn una_cadena_de_accesos_verifica() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        for i in 0..3 {
            registrar(&pool, &evento(2, 10, &format!("ver_{i}")), Some(&sk)).await.unwrap();
        }
        assert_eq!(verificar(&pool, Some(&vk)).await.unwrap(), Auditoria::Intacta);
    }

    #[tokio::test]
    async fn alterar_un_evento_en_la_base_rompe_la_cadena() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        for _ in 0..3 {
            registrar(&pool, &evento(2, 10, "ver"), Some(&sk)).await.unwrap();
        }
        // Alguien edita el contenido del eslabón 1 por debajo de la app.
        sqlx::query("UPDATE acceso SET contenido = json_set(contenido, '$.usuario_id', 999) WHERE secuencia = 1")
            .execute(&pool).await.unwrap();
        assert!(matches!(
            verificar(&pool, Some(&vk)).await.unwrap(),
            Auditoria::Rota { secuencia: 1, .. }
        ));
    }

    #[tokio::test]
    async fn borrar_un_eslabon_intermedio_se_detecta() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        for _ in 0..3 {
            registrar(&pool, &evento(2, 10, "ver"), Some(&sk)).await.unwrap();
        }
        sqlx::query("DELETE FROM acceso WHERE secuencia = 1").execute(&pool).await.unwrap();
        assert!(matches!(
            verificar(&pool, Some(&vk)).await.unwrap(),
            Auditoria::Rota { .. }
        ));
    }

    #[tokio::test]
    async fn sin_clave_se_encadena_igual() {
        let pool = base().await;
        for _ in 0..3 {
            registrar(&pool, &evento(2, 10, "ver"), None).await.unwrap();
        }
        // Sin clave: verifica por hash. Con clave, esos eventos sin firma NO cuelan.
        assert_eq!(verificar(&pool, None).await.unwrap(), Auditoria::Intacta);
        let (vk, _) = guaca::firma::generar_claves();
        assert!(matches!(
            verificar(&pool, Some(&vk)).await.unwrap(),
            Auditoria::Rota { secuencia: 0, .. }
        ));
    }

    #[tokio::test]
    async fn el_rastro_por_paciente_filtra() {
        let pool = base().await;
        let (_, sk) = guaca::firma::generar_claves();
        registrar(&pool, &evento(2, 10, "ver"), Some(&sk)).await.unwrap();
        registrar(&pool, &evento(3, 20, "ver"), Some(&sk)).await.unwrap();
        registrar(&pool, &evento(2, 10, "editar"), Some(&sk)).await.unwrap();
        assert_eq!(por_paciente(&pool, 10).await.unwrap().len(), 2);
        assert_eq!(por_paciente(&pool, 20).await.unwrap().len(), 1);
    }

    /// Directiva #8: 100+ accesos CONCURRENTES no bifurcan la cadena ni pierden
    /// registros. Con timeout, para distinguir un interbloqueo de lentitud.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_accesos_concurrentes_dejan_la_cadena_intacta() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let mut tareas = Vec::new();
        for i in 0..100 {
            let p = pool.clone();
            let s = sk.clone();
            tareas.push(tokio::spawn(async move {
                registrar(&p, &evento(2, (i % 5) as i64, "ver"), Some(&s)).await
            }));
        }
        let plazo = std::time::Duration::from_secs(60);
        for t in tareas {
            let r = tokio::time::timeout(plazo, t).await
                .expect("un acceso no respondió a tiempo: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic");
            r.expect("cada acceso concurrente debe registrarse, con reintento si hubo carrera");
        }
        // 100 filas, secuencias 0..99 contiguas, cadena firmada íntegra.
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM acceso").fetch_one(&pool).await.unwrap();
        assert_eq!(n.0, 100, "faltaron registros bajo concurrencia");
        assert_eq!(verificar(&pool, Some(&vk)).await.unwrap(), Auditoria::Intacta);
    }
}
