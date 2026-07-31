//! Laboratorio e imágenes diagnósticas: lo que responde a una orden médica.
//!
//! Los dos servicios comparten la mitad de arriba —una orden, un estado, una
//! bandeja— y se separan en la de abajo: el laboratorio produce **parámetros con
//! sus referencias**, la imagenología produce **un informe redactado**. Por eso
//! viven en un módulo con dos secciones y no en dos módulos que se copian el
//! listado de órdenes.
//!
//! Aislamiento: nada de aquí lleva `institucion_id`. Todo cuelga de la cadena
//! `orden → historia → paciente → institución`, que es la del resto del clínico.
//! Un resultado con dueño propio podría discrepar de la orden que lo pidió.
//!
//! # La regla que ordena el módulo: cerrado es cerrado
//!
//! Un resultado **validado** y un informe **firmado** no se editan. Es la misma
//! inmutabilidad de la historia y del documento clínico, y aquí pesa más: sobre
//! un resultado de laboratorio un médico ya decidió una conducta. Reescribirlo
//! deja la decisión apuntando a un dato que ya no existe. Se corrige emitiendo
//! otro y diciendo que corrige al primero, no borrando el rastro.

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Cuál de los dos servicios diagnósticos.
///
/// Existe para que el tipo de orden no se escriba a mano en cada consulta: la
/// cadena «Laboratorio» tiene que ser EXACTAMENTE la que crea la historia
/// clínica (`historias.component.ts:1226`) y la que admite el CHECK de la 0027,
/// y una errata aquí produciría una bandeja vacía sin ningún error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Servicio {
    Laboratorio,
    Imagenologia,
}

impl Servicio {
    pub fn tipo_orden(self) -> &'static str {
        match self {
            Servicio::Laboratorio => "Laboratorio",
            Servicio::Imagenologia => "Imagenología",
        }
    }

    /// La tabla donde vive su respuesta. Se interpola en el SQL, y puede hacerse
    /// porque sale de esta enumeración cerrada: no hay camino desde una entrada
    /// del usuario hasta aquí.
    fn tabla_resultado(self) -> &'static str {
        match self {
            Servicio::Laboratorio => "resultado_laboratorio",
            Servicio::Imagenologia => "informe_imagen",
        }
    }
}

/// Los cuatro estados de una orden, tal como los escribe el producto.
pub const ESTADOS: [&str; 4] = ["Pendiente", "En Proceso", "Completada", "Cancelada"];

/// Una orden vista desde la bandeja del servicio, con paciente y médico ya
/// resueltos y con la respuesta a «¿ya tiene resultado?».
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct OrdenDx {
    pub id: i64,
    pub historia_id: i64,
    pub paciente_id: Option<i64>,
    pub medico_ordena_id: Option<i64>,
    pub fecha_orden: String,
    pub estado: String,
    pub tipo: String,
    pub codigo_cups: Option<String>,
    pub descripcion: String,
    pub justificacion: String,
    pub paciente_nombre: Option<String>,
    pub paciente_identificacion: Option<String>,
    pub medico_nombre: Option<String>,
    pub tiene_resultado: bool,
}

/// Qué pasó al escribir sobre algo que puede estar cerrado.
#[derive(Debug, Clone, PartialEq)]
pub enum Escritura<T> {
    Hecho(T),
    /// No existe, no es del ámbito, o no es una orden de este servicio. Las tres
    /// dan lo mismo: distinguirlas diría qué hay en la institución de al lado.
    NoExiste,
    /// Validado o firmado. No se edita, se emite otro.
    Cerrado,
    /// Un estado que no es ninguno de los cuatro del producto.
    EstadoInvalido,
}

fn filtro_ambito(ambito: Ambito, col: &str) -> Option<(String, Option<i64>)> {
    match ambito {
        Ambito::Superadmin => Some(("1 = 1".into(), None)),
        Ambito::Institucion(i) => Some((format!("{col} = ?"), Some(i))),
        Ambito::Ninguno => None,
    }
}

fn ahora() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn columnas_orden(servicio: Servicio) -> String {
    let tabla = servicio.tabla_resultado();
    format!(
        "o.id, o.historia_id, p.id AS paciente_id, h.medico_id AS medico_ordena_id, \
         o.fecha_orden, o.estado, o.tipo, o.codigo_cups, o.descripcion, o.justificacion, \
         (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente_nombre, \
         p.numero_documento AS paciente_identificacion, u.nombre AS medico_nombre, \
         EXISTS (SELECT 1 FROM {tabla} r WHERE r.orden_id = o.id) AS tiene_resultado"
    )
}

const DESDE_ORDEN: &str = "FROM orden o \
    JOIN historia h ON h.id = o.historia_id \
    JOIN paciente p ON p.id = h.paciente_id \
    LEFT JOIN usuario u ON u.id = h.medico_id";

/// La bandeja del servicio: las órdenes de su tipo dentro del ámbito.
pub async fn listar_ordenes(
    pool: &crate::Pool,
    ambito: Ambito,
    servicio: Servicio,
    estado: Option<&str>,
    paciente_id: Option<i64>,
) -> Result<Vec<OrdenDx>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT {} {DESDE_ORDEN} WHERE o.tipo = ? AND {filtro}",
        columnas_orden(servicio)
    );
    if estado.is_some() {
        sql.push_str(" AND o.estado = ?");
    }
    if paciente_id.is_some() {
        sql.push_str(" AND p.id = ?");
    }
    // Lo pendiente primero y, dentro, lo más antiguo: quien lleva más esperando
    // un examen se atiende antes. Ordenar por fecha descendente —lo natural en
    // un listado— pondría arriba justo lo que menos corre prisa.
    sql.push_str(
        " ORDER BY CASE o.estado WHEN 'Pendiente' THEN 0 WHEN 'En Proceso' THEN 1 ELSE 2 END, \
         o.fecha_orden, o.id",
    );

    let mut q = sqlx::query_as::<_, OrdenDx>(&sql).bind(servicio.tipo_orden());
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(e) = estado {
        q = q.bind(e.to_string());
    }
    if let Some(p) = paciente_id {
        q = q.bind(p);
    }
    q.fetch_all(pool).await
}

