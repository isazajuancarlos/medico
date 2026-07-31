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
use medico_dominio::{Ambito, Visibilidad};
use serde_json::Value;

/// Un acceso que se va a registrar. `ts` en UTC (RFC 3339); el llamante lo pone
/// para no atar el módulo al reloj.
///
/// El **objeto** del acceso tiene dos formas y nunca se mezclan (directiva #20:
/// no se rellena el que no aplica con un valor falso):
/// - **recurso único** (ver/editar una historia, un documento, una imagen):
///   lleva `paciente_id = Some` y `recurso_id = Some`, con `alcance = None`.
/// - **colección** (un listado o una exportación): lleva `alcance = Some`, con
///   `paciente_id = None` y `recurso_id = None` — no hay UN paciente ni UN id.
///
/// Un evento de recurso único NO inserta la clave `alcance`, así que su contenido
/// canónico es byte a byte el mismo de antes de existir las colecciones: las
/// cadenas ya selladas siguen verificando.
#[derive(Debug, Clone)]
pub struct Evento {
    pub usuario_id: i64,
    pub rol: String,
    /// Institución del usuario, o `None` para el superadmin.
    pub institucion_id: Option<i64>,
    /// `ver`, `editar`, `exportar`, `imprimir`, `listar`…
    pub accion: String,
    /// `historia`, `documento`, `imagen`, `paciente`, `pacientes`…
    pub recurso: String,
    /// Id del recurso único; `None` en una colección (no hay uno solo).
    pub recurso_id: Option<i64>,
    /// El paciente cuyo dato se tocó — la clave del rastro de la Ley 1581.
    /// `None` en una colección (el conjunto va en `alcance`).
    pub paciente_id: Option<i64>,
    /// El alcance de una colección (listado o exportación); `None` en un recurso
    /// único.
    pub alcance: Option<Alcance>,
    pub ts: String,
}

/// El alcance de un acceso a una **colección**. Un **listado** guarda a qué
/// ámbito se consultó, con qué filtro y cuántos resultados devolvió — pero NO los
/// ids: ver un nombre en una lista filtrada no es acceder al expediente, y
/// abanicar un eslabón por fila ahogaría la cadena. Una **exportación** guarda
/// ADEMÁS los `pacientes` que sacó, porque el dato salió y el titular tiene
/// derecho a saber si el suyo se exportó (Ley 1581); `por_paciente` los casa.
#[derive(Debug, Clone)]
pub struct Alcance {
    /// Descripción del ámbito consultado, p. ej. `"institucion:1"` o `"superadmin"`.
    pub ambito: String,
    /// El filtro/búsqueda aplicado, si hubo.
    pub filtro: Option<String>,
    /// Cuántos resultados devolvió el acceso.
    pub n: i64,
    /// Los pacientes que el acceso cubrió — SOLO en una exportación (el dato
    /// salió); vacío en un listado.
    pub pacientes: Vec<i64>,
}

impl Alcance {
    /// El objeto canónico anidado. Se construye con `serde_json::Map` (que sin
    /// `preserve_order` es un `BTreeMap`: claves ordenadas → determinista).
    fn a_valor(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("ambito".into(), Value::from(self.ambito.clone()));
        m.insert(
            "filtro".into(),
            self.filtro.clone().map_or(Value::Null, Value::from),
        );
        m.insert("n".into(), Value::from(self.n));
        m.insert("pacientes".into(), Value::from(self.pacientes.clone()));
        Value::Object(m)
    }
}

