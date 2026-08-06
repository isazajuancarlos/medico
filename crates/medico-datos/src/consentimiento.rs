//! Consentimiento informado. Port de `backend/routers/consentimiento.py`.
//!
//! El juicio de si un consentimiento vale vive en
//! [`medico_dominio::consentimiento`] y aquí no se repite: este módulo lee, mide
//! y escribe filas, y llama a `evaluar` para adjuntar el veredicto.
//!
//! # Aislamiento
//!
//! La tabla **no lleva `institucion_id`**: es clínica y cuelga del paciente,
//! como `historia` y `documento_clinico`. Toda vía de entrada pasa por el
//! paciente, y el paciente por [`filtro_ambito`](crate::filtro_ambito). Un
//! consentimiento de otra institución responde **404, nunca 403**: un 403
//! confirma que el id existe, y con eso se enumera quién es paciente de la
//! clínica de al lado — que también es un dato de salud.
//!
//! # Lo que el Python no validaba, y aquí sí
//!
//! `crear` recibía `historia_id` del cuerpo y lo escribía sin mirarlo
//! (`ConsentimientoInformado(**payload.model_dump())`). Es exactamente el hueco
//! que se cazó hoy en la facturación con `paciente_id`: un identificador que
//! llega del navegador y entra en un `INSERT` sin comprobar de quién es. Aquí la
//! historia tiene que ser **del mismo paciente**, que además es más estricto que
//! «de la misma institución» y es lo único que tiene sentido: un consentimiento
//! cuelga de la atención de ESE paciente o de ninguna.

use medico_dominio::consentimiento::{evaluar, AJuzgar, Validez};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

/// La fila, tal cual está en la base.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Fila {
    pub id: i64,
    pub paciente_id: i64,
    pub historia_id: Option<i64>,
    pub medico_id: i64,
    pub fecha_firma: String,
    pub tipo_procedimiento: String,
    pub codigo_cups: Option<String>,
    pub descripcion_procedimiento: String,
    pub riesgos_explicados: String,
    pub beneficios_esperados: String,
    pub alternativas_explicadas: String,
    pub consecuencias_no_realizarlo: Option<String>,
    pub paciente_comprende: bool,
    pub acepta_procedimiento: bool,
    pub firma_paciente_path: Option<String>,
    pub firma_medico_path: Option<String>,
    pub testigo_nombre: Option<String>,
    pub testigo_documento: Option<String>,
    pub testigo_firma_path: Option<String>,
    pub revocado: bool,
    pub fecha_revocacion: Option<String>,
    pub motivo_revocacion: Option<String>,
}

impl Fila {
    fn a_juzgar(&self) -> AJuzgar<'_> {
        AJuzgar {
            descripcion_procedimiento: &self.descripcion_procedimiento,
            riesgos_explicados: &self.riesgos_explicados,
            beneficios_esperados: &self.beneficios_esperados,
            alternativas_explicadas: &self.alternativas_explicadas,
            paciente_comprende: self.paciente_comprende,
            acepta_procedimiento: self.acepta_procedimiento,
            firma_paciente_path: self.firma_paciente_path.as_deref(),
            firma_medico_path: self.firma_medico_path.as_deref(),
            revocado: self.revocado,
        }
    }
}

/// Lo que sale por la API: la fila, los dos nombres y el veredicto.
#[derive(Debug, Clone, Serialize)]
pub struct Lectura {
    #[serde(flatten)]
    pub fila: Fila,
    pub paciente: Option<String>,
    pub medico: Option<String>,
    #[serde(flatten)]
    pub validez: Validez,
}

/// Un consentimiento nuevo.
///
/// **`medico_id` no está aquí a propósito**: lo pone la ruta con el usuario de
/// la sesión. Si viniera del cuerpo, cualquiera podría firmar un consentimiento
/// en nombre de otro profesional.
#[derive(Debug, Clone, Deserialize)]
pub struct Nuevo {
    pub paciente_id: i64,
    #[serde(default)]
    pub historia_id: Option<i64>,
    pub tipo_procedimiento: String,
    #[serde(default)]
    pub codigo_cups: Option<String>,
    #[serde(default)]
    pub descripcion_procedimiento: String,
    #[serde(default)]
    pub riesgos_explicados: String,
    #[serde(default)]
    pub beneficios_esperados: String,
    #[serde(default)]
    pub alternativas_explicadas: String,
    #[serde(default)]
    pub consecuencias_no_realizarlo: Option<String>,
    #[serde(default)]
    pub paciente_comprende: bool,
    #[serde(default)]
    pub acepta_procedimiento: bool,
    #[serde(default)]
    pub firma_paciente_path: Option<String>,
    #[serde(default)]
    pub firma_medico_path: Option<String>,
    #[serde(default)]
    pub testigo_nombre: Option<String>,
    #[serde(default)]
    pub testigo_documento: Option<String>,
    #[serde(default)]
    pub testigo_firma_path: Option<String>,
}

