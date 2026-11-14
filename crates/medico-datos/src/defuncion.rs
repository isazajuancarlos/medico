//! Certificado de defunción (#249). El gemelo del registro de nacimiento.
//!
//! # Lo que medico ya sabía y lo que faltaba
//!
//! El sistema registra que alguien murió en dos sitios —`hospitalizacion.estado`
//! y `atencion_urgencias.destino_salida`— y el RIPS lo reporta. Lo que no existía
//! es el CERTIFICADO: la cadena de causas, la manera de muerte y quién responde.
//! Un egreso que dice «murió» no dice de qué.
//!
//! # Este módulo no deduce la causa básica, y es su regla más importante
//!
//! Elegir cuál de las causas de la cadena es la BÁSICA es un acto médico. Un
//! programa que la dedujera —«la última de la cadena»— produciría estadística
//! vital falsa a escala nacional, y saldría bien formada. Aquí se exige, no se
//! calcula.

use medico_dominio::Ambito;

use crate::pertenencia::{opcional_es_del_paciente, Enlace};
use serde::{Deserialize, Serialize};

pub const SITIOS: [(&str, &str); 4] = [
    ("INSTITUCION", "En la institución"),
    ("DOMICILIO", "En el domicilio"),
    ("VIA_PUBLICA", "En la vía pública"),
    ("OTRO", "Otro sitio"),
];

pub const MANERAS: [(&str, &str); 4] = [
    ("NATURAL", "Natural"),
    ("VIOLENTA", "Violenta"),
    ("EN_ESTUDIO", "En estudio"),
    ("NO_DETERMINADA", "No determinada"),
];