/// El contenido canónico que se hashea y firma. Única fuente: registrar y
/// verificar lo construyen igual, o el hash no cuadraría. La clave `alcance` solo
/// aparece en una colección — un recurso único produce el contenido de siempre.
fn contenido_de(e: &Evento) -> BTreeMap<String, Value> {
    debug_assert!(
        e.paciente_id.is_some() != e.alcance.is_some(),
        "un evento es de recurso único (paciente_id) O de colección (alcance), nunca ambos ni ninguno",
    );
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
    m.insert(
        "recurso_id".into(),
        e.recurso_id.map_or(Value::Null, Value::from),
    );
    m.insert(
        "paciente_id".into(),
        e.paciente_id.map_or(Value::Null, Value::from),
    );
    if let Some(a) = &e.alcance {
        m.insert("alcance".into(), a.a_valor());
    }
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

/// `true` si el paciente existe y es visible para el ámbito. Mismo guard que usan
/// historia y documento: la institución vive en `paciente`, no en la bitácora.
async fn paciente_es_del_ambito(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(paciente_id)
        .fetch_optional(pool)
        .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

/// El rastro de un paciente: sus accesos en orden. Para responder «¿quién vio mi
/// historia?» (derecho del titular, Ley 1581). Devuelve el JSON de cada evento.
///
/// Filtra por `ambito` en la propia capa de datos (defensa en profundidad, como
/// el resto de consultas de paciente): un ámbito ajeno recibe vacío —
/// indistinguible de «sin accesos»—, aunque el llamante olvide comprobarlo.
pub async fn por_paciente(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    if !paciente_es_del_ambito(pool, ambito, paciente_id).await? {
        return Ok(Vec::new());
    }
    // El rastro incluye DOS maneras de haber tocado al paciente: el acceso a un
    // recurso suyo (`paciente_id`) y la exportación que lo cubrió (su id dentro de
    // `alcance.pacientes`). Un listado no lo incluye — no guarda ids. El coalesce
    // a '[]' evita que `json_each` falle en los eventos sin alcance.
    let filas: Vec<(String,)> = sqlx::query_as(
        "SELECT contenido FROM acceso \
         WHERE json_extract(contenido, '$.paciente_id') = ? \
            OR EXISTS ( \
                 SELECT 1 FROM json_each(coalesce(json_extract(contenido, '$.alcance.pacientes'), '[]')) \
                 WHERE value = ? \
               ) \
         ORDER BY secuencia ASC",
    )
    .bind(paciente_id)
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
            recurso_id: Some(paciente),
            paciente_id: Some(paciente),
            alcance: None,
            ts: "2026-07-29T17:00:00Z".into(),
        }
    }

    /// Un acceso a una COLECCIÓN. `pacientes` vacío = listado; con ids = export.
    fn coleccion(usuario: i64, accion: &str, recurso: &str, n: i64, pacientes: Vec<i64>) -> Evento {
        Evento {
            usuario_id: usuario,
            rol: "medico".into(),
            institucion_id: Some(1),
            accion: accion.into(),
            recurso: recurso.into(),
            recurso_id: None,
            paciente_id: None,
            alcance: Some(Alcance {
                ambito: "institucion:1".into(),
                filtro: Some("apellido=Ruiz".into()),
                n,
                pacientes,
            }),
            ts: "2026-07-29T17:00:00Z".into(),
        }
    }

    async fn sembrar_paciente(pool: &crate::Pool, id: i64, institucion: i64) {
        // La institución debe existir (FK); OR IGNORE para poder sembrar varios
        // pacientes de la misma sin chocar.
        sqlx::query("INSERT OR IGNORE INTO institucion (id,nombre,nit) VALUES (?,'Inst',?)")
            .bind(institucion)
            .bind(format!("900{institucion}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) VALUES (?,'CC',?,'Ana','Ruiz','1990-01-01','F',?)",
        )
        .bind(id)
        .bind(format!("doc{id}"))
        .bind(institucion)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn por_paciente_respeta_el_ambito() {
        let pool = base().await;
        sembrar_paciente(&pool, 10, 1).await;
        for i in 0..2 {
            registrar(&pool, &evento(2, 10, &format!("ver_{i}")), None).await.unwrap();
        }
        // El ámbito dueño ve el rastro; el superadmin también.
        assert_eq!(por_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 2);
        assert_eq!(por_paciente(&pool, Ambito::Superadmin, 10).await.unwrap().len(), 2);
        // Un ámbito AJENO no ve nada, aunque conozca el id (indistinguible de
        // «sin accesos»). Es la defensa en profundidad de #88.
        assert!(por_paciente(&pool, Ambito::institucion(2), 10).await.unwrap().is_empty());
        // Sin ámbito, nada.
        assert!(por_paciente(&pool, Ambito::Ninguno, 10).await.unwrap().is_empty());
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
        sembrar_paciente(&pool, 10, 1).await;
        sembrar_paciente(&pool, 20, 1).await;
        let (_, sk) = guaca::firma::generar_claves();
        registrar(&pool, &evento(2, 10, "ver"), Some(&sk)).await.unwrap();
        registrar(&pool, &evento(3, 20, "ver"), Some(&sk)).await.unwrap();
        registrar(&pool, &evento(2, 10, "editar"), Some(&sk)).await.unwrap();
        assert_eq!(por_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 2);
        assert_eq!(por_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().len(), 1);
    }

    /// Un LISTADO no ensucia el rastro individual: se registra como colección
    /// (sin ids) y no aparece en `por_paciente` de nadie, aunque los haya visto en
    /// la lista. Solo el acceso directo al expediente cuenta como rastro del titular.
    #[tokio::test]
    async fn un_listado_no_ensucia_el_rastro_individual() {
        let pool = base().await;
        sembrar_paciente(&pool, 10, 1).await;
        let (_, sk) = guaca::firma::generar_claves();
        // El usuario lista pacientes (5 resultados) y luego abre el expediente de uno.
        registrar(&pool, &coleccion(2, "listar", "pacientes", 5, vec![]), Some(&sk)).await.unwrap();
        registrar(&pool, &evento(2, 10, "ver"), Some(&sk)).await.unwrap();
        // El rastro del paciente 10 trae SOLO el acceso directo, no el listado.
        let rastro = por_paciente(&pool, Ambito::institucion(1), 10).await.unwrap();
        assert_eq!(rastro.len(), 1, "el listado no debe aparecer en el rastro individual");
        assert!(rastro[0].contains("\"accion\":\"ver\""));
    }

    /// Una EXPORTACIÓN sí aparece en el rastro de CADA paciente que cubrió — el
    /// dato salió y el titular tiene derecho a saberlo (Ley 1581) — y NO en el de
    /// quien no cubrió. Prueba que discrimina (directiva #8).
    #[tokio::test]
    async fn una_exportacion_aparece_en_el_rastro_de_cada_afectado() {
        let pool = base().await;
        for p in [10, 20, 30] {
            sembrar_paciente(&pool, p, 1).await;
        }
        let (_, sk) = guaca::firma::generar_claves();
        // Se exportan 10 y 20 (no 30).
        registrar(&pool, &coleccion(2, "exportar", "rips", 2, vec![10, 20]), Some(&sk)).await.unwrap();
        assert_eq!(por_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 1);
        assert_eq!(por_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().len(), 1);
        // 30 no se exportó: su rastro está vacío.
        assert!(por_paciente(&pool, Ambito::institucion(1), 30).await.unwrap().is_empty());
        // Y el export solo lo ve el ámbito dueño.
        assert!(por_paciente(&pool, Ambito::institucion(2), 10).await.unwrap().is_empty());
    }

    /// El contenido de un recurso único no lleva la clave `alcance` y su
    /// `paciente_id` es un número — la forma canónica de siempre, para que las
    /// cadenas ya selladas antes de existir las colecciones sigan verificando.
    #[tokio::test]
    async fn el_recurso_unico_conserva_su_forma_canonica() {
        let pool = base().await;
        registrar(&pool, &evento(2, 10, "ver"), None).await.unwrap();
        let (cont,): (String,) =
            sqlx::query_as("SELECT contenido FROM acceso WHERE secuencia = 0")
                .fetch_one(&pool).await.unwrap();
        let v: Value = serde_json::from_str(&cont).unwrap();
        assert!(v.get("alcance").is_none(), "un recurso único no debe insertar 'alcance'");
        assert_eq!(v.get("paciente_id"), Some(&Value::from(10)));
        assert_eq!(v.get("recurso_id"), Some(&Value::from(10)));
    }

    /// Una cadena que MEZCLA recurso único, listado y exportación verifica entera:
    /// las tres formas conviven bajo el mismo sellado.
    #[tokio::test]
    async fn una_cadena_mixta_verifica() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        registrar(&pool, &evento(2, 10, "ver"), Some(&sk)).await.unwrap();
        registrar(&pool, &coleccion(2, "listar", "pacientes", 5, vec![]), Some(&sk)).await.unwrap();
        registrar(&pool, &coleccion(2, "exportar", "rips", 2, vec![10, 20]), Some(&sk)).await.unwrap();
        assert_eq!(verificar(&pool, Some(&vk)).await.unwrap(), Auditoria::Intacta);
    }

    /// Directiva #8: 100 accesos que compiten entre sí no bifurcan la cadena ni
    /// pierden registros. Con timeout, para distinguir un interbloqueo de lentitud.
    ///
    /// **Ocho escritores a la vez, no cien**, y el número no es tímido: es el
    /// tamaño del pool (`crate::abrir`). Lanzar las cien tareas de golpe NUNCA
    /// produjo cien escritores concurrentes —SQLite tiene un solo escritor y el
    /// pool, ocho conexiones—: las otras 92 hacían cola en `pool.acquire()`, que
    /// trae un plazo POR DEFECTO de 30 s que nadie eligió. Medido el 2026-07-31,
    /// esa cola tardaba 25,3 s en depuración, el 83 % del plazo; con la máquina
    /// cargada lo pasaba y la prueba moría con `PoolTimedOut` — es decir,
    /// anunciando que se había roto la cadena de auditoría cuando lo que faltó
    /// fue una conexión. Dos plazos para la misma espera, y el que mandaba era
    /// el que no estaba escrito.
    ///
    /// La carrera que aquí importa —dos escritores leen el mismo último eslabón
    /// y ambos intentan insertar la secuencia N— se da con dos concurrentes y se
    /// exprime con ocho. Los cien accesos siguen ocurriendo; lo que se quita es
    /// la sala de espera de sqlx, que no forma parte de lo que se afirma.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_accesos_con_ocho_escritores_a_la_vez_dejan_la_cadena_intacta() {
        /// Tantos como conexiones tiene el pool: más solo harían cola.
        const ESCRITORES: usize = 8;
        const TOTAL: usize = 100;

        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();

        let mut tareas = Vec::new();
        for w in 0..ESCRITORES {
            let p = pool.clone();
            let s = sk.clone();
            // Reparto exacto: los primeros `TOTAL % ESCRITORES` llevan uno más.
            let cuantos = TOTAL / ESCRITORES + usize::from(w < TOTAL % ESCRITORES);
            tareas.push(tokio::spawn(async move {
                for i in 0..cuantos {
                    registrar(&p, &evento(2, (i % 5) as i64, "ver"), Some(&s)).await?;
                }
                Ok::<(), sqlx::Error>(())
            }));
        }

        let plazo = std::time::Duration::from_secs(60);
        for t in tareas {
            let r = tokio::time::timeout(plazo, t).await
                .expect("un escritor no respondió a tiempo: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic");
            r.expect("cada acceso concurrente debe registrarse, con reintento si hubo carrera");
        }

        // 100 filas, secuencias 0..99 contiguas, cadena firmada íntegra.
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM acceso").fetch_one(&pool).await.unwrap();
        assert_eq!(n.0 as usize, TOTAL, "faltaron registros bajo concurrencia");
        assert_eq!(verificar(&pool, Some(&vk)).await.unwrap(), Auditoria::Intacta);
    }
}