pub async fn obtener_orden(
    pool: &crate::Pool,
    ambito: Ambito,
    servicio: Servicio,
    id: i64,
) -> Result<Option<OrdenDx>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {} {DESDE_ORDEN} WHERE o.id = ? AND o.tipo = ? AND {filtro}",
        columnas_orden(servicio)
    );
    let mut q = sqlx::query_as::<_, OrdenDx>(&sql).bind(id).bind(servicio.tipo_orden());
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Cambia el estado de una orden del servicio. Rechaza un estado que no sea uno
/// de los cuatro **antes** de tocar la base: el CHECK del esquema lo pararía
/// igual, pero devolviendo un error de SQL en vez de un motivo.
pub async fn cambiar_estado_orden(
    pool: &crate::Pool,
    ambito: Ambito,
    servicio: Servicio,
    id: i64,
    estado: &str,
) -> Result<Escritura<OrdenDx>, sqlx::Error> {
    if !ESTADOS.contains(&estado) {
        return Ok(Escritura::EstadoInvalido);
    }
    if obtener_orden(pool, ambito, servicio, id).await?.is_none() {
        return Ok(Escritura::NoExiste);
    }
    sqlx::query("UPDATE orden SET estado = ? WHERE id = ?")
        .bind(estado)
        .bind(id)
        .execute(pool)
        .await?;
    let orden = obtener_orden(pool, ambito, servicio, id).await?.expect("acaba de existir");
    Ok(Escritura::Hecho(orden))
}

/// Paciente e historia de una orden del servicio, si es del ámbito.
async fn paciente_de_orden(
    pool: &crate::Pool,
    ambito: Ambito,
    servicio: Servicio,
    orden_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    Ok(obtener_orden(pool, ambito, servicio, orden_id).await?.and_then(|o| o.paciente_id))
}

// =============================================================================
// LABORATORIO
// =============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct ResultadoLab {
    pub id: i64,
    pub orden_id: i64,
    pub paciente_id: i64,
    pub bioanalista_id: i64,
    pub bioanalista_nombre: Option<String>,
    pub fecha_resultado: String,
    /// JSON tal como lo guardó quien procesó. Se devuelve como texto y lo
    /// interpreta el cliente; el servidor no reordena ni completa parámetros.
    pub resultados_json: Value,
    pub observaciones: Option<String>,
    pub valores_criticos: bool,
    pub validado: bool,
    pub fecha_validacion: Option<String>,
    pub validado_por_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResultadoNuevo {
    #[serde(default)]
    pub resultados_json: Value,
    pub observaciones: Option<String>,
    #[serde(default)]
    pub valores_criticos: bool,
}

const COLS_LAB: &str = "r.id, r.orden_id, r.paciente_id, r.bioanalista_id, \
    u.nombre AS bioanalista_nombre, r.fecha_resultado, r.resultados_json, r.observaciones, \
    r.valores_criticos, r.validado, r.fecha_validacion, r.validado_por_id";

/// Decodifica la fila cruda (con `resultados_json` como TEXT) al tipo público.
///
/// Va a mano y no por `sqlx::types::Json` porque un JSON que se guardó mal
/// —cosa que la base no impide— reventaría la consulta entera y con ella la
/// bandeja; aquí se degrada a `null` en ESE resultado y el resto se sirve.
#[derive(sqlx::FromRow)]
struct FilaLab {
    id: i64,
    orden_id: i64,
    paciente_id: i64,
    bioanalista_id: i64,
    bioanalista_nombre: Option<String>,
    fecha_resultado: String,
    resultados_json: String,
    observaciones: Option<String>,
    valores_criticos: bool,
    validado: bool,
    fecha_validacion: Option<String>,
    validado_por_id: Option<i64>,
}

impl From<FilaLab> for ResultadoLab {
    fn from(f: FilaLab) -> Self {
        ResultadoLab {
            id: f.id,
            orden_id: f.orden_id,
            paciente_id: f.paciente_id,
            bioanalista_id: f.bioanalista_id,
            bioanalista_nombre: f.bioanalista_nombre,
            fecha_resultado: f.fecha_resultado,
            resultados_json: serde_json::from_str(&f.resultados_json).unwrap_or(Value::Null),
            observaciones: f.observaciones,
            valores_criticos: f.valores_criticos,
            validado: f.validado,
            fecha_validacion: f.fecha_validacion,
            validado_por_id: f.validado_por_id,
        }
    }
}

/// ¿Hay valores críticos?
///
/// La respuesta NO es solo la casilla que marcó quien procesa: también lo es
/// cualquier parámetro que venga con `"critico": true`. Se toma la disyunción
/// porque las dos afirman lo mismo y una de ellas se olvida — y un potasio de
/// 7,2 que no enciende la alerta porque nadie marcó la casilla es el fallo que
/// cuesta un paciente, no una molestia de interfaz.
///
/// Al revés no: marcar la casilla sin ningún parámetro crítico SÍ vale. Quien
/// procesa puede ver algo que la referencia no captura, y el sistema no está
/// para contradecirlo.
pub fn valores_criticos_de(declarado: bool, resultados: &Value) -> bool {
    if declarado {
        return true;
    }
    let Some(mapa) = resultados.as_object() else {
        return false;
    };
    mapa.values()
        .any(|p| p.get("critico").and_then(Value::as_bool).unwrap_or(false))
}

