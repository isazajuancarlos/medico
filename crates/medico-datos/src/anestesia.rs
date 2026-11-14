//! El acto anestésico: valoración preanestésica y recuperación (#249).
//!
//! Son los dos módulos del bloque quirúrgico, y se construyen juntos porque son
//! los dos extremos de la misma cirugía: uno dice si el paciente puede entrar,
//! el otro si puede salir.
//!
//! # Lo que había era una CASILLA, no un registro
//!
//! `programacion_quirurgica.valoracion_preanestesica` es un booleano desde la
//! 0047: dice que la valoración se hizo y no dice qué dijo. Es lo mismo que
//! `dispensacion` frente al MAR — la marca sin el hecho. La casilla se queda
//! (la rellena quien recibe al paciente en la puerta) y lo que cambia es que
//! ahora existe el documento, así que deja de ser la única evidencia.
//!
//! # Las dos escalas gobiernan una decisión, y por eso no van en un JSON
//!
//! La ASA decide si el paciente entra; la Aldrete, si sale de recuperación. Un
//! valor que gobierna una decisión clínica en un `contenido` de texto libre es un
//! valor que nadie puede contar ni vigilar. Van como columnas con su rango.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

/// El umbral de Aldrete para salir de recuperación sin explicación.
///
/// No es un muro: por debajo se puede salir —a UCI, o fallecido— pero hay que
/// decir por qué. Es «lo inusual con su razón», no «lo inusual prohibido».
pub const ALDRETE_PARA_SALIR: i64 = 9;

pub const DESTINOS: [(&str, &str); 4] = [
    ("HOSPITALIZACION", "A hospitalización"),
    ("DOMICILIO", "A su domicilio"),
    ("UCI", "A la UCI"),
    ("FALLECIDO", "Falleció"),
];