/// Una edición. Todo opcional: lo que no venga no se toca.
///
/// **Ni `revocado` ni sus dos campos se pueden editar por aquí.** La revocación
/// tiene su propia ruta porque es un acto del paciente, no una corrección de
/// captura; dejarla entrar en el actualizador general permitiría *des*-revocar
/// un consentimiento con un `PUT`, y con eso la constancia dejaría de serlo.
/// Tampoco `paciente_id`: mover un consentimiento de titular no es editarlo.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Cambios {
    pub historia_id: Option<Option<i64>>,
    pub tipo_procedimiento: Option<String>,
    pub codigo_cups: Option<Option<String>>,
    pub descripcion_procedimiento: Option<String>,
    pub riesgos_explicados: Option<String>,
    pub beneficios_esperados: Option<String>,
    pub alternativas_explicadas: Option<String>,
    pub consecuencias_no_realizarlo: Option<Option<String>>,
    pub paciente_comprende: Option<bool>,
    pub acepta_procedimiento: Option<bool>,
    pub firma_paciente_path: Option<Option<String>>,
    pub firma_medico_path: Option<Option<String>>,
    pub testigo_nombre: Option<Option<String>>,
    pub testigo_documento: Option<Option<String>>,
    pub testigo_firma_path: Option<Option<String>>,
}

/// Por qué no se pudo escribir.
///
/// `NoExiste` cubre también «es de otra institución», y eso es la regla del 404:
/// las dos situaciones tienen que ser indistinguibles desde fuera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estorbo {
    /// No existe, o es de otra institución.
    NoExiste,
    /// El paciente del cuerpo no existe o es de otra institución.
    PacienteAjeno,
    /// La historia indicada no es de ese paciente (o no existe).
    HistoriaAjena,
    /// Está revocado: no se edita. Se registra uno nuevo, para que quede
    /// constancia de que hubo una revocación en medio.
    Revocado,
}

const CAMPOS: &str = "id, paciente_id, historia_id, medico_id, fecha_firma, tipo_procedimiento, \
    codigo_cups, descripcion_procedimiento, riesgos_explicados, beneficios_esperados, \
    alternativas_explicadas, consecuencias_no_realizarlo, paciente_comprende, \
    acepta_procedimiento, firma_paciente_path, firma_medico_path, testigo_nombre, \
    testigo_documento, testigo_firma_path, revocado, fecha_revocacion, motivo_revocacion";

const NOMBRE_PACIENTE: &str =
    "(p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,''))";

/// ¿Ese paciente es del ámbito? `false` si no existe: las dos cosas se responden
/// igual desde fuera.
async fn paciente_visible(
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

/// ¿Esa historia es de ese paciente? Es la comprobación que faltaba en el
/// Python. Se pregunta por el paciente y no por la institución a propósito: es
/// más estricta y es la única que significa algo aquí.
async fn historia_del_paciente(
    pool: &crate::Pool,
    historia_id: i64,
    paciente_id: i64,
) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM historia WHERE id = ? AND paciente_id = ?")
            .bind(historia_id)
            .bind(paciente_id)
            .fetch_optional(pool)
            .await?;
    Ok(fila.is_some())
}

