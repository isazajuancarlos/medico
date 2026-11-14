//! Turnos del personal (#249).
//!
//! # No es la agenda, y confundirlos tiene consecuencia
//!
//! `agenda_medico` y `bloque_horario` (0035) son la disponibilidad para AGENDAR
//! citas. Un turno es quién está de servicio en la planta a las tres de la
//! mañana: no atiende citas y aun así responde. Usar la agenda para esto pondría
//! al médico de turno como disponible para consulta externa, y le llegarían
//! pacientes a las tres de la mañana.
//!
//! # La regla del módulo es que nadie está en dos sitios a la vez
//!
//! Un turno solapado no es un dato raro: es un turno que alguien no va a cubrir,
//! porque la persona está en el otro. Y no se puede impedir con un `CHECK` —SQLite
//! no compara filas entre sí—, así que se comprueba antes de escribir y **se dice
//! con cuál choca**.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

pub const TIPOS: [(&str, &str); 2] = [
    ("PRESENCIAL", "Presencial"),
    ("DISPONIBILIDAD", "Disponibilidad"),
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Turno {
    pub id: i64,
    pub usuario_id: i64,
    pub servicio: String,
    pub tipo: String,
    pub inicio: String,
    pub fin: String,
    pub notas: Option<String>,
    pub creado_en: String,
    pub quien: String,
    pub rol: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevoTurno {
    pub usuario_id: i64,
    pub servicio: String,
    #[serde(default = "presencial")]
    pub tipo: String,
    pub inicio: String,
    pub fin: String,
    #[serde(default)]
    pub notas: Option<String>,
}

fn presencial() -> String {
    "PRESENCIAL".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Falta {
    UsuarioInexistente,
    TipoDesconocido,
    ServicioVacio,
    /// Termina antes de empezar. Imposible, no inusual.
    FinAntesDelInicio,
    /// Choca con otro turno de la MISMA persona, y dice con cuál.
    SolapaCon(i64),
    TurnoInexistente,
}

impl Falta {
    pub fn clave(&self) -> &'static str {
        match self {
            Falta::UsuarioInexistente => "usuario_inexistente",
            Falta::TipoDesconocido => "tipo",
            Falta::ServicioVacio => "servicio",
            Falta::FinAntesDelInicio => "fin_antes_del_inicio",
            Falta::SolapaCon(_) => "solapa",
            Falta::TurnoInexistente => "turno_inexistente",
        }
    }

    pub fn mensaje(&self) -> String {
        match self {
            Falta::UsuarioInexistente => "esa persona no existe en esta institución".into(),
            Falta::TipoDesconocido => "el tipo va PRESENCIAL o DISPONIBILIDAD".into(),
            Falta::ServicioVacio => "el servicio es obligatorio".into(),
            Falta::FinAntesDelInicio => "el turno termina antes de empezar".into(),
            Falta::SolapaCon(id) => format!(
                "esa persona ya tiene el turno {id} en ese horario: un turno solapado es un \
                 turno que alguien no va a cubrir"
            ),
            Falta::TurnoInexistente => "ese turno no existe en esta institución".into(),
        }
    }
}

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

const SELECT_TURNO: &str = "\
    SELECT t.id, t.usuario_id, t.servicio, t.tipo, t.inicio, t.fin, t.notas, t.creado_en, \
    COALESCE(u.nombre,'') AS quien, COALESCE(u.rol,'') AS rol \
    FROM turno_personal t LEFT JOIN usuario u ON u.id = t.usuario_id";

/// Los turnos del ámbito, opcionalmente acotados a un rango.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Vec<Turno>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_TURNO);
    sql.push_str(" WHERE 1=1");
    if inst.is_some() {
        sql.push_str(" AND t.institucion_id = ?");
    }
    if desde.is_some() {
        sql.push_str(" AND date(t.fin) >= date(?)");
    }
    if hasta.is_some() {
        sql.push_str(" AND date(t.inicio) <= date(?)");
    }
    sql.push_str(" ORDER BY t.inicio, t.id");
    let mut q = sqlx::query_as::<_, Turno>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(d) = desde {
        q = q.bind(d);
    }
    if let Some(h) = hasta {
        q = q.bind(h);
    }
    q.fetch_all(pool).await
}

/// Programa un turno.
pub async fn programar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevoTurno,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if !TIPOS.iter().any(|(t, _)| *t == n.tipo) {
        return Ok(Err(Falta::TipoDesconocido));
    }
    if n.servicio.trim().is_empty() {
        return Ok(Err(Falta::ServicioVacio));
    }
    if n.fin <= n.inicio {
        return Ok(Err(Falta::FinAntesDelInicio));
    }
    let Some(Some(institucion_id)) = institucion(ambito) else {
        return Ok(Err(Falta::UsuarioInexistente));
    };
    let existe: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM usuario WHERE id = ? AND institucion_id = ?")
            .bind(n.usuario_id)
            .bind(institucion_id)
            .fetch_optional(pool)
            .await?;
    if existe.is_none() {
        return Ok(Err(Falta::UsuarioInexistente));
    }

    // El solape. Dos intervalos se cruzan si cada uno empieza antes de que el
    // otro termine. Se compara con `<` y no `<=` a propósito: un turno que
    // empieza justo cuando el otro termina es el RELEVO, y es lo normal.
    let choque: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM turno_personal WHERE usuario_id = ? AND inicio < ? AND fin > ? LIMIT 1",
    )
    .bind(n.usuario_id)
    .bind(&n.fin)
    .bind(&n.inicio)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = choque {
        return Ok(Err(Falta::SolapaCon(id)));
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO turno_personal (institucion_id, usuario_id, servicio, tipo, inicio, fin, \
         notas, creado_en) VALUES (?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(institucion_id)
    .bind(n.usuario_id)
    .bind(n.servicio.trim())
    .bind(&n.tipo)
    .bind(&n.inicio)
    .bind(&n.fin)
    .bind(n.notas.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Quita un turno. Aquí SÍ se borra —al revés que el MAR o el certificado—:
/// un turno es una previsión, no una constancia de algo que ocurrió.
pub async fn quitar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Result<(), Falta>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::TurnoInexistente));
    };
    let mut sql = String::from("DELETE FROM turno_personal WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let r = q.execute(pool).await?;
    if r.rows_affected() == 0 {
        return Ok(Err(Falta::TurnoInexistente));
    }
    Ok(Ok(()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401'),(2,'Dos','900999','050010999901')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (6,'Dr. Paz','d@x.co','h','medico',1,1), \
                    (7,'Enf. Sol','e@x.co','h','enfermeria',1,1), \
                    (8,'Ajeno','a@x.co','h','medico',2,1)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    fn turno(usuario: i64, inicio: &str, fin: &str) -> NuevoTurno {
        NuevoTurno {
            usuario_id: usuario,
            servicio: "URGENCIAS".into(),
            tipo: "PRESENCIAL".into(),
            inicio: inicio.into(),
            fin: fin.into(),
            notas: None,
        }
    }

    #[tokio::test]
    async fn se_programa_y_se_lista() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        programar(&pool, a, &turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().unwrap();
        let v = listar(&pool, a, None, None).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].quien, "Dr. Paz");
        assert_eq!(v[0].servicio, "URGENCIAS");
    }

    /// **Nadie está en dos sitios a la vez.** Es la regla del módulo, y va con
    /// sus dos parejas: el solape se rechaza, y el RELEVO —empezar justo cuando
    /// el otro termina— se permite. Sin la segunda, la regla impediría el
    /// funcionamiento normal de un hospital.
    #[tokio::test]
    async fn el_solape_se_rechaza_pero_el_relevo_pasa() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let primero = programar(&pool, a, &turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().unwrap();

        // Se cruza por el medio.
        assert_eq!(
            programar(&pool, a, &turno(6, "2026-08-16T18:00:00Z", "2026-08-17T06:00:00Z"))
                .await.unwrap(),
            Err(Falta::SolapaCon(primero))
        );
        // Lo contiene entero.
        assert!(matches!(
            programar(&pool, a, &turno(6, "2026-08-16T06:00:00Z", "2026-08-16T20:00:00Z"))
                .await.unwrap(),
            Err(Falta::SolapaCon(_))
        ));
        // EL RELEVO: empieza exactamente cuando el otro acaba. Es lo normal.
        assert!(programar(&pool, a, &turno(6, "2026-08-16T19:00:00Z", "2026-08-17T07:00:00Z"))
            .await.unwrap().is_ok());
        // Y OTRA persona sí puede estar en el mismo horario.
        assert!(programar(&pool, a, &turno(7, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn el_turno_al_reves_y_el_tipo_inventado_se_rechazan() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        assert_eq!(
            programar(&pool, a, &turno(6, "2026-08-16T19:00:00Z", "2026-08-16T07:00:00Z"))
                .await.unwrap(),
            Err(Falta::FinAntesDelInicio)
        );
        let mut n = turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z");
        n.tipo = "GUARDIA".into();
        assert_eq!(programar(&pool, a, &n).await.unwrap(), Err(Falta::TipoDesconocido));
        n.tipo = "PRESENCIAL".into();
        n.servicio = "  ".into();
        assert_eq!(programar(&pool, a, &n).await.unwrap(), Err(Falta::ServicioVacio));
    }

    #[tokio::test]
    async fn no_se_programa_a_la_persona_de_otra_institucion() {
        let pool = base().await;
        assert_eq!(
            programar(&pool, Ambito::Institucion(1),
                      &turno(8, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z")).await.unwrap(),
            Err(Falta::UsuarioInexistente)
        );
        assert!(programar(&pool, Ambito::Institucion(1),
                          &turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn el_rango_recorta_y_el_ambito_aisla() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        programar(&pool, a, &turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().unwrap();
        programar(&pool, a, &turno(7, "2026-09-01T07:00:00Z", "2026-09-01T19:00:00Z"))
            .await.unwrap().unwrap();
        assert_eq!(listar(&pool, a, Some("2026-08-01"), Some("2026-08-31")).await.unwrap().len(), 1);
        assert_eq!(listar(&pool, a, None, None).await.unwrap().len(), 2, "sin filtro, los dos");
        assert!(listar(&pool, Ambito::Institucion(2), None, None).await.unwrap().is_empty());
        assert!(listar(&pool, Ambito::Ninguno, None, None).await.unwrap().is_empty());
    }

    /// Un turno SÍ se borra, al revés que el MAR o el certificado: es una
    /// previsión, no la constancia de algo que ocurrió.
    #[tokio::test]
    async fn el_turno_se_quita_y_libera_el_horario() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = programar(&pool, a, &turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().unwrap();
        quitar(&pool, a, id).await.unwrap().unwrap();
        assert!(listar(&pool, a, None, None).await.unwrap().is_empty());
        // Y el horario queda libre otra vez.
        assert!(programar(&pool, a, &turno(6, "2026-08-16T07:00:00Z", "2026-08-16T19:00:00Z"))
            .await.unwrap().is_ok());
        assert_eq!(quitar(&pool, a, 9999).await.unwrap(), Err(Falta::TurnoInexistente));
    }
}