/// La manera que obliga a dejar constancia para el forense.
pub const VIOLENTA: &str = "VIOLENTA";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Defuncion {
    pub id: i64,
    pub paciente_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub atencion_urgencias_id: Option<i64>,
    pub fecha_defuncion: String,
    pub sitio: String,
    pub manera: String,
    pub causa_directa: Option<String>,
    pub antecedente_a: Option<String>,
    pub antecedente_b: Option<String>,
    pub antecedente_c: Option<String>,
    pub causa_basica: String,
    pub numero_certificado: Option<String>,
    pub certificado_por: i64,
    pub observaciones: Option<String>,
    pub anulada: bool,
    pub motivo_anulacion: Option<String>,
    pub creado_en: String,
    pub paciente_nombre: String,
    pub paciente_identificacion: String,
    pub quien_certifica: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Nueva {
    pub paciente_id: i64,
    #[serde(default)]
    pub hospitalizacion_id: Option<i64>,
    #[serde(default)]
    pub atencion_urgencias_id: Option<i64>,
    pub fecha_defuncion: String,
    pub sitio: String,
    pub manera: String,
    #[serde(default)]
    pub causa_directa: Option<String>,
    #[serde(default)]
    pub antecedente_a: Option<String>,
    #[serde(default)]
    pub antecedente_b: Option<String>,
    #[serde(default)]
    pub antecedente_c: Option<String>,
    pub causa_basica: String,
    #[serde(default)]
    pub numero_certificado: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    PacienteInexistente,
    SitioDesconocido,
    ManeraDesconocida,
    /// Sin causa básica no hay certificado: es la que va a la estadística.
    SinCausaBasica,
    /// Muerte violenta sin una línea de constancia. No se exige un texto
    /// concreto —eso sería fingir que el sistema entiende el caso— pero sí que
    /// haya alguno: el forense trabaja con esto.
    ViolentaSinConstancia,
    /// Ya hay un certificado vivo para ese paciente. Nadie muere dos veces; si
    /// hay que corregirlo, se anula el anterior y se expide otro.
    YaTieneCertificado,
    SinMotivoDeAnulacion,
    YaAnulada,
    /// La fecha de defunción es anterior al ingreso que se le enlaza. Es
    /// IMPOSIBLE, no inusual: el paciente no pudo morir antes de llegar.
    MurioAntesDeIngresar,
    /// El ingreso o la atención de urgencias que se enlaza **no es de este
    /// paciente** — o no existe, o es de otra institución. Las tres se responden
    /// igual a propósito: distinguirlas diría que existe.
    AtencionQueNoEsDelPaciente,
    /// La fecha de defunción no se pudo interpretar como fecha-hora, o cae en un
    /// instante que no existe en la zona del prestador. No se guarda un valor
    /// crudo ni un default (D20): sin fecha comparable, el fail-safe #1 no puede
    /// proteger, así que se rechaza el certificado.
    FechaDefuncionInvalida,
}

pub const TODAS: [Falta; 11] = [
    Falta::PacienteInexistente,
    Falta::SitioDesconocido,
    Falta::ManeraDesconocida,
    Falta::SinCausaBasica,
    Falta::ViolentaSinConstancia,
    Falta::YaTieneCertificado,
    Falta::SinMotivoDeAnulacion,
    Falta::YaAnulada,
    Falta::MurioAntesDeIngresar,
    Falta::AtencionQueNoEsDelPaciente,
    Falta::FechaDefuncionInvalida,
];

impl Falta {
    pub fn clave(self) -> &'static str {
        match self {
            Falta::PacienteInexistente => "paciente_inexistente",
            Falta::SitioDesconocido => "sitio",
            Falta::ManeraDesconocida => "manera",
            Falta::SinCausaBasica => "sin_causa_basica",
            Falta::ViolentaSinConstancia => "violenta_sin_constancia",
            Falta::YaTieneCertificado => "ya_tiene_certificado",
            Falta::SinMotivoDeAnulacion => "sin_motivo_anulacion",
            Falta::YaAnulada => "ya_anulada",
            Falta::MurioAntesDeIngresar => "murio_antes_de_ingresar",
            Falta::AtencionQueNoEsDelPaciente => "atencion_que_no_es_del_paciente",
            Falta::FechaDefuncionInvalida => "fecha_defuncion_invalida",
        }
    }

    pub fn de_clave(clave: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == clave)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::PacienteInexistente => "ese paciente no existe en esta institución",
            Falta::SitioDesconocido => "el sitio de la defunción no es uno de los del certificado",
            Falta::ManeraDesconocida => "la manera de muerte no es una de las del certificado",
            Falta::SinCausaBasica => {
                "la causa básica de muerte es obligatoria: es la que va a la estadística vital, \
                 y elegirla es un acto médico que el sistema no puede deducir de la cadena"
            }
            Falta::ViolentaSinConstancia => {
                "una muerte violenta tiene que dejar constancia en las observaciones"
            }
            Falta::YaTieneCertificado => {
                "ese paciente ya tiene un certificado de defunción vigente. Para corregirlo, \
                 anule el anterior y expida uno nuevo"
            }
            Falta::SinMotivoDeAnulacion => "para anular un certificado hay que decir por qué",
            Falta::YaAnulada => "ese certificado ya estaba anulado",
            Falta::MurioAntesDeIngresar => {
                "la fecha de defunción es anterior al ingreso que se le enlaza"
            }
            Falta::AtencionQueNoEsDelPaciente => {
                "la hospitalización o la atención de urgencias que se enlaza no es de ese paciente"
            }
            Falta::FechaDefuncionInvalida => {
                "la fecha de defunción no es una fecha-hora válida en la zona del prestador"
            }
        }
    }
}

/// Comprueba lo que se puede comprobar sin base.
pub fn revisar(n: &Nueva) -> Result<(), Falta> {
    if !SITIOS.iter().any(|(c, _)| *c == n.sitio) {
        return Err(Falta::SitioDesconocido);
    }
    if !MANERAS.iter().any(|(c, _)| *c == n.manera) {
        return Err(Falta::ManeraDesconocida);
    }
    if n.causa_basica.trim().is_empty() {
        return Err(Falta::SinCausaBasica);
    }
    let hay_constancia = n
        .observaciones
        .as_deref()
        .is_some_and(|o| !o.trim().is_empty());
    if n.manera == VIOLENTA && !hay_constancia {
        return Err(Falta::ViolentaSinConstancia);
    }
    Ok(())
}

const SELECT_ENRIQUECIDO: &str = "\
    SELECT d.id, d.paciente_id, d.hospitalizacion_id, d.atencion_urgencias_id, \
    d.fecha_defuncion, d.sitio, d.manera, d.causa_directa, d.antecedente_a, d.antecedente_b, \
    d.antecedente_c, d.causa_basica, d.numero_certificado, d.certificado_por, d.observaciones, \
    d.anulada, d.motivo_anulacion, d.creado_en, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) \
        AS paciente_nombre, \
    p.numero_documento AS paciente_identificacion, \
    COALESCE(u.nombre,'') AS quien_certifica \
    FROM defuncion d JOIN paciente p ON p.id = d.paciente_id \
    LEFT JOIN usuario u ON u.id = d.certificado_por";

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

