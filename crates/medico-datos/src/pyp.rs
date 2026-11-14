//! Promoción y prevención (#249): los cuatro programas del bloque.
//!
//! # El vocabulario sale del catálogo del MUV que ya estaba en el repositorio
//!
//! Iba a escribir los códigos citando la norma de memoria. No hizo falta:
//! `muv_finalidad.json` está vendorizado desde antes y es la tabla del
//! validador. Los cuatro programas se anclan a su finalidad, y eso tiene premio
//! inmediato: la consulta de un control de P&P puede salir en el RIPS con su
//! código propio en vez del `12` genérico.
//!
//! # Dos formas, no cuatro
//!
//! Tres son controles periódicos y el cuarto es un registro de dosis. Meter la
//! vacuna en la forma del control dejaría «hallazgos» y «conducta» nulos siempre
//! y el lote en un campo libre.
//!
//! # La inscripción y el control son DOS cosas
//!
//! Se puede estar inscrito sin haber ido nunca —y eso es justo lo que el
//! indicador de adherencia mide—. Un modelo que solo guardara controles no podría
//! distinguir «no está en el programa» de «está y no ha venido», que son las dos
//! respuestas opuestas a la misma pregunta.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

use crate::pertenencia::{opcional_es_del_paciente, Enlace};

/// Los cuatro programas, con su finalidad del MUV y su nombre.
///
/// La finalidad **no se inventa**: sale de `muv_finalidad.json`, y
/// `las_cuatro_finalidades_existen_en_el_catalogo_del_muv` lo ata al artefacto —
/// si alguien cambia un código por otro que no está en la tabla del validador,
/// la prueba se pone roja antes de que el MUV rechace el paquete.
pub const PROGRAMAS: [(&str, &str, &str); 4] = [
    ("CRECIMIENTO_DESARROLLO", "11", "Crecimiento y desarrollo"),
    ("VACUNACION", "14", "Vacunación"),
    ("SALUD_REPRODUCTIVA", "19", "Salud reproductiva y planificación"),
    ("PRENATAL", "23", "Control prenatal"),
];

/// El programa donde las semanas de gestación son obligatorias.
pub const PRENATAL: &str = "PRENATAL";
/// El programa que no lleva controles sino dosis.
pub const VACUNACION: &str = "VACUNACION";

pub fn finalidad_de(programa: &str) -> Option<&'static str> {
    PROGRAMAS
        .iter()
        .find(|(p, _, _)| *p == programa)
        .map(|(_, f, _)| *f)
}

