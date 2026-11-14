//! Habilitación (#249): equipos biomédicos, comités y vigilancia de infecciones.
//!
//! Los tres se construyen juntos porque comparten lo que los hace útiles: **no
//! son listas, son la respuesta a una pregunta que hace un inspector**. Un
//! inventario de equipos que no diga cuáles tienen el mantenimiento vencido, o
//! una lista de comités que no diga cuáles están en mora, es papel.
//!
//! # `item_inventario` no servía, y decirlo importa
//!
//! La 0025 tiene un tipo `DISPOSITIVO` y la tentación era meter ahí los equipos.
//! No sirve: esa tabla modela consumibles —cantidad, lote, vencimiento, mínimo—
//! y un ecógrafo no es «cantidad_actual = 1». Lo que un equipo necesita es serie,
//! hoja de vida, periodicidad y próximo mantenimiento.
//!
//! Es el juicio de `terceros` con el signo contrario: allí buscar por parecido
//! habría dado por ausente lo que ya estaba, y aquí habría dado por presente lo
//! que no está. Lo que decide es qué preguntas tiene que responder la tabla.
//!
//! # Lo VENCIDO no se guarda: se compara al leer
//!
//! Un booleano `vencido` en la fila estaría mintiendo desde el día siguiente a
//! escribirlo. La fecha del próximo mantenimiento sí se guarda —es una decisión,
//! no un cálculo—; que esté vencida se responde comparando con hoy.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

use crate::pertenencia::{opcional_es_del_paciente, Enlace};

pub const ESTADOS_EQUIPO: [&str; 4] = [
    "ACTIVO",
    "EN_MANTENIMIENTO",
    "FUERA_DE_SERVICIO",
    "DADO_DE_BAJA",
];
pub const TIPOS_MANTENIMIENTO: [&str; 3] = ["PREVENTIVO", "CORRECTIVO", "CALIBRACION"];

pub const TIPOS_COMITE: [(&str, &str); 6] = [
    ("HISTORIAS_CLINICAS", "Comité de historias clínicas"),
    ("INFECCIONES", "Comité de infecciones"),
    ("FARMACIA_Y_TERAPEUTICA", "Comité de farmacia y terapéutica"),
    ("ETICA_HOSPITALARIA", "Comité de ética hospitalaria"),
    ("SEGURIDAD_DEL_PACIENTE", "Comité de seguridad del paciente"),
    ("OTRO", "Otro comité"),
];

pub const TIPOS_IAAS: [(&str, &str); 5] = [
    ("NEUMONIA_ASOCIADA_A_VM", "Neumonía asociada a ventilación mecánica"),
    ("INFECCION_SITIO_OPERATORIO", "Infección de sitio operatorio"),
    ("ITU_ASOCIADA_A_CATETER", "Infección urinaria asociada a catéter"),
    ("BACTERIEMIA_ASOCIADA_A_CVC", "Bacteriemia asociada a catéter central"),
    ("OTRA", "Otra"),
];

