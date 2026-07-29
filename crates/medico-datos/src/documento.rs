//! Documento clínico: la espina de las notas que cuelgan del paciente. Un solo
//! módulo sirve a todos los tipos (evolución, epicrisis, interconsulta, nota
//! operatoria, consentimiento, incapacidad, contrarreferencia, valoraciones…):
//! lo común vive aquí y lo específico de cada tipo va en `contenido` (JSON).
//!
//! Aislamiento por la cadena documento→paciente→institución, igual que la
//! historia. Un documento firmado es inmutable (`crate::Resultado::Firmada`).

pub use crate::Resultado;
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Documento {
    pub id: i64,
    pub tipo: String,
    pub paciente_id: i64,
    pub historia_id: Option<i64>,
    pub medico_id: i64,
    pub fecha: String,
    pub titulo: String,
    /// Los campos propios del tipo, como objeto JSON (no como cadena escapada).
    pub contenido: Json<serde_json::Value>,
    pub firmada: bool,
}

/// Lo que llega para crear un documento. El `medico_id` no viene del cliente: lo
/// pone el autor autenticado (se pasa aparte). El `paciente_id` sí, y se valida
/// contra el ámbito.
#[derive(Debug, Clone, Deserialize)]
pub struct DocumentoNuevo {
    pub tipo: String,
    pub paciente_id: i64,
    #[serde(default)]
    pub historia_id: Option<i64>,
    #[serde(default)]
    pub titulo: String,
    #[serde(default = "objeto_vacio")]
    pub contenido: serde_json::Value,
}

/// Lo editable de un documento. No cambian ni el tipo, ni el paciente, ni el
/// autor: un documento no se transforma en otro ni cambia de dueño editándolo.
#[derive(Debug, Clone, Deserialize)]
pub struct DocumentoEdicion {
    #[serde(default)]
    pub historia_id: Option<i64>,
    #[serde(default)]
    pub titulo: String,
    #[serde(default = "objeto_vacio")]
    pub contenido: serde_json::Value,
}

fn objeto_vacio() -> serde_json::Value {
    serde_json::json!({})
}

const COLUMNAS: &str =
    "id, tipo, paciente_id, historia_id, medico_id, fecha, titulo, contenido, firmada";

async fn paciente_es_del_ambito(pool: &crate::Pool, ambito: Ambito, paciente_id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(paciente_id)
        .fetch_optional(pool)
        .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

/// El paciente y el estado de firma de un documento visible para el ámbito.
/// `None` si no existe o es ajeno —indistinguibles—.
async fn paciente_y_firma(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<(i64, bool)>, sqlx::Error> {
    let fila: Option<(i64, bool)> =
        sqlx::query_as("SELECT paciente_id, firmada FROM documento_clinico WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    match fila {
        Some((pid, firmada)) if paciente_es_del_ambito(pool, ambito, pid).await? => Ok(Some((pid, firmada))),
        _ => Ok(None),
    }
}

/// Los documentos de un paciente visible, opcionalmente filtrados por tipo.
/// Vacío si el paciente no es del ámbito (indistinguible de «sin documentos»).
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
    tipo: Option<&str>,
) -> Result<Vec<Documento>, sqlx::Error> {
    if !paciente_es_del_ambito(pool, ambito, paciente_id).await? {
        return Ok(Vec::new());
    }
    match tipo {
        Some(t) => {
            sqlx::query_as::<_, Documento>(&format!(
                "SELECT {COLUMNAS} FROM documento_clinico WHERE paciente_id = ? AND tipo = ? ORDER BY fecha DESC, id DESC"
            ))
            .bind(paciente_id)
            .bind(t)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, Documento>(&format!(
                "SELECT {COLUMNAS} FROM documento_clinico WHERE paciente_id = ? ORDER BY fecha DESC, id DESC"
            ))
            .bind(paciente_id)
            .fetch_all(pool)
            .await
        }
    }
}

/// La fila de una worklist: el documento con los nombres ya unidos, sin el
/// `contenido` (la lista no lo necesita; el editor lo pide con `obtener`).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DocumentoVista {
    pub id: i64,
    pub tipo: String,
    pub paciente_id: i64,
    pub medico_id: i64,
    pub fecha: String,
    pub titulo: String,
    /// Se incluye el `contenido` en la worklist para que cada lista muestre los
    /// campos propios de su tipo sin una segunda petición por fila.
    pub contenido: Json<serde_json::Value>,
    pub firmada: bool,
    pub paciente_nombre: String,
    pub paciente_identificacion: String,
    pub medico_nombre: String,
}

