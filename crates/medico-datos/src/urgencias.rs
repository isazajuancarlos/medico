//! Urgencias: cola de triage, registro y egreso. Port a Rust de
//! `backend/routers/urgencias.py`.
//!
//! Como el resto del acceso a datos, el ámbito se aplica por la institución del
//! PACIENTE: un ajeno es indistinguible de uno inexistente. El triage se guarda
//! como CLAVE (`TRIAGE_1..5`) para que ordenar por ella dé la prioridad correcta
//! —rojo primero—; el original ordenaba por la etiqueta y quedaba alfabético.

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

/// Niveles de triage válidos, de más a menos urgente. El orden de este arreglo
/// es el de prioridad, y coincide con el orden lexicográfico de las claves.
pub const TRIAGES: [&str; 5] = ["TRIAGE_1", "TRIAGE_2", "TRIAGE_3", "TRIAGE_4", "TRIAGE_5"];

/// Destinos de salida válidos. `destino_salida` NULL = sigue en la cola.
pub const DESTINOS: [&str; 6] =
    ["DOMICILIO", "HOSPITALIZACION", "REMISION", "OBSERVACION", "FUGA", "FALLECIDO"];

/// La etiqueta humana de un nivel de triage (para mostrar; el orden va por clave).
pub fn etiqueta_triage(clave: &str) -> &'static str {
    match clave {
        "TRIAGE_1" => "Rojo - Resucitación",
        "TRIAGE_2" => "Naranja - Emergencia",
        "TRIAGE_3" => "Amarillo - Urgente",
        "TRIAGE_4" => "Verde - Menos Urgente",
        "TRIAGE_5" => "Azul - No Urgente",
        _ => "Desconocido",
    }
}