/// La fila, si es visible para el ámbito. `None` si no existe o es ajena.
async fn fila_visible(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Fila>, sqlx::Error> {
    let Some((clausula, valor)) = crate::filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {} FROM consentimiento_informado c JOIN paciente p ON p.id = c.paciente_id \
         WHERE c.id = ? AND {clausula}",
        CAMPOS
            .split(", ")
            .map(|x| format!("c.{x}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut q = sqlx::query_as::<_, Fila>(&sql).bind(id);
    if let Some(v) = valor {
        q = q.bind(v);
    }
    q.fetch_optional(pool).await
}

/// Adjunta los dos nombres y el veredicto.
///
/// Los nombres se buscan **acotados a la institución del paciente**, no por id
/// suelto: el médico que firmó es de la misma institución, y una consulta sin
/// acotar devolvería el nombre de un profesional ajeno si algún día una fila
/// quedara mal enlazada. Cuando no casa, sale `null` — no se rellena con el id
/// ni con una cadena vacía (directiva 20).
async fn a_lectura(pool: &crate::Pool, fila: Fila) -> Result<Lectura, sqlx::Error> {
    let paciente: Option<(String,)> = sqlx::query_as(&format!(
        "SELECT {NOMBRE_PACIENTE} FROM paciente p WHERE p.id = ?"
    ))
    .bind(fila.paciente_id)
    .fetch_optional(pool)
    .await?;
    let medico: Option<(String,)> = sqlx::query_as(
        "SELECT u.nombre FROM usuario u JOIN paciente p ON p.id = ? \
         WHERE u.id = ? AND u.institucion_id = p.institucion_id",
    )
    .bind(fila.paciente_id)
    .bind(fila.medico_id)
    .fetch_optional(pool)
    .await?;
    let validez = evaluar(fila.a_juzgar());
    Ok(Lectura {
        fila,
        paciente: paciente.map(|(n,)| n),
        medico: medico.map(|(n,)| n),
        validez,
    })
}

/// Lista los consentimientos del ámbito, del más reciente al más antiguo.
///
/// `solo_validos` filtra **después** de evaluar, no en SQL, porque la validez no
/// es una columna: es el juicio del dominio. Meterla en el `WHERE` sería
/// escribir la regla por segunda vez en otro lenguaje, que es como acaban dos
/// reglas divergiendo.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: Option<i64>,
    solo_validos: bool,
) -> Result<Vec<Lectura>, sqlx::Error> {
    let Some((clausula, valor)) = crate::filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let campos = CAMPOS
        .split(", ")
        .map(|x| format!("c.{x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!(
        "SELECT {campos} FROM consentimiento_informado c \
         JOIN paciente p ON p.id = c.paciente_id WHERE {clausula}"
    );
    if paciente_id.is_some() {
        sql.push_str(" AND c.paciente_id = ?");
    }
    sql.push_str(" ORDER BY c.fecha_firma DESC, c.id DESC");

    let mut q = sqlx::query_as::<_, Fila>(&sql);
    if let Some(v) = valor {
        q = q.bind(v);
    }
    if let Some(p) = paciente_id {
        q = q.bind(p);
    }
    let filas = q.fetch_all(pool).await?;

    let mut salida = Vec::with_capacity(filas.len());
    for f in filas {
        let l = a_lectura(pool, f).await?;
        if !solo_validos || l.validez.valido {
            salida.push(l);
        }
    }
    Ok(salida)
}

/// Un consentimiento por id, si es del ámbito.
pub async fn detalle(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Lectura>, sqlx::Error> {
    match fila_visible(pool, ambito, id).await? {
        Some(f) => Ok(Some(a_lectura(pool, f).await?)),
        None => Ok(None),
    }
}

/// Registra un consentimiento. `medico_id` es el de la sesión, no el del cuerpo.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    medico_id: i64,
    n: &Nuevo,
) -> Result<Result<Lectura, Estorbo>, sqlx::Error> {
    if !paciente_visible(pool, ambito, n.paciente_id).await? {
        return Ok(Err(Estorbo::PacienteAjeno));
    }
    if let Some(h) = n.historia_id
        && !historia_del_paciente(pool, h, n.paciente_id).await?
    {
        return Ok(Err(Estorbo::HistoriaAjena));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO consentimiento_informado \
         (paciente_id, historia_id, medico_id, fecha_firma, tipo_procedimiento, codigo_cups, \
          descripcion_procedimiento, riesgos_explicados, beneficios_esperados, \
          alternativas_explicadas, consecuencias_no_realizarlo, paciente_comprende, \
          acepta_procedimiento, firma_paciente_path, firma_medico_path, testigo_nombre, \
          testigo_documento, testigo_firma_path) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.paciente_id)
    .bind(n.historia_id)
    .bind(medico_id)
    .bind(crate::ahora())
    .bind(&n.tipo_procedimiento)
    .bind(&n.codigo_cups)
    .bind(&n.descripcion_procedimiento)
    .bind(&n.riesgos_explicados)
    .bind(&n.beneficios_esperados)
    .bind(&n.alternativas_explicadas)
    .bind(&n.consecuencias_no_realizarlo)
    .bind(n.paciente_comprende)
    .bind(n.acepta_procedimiento)
    .bind(&n.firma_paciente_path)
    .bind(&n.firma_medico_path)
    .bind(&n.testigo_nombre)
    .bind(&n.testigo_documento)
    .bind(&n.testigo_firma_path)
    .fetch_one(pool)
    .await?;

    let f = fila_visible(pool, ambito, id)
        .await?
        .expect("recién insertada y del ámbito comprobado");
    Ok(Ok(a_lectura(pool, f).await?))
}

/// Edita un consentimiento no revocado.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &Cambios,
) -> Result<Result<Lectura, Estorbo>, sqlx::Error> {
    let Some(actual) = fila_visible(pool, ambito, id).await? else {
        return Ok(Err(Estorbo::NoExiste));
    };
    if actual.revocado {
        return Ok(Err(Estorbo::Revocado));
    }
    if let Some(Some(h)) = c.historia_id
        && !historia_del_paciente(pool, h, actual.paciente_id).await?
    {
        return Ok(Err(Estorbo::HistoriaAjena));
    }

    // `COALESCE(?, columna)` no serviría: no distinguiría «no lo mandes» de
    // «ponlo a NULL», y `codigo_cups` o la firma se tienen que poder borrar.
    // Se arma el SET con lo que vino y nada más.
    let mut sets: Vec<&str> = Vec::new();
    macro_rules! texto {
        ($campo:ident, $col:literal) => {
            if c.$campo.is_some() {
                sets.push(concat!($col, " = ?"));
            }
        };
    }
    texto!(historia_id, "historia_id");
    texto!(tipo_procedimiento, "tipo_procedimiento");
    texto!(codigo_cups, "codigo_cups");
    texto!(descripcion_procedimiento, "descripcion_procedimiento");
    texto!(riesgos_explicados, "riesgos_explicados");
    texto!(beneficios_esperados, "beneficios_esperados");
    texto!(alternativas_explicadas, "alternativas_explicadas");
    texto!(consecuencias_no_realizarlo, "consecuencias_no_realizarlo");
    texto!(paciente_comprende, "paciente_comprende");
    texto!(acepta_procedimiento, "acepta_procedimiento");
    texto!(firma_paciente_path, "firma_paciente_path");
    texto!(firma_medico_path, "firma_medico_path");
    texto!(testigo_nombre, "testigo_nombre");
    texto!(testigo_documento, "testigo_documento");
    texto!(testigo_firma_path, "testigo_firma_path");

    if sets.is_empty() {
        return Ok(Ok(a_lectura(pool, actual).await?));
    }

    let sql = format!(
        "UPDATE consentimiento_informado SET {} WHERE id = ?",
        sets.join(", ")
    );
    let mut q = sqlx::query(&sql);
    if let Some(v) = c.historia_id {
        q = q.bind(v);
    }
    if let Some(v) = &c.tipo_procedimiento {
        q = q.bind(v);
    }
    if let Some(v) = &c.codigo_cups {
        q = q.bind(v);
    }
    if let Some(v) = &c.descripcion_procedimiento {
        q = q.bind(v);
    }
    if let Some(v) = &c.riesgos_explicados {
        q = q.bind(v);
    }
    if let Some(v) = &c.beneficios_esperados {
        q = q.bind(v);
    }
    if let Some(v) = &c.alternativas_explicadas {
        q = q.bind(v);
    }
    if let Some(v) = &c.consecuencias_no_realizarlo {
        q = q.bind(v);
    }
    if let Some(v) = c.paciente_comprende {
        q = q.bind(v);
    }
    if let Some(v) = c.acepta_procedimiento {
        q = q.bind(v);
    }
    if let Some(v) = &c.firma_paciente_path {
        q = q.bind(v);
    }
    if let Some(v) = &c.firma_medico_path {
        q = q.bind(v);
    }
    if let Some(v) = &c.testigo_nombre {
        q = q.bind(v);
    }
    if let Some(v) = &c.testigo_documento {
        q = q.bind(v);
    }
    if let Some(v) = &c.testigo_firma_path {
        q = q.bind(v);
    }
    q.bind(id).execute(pool).await?;

    let f = fila_visible(pool, ambito, id)
        .await?
        .expect("estaba visible hace una línea");
    Ok(Ok(a_lectura(pool, f).await?))
}

/// Registra que el paciente retiró su consentimiento.
///
/// **No comprueba el estado previo, y eso no es un olvido.** El derecho a
/// revocar no depende de en qué punto esté el trámite: un consentimiento ya
/// revocado se puede volver a revocar, uno inválido también, y uno de hace tres
/// años igual. Lo único que se exige —y lo exige el dominio, no esta función—
/// es que haya constancia de lo que el paciente manifestó.
///
/// El único `Estorbo` que puede devolver es `NoExiste`, que aquí significa
/// «no es de esta institución»: eso no es una condición sobre la revocación,
/// es que el consentimiento no es suyo para revocarlo.
pub async fn revocar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    motivo: &str,
) -> Result<Result<Lectura, Estorbo>, sqlx::Error> {
    if fila_visible(pool, ambito, id).await?.is_none() {
        return Ok(Err(Estorbo::NoExiste));
    }
    sqlx::query(
        "UPDATE consentimiento_informado \
         SET revocado = 1, fecha_revocacion = ?, motivo_revocacion = ? WHERE id = ?",
    )
    .bind(crate::ahora())
    .bind(motivo)
    .bind(id)
    .execute(pool)
    .await?;

    let f = fila_visible(pool, ambito, id)
        .await?
        .expect("estaba visible hace una línea");
    Ok(Ok(a_lectura(pool, f).await?))
}