/// Los documentos VISIBLES para el ámbito (no de un solo paciente), con los
/// nombres unidos, para las worklists por tipo. El ámbito se aplica en el SQL
/// sobre `paciente.institucion_id`: un ajeno sencillamente no entra. Filtros
/// opcionales por paciente y por tipo.
pub async fn listar_ambito(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: Option<i64>,
    tipo: Option<&str>,
) -> Result<Vec<DocumentoVista>, sqlx::Error> {
    // Ninguno no ve nada: no es una consulta vacía, es no preguntar.
    let filtro_inst = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let mut sql = String::from(
        "SELECT d.id, d.tipo, d.paciente_id, d.medico_id, d.fecha, d.titulo, d.contenido, d.firmada, \
         TRIM(REPLACE(REPLACE(p.nombre1 || ' ' || COALESCE(p.nombre2,'') || ' ' || p.apellido1 || ' ' || COALESCE(p.apellido2,''), '  ', ' '), '  ', ' ')) AS paciente_nombre, \
         p.numero_documento AS paciente_identificacion, \
         u.nombre AS medico_nombre \
         FROM documento_clinico d \
         JOIN paciente p ON p.id = d.paciente_id \
         JOIN usuario u ON u.id = d.medico_id \
         WHERE 1=1",
    );
    if filtro_inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    if paciente_id.is_some() {
        sql.push_str(" AND d.paciente_id = ?");
    }
    if tipo.is_some() {
        sql.push_str(" AND d.tipo = ?");
    }
    sql.push_str(" ORDER BY d.fecha DESC, d.id DESC");

    let mut q = sqlx::query_as::<_, DocumentoVista>(&sql);
    if let Some(i) = filtro_inst {
        q = q.bind(i);
    }
    if let Some(p) = paciente_id {
        q = q.bind(p);
    }
    if let Some(t) = tipo {
        q = q.bind(t);
    }
    q.fetch_all(pool).await
}

/// Un documento por id, si su paciente es visible para el ámbito.
pub async fn obtener(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<Documento>, sqlx::Error> {
    let d = sqlx::query_as::<_, Documento>(&format!("SELECT {COLUMNAS} FROM documento_clinico WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    match d {
        Some(d) if paciente_es_del_ambito(pool, ambito, d.paciente_id).await? => Ok(Some(d)),
        _ => Ok(None),
    }
}

/// Crea un documento para un paciente del ámbito, con `medico_id` como autor.
/// `None` si el paciente no existe o no le pertenece al ámbito.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: i64,
    d: &DocumentoNuevo,
) -> Result<Option<i64>, sqlx::Error> {
    if !paciente_es_del_ambito(pool, ambito, d.paciente_id).await? {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO documento_clinico (tipo, paciente_id, historia_id, medico_id, titulo, contenido) \
         VALUES (?,?,?,?,?,?)",
    )
    .bind(&d.tipo)
    .bind(d.paciente_id)
    .bind(d.historia_id)
    .bind(medico_id)
    .bind(&d.titulo)
    .bind(Json(&d.contenido))
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Edita un documento de borrador. Se niega sobre uno firmado (`Firmada`), que es
/// inmutable. No cambia tipo, paciente ni autor.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    d: &DocumentoEdicion,
) -> Result<Resultado, sqlx::Error> {
    let Some((_, firmada)) = paciente_y_firma(pool, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if firmada {
        return Ok(Resultado::Firmada);
    }
    sqlx::query("UPDATE documento_clinico SET historia_id = ?, titulo = ?, contenido = ? WHERE id = ?")
        .bind(d.historia_id)
        .bind(&d.titulo)
        .bind(Json(&d.contenido))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Resultado::Hecho)
}

/// Firma un documento visible y sin firmar. `false` si no existe, es ajeno o ya
/// estaba firmado.
pub async fn firmar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    match paciente_y_firma(pool, ambito, id).await? {
        Some((_, false)) => {
            let r = sqlx::query("UPDATE documento_clinico SET firmada = 1 WHERE id = ? AND firmada = 0")
                .bind(id)
                .execute(pool)
                .await?;
            Ok(r.rows_affected() > 0)
        }
        _ => Ok(false),
    }
}