// `u.` en todas las columnas: la consulta une con `paciente`, y `id`/`paciente_id`
// existen en ambas tablas. SQLite devuelve el nombre sin calificar, así que
// `FromRow` sigue mapeando por nombre.
const COLS: &str = "u.id, u.paciente_id, u.triage, u.motivo_consulta, u.signos_vitales, \
    u.enfermera_triage_id, u.fecha_ingreso, u.diagnostico_ingreso, u.destino_salida, \
    u.observaciones_egreso, u.fecha_egreso";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Atencion {
    pub id: i64,
    pub paciente_id: i64,
    pub triage: String,
    pub motivo_consulta: String,
    pub signos_vitales: Option<String>,
    pub enfermera_triage_id: i64,
    pub fecha_ingreso: String,
    pub diagnostico_ingreso: Option<String>,
    pub destino_salida: Option<String>,
    pub observaciones_egreso: Option<String>,
    pub fecha_egreso: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TriageNuevo {
    pub paciente_id: i64,
    pub triage: String,
    pub motivo_consulta: String,
    pub signos_vitales: Option<String>,
}

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

/// La cola: atenciones AÚN sin egreso, del ámbito, por prioridad de triage y
/// luego por antigüedad. Vacía para el ámbito Ninguno.
pub async fn cola(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Atencion>, sqlx::Error> {
    let (filtro, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(i) => ("p.institucion_id = ?", Some(i)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let sql = format!(
        "SELECT {COLS} FROM atencion_urgencias u JOIN paciente p ON p.id = u.paciente_id \
         WHERE u.destino_salida IS NULL AND {filtro} \
         ORDER BY u.triage ASC, u.fecha_ingreso ASC"
    );
    let mut q = sqlx::query_as::<_, Atencion>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Registra un triage. `None` si el triage no es válido o el paciente no es del
/// ámbito (indistinguible de inexistente). `fecha_ingreso` la pone el servidor.
pub async fn registrar(
    pool: &crate::Pool,
    ambito: Ambito,
    enfermera_id: i64,
    n: &TriageNuevo,
) -> Result<Option<i64>, sqlx::Error> {
    if !TRIAGES.contains(&n.triage.as_str()) {
        return Ok(None);
    }
    if !paciente_es_del_ambito(pool, ambito, n.paciente_id).await? {
        return Ok(None);
    }
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let r = sqlx::query(
        "INSERT INTO atencion_urgencias \
         (paciente_id, triage, motivo_consulta, signos_vitales, enfermera_triage_id, fecha_ingreso) \
         VALUES (?,?,?,?,?,?)",
    )
    .bind(n.paciente_id)
    .bind(&n.triage)
    .bind(&n.motivo_consulta)
    .bind(&n.signos_vitales)
    .bind(enfermera_id)
    .bind(&fecha)
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Registra el egreso de una atención abierta del ámbito. `false` si el destino
/// no es válido, si no existe/está cerrada, o si es de otra institución.
pub async fn egreso(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    destino: &str,
    observaciones: Option<&str>,
) -> Result<bool, sqlx::Error> {
    if !DESTINOS.contains(&destino) {
        return Ok(false);
    }
    // La institución sale de la MISMA consulta que exige que esté abierta: entre
    // comprobar y actualizar no cabe un olvido.
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM atencion_urgencias u JOIN paciente p ON p.id = u.paciente_id \
         WHERE u.id = ? AND u.destino_salida IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((inst,)) = fila else {
        return Ok(false);
    };
    if ambito.puede_ver(Some(inst)) != Visibilidad::Si {
        return Ok(false);
    }
    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let r = sqlx::query(
        "UPDATE atencion_urgencias SET destino_salida = ?, observaciones_egreso = ?, fecha_egreso = ? \
         WHERE id = ? AND destino_salida IS NULL",
    )
    .bind(destino)
    .bind(observaciones)
    .bind(&fecha)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let url = format!(
            "sqlite:file:urg_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Enf','e@x.co','h','enfermera',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        pool
    }

    fn triage(paciente_id: i64, nivel: &str) -> TriageNuevo {
        TriageNuevo {
            paciente_id,
            triage: nivel.into(),
            motivo_consulta: "dolor".into(),
            signos_vitales: None,
        }
    }

    #[tokio::test]
    async fn la_cola_ordena_por_prioridad_no_por_antiguedad() {
        let pool = base().await;
        // Se registra primero el MENOS urgente; la cola debe poner el rojo primero.
        registrar(&pool, Ambito::institucion(1), 5, &triage(10, "TRIAGE_4")).await.unwrap();
        registrar(&pool, Ambito::institucion(1), 5, &triage(10, "TRIAGE_1")).await.unwrap();
        registrar(&pool, Ambito::institucion(1), 5, &triage(10, "TRIAGE_3")).await.unwrap();
        let cola = cola(&pool, Ambito::institucion(1)).await.unwrap();
        let orden: Vec<&str> = cola.iter().map(|a| a.triage.as_str()).collect();
        assert_eq!(orden, ["TRIAGE_1", "TRIAGE_3", "TRIAGE_4"]);
    }

    #[tokio::test]
    async fn no_se_registra_triage_de_paciente_ajeno_ni_nivel_invalido() {
        let pool = base().await;
        // Paciente 20 es de la institución 2: la 1 no lo registra.
        assert!(registrar(&pool, Ambito::institucion(1), 5, &triage(20, "TRIAGE_1")).await.unwrap().is_none());
        // Nivel inventado: rechazado.
        assert!(registrar(&pool, Ambito::institucion(1), 5, &triage(10, "TRIAGE_9")).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn la_cola_es_scoped_por_institucion() {
        let pool = base().await;
        registrar(&pool, Ambito::institucion(1), 5, &triage(10, "TRIAGE_2")).await.unwrap();
        assert_eq!(cola(&pool, Ambito::institucion(1)).await.unwrap().len(), 1);
        assert!(cola(&pool, Ambito::institucion(2)).await.unwrap().is_empty());
        assert!(cola(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert_eq!(cola(&pool, Ambito::Superadmin).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn el_egreso_saca_de_la_cola_y_respeta_el_ambito() {
        let pool = base().await;
        let id = registrar(&pool, Ambito::institucion(1), 5, &triage(10, "TRIAGE_2")).await.unwrap().unwrap();
        // Un ámbito ajeno no egresa lo que no es suyo.
        assert!(!egreso(&pool, Ambito::institucion(2), id, "DOMICILIO", None).await.unwrap());
        // Destino inventado: rechazado.
        assert!(!egreso(&pool, Ambito::institucion(1), id, "A_CASA", None).await.unwrap());
        // El dueño sí; y entonces sale de la cola.
        assert!(egreso(&pool, Ambito::institucion(1), id, "DOMICILIO", Some("estable")).await.unwrap());
        assert!(cola(&pool, Ambito::institucion(1)).await.unwrap().is_empty());
        // Egresar dos veces no hace nada.
        assert!(!egreso(&pool, Ambito::institucion(1), id, "DOMICILIO", None).await.unwrap());
    }
}
