//! Telemedicina (#249).
//!
//! # Lo que ya estaba, medido antes de escribir
//!
//! `historia.modalidad_grupo_servicio` existe desde la 0005, y el CodeSystem
//! `ColombianTechModality` está vendorizado con las cuatro modalidades de
//! telemedicina. O sea que una teleconsulta **ya se podía reportar** al RIPS y al
//! RDA con su modalidad correcta.
//!
//! Es el caso de `terceros` otra vez: buscar «telemedicina» no encontraba nada
//! porque en este producto se llama `modalidad`.
//!
//! # Lo que faltaba, y es lo único que esta tabla añade
//!
//! **Quién está en el otro extremo.** Sin eso, el registro no distingue una
//! telexperticia con un especialista de otra institución de una consulta normal
//! mal etiquetada — y la diferencia entre las dos es quién responde por el acto.
//!
//! # La modalidad se valida contra el ARTEFACTO
//!
//! Las cuatro salen de `medico_dominio::ihce::terminologia`, que lee el CodeSystem
//! vendorizado. No se escriben a mano aquí: si el IG cambia, la prueba lo dice.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

/// Las cuatro modalidades de telemedicina del CodeSystem `ColombianTechModality`.
///
/// Se declaran con su código y su nombre para la pantalla; que existan de verdad
/// en el artefacto lo ata `las_cuatro_modalidades_estan_en_el_codesystem`.
pub const MODALIDADES: [(&str, &str); 4] = [
    ("06", "Telemedicina interactiva"),
    ("07", "Telemedicina no interactiva"),
    ("08", "Telemedicina - Telexperticia"),
    ("09", "Telemedicina - Telemonitoreo"),
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Teleconsulta {
    pub id: i64,
    pub historia_id: i64,
    pub modalidad: String,
    pub institucion_remota: Option<String>,
    pub profesional_remoto: Option<String>,
    pub plataforma: Option<String>,
    pub consentimiento_id: Option<i64>,
    pub incidencias: Option<String>,
    pub creado_en: String,
    pub paciente_id: i64,
    pub paciente: String,
    pub fecha_atencion: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaTeleconsulta {
    pub historia_id: i64,
    pub modalidad: String,
    #[serde(default)]
    pub institucion_remota: Option<String>,
    #[serde(default)]
    pub profesional_remoto: Option<String>,
    #[serde(default)]
    pub plataforma: Option<String>,
    #[serde(default)]
    pub consentimiento_id: Option<i64>,
    #[serde(default)]
    pub incidencias: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    HistoriaInexistente,
    /// Ya está marcada como teleconsulta: una atención es teleconsulta o no.
    YaEsTeleconsulta,
    /// La modalidad no es una de las CUATRO de telemedicina. Un `01` intramural
    /// aquí sería una consulta presencial disfrazada.
    ModalidadNoEsTelemedicina,
    /// No se dijo quién está en el otro extremo, que es lo único que esta tabla
    /// añade sobre `historia`.
    SinExtremoRemoto,
    /// El consentimiento es de OTRO paciente. Es la misma comprobación que hace
    /// quirófano, y por la misma razón.
    ConsentimientoDeOtroPaciente,
}

pub const TODAS: [Falta; 5] = [
    Falta::HistoriaInexistente,
    Falta::YaEsTeleconsulta,
    Falta::ModalidadNoEsTelemedicina,
    Falta::SinExtremoRemoto,
    Falta::ConsentimientoDeOtroPaciente,
];

impl Falta {
    pub fn clave(self) -> &'static str {
        match self {
            Falta::HistoriaInexistente => "historia_inexistente",
            Falta::YaEsTeleconsulta => "ya_es_teleconsulta",
            Falta::ModalidadNoEsTelemedicina => "modalidad",
            Falta::SinExtremoRemoto => "sin_extremo_remoto",
            Falta::ConsentimientoDeOtroPaciente => "consentimiento_ajeno",
        }
    }

    pub fn de_clave(c: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == c)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::HistoriaInexistente => "esa atención no existe en esta institución",
            Falta::YaEsTeleconsulta => "esa atención ya está registrada como teleconsulta",
            Falta::ModalidadNoEsTelemedicina => {
                "la modalidad tiene que ser una de las cuatro de telemedicina: 06 interactiva, \
                 07 no interactiva, 08 telexperticia o 09 telemonitoreo"
            }
            Falta::SinExtremoRemoto => {
                "hay que decir quién está en el otro extremo —la institución o el profesional—: \
                 sin eso, esta fila no dice nada que la atención no dijera ya"
            }
            Falta::ConsentimientoDeOtroPaciente => {
                "ese consentimiento es de otro paciente"
            }
        }
    }
}