pub async fn resultado_de_orden(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
) -> Result<Option<ResultadoLab>, sqlx::Error> {
    if paciente_de_orden(pool, ambito, Servicio::Laboratorio, orden_id).await?.is_none() {
        return Ok(None);
    }
    let sql = format!(
        "SELECT {COLS_LAB} FROM resultado_laboratorio r \
         LEFT JOIN usuario u ON u.id = r.bioanalista_id WHERE r.orden_id = ?"
    );
    Ok(sqlx::query_as::<_, FilaLab>(&sql)
        .bind(orden_id)
        .fetch_optional(pool)
        .await?
        .map(Into::into))
}

/// Registra el resultado y pasa la orden a «En Proceso» si estaba pendiente.
///
/// Si ya hay resultado, es una edición: se delega en [`actualizar_resultado`],
/// que respeta la validación. Así la interfaz puede reintentar sin que un doble
/// clic produzca dos resultados ni un 500 por el UNIQUE.
pub async fn crear_resultado(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    bioanalista_id: i64,
    r: &ResultadoNuevo,
) -> Result<Escritura<ResultadoLab>, sqlx::Error> {
    let Some(paciente_id) = paciente_de_orden(pool, ambito, Servicio::Laboratorio, orden_id).await?
    else {
        return Ok(Escritura::NoExiste);
    };
    if resultado_de_orden(pool, ambito, orden_id).await?.is_some() {
        return actualizar_resultado(pool, ambito, orden_id, r).await;
    }
    let criticos = valores_criticos_de(r.valores_criticos, &r.resultados_json);
    sqlx::query(
        "INSERT INTO resultado_laboratorio \
         (orden_id, paciente_id, bioanalista_id, fecha_resultado, resultados_json, \
          observaciones, valores_criticos) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(orden_id)
    .bind(paciente_id)
    .bind(bioanalista_id)
    .bind(ahora())
    .bind(r.resultados_json.to_string())
    .bind(&r.observaciones)
    .bind(criticos)
    .execute(pool)
    .await?;
    // La orden pasa a «En Proceso», no a «Completada»: falta validarlo. Darla
    // por terminada aquí haría que un resultado sin revisar apareciera cerrado.
    sqlx::query("UPDATE orden SET estado = 'En Proceso' WHERE id = ? AND estado = 'Pendiente'")
        .bind(orden_id)
        .execute(pool)
        .await?;
    let creado = resultado_de_orden(pool, ambito, orden_id).await?.expect("acaba de insertarse");
    Ok(Escritura::Hecho(creado))
}

pub async fn actualizar_resultado(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    r: &ResultadoNuevo,
) -> Result<Escritura<ResultadoLab>, sqlx::Error> {
    let Some(actual) = resultado_de_orden(pool, ambito, orden_id).await? else {
        return Ok(Escritura::NoExiste);
    };
    if actual.validado {
        return Ok(Escritura::Cerrado);
    }
    let criticos = valores_criticos_de(r.valores_criticos, &r.resultados_json);
    sqlx::query(
        "UPDATE resultado_laboratorio SET resultados_json = ?, observaciones = ?, \
         valores_criticos = ? WHERE orden_id = ? AND validado = 0",
    )
    .bind(r.resultados_json.to_string())
    .bind(&r.observaciones)
    .bind(criticos)
    .bind(orden_id)
    .execute(pool)
    .await?;
    let actualizado = resultado_de_orden(pool, ambito, orden_id).await?.expect("existe");
    Ok(Escritura::Hecho(actualizado))
}

/// Valida el resultado y cierra la orden. Desde aquí ya no se edita.
///
/// `validado_por_id` queda registrado: la validación afirma que alguien revisó
/// esos números, y una afirmación sin autor no acredita nada.
pub async fn validar_resultado(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    usuario_id: i64,
) -> Result<Escritura<ResultadoLab>, sqlx::Error> {
    let Some(actual) = resultado_de_orden(pool, ambito, orden_id).await? else {
        return Ok(Escritura::NoExiste);
    };
    if actual.validado {
        return Ok(Escritura::Cerrado);
    }
    let mut tx = pool.begin().await?;
    // El `AND validado = 0` es lo que decide, y hay que MIRAR si afectó alguna
    // fila. Sin esta comprobación, cien validaciones concurrentes «ganaban» las
    // cien: cada una leía `validado = 0` antes de que la primera escribiera, el
    // UPDATE no afectaba nada y todas devolvían `Hecho` con la fecha y el autor
    // de otro. La prueba de concurrencia lo cazó con 96 ganadoras de 100.
    let marcado = sqlx::query(
        "UPDATE resultado_laboratorio SET validado = 1, fecha_validacion = ?, \
         validado_por_id = ? WHERE orden_id = ? AND validado = 0",
    )
    .bind(ahora())
    .bind(usuario_id)
    .bind(orden_id)
    .execute(&mut *tx)
    .await?;
    if marcado.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(Escritura::Cerrado);
    }
    sqlx::query("UPDATE orden SET estado = 'Completada' WHERE id = ?")
        .bind(orden_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let validado = resultado_de_orden(pool, ambito, orden_id).await?.expect("existe");
    Ok(Escritura::Hecho(validado))
}

/// Historial de laboratorio de un paciente, del más reciente al más antiguo.
pub async fn resultados_de_paciente(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<Vec<ResultadoLab>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT {COLS_LAB} FROM resultado_laboratorio r \
         JOIN paciente p ON p.id = r.paciente_id \
         LEFT JOIN usuario u ON u.id = r.bioanalista_id \
         WHERE r.paciente_id = ? AND {filtro} ORDER BY r.fecha_resultado DESC, r.id DESC"
    );
    let mut q = sqlx::query_as::<_, FilaLab>(&sql).bind(paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_all(pool).await?.into_iter().map(Into::into).collect())
}

// --- Perfiles de laboratorio --------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Perfil {
    pub id: i64,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub examenes: Value,
    pub activo: bool,
}

#[derive(sqlx::FromRow)]
struct FilaPerfil {
    id: i64,
    nombre: String,
    descripcion: Option<String>,
    examenes: String,
    activo: bool,
}