pub fn nombre_de(programa: &str) -> Option<&'static str> {
    PROGRAMAS
        .iter()
        .find(|(p, _, _)| *p == programa)
        .map(|(_, _, n)| *n)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Inscripcion {
    pub id: i64,
    pub paciente_id: i64,
    pub programa: String,
    pub finalidad_muv: String,
    pub fecha_inscripcion: String,
    pub estado: String,
    pub fecha_egreso: Option<String>,
    pub motivo_egreso: Option<String>,
    pub creado_en: String,
    pub paciente: String,
    pub identificacion: String,
    /// Cuántos controles lleva. Sale de contar, no de una columna que se
    /// actualiza — un contador guardado se desincroniza al primer borrado.
    pub controles: i64,
    pub ultimo_control: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaInscripcion {
    pub paciente_id: i64,
    pub programa: String,
    pub fecha_inscripcion: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Control {
    pub id: i64,
    pub inscripcion_id: i64,
    pub paciente_id: i64,
    pub profesional_id: i64,
    pub fecha: String,
    pub numero_control: i64,
    pub semanas_gestacion: Option<i64>,
    pub riesgo: Option<String>,
    pub hallazgos: Option<String>,
    pub conducta: Option<String>,
    pub proximo_control: Option<String>,
    pub creado_en: String,
    pub paciente: String,
    pub programa: String,
    pub profesional: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevoControl {
    pub inscripcion_id: i64,
    pub fecha: String,
    #[serde(default)]
    pub semanas_gestacion: Option<i64>,
    #[serde(default)]
    pub riesgo: Option<String>,
    #[serde(default)]
    pub hallazgos: Option<String>,
    #[serde(default)]
    pub conducta: Option<String>,
    #[serde(default)]
    pub proximo_control: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Vacuna {
    pub id: i64,
    pub paciente_id: i64,
    pub inscripcion_id: Option<i64>,
    pub aplicado_por: i64,
    pub fecha: String,
    pub biologico: String,
    pub dosis_numero: i64,
    pub lote: String,
    pub vencimiento_lote: Option<String>,
    pub via: Option<String>,
    pub sitio_aplicacion: Option<String>,
    pub observaciones: Option<String>,
    pub creado_en: String,
    pub paciente: String,
    pub identificacion: String,
    pub aplicador: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaVacuna {
    pub paciente_id: i64,
    #[serde(default)]
    pub inscripcion_id: Option<i64>,
    pub fecha: String,
    pub biologico: String,
    pub dosis_numero: i64,
    pub lote: String,
    #[serde(default)]
    pub vencimiento_lote: Option<String>,
    #[serde(default)]
    pub via: Option<String>,
    #[serde(default)]
    pub sitio_aplicacion: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    PacienteInexistente,
    ProgramaDesconocido,
    /// Ya está inscrito y activo en ese programa. Dos inscripciones activas
    /// dejarían los controles repartidos entre las dos.
    YaInscrito,
    InscripcionInexistente,
    /// La inscripción está egresada: no se le cuelgan controles nuevos.
    InscripcionEgresada,
    /// Control prenatal sin semanas de gestación. De ellas dependen el esquema
    /// de controles y la clasificación de riesgo — no se deducen de la fecha.
    PrenatalSinSemanas,
    /// Un control sobre la inscripción de vacunación: la vacuna se registra como
    /// dosis, no como control.
    VacunacionNoLlevaControles,
    RiesgoDesconocido,
    SemanasImposibles,
    DosisInvalida,
    /// Vacuna sin lote. Sin él, un retiro de mercado no puede encontrar a quién
    /// se le puso, y el registro deja de servir para lo único que sirve.
    VacunaSinLote,
    /// Egresar sin decir por qué deja sin saber si terminó, se trasladó o
    /// abandonó — y de eso vive el indicador de adherencia.
    EgresoSinMotivo,
    YaEgresada,
    /// La fecha del acto (control/vacuna) no es una fecha-hora reconocible. Se
    /// NORMALIZA a UTC al escribir (`zona::a_utc_rfc3339`), para que el fail-safe #1
    /// de retención (#758) compare vacuna/control —hoy tomados de un
    /// `<input datetime-local>` naive— contra la defunción en el MISMO marco horario.
    /// Sin fecha comparable no se registra el acto (D20: nunca un default silencioso).
    FechaInvalida,
}

pub const TODAS: [Falta; 14] = [
    Falta::PacienteInexistente,
    Falta::ProgramaDesconocido,
    Falta::YaInscrito,
    Falta::InscripcionInexistente,
    Falta::InscripcionEgresada,
    Falta::PrenatalSinSemanas,
    Falta::VacunacionNoLlevaControles,
    Falta::RiesgoDesconocido,
    Falta::SemanasImposibles,
    Falta::DosisInvalida,
    Falta::VacunaSinLote,
    Falta::EgresoSinMotivo,
    Falta::YaEgresada,
    Falta::FechaInvalida,
];

impl Falta {
    pub fn clave(self) -> &'static str {
        match self {
            Falta::PacienteInexistente => "paciente_inexistente",
            Falta::ProgramaDesconocido => "programa",
            Falta::YaInscrito => "ya_inscrito",
            Falta::InscripcionInexistente => "inscripcion_inexistente",
            Falta::InscripcionEgresada => "inscripcion_egresada",
            Falta::PrenatalSinSemanas => "prenatal_sin_semanas",
            Falta::VacunacionNoLlevaControles => "vacunacion_no_lleva_controles",
            Falta::RiesgoDesconocido => "riesgo",
            Falta::SemanasImposibles => "semanas",
            Falta::DosisInvalida => "dosis",
            Falta::VacunaSinLote => "sin_lote",
            Falta::EgresoSinMotivo => "egreso_sin_motivo",
            Falta::YaEgresada => "ya_egresada",
            Falta::FechaInvalida => "fecha",
        }
    }

    pub fn de_clave(c: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == c)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::PacienteInexistente => "ese paciente no existe en esta institución",
            Falta::ProgramaDesconocido => "ese no es uno de los cuatro programas de P&P",
            Falta::YaInscrito => {
                "ese paciente ya está inscrito y activo en ese programa. Para reinscribirlo, \
                 egrese primero la inscripción anterior"
            }
            Falta::InscripcionInexistente => "esa inscripción no existe en esta institución",
            Falta::InscripcionEgresada => {
                "esa inscripción está egresada: no se le pueden añadir controles"
            }
            Falta::PrenatalSinSemanas => {
                "un control prenatal tiene que registrar las semanas de gestación: de ellas \
                 dependen el esquema de controles y la clasificación de riesgo, y no se deducen \
                 de la fecha"
            }
            Falta::VacunacionNoLlevaControles => {
                "la vacunación se registra como DOSIS aplicadas, no como controles"
            }
            Falta::RiesgoDesconocido => "el riesgo debe ser BAJO o ALTO",
            Falta::SemanasImposibles => "las semanas de gestación van entre 1 y 45",
            Falta::DosisInvalida => "el número de dosis empieza en 1",
            Falta::VacunaSinLote => {
                "el lote del biológico es obligatorio: sin él, el día que se retire un lote del \
                 mercado no se puede saber a quién se le aplicó"
            }
            Falta::EgresoSinMotivo => "para egresar del programa hay que decir por qué",
            Falta::YaEgresada => "esa inscripción ya estaba egresada",
            Falta::FechaInvalida => "la fecha del acto no es una fecha-hora válida",
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

/// Comprueba un control contra el PROGRAMA al que pertenece.
///
/// Recibe el programa aparte porque la regla depende de él y la fila del control
/// no lo lleva: exigir semanas de gestación en un control de crecimiento sería
/// pedirle a un niño de dos años que declare su embarazo.
pub fn revisar_control(n: &NuevoControl, programa: &str) -> Result<(), Falta> {
    if programa == VACUNACION {
        return Err(Falta::VacunacionNoLlevaControles);
    }
    if let Some(r) = n.riesgo.as_deref()
        && !["BAJO", "ALTO"].contains(&r)
    {
        return Err(Falta::RiesgoDesconocido);
    }
    if let Some(s) = n.semanas_gestacion
        && !(1..=45).contains(&s)
    {
        return Err(Falta::SemanasImposibles);
    }
    if programa == PRENATAL && n.semanas_gestacion.is_none() {
        return Err(Falta::PrenatalSinSemanas);
    }
    Ok(())
}

pub fn revisar_vacuna(n: &NuevaVacuna) -> Result<(), Falta> {
    if n.dosis_numero < 1 {
        return Err(Falta::DosisInvalida);
    }
    if n.lote.trim().is_empty() {
        return Err(Falta::VacunaSinLote);
    }
    Ok(())
}

const SELECT_INSCRIPCION: &str = "\
    SELECT i.id, i.paciente_id, i.programa, i.finalidad_muv, i.fecha_inscripcion, i.estado, \
    i.fecha_egreso, i.motivo_egreso, i.creado_en, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente, p.numero_documento AS identificacion, \
    (SELECT COUNT(*) FROM control_pyp c WHERE c.inscripcion_id = i.id) AS controles, \
    (SELECT MAX(c.fecha) FROM control_pyp c WHERE c.inscripcion_id = i.id) AS ultimo_control \
    FROM inscripcion_pyp i JOIN paciente p ON p.id = i.paciente_id";

/// Las inscripciones de los programas que se pidan. **Lista vacía = ninguno**,
/// no «todos».
///
/// Recibe un CONJUNTO y no un `Option<&str>`, y el cambio no es cosmético: con
/// el `Option`, «no filtres» y «el programa que me pidieron no vale» se
/// escribían igual —`None`—, y la pantalla del panel las confundía. Un valor con
/// dos significados son dos campos (directiva 32); aquí la forma correcta es que
/// el llamante enumere lo que SÍ quiere ver, porque quien conoce la licencia es
/// él y no esta capa. Así el que se equivoque enseña de menos, nunca de más.
pub async fn listar_inscripciones(
    pool: &crate::Pool,
    ambito: Ambito,
    programas: &[&str],
) -> Result<Vec<Inscripcion>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    if programas.is_empty() {
        return Ok(Vec::new());
    }
    let mut sql = String::from(SELECT_INSCRIPCION);
    sql.push_str(" WHERE 1=1");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    // Los `?` los pone el número de programas, no su contenido: ningún valor del
    // cliente entra en la cadena.
    sql.push_str(" AND i.programa IN (");
    for k in 0..programas.len() {
        if k > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
    sql.push(')');
    sql.push_str(" ORDER BY i.fecha_inscripcion DESC, i.id DESC");
    let mut q = sqlx::query_as::<_, Inscripcion>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for p in programas {
        q = q.bind(*p);
    }
    q.fetch_all(pool).await
}

/// El programa de una inscripción, dentro del ámbito. `None` si no existe o es
/// ajena — las dos se responden igual.
///
/// Lo necesita el panel: sus cuatro programas comparten pantalla, así que antes
/// de enseñar los controles de una inscripción hay que saber de qué programa es
/// para preguntar si ESE está licenciado.
pub async fn programa_de_inscripcion(
    pool: &crate::Pool,
    ambito: Ambito,
    inscripcion_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    Ok(programa_de(pool, ambito, inscripcion_id).await?.map(|(p, _)| p))
}

/// El programa y el estado de una inscripción, dentro del ámbito.
async fn programa_de(
    pool: &crate::Pool,
    ambito: Ambito,
    inscripcion_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(None);
    };
    let mut sql = String::from(
        "SELECT i.programa, i.estado FROM inscripcion_pyp i \
         JOIN paciente p ON p.id = i.paciente_id WHERE i.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_as::<_, (String, String)>(&sql).bind(inscripcion_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// ¿Esta inscripción es del programa que dice la ruta, dentro del ámbito?
///
/// Existe porque la puerta de licencia mira el PREFIJO de la ruta: sin esta
/// comprobación, quien tenga licenciada la vacunación podría mandar un control
/// contra la inscripción prenatal de un paciente por una ruta que su licencia sí
/// abre. Devuelve `false` también cuando la inscripción no existe o es ajena —
/// las tres se responden igual, porque quien no puede verla no puede saber cuál
/// de las tres es.
pub async fn es_del_programa(
    pool: &crate::Pool,
    ambito: Ambito,
    inscripcion_id: i64,
    programa: &str,
) -> Result<bool, sqlx::Error> {
    Ok(programa_de(pool, ambito, inscripcion_id)
        .await?
        .is_some_and(|(p, _)| p == programa))
}

pub async fn inscribir(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaInscripcion,
    usuario_id: Option<i64>,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    let Some(finalidad) = finalidad_de(&n.programa) else {
        return Ok(Err(Falta::ProgramaDesconocido));
    };
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::PacienteInexistente));
    };
    let mut sql = String::from("SELECT 1 FROM paciente WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(n.paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::PacienteInexistente));
    }

    let activas: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inscripcion_pyp WHERE paciente_id = ? AND programa = ? \
         AND estado = 'ACTIVA'",
    )
    .bind(n.paciente_id)
    .bind(&n.programa)
    .fetch_one(pool)
    .await?;
    if activas > 0 {
        return Ok(Err(Falta::YaInscrito));
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO inscripcion_pyp (paciente_id, programa, finalidad_muv, fecha_inscripcion, \
         creado_por, creado_en) VALUES (?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.paciente_id)
    .bind(&n.programa)
    .bind(finalidad)
    .bind(&n.fecha_inscripcion)
    .bind(usuario_id)
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

pub async fn egresar(
    pool: &crate::Pool,
    ambito: Ambito,
    inscripcion_id: i64,
    motivo: &str,
) -> Result<Result<(), Falta>, sqlx::Error> {
    if motivo.trim().is_empty() {
        return Ok(Err(Falta::EgresoSinMotivo));
    }
    match programa_de(pool, ambito, inscripcion_id).await? {
        None => return Ok(Err(Falta::InscripcionInexistente)),
        Some((_, estado)) if estado == "EGRESADA" => return Ok(Err(Falta::YaEgresada)),
        Some(_) => {}
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query(
        "UPDATE inscripcion_pyp SET estado = 'EGRESADA', fecha_egreso = ?, motivo_egreso = ? \
         WHERE id = ?",
    )
    .bind(&ahora)
    .bind(motivo.trim())
    .bind(inscripcion_id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

const SELECT_CONTROL: &str = "\
    SELECT c.id, c.inscripcion_id, c.paciente_id, c.profesional_id, c.fecha, c.numero_control, \
    c.semanas_gestacion, c.riesgo, c.hallazgos, c.conducta, c.proximo_control, c.creado_en, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente, i.programa, \
    COALESCE(u.nombre,'') AS profesional \
    FROM control_pyp c \
    JOIN paciente p ON p.id = c.paciente_id \
    JOIN inscripcion_pyp i ON i.id = c.inscripcion_id \
    LEFT JOIN usuario u ON u.id = c.profesional_id";

pub async fn controles_de(
    pool: &crate::Pool,
    ambito: Ambito,
    inscripcion_id: i64,
) -> Result<Vec<Control>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_CONTROL);
    sql.push_str(" WHERE c.inscripcion_id = ?");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    sql.push_str(" ORDER BY c.numero_control, c.id");
    let mut q = sqlx::query_as::<_, Control>(&sql).bind(inscripcion_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Registra un control. El `numero_control` **se calcula**, no se recibe: quien
/// lo enviara podría repetir el 3 y el reporte contaría dos terceros controles.
pub async fn controlar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevoControl,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    let Some((programa, estado)) = programa_de(pool, ambito, n.inscripcion_id).await? else {
        return Ok(Err(Falta::InscripcionInexistente));
    };
    if estado == "EGRESADA" {
        return Ok(Err(Falta::InscripcionEgresada));
    }
    if let Err(f) = revisar_control(n, &programa) {
        return Ok(Err(f));
    }
    let paciente_id: i64 =
        sqlx::query_scalar("SELECT paciente_id FROM inscripcion_pyp WHERE id = ?")
            .bind(n.inscripcion_id)
            .fetch_one(pool)
            .await?;
    let siguiente: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(numero_control), 0) + 1 FROM control_pyp WHERE inscripcion_id = ?",
    )
    .bind(n.inscripcion_id)
    .fetch_one(pool)
    .await?;

    // La `fecha` viene de un `<input datetime-local>` (hora local naive). Se SELLA a
    // UTC como la defunción (`zona::a_utc_rfc3339`), o el fail-safe #1 de #758 compararía
    // este control contra la muerte en relojes distintos y un acto post-mortem hasta 5 h
    // pasaría por «anterior» → purga de un vivo (R1 del concilio 2026-09-26, fable).
    let Some(fecha_utc) = medico_dominio::zona::a_utc_rfc3339(&n.fecha) else {
        return Ok(Err(Falta::FechaInvalida));
    };
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO control_pyp (inscripcion_id, paciente_id, profesional_id, fecha, \
         numero_control, semanas_gestacion, riesgo, hallazgos, conducta, proximo_control, \
         creado_en) VALUES (?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.inscripcion_id)
    .bind(paciente_id)
    .bind(usuario_id)
    .bind(&fecha_utc)
    .bind(siguiente)
    .bind(n.semanas_gestacion)
    .bind(n.riesgo.as_deref())
    .bind(n.hallazgos.as_deref())
    .bind(n.conducta.as_deref())
    .bind(n.proximo_control.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

const SELECT_VACUNA: &str = "\
    SELECT v.id, v.paciente_id, v.inscripcion_id, v.aplicado_por, v.fecha, v.biologico, \
    v.dosis_numero, v.lote, v.vencimiento_lote, v.via, v.sitio_aplicacion, v.observaciones, \
    v.creado_en, (p.nombre1 || ' ' || p.apellido1) AS paciente, \
    p.numero_documento AS identificacion, COALESCE(u.nombre,'') AS aplicador \
    FROM vacuna_aplicada v JOIN paciente p ON p.id = v.paciente_id \
    LEFT JOIN usuario u ON u.id = v.aplicado_por";

/// Las vacunas del ámbito. Con `lote`, responde la pregunta del día del retiro:
/// **a quién le pusimos este lote**.
pub async fn listar_vacunas(
    pool: &crate::Pool,
    ambito: Ambito,
    lote: Option<&str>,
) -> Result<Vec<Vacuna>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_VACUNA);
    sql.push_str(" WHERE 1=1");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    if lote.is_some() {
        sql.push_str(" AND v.lote = ?");
    }
    sql.push_str(" ORDER BY v.fecha DESC, v.id DESC");
    let mut q = sqlx::query_as::<_, Vacuna>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(l) = lote {
        q = q.bind(l);
    }
    q.fetch_all(pool).await
}

pub async fn aplicar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaVacuna,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar_vacuna(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::PacienteInexistente));
    };
    let mut sql = String::from("SELECT 1 FROM paciente WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(n.paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::PacienteInexistente));
    }
    // La inscripción a la que se cuelga la dosis tiene que ser de ese paciente.
    // Se responde `InscripcionInexistente` —la variante que ya existía— porque
    // ajena, inexistente y de otro paciente son la misma respuesta a propósito:
    // no hay variante nueva que añadir, hay una comprobación que faltaba.
    if !opcional_es_del_paciente(pool, Enlace::InscripcionPyp, n.inscripcion_id, n.paciente_id)
        .await?
    {
        return Ok(Err(Falta::InscripcionInexistente));
    }

    // La `fecha` viene de un `<input datetime-local>` (hora local naive). Se SELLA a
    // UTC como la defunción (`zona::a_utc_rfc3339`): sin esto el fail-safe #1 de #758
    // compararía esta vacuna contra la muerte en relojes distintos y una dosis
    // post-mortem hasta 5 h pasaría por «anterior» → purga de un vivo. La población de
    // vacunación (sin `historia`) es justo la que ese fail-safe protege (R1 del concilio
    // 2026-09-26, fable).
    let Some(fecha_utc) = medico_dominio::zona::a_utc_rfc3339(&n.fecha) else {
        return Ok(Err(Falta::FechaInvalida));
    };
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO vacuna_aplicada (paciente_id, inscripcion_id, aplicado_por, fecha, \
         biologico, dosis_numero, lote, vencimiento_lote, via, sitio_aplicacion, observaciones, \
         creado_en) VALUES (?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.paciente_id)
    .bind(n.inscripcion_id)
    .bind(usuario_id)
    .bind(&fecha_utc)
    .bind(n.biologico.trim())
    .bind(n.dosis_numero)
    .bind(n.lote.trim())
    .bind(n.vencimiento_lote.as_deref())
    .bind(n.via.as_deref())
    .bind(n.sitio_aplicacion.as_deref())
    .bind(n.observaciones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Ata los cuatro programas al artefacto del validador.** Si alguien
    /// cambia una finalidad por un código que no está en `muv_finalidad.json`, se
    /// pone roja aquí en vez de que el MUV rechace el paquete del cliente.
    #[test]
    fn las_cuatro_finalidades_existen_en_el_catalogo_del_muv() {
        for (programa, finalidad, _) in PROGRAMAS {
            assert!(
                medico_dominio::muv::finalidad_existe(finalidad),
                "la finalidad «{finalidad}» de {programa} no está en la tabla del MUV"
            );
        }
        // Positivo de control: un código inventado NO está, o sea que la
        // comprobación de arriba discrimina y no dice que sí a todo.
        assert!(!medico_dominio::muv::finalidad_existe("99"));
    }

    #[test]
    fn cada_programa_tiene_su_finalidad_y_su_nombre() {
        assert_eq!(finalidad_de("PRENATAL"), Some("23"));
        assert_eq!(finalidad_de("VACUNACION"), Some("14"));
        assert_eq!(finalidad_de("INVENTADO"), None);
        assert!(nombre_de("CRECIMIENTO_DESARROLLO").is_some());
    }

    /// Compartidas con `contra_la_base`.
    pub(super) fn control_publico() -> NuevoControl { control() }
    pub(super) fn vacuna_publica() -> NuevaVacuna { vacuna() }

    fn control() -> NuevoControl {
        NuevoControl {
            inscripcion_id: 1,
            fecha: "2026-08-16T09:00:00Z".into(),
            semanas_gestacion: Some(12),
            riesgo: Some("BAJO".into()),
            hallazgos: None,
            conducta: None,
            proximo_control: Some("2026-09-16".into()),
        }
    }

    /// El prenatal EXIGE semanas; el de crecimiento NO las exige. Las dos mitades
    /// juntas: con una sola, la regla podría estar exigiéndoselas a todos —y
    /// entonces un niño de dos años tendría que declarar su embarazo— o a nadie.
    #[test]
    fn las_semanas_son_del_prenatal_y_solo_del_prenatal() {
        let mut n = control();
        n.semanas_gestacion = None;
        assert_eq!(revisar_control(&n, "PRENATAL"), Err(Falta::PrenatalSinSemanas));
        assert_eq!(revisar_control(&n, "CRECIMIENTO_DESARROLLO"), Ok(()));
        assert_eq!(revisar_control(&n, "SALUD_REPRODUCTIVA"), Ok(()));
        // Y con semanas, el prenatal pasa.
        assert_eq!(revisar_control(&control(), "PRENATAL"), Ok(()));
    }

    #[test]
    fn la_vacunacion_no_lleva_controles() {
        assert_eq!(
            revisar_control(&control(), VACUNACION),
            Err(Falta::VacunacionNoLlevaControles)
        );
    }

    #[test]
    fn el_riesgo_y_las_semanas_fuera_de_rango_se_rechazan() {
        let mut n = control();
        n.riesgo = Some("REGULAR".into());
        assert_eq!(revisar_control(&n, "PRENATAL"), Err(Falta::RiesgoDesconocido));
        let mut n = control();
        n.semanas_gestacion = Some(50);
        assert_eq!(revisar_control(&n, "PRENATAL"), Err(Falta::SemanasImposibles));
        // Sin riesgo declarado pasa: no siempre se clasifica en el primer control.
        let mut n = control();
        n.riesgo = None;
        assert_eq!(revisar_control(&n, "PRENATAL"), Ok(()));
    }

    fn vacuna() -> NuevaVacuna {
        NuevaVacuna {
            paciente_id: 10,
            inscripcion_id: None,
            fecha: "2026-08-16T09:00:00Z".into(),
            biologico: "Pentavalente".into(),
            dosis_numero: 1,
            lote: "L-2026-08".into(),
            vencimiento_lote: Some("2027-01-31".into()),
            via: Some("IM".into()),
            sitio_aplicacion: Some("muslo izquierdo".into()),
            observaciones: None,
        }
    }

    /// La regla que hace útil el registro entero.
    #[test]
    fn la_vacuna_sin_lote_se_rechaza() {
        let mut n = vacuna();
        n.lote = "   ".into();
        assert_eq!(revisar_vacuna(&n), Err(Falta::VacunaSinLote));
        assert_eq!(revisar_vacuna(&vacuna()), Ok(()));
    }

    #[test]
    fn la_dosis_cero_se_rechaza() {
        let mut n = vacuna();
        n.dosis_numero = 0;
        assert_eq!(revisar_vacuna(&n), Err(Falta::DosisInvalida));
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
    /// Los cuatro programas. Se DERIVA del catálogo en vez de escribirse: la
    /// firma de `listar_inscripciones` ya no tiene ningún valor que signifique
    /// «todos», y una lista copiada a mano envejecería en silencio el día que
    /// entre un quinto programa.
    fn todos() -> Vec<&'static str> {
        PROGRAMAS.iter().map(|(p, _, _)| *p).collect()
    }


    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401'),(2,'Dos','900999','050010999901')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (6,'Enf. Sol','e@x.co','h','enfermeria',1,1)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1','Ana','Ruiz','1996-01-01','F',1), \
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    fn inscripcion(paciente: i64, programa: &str) -> NuevaInscripcion {
        NuevaInscripcion {
            paciente_id: paciente,
            programa: programa.into(),
            fecha_inscripcion: "2026-08-16".into(),
        }
    }

    /// **El conjunto vacío NO significa «todos», y pedir uno no trae los otros.**
    ///
    /// Caso rojo del filtro que fallaba ABIERTO y que encontró la revisión de
    /// seguridad de esta rama. La firma era `Option<&str>`, donde `None` quería
    /// decir «sin filtro de programa» — o sea los cuatro—; y la pantalla del
    /// panel convertía en `None` justo el programa que NO estaba licenciado. El
    /// resultado: pedir un programa que no compraste enseñaba TODOS, y entrar
    /// sin filtro también. Con un conjunto, «nada que ver» y «todo» dejan de
    /// escribirse igual (directiva 32).
    #[tokio::test]
    async fn el_conjunto_vacio_no_significa_todos() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().unwrap();
        inscribir(&pool, a, &inscripcion(10, "CRECIMIENTO_DESARROLLO"), Some(6))
            .await.unwrap().unwrap();

        // Lo que el defecto convertía en «todos».
        assert!(
            listar_inscripciones(&pool, a, &[]).await.unwrap().is_empty(),
            "el conjunto vacío tiene que enseñar NADA: si enseña algo, el que se \
             equivoque enseñará de más"
        );

        // Uno pedido, uno devuelto — y el otro no se cuela.
        let solo = listar_inscripciones(&pool, a, &["PRENATAL"]).await.unwrap();
        assert_eq!(solo.len(), 1);
        assert_eq!(solo[0].programa, "PRENATAL");

        // POSITIVO DE CONTROL: pidiendo los dos salen los dos, así que el vacío
        // de arriba no es que la consulta esté rota.
        assert_eq!(
            listar_inscripciones(&pool, a, &["PRENATAL", "CRECIMIENTO_DESARROLLO"])
                .await.unwrap().len(),
            2
        );
        assert_eq!(listar_inscripciones(&pool, a, &todos()).await.unwrap().len(), 2);
    }

    /// **La inscripción a la que se cuelga la dosis tiene que ser de ese
    /// paciente** — la cuarta de las claves foráneas que se ataban en crudo.
    ///
    /// Responde `InscripcionInexistente`, que ya existía: ajena, inexistente y
    /// de otro paciente son la MISMA respuesta a propósito. No hacía falta una
    /// variante nueva, hacía falta la comprobación.
    #[tokio::test]
    async fn la_inscripcion_que_no_es_del_paciente_no_lleva_la_dosis() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        // Una inscripción del paciente 20, que es de la OTRA institución.
        let ajena = inscribir(&pool, Ambito::Institucion(2), &inscripcion(20, "VACUNACION"), None)
            .await.unwrap().unwrap();
        let propia = inscribir(&pool, a, &inscripcion(10, "VACUNACION"), Some(6))
            .await.unwrap().unwrap();

        let con = |i: Option<i64>| {
            let mut v = super::pruebas::vacuna_publica();
            v.inscripcion_id = i;
            v
        };
        assert_eq!(
            aplicar(&pool, a, &con(Some(ajena)), 6).await.unwrap(),
            Err(Falta::InscripcionInexistente)
        );
        assert_eq!(
            aplicar(&pool, a, &con(Some(9999)), 6).await.unwrap(),
            Err(Falta::InscripcionInexistente),
            "el id inexistente tiene que responder igual que el ajeno"
        );
        // POSITIVO DE CONTROL: la inscripción del propio paciente sí lleva la
        // dosis, y sin inscripción también (colgarla es opcional).
        assert!(aplicar(&pool, a, &con(Some(propia)), 6).await.unwrap().is_ok());
        assert!(aplicar(&pool, a, &con(None), 6).await.unwrap().is_ok());
    }

    /// **R1 (concilio 2026-09-26, fable→opus, por ejecución): una vacuna posterior a la
    /// muerte BLOQUEA la purga, por la vía real (`aplicar`).** El paciente de vacunación
    /// no tiene `historia`, así que la vacuna es su ÚNICO acto de vida — la población que
    /// el fail-safe #1 de #758 protege. La `fecha` llega de un `<input datetime-local>`
    /// (hora local naive); tras sellar la defunción a UTC (0078/H2), una vacuna naive
    /// leída como UTC caía 5 h "antes" de la muerte y NO bloqueaba → purga de un VIVO
    /// (fail-open). Ahora `aplicar` sella la vacuna a UTC igual que la muerte.
    /// **MUTANTE que discrimina (D33):** bind `&n.fecha` crudo en `aplicar` en vez de
    /// `&fecha_utc` → la vacuna vuelve a caer "antes" → el paciente 10 reaparece elegible
    /// (`assert!` de abajo falla). Verificado por ejecución.
    #[tokio::test]
    async fn una_vacuna_naive_posterior_a_la_muerte_bloquea_la_purga() {
        let pool = base().await;
        // Muerte de hace 21 años (vencida a 20), tecleada como la manda la pantalla:
        // hora LOCAL naive, sin zona.
        let muerte_utc = chrono::Utc::now() - chrono::Duration::days(21 * 365);
        let muerte_local = muerte_utc
            .with_timezone(&medico_dominio::zona::POR_DEFECTO)
            .naive_local();
        let n = crate::defuncion::Nueva {
            paciente_id: 10,
            hospitalizacion_id: None,
            atencion_urgencias_id: None,
            fecha_defuncion: muerte_local.format("%Y-%m-%dT%H:%M:%S").to_string(),
            sitio: "INSTITUCION".into(),
            manera: "NATURAL".into(),
            causa_directa: None,
            antecedente_a: None,
            antecedente_b: None,
            antecedente_c: None,
            causa_basica: "J189".into(),
            numero_certificado: None,
            observaciones: None,
        };
        crate::defuncion::certificar(&pool, Ambito::Institucion(1), &n, 6)
            .await.unwrap().unwrap();

        // VERDE (control): sin ningún acto posterior, el fallecido de hace 21 años es elegible.
        let antes = crate::retencion::elegibles(
            &pool, Ambito::Superadmin, chrono::Utc::now(), crate::retencion::ANOS_POR_DEFECTO,
        ).await.unwrap();
        assert_eq!(antes.len(), 1, "sin actos posteriores, el fallecido de hace 21 años es elegible");

        // Una vacuna 2 h DESPUÉS de la muerte (hora local naive, como la manda la pantalla):
        // el paciente estaba VIVO → la muerte se tecleó mal → NO debe poder purgarse.
        let mut v = super::pruebas::vacuna_publica();
        v.paciente_id = 10;
        v.inscripcion_id = None;
        v.fecha = (muerte_local + chrono::Duration::hours(2))
            .format("%Y-%m-%dT%H:%M:%S").to_string();
        aplicar(&pool, Ambito::Institucion(1), &v, 6).await.unwrap().unwrap();

        let despues = crate::retencion::elegibles(
            &pool, Ambito::Superadmin, chrono::Utc::now(), crate::retencion::ANOS_POR_DEFECTO,
        ).await.unwrap();
        assert!(
            despues.is_empty(),
            "una vacuna POSTERIOR a la muerte (fecha mal tecleada) bloquea la purga de un vivo"
        );
    }

    /// Una fecha de acto que no parsea se rechaza (`FechaInvalida`), no se guarda cruda
    /// (D20): sin fecha comparable el fail-safe #1 quedaría ciego a ese acto.
    #[tokio::test]
    async fn una_fecha_de_vacuna_ilegible_se_rechaza() {
        let pool = base().await;
        let mut v = super::pruebas::vacuna_publica();
        v.paciente_id = 10;
        v.inscripcion_id = None;
        v.fecha = "ayer por la tarde".into();
        assert_eq!(
            aplicar(&pool, Ambito::Institucion(1), &v, 6).await.unwrap(),
            Err(Falta::FechaInvalida)
        );
    }

    /// La migración 0079 pasa a UTC las fechas NAIVE de `vacuna_aplicada` y `control_pyp`
    /// y deja intactas las que ya traen zona (D22). Corre el SQL REAL del archivo.
    #[tokio::test]
    async fn la_migracion_0079_normaliza_solo_las_fechas_naive_de_los_actos() {
        let pool = base().await;
        // Una vacuna naive y una ya en UTC, sembradas a mano (saltándose el sellado de `aplicar`).
        sqlx::query(
            "INSERT INTO vacuna_aplicada (id,paciente_id,aplicado_por,fecha,biologico,dosis_numero,lote,creado_en) \
             VALUES (1,10,6,'2005-06-15T10:00','BCG',1,'L-1','2005-06-15T10:00:00Z'), \
                    (2,10,6,'2005-06-15T10:00:00Z','BCG',1,'L-1','2005-06-15T10:00:00Z')",
        ).execute(&pool).await.unwrap();
        sqlx::query(include_str!("../migrations/0079_actos_pyp_a_utc.sql"))
            .execute(&pool).await.unwrap();
        let naive: String = sqlx::query_scalar("SELECT fecha FROM vacuna_aplicada WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        let zonif: String = sqlx::query_scalar("SELECT fecha FROM vacuna_aplicada WHERE id = 2")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(naive, "2005-06-15T15:00:00Z", "la naive -05 se corrió +5 h a UTC");
        assert_eq!(zonif, "2005-06-15T10:00:00Z", "la que ya estaba en UTC no se tocó");
    }

    /// `programa_de_inscripcion` responde IGUAL para la ajena y la inexistente:
    /// es lo que deja al panel filtrar por licencia sin abrir un oráculo.
    #[tokio::test]
    async fn el_programa_de_una_inscripcion_ajena_no_se_sabe() {
        let pool = base().await;
        let id = inscribir(&pool, Ambito::Institucion(1), &inscripcion(10, "PRENATAL"), Some(6))
            .await.unwrap().unwrap();
        assert_eq!(
            programa_de_inscripcion(&pool, Ambito::Institucion(2), id).await.unwrap(),
            None
        );
        assert_eq!(
            programa_de_inscripcion(&pool, Ambito::Institucion(1), 9999).await.unwrap(),
            None
        );
        // Positivo de control: desde su propia institución sí se sabe.
        assert_eq!(
            programa_de_inscripcion(&pool, Ambito::Institucion(1), id).await.unwrap().as_deref(),
            Some("PRENATAL")
        );
    }

    #[tokio::test]
    async fn se_inscribe_con_la_finalidad_del_muv() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().unwrap();
        let v = listar_inscripciones(&pool, a, &todos()).await.unwrap();
        assert_eq!(v.len(), 1);
        // La finalidad se guarda EN LA FILA, no se deduce al leer.
        assert_eq!(v[0].finalidad_muv, "23");
        assert_eq!(v[0].controles, 0, "inscrito y sin venir: es lo que mide la adherencia");
    }

    /// Dos inscripciones activas repartirían los controles entre las dos. Y
    /// reinscribir DESPUÉS de egresar sí se puede: un segundo embarazo es normal.
    #[tokio::test]
    async fn una_activa_por_programa_pero_se_puede_reinscribir() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().unwrap();
        assert_eq!(
            inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap(),
            Err(Falta::YaInscrito)
        );
        egresar(&pool, a, id, "parto atendido").await.unwrap().unwrap();
        assert!(inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().is_ok());
        // Y en OTRO programa se puede estar a la vez: son programas distintos.
        assert!(inscribir(&pool, a, &inscripcion(10, "SALUD_REPRODUCTIVA"), Some(6)).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn no_se_inscribe_al_paciente_de_otra_institucion() {
        let pool = base().await;
        assert_eq!(
            inscribir(&pool, Ambito::Institucion(1), &inscripcion(20, "PRENATAL"), Some(6)).await.unwrap(),
            Err(Falta::PacienteInexistente)
        );
        assert!(inscribir(&pool, Ambito::Institucion(1), &inscripcion(10, "PRENATAL"), Some(6))
            .await.unwrap().is_ok());
    }

    /// El número de control lo pone el SERVIDOR. Si lo enviara el cliente, dos
    /// terceros controles contarían como tres y el reporte mentiría.
    #[tokio::test]
    async fn el_numero_de_control_lo_cuenta_el_servidor() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().unwrap();
        for _ in 0..3 {
            let mut n = super::pruebas::control_publico();
            n.inscripcion_id = id;
            controlar(&pool, a, &n, 6).await.unwrap().unwrap();
        }
        let cs = controles_de(&pool, a, id).await.unwrap();
        assert_eq!(cs.iter().map(|c| c.numero_control).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(cs[0].programa, "PRENATAL");
    }

    /// A una inscripción egresada no se le cuelgan controles: quedarían fuera de
    /// cualquier periodo y del indicador.
    #[tokio::test]
    async fn la_inscripcion_egresada_no_admite_controles() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().unwrap();
        egresar(&pool, a, id, "traslado a otra IPS").await.unwrap().unwrap();
        let mut n = super::pruebas::control_publico();
        n.inscripcion_id = id;
        assert_eq!(controlar(&pool, a, &n, 6).await.unwrap(), Err(Falta::InscripcionEgresada));
        assert_eq!(egresar(&pool, a, id, "otra vez").await.unwrap(), Err(Falta::YaEgresada));
        assert_eq!(egresar(&pool, a, id, "  ").await.unwrap(), Err(Falta::EgresoSinMotivo));
    }

    /// La pregunta del día del retiro: a quién le pusimos ESTE lote.
    #[tokio::test]
    async fn el_lote_encuentra_a_quien_se_le_puso() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let mut malo = super::pruebas::vacuna_publica();
        malo.lote = "L-MALO".into();
        aplicar(&pool, a, &malo, 6).await.unwrap().unwrap();
        aplicar(&pool, a, &super::pruebas::vacuna_publica(), 6).await.unwrap().unwrap();

        let del_lote = listar_vacunas(&pool, a, Some("L-MALO")).await.unwrap();
        assert_eq!(del_lote.len(), 1, "solo los de ese lote");
        assert_eq!(del_lote[0].paciente, "Ana Ruiz");
        assert_eq!(listar_vacunas(&pool, a, None).await.unwrap().len(), 2, "sin filtro, las dos");
    }

    /// Una inscripción de un programa NO responde como si fuera de otro. Es la
    /// comprobación que cierra el hueco que abrió la primera versión de las
    /// rutas: la puerta de licencia mira el prefijo, así que sin esto una
    /// licencia de vacunación abría el control de un prenatal.
    #[tokio::test]
    async fn una_inscripcion_no_pasa_por_la_de_otro_programa() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = inscribir(&pool, a, &inscripcion(10, "PRENATAL"), Some(6)).await.unwrap().unwrap();
        assert!(es_del_programa(&pool, a, id, "PRENATAL").await.unwrap());
        assert!(!es_del_programa(&pool, a, id, "VACUNACION").await.unwrap());
        assert!(!es_del_programa(&pool, a, id, "CRECIMIENTO_DESARROLLO").await.unwrap());
        // Y una inscripción ajena tampoco: la institución 2 no la ve.
        assert!(!es_del_programa(&pool, Ambito::Institucion(2), id, "PRENATAL").await.unwrap());
        // Ni una que no existe.
        assert!(!es_del_programa(&pool, a, 9999, "PRENATAL").await.unwrap());
    }

    #[tokio::test]
    async fn el_ambito_aisla_vacunas_e_inscripciones() {
        let pool = base().await;
        inscribir(&pool, Ambito::Institucion(1), &inscripcion(10, "PRENATAL"), Some(6))
            .await.unwrap().unwrap();
        aplicar(&pool, Ambito::Institucion(1), &super::pruebas::vacuna_publica(), 6)
            .await.unwrap().unwrap();
        assert!(listar_inscripciones(&pool, Ambito::Institucion(2), &todos()).await.unwrap().is_empty());
        assert!(listar_vacunas(&pool, Ambito::Institucion(2), None).await.unwrap().is_empty());
        assert!(listar_inscripciones(&pool, Ambito::Ninguno, &todos()).await.unwrap().is_empty());
        assert_eq!(listar_inscripciones(&pool, Ambito::Institucion(1), &todos()).await.unwrap().len(), 1);
    }

    /// El `CHECK` por la puerta de atrás: ni vacuna sin lote ni egreso mudo.
    #[tokio::test]
    async fn los_checks_cierran_la_puerta_de_atras() {
        let pool = base().await;
        let sql_v = "INSERT INTO vacuna_aplicada (paciente_id,aplicado_por,fecha,biologico,\
                     dosis_numero,lote,creado_en) VALUES (10,6,'2026-08-16','BCG',1,?,'x')";
        sqlx::query(sql_v).bind("L-1").execute(&pool).await.expect("con lote entra");
        for (lote, que) in [("", "lote vacío"), ("   ", "lote en blanco")] {
            assert!(sqlx::query(sql_v).bind(lote).execute(&pool).await.is_err(), "{que}");
        }
        let sql_i = "INSERT INTO inscripcion_pyp (paciente_id,programa,finalidad_muv,\
                     fecha_inscripcion,estado,fecha_egreso,motivo_egreso,creado_en) \
                     VALUES (10,'PRENATAL','23','2026-08-16','EGRESADA',?,?,'x')";
        sqlx::query(sql_i).bind(Some("2026-08-20")).bind(Some("parto"))
            .execute(&pool).await.expect("el egreso completo entra");
        let sin: Option<&str> = None;
        assert!(sqlx::query(sql_i).bind(Some("2026-08-20")).bind(sin).execute(&pool).await.is_err(),
                "egreso sin motivo");
        assert!(sqlx::query(sql_i).bind(sin).bind(Some("parto")).execute(&pool).await.is_err(),
                "egreso sin fecha");
    }
}
