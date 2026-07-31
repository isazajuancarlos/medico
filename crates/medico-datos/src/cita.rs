//! Citas. Toda consulta lleva el ámbito; una cita solo se ve, crea o cambia
//! dentro de la institución de quien la maneja. La agenda —franjas, bloqueos,
//! turnos libres e indicadores— vive en [`crate::agenda`], que lee de aquí para
//! saber qué horas están cogidas.

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

/// Una cita con el nombre del paciente y del médico ya resueltos, para listar
/// sin que el cliente tenga que cruzar tablas.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CitaVista {
    pub id: i64,
    pub paciente_id: i64,
    pub paciente_nombre: String,
    pub medico_id: Option<i64>,
    pub medico_nombre: Option<String>,
    pub fecha: String,
    pub hora: String,
    pub estado: String,
    pub motivo: Option<String>,
    /// PRIMERA_VEZ / CONTROL / URGENCIA. `None` en las citas anteriores a la
    /// migración 0035: significa **no se sabe**, y la pantalla lo deja vacío.
    /// Hasta entonces el navegador rellenaba «Primera Vez» para todas.
    pub tipo: Option<String>,
    /// Igual: `None` es «no se registró», no media hora.
    pub duracion_minutos: Option<i64>,
    /// Cuándo la PIDIÓ el paciente (YYYY-MM-DD). Es el numerador del indicador
    /// de oportunidad de la Resolución 256 de 2016.
    pub solicitada_en: Option<String>,
    pub institucion_id: i64,
}

const SELECT_VISTA: &str = "SELECT c.id, c.paciente_id, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente_nombre, \
    c.medico_id, u.nombre AS medico_nombre, c.fecha, c.hora, c.estado, c.motivo, \
    c.tipo, c.duracion_minutos, c.solicitada_en, c.institucion_id \
    FROM cita c \
    JOIN paciente p ON p.id = c.paciente_id \
    LEFT JOIN usuario u ON u.id = c.medico_id AND u.institucion_id = c.institucion_id";

/// Lista las citas del ámbito, opcionalmente filtradas por fecha (YYYY-MM-DD).
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    fecha: Option<&str>,
) -> Result<Vec<CitaVista>, sqlx::Error> {
    let (cond, inst) = match ambito {
        Ambito::Superadmin => ("1 = 1", None),
        Ambito::Institucion(id) => ("c.institucion_id = ?", Some(id)),
        Ambito::Ninguno => return Ok(Vec::new()),
    };
    let filtro_fecha = if fecha.is_some() { " AND c.fecha = ?" } else { "" };
    let sql = format!("{SELECT_VISTA} WHERE {cond}{filtro_fecha} ORDER BY c.fecha, c.hora");
    let mut q = sqlx::query_as::<_, CitaVista>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(f) = fecha {
        q = q.bind(f);
    }
    q.fetch_all(pool).await
}

#[derive(Debug, Clone, Deserialize)]
pub struct CitaNueva {
    pub paciente_id: i64,
    pub medico_id: Option<i64>,
    pub fecha: String,
    pub hora: String,
    pub motivo: Option<String>,
    /// PRIMERA_VEZ / CONTROL / URGENCIA. Opcional porque una cita se puede
    /// agendar por teléfono sin saberlo todavía; lo que NO se hace es
    /// rellenarla, que era el defecto que corrige la migración 0035.
    pub tipo: Option<String>,
    pub duracion_minutos: Option<i64>,
    /// Cuándo la pidió el paciente. Sin ella, esta cita no cuenta para el
    /// indicador de oportunidad y el indicador lo dice.
    pub solicitada_en: Option<String>,
}