// ── La valoración preanestésica ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Valoracion {
    pub id: i64,
    pub programacion_id: i64,
    pub paciente_id: i64,
    pub anestesiologo_id: i64,
    pub fecha: String,
    pub asa: i64,
    pub asa_urgencia: bool,
    pub mallampati: Option<i64>,
    pub ayuno_horas: Option<i64>,
    pub comorbilidades: Option<String>,
    pub plan_anestesico: Option<String>,
    pub riesgos_explicados: bool,
    pub apto: bool,
    pub motivo_no_apto: Option<String>,
    pub observaciones: Option<String>,
    pub creado_en: String,
    pub paciente: String,
    pub procedimiento: String,
    pub anestesiologo: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaValoracion {
    pub programacion_id: i64,
    pub asa: i64,
    #[serde(default)]
    pub asa_urgencia: bool,
    #[serde(default)]
    pub mallampati: Option<i64>,
    #[serde(default)]
    pub ayuno_horas: Option<i64>,
    #[serde(default)]
    pub comorbilidades: Option<String>,
    #[serde(default)]
    pub plan_anestesico: Option<String>,
    #[serde(default)]
    pub riesgos_explicados: bool,
    pub apto: bool,
    #[serde(default)]
    pub motivo_no_apto: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

// ── La recuperación postanestésica ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Recuperacion {
    pub id: i64,
    pub programacion_id: i64,
    pub paciente_id: i64,
    pub responsable_id: i64,
    pub ingreso_recuperacion: String,
    pub salida_recuperacion: Option<String>,
    pub aldrete_ingreso: i64,
    pub aldrete_salida: Option<i64>,
    pub complicaciones: Option<String>,
    pub destino: Option<String>,
    pub observaciones: Option<String>,
    pub creado_en: String,
    pub paciente: String,
    pub procedimiento: String,
    pub responsable: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaRecuperacion {
    pub programacion_id: i64,
    pub aldrete_ingreso: i64,
    #[serde(default)]
    pub complicaciones: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

/// Lo que se anota al SACAR al paciente de recuperación.
#[derive(Debug, Clone, Deserialize)]
pub struct Salida {
    pub aldrete_salida: i64,
    pub destino: String,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// La cirugía no existe, o su paciente es de otra institución.
    ProgramacionInexistente,
    AsaFueraDeRango,
    MallampatiFueraDeRango,
    AldreteFueraDeRango,
    /// Declarado NO apto sin decir qué corregir para reprogramar.
    NoAptoSinMotivo,
    DestinoDesconocido,
    /// Sale con Aldrete por debajo del umbral y sin explicación.
    SaleBajoSinExplicacion,
    /// Ya hay valoración (o recuperación) para esa cirugía. Dos contradictorias
    /// dejarían sin saber cuál rige.
    YaExiste,
    /// Se intenta sacar de recuperación a quien ya salió.
    YaSalio,
}

pub const TODAS: [Falta; 9] = [
    Falta::ProgramacionInexistente,
    Falta::AsaFueraDeRango,
    Falta::MallampatiFueraDeRango,
    Falta::AldreteFueraDeRango,
    Falta::NoAptoSinMotivo,
    Falta::DestinoDesconocido,
    Falta::SaleBajoSinExplicacion,
    Falta::YaExiste,
    Falta::YaSalio,
];

impl Falta {
    pub fn clave(self) -> &'static str {
        match self {
            Falta::ProgramacionInexistente => "programacion_inexistente",
            Falta::AsaFueraDeRango => "asa",
            Falta::MallampatiFueraDeRango => "mallampati",
            Falta::AldreteFueraDeRango => "aldrete",
            Falta::NoAptoSinMotivo => "no_apto_sin_motivo",
            Falta::DestinoDesconocido => "destino",
            Falta::SaleBajoSinExplicacion => "sale_bajo_sin_explicacion",
            Falta::YaExiste => "ya_existe",
            Falta::YaSalio => "ya_salio",
        }
    }

    pub fn de_clave(c: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == c)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::ProgramacionInexistente => "esa cirugía no existe en esta institución",
            Falta::AsaFueraDeRango => "la clasificación ASA va de I a VI",
            Falta::MallampatiFueraDeRango => "la clasificación Mallampati va de 1 a 4",
            Falta::AldreteFueraDeRango => "la escala de Aldrete va de 0 a 10",
            Falta::NoAptoSinMotivo => {
                "si el paciente NO es apto hay que decir por qué: sin eso, el cirujano no sabe \
                 qué corregir para reprogramar"
            }
            Falta::DestinoDesconocido => "el destino al salir de recuperación no es uno del producto",
            Falta::SaleBajoSinExplicacion => {
                "sale de recuperación con Aldrete por debajo de 9 y sin explicación. No está \
                 prohibido —se traslada a UCI, o falleció— pero tiene que quedar dicho"
            }
            Falta::YaExiste => "esa cirugía ya tiene registro; dos contradictorios no dejan saber cuál rige",
            Falta::YaSalio => "ese paciente ya salió de recuperación",
        }
    }
}

fn hay(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.trim().is_empty())
}

pub fn revisar_valoracion(n: &NuevaValoracion) -> Result<(), Falta> {
    if !(1..=6).contains(&n.asa) {
        return Err(Falta::AsaFueraDeRango);
    }
    if let Some(m) = n.mallampati
        && !(1..=4).contains(&m)
    {
        return Err(Falta::MallampatiFueraDeRango);
    }
    if !n.apto && !hay(&n.motivo_no_apto) {
        return Err(Falta::NoAptoSinMotivo);
    }
    Ok(())
}

