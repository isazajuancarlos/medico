//! Hospitalización: camas, ingreso y egreso. Port a Rust de
//! `backend/routers/hospitalizacion.py`.
//!
//! Ingresar y egresar mueven DOS filas —la hospitalización y el estado de la
//! cama— y van en UNA transacción: nunca queda una cama ocupada sin
//! hospitalización, ni al revés. La toma de la cama usa
//! `UPDATE ... WHERE estado='DISPONIBLE'` y comprueba filas afectadas, para que
//! dos ingresos simultáneos a la misma cama no la asignen dos veces.
//!
//! Las hospitalizaciones se filtran por la institución del PACIENTE (ajeno ==
//! inexistente). Las camas son globales, como en el original.

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

const VIAS: [&str; 3] = ["URGENCIAS", "PROGRAMADA", "REMISION"];
const TIPOS_EGRESO: [&str; 4] = ["ALTA_MEDICA", "ALTA_VOLUNTARIA", "REMISION", "DEFUNCION"];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Cama {
    pub id: i64,
    pub numero: String,
    pub piso: String,
    pub servicio: String,
    pub estado: String,
    pub tiene_oxigeno: bool,
    pub tiene_monitor: bool,
    pub es_aislamiento: bool,
}

/// Una hospitalización con los nombres ya unidos, como la esperaba la pantalla.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Hospitalizacion {
    pub id: i64,
    pub paciente_id: i64,
    pub cama_id: i64,
    pub medico_tratante_id: i64,
    pub fecha_ingreso: String,
    pub fecha_egreso: Option<String>,
    pub via_ingreso: String,
    pub estado: String,
    pub diagnostico_ingreso: Option<String>,
    pub motivo_ingreso: Option<String>,
    pub observaciones_ingreso: Option<String>,
    pub diagnostico_egreso: Option<String>,
    pub resumen_clinico: Option<String>,
    pub paciente_nombre: String,
    pub paciente_identificacion: String,
    pub cama_numero: String,
    pub cama_piso: String,
    pub cama_servicio: String,
    pub medico_nombre: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MedicoBasico {
    pub id: i64,
    pub nombre_completo: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ingreso {
    pub paciente_id: i64,
    pub cama_id: i64,
    pub medico_id: i64,
    pub via_ingreso: Option<String>,
    pub diagnostico_ingreso: Option<String>,
    pub motivo_ingreso: Option<String>,
    pub observaciones_ingreso: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Egreso {
    pub tipo_egreso: String,
    pub diagnostico_egreso: Option<String>,
    pub resumen_clinico: Option<String>,
}

const SELECT_ENRIQUECIDO: &str = "\
    SELECT h.id, h.paciente_id, h.cama_id, h.medico_tratante_id, h.fecha_ingreso, \
      h.fecha_egreso, h.via_ingreso, h.estado, h.diagnostico_ingreso, h.motivo_ingreso, \
      h.observaciones_ingreso, h.diagnostico_egreso, h.resumen_clinico, \
      (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente_nombre, \
      p.numero_documento AS paciente_identificacion, \
      c.numero AS cama_numero, c.piso AS cama_piso, c.servicio AS cama_servicio, \
      u.nombre AS medico_nombre \
    FROM hospitalizacion h \
    JOIN paciente p ON p.id = h.paciente_id \
    JOIN cama c ON c.id = h.cama_id \
    JOIN usuario u ON u.id = h.medico_tratante_id";

/// Hospitalizaciones del ámbito con un estado dado (por defecto ACTIVA), de la
/// más reciente a la más antigua. Vacío para el ámbito Ninguno.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    estado: Option<&str>,
) -> Result<Vec<Hospitalizacion>, sqlx::Error> {
    let (filtro, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(i) => ("p.institucion_id = ?", Some(i)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let estado = estado.unwrap_or("ACTIVA");
    let sql = format!(
        "{SELECT_ENRIQUECIDO} WHERE h.estado = ? AND {filtro} ORDER BY h.fecha_ingreso DESC"
    );
    let mut q = sqlx::query_as::<_, Hospitalizacion>(&sql).bind(estado);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Camas, opcionalmente por estado y/o servicio. Globales, ordenadas por piso y
/// número.
pub async fn listar_camas(
    pool: &crate::Pool,
    estado: Option<&str>,
    servicio: Option<&str>,
) -> Result<Vec<Cama>, sqlx::Error> {
    let mut sql = String::from("SELECT id, numero, piso, servicio, estado, tiene_oxigeno, tiene_monitor, es_aislamiento FROM cama WHERE 1 = 1");
    if estado.is_some() {
        sql.push_str(" AND estado = ?");
    }
    if servicio.is_some() {
        sql.push_str(" AND servicio = ?");
    }
    sql.push_str(" ORDER BY piso, numero");
    let mut q = sqlx::query_as::<_, Cama>(&sql);
    if let Some(e) = estado {
        q = q.bind(e);
    }
    if let Some(s) = servicio {
        q = q.bind(s);
    }
    q.fetch_all(pool).await
}

/// Médicos y anestesiólogos activos del ámbito, para elegir tratante.
pub async fn listar_medicos(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<MedicoBasico>, sqlx::Error> {
    let (filtro, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(i) => ("institucion_id = ?", Some(i)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let sql = format!(
        "SELECT id, nombre AS nombre_completo FROM usuario \
         WHERE rol IN ('medico','anestesiologo') AND activo = 1 AND {filtro} ORDER BY nombre"
    );
    let mut q = sqlx::query_as::<_, MedicoBasico>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Ingresa un paciente a una cama disponible. `None` si el paciente no es del
/// ámbito, la vía no es válida, la cama no existe o no está disponible. La cama
/// pasa a OCUPADA en la misma transacción.
pub async fn ingresar(
    pool: &crate::Pool,
    ambito: Ambito,
    i: &Ingreso,
) -> Result<Option<i64>, sqlx::Error> {
    let via = i.via_ingreso.as_deref().unwrap_or("PROGRAMADA");
    if !VIAS.contains(&via) {
        return Ok(None);
    }
    let mut tx = pool.begin().await?;

    // El paciente debe ser del ámbito.
    let inst: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(i.paciente_id)
        .fetch_optional(&mut *tx)
        .await?;
    let del_ambito = inst.is_some_and(|(x,)| ambito.puede_ver(Some(x)) == Visibilidad::Si);
    if !del_ambito {
        return Ok(None);
    }

    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let r = sqlx::query(
        "INSERT INTO hospitalizacion \
         (paciente_id, cama_id, medico_tratante_id, fecha_ingreso, via_ingreso, estado, \
          diagnostico_ingreso, motivo_ingreso, observaciones_ingreso) \
         VALUES (?,?,?,?,?, 'ACTIVA', ?,?,?)",
    )
    .bind(i.paciente_id)
    .bind(i.cama_id)
    .bind(i.medico_id)
    .bind(&fecha)
    .bind(via)
    .bind(&i.diagnostico_ingreso)
    .bind(&i.motivo_ingreso)
    .bind(&i.observaciones_ingreso)
    .execute(&mut *tx)
    .await;
    let id = match r {
        Ok(r) => r.last_insert_rowid(),
        // FK inválida (cama o médico inexistente): no es un error del servidor.
        Err(sqlx::Error::Database(_)) => return Ok(None),
        Err(e) => return Err(e),
    };

    // Toma de la cama con guarda de carrera: solo si sigue DISPONIBLE. Si otro
    // ingreso la tomó entremedias, filas=0 y se deshace todo.
    let tomada = sqlx::query("UPDATE cama SET estado = 'OCUPADA' WHERE id = ? AND estado = 'DISPONIBLE'")
        .bind(i.cama_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if tomada == 0 {
        tx.rollback().await?;
        return Ok(None);
    }
    tx.commit().await?;
    Ok(Some(id))
}

/// Registra el egreso de una hospitalización ACTIVA del ámbito y libera la cama.
/// `false` si el tipo de egreso no es válido, o si no existe / no está activa /
/// es de otra institución.
pub async fn egreso(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    e: &Egreso,
) -> Result<bool, sqlx::Error> {
    if !TIPOS_EGRESO.contains(&e.tipo_egreso.as_str()) {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;

    // Institución y cama salen de la MISMA consulta que exige que esté ACTIVA.
    let fila: Option<(i64, i64)> = sqlx::query_as(
        "SELECT p.institucion_id, h.cama_id FROM hospitalizacion h \
         JOIN paciente p ON p.id = h.paciente_id \
         WHERE h.id = ? AND h.estado = 'ACTIVA'",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((inst, cama_id)) = fila else {
        return Ok(false);
    };
    if ambito.puede_ver(Some(inst)) != Visibilidad::Si {
        return Ok(false);
    }

    let fecha = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    sqlx::query(
        "UPDATE hospitalizacion SET estado = ?, fecha_egreso = ?, diagnostico_egreso = ?, \
         resumen_clinico = ? WHERE id = ? AND estado = 'ACTIVA'",
    )
    .bind(&e.tipo_egreso)
    .bind(&fecha)
    .bind(&e.diagnostico_egreso)
    .bind(&e.resumen_clinico)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE cama SET estado = 'DISPONIBLE' WHERE id = ?")
        .bind(cama_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let url = format!(
            "sqlite:file:hosp_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dr House','h@x.co','x','medico',1,1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F',1),(20,'CC','222','Beto','Paz','1985-01-01','M',2)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO cama (id,numero,piso,servicio) VALUES (1,'101-A','1','GENERAL'),(2,'201-B','2','UCI')").execute(&pool).await.unwrap();
        pool
    }

    fn ingreso(paciente_id: i64, cama_id: i64) -> Ingreso {
        Ingreso {
            paciente_id,
            cama_id,
            medico_id: 5,
            via_ingreso: Some("PROGRAMADA".into()),
            diagnostico_ingreso: Some("J18.9".into()),
            motivo_ingreso: None,
            observaciones_ingreso: None,
        }
    }

    #[tokio::test]
    async fn ingresar_ocupa_la_cama_y_aparece_en_la_lista() {
        let pool = base().await;
        let id = ingresar(&pool, Ambito::institucion(1), &ingreso(10, 1)).await.unwrap().unwrap();
        // La cama quedó ocupada.
        let camas = listar_camas(&pool, Some("OCUPADA"), None).await.unwrap();
        assert_eq!(camas.len(), 1);
        assert_eq!(camas[0].id, 1);
        // La hospitalización aparece, enriquecida.
        let lista = listar(&pool, Ambito::institucion(1), None).await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, id);
        assert_eq!(lista[0].paciente_nombre, "Ana Ruiz");
        assert_eq!(lista[0].cama_numero, "101-A");
        assert_eq!(lista[0].medico_nombre, "Dr House");
    }

    #[tokio::test]
    async fn no_ingresa_en_cama_ocupada_ni_paciente_ajeno_ni_via_invalida() {
        let pool = base().await;
        ingresar(&pool, Ambito::institucion(1), &ingreso(10, 1)).await.unwrap().unwrap();
        // La cama 1 ya está ocupada.
        assert!(ingresar(&pool, Ambito::institucion(1), &ingreso(10, 1)).await.unwrap().is_none());
        // Paciente 20 es de la institución 2.
        assert!(ingresar(&pool, Ambito::institucion(1), &ingreso(20, 2)).await.unwrap().is_none());
        // Vía inventada.
        let mut mala = ingreso(10, 2);
        mala.via_ingreso = Some("EN_HELICOPTERO".into());
        assert!(ingresar(&pool, Ambito::institucion(1), &mala).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn el_egreso_libera_la_cama_y_saca_de_los_activos() {
        let pool = base().await;
        let id = ingresar(&pool, Ambito::institucion(1), &ingreso(10, 1)).await.unwrap().unwrap();
        let e = Egreso { tipo_egreso: "ALTA_MEDICA".into(), diagnostico_egreso: Some("J18.9".into()), resumen_clinico: Some("mejoró".into()) };
        // Un ámbito ajeno no egresa lo que no es suyo.
        assert!(!egreso(&pool, Ambito::institucion(2), id, &e).await.unwrap());
        // Tipo de egreso inventado.
        let malo = Egreso { tipo_egreso: "SE_FUE".into(), diagnostico_egreso: None, resumen_clinico: None };
        assert!(!egreso(&pool, Ambito::institucion(1), id, &malo).await.unwrap());
        // El dueño sí; libera la cama y sale de los ACTIVA.
        assert!(egreso(&pool, Ambito::institucion(1), id, &e).await.unwrap());
        assert!(listar(&pool, Ambito::institucion(1), None).await.unwrap().is_empty());
        assert_eq!(listar_camas(&pool, Some("DISPONIBLE"), None).await.unwrap().len(), 2);
        // Egresar dos veces no hace nada.
        assert!(!egreso(&pool, Ambito::institucion(1), id, &e).await.unwrap());
    }

    #[tokio::test]
    async fn la_lista_y_los_medicos_respetan_el_ambito() {
        let pool = base().await;
        ingresar(&pool, Ambito::institucion(1), &ingreso(10, 1)).await.unwrap().unwrap();
        assert_eq!(listar(&pool, Ambito::institucion(1), None).await.unwrap().len(), 1);
        assert!(listar(&pool, Ambito::institucion(2), None).await.unwrap().is_empty());
        assert!(listar(&pool, Ambito::Ninguno, None).await.unwrap().is_empty());
        // El médico de la institución 1 aparece para ella, no para la 2.
        assert_eq!(listar_medicos(&pool, Ambito::institucion(1)).await.unwrap().len(), 1);
        assert!(listar_medicos(&pool, Ambito::institucion(2)).await.unwrap().is_empty());
    }
}