/// Lista los certificados del ámbito, del más reciente al más antiguo.
///
/// Incluye los ANULADOS, por lo mismo que la hoja del MAR: esconderlos diría que
/// el certificado nunca se expidió, y lo que pasó es que se corrigió.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Vec<Defuncion>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_ENRIQUECIDO);
    sql.push_str(" WHERE 1=1");
    if desde.is_some() {
        sql.push_str(" AND date(d.fecha_defuncion) >= date(?)");
    }
    if hasta.is_some() {
        sql.push_str(" AND date(d.fecha_defuncion) <= date(?)");
    }
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    sql.push_str(" ORDER BY d.fecha_defuncion DESC, d.id DESC");
    let mut q = sqlx::query_as::<_, Defuncion>(&sql);
    if let Some(d) = desde {
        q = q.bind(d);
    }
    if let Some(h) = hasta {
        q = q.bind(h);
    }
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// El certificado VIGENTE de un paciente, si lo tiene.
///
/// Es lo que alimenta `Patient.deceased` del RDA sin tener que deducirlo del
/// egreso: el certificado es la declaración, el egreso es la atención.
pub async fn vigente_de(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<Option<Defuncion>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(None);
    };
    let mut sql = String::from(SELECT_ENRIQUECIDO);
    sql.push_str(" WHERE d.paciente_id = ? AND d.anulada = 0");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_as::<_, Defuncion>(&sql).bind(paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Expide un certificado. Devuelve el id.
pub async fn certificar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &Nueva,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::PacienteInexistente));
    };
    let mut comprobar = String::from("SELECT 1 FROM paciente WHERE id = ?");
    if inst.is_some() {
        comprobar.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&comprobar).bind(n.paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::PacienteInexistente));
    }

    // Nadie muere dos veces. Se comprueba aquí para poder DECIR por qué; el
    // índice único parcial lo impide igual por la puerta de atrás.
    if vigente_de(pool, ambito, n.paciente_id).await?.is_some() {
        return Ok(Err(Falta::YaTieneCertificado));
    }

    // **La atención que se enlaza tiene que ser DE ESTE PACIENTE.** Sin esto, la
    // consulta de abajo leía `fecha_ingreso` de CUALQUIER hospitalización de
    // CUALQUIER institución —era la única de las 9 consultas `FROM
    // hospitalizacion` de este crate sin `JOIN paciente`—, y la diferencia entre
    // `MurioAntesDeIngresar` y el resto la convertía en un oráculo: por bisección
    // sobre `fecha_defuncion` se reconstruía la fecha y hora de ingreso de un
    // paciente de otro prestador, sin escribir una fila en la rama que porta la
    // señal. La existencia de un ingreso ajeno es dato de salud, y
    // `aislamiento.rs` ya declara por escrito que es justo lo que no se filtra.
    //
    // Se acota por PACIENTE y no solo por institución porque la regla clínica es
    // esa —el certificado enlaza la atención en la que murió—, y de paso cierra
    // el oráculo entero en vez de estrecharlo a los ingresos de la casa.
    for (enlace, id) in [
        (Enlace::Hospitalizacion, n.hospitalizacion_id),
        (Enlace::Urgencias, n.atencion_urgencias_id),
    ] {
        if !opcional_es_del_paciente(pool, enlace, id, n.paciente_id).await? {
            return Ok(Err(Falta::AtencionQueNoEsDelPaciente));
        }
    }

    // La fecha entra CRUDA del formulario (hora local naive); se sella a UTC antes de
    // TODO uso, para que el fail-safe #1 (que compara contra actos en UTC) y la
    // comparación de abajo trabajen en el mismo marco. Sin fecha comparable no hay
    // certificado (D20).
    let Some(fecha_utc) = medico_dominio::zona::a_utc_rfc3339(&n.fecha_defuncion) else {
        return Ok(Err(Falta::FechaDefuncionInvalida));
    };

    // Lo IMPOSIBLE, no lo inusual (directiva 21): morir antes de ingresar. NO se
    // exige que la hospitalización esté ya egresada como DEFUNCION — certificar
    // en el momento de la muerte, antes de cerrar el egreso, es el orden normal
    // de trabajo, y bloquearlo sería el sesgo a cerrar. La comparación es correcta
    // ahora que `fecha_utc` está en UTC como el `fecha_ingreso` que escribe el sistema.
    if let Some(h) = n.hospitalizacion_id {
        let ingreso: Option<String> = sqlx::query_scalar(
            "SELECT fecha_ingreso FROM hospitalizacion WHERE id = ? AND paciente_id = ?",
        )
        .bind(h)
        .bind(n.paciente_id)
        .fetch_optional(pool)
        .await?;
        if let Some(ing) = ingreso
            && fecha_utc.as_str() < ing.as_str()
        {
            return Ok(Err(Falta::MurioAntesDeIngresar));
        }
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO defuncion (paciente_id, hospitalizacion_id, atencion_urgencias_id, \
         fecha_defuncion, sitio, manera, causa_directa, antecedente_a, antecedente_b, \
         antecedente_c, causa_basica, numero_certificado, certificado_por, observaciones, \
         creado_en) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.paciente_id)
    .bind(n.hospitalizacion_id)
    .bind(n.atencion_urgencias_id)
    .bind(&fecha_utc)
    .bind(&n.sitio)
    .bind(&n.manera)
    .bind(n.causa_directa.as_deref())
    .bind(n.antecedente_a.as_deref())
    .bind(n.antecedente_b.as_deref())
    .bind(n.antecedente_c.as_deref())
    .bind(n.causa_basica.trim())
    .bind(n.numero_certificado.as_deref())
    .bind(usuario_id)
    .bind(n.observaciones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Anula un certificado. No lo borra: es prueba, igual que el MAR.
pub async fn anular(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    motivo: &str,
    usuario_id: i64,
) -> Result<Result<(), Falta>, sqlx::Error> {
    if motivo.trim().is_empty() {
        return Ok(Err(Falta::SinMotivoDeAnulacion));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::PacienteInexistente));
    };
    let mut sql = String::from(
        "SELECT d.anulada FROM defuncion d JOIN paciente p ON p.id = d.paciente_id WHERE d.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, bool>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    match q.fetch_optional(pool).await? {
        None => return Ok(Err(Falta::PacienteInexistente)),
        Some(true) => return Ok(Err(Falta::YaAnulada)),
        Some(false) => {}
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query(
        "UPDATE defuncion SET anulada = 1, anulada_por = ?, anulada_en = ?, \
         motivo_anulacion = ? WHERE id = ?",
    )
    .bind(usuario_id)
    .bind(&ahora)
    .bind(motivo.trim())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nueva() -> Nueva {
        Nueva {
            paciente_id: 10,
            hospitalizacion_id: None,
            atencion_urgencias_id: None,
            fecha_defuncion: "2026-08-16T04:30:00Z".into(),
            sitio: "INSTITUCION".into(),
            manera: "NATURAL".into(),
            causa_directa: Some("J960".into()),
            antecedente_a: Some("J180".into()),
            antecedente_b: None,
            antecedente_c: None,
            causa_basica: "J189".into(),
            numero_certificado: Some("A-0001".into()),
            observaciones: None,
        }
    }

    #[test]
    fn el_certificado_normal_pasa() {
        assert_eq!(revisar(&nueva()), Ok(()));
    }

    #[test]
    fn sin_causa_basica_se_rechaza() {
        let mut n = nueva();
        n.causa_basica = "  ".into();
        assert_eq!(revisar(&n), Err(Falta::SinCausaBasica));
    }

    /// La causa básica NO se deduce de la cadena: un certificado con cadena
    /// completa y sin básica se rechaza igual. Es la regla del módulo.
    #[test]
    fn la_cadena_completa_no_sustituye_a_la_causa_basica() {
        let mut n = nueva();
        n.causa_directa = Some("J960".into());
        n.antecedente_a = Some("J180".into());
        n.antecedente_b = Some("E119".into());
        n.causa_basica = String::new();
        assert_eq!(revisar(&n), Err(Falta::SinCausaBasica));
    }

    #[test]
    fn la_violenta_sin_constancia_se_rechaza() {
        let mut n = nueva();
        n.manera = "VIOLENTA".into();
        assert_eq!(revisar(&n), Err(Falta::ViolentaSinConstancia));
    }

    /// Su pareja: con constancia pasa. Sin este caso, la regla podría estar
    /// rechazando toda muerte violenta.
    #[test]
    fn la_violenta_con_constancia_pasa() {
        let mut n = nueva();
        n.manera = "VIOLENTA".into();
        n.observaciones = Some("herida por arma de fuego; se avisa a Fiscalía".into());
        assert_eq!(revisar(&n), Ok(()));
    }

    /// Y las otras tres maneras NO exigen constancia: la regla es de la
    /// violenta, no de todas.
    #[test]
    fn las_otras_maneras_no_exigen_constancia() {
        for (m, _) in MANERAS.iter().filter(|(m, _)| *m != VIOLENTA) {
            let mut n = nueva();
            n.manera = (*m).into();
            assert_eq!(revisar(&n), Ok(()), "«{m}» no debería exigir constancia");
        }
    }

    #[test]
    fn los_cuatro_sitios_del_certificado_se_aceptan() {
        for (s, _) in SITIOS {
            let mut n = nueva();
            n.sitio = s.into();
            assert_eq!(revisar(&n), Ok(()), "«{s}» es un sitio del certificado");
        }
    }

    #[test]
    fn el_sitio_o_la_manera_inventados_se_rechazan() {
        let mut n = nueva();
        n.sitio = "EN_EL_MAR".into();
        assert_eq!(revisar(&n), Err(Falta::SitioDesconocido));
        let mut n = nueva();
        n.manera = "SOSPECHOSA".into();
        assert_eq!(revisar(&n), Err(Falta::ManeraDesconocida));
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
             VALUES (10,'CC','1','Ana','Ruiz','1940-01-01','F',1), \
                    (20,'CC','2','Beto','Paz','1935-01-01','M',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (1,'101','1','MEDICINA','OCUPADA',1)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (77,10,1,6,'2026-08-15T10:00:00Z','ACTIVA')",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    fn de(paciente: i64) -> Nueva {
        let mut n = super::pruebas_nueva();
        n.paciente_id = paciente;
        n
    }

    // ── H2 del concilio 2026-09-26: fecha_defuncion se sella a UTC ──

    /// El camino de escritura SELLA la fecha NAIVE del formulario a UTC. Sin esto,
    /// `datetime()` la leía como UTC y el fail-safe #1 corría la muerte 5 h. 10:00 en
    /// Bogotá (-05) → 15:00 UTC.
    #[tokio::test]
    async fn certificar_sella_la_fecha_naive_a_utc() {
        let pool = base().await;
        let mut n = de(10);
        n.fecha_defuncion = "2026-08-16T10:00".into(); // naive, como el <input datetime-local>
        let id = certificar(&pool, Ambito::Institucion(1), &n, 6).await.unwrap().unwrap();
        let guardada: String =
            sqlx::query_scalar("SELECT fecha_defuncion FROM defuncion WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(guardada, "2026-08-16T15:00:00Z", "la hora local -05 se sella a UTC");
    }

    /// Una fecha que ya trae zona (`Z`) NO se corre: se normaliza a UTC igual (idempotente).
    #[tokio::test]
    async fn certificar_deja_en_utc_una_fecha_ya_zonificada() {
        let pool = base().await;
        let id = certificar(&pool, Ambito::Institucion(1), &de(10), 6).await.unwrap().unwrap();
        let guardada: String =
            sqlx::query_scalar("SELECT fecha_defuncion FROM defuncion WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(guardada, "2026-08-16T04:30:00Z", "una fecha ya en UTC se conserva");
    }

    /// Una fecha de defunción ilegible se RECHAZA (D20): sin fecha comparable el fail-safe
    /// #1 no puede proteger, así que no se guarda un valor crudo ni un default.
    #[tokio::test]
    async fn una_fecha_de_defuncion_ilegible_se_rechaza() {
        let pool = base().await;
        let mut n = de(10);
        n.fecha_defuncion = "ayer por la tarde".into();
        assert_eq!(
            certificar(&pool, Ambito::Institucion(1), &n, 6).await.unwrap(),
            Err(Falta::FechaDefuncionInvalida)
        );
    }

    /// RED→VERDE de extremo (D33): un fallecido REAL hace 21 años, con la muerte tecleada
    /// en formato del formulario (hora local naive) y una vacuna 2 h ANTES de morir, SIGUE
    /// siendo elegible. Antes, la fecha naive leída como UTC corría la muerte 5 h y la
    /// vacuna (UTC real) caía "después" → NO elegible para siempre. Mutar
    /// `zona::a_utc_rfc3339` para que devuelva la cadena cruda vuelve a poner 0.
    #[tokio::test]
    async fn una_muerte_naive_con_acto_previo_sigue_siendo_elegible() {
        let pool = base().await;
        let muerte_utc = chrono::Utc::now() - chrono::Duration::days(21 * 365);
        let muerte_local = muerte_utc
            .with_timezone(&medico_dominio::zona::POR_DEFECTO)
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        // Paciente 20: sin hospitalización (base() solo hospitaliza al 10, en 2026).
        let mut n = de(20);
        n.fecha_defuncion = muerte_local;
        certificar(&pool, Ambito::Institucion(2), &n, 6).await.unwrap().unwrap();
        // Una vacuna 2 h ANTES de morir (UTC real): acto de vida legítimo pre-mortem.
        let antes = (muerte_utc - chrono::Duration::hours(2))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO vacuna_aplicada (paciente_id,aplicado_por,fecha,biologico,dosis_numero,lote,creado_en) \
             VALUES (20,6,?,'BCG',1,'L-1',?)",
        )
        .bind(&antes)
        .bind(&antes)
        .execute(&pool)
        .await
        .unwrap();
        let e = crate::retencion::elegibles(
            &pool,
            Ambito::Superadmin,
            chrono::Utc::now(),
            crate::retencion::ANOS_POR_DEFECTO,
        )
        .await
        .unwrap();
        assert_eq!(e.len(), 1, "un fallecido real con una vacuna 2 h ANTES de morir sigue elegible");
    }

    /// La migración 0078 pasa a UTC las defunciones NAIVE existentes y deja intactas las
    /// que ya traen zona (D22). Corre el SQL REAL del archivo sobre filas sembradas a mano.
    #[tokio::test]
    async fn la_migracion_0078_normaliza_solo_las_fechas_naive() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO defuncion (id,paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (1,10,'2005-06-15T10:00','INSTITUCION','NATURAL','J189',6,0,'2005-06-15T10:00:00Z')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO defuncion (id,paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (2,20,'2005-06-15T10:00:00Z','INSTITUCION','NATURAL','J189',6,0,'2005-06-15T10:00:00Z')",
        ).execute(&pool).await.unwrap();
        sqlx::query(include_str!("../migrations/0078_defuncion_a_utc.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let naive: String = sqlx::query_scalar("SELECT fecha_defuncion FROM defuncion WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        let zonif: String = sqlx::query_scalar("SELECT fecha_defuncion FROM defuncion WHERE id = 2")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(naive, "2005-06-15T15:00:00Z", "la naive -05 se corrió +5 h a UTC");
        assert_eq!(zonif, "2005-06-15T10:00:00Z", "la que ya estaba en UTC no se tocó");
    }

    #[tokio::test]
    async fn se_certifica_y_se_lista() {
        let pool = base().await;
        let id = certificar(&pool, Ambito::Institucion(1), &de(10), 6).await.unwrap().unwrap();
        let filas = listar(&pool, Ambito::Institucion(1), None, None).await.unwrap();
        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].id, id);
        assert_eq!(filas[0].paciente_nombre, "Ana Ruiz");
        assert_eq!(filas[0].quien_certifica, "Dr. Paz");
        assert_eq!(filas[0].causa_basica, "J189");
    }

    /// Nadie muere dos veces. Y la corrección SÍ es posible: anular y reexpedir.
    /// Las dos mitades en la misma prueba, porque una sin la otra deja el módulo
    /// o abierto o inservible.
    #[tokio::test]
    async fn no_hay_dos_certificados_vivos_pero_si_se_puede_reexpedir() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let primero = certificar(&pool, a, &de(10), 6).await.unwrap().unwrap();
        assert_eq!(
            certificar(&pool, a, &de(10), 6).await.unwrap(),
            Err(Falta::YaTieneCertificado)
        );
        anular(&pool, a, primero, "causa básica equivocada", 6).await.unwrap().unwrap();
        // Ahora sí entra el nuevo: el índice único es PARCIAL sobre las no
        // anuladas, y sin ese `WHERE` la reexpedición chocaría con el anulado.
        assert!(certificar(&pool, a, &de(10), 6).await.unwrap().is_ok());
        assert_eq!(listar(&pool, a, None, None).await.unwrap().len(), 2, "el anulado sigue");
    }

    #[tokio::test]
    async fn no_se_certifica_al_paciente_de_otra_institucion() {
        let pool = base().await;
        assert_eq!(
            certificar(&pool, Ambito::Institucion(1), &de(20), 6).await.unwrap(),
            Err(Falta::PacienteInexistente)
        );
        // Positivo de control: el propio sí entra.
        assert!(certificar(&pool, Ambito::Institucion(1), &de(10), 6).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn una_institucion_no_ve_los_certificados_de_la_otra() {
        let pool = base().await;
        certificar(&pool, Ambito::Institucion(1), &de(10), 6).await.unwrap().unwrap();
        assert!(listar(&pool, Ambito::Institucion(2), None, None).await.unwrap().is_empty());
        assert_eq!(listar(&pool, Ambito::Institucion(1), None, None).await.unwrap().len(), 1);
    }

    /// Morir antes de ingresar es IMPOSIBLE y se rechaza.
    #[tokio::test]
    async fn no_se_muere_antes_de_ingresar() {
        let pool = base().await;
        let mut n = de(10);
        n.hospitalizacion_id = Some(77);
        n.fecha_defuncion = "2026-08-14T09:00:00Z".into();  // el ingreso es el 15
        assert_eq!(
            certificar(&pool, Ambito::Institucion(1), &n, 6).await.unwrap(),
            Err(Falta::MurioAntesDeIngresar)
        );
    }

    /// Y su pareja, que es la que impide el sesgo a cerrar (directiva 21):
    /// certificar sobre una hospitalización **todavía ACTIVA** SÍ se permite.
    /// Certificar en el momento de la muerte, antes de cerrar el egreso, es el
    /// orden normal de trabajo; exigir el egreso primero bloquearía al cliente.
    #[tokio::test]
    async fn se_certifica_sobre_una_hospitalizacion_todavia_abierta() {
        let pool = base().await;
        let mut n = de(10);
        n.hospitalizacion_id = Some(77);
        n.fecha_defuncion = "2026-08-16T04:30:00Z".into();
        assert!(certificar(&pool, Ambito::Institucion(1), &n, 6).await.unwrap().is_ok());
    }

    /// **El ingreso que se enlaza tiene que ser de ESTE paciente**, y las tres
    /// formas de que no lo sea se responden IGUAL.
    ///
    /// Caso rojo del oráculo entre instituciones que encontró la revisión de
    /// seguridad de esta rama (tres revisores por separado). Antes, la consulta
    /// de `fecha_ingreso` no llevaba ámbito ninguno: era la única de las nueve
    /// `FROM hospitalizacion` del crate sin `JOIN paciente`. Con eso, la
    /// diferencia entre `MurioAntesDeIngresar` y el resto respondía a una
    /// pregunta sobre datos de OTRO prestador, y la rama que porta la señal
    /// retorna antes del `INSERT` — o sea, sonda gratis y repetible.
    ///
    /// **Lo que discrimina de verdad es la segunda mitad**: no basta con que el
    /// ajeno se rechace, tiene que rechazarse con la MISMA respuesta a los dos
    /// lados de la fecha. Si contestara distinto, la bisección seguiría en pie
    /// aunque el `INSERT` no ocurriera nunca.
    #[tokio::test]
    async fn el_ingreso_que_no_es_del_paciente_no_contesta_distinto() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (2,'201','2','MEDICINA','OCUPADA',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (7,'Dra. Sol','s@x.co','h','medico',2,1)",
        )
        .execute(&pool).await.unwrap();
        // El secreto que se quería sonsacar: la hora de ingreso de un paciente
        // de la institución 2.
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (88,20,2,7,'2026-08-14T03:17:00Z','ACTIVA')",
        )
        .execute(&pool).await.unwrap();

        let a = Ambito::Institucion(1);
        let sonda = |fecha: &str, h: i64| {
            let mut n = de(10);
            n.hospitalizacion_id = Some(h);
            n.fecha_defuncion = fecha.to_string();
            n
        };

        // Los dos lados de la hora de ingreso ajena, y el id que no existe:
        // las tres respuestas tienen que ser la misma.
        for (fecha, h, que) in [
            ("2026-08-14T00:00:00Z", 88, "antes del ingreso ajeno"),
            ("2026-08-15T00:00:00Z", 88, "después del ingreso ajeno"),
            ("2026-08-15T00:00:00Z", 9999, "un id que no existe"),
        ] {
            assert_eq!(
                certificar(&pool, a, &sonda(fecha, h), 6).await.unwrap(),
                Err(Falta::AtencionQueNoEsDelPaciente),
                "{que}: una respuesta distinta a las otras dos es el oráculo"
            );
        }
        assert!(
            listar(&pool, a, None, None).await.unwrap().is_empty(),
            "ninguna sonda puede haber expedido un certificado"
        );

        // POSITIVO DE CONTROL, sin el cual esto pasaría con una función que
        // rechazara SIEMPRE: el ingreso propio sigue entrando.
        assert!(certificar(&pool, a, &sonda("2026-08-16T04:30:00Z", 77), 6)
            .await
            .unwrap()
            .is_ok());
    }

    /// `vigente_de` es lo que alimentaría `Patient.deceased`: tras anular, ya no
    /// hay declaración vigente, y eso NO es lo mismo que decir que está vivo.
    #[tokio::test]
    async fn el_vigente_desaparece_al_anular() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = certificar(&pool, a, &de(10), 6).await.unwrap().unwrap();
        assert!(vigente_de(&pool, a, 10).await.unwrap().is_some());
        anular(&pool, a, id, "se expidió al paciente equivocado", 6).await.unwrap().unwrap();
        assert!(vigente_de(&pool, a, 10).await.unwrap().is_none());
        assert_eq!(anular(&pool, a, id, "otra vez", 6).await.unwrap(), Err(Falta::YaAnulada));
    }

    #[tokio::test]
    async fn anular_sin_motivo_se_rechaza() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = certificar(&pool, a, &de(10), 6).await.unwrap().unwrap();
        assert_eq!(anular(&pool, a, id, "  ", 6).await.unwrap(), Err(Falta::SinMotivoDeAnulacion));
    }

    /// El `CHECK` por la puerta de atrás, con su positivo de control.
    #[tokio::test]
    async fn el_check_rechaza_la_violenta_muda_y_el_vocabulario_inventado() {
        let pool = base().await;
        let sql = "INSERT INTO defuncion (paciente_id,fecha_defuncion,sitio,manera,causa_basica,\
                   certificado_por,creado_en,observaciones) \
                   VALUES (10,'2026-08-16T04:30:00Z',?,?,'J189',6,'x',?)";
        sqlx::query(sql).bind("INSTITUCION").bind("VIOLENTA").bind(Some("con constancia"))
            .execute(&pool).await.expect("la violenta CON constancia tiene que entrar");
        for (sitio, manera, obs, que) in [
            ("INSTITUCION", "VIOLENTA", None, "violenta sin constancia"),
            ("INSTITUCION", "VIOLENTA", Some("   "), "constancia en blanco"),
            ("EN_EL_MAR", "NATURAL", None, "sitio inventado"),
            ("INSTITUCION", "SOSPECHOSA", None, "manera inventada"),
        ] {
            let r = sqlx::query(sql).bind(sitio).bind(manera).bind(obs).execute(&pool).await;
            assert!(r.is_err(), "el CHECK tenía que rechazar: {que}");
        }
    }
}

/// La compartida por los dos módulos de prueba.
#[cfg(test)]
fn pruebas_nueva() -> Nueva {
    Nueva {
        paciente_id: 10,
        hospitalizacion_id: None,
        atencion_urgencias_id: None,
        fecha_defuncion: "2026-08-16T04:30:00Z".into(),
        sitio: "INSTITUCION".into(),
        manera: "NATURAL".into(),
        causa_directa: Some("J960".into()),
        antecedente_a: Some("J180".into()),
        antecedente_b: None,
        antecedente_c: None,
        causa_basica: "J189".into(),
        numero_certificado: None,
        observaciones: None,
    }
}