fn hay(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.trim().is_empty())
}

pub fn revisar(n: &NuevaTeleconsulta) -> Result<(), Falta> {
    if !MODALIDADES.iter().any(|(c, _)| *c == n.modalidad) {
        return Err(Falta::ModalidadNoEsTelemedicina);
    }
    if !hay(&n.institucion_remota) && !hay(&n.profesional_remoto) {
        return Err(Falta::SinExtremoRemoto);
    }
    Ok(())
}

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

const SELECT: &str = "\
    SELECT t.id, t.historia_id, t.modalidad, t.institucion_remota, t.profesional_remoto, \
    t.plataforma, t.consentimiento_id, t.incidencias, t.creado_en, \
    p.id AS paciente_id, (p.nombre1 || ' ' || p.apellido1) AS paciente, h.fecha AS fecha_atencion \
    FROM teleconsulta t \
    JOIN historia h ON h.id = t.historia_id \
    JOIN paciente p ON p.id = h.paciente_id";

pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Teleconsulta>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT);
    if inst.is_some() {
        sql.push_str(" WHERE p.institucion_id = ?");
    }
    sql.push_str(" ORDER BY h.fecha DESC, t.id DESC");
    let mut q = sqlx::query_as::<_, Teleconsulta>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Marca una atención como teleconsulta.