/// Crea una cita EN la institución de quien la agenda; el paciente debe ser de
/// esa misma institución. `None` si el ámbito no puede agendar, o si el paciente
/// no le pertenece (no se distingue: pedir por un paciente ajeno es como pedir
/// por uno inexistente).
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    nueva: &CitaNueva,
) -> Result<Option<i64>, sqlx::Error> {
    let institucion_id = match ambito {
        Ambito::Institucion(id) => id,
        Ambito::Superadmin | Ambito::Ninguno => return Ok(None),
    };
    // El paciente tiene que ser de la institución: una cita no cruza fronteras.
    let del_paciente: Option<(i64,)> =
        sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
            .bind(nueva.paciente_id)
            .fetch_optional(pool)
            .await?;
    if del_paciente.map(|(i,)| i) != Some(institucion_id) {
        return Ok(None);
    }
    // Y el MÉDICO también, CUANDO VIENE: una cita sin profesional asignado es un
    // estado legítimo —se agenda y se asigna después—, así que `None` pasa. Lo
    // que no puede pasar es un id ajeno: sin esto, `medico_id` solo tenía por
    // freno la clave foránea a `usuario(id)`, que abarca todas las instituciones,
    // y servía además para saber qué ids existen.
    if let Some(medico_id) = nueva.medico_id
        && !crate::usuario::es_del_ambito(pool, ambito, medico_id).await?
    {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO cita (institucion_id, paciente_id, medico_id, fecha, hora, motivo, \
                           tipo, duracion_minutos, solicitada_en) \
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(nueva.paciente_id)
    .bind(nueva.medico_id)
    .bind(&nueva.fecha)
    .bind(&nueva.hora)
    .bind(&nueva.motivo)
    .bind(&nueva.tipo)
    .bind(nueva.duracion_minutos)
    .bind(&nueva.solicitada_en)
    .execute(pool)
    .await;
    match r {
        Ok(r) => Ok(Some(r.last_insert_rowid())),
        // Un `tipo` fuera del CHECK o una duración imposible se rechazan; no se
        // guardan «como vengan» ni se sustituyen por un valor válido.
        Err(sqlx::Error::Database(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Cambia el estado de una cita **si es visible para el ámbito**. `false` si no
/// existe o no es suya. Valida el estado contra el CHECK de la tabla, así que un
/// valor inválido no pasa.
pub async fn cambiar_estado(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    estado: &str,
) -> Result<bool, sqlx::Error> {
    let condicion = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return Ok(false),
    };
    let filtro = if condicion.is_some() {
        "id = ? AND institucion_id = ?"
    } else {
        "id = ?"
    };
    let sql = format!("UPDATE cita SET estado = ? WHERE {filtro}");
    let mut q = sqlx::query(&sql).bind(estado).bind(id);
    if let Some(i) = condicion {
        q = q.bind(i);
    }
    // Un estado fuera del CHECK devuelve error; se traduce a «no se pudo».
    match q.execute(pool).await {
        Ok(r) => Ok(r.rows_affected() > 0),
        Err(sqlx::Error::Database(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// El id de institución de una cita, si es visible para el ámbito. Auxiliar de
/// las pruebas y de comprobaciones puntuales.
pub async fn visible(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM cita WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos instituciones, un paciente y un médico en cada una, y una cita en la 1.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (5,'Dra Uno','u1@x.co','h','medico',1,1),(6,'Dr Dos','u2@x.co','h','medico',2,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,
                                   fecha_nacimiento,sexo,institucion_id)
             VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F',1),
                    (20,'CC','222','Beto','Paz','1985-05-05','M',2)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cita (id,institucion_id,paciente_id,medico_id,fecha,hora,estado)
             VALUES (100,1,10,5,'2026-08-01','09:00','PROGRAMADA')",
        ).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn listar_es_scoped_y_resuelve_nombres() {
        let pool = base().await;
        let cs = listar(&pool, Ambito::institucion(1), None).await.unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].paciente_nombre, "Ana Ruiz");
        assert_eq!(cs[0].medico_nombre.as_deref(), Some("Dra Uno"));
        // La institución 2 no ve la cita de la 1.
        assert!(listar(&pool, Ambito::institucion(2), None).await.unwrap().is_empty());
        assert!(listar(&pool, Ambito::Ninguno, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn filtra_por_fecha() {
        let pool = base().await;
        assert_eq!(listar(&pool, Ambito::institucion(1), Some("2026-08-01")).await.unwrap().len(), 1);
        assert!(listar(&pool, Ambito::institucion(1), Some("2026-08-02")).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn crear_para_paciente_propio_funciona() {
        let pool = base().await;
        let nueva = CitaNueva { paciente_id: 10, medico_id: Some(5), fecha: "2026-08-05".into(), hora: "10:30".into(), motivo: Some("Control".into()), tipo: Some("CONTROL".into()), duracion_minutos: Some(30), solicitada_en: Some("2026-08-01".into()) };
        assert!(crear(&pool, Ambito::institucion(1), &nueva).await.unwrap().is_some());
    }

    /// No se puede agendar una cita a un paciente de otra institución.
    #[tokio::test]
    async fn crear_para_paciente_ajeno_no_se_puede() {
        let pool = base().await;
        let nueva = CitaNueva { paciente_id: 20, medico_id: None, fecha: "2026-08-05".into(), hora: "10:30".into(), motivo: None, tipo: None, duracion_minutos: None, solicitada_en: None };
        assert!(crear(&pool, Ambito::institucion(1), &nueva).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn cambiar_estado_el_propio_si_el_ajeno_no() {
        let pool = base().await;
        assert!(cambiar_estado(&pool, Ambito::institucion(1), 100, "ATENDIDA").await.unwrap());
        // La institución 2 no puede tocar la cita de la 1.
        assert!(!cambiar_estado(&pool, Ambito::institucion(2), 100, "CANCELADA").await.unwrap());
    }

    /// Las citas viejas NO traen tipo ni duración, y eso se lee como «no se
    /// sabe». Antes el navegador rellenaba «Primera Vez» y 30 minutos para
    /// todas, indistinguible de un dato registrado.
    #[tokio::test]
    async fn la_cita_anterior_a_la_migracion_no_finge_tipo_ni_duracion() {
        let pool = base().await;
        let c = &listar(&pool, Ambito::institucion(1), None).await.unwrap()[0];
        assert_eq!(c.tipo, None);
        assert_eq!(c.duracion_minutos, None);
        assert_eq!(c.solicitada_en, None);
    }

    #[tokio::test]
    async fn el_tipo_y_la_duracion_se_guardan_y_vuelven() {
        let pool = base().await;
        let nueva = CitaNueva {
            paciente_id: 10, medico_id: Some(5), fecha: "2026-08-05".into(),
            hora: "10:30".into(), motivo: None, tipo: Some("URGENCIA".into()),
            duracion_minutos: Some(45), solicitada_en: Some("2026-08-04".into()),
        };
        crear(&pool, Ambito::institucion(1), &nueva).await.unwrap().unwrap();
        let cs = listar(&pool, Ambito::institucion(1), Some("2026-08-05")).await.unwrap();
        assert_eq!(cs[0].tipo.as_deref(), Some("URGENCIA"));
        assert_eq!(cs[0].duracion_minutos, Some(45));
        assert_eq!(cs[0].solicitada_en.as_deref(), Some("2026-08-04"));
    }

    /// Un tipo inventado se rechaza; no se guarda «como venga».
    #[tokio::test]
    async fn un_tipo_fuera_del_vocabulario_no_pasa() {
        let pool = base().await;
        let nueva = CitaNueva {
            paciente_id: 10, medico_id: None, fecha: "2026-08-05".into(),
            hora: "10:30".into(), motivo: None, tipo: Some("Primera Vez".into()),
            duracion_minutos: None, solicitada_en: None,
        };
        assert!(
            crear(&pool, Ambito::institucion(1), &nueva).await.unwrap().is_none(),
            "entró el vocabulario de la pantalla en la columna"
        );
    }

    #[tokio::test]
    async fn una_duracion_imposible_no_pasa() {
        let pool = base().await;
        for d in [0i64, -30, 1441] {
            let nueva = CitaNueva {
                paciente_id: 10, medico_id: None, fecha: "2026-08-05".into(),
                hora: "10:30".into(), motivo: None, tipo: None,
                duracion_minutos: Some(d), solicitada_en: None,
            };
            assert!(
                crear(&pool, Ambito::institucion(1), &nueva).await.unwrap().is_none(),
                "se aceptó una duración de {d} minutos"
            );
        }
    }

    #[tokio::test]
    async fn un_estado_invalido_no_pasa() {
        let pool = base().await;
        assert!(!cambiar_estado(&pool, Ambito::institucion(1), 100, "INVENTADO").await.unwrap());
    }
}