impl From<FilaPerfil> for Perfil {
    fn from(f: FilaPerfil) -> Self {
        Perfil {
            id: f.id,
            nombre: f.nombre,
            descripcion: f.descripcion,
            examenes: serde_json::from_str(&f.examenes).unwrap_or(Value::Null),
            activo: f.activo,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PerfilNuevo {
    pub nombre: String,
    pub descripcion: Option<String>,
    #[serde(default)]
    pub examenes: Value,
}

/// Qué pasó al dar de alta un perfil.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoPerfil {
    Creado(Box<Perfil>),
    NoExiste,
    SinNombre,
    NombreRepetido(String),
}

pub async fn listar_perfiles(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Perfil>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT id, nombre, descripcion, examenes, activo FROM perfil_laboratorio \
         WHERE activo = 1 AND {filtro} ORDER BY nombre"
    );
    let mut q = sqlx::query_as::<_, FilaPerfil>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_all(pool).await?.into_iter().map(Into::into).collect())
}

pub async fn crear_perfil(
    pool: &crate::Pool,
    ambito: Ambito,
    institucion_id: i64,
    p: &PerfilNuevo,
) -> Result<ResultadoPerfil, sqlx::Error> {
    if ambito.puede_ver(Some(institucion_id)) != Visibilidad::Si {
        return Ok(ResultadoPerfil::NoExiste);
    }
    if p.nombre.trim().is_empty() {
        return Ok(ResultadoPerfil::SinNombre);
    }
    let r = sqlx::query(
        "INSERT INTO perfil_laboratorio (institucion_id, nombre, descripcion, examenes) \
         VALUES (?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(p.nombre.trim())
    .bind(&p.descripcion)
    .bind(p.examenes.to_string())
    .execute(pool)
    .await;
    match r {
        Ok(r) => {
            let id = r.last_insert_rowid();
            let sql = "SELECT id, nombre, descripcion, examenes, activo FROM perfil_laboratorio WHERE id = ?";
            let fila = sqlx::query_as::<_, FilaPerfil>(sql).bind(id).fetch_one(pool).await?;
            Ok(ResultadoPerfil::Creado(Box::new(fila.into())))
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Ok(ResultadoPerfil::NombreRepetido(p.nombre.trim().to_owned()))
        }
        Err(e) => Err(e),
    }
}

/// Retira un perfil. **Se desactiva, no se borra**: los resultados que ya se
/// capturaron con él siguen citando sus parámetros y sus referencias, y una fila
/// borrada dejaría un hemograma antiguo sin poder explicar de dónde salió el
/// rango que se usó para leerlo.
pub async fn eliminar_perfil(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    let sql = format!("UPDATE perfil_laboratorio SET activo = 0 WHERE id = ? AND activo = 1 AND {filtro}");
    let mut q = sqlx::query(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

// =============================================================================
// IMAGENOLOGÍA
// =============================================================================

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InformeImagen {
    pub id: i64,
    pub orden_id: i64,
    pub paciente_id: i64,
    pub radiologo_id: i64,
    pub radiologo_nombre: Option<String>,
    pub fecha_resultado: String,
    pub tecnica: String,
    pub hallazgos: String,
    pub impresion_diagnostica: String,
    pub recomendaciones: Option<String>,
    pub clasificacion: Option<String>,
    pub imagenes_paths: Vec<String>,
    pub firmado: bool,
    pub fecha_firma: Option<String>,
    pub firmado_por_id: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct FilaInforme {
    id: i64,
    orden_id: i64,
    paciente_id: i64,
    radiologo_id: i64,
    radiologo_nombre: Option<String>,
    fecha_resultado: String,
    tecnica: String,
    hallazgos: String,
    impresion_diagnostica: String,
    recomendaciones: Option<String>,
    clasificacion: Option<String>,
    imagenes_paths: String,
    firmado: bool,
    fecha_firma: Option<String>,
    firmado_por_id: Option<i64>,
}

impl From<FilaInforme> for InformeImagen {
    fn from(f: FilaInforme) -> Self {
        InformeImagen {
            id: f.id,
            orden_id: f.orden_id,
            paciente_id: f.paciente_id,
            radiologo_id: f.radiologo_id,
            radiologo_nombre: f.radiologo_nombre,
            fecha_resultado: f.fecha_resultado,
            tecnica: f.tecnica,
            hallazgos: f.hallazgos,
            impresion_diagnostica: f.impresion_diagnostica,
            recomendaciones: f.recomendaciones,
            clasificacion: f.clasificacion,
            imagenes_paths: serde_json::from_str(&f.imagenes_paths).unwrap_or_default(),
            firmado: f.firmado,
            fecha_firma: f.fecha_firma,
            firmado_por_id: f.firmado_por_id,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InformeNuevo {
    #[serde(default)]
    pub tecnica: String,
    #[serde(default)]
    pub hallazgos: String,
    #[serde(default)]
    pub impresion_diagnostica: String,
    pub recomendaciones: Option<String>,
    pub clasificacion: Option<String>,
}

const COLS_INF: &str = "i.id, i.orden_id, i.paciente_id, i.radiologo_id, \
    u.nombre AS radiologo_nombre, i.fecha_resultado, i.tecnica, i.hallazgos, \
    i.impresion_diagnostica, i.recomendaciones, i.clasificacion, i.imagenes_paths, \
    i.firmado, i.fecha_firma, i.firmado_por_id";

pub async fn informe_de_orden(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
) -> Result<Option<InformeImagen>, sqlx::Error> {
    if paciente_de_orden(pool, ambito, Servicio::Imagenologia, orden_id).await?.is_none() {
        return Ok(None);
    }
    let sql = format!(
        "SELECT {COLS_INF} FROM informe_imagen i \
         LEFT JOIN usuario u ON u.id = i.radiologo_id WHERE i.orden_id = ?"
    );
    Ok(sqlx::query_as::<_, FilaInforme>(&sql)
        .bind(orden_id)
        .fetch_optional(pool)
        .await?
        .map(Into::into))
}

pub async fn crear_informe(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    radiologo_id: i64,
    i: &InformeNuevo,
) -> Result<Escritura<InformeImagen>, sqlx::Error> {
    let Some(paciente_id) = paciente_de_orden(pool, ambito, Servicio::Imagenologia, orden_id).await?
    else {
        return Ok(Escritura::NoExiste);
    };
    if informe_de_orden(pool, ambito, orden_id).await?.is_some() {
        return actualizar_informe(pool, ambito, orden_id, i).await;
    }
    sqlx::query(
        "INSERT INTO informe_imagen \
         (orden_id, paciente_id, radiologo_id, fecha_resultado, tecnica, hallazgos, \
          impresion_diagnostica, recomendaciones, clasificacion) VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(orden_id)
    .bind(paciente_id)
    .bind(radiologo_id)
    .bind(ahora())
    .bind(&i.tecnica)
    .bind(&i.hallazgos)
    .bind(&i.impresion_diagnostica)
    .bind(&i.recomendaciones)
    .bind(&i.clasificacion)
    .execute(pool)
    .await?;
    sqlx::query("UPDATE orden SET estado = 'En Proceso' WHERE id = ? AND estado = 'Pendiente'")
        .bind(orden_id)
        .execute(pool)
        .await?;
    let creado = informe_de_orden(pool, ambito, orden_id).await?.expect("acaba de insertarse");
    Ok(Escritura::Hecho(creado))
}

/// Edita el borrador. Se niega sobre uno firmado.
///
/// `COALESCE` no vale aquí: los tres campos del informe son texto obligatorio y
/// un borrado deliberado («esto no era un hallazgo») tiene que poder guardarse
/// vacío. Lo que llega es el informe entero, como lo manda la pantalla.
pub async fn actualizar_informe(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    i: &InformeNuevo,
) -> Result<Escritura<InformeImagen>, sqlx::Error> {
    let Some(actual) = informe_de_orden(pool, ambito, orden_id).await? else {
        return Ok(Escritura::NoExiste);
    };
    if actual.firmado {
        return Ok(Escritura::Cerrado);
    }
    sqlx::query(
        "UPDATE informe_imagen SET tecnica = ?, hallazgos = ?, impresion_diagnostica = ?, \
         recomendaciones = ?, clasificacion = ? WHERE orden_id = ? AND firmado = 0",
    )
    .bind(&i.tecnica)
    .bind(&i.hallazgos)
    .bind(&i.impresion_diagnostica)
    .bind(&i.recomendaciones)
    .bind(&i.clasificacion)
    .bind(orden_id)
    .execute(pool)
    .await?;
    let actualizado = informe_de_orden(pool, ambito, orden_id).await?.expect("existe");
    Ok(Escritura::Hecho(actualizado))
}

/// Firma el informe y cierra la orden. Desde aquí es de solo lectura.
pub async fn firmar_informe(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    usuario_id: i64,
) -> Result<Escritura<InformeImagen>, sqlx::Error> {
    let Some(actual) = informe_de_orden(pool, ambito, orden_id).await? else {
        return Ok(Escritura::NoExiste);
    };
    if actual.firmado {
        return Ok(Escritura::Cerrado);
    }
    let mut tx = pool.begin().await?;
    // Igual que en la validación del laboratorio: quien no afecta ninguna fila
    // llegó tarde, y decirle que firmó le atribuiría la firma de otro.
    let marcado = sqlx::query(
        "UPDATE informe_imagen SET firmado = 1, fecha_firma = ?, firmado_por_id = ? \
         WHERE orden_id = ? AND firmado = 0",
    )
    .bind(ahora())
    .bind(usuario_id)
    .bind(orden_id)
    .execute(&mut *tx)
    .await?;
    if marcado.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(Escritura::Cerrado);
    }
    sqlx::query("UPDATE orden SET estado = 'Completada' WHERE id = ?")
        .bind(orden_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let firmado = informe_de_orden(pool, ambito, orden_id).await?.expect("existe");
    Ok(Escritura::Hecho(firmado))
}

/// Añade una imagen ya almacenada al informe.
///
/// Recibe la RUTA que sirve esos bytes, no los bytes: quien los guarda es el
/// mismo camino que guarda los adjuntos de la historia —con su comprobación de
/// tipo por firma y su cifrado en reposo—, y duplicarlo aquí habría creado un
/// segundo almacén con otras reglas. Un informe firmado ya no admite más.
pub async fn adjuntar_imagen(
    pool: &crate::Pool,
    ambito: Ambito,
    orden_id: i64,
    ruta: &str,
) -> Result<Escritura<InformeImagen>, sqlx::Error> {
    let Some(actual) = informe_de_orden(pool, ambito, orden_id).await? else {
        return Ok(Escritura::NoExiste);
    };
    if actual.firmado {
        return Ok(Escritura::Cerrado);
    }
    let mut rutas = actual.imagenes_paths.clone();
    if !rutas.iter().any(|r| r == ruta) {
        rutas.push(ruta.to_owned());
    }
    sqlx::query("UPDATE informe_imagen SET imagenes_paths = ? WHERE orden_id = ? AND firmado = 0")
        .bind(serde_json::to_string(&rutas).unwrap_or_else(|_| "[]".into()))
        .bind(orden_id)
        .execute(pool)
        .await?;
    let actualizado = informe_de_orden(pool, ambito, orden_id).await?.expect("existe");
    Ok(Escritura::Hecho(actualizado))
}

pub async fn informes_de_paciente(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<Vec<InformeImagen>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT {COLS_INF} FROM informe_imagen i \
         JOIN paciente p ON p.id = i.paciente_id \
         LEFT JOIN usuario u ON u.id = i.radiologo_id \
         WHERE i.paciente_id = ? AND {filtro} ORDER BY i.fecha_resultado DESC, i.id DESC"
    );
    let mut q = sqlx::query_as::<_, FilaInforme>(&sql).bind(paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_all(pool).await?.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    /// Dos instituciones. En la 1: paciente 10, historia 100, una orden de
    /// laboratorio (1) y una de imágenes (2). En la 2: lo mismo, con la orden 3.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES \
             (5,'Dra Ruiz','d@x.co','h','medico',1,1), \
             (6,'Bioanalista','b@x.co','h','bacteriologo',1,1), \
             (7,'Radióloga','r@x.co','h','radiologo',1,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1017','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2028','Beto','Paz','1985-01-01','M',2)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento) \
             VALUES (100,'HC1',10,5,'m','e','J00','plan'),(200,'HC2',20,5,'m','e','J00','plan')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO orden (id,historia_id,tipo,descripcion,justificacion) VALUES \
             (1,100,'Laboratorio','Hemograma completo','Control'), \
             (2,100,'Imagenología','Radiografía de tórax PA','Tos persistente'), \
             (3,200,'Laboratorio','Glicemia','Control')",
        ).execute(&pool).await.unwrap();
        pool
    }

    fn hemograma() -> ResultadoNuevo {
        ResultadoNuevo {
            resultados_json: json!({
                "hemoglobina": {"valor":"13.5","unidad":"g/dL","ref_min":"12","ref_max":"16","critico":false},
                "leucocitos":  {"valor":"7200","unidad":"/mm3","ref_min":"4500","ref_max":"11000","critico":false}
            }),
            observaciones: Some("Muestra en ayunas".into()),
            valores_criticos: false,
        }
    }

    fn informe() -> InformeNuevo {
        InformeNuevo {
            tecnica: "Proyección PA y lateral".into(),
            hallazgos: "Campos pulmonares sin consolidaciones.".into(),
            impresion_diagnostica: "Estudio dentro de límites normales.".into(),
            recomendaciones: None,
            clasificacion: None,
        }
    }

    /// El defecto de la 0007 era «PENDIENTE»; la interfaz compara con
    /// «Pendiente». Esta prueba es la que habría cazado la pantalla muerta.
    #[tokio::test]
    async fn una_orden_nace_con_el_estado_que_la_interfaz_compara() {
        let pool = base().await;
        let os = listar_ordenes(&pool, Ambito::institucion(1), Servicio::Laboratorio, None, None).await.unwrap();
        assert_eq!(os.len(), 1);
        assert_eq!(os[0].estado, "Pendiente");
        assert_eq!(os[0].tipo, "Laboratorio");
        assert_eq!(os[0].paciente_nombre.as_deref(), Some("Ana Ruiz"));
        assert_eq!(os[0].paciente_identificacion.as_deref(), Some("1017"));
        assert_eq!(os[0].medico_nombre.as_deref(), Some("Dra Ruiz"));
        assert!(!os[0].tiene_resultado);
    }

    /// Cada bandeja ve lo suyo. Sin esto, la de laboratorio mostraría también las
    /// radiografías, que es lo que pasaba listando `orden` sin filtrar por tipo.
    #[tokio::test]
    async fn cada_servicio_ve_solo_sus_ordenes() {
        let pool = base().await;
        let lab = listar_ordenes(&pool, Ambito::institucion(1), Servicio::Laboratorio, None, None).await.unwrap();
        let img = listar_ordenes(&pool, Ambito::institucion(1), Servicio::Imagenologia, None, None).await.unwrap();
        assert_eq!(lab.len(), 1);
        assert_eq!(lab[0].id, 1);
        assert_eq!(img.len(), 1);
        assert_eq!(img[0].id, 2);
        // Y una orden de laboratorio no se alcanza pidiéndola como de imágenes.
        assert!(obtener_orden(&pool, Ambito::institucion(1), Servicio::Imagenologia, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn la_orden_ajena_es_indistinguible_de_la_inexistente() {
        let pool = base().await;
        assert!(obtener_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 3).await.unwrap().is_none());
        assert_eq!(
            crear_resultado(&pool, Ambito::institucion(1), 3, 6, &hemograma()).await.unwrap(),
            Escritura::NoExiste
        );
        assert!(resultado_de_orden(&pool, Ambito::institucion(1), 3).await.unwrap().is_none());
        assert!(resultados_de_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().is_empty());
        assert!(listar_ordenes(&pool, Ambito::Ninguno, Servicio::Laboratorio, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn el_resultado_mueve_la_orden_a_en_proceso_y_la_validacion_la_completa() {
        let pool = base().await;
        let Escritura::Hecho(r) = crear_resultado(&pool, Ambito::institucion(1), 1, 6, &hemograma()).await.unwrap()
        else { panic!("debía crearse") };
        assert_eq!(r.paciente_id, 10);
        assert_eq!(r.bioanalista_nombre.as_deref(), Some("Bioanalista"));
        assert!(!r.validado);
        assert_eq!(r.resultados_json["hemoglobina"]["valor"], "13.5");
        let orden = obtener_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 1).await.unwrap().unwrap();
        assert_eq!(orden.estado, "En Proceso", "un resultado sin validar NO cierra la orden");
        assert!(orden.tiene_resultado);

        let Escritura::Hecho(v) = validar_resultado(&pool, Ambito::institucion(1), 1, 5).await.unwrap()
        else { panic!("debía validarse") };
        assert!(v.validado);
        assert_eq!(v.validado_por_id, Some(5), "la validación deja QUIÉN validó");
        assert!(v.fecha_validacion.is_some());
        assert_eq!(
            obtener_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 1).await.unwrap().unwrap().estado,
            "Completada"
        );
    }

    /// Validado es cerrado. Un resultado sobre el que un médico ya decidió no se
    /// reescribe.
    #[tokio::test]
    async fn un_resultado_validado_no_se_edita_ni_se_revalida() {
        let pool = base().await;
        crear_resultado(&pool, Ambito::institucion(1), 1, 6, &hemograma()).await.unwrap();
        validar_resultado(&pool, Ambito::institucion(1), 1, 5).await.unwrap();

        let mut otro = hemograma();
        otro.resultados_json = json!({"hemoglobina": {"valor":"6.0","unidad":"g/dL","critico":true}});
        assert_eq!(
            actualizar_resultado(&pool, Ambito::institucion(1), 1, &otro).await.unwrap(),
            Escritura::Cerrado
        );
        assert_eq!(
            crear_resultado(&pool, Ambito::institucion(1), 1, 6, &otro).await.unwrap(),
            Escritura::Cerrado,
            "reenviar el alta tampoco puede pisar lo validado"
        );
        assert_eq!(
            validar_resultado(&pool, Ambito::institucion(1), 1, 5).await.unwrap(),
            Escritura::Cerrado
        );
        // Y el valor sigue siendo el original.
        let r = resultado_de_orden(&pool, Ambito::institucion(1), 1).await.unwrap().unwrap();
        assert_eq!(r.resultados_json["hemoglobina"]["valor"], "13.5");
    }

    /// Antes de validar SÍ se edita, y volver a enviar el alta actualiza en vez
    /// de romper por el UNIQUE. Es lo que hace un doble clic en la pantalla.
    #[tokio::test]
    async fn el_borrador_se_edita_y_el_reenvio_no_duplica() {
        let pool = base().await;
        crear_resultado(&pool, Ambito::institucion(1), 1, 6, &hemograma()).await.unwrap();
        let mut corregido = hemograma();
        corregido.observaciones = Some("Se repite la muestra".into());
        assert!(matches!(
            crear_resultado(&pool, Ambito::institucion(1), 1, 6, &corregido).await.unwrap(),
            Escritura::Hecho(_)
        ));
        let cuantos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resultado_laboratorio WHERE orden_id = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(cuantos, 1);
        let r = resultado_de_orden(&pool, Ambito::institucion(1), 1).await.unwrap().unwrap();
        assert_eq!(r.observaciones.as_deref(), Some("Se repite la muestra"));
    }

    /// Un parámetro crítico enciende la alerta AUNQUE nadie marque la casilla.
    /// Y discrimina: sin parámetros críticos y sin casilla, no se enciende.
    #[tokio::test]
    async fn los_valores_criticos_no_dependen_de_que_alguien_se_acuerde() {
        assert!(!valores_criticos_de(false, &json!({"k": {"valor":"4.1","critico":false}})));
        assert!(valores_criticos_de(false, &json!({"k": {"valor":"7.2","critico":true}})));
        assert!(valores_criticos_de(true, &json!({"k": {"valor":"4.1","critico":false}})),
                "quien procesa puede ver algo que la referencia no captura");
        assert!(!valores_criticos_de(false, &json!({})));
        assert!(!valores_criticos_de(false, &Value::Null));

        // Y de punta a punta, que es donde importa.
        let pool = base().await;
        let mut r = hemograma();
        r.resultados_json = json!({"potasio": {"valor":"7.2","unidad":"mEq/L","critico":true}});
        r.valores_criticos = false; // la casilla, sin marcar
        let Escritura::Hecho(guardado) = crear_resultado(&pool, Ambito::institucion(1), 1, 6, &r).await.unwrap()
        else { panic!() };
        assert!(guardado.valores_criticos, "un potasio de 7,2 no puede pasar por normal");
    }

    #[tokio::test]
    async fn el_estado_de_la_orden_solo_admite_los_cuatro_del_producto() {
        let pool = base().await;
        assert_eq!(
            cambiar_estado_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 1, "PENDIENTE").await.unwrap(),
            Escritura::EstadoInvalido,
            "la escritura vieja en mayúsculas ya no entra"
        );
        assert_eq!(
            cambiar_estado_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 1, "loquesea").await.unwrap(),
            Escritura::EstadoInvalido
        );
        let Escritura::Hecho(o) = cambiar_estado_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 1, "En Proceso").await.unwrap()
        else { panic!("«En Proceso» sí es válido") };
        assert_eq!(o.estado, "En Proceso");
        // Y sobre una orden ajena, ni con un estado válido.
        assert_eq!(
            cambiar_estado_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 3, "Completada").await.unwrap(),
            Escritura::NoExiste
        );
    }

    #[tokio::test]
    async fn el_informe_se_edita_hasta_firmarlo_y_despues_no() {
        let pool = base().await;
        let Escritura::Hecho(inf) = crear_informe(&pool, Ambito::institucion(1), 2, 7, &informe()).await.unwrap()
        else { panic!("debía crearse") };
        assert_eq!(inf.radiologo_nombre.as_deref(), Some("Radióloga"));
        assert!(!inf.firmado);
        assert!(inf.imagenes_paths.is_empty());
        assert_eq!(
            obtener_orden(&pool, Ambito::institucion(1), Servicio::Imagenologia, 2).await.unwrap().unwrap().estado,
            "En Proceso"
        );

        // Adjuntar una imagen, y que no se duplique al repetir.
        let Escritura::Hecho(con_imagen) = adjuntar_imagen(&pool, Ambito::institucion(1), 2, "/api/imagenes/9/archivo").await.unwrap()
        else { panic!() };
        assert_eq!(con_imagen.imagenes_paths, vec!["/api/imagenes/9/archivo"]);
        let Escritura::Hecho(igual) = adjuntar_imagen(&pool, Ambito::institucion(1), 2, "/api/imagenes/9/archivo").await.unwrap()
        else { panic!() };
        assert_eq!(igual.imagenes_paths.len(), 1);

        let Escritura::Hecho(firmado) = firmar_informe(&pool, Ambito::institucion(1), 2, 7).await.unwrap()
        else { panic!("debía firmarse") };
        assert!(firmado.firmado);
        assert_eq!(firmado.firmado_por_id, Some(7));
        assert_eq!(
            obtener_orden(&pool, Ambito::institucion(1), Servicio::Imagenologia, 2).await.unwrap().unwrap().estado,
            "Completada"
        );

        // Cerrado es cerrado: ni editar, ni adjuntar, ni volver a firmar.
        let mut otro = informe();
        otro.hallazgos = "Masa en lóbulo superior derecho.".into();
        assert_eq!(actualizar_informe(&pool, Ambito::institucion(1), 2, &otro).await.unwrap(), Escritura::Cerrado);
        assert_eq!(crear_informe(&pool, Ambito::institucion(1), 2, 7, &otro).await.unwrap(), Escritura::Cerrado);
        assert_eq!(adjuntar_imagen(&pool, Ambito::institucion(1), 2, "/api/imagenes/10/archivo").await.unwrap(), Escritura::Cerrado);
        assert_eq!(firmar_informe(&pool, Ambito::institucion(1), 2, 7).await.unwrap(), Escritura::Cerrado);
        let sigue = informe_de_orden(&pool, Ambito::institucion(1), 2).await.unwrap().unwrap();
        assert_eq!(sigue.hallazgos, "Campos pulmonares sin consolidaciones.");
        assert_eq!(sigue.imagenes_paths.len(), 1);
    }

    #[tokio::test]
    async fn los_historiales_del_paciente_traen_lo_suyo() {
        let pool = base().await;
        crear_resultado(&pool, Ambito::institucion(1), 1, 6, &hemograma()).await.unwrap();
        crear_informe(&pool, Ambito::institucion(1), 2, 7, &informe()).await.unwrap();
        assert_eq!(resultados_de_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 1);
        assert_eq!(informes_de_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 1);
        // Y el de la otra institución no se alcanza desde aquí.
        assert!(resultados_de_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().is_empty());
        assert!(informes_de_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn los_perfiles_son_de_la_institucion_y_se_retiran_sin_borrarse() {
        let pool = base().await;
        let nuevo = PerfilNuevo {
            nombre: "Hemograma".into(),
            descripcion: Some("Cuadro hemático completo".into()),
            examenes: json!([{"nombre":"Hemoglobina","unidad":"g/dL","ref_min":"12","ref_max":"16"}]),
        };
        let ResultadoPerfil::Creado(p) = crear_perfil(&pool, Ambito::institucion(1), 1, &nuevo).await.unwrap()
        else { panic!("debía crearse") };
        assert_eq!(p.examenes[0]["nombre"], "Hemoglobina");
        // Mismo nombre en la misma institución: no.
        assert_eq!(
            crear_perfil(&pool, Ambito::institucion(1), 1, &nuevo).await.unwrap(),
            ResultadoPerfil::NombreRepetido("Hemograma".into())
        );
        // En otra institución: sí, sus referencias son otras.
        assert!(matches!(crear_perfil(&pool, Ambito::institucion(2), 2, &nuevo).await.unwrap(), ResultadoPerfil::Creado(_)));
        // Sin nombre: no.
        let sin = PerfilNuevo { nombre: "  ".into(), descripcion: None, examenes: json!([]) };
        assert_eq!(crear_perfil(&pool, Ambito::institucion(1), 1, &sin).await.unwrap(), ResultadoPerfil::SinNombre);

        assert_eq!(listar_perfiles(&pool, Ambito::institucion(1)).await.unwrap().len(), 1);
        assert!(listar_perfiles(&pool, Ambito::institucion(2)).await.unwrap().len() == 1);
        // Retirar lo saca de la lista, pero la fila sigue: los resultados
        // capturados con él tienen que poder explicar sus rangos.
        assert!(eliminar_perfil(&pool, Ambito::institucion(1), p.id).await.unwrap());
        assert!(listar_perfiles(&pool, Ambito::institucion(1)).await.unwrap().is_empty());
        let vivas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM perfil_laboratorio WHERE id = ?")
            .bind(p.id).fetch_one(&pool).await.unwrap();
        assert_eq!(vivas, 1);
        // Y el de la institución ajena no se puede retirar.
        assert!(!eliminar_perfil(&pool, Ambito::institucion(2), p.id).await.unwrap());
    }

    /// Directiva 8: 100 intentos de validar el mismo resultado a la vez. Solo uno
    /// puede ganar; los otros 99 tienen que ver «cerrado», no escribir encima ni
    /// dejar la orden a medio camino. Con timeout, para distinguir un
    /// interbloqueo de lentitud.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_validaciones_concurrentes_solo_una_gana() {
        let pool = base().await;
        crear_resultado(&pool, Ambito::institucion(1), 1, 6, &hemograma()).await.unwrap();

        let mut tareas = Vec::new();
        for n in 0..100i64 {
            let p = pool.clone();
            tareas.push(tokio::spawn(async move {
                validar_resultado(&p, Ambito::institucion(1), 1, 5 + (n % 3)).await
            }));
        }
        let plazo = std::time::Duration::from_secs(120);
        let (mut ganadas, mut cerradas) = (0, 0);
        for t in tareas {
            match tokio::time::timeout(plazo, t).await
                .expect("una validación no respondió: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic")
                .expect("ninguna debía fallar por SQL")
            {
                Escritura::Hecho(r) => {
                    assert!(r.validado);
                    ganadas += 1;
                }
                Escritura::Cerrado => cerradas += 1,
                otro => panic!("resultado inesperado: {otro:?}"),
            }
        }
        assert_eq!(ganadas, 1, "validaron {ganadas} veces el mismo resultado");
        assert_eq!(cerradas, 99);
        let r = resultado_de_orden(&pool, Ambito::institucion(1), 1).await.unwrap().unwrap();
        assert!(r.validado);
        assert!(r.validado_por_id.is_some());
        assert_eq!(
            obtener_orden(&pool, Ambito::institucion(1), Servicio::Laboratorio, 1).await.unwrap().unwrap().estado,
            "Completada"
        );
    }
}