///
/// **También escribe la modalidad en `historia`**, que es de donde la leen el
/// RIPS y el RDA. Guardarla solo aquí dejaría el archivo diciendo que fue
/// presencial: dos copias del mismo dato en desacuerdo, y la que sale al mundo
/// sería la equivocada.
pub async fn registrar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaTeleconsulta,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::HistoriaInexistente));
    };
    let mut sql = String::from(
        "SELECT p.id FROM historia h JOIN paciente p ON p.id = h.paciente_id WHERE h.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(n.historia_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let Some(paciente_id) = q.fetch_optional(pool).await? else {
        return Ok(Err(Falta::HistoriaInexistente));
    };

    let ya: Option<i64> = sqlx::query_scalar("SELECT 1 FROM teleconsulta WHERE historia_id = ?")
        .bind(n.historia_id)
        .fetch_optional(pool)
        .await?;
    if ya.is_some() {
        return Ok(Err(Falta::YaEsTeleconsulta));
    }

    if let Some(c) = n.consentimiento_id {
        let de: Option<i64> =
            sqlx::query_scalar("SELECT paciente_id FROM consentimiento_informado WHERE id = ?")
                .bind(c)
                .fetch_optional(pool)
                .await?;
        if de != Some(paciente_id) {
            return Ok(Err(Falta::ConsentimientoDeOtroPaciente));
        }
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO teleconsulta (historia_id, modalidad, institucion_remota, \
         profesional_remoto, plataforma, consentimiento_id, incidencias, creado_en) \
         VALUES (?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.historia_id)
    .bind(&n.modalidad)
    .bind(n.institucion_remota.as_deref())
    .bind(n.profesional_remoto.as_deref())
    .bind(n.plataforma.as_deref())
    .bind(n.consentimiento_id)
    .bind(n.incidencias.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;

    // Y la modalidad al sitio de donde la leen el RIPS y el RDA.
    sqlx::query("UPDATE historia SET modalidad_grupo_servicio = ? WHERE id = ?")
        .bind(&n.modalidad)
        .bind(n.historia_id)
        .execute(pool)
        .await?;

    Ok(Ok(id))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Ata las cuatro al artefacto.** Si el IG vendorizado cambiara un código,
    /// esto se pone rojo antes de que el servidor nacional rechace el documento.
    #[test]
    fn las_cuatro_modalidades_estan_en_el_codesystem() {
        use medico_dominio::ihce::terminologia;
        for (codigo, _) in MODALIDADES {
            assert!(
                terminologia::display(terminologia::SISTEMA_MODALIDAD, codigo).is_some(),
                "la modalidad «{codigo}» no está en el CodeSystem vendorizado"
            );
        }
        // Positivo de control: un código inventado NO está, o sea que la
        // comprobación discrimina.
        assert!(terminologia::display(terminologia::SISTEMA_MODALIDAD, "99").is_none());
        // Y el control por el otro lado: `01` SÍ está en el CodeSystem pero NO es
        // telemedicina — que es justo lo que este módulo tiene que rechazar.
        assert!(terminologia::display(terminologia::SISTEMA_MODALIDAD, "01").is_some());
        assert!(!MODALIDADES.iter().any(|(c, _)| *c == "01"));
    }

    pub(super) fn nueva_publica() -> NuevaTeleconsulta { nueva() }

    fn nueva() -> NuevaTeleconsulta {
        NuevaTeleconsulta {
            historia_id: 100,
            modalidad: "08".into(),
            institucion_remota: Some("Hospital San Vicente".into()),
            profesional_remoto: None,
            plataforma: Some("plataforma institucional".into()),
            consentimiento_id: None,
            incidencias: None,
        }
    }

    #[test]
    fn la_modalidad_presencial_se_rechaza() {
        let mut n = nueva();
        n.modalidad = "01".into();
        assert_eq!(revisar(&n), Err(Falta::ModalidadNoEsTelemedicina));
        for (c, _) in MODALIDADES {
            let mut n = nueva();
            n.modalidad = c.into();
            assert_eq!(revisar(&n), Ok(()), "«{c}» sí es telemedicina");
        }
    }

    /// Sin extremo remoto la fila no dice nada que la atención no dijera ya. Con
    /// cualquiera de los dos, basta.
    #[test]
    fn hace_falta_al_menos_un_extremo_remoto() {
        let mut n = nueva();
        n.institucion_remota = None;
        assert_eq!(revisar(&n), Err(Falta::SinExtremoRemoto));
        n.profesional_remoto = Some("Dr. Gómez, cardiología".into());
        assert_eq!(revisar(&n), Ok(()));
        // Y un espacio en blanco no cuenta como extremo.
        n.profesional_remoto = Some("   ".into());
        assert_eq!(revisar(&n), Err(Falta::SinExtremoRemoto));
    }
}

#[cfg(test)]
mod contra_la_base {
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
             VALUES (6,'Dr. Paz','d@x.co','h','medico',1,1)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1), \
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,creado_por,\
             motivo_consulta,enfermedad_actual,plan_tratamiento,diagnostico_principal) \
             VALUES (100,'2026-08-16T09:00:00Z','H100',10,6,6,'dolor','ayer','reposo','A000'), \
                    (200,'2026-08-16T09:00:00Z','H200',20,6,6,'dolor','ayer','reposo','A000')",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    /// **La modalidad también se escribe en `historia`**, que es de donde la leen
    /// el RIPS y el RDA. Sin esto, el archivo que sale al mundo diría que la
    /// atención fue presencial.
    #[tokio::test]
    async fn registrar_deja_la_modalidad_donde_la_lee_el_rips() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        registrar(&pool, a, &super::pruebas::nueva_publica()).await.unwrap().unwrap();
        let m: Option<String> =
            sqlx::query_scalar("SELECT modalidad_grupo_servicio FROM historia WHERE id = 100")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(m.as_deref(), Some("08"));
        let v = listar(&pool, a).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].paciente, "Ana Ruiz");
    }

    #[tokio::test]
    async fn una_atencion_es_teleconsulta_una_sola_vez() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        registrar(&pool, a, &super::pruebas::nueva_publica()).await.unwrap().unwrap();
        assert_eq!(
            registrar(&pool, a, &super::pruebas::nueva_publica()).await.unwrap(),
            Err(Falta::YaEsTeleconsulta)
        );
    }

    #[tokio::test]
    async fn no_se_marca_la_atencion_de_otra_institucion() {
        let pool = base().await;
        let mut n = super::pruebas::nueva_publica();
        n.historia_id = 200;
        assert_eq!(
            registrar(&pool, Ambito::Institucion(1), &n).await.unwrap(),
            Err(Falta::HistoriaInexistente)
        );
        assert!(registrar(&pool, Ambito::Institucion(1), &super::pruebas::nueva_publica())
            .await.unwrap().is_ok());
        assert!(listar(&pool, Ambito::Institucion(2)).await.unwrap().is_empty());
    }

    /// El consentimiento tiene que ser DEL MISMO paciente. Es la comprobación que
    /// ya hace quirófano, y por la misma razón: un consentimiento ajeno es un
    /// consentimiento que no existe.
    #[tokio::test]
    async fn el_consentimiento_ajeno_se_rechaza() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO consentimiento_informado (id,paciente_id,medico_id,tipo_procedimiento,\
             descripcion_procedimiento,riesgos_explicados,beneficios_esperados,\
             alternativas_explicadas) \
             VALUES (1,20,6,'TELEMEDICINA','teleconsulta','los del acto','los del acto','presencial')",
        )
        .execute(&pool).await.unwrap();
        let mut n = super::pruebas::nueva_publica();
        n.consentimiento_id = Some(1);
        assert_eq!(
            registrar(&pool, Ambito::Institucion(1), &n).await.unwrap(),
            Err(Falta::ConsentimientoDeOtroPaciente)
        );
    }
}