/// Borra un documento de borrador. Se niega sobre uno firmado (`Firmada`).
pub async fn eliminar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Resultado, sqlx::Error> {
    let Some((_, firmada)) = paciente_y_firma(pool, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if firmada {
        return Ok(Resultado::Firmada);
    }
    sqlx::query("DELETE FROM documento_clinico WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Resultado::Hecho)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dra','d@x.co','h','medico',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        pool
    }

    fn evolucion(paciente_id: i64) -> DocumentoNuevo {
        DocumentoNuevo {
            tipo: "EVOLUCION".into(),
            paciente_id,
            historia_id: None,
            titulo: "Control".into(),
            contenido: json!({"subjetivo": "mejor", "objetivo": "afebril", "analisis": "evoluciona bien", "plan": "seguir"}),
        }
    }

    #[tokio::test]
    async fn crear_y_listar_por_tipo() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &evolucion(10)).await.unwrap().unwrap();
        let evs = listar(&pool, Ambito::institucion(1), 10, Some("EVOLUCION")).await.unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].id, id);
        assert_eq!(evs[0].medico_id, 5);
        assert_eq!(evs[0].contenido.0["subjetivo"], "mejor");
        // Un tipo distinto no la trae.
        assert!(listar(&pool, Ambito::institucion(1), 10, Some("EPICRISIS")).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn el_paciente_ajeno_no_deja_crear_ni_ver() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::institucion(1), 5, &evolucion(20)).await.unwrap().is_none());
        let id = crear(&pool, Ambito::institucion(2), 5, &evolucion(20)).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::institucion(1), id).await.unwrap().is_none());
        assert!(listar(&pool, Ambito::institucion(1), 20, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn editar_cambia_el_contenido() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &evolucion(10)).await.unwrap().unwrap();
        let edit = DocumentoEdicion {
            historia_id: None,
            titulo: "Control 2".into(),
            contenido: json!({"subjetivo": "estable"}),
        };
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &edit).await.unwrap(), Resultado::Hecho);
        let d = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(d.titulo, "Control 2");
        assert_eq!(d.contenido.0["subjetivo"], "estable");
    }

    #[tokio::test]
    async fn un_documento_firmado_es_inmutable() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &evolucion(10)).await.unwrap().unwrap();
        assert!(firmar(&pool, Ambito::institucion(1), id).await.unwrap());
        assert!(!firmar(&pool, Ambito::institucion(1), id).await.unwrap());
        let edit = DocumentoEdicion { historia_id: None, titulo: "x".into(), contenido: json!({}) };
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &edit).await.unwrap(), Resultado::Firmada);
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), Resultado::Firmada);
    }

    #[tokio::test]
    async fn la_ajena_no_se_firma_edita_ni_borra() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(2), 5, &evolucion(20)).await.unwrap().unwrap();
        assert!(!firmar(&pool, Ambito::institucion(1), id).await.unwrap());
        let edit = DocumentoEdicion { historia_id: None, titulo: "x".into(), contenido: json!({}) };
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &edit).await.unwrap(), Resultado::NoExiste);
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), Resultado::NoExiste);
        assert!(obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn eliminar_una_de_borrador_la_quita() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &evolucion(10)).await.unwrap().unwrap();
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), Resultado::Hecho);
        assert!(obtener(&pool, Ambito::institucion(1), id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn el_superadmin_ve_de_cualquier_institucion() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(2), 5, &evolucion(20)).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::Superadmin, id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn la_worklist_por_ambito_trae_nombres_y_respeta_la_institucion() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), 5, &evolucion(10)).await.unwrap();
        crear(&pool, Ambito::institucion(2), 5, &evolucion(20)).await.unwrap();
        // La institución 1 solo ve la suya, con el nombre del paciente unido.
        let w = listar_ambito(&pool, Ambito::institucion(1), None, Some("EVOLUCION")).await.unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].paciente_nombre, "Ana Ruiz");
        assert_eq!(w[0].medico_nombre, "Dra");
        // El superadmin ve las dos.
        assert_eq!(listar_ambito(&pool, Ambito::Superadmin, None, Some("EVOLUCION")).await.unwrap().len(), 2);
        // Ninguno no ve nada.
        assert!(listar_ambito(&pool, Ambito::Ninguno, None, None).await.unwrap().is_empty());
    }
}