pub fn revisar_salida(s: &Salida) -> Result<(), Falta> {
    if !(0..=10).contains(&s.aldrete_salida) {
        return Err(Falta::AldreteFueraDeRango);
    }
    if !DESTINOS.iter().any(|(c, _)| *c == s.destino) {
        return Err(Falta::DestinoDesconocido);
    }
    if s.aldrete_salida < ALDRETE_PARA_SALIR && !hay(&s.observaciones) {
        return Err(Falta::SaleBajoSinExplicacion);
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

/// El paciente de una cirugía, DENTRO del ámbito. Devuelve `None` si la cirugía
/// no existe o es ajena — las dos se responden igual.
async fn paciente_de(
    pool: &crate::Pool,
    ambito: Ambito,
    programacion_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(None);
    };
    let mut sql = String::from(
        "SELECT p.id FROM programacion_quirurgica g JOIN paciente p ON p.id = g.paciente_id \
         WHERE g.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(programacion_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

const SELECT_VALORACION: &str = "\
    SELECT v.id, v.programacion_id, v.paciente_id, v.anestesiologo_id, v.fecha, v.asa, \
    v.asa_urgencia, v.mallampati, v.ayuno_horas, v.comorbilidades, v.plan_anestesico, \
    v.riesgos_explicados, v.apto, v.motivo_no_apto, v.observaciones, v.creado_en, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente, g.procedimiento, \
    COALESCE(u.nombre,'') AS anestesiologo \
    FROM valoracion_preanestesica v \
    JOIN paciente p ON p.id = v.paciente_id \
    JOIN programacion_quirurgica g ON g.id = v.programacion_id \
    LEFT JOIN usuario u ON u.id = v.anestesiologo_id";

const SELECT_RECUPERACION: &str = "\
    SELECT r.id, r.programacion_id, r.paciente_id, r.responsable_id, r.ingreso_recuperacion, \
    r.salida_recuperacion, r.aldrete_ingreso, r.aldrete_salida, r.complicaciones, r.destino, \
    r.observaciones, r.creado_en, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente, g.procedimiento, \
    COALESCE(u.nombre,'') AS responsable \
    FROM recuperacion_postanestesica r \
    JOIN paciente p ON p.id = r.paciente_id \
    JOIN programacion_quirurgica g ON g.id = r.programacion_id \
    LEFT JOIN usuario u ON u.id = r.responsable_id";

async fn listar_con<T>(
    pool: &crate::Pool,
    ambito: Ambito,
    select: &str,
    orden: &str,
) -> Result<Vec<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(select);
    if inst.is_some() {
        sql.push_str(" WHERE p.institucion_id = ?");
    }
    sql.push_str(orden);
    let mut q = sqlx::query_as::<_, T>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

pub async fn listar_valoraciones(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Valoracion>, sqlx::Error> {
    listar_con(pool, ambito, SELECT_VALORACION, " ORDER BY v.fecha DESC, v.id DESC").await
}

pub async fn listar_recuperaciones(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Recuperacion>, sqlx::Error> {
    listar_con(
        pool,
        ambito,
        SELECT_RECUPERACION,
        " ORDER BY r.ingreso_recuperacion DESC, r.id DESC",
    )
    .await
}

/// ¿Esta cirugía tiene ya su valoración preanestésica?
///
/// Existe para que la pantalla de quirófano pueda enseñar el DOCUMENTO en vez de
/// la casilla: la casilla dice que se hizo, esto dice que está.
pub async fn hay_valoracion(
    pool: &crate::Pool,
    ambito: Ambito,
    programacion_id: i64,
) -> Result<bool, sqlx::Error> {
    if paciente_de(pool, ambito, programacion_id).await?.is_none() {
        return Ok(false);
    }
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM valoracion_preanestesica WHERE programacion_id = ?",
    )
    .bind(programacion_id)
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

pub async fn valorar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaValoracion,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar_valoracion(n) {
        return Ok(Err(f));
    }
    let Some(paciente_id) = paciente_de(pool, ambito, n.programacion_id).await? else {
        return Ok(Err(Falta::ProgramacionInexistente));
    };
    if hay_valoracion(pool, ambito, n.programacion_id).await? {
        return Ok(Err(Falta::YaExiste));
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO valoracion_preanestesica (programacion_id, paciente_id, anestesiologo_id, \
         fecha, asa, asa_urgencia, mallampati, ayuno_horas, comorbilidades, plan_anestesico, \
         riesgos_explicados, apto, motivo_no_apto, observaciones, creado_en) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.programacion_id)
    .bind(paciente_id)
    .bind(usuario_id)
    .bind(&ahora)
    .bind(n.asa)
    .bind(n.asa_urgencia)
    .bind(n.mallampati)
    .bind(n.ayuno_horas)
    .bind(n.comorbilidades.as_deref())
    .bind(n.plan_anestesico.as_deref())
    .bind(n.riesgos_explicados)
    .bind(n.apto)
    .bind(n.motivo_no_apto.as_deref())
    .bind(n.observaciones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Recibe al paciente en recuperación.
pub async fn recibir(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaRecuperacion,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if !(0..=10).contains(&n.aldrete_ingreso) {
        return Ok(Err(Falta::AldreteFueraDeRango));
    }
    let Some(paciente_id) = paciente_de(pool, ambito, n.programacion_id).await? else {
        return Ok(Err(Falta::ProgramacionInexistente));
    };
    let ya: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recuperacion_postanestesica WHERE programacion_id = ?",
    )
    .bind(n.programacion_id)
    .fetch_one(pool)
    .await?;
    if ya > 0 {
        return Ok(Err(Falta::YaExiste));
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO recuperacion_postanestesica (programacion_id, paciente_id, responsable_id, \
         ingreso_recuperacion, aldrete_ingreso, complicaciones, observaciones, creado_en) \
         VALUES (?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.programacion_id)
    .bind(paciente_id)
    .bind(usuario_id)
    .bind(&ahora)
    .bind(n.aldrete_ingreso)
    .bind(n.complicaciones.as_deref())
    .bind(n.observaciones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Saca al paciente de recuperación: puntúa, dice a dónde va y, si sale bajo,
/// por qué.
pub async fn dar_salida(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    s: &Salida,
) -> Result<Result<(), Falta>, sqlx::Error> {
    if let Err(f) = revisar_salida(s) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::ProgramacionInexistente));
    };
    let mut sql = String::from(
        "SELECT r.salida_recuperacion FROM recuperacion_postanestesica r \
         JOIN paciente p ON p.id = r.paciente_id WHERE r.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, Option<String>>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    match q.fetch_optional(pool).await? {
        None => return Ok(Err(Falta::ProgramacionInexistente)),
        Some(Some(_)) => return Ok(Err(Falta::YaSalio)),
        Some(None) => {}
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query(
        "UPDATE recuperacion_postanestesica SET salida_recuperacion = ?, aldrete_salida = ?, \
         destino = ?, observaciones = COALESCE(?, observaciones) WHERE id = ?",
    )
    .bind(&ahora)
    .bind(s.aldrete_salida)
    .bind(&s.destino)
    .bind(s.observaciones.as_deref())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Compartida con `contra_la_base`.
    pub(super) fn valoracion_publica() -> NuevaValoracion {
        valoracion()
    }

    fn valoracion() -> NuevaValoracion {
        NuevaValoracion {
            programacion_id: 1,
            asa: 2,
            asa_urgencia: false,
            mallampati: Some(2),
            ayuno_horas: Some(8),
            comorbilidades: Some("HTA controlada".into()),
            plan_anestesico: Some("general balanceada".into()),
            riesgos_explicados: true,
            apto: true,
            motivo_no_apto: None,
            observaciones: None,
        }
    }

    #[test]
    fn la_valoracion_normal_pasa() {
        assert_eq!(revisar_valoracion(&valoracion()), Ok(()));
    }

    #[test]
    fn el_no_apto_sin_motivo_se_rechaza_y_con_motivo_pasa() {
        let mut n = valoracion();
        n.apto = false;
        assert_eq!(revisar_valoracion(&n), Err(Falta::NoAptoSinMotivo));
        n.motivo_no_apto = Some("ayuno insuficiente: 3 horas".into());
        assert_eq!(revisar_valoracion(&n), Ok(()));
    }

    #[test]
    fn las_seis_asa_se_aceptan_y_la_septima_no() {
        for asa in 1..=6 {
            let mut n = valoracion();
            n.asa = asa;
            assert_eq!(revisar_valoracion(&n), Ok(()), "ASA {asa} existe");
        }
        for asa in [0, 7] {
            let mut n = valoracion();
            n.asa = asa;
            assert_eq!(revisar_valoracion(&n), Err(Falta::AsaFueraDeRango));
        }
    }

    #[test]
    fn mallampati_fuera_de_rango_se_rechaza_pero_ausente_pasa() {
        let mut n = valoracion();
        n.mallampati = Some(5);
        assert_eq!(revisar_valoracion(&n), Err(Falta::MallampatiFueraDeRango));
        n.mallampati = None;
        assert_eq!(revisar_valoracion(&n), Ok(()), "no siempre se explora");
    }

    fn salida(aldrete: i64) -> Salida {
        Salida {
            aldrete_salida: aldrete,
            destino: "DOMICILIO".into(),
            observaciones: None,
        }
    }

    /// El umbral con sus DOS lados: 9 sale limpio, 8 exige explicación, y 8 CON
    /// explicación sale. Sin los tres casos, la regla podría estar prohibiendo
    /// toda salida baja —que sería falso— o no vigilando nada.
    #[test]
    fn el_umbral_de_aldrete_tiene_sus_tres_casos() {
        assert_eq!(revisar_salida(&salida(9)), Ok(()));
        assert_eq!(revisar_salida(&salida(10)), Ok(()));
        assert_eq!(revisar_salida(&salida(8)), Err(Falta::SaleBajoSinExplicacion));
        let mut baja = salida(8);
        baja.destino = "UCI".into();
        baja.observaciones = Some("se traslada a UCI por inestabilidad hemodinámica".into());
        assert_eq!(revisar_salida(&baja), Ok(()), "bajo CON razón sí sale");
    }

    #[test]
    fn el_aldrete_fuera_de_escala_y_el_destino_inventado_se_rechazan() {
        assert_eq!(revisar_salida(&salida(11)), Err(Falta::AldreteFueraDeRango));
        assert_eq!(revisar_salida(&salida(-1)), Err(Falta::AldreteFueraDeRango));
        let mut s = salida(10);
        s.destino = "A_SU_CASA".into();
        assert_eq!(revisar_salida(&s), Err(Falta::DestinoDesconocido));
    }

    #[test]
    fn las_faltas_van_y_vuelven_por_su_clave() {
        let mut vistas = std::collections::BTreeSet::new();
        for f in TODAS {
            assert!(vistas.insert(f.clave()), "clave repetida: {}", f.clave());
            assert_eq!(Falta::de_clave(f.clave()), Some(f));
        }
        assert_eq!(Falta::de_clave("inventada"), None);
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
            "INSERT INTO quirofano (id,institucion_id,nombre) VALUES (1,1,'Sala 1'),(2,2,'Sala A')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO programacion_quirurgica (id,quirofano_id,paciente_id,cirujano_id,\
             fecha_programada,procedimiento) \
             VALUES (1,1,10,6,'2026-08-17T08:00:00Z','Colecistectomía'), \
                    (2,2,20,6,'2026-08-17T08:00:00Z','Hernioplastia')",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn se_valora_y_se_lista() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = valorar(&pool, a, &super::pruebas::valoracion_publica(), 6).await.unwrap().unwrap();
        let v = listar_valoraciones(&pool, a).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, id);
        assert_eq!(v[0].paciente, "Ana Ruiz");
        assert_eq!(v[0].procedimiento, "Colecistectomía");
        // El paciente NO se recibió: se derivó de la cirugía.
        assert_eq!(v[0].paciente_id, 10);
        assert!(hay_valoracion(&pool, a, 1).await.unwrap());
    }

    /// La casilla de la lista de verificación decía que la valoración se hizo;
    /// esto dice si el documento ESTÁ. Sin valoración, `hay_valoracion` es falso
    /// aunque la casilla esté marcada — que es justo el hueco que cierra.
    #[tokio::test]
    async fn sin_documento_no_hay_valoracion_aunque_la_casilla_diga_que_si() {
        let pool = base().await;
        sqlx::query("UPDATE programacion_quirurgica SET valoracion_preanestesica = 1 WHERE id = 1")
            .execute(&pool).await.unwrap();
        assert!(!hay_valoracion(&pool, Ambito::Institucion(1), 1).await.unwrap());
    }

    #[tokio::test]
    async fn no_se_valora_la_cirugia_de_otra_institucion() {
        let pool = base().await;
        let mut n = super::pruebas::valoracion_publica();
        n.programacion_id = 2;
        assert_eq!(
            valorar(&pool, Ambito::Institucion(1), &n, 6).await.unwrap(),
            Err(Falta::ProgramacionInexistente)
        );
        assert!(valorar(&pool, Ambito::Institucion(1), &super::pruebas::valoracion_publica(), 6)
            .await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn no_hay_dos_valoraciones_para_la_misma_cirugia() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        valorar(&pool, a, &super::pruebas::valoracion_publica(), 6).await.unwrap().unwrap();
        assert_eq!(
            valorar(&pool, a, &super::pruebas::valoracion_publica(), 6).await.unwrap(),
            Err(Falta::YaExiste)
        );
    }

    #[tokio::test]
    async fn el_ciclo_de_recuperacion_entera() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = recibir(
            &pool, a,
            &NuevaRecuperacion { programacion_id: 1, aldrete_ingreso: 7, complicaciones: None, observaciones: None },
            6,
        ).await.unwrap().unwrap();
        let r = listar_recuperaciones(&pool, a).await.unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].salida_recuperacion.is_none(), "sigue en recuperación");

        dar_salida(&pool, a, id, &Salida {
            aldrete_salida: 10, destino: "HOSPITALIZACION".into(), observaciones: None,
        }).await.unwrap().unwrap();
        let r = listar_recuperaciones(&pool, a).await.unwrap();
        assert!(r[0].salida_recuperacion.is_some());
        assert_eq!(r[0].aldrete_salida, Some(10));
        assert_eq!(r[0].destino.as_deref(), Some("HOSPITALIZACION"));

        // Y no se sale dos veces: la segunda pisaría la hora de la primera.
        assert_eq!(
            dar_salida(&pool, a, id, &Salida {
                aldrete_salida: 10, destino: "DOMICILIO".into(), observaciones: None,
            }).await.unwrap(),
            Err(Falta::YaSalio)
        );
    }

    /// El `CHECK` de la salida por la puerta de atrás, con su positivo de control.
    #[tokio::test]
    async fn el_check_exige_puntuar_y_decir_destino_para_salir() {
        let pool = base().await;
        let sql = "INSERT INTO recuperacion_postanestesica (programacion_id,paciente_id,\
                   responsable_id,ingreso_recuperacion,aldrete_ingreso,salida_recuperacion,\
                   aldrete_salida,destino,observaciones,creado_en) \
                   VALUES (1,10,6,'2026-08-17T10:00:00Z',8,?,?,?,?,'x')";
        // Positivo de control: la salida COMPLETA entra.
        sqlx::query(sql).bind(Some("2026-08-17T12:00:00Z")).bind(Some(10)).bind(Some("DOMICILIO"))
            .bind(Option::<String>::None)
            .execute(&pool).await.expect("la salida completa tiene que entrar");
        sqlx::query("DELETE FROM recuperacion_postanestesica").execute(&pool).await.unwrap();
        let sin: Option<&str> = None;
        /// (salida, aldrete de salida, destino, observaciones, qué mide)
        type Caso<'a> = (Option<&'a str>, Option<i64>, Option<&'a str>, Option<&'a str>, &'a str);
        let casos: [Caso; 4] = [
            (Some("2026-08-17T12:00:00Z"), None, Some("DOMICILIO"), sin, "salida sin Aldrete"),
            (Some("2026-08-17T12:00:00Z"), Some(10), None, sin, "salida sin destino"),
            (Some("2026-08-17T12:00:00Z"), Some(6), Some("UCI"), sin, "Aldrete bajo sin explicación"),
            (None, Some(10), Some("DOMICILIO"), sin, "puntuación sin salida"),
        ];
        for (sal, ald, dest, obs, que) in casos {
            let r = sqlx::query(sql).bind(sal).bind(ald).bind(dest).bind(obs)
                .execute(&pool).await;
            assert!(r.is_err(), "el CHECK tenía que rechazar: {que}");
        }
        // Y el bajo CON explicación sí entra: la regla es «con su razón», no «prohibido».
        sqlx::query(sql).bind(Some("2026-08-17T12:00:00Z")).bind(Some(6)).bind(Some("UCI"))
            .bind(Some("inestabilidad hemodinámica"))
            .execute(&pool).await.expect("el Aldrete bajo CON razón tiene que entrar");
    }

    #[tokio::test]
    async fn el_ambito_sin_institucion_no_ve_ni_escribe() {
        let pool = base().await;
        assert!(listar_valoraciones(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert!(listar_recuperaciones(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert_eq!(
            valorar(&pool, Ambito::Ninguno, &super::pruebas::valoracion_publica(), 6).await.unwrap(),
            Err(Falta::ProgramacionInexistente)
        );
    }
}