/// Los tres tipos que EXIGEN decir a qué dispositivo se asocian.
///
/// Una neumonía «asociada a ventilación mecánica» que no diga a qué ventilador
/// ni cuántos días llevaba no es un evento de vigilancia: es una nota.
pub const IAAS_CON_DISPOSITIVO: [&str; 3] = [
    "NEUMONIA_ASOCIADA_A_VM",
    "ITU_ASOCIADA_A_CATETER",
    "BACTERIEMIA_ASOCIADA_A_CVC",
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Equipo {
    pub id: i64,
    pub nombre: String,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub serie: Option<String>,
    pub ubicacion: Option<String>,
    pub registro_invima: Option<String>,
    pub fecha_adquisicion: Option<String>,
    pub estado: String,
    pub periodicidad_meses: Option<i64>,
    pub proximo_mantenimiento: Option<String>,
    pub motivo_baja: Option<String>,
    pub creado_en: String,
    pub mantenimientos: i64,
    pub ultimo_mantenimiento: Option<String>,
}

impl Equipo {
    /// ¿Le pasó la fecha del próximo mantenimiento?
    ///
    /// Se responde COMPARANDO, no leyendo una columna. Y devuelve tres cosas y no
    /// dos: `None` es «no se le ha puesto cronograma», que en una visita de
    /// habilitación es un hallazgo distinto de «vencido» — y peor.
    pub fn vencido(&self, hoy: &str) -> Option<bool> {
        if self.estado == "DADO_DE_BAJA" {
            return None;
        }
        self.proximo_mantenimiento
            .as_deref()
            .map(|p| p.as_bytes() < hoy.as_bytes())
    }

    /// Sin periodicidad declarada no hay cronograma, y eso se reporta.
    pub fn sin_cronograma(&self) -> bool {
        self.estado != "DADO_DE_BAJA" && self.periodicidad_meses.is_none()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevoEquipo {
    pub nombre: String,
    #[serde(default)]
    pub marca: Option<String>,
    #[serde(default)]
    pub modelo: Option<String>,
    #[serde(default)]
    pub serie: Option<String>,
    #[serde(default)]
    pub ubicacion: Option<String>,
    #[serde(default)]
    pub registro_invima: Option<String>,
    #[serde(default)]
    pub fecha_adquisicion: Option<String>,
    #[serde(default)]
    pub periodicidad_meses: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevoMantenimiento {
    pub equipo_id: i64,
    pub tipo: String,
    pub fecha: String,
    pub responsable: String,
    #[serde(default)]
    pub hallazgos: Option<String>,
    pub conforme: bool,
    #[serde(default)]
    pub costo: Option<f64>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Comite {
    pub id: i64,
    pub tipo: String,
    pub nombre: String,
    pub periodicidad_meses: Option<i64>,
    pub activo: bool,
    pub creado_en: String,
    pub sesiones: i64,
    pub ultima_sesion: Option<String>,
}

impl Comite {
    /// ¿Lleva más tiempo del que debería sin sesionar?
    ///
    /// `None` cuando no se puede saber: sin periodicidad declarada no hay mora
    /// que medir, y decir «al día» sería afirmar lo que nadie comprobó.
    pub fn en_mora(&self, hoy: &str) -> Option<bool> {
        if !self.activo {
            return None;
        }
        let meses = self.periodicidad_meses?;
        let Some(ultima) = self.ultima_sesion.as_deref() else {
            // Activo, con periodicidad y sin una sola sesión: está en mora por
            // definición, y es el caso que más se ve en una visita.
            return Some(true);
        };
        let Ok(d) = chrono::NaiveDate::parse_from_str(&ultima[..10.min(ultima.len())], "%Y-%m-%d")
        else {
            return None;
        };
        let Ok(h) = chrono::NaiveDate::parse_from_str(hoy, "%Y-%m-%d") else {
            return None;
        };
        // Meses aproximados a 30 días: la mora se mide en semanas de diferencia,
        // no en días exactos, y un calendario completo aquí no cambia ninguna
        // decisión. Va escrito para que nadie lo tome por exacto.
        Some((h - d).num_days() > meses * 30)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevoComite {
    pub tipo: String,
    pub nombre: String,
    #[serde(default)]
    pub periodicidad_meses: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaSesion {
    pub comite_id: i64,
    pub fecha: String,
    pub asistentes: String,
    #[serde(default)]
    pub temas: Option<String>,
    #[serde(default)]
    pub decisiones: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EventoIaas {
    pub id: i64,
    pub paciente_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub tipo: String,
    pub fecha_deteccion: String,
    pub dispositivo_asociado: Option<String>,
    pub dias_dispositivo: Option<i64>,
    pub microorganismo: Option<String>,
    pub notificado: bool,
    pub fecha_notificacion: Option<String>,
    pub desenlace: Option<String>,
    pub creado_en: String,
    pub paciente: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevoEventoIaas {
    pub paciente_id: i64,
    #[serde(default)]
    pub hospitalizacion_id: Option<i64>,
    pub tipo: String,
    pub fecha_deteccion: String,
    #[serde(default)]
    pub dispositivo_asociado: Option<String>,
    #[serde(default)]
    pub dias_dispositivo: Option<i64>,
    #[serde(default)]
    pub microorganismo: Option<String>,
    #[serde(default)]
    pub desenlace: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    NombreVacio,
    EstadoDesconocido,
    TipoDesconocido,
    EquipoInexistente,
    ComiteInexistente,
    PacienteInexistente,
    /// Mantenimiento no conforme y sin decir qué se encontró: una firma sin
    /// contenido.
    NoConformeSinHallazgos,
    /// Sesión de comité sin asistentes. Un acta sin quién estuvo no es un acta.
    SesionSinAsistentes,
    /// Infección «asociada a dispositivo» sin decir a cuál.
    IaasSinDispositivo,
    PeriodicidadInvalida,
    /// Dar de baja sin motivo deja la hoja de vida sin explicar por qué el
    /// aparato ya no está.
    BajaSinMotivo,
    /// La atención que se enlaza **no es de este paciente** — o no existe, o es
    /// de otra institución. Las tres se responden igual: distinguirlas diría que
    /// existe.
    AtencionQueNoEsDelPaciente,
}

pub const TODAS: [Falta; 12] = [
    Falta::NombreVacio,
    Falta::EstadoDesconocido,
    Falta::TipoDesconocido,
    Falta::EquipoInexistente,
    Falta::ComiteInexistente,
    Falta::PacienteInexistente,
    Falta::NoConformeSinHallazgos,
    Falta::SesionSinAsistentes,
    Falta::IaasSinDispositivo,
    Falta::PeriodicidadInvalida,
    Falta::BajaSinMotivo,
    Falta::AtencionQueNoEsDelPaciente,
];

impl Falta {
    pub fn clave(self) -> &'static str {
        match self {
            Falta::NombreVacio => "nombre",
            Falta::EstadoDesconocido => "estado",
            Falta::TipoDesconocido => "tipo",
            Falta::EquipoInexistente => "equipo_inexistente",
            Falta::ComiteInexistente => "comite_inexistente",
            Falta::PacienteInexistente => "paciente_inexistente",
            Falta::NoConformeSinHallazgos => "no_conforme_sin_hallazgos",
            Falta::SesionSinAsistentes => "sesion_sin_asistentes",
            Falta::IaasSinDispositivo => "iaas_sin_dispositivo",
            Falta::PeriodicidadInvalida => "periodicidad",
            Falta::BajaSinMotivo => "baja_sin_motivo",
            Falta::AtencionQueNoEsDelPaciente => "atencion_que_no_es_del_paciente",
        }
    }

    pub fn de_clave(c: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == c)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::NombreVacio => "el nombre es obligatorio",
            Falta::EstadoDesconocido => "ese no es un estado del producto",
            Falta::TipoDesconocido => "ese no es un tipo del producto",
            Falta::EquipoInexistente => "ese equipo no existe en esta institución",
            Falta::ComiteInexistente => "ese comité no existe en esta institución",
            Falta::PacienteInexistente => "ese paciente no existe en esta institución",
            Falta::NoConformeSinHallazgos => {
                "un mantenimiento que no deja el equipo conforme tiene que decir qué encontró"
            }
            Falta::SesionSinAsistentes => "un acta sin asistentes no es un acta",
            Falta::IaasSinDispositivo => {
                "una infección asociada a dispositivo tiene que decir a cuál: sin eso no es un \
                 evento de vigilancia, es una nota"
            }
            Falta::PeriodicidadInvalida => "la periodicidad va en meses y tiene que ser mayor que cero",
            Falta::BajaSinMotivo => "dar de baja un equipo exige decir por qué",
            Falta::AtencionQueNoEsDelPaciente => {
                "la hospitalización que se enlaza no es de ese paciente"
            }
        }
    }
}

fn vacio(s: &str) -> bool {
    s.trim().is_empty()
}

pub fn revisar_equipo(n: &NuevoEquipo) -> Result<(), Falta> {
    if vacio(&n.nombre) {
        return Err(Falta::NombreVacio);
    }
    if let Some(p) = n.periodicidad_meses
        && p <= 0
    {
        return Err(Falta::PeriodicidadInvalida);
    }
    Ok(())
}

pub fn revisar_mantenimiento(n: &NuevoMantenimiento) -> Result<(), Falta> {
    if !TIPOS_MANTENIMIENTO.contains(&n.tipo.as_str()) {
        return Err(Falta::TipoDesconocido);
    }
    if vacio(&n.responsable) {
        return Err(Falta::NombreVacio);
    }
    if !n.conforme && !n.hallazgos.as_deref().is_some_and(|h| !vacio(h)) {
        return Err(Falta::NoConformeSinHallazgos);
    }
    Ok(())
}

pub fn revisar_sesion(n: &NuevaSesion) -> Result<(), Falta> {
    if vacio(&n.asistentes) {
        return Err(Falta::SesionSinAsistentes);
    }
    Ok(())
}

pub fn revisar_iaas(n: &NuevoEventoIaas) -> Result<(), Falta> {
    if !TIPOS_IAAS.iter().any(|(t, _)| *t == n.tipo) {
        return Err(Falta::TipoDesconocido);
    }
    if IAAS_CON_DISPOSITIVO.contains(&n.tipo.as_str())
        && !n.dispositivo_asociado.as_deref().is_some_and(|d| !vacio(d))
    {
        return Err(Falta::IaasSinDispositivo);
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

const SELECT_EQUIPO: &str = "\
    SELECT e.id, e.nombre, e.marca, e.modelo, e.serie, e.ubicacion, e.registro_invima, \
    e.fecha_adquisicion, e.estado, e.periodicidad_meses, e.proximo_mantenimiento, \
    e.motivo_baja, e.creado_en, \
    (SELECT COUNT(*) FROM mantenimiento_equipo m WHERE m.equipo_id = e.id) AS mantenimientos, \
    (SELECT MAX(m.fecha) FROM mantenimiento_equipo m WHERE m.equipo_id = e.id) AS ultimo_mantenimiento \
    FROM equipo_biomedico e";

pub async fn listar_equipos(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Equipo>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_EQUIPO);
    if inst.is_some() {
        sql.push_str(" WHERE e.institucion_id = ?");
    }
    sql.push_str(" ORDER BY e.nombre, e.id");
    let mut q = sqlx::query_as::<_, Equipo>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

pub async fn registrar_equipo(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevoEquipo,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar_equipo(n) {
        return Ok(Err(f));
    }
    let Some(Some(institucion_id)) = institucion(ambito) else {
        // Superadmin no tiene institución a la que colgarle un equipo, y
        // `Ninguno` no puede nada. Las dos responden igual.
        return Ok(Err(Falta::EquipoInexistente));
    };
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO equipo_biomedico (institucion_id, nombre, marca, modelo, serie, ubicacion, \
         registro_invima, fecha_adquisicion, periodicidad_meses, creado_en) \
         VALUES (?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(institucion_id)
    .bind(n.nombre.trim())
    .bind(n.marca.as_deref())
    .bind(n.modelo.as_deref())
    .bind(n.serie.as_deref())
    .bind(n.ubicacion.as_deref())
    .bind(n.registro_invima.as_deref())
    .bind(n.fecha_adquisicion.as_deref())
    .bind(n.periodicidad_meses)
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Registra un mantenimiento y **recalcula el próximo** con la periodicidad del
/// equipo.
///
/// El próximo se escribe aquí y no lo teclea nadie: dejarlo a mano es cómo un
/// cronograma se convierte en una fecha que nadie movió desde 2019.
pub async fn registrar_mantenimiento(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevoMantenimiento,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar_mantenimiento(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::EquipoInexistente));
    };
    let mut sql = String::from("SELECT periodicidad_meses FROM equipo_biomedico WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, Option<i64>>(&sql).bind(n.equipo_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let Some(periodicidad) = q.fetch_optional(pool).await? else {
        return Ok(Err(Falta::EquipoInexistente));
    };

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO mantenimiento_equipo (equipo_id, tipo, fecha, responsable, hallazgos, \
         conforme, costo, creado_en) VALUES (?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.equipo_id)
    .bind(&n.tipo)
    .bind(&n.fecha)
    .bind(n.responsable.trim())
    .bind(n.hallazgos.as_deref())
    .bind(n.conforme)
    .bind(n.costo)
    .bind(&ahora)
    .fetch_one(pool)
    .await?;

    // Solo si hay periodicidad. Sin ella no se inventa una fecha: el equipo
    // seguirá saliendo como «sin cronograma», que es la verdad.
    if let Some(meses) = periodicidad
        && let Some(proximo) = sumar_meses(&n.fecha, meses)
    {
        sqlx::query("UPDATE equipo_biomedico SET proximo_mantenimiento = ? WHERE id = ?")
            .bind(&proximo)
            .bind(n.equipo_id)
            .execute(pool)
            .await?;
    }
    Ok(Ok(id))
}

/// Suma meses a una fecha `AAAA-MM-DD` (los primeros 10 caracteres de lo que
/// llegue). Devuelve `None` si no se puede leer — y entonces el próximo
/// mantenimiento se queda como estaba, en vez de moverse a una fecha inventada.
fn sumar_meses(fecha: &str, meses: i64) -> Option<String> {
    let d = chrono::NaiveDate::parse_from_str(&fecha[..10.min(fecha.len())], "%Y-%m-%d").ok()?;
    let total = d.month0() as i64 + meses;
    let anio = d.year() as i64 + total.div_euclid(12);
    let mes = total.rem_euclid(12) as u32 + 1;
    // Si el día no existe en el mes destino (31 de febrero), se retrocede al
    // último día del mes. Saltar al 1 del siguiente adelantaría el cronograma.
    let mut dia = d.day();
    loop {
        if let Some(x) = chrono::NaiveDate::from_ymd_opt(anio as i32, mes, dia) {
            return Some(x.format("%Y-%m-%d").to_string());
        }
        dia -= 1;
        if dia == 0 {
            return None;
        }
    }
}

use chrono::Datelike;

const SELECT_COMITE: &str = "\
    SELECT c.id, c.tipo, c.nombre, c.periodicidad_meses, c.activo, c.creado_en, \
    (SELECT COUNT(*) FROM sesion_comite s WHERE s.comite_id = c.id) AS sesiones, \
    (SELECT MAX(s.fecha) FROM sesion_comite s WHERE s.comite_id = c.id) AS ultima_sesion \
    FROM comite c";

pub async fn listar_comites(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Comite>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_COMITE);
    if inst.is_some() {
        sql.push_str(" WHERE c.institucion_id = ?");
    }
    sql.push_str(" ORDER BY c.tipo, c.id");
    let mut q = sqlx::query_as::<_, Comite>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

pub async fn crear_comite(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevoComite,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if !TIPOS_COMITE.iter().any(|(t, _)| *t == n.tipo) {
        return Ok(Err(Falta::TipoDesconocido));
    }
    if vacio(&n.nombre) {
        return Ok(Err(Falta::NombreVacio));
    }
    if let Some(p) = n.periodicidad_meses
        && p <= 0
    {
        return Ok(Err(Falta::PeriodicidadInvalida));
    }
    let Some(Some(institucion_id)) = institucion(ambito) else {
        return Ok(Err(Falta::ComiteInexistente));
    };
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO comite (institucion_id, tipo, nombre, periodicidad_meses, creado_en) \
         VALUES (?,?,?,?,?) RETURNING id",
    )
    .bind(institucion_id)
    .bind(&n.tipo)
    .bind(n.nombre.trim())
    .bind(n.periodicidad_meses)
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

pub async fn sesionar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaSesion,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar_sesion(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::ComiteInexistente));
    };
    let mut sql = String::from("SELECT 1 FROM comite WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(n.comite_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::ComiteInexistente));
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO sesion_comite (comite_id, fecha, asistentes, temas, decisiones, creado_en) \
         VALUES (?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.comite_id)
    .bind(&n.fecha)
    .bind(n.asistentes.trim())
    .bind(n.temas.as_deref())
    .bind(n.decisiones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

const SELECT_IAAS: &str = "\
    SELECT v.id, v.paciente_id, v.hospitalizacion_id, v.tipo, v.fecha_deteccion, \
    v.dispositivo_asociado, v.dias_dispositivo, v.microorganismo, v.notificado, \
    v.fecha_notificacion, v.desenlace, v.creado_en, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente \
    FROM evento_iaas v JOIN paciente p ON p.id = v.paciente_id";

pub async fn listar_iaas(
    pool: &crate::Pool,
    ambito: Ambito,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Vec<EventoIaas>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_IAAS);
    sql.push_str(" WHERE 1=1");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    if desde.is_some() {
        sql.push_str(" AND date(v.fecha_deteccion) >= date(?)");
    }
    if hasta.is_some() {
        sql.push_str(" AND date(v.fecha_deteccion) <= date(?)");
    }
    sql.push_str(" ORDER BY v.fecha_deteccion DESC, v.id DESC");
    let mut q = sqlx::query_as::<_, EventoIaas>(&sql);
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

pub async fn reportar_iaas(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevoEventoIaas,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar_iaas(n) {
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
    if !opcional_es_del_paciente(
        pool,
        Enlace::Hospitalizacion,
        n.hospitalizacion_id,
        n.paciente_id,
    )
    .await?
    {
        return Ok(Err(Falta::AtencionQueNoEsDelPaciente));
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO evento_iaas (paciente_id, hospitalizacion_id, reportado_por, tipo, \
         fecha_deteccion, dispositivo_asociado, dias_dispositivo, microorganismo, desenlace, \
         observaciones, creado_en) VALUES (?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.paciente_id)
    .bind(n.hospitalizacion_id)
    .bind(usuario_id)
    .bind(&n.tipo)
    .bind(&n.fecha_deteccion)
    .bind(n.dispositivo_asociado.as_deref())
    .bind(n.dias_dispositivo)
    .bind(n.microorganismo.as_deref())
    .bind(n.desenlace.as_deref())
    .bind(n.observaciones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Marca un evento como notificado al sistema nacional de vigilancia.
///
/// La fecha la pone el SERVIDOR: «notificado» sin fecha es una afirmación que
/// nadie puede comprobar, y el `CHECK` de la migración lo impide igual.
pub async fn notificar_iaas(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Result<(), Falta>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::PacienteInexistente));
    };
    let mut sql = String::from(
        "SELECT 1 FROM evento_iaas v JOIN paciente p ON p.id = v.paciente_id WHERE v.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::PacienteInexistente));
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query("UPDATE evento_iaas SET notificado = 1, fecha_notificacion = ? WHERE id = ?")
        .bind(&ahora)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Ok(()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mantenimiento_no_conforme_exige_hallazgos() {
        let mut n = NuevoMantenimiento {
            equipo_id: 1,
            tipo: "PREVENTIVO".into(),
            fecha: "2026-08-16".into(),
            responsable: "Biomédica S.A.".into(),
            hallazgos: None,
            conforme: false,
            costo: None,
        };
        assert_eq!(revisar_mantenimiento(&n), Err(Falta::NoConformeSinHallazgos));
        n.hallazgos = Some("fuga en el circuito de paciente".into());
        assert_eq!(revisar_mantenimiento(&n), Ok(()));
        // Y el conforme NO los exige: la regla es del no conforme.
        n.conforme = true;
        n.hallazgos = None;
        assert_eq!(revisar_mantenimiento(&n), Ok(()));
    }

    #[test]
    fn la_infeccion_asociada_a_dispositivo_exige_el_dispositivo() {
        for tipo in IAAS_CON_DISPOSITIVO {
            let n = NuevoEventoIaas {
                paciente_id: 10,
                hospitalizacion_id: None,
                tipo: tipo.into(),
                fecha_deteccion: "2026-08-16".into(),
                dispositivo_asociado: None,
                dias_dispositivo: None,
                microorganismo: None,
                desenlace: None,
                observaciones: None,
            };
            assert_eq!(revisar_iaas(&n), Err(Falta::IaasSinDispositivo), "{tipo}");
        }
        // Su pareja: los otros dos tipos NO lo exigen.
        for tipo in ["INFECCION_SITIO_OPERATORIO", "OTRA"] {
            let n = NuevoEventoIaas {
                paciente_id: 10,
                hospitalizacion_id: None,
                tipo: tipo.into(),
                fecha_deteccion: "2026-08-16".into(),
                dispositivo_asociado: None,
                dias_dispositivo: None,
                microorganismo: None,
                desenlace: None,
                observaciones: None,
            };
            assert_eq!(revisar_iaas(&n), Ok(()), "{tipo} no lleva dispositivo obligatorio");
        }
    }

    #[test]
    fn la_sesion_sin_asistentes_se_rechaza() {
        let mut n = NuevaSesion {
            comite_id: 1,
            fecha: "2026-08-16".into(),
            asistentes: "   ".into(),
            temas: None,
            decisiones: None,
        };
        assert_eq!(revisar_sesion(&n), Err(Falta::SesionSinAsistentes));
        n.asistentes = "Dra. Ruiz, Enf. Sol".into();
        assert_eq!(revisar_sesion(&n), Ok(()));
    }

    /// `vencido` devuelve TRES cosas y no dos: sin cronograma es un hallazgo
    /// distinto de vencido, y peor.
    #[test]
    fn el_vencimiento_distingue_al_dia_vencido_y_sin_cronograma() {
        let base = Equipo {
            id: 1,
            nombre: "Ecógrafo".into(),
            marca: None,
            modelo: None,
            serie: None,
            ubicacion: None,
            registro_invima: None,
            fecha_adquisicion: None,
            estado: "ACTIVO".into(),
            periodicidad_meses: Some(6),
            proximo_mantenimiento: Some("2026-09-01".into()),
            motivo_baja: None,
            creado_en: "x".into(),
            mantenimientos: 1,
            ultimo_mantenimiento: Some("2026-03-01".into()),
        };
        assert_eq!(base.vencido("2026-08-16"), Some(false), "al día");
        let mut v = base.clone();
        v.proximo_mantenimiento = Some("2026-08-01".into());
        assert_eq!(v.vencido("2026-08-16"), Some(true), "vencido");
        let mut sin = base.clone();
        sin.proximo_mantenimiento = None;
        assert_eq!(sin.vencido("2026-08-16"), None, "no se puede saber");
        assert!(!base.sin_cronograma());
        let mut nada = base.clone();
        nada.periodicidad_meses = None;
        assert!(nada.sin_cronograma());
        // Un equipo dado de baja no vence: ya no está.
        let mut baja = base;
        baja.estado = "DADO_DE_BAJA".into();
        assert_eq!(baja.vencido("2027-01-01"), None);
    }

    /// El comité activo con periodicidad y SIN una sola sesión está en mora por
    /// definición: es el caso que más se ve en una visita, y el que un `MAX()`
    /// nulo dejaría pasar por «al día».
    #[test]
    fn el_comite_sin_sesiones_esta_en_mora() {
        let base = Comite {
            id: 1,
            tipo: "INFECCIONES".into(),
            nombre: "Comité de infecciones".into(),
            periodicidad_meses: Some(1),
            activo: true,
            creado_en: "x".into(),
            sesiones: 0,
            ultima_sesion: None,
        };
        assert_eq!(base.en_mora("2026-08-16"), Some(true));
        let mut reciente = base.clone();
        reciente.sesiones = 1;
        reciente.ultima_sesion = Some("2026-08-01".into());
        assert_eq!(reciente.en_mora("2026-08-16"), Some(false), "sesionó hace dos semanas");
        let mut vieja = base.clone();
        vieja.sesiones = 1;
        vieja.ultima_sesion = Some("2026-01-01".into());
        assert_eq!(vieja.en_mora("2026-08-16"), Some(true));
        // Sin periodicidad no hay mora que medir: decir «al día» sería afirmar
        // lo que nadie comprobó.
        let mut sin = base.clone();
        sin.periodicidad_meses = None;
        assert_eq!(sin.en_mora("2026-08-16"), None);
        // Y un comité inactivo tampoco.
        let mut inactivo = base;
        inactivo.activo = false;
        assert_eq!(inactivo.en_mora("2026-08-16"), None);
    }

    /// Sumar meses respeta el fin de mes: el 31 de enero + 1 mes es el 28 (o 29)
    /// de febrero, no el 3 de marzo. Adelantar el cronograma haría que el
    /// mantenimiento «venciera» antes de tiempo.
    #[test]
    fn sumar_meses_no_se_pasa_de_mes() {
        assert_eq!(sumar_meses("2026-01-31", 1).as_deref(), Some("2026-02-28"));
        assert_eq!(sumar_meses("2026-03-15", 6).as_deref(), Some("2026-09-15"));
        assert_eq!(sumar_meses("2026-12-01", 1).as_deref(), Some("2027-01-01"));
        // Una fecha ilegible no produce una inventada.
        assert_eq!(sumar_meses("el martes", 1), None);
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
        pool
    }

    fn equipo() -> NuevoEquipo {
        NuevoEquipo {
            nombre: "Ventilador".into(),
            marca: Some("Acme".into()),
            modelo: None,
            serie: Some("VS-001".into()),
            ubicacion: Some("UCI".into()),
            registro_invima: None,
            fecha_adquisicion: Some("2024-01-01".into()),
            periodicidad_meses: Some(6),
        }
    }

    /// El próximo mantenimiento **lo calcula el servidor** al registrar uno.
    /// Dejarlo a mano es cómo un cronograma se convierte en una fecha que nadie
    /// movió desde 2019.
    /// **El ingreso que se enlaza tiene que ser de ese paciente** — caso rojo de una
    /// de las cuatro claves foráneas que se ataban en crudo.
    ///
    /// Las encontró la revisión de seguridad de esta rama, y las dejó por debajo
    /// del listón porque hoy ninguna consulta de lectura une estas tablas: el
    /// daño es una fila de la institución A apuntando a una de la B, no una
    /// fuga. Se cierran igual, y en la misma pasada que la quinta —la de
    /// `defuncion`, donde el mismo descuido SÍ era un oráculo—, porque son **un
    /// defecto mirado por dos lados** y el lado callado no se queja nunca
    /// (directiva 23).
    #[tokio::test]
    async fn el_ingreso_que_no_es_del_paciente_no_se_enlaza() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (1,'101','1','MEDICINA','OCUPADA',1),(2,'201','2','MEDICINA','OCUPADA',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (77,10,1,6,'2026-08-15T10:00:00Z','ACTIVA'), \
                    (88,20,2,6,'2026-08-14T03:17:00Z','ACTIVA')",
        )
        .execute(&pool).await.unwrap();
        const ESPERADO: Result<i64, Falta> = Err(Falta::AtencionQueNoEsDelPaciente);
        let iaas = |h: Option<i64>| NuevoEventoIaas {
            paciente_id: 10, hospitalizacion_id: h, tipo: "OTRA".into(),
            fecha_deteccion: "2026-08-16".into(), dispositivo_asociado: None,
            dias_dispositivo: None, microorganismo: None, desenlace: None, observaciones: None,
        };
        let a = Ambito::Institucion(1);
        assert_eq!(reportar_iaas(&pool, a, &iaas(Some(88)), 6).await.unwrap(), ESPERADO);
        assert_eq!(reportar_iaas(&pool, a, &iaas(Some(9999)), 6).await.unwrap(), ESPERADO);
        // POSITIVO DE CONTROL: el ingreso del propio paciente sí entra.
        assert!(reportar_iaas(&pool, a, &iaas(Some(77)), 6).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn el_mantenimiento_mueve_el_proximo() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_equipo(&pool, a, &equipo()).await.unwrap().unwrap();
        let antes = listar_equipos(&pool, a).await.unwrap();
        assert!(antes[0].proximo_mantenimiento.is_none(), "recién nacido, sin próximo");

        registrar_mantenimiento(&pool, a, &NuevoMantenimiento {
            equipo_id: id, tipo: "PREVENTIVO".into(), fecha: "2026-03-15".into(),
            responsable: "Biomédica S.A.".into(), hallazgos: None, conforme: true, costo: Some(120000.0),
        }).await.unwrap().unwrap();

        let despues = listar_equipos(&pool, a).await.unwrap();
        assert_eq!(despues[0].proximo_mantenimiento.as_deref(), Some("2026-09-15"));
        assert_eq!(despues[0].mantenimientos, 1);
    }

    /// Sin periodicidad NO se inventa un próximo: el equipo sigue saliendo como
    /// «sin cronograma», que es la verdad y es lo que la visita anota.
    #[tokio::test]
    async fn sin_periodicidad_no_se_inventa_el_proximo() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let mut n = equipo();
        n.periodicidad_meses = None;
        n.serie = Some("VS-002".into());
        let id = registrar_equipo(&pool, a, &n).await.unwrap().unwrap();
        registrar_mantenimiento(&pool, a, &NuevoMantenimiento {
            equipo_id: id, tipo: "CORRECTIVO".into(), fecha: "2026-03-15".into(),
            responsable: "X".into(), hallazgos: Some("cambio de sensor".into()),
            conforme: false, costo: None,
        }).await.unwrap().unwrap();
        let v = listar_equipos(&pool, a).await.unwrap();
        assert!(v[0].proximo_mantenimiento.is_none());
        assert!(v[0].sin_cronograma());
    }

    #[tokio::test]
    async fn el_ambito_aisla_equipos_comites_e_iaas() {
        let pool = base().await;
        registrar_equipo(&pool, Ambito::Institucion(1), &equipo()).await.unwrap().unwrap();
        crear_comite(&pool, Ambito::Institucion(1), &NuevoComite {
            tipo: "INFECCIONES".into(), nombre: "Comité".into(), periodicidad_meses: Some(1),
        }).await.unwrap().unwrap();
        reportar_iaas(&pool, Ambito::Institucion(1), &NuevoEventoIaas {
            paciente_id: 10, hospitalizacion_id: None, tipo: "OTRA".into(),
            fecha_deteccion: "2026-08-16".into(), dispositivo_asociado: None,
            dias_dispositivo: None, microorganismo: None, desenlace: None, observaciones: None,
        }, 6).await.unwrap().unwrap();

        assert!(listar_equipos(&pool, Ambito::Institucion(2)).await.unwrap().is_empty());
        assert!(listar_comites(&pool, Ambito::Institucion(2)).await.unwrap().is_empty());
        assert!(listar_iaas(&pool, Ambito::Institucion(2), None, None).await.unwrap().is_empty());
        assert_eq!(listar_equipos(&pool, Ambito::Institucion(1)).await.unwrap().len(), 1);
        assert!(listar_equipos(&pool, Ambito::Ninguno).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_se_reporta_infeccion_del_paciente_de_otra_institucion() {
        let pool = base().await;
        let n = NuevoEventoIaas {
            paciente_id: 20, hospitalizacion_id: None, tipo: "OTRA".into(),
            fecha_deteccion: "2026-08-16".into(), dispositivo_asociado: None,
            dias_dispositivo: None, microorganismo: None, desenlace: None, observaciones: None,
        };
        assert_eq!(
            reportar_iaas(&pool, Ambito::Institucion(1), &n, 6).await.unwrap(),
            Err(Falta::PacienteInexistente)
        );
    }

    /// Notificar pone la fecha; el `CHECK` impide la afirmación sin fecha por la
    /// puerta de atrás, con su positivo de control.
    #[tokio::test]
    async fn notificado_sin_fecha_no_entra_ni_por_la_puerta_de_atras() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = reportar_iaas(&pool, a, &NuevoEventoIaas {
            paciente_id: 10, hospitalizacion_id: None, tipo: "OTRA".into(),
            fecha_deteccion: "2026-08-16".into(), dispositivo_asociado: None,
            dias_dispositivo: None, microorganismo: None, desenlace: None, observaciones: None,
        }, 6).await.unwrap().unwrap();
        notificar_iaas(&pool, a, id).await.unwrap().unwrap();
        let v = listar_iaas(&pool, a, None, None).await.unwrap();
        assert!(v[0].notificado);
        assert!(v[0].fecha_notificacion.is_some());

        let sql = "INSERT INTO evento_iaas (paciente_id,reportado_por,tipo,fecha_deteccion,\
                   notificado,fecha_notificacion,creado_en) \
                   VALUES (10,6,'OTRA','2026-08-16',?,?,'x')";
        sqlx::query(sql).bind(1).bind(Some("2026-08-16")).execute(&pool).await
            .expect("notificado CON fecha entra");
        let sin: Option<&str> = None;
        assert!(sqlx::query(sql).bind(1).bind(sin).execute(&pool).await.is_err(),
                "notificado SIN fecha no");
    }
}
