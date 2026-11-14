// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Salas de cirugía y programación quirúrgica.
//!
//! # Lo que este módulo existe para no repetir
//!
//! El listado del lado Python **no acotaba nada**, y su función de enriquecido
//! añade el nombre completo y el DOCUMENTO DE IDENTIDAD del paciente. Medido el
//! 2026-08-06 con dos instituciones: cada admin veía las cirugías de la otra con
//! `paciente='PacienteDOS Secreto' doc='doc2'`. Expone más que la fuga de
//! facturación —allí eran importes; aquí es qué cirugía se le va a practicar a
//! quién— y es categoría especial de la Ley 1581.
//!
//! Y la creación no comprobaba ninguno de los cuatro id del cuerpo: se programaba
//! una cirugía propia nombrando al PACIENTE del vecino, y la respuesta devolvía
//! su nombre y su documento. La clave foránea no protege de eso — es global.
//!
//! # Cómo se acota aquí
//!
//! `programacion_quirurgica` **no lleva `institucion_id`**: cuelga del paciente,
//! igual que la historia. Todo pasa por `JOIN paciente` y [`crate::filtro_ambito`].
//! La sala sí lleva institución propia desde la migración 0047, y **no admite
//! salas sin dueño**: esa tabla nace vacía, así que no hay filas huérfanas que
//! acomodar y la regla «la mía o la de nadie» del Python no hace falta.
//!
//! # 404 y nunca 403
//!
//! Un id ajeno y uno inexistente se responden igual. Un 403 confirmaría que el
//! id existe, y con eso se enumera el padrón del prestador de al lado.

use medico_dominio::quirofano::{transicion, Accion, EfectoEnSala, EstadoCirugia};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

use crate::filtro_ambito;

// ─────────────────────────────────────────────────────────────────────────────
// Tipos
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Sala {
    pub id: i64,
    pub nombre: String,
    pub tipo: String,
    pub estado: String,
    /// Qué tiene la sala. **Sale como OBJETO JSON, no como cadena**: el frontal
    /// lo declara `Record<string, unknown>` y el modelo Python lo emite así
    /// (`Column(JSON)`). Mandarlo como texto le habría dado `"{}"` donde espera
    /// `{}`, y eso no falla: se pinta vacío y nadie se entera.
    ///
    /// Un contenido que no sea JSON válido se sirve como objeto vacío en vez de
    /// tumbar el listado entero. La sala existe y su nombre y su estado —que es
    /// lo que decide si se puede operar— son correctos; perder el listado de
    /// quirófanos por un inventario mal escrito sería cambiar un dato cosmético
    /// roto por una pantalla muerta.
    pub equipamiento: serde_json::Value,
    pub institucion_id: i64,
}

/// La fila tal cual la lee SQLite. `equipamiento` es TEXT en la base.
#[derive(sqlx::FromRow)]
struct FilaSala {
    id: i64,
    nombre: String,
    tipo: String,
    estado: String,
    equipamiento: String,
    institucion_id: i64,
}

impl From<FilaSala> for Sala {
    fn from(f: FilaSala) -> Self {
        Sala {
            id: f.id,
            nombre: f.nombre,
            tipo: f.tipo,
            estado: f.estado,
            equipamiento: serde_json::from_str(&f.equipamiento)
                .unwrap_or_else(|_| serde_json::json!({})),
            institucion_id: f.institucion_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MedicoBasico {
    pub id: i64,
    pub nombre_completo: String,
    pub rol: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Programacion {
    pub id: i64,
    pub quirofano_id: i64,
    pub paciente_id: i64,
    pub cirujano_id: i64,
    pub anestesiologo_id: Option<i64>,
    pub consentimiento_id: Option<i64>,
    pub fecha_programada: String,
    pub duracion_estimada_min: i64,
    pub procedimiento: String,
    pub codigo_cups: Option<String>,
    pub tipo_anestesia: Option<String>,
    pub ayuno_verificado: bool,
    pub labs_prequirurgicos: bool,
    pub valoracion_preanestesica: bool,
    pub hemoclasificacion: Option<String>,
    pub reserva_sangre: bool,
    pub estado: String,
    pub hora_inicio_real: Option<String>,
    /// La hora en que una cirugía SUSPENDIDA se reanudó. Es un dato distinto de
    /// `hora_inicio_real` —el primer inicio— y no lo pisa (directiva 32, #537).
    pub hora_reinicio: Option<String>,
    pub hora_fin_real: Option<String>,
    pub complicaciones: Option<String>,
    pub nota_quirurgica: Option<String>,
    pub hallazgos_quirurgicos: Option<String>,
    pub sangrado_estimado_ml: Option<i64>,
    pub especimenes_patologia: Option<String>,
    // Los cuatro nombres que el frontal pinta. Salen del MISMO `SELECT` que la
    // fila, con sus `JOIN`, y no de cuatro consultas sueltas como en el Python:
    // ahí cada `db.get(...)` iba sin acotar, así que el aislamiento dependía de
    // que el padre estuviera bien filtrado y de que nadie tocara el orden.
    pub paciente_nombre: Option<String>,
    pub paciente_identificacion: Option<String>,
    pub quirofano_nombre: Option<String>,
    pub cirujano_nombre: Option<String>,
    pub anestesiologo_nombre: Option<String>,
}

/// Lo que llega al crear. Los nombres son los de `ProgramacionCreate` del
/// frontal.
#[derive(Debug, Clone, Deserialize)]
pub struct ProgramacionNueva {
    pub quirofano_id: i64,
    pub paciente_id: i64,
    pub cirujano_id: i64,
    pub anestesiologo_id: Option<i64>,
    /// No lo manda el frontal hoy. Se acepta porque el esquema lo tiene y
    /// enlazarlo es clínicamente lo correcto; cuando viene, se comprueba que sea
    /// del MISMO paciente.
    pub consentimiento_id: Option<i64>,
    pub fecha_programada: String,
    #[serde(default = "una_hora")]
    pub duracion_estimada_min: i64,
    pub procedimiento: String,
    pub codigo_cups: Option<String>,
    pub tipo_anestesia: Option<String>,
}

fn una_hora() -> i64 {
    60
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChecklistCambio {
    pub ayuno_verificado: Option<bool>,
    pub labs_prequirurgicos: Option<bool>,
    pub valoracion_preanestesica: Option<bool>,
    /// El grupo sanguíneo. **Una cadena vacía lo BORRA**, y no es un detalle de
    /// forma: con `COALESCE` a secas, un «O+» mal capturado solo se puede
    /// sustituir por otro grupo, nunca vaciar — y en una lista de verificación
    /// prequirúrgica «no lo sé» y «O+ equivocado» no son lo mismo. Lo señaló la
    /// revisión de esta rama.
    pub hemoclasificacion: Option<String>,
    pub reserva_sangre: Option<bool>,
}

/// Lo que se anota al cerrar una cirugía. Todo opcional: la nota operatoria se
/// escribe después, y obligarla aquí dejaría cirugías abiertas.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CierreCirugia {
    pub nota_quirurgica: Option<String>,
    pub hallazgos_quirurgicos: Option<String>,
    pub sangrado_estimado_ml: Option<i64>,
    pub especimenes_patologia: Option<String>,
    pub complicaciones: Option<String>,
    /// Solo para cancelar.
    pub motivo: Option<String>,
}

/// Por qué no se pudo crear. **Cada variante nombra el CAMPO**, no «datos
/// inválidos»: quien captura tiene que saber cuál de los cuatro id está mal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalloAlCrear {
    /// Ese id no existe, o es de otra institución. Las dos cosas se dicen igual.
    NoVisible(&'static str),
    /// La sala existe y es suya, pero no se puede usar.
    SalaNoDisponible(String),
    /// La fecha no es RFC 3339. Se rechaza aquí y no en el `INSERT`: un `CHECK`
    /// reventado sale como 500, y esto es entrada del cliente.
    FechaInvalida(String),
    /// Un dato del cuerpo que el esquema rechazaría. Mismo motivo.
    Invalido(String),
}

impl FalloAlCrear {
    pub fn mensaje(&self) -> String {
        match self {
            FalloAlCrear::NoVisible(que) => format!("{que} no encontrado"),
            FalloAlCrear::SalaNoDisponible(estado) => {
                format!("la sala no está disponible (estado: {estado})")
            }
            FalloAlCrear::FechaInvalida(f) => format!(
                "«{f}» no es una fecha: fecha_programada se escribe en RFC 3339, \
                 como 2026-09-01T13:00:00Z"
            ),
            FalloAlCrear::Invalido(m) => m.clone(),
        }
    }
}

/// El resultado de operar sobre una programación existente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resultado {
    Hecho,
    /// No existe o no es del ámbito.
    NoExiste,
    /// Existe, pero la máquina de estados no permite eso ahora.
    Rechazado(String),
    /// La fila tiene un estado que el dominio no conoce. No se adivina.
    EstadoCorrupto(String),
}

const CAMPOS: &str = "q.id, q.quirofano_id, q.paciente_id, q.cirujano_id, q.anestesiologo_id, \
    q.consentimiento_id, q.fecha_programada, q.duracion_estimada_min, q.procedimiento, \
    q.codigo_cups, q.tipo_anestesia, q.ayuno_verificado, q.labs_prequirurgicos, \
    q.valoracion_preanestesica, q.hemoclasificacion, q.reserva_sangre, q.estado, \
    q.hora_inicio_real, q.hora_reinicio, q.hora_fin_real, q.complicaciones, q.nota_quirurgica, \
    q.hallazgos_quirurgicos, q.sangrado_estimado_ml, q.especimenes_patologia";

const NOMBRE: &str =
    "(p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,''))";

/// El `FROM` con sus enlaces. Va en una constante porque el aislamiento depende
/// de que `paciente` esté SIEMPRE unida: es la tabla que lleva la institución.
const DESDE: &str = "FROM programacion_quirurgica q \
     JOIN paciente  p ON p.id = q.paciente_id \
     JOIN quirofano s ON s.id = q.quirofano_id \
     JOIN usuario   c ON c.id = q.cirujano_id \
     LEFT JOIN usuario a ON a.id = q.anestesiologo_id";

fn seleccion() -> String {
    format!(
        "SELECT {CAMPOS}, {NOMBRE} AS paciente_nombre, p.numero_documento AS paciente_identificacion, \
         s.nombre AS quirofano_nombre, c.nombre AS cirujano_nombre, a.nombre AS anestesiologo_nombre \
         {DESDE}"
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Lectura
// ─────────────────────────────────────────────────────────────────────────────

/// Las salas del ámbito. Vacío para [`Ambito::Ninguno`].
pub async fn listar_salas(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Sala>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT id, nombre, tipo, estado, equipamiento, institucion_id FROM quirofano \
         WHERE {filtro} ORDER BY nombre"
    );
    let mut q = sqlx::query_as::<_, FilaSala>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.fetch_all(pool).await?.into_iter().map(Sala::from).collect())
}

/// Quién puede operar o anestesiar, en el ámbito. Los mismos dos roles que el
/// Python, y `activo = 1`: un profesional dado de baja no se agenda.
pub async fn listar_medicos(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<MedicoBasico>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT id, nombre AS nombre_completo, rol FROM usuario \
         WHERE {filtro} AND activo = 1 AND rol IN ('medico','anestesiologo') \
         ORDER BY nombre"
    );
    let mut q = sqlx::query_as::<_, MedicoBasico>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

pub async fn listar_programaciones(
    pool: &crate::Pool,
    ambito: Ambito,
    estado: Option<&str>,
) -> Result<Vec<Programacion>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!("{} WHERE {filtro}", seleccion());
    if estado.is_some() {
        sql.push_str(" AND q.estado = ?");
    }
    sql.push_str(" ORDER BY q.fecha_programada DESC, q.id DESC");

    let mut c = sqlx::query_as::<_, Programacion>(&sql);
    if let Some(i) = inst {
        c = c.bind(i);
    }
    if let Some(e) = estado {
        c = c.bind(e.to_string());
    }
    c.fetch_all(pool).await
}

pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Programacion>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(None);
    };
    let sql = format!("{} WHERE q.id = ? AND {filtro}", seleccion());
    let mut c = sqlx::query_as::<_, Programacion>(&sql).bind(id);
    if let Some(i) = inst {
        c = c.bind(i);
    }
    c.fetch_optional(pool).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Comprobaciones de pertenencia
// ─────────────────────────────────────────────────────────────────────────────

/// De qué institución es ese paciente, **si quien pregunta puede verlo**. `None`
/// si no existe o es ajeno: las dos cosas se responden igual.
///
/// Devuelve la institución y no un `bool` porque es el dato con el que se
/// comprueba todo lo demás: la sala, el cirujano y el anestesiólogo tienen que
/// ser de la institución DEL PACIENTE.
async fn institucion_del_paciente(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(fila.and_then(|(i,)| (ambito.puede_ver(Some(i)) == Visibilidad::Si).then_some(i)))
}

/// Un profesional del ámbito, activo y con uno de los dos roles que operan.
///
/// **Exige el ROL, y el Python no.** Allí se comprobaba solo la institución, así
/// que se podía nombrar cirujano al regente de farmacia: la programación
/// quedaba con un responsable que no puede serlo, y la nota operatoria acabaría
/// firmada por quien no operó.
async fn profesional_visible(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<bool, sqlx::Error> {
    let fila: Option<(Option<i64>, String, i64)> = sqlx::query_as(
        "SELECT institucion_id, rol, activo FROM usuario WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(inst, rol, activo)| {
        ambito.puede_ver(inst) == Visibilidad::Si
            && activo == 1
            && matches!(rol.as_str(), "medico" | "anestesiologo")
    }))
}

async fn sala_del_ambito(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let fila: Option<(i64, String)> =
        sqlx::query_as("SELECT institucion_id, estado FROM quirofano WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(fila.and_then(|(inst, estado)| {
        (ambito.puede_ver(Some(inst)) == Visibilidad::Si).then_some(estado)
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Escritura
// ─────────────────────────────────────────────────────────────────────────────

/// Programa una cirugía. **Los cuatro id del cuerpo se comprueban primero**, y
/// cada uno responde con su propio nombre para que quien captura sepa cuál está
/// mal — sin decir jamás si existe en otra institución.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &ProgramacionNueva,
) -> Result<Result<i64, FalloAlCrear>, sqlx::Error> {
    // La fecha se PARSEA. El esquema la documenta como RFC 3339 y entraba como
    // texto libre; el `ORDER BY fecha_programada DESC` es lexicográfico, así que
    // un formato distinto reordena el listado en silencio. Es el patrón que este
    // repositorio persigue en otros sitios: el dato que *parece* estar.
    if chrono::DateTime::parse_from_rfc3339(&n.fecha_programada).is_err() {
        return Ok(Err(FalloAlCrear::FechaInvalida(n.fecha_programada.clone())));
    }
    if n.duracion_estimada_min <= 0 {
        return Ok(Err(FalloAlCrear::Invalido(
            "la duración estimada tiene que ser mayor que cero".into(),
        )));
    }
    if let Some(t) = &n.tipo_anestesia
        && !matches!(t.as_str(), "GENERAL" | "REGIONAL" | "LOCAL" | "SEDACION")
    {
        return Ok(Err(FalloAlCrear::Invalido(format!(
            "«{t}» no es un tipo de anestesia: GENERAL, REGIONAL, LOCAL o SEDACION"
        ))));
    }

    // TODO se comprueba contra la institución DEL PACIENTE, no contra la de
    // quien llama, y esa diferencia solo se ve con el superadmin.
    //
    // Para un admin de institución las dos coinciden y el caso normal no cambia.
    // Para el SUPERADMIN, `puede_ver` dice que sí a todo: podía crear una
    // programación con paciente de la institución 1, sala de la 2 y cirujano de
    // la 3, y entonces la institución 1 veía en su listado el nombre de una sala
    // y un cirujano ajenos —y podía cambiar el estado de la sala de la 2 al
    // iniciar—. No es un atacante, es el dueño del sistema; pero una fila
    // incoherente que ningún filtro puede alcanzar no la arregla nadie después.
    // Lo señaló la revisión de esta rama.
    let Some(duenio) = institucion_del_paciente(pool, ambito, n.paciente_id).await? else {
        return Ok(Err(FalloAlCrear::NoVisible("paciente")));
    };
    let suyo = Ambito::Institucion(duenio);
    if !profesional_visible(pool, suyo, n.cirujano_id).await? {
        return Ok(Err(FalloAlCrear::NoVisible("cirujano")));
    }
    if let Some(a) = n.anestesiologo_id
        && !profesional_visible(pool, suyo, a).await?
    {
        return Ok(Err(FalloAlCrear::NoVisible("anestesiólogo")));
    }
    if let Some(c) = n.consentimiento_id {
        let fila: Option<(i64,)> =
            sqlx::query_as("SELECT paciente_id FROM consentimiento_informado WHERE id = ?")
                .bind(c)
                .fetch_optional(pool)
                .await?;
        // EL AJENO Y EL INEXISTENTE DAN LO MISMO, y la primera versión no.
        //
        // Distinguía «no existe» (404) de «es de otro paciente» (400), y eso
        // contradecía la regla que este mismo módulo declara en su cabecera. La
        // revisión de la rama refutó el ataque —el id es una secuencia global y
        // el 400 salía igual para un consentimiento de la propia institución—,
        // pero una regla escrita en absoluto que el código incumple es peor que
        // el oráculo: la próxima vez nadie sabe cuál de las dos manda.
        if !matches!(fila, Some((pac,)) if pac == n.paciente_id) {
            return Ok(Err(FalloAlCrear::NoVisible("consentimiento")));
        }
    }
    let Some(estado_sala) = sala_del_ambito(pool, suyo, n.quirofano_id).await? else {
        return Ok(Err(FalloAlCrear::NoVisible("sala de quirófano")));
    };
    if matches!(estado_sala.as_str(), "MANTENIMIENTO" | "BLOQUEADO") {
        return Ok(Err(FalloAlCrear::SalaNoDisponible(estado_sala)));
    }

    let r = sqlx::query(
        "INSERT INTO programacion_quirurgica \
         (quirofano_id, paciente_id, cirujano_id, anestesiologo_id, consentimiento_id, \
          fecha_programada, duracion_estimada_min, procedimiento, codigo_cups, tipo_anestesia) \
         VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(n.quirofano_id)
    .bind(n.paciente_id)
    .bind(n.cirujano_id)
    .bind(n.anestesiologo_id)
    .bind(n.consentimiento_id)
    .bind(&n.fecha_programada)
    .bind(n.duracion_estimada_min)
    .bind(&n.procedimiento)
    .bind(&n.codigo_cups)
    .bind(&n.tipo_anestesia)
    .execute(pool)
    .await?;

    Ok(Ok(r.last_insert_rowid()))
}

/// La lista de verificación. No cambia el estado de la cirugía: es la
/// preparación, y se corrige tantas veces como haga falta antes de entrar.
///
/// **Una cirugía ya cerrada no admite checklist**: retocar la preparación de
/// algo que ya ocurrió reescribe la historia sin dejar rastro.
pub async fn actualizar_checklist(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &ChecklistCambio,
) -> Result<Resultado, sqlx::Error> {
    let Some(actual) = obtener(pool, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    let Some(estado) = EstadoCirugia::desde_texto(&actual.estado) else {
        return Ok(Resultado::EstadoCorrupto(actual.estado));
    };
    if estado.es_final() {
        return Ok(Resultado::Rechazado(format!(
            "no se puede cambiar la lista de verificación de una cirugía en estado {}",
            estado.como_texto()
        )));
    }
    // `AND estado = ?` con el estado LEÍDO, igual que `mover`. Sin esta cláusula
    // hay una ventana en la que un `completar` concurrente cierra la cirugía y el
    // checklist se escribe igual sobre una COMPLETADA — justo lo que el párrafo
    // de arriba dice impedir. Lo señaló la revisión de esta rama: era el único
    // escritor del módulo sin compare-and-swap.
    let escrito = sqlx::query(
        "UPDATE programacion_quirurgica SET \
         ayuno_verificado = COALESCE(?, ayuno_verificado), \
         labs_prequirurgicos = COALESCE(?, labs_prequirurgicos), \
         valoracion_preanestesica = COALESCE(?, valoracion_preanestesica), \
         hemoclasificacion = CASE WHEN ? THEN ? ELSE hemoclasificacion END, \
         reserva_sangre = COALESCE(?, reserva_sangre) \
         WHERE id = ? AND estado = ?",
    )
    .bind(c.ayuno_verificado)
    .bind(c.labs_prequirurgicos)
    .bind(c.valoracion_preanestesica)
    // Dos binds y no uno: el PRIMERO dice si el campo venía en la petición y el
    // segundo qué escribir. Con `COALESCE` a secas, «vacío» y «ausente» colapsan
    // en el mismo NULL y el valor no se puede borrar nunca.
    .bind(c.hemoclasificacion.is_some())
    .bind(
        c.hemoclasificacion
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(str::to_string),
    )
    .bind(c.reserva_sangre)
    .bind(id)
    .bind(estado.como_texto())
    .execute(pool)
    .await?;
    if escrito.rows_affected() == 0 {
        return Ok(Resultado::Rechazado(
            "la cirugía cambió de estado mientras se procesaba la petición".into(),
        ));
    }
    Ok(Resultado::Hecho)
}

/// Mueve la cirugía por su máquina de estados y, en la MISMA transacción, ajusta
/// la sala.
///
/// Que sea una sola transacción no es prolijidad: si la cirugía pasara a
/// `EN_CURSO` y la sala se quedara `DISPONIBLE`, otra programación entraría en un
/// quirófano ocupado. Y al revés —sala `EN_USO` sin cirugía que la libere— la
/// deja inutilizable hasta que alguien lo arregle a mano.
pub async fn mover(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    accion: Accion,
    datos: &CierreCirugia,
) -> Result<Resultado, sqlx::Error> {
    let Some(actual) = obtener(pool, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    let Some(estado) = EstadoCirugia::desde_texto(&actual.estado) else {
        return Ok(Resultado::EstadoCorrupto(actual.estado));
    };
    let paso = match transicion(estado, accion) {
        Ok(p) => p,
        Err(r) => return Ok(Resultado::Rechazado(r.mensaje())),
    };

    let ahora = crate::ahora();
    let mut tx = pool.begin().await?;

    // El `WHERE estado = ?` con el estado que se leyó es lo que impide que dos
    // peticiones simultáneas apliquen la misma transición dos veces: la segunda
    // no encuentra fila y no escribe nada.
    // El primer inicio y el reinicio de una suspendida son DOS datos (directiva
    // 32, #537). `Iniciar` es el verbo de los dos —`(Programada, Iniciar)` y
    // `(Suspendida, Iniciar)`—, así que se distinguen por el estado de PARTIDA:
    // desde PROGRAMADA fija `hora_inicio_real` (la hora nominal del primer
    // inicio, que mide cuándo empezó la atención de verdad); desde SUSPENDIDA
    // fija `hora_reinicio` y NO toca la primera. Sin esta separación, reanudar
    // pisaba la hora del primer inicio y la marca clínica se perdía sin avisar.
    let es_primer_inicio = accion == Accion::Iniciar && estado == EstadoCirugia::Programada;
    let es_reinicio = accion == Accion::Iniciar && estado == EstadoCirugia::Suspendida;
    let escrito = sqlx::query(
        "UPDATE programacion_quirurgica SET \
         estado = ?, \
         hora_inicio_real = CASE WHEN ? THEN ? ELSE hora_inicio_real END, \
         hora_reinicio    = CASE WHEN ? THEN ? ELSE hora_reinicio END, \
         hora_fin_real    = CASE WHEN ? THEN ? ELSE hora_fin_real END, \
         nota_quirurgica       = COALESCE(?, nota_quirurgica), \
         hallazgos_quirurgicos = COALESCE(?, hallazgos_quirurgicos), \
         sangrado_estimado_ml  = COALESCE(?, sangrado_estimado_ml), \
         especimenes_patologia = COALESCE(?, especimenes_patologia), \
         complicaciones        = COALESCE(?, complicaciones) \
         WHERE id = ? AND estado = ?",
    )
    .bind(paso.destino.como_texto())
    .bind(es_primer_inicio)
    .bind(&ahora)
    .bind(es_reinicio)
    .bind(&ahora)
    .bind(accion == Accion::Completar)
    .bind(&ahora)
    .bind(&datos.nota_quirurgica)
    .bind(&datos.hallazgos_quirurgicos)
    .bind(datos.sangrado_estimado_ml)
    .bind(&datos.especimenes_patologia)
    .bind(motivo_o_complicacion(accion, datos))
    .bind(id)
    .bind(estado.como_texto())
    .execute(&mut *tx)
    .await?;

    if escrito.rows_affected() == 0 {
        tx.rollback().await?;
        // Otra petición la movió entre la lectura y la escritura.
        return Ok(Resultado::Rechazado(
            "la cirugía cambió de estado mientras se procesaba la petición".into(),
        ));
    }

    match paso.sala {
        EfectoEnSala::Ninguno => {}
        EfectoEnSala::Ocupar => {
            // EL `rows_affected` NO SE DESCARTA, y esa línea es el arreglo.
            //
            // La primera versión lanzaba este `UPDATE` y seguía. La transacción
            // hacía atómicas las dos escrituras, sí, pero **no impedía la doble
            // ocupación**: dos cirugías podían quedar EN_CURSO en la misma sala
            // —medido por la revisión de esta rama— y al terminar la primera, la
            // sala volvía a DISPONIBLE con la segunda operando dentro. El
            // docstring de esta función afirmaba justo lo contrario.
            let ocupada = sqlx::query(
                "UPDATE quirofano SET estado = 'EN_USO' WHERE id = ? AND estado = 'DISPONIBLE'",
            )
            .bind(actual.quirofano_id)
            .execute(&mut *tx)
            .await?;
            if ocupada.rows_affected() == 0 {
                tx.rollback().await?;
                return Ok(Resultado::Rechazado(
                    "la sala no está disponible: ya hay una cirugía en curso, o está en \
                     mantenimiento o bloqueada"
                        .into(),
                ));
            }
        }
        EfectoEnSala::Liberar => {
            // Solo desde `EN_USO`. Una sala en MANTENIMIENTO o BLOQUEADO la puso
            // así una persona por un motivo, y terminar una cirugía no lo
            // deshace.
            sqlx::query("UPDATE quirofano SET estado = 'DISPONIBLE' WHERE id = ? AND estado = 'EN_USO'")
                .bind(actual.quirofano_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;
    Ok(Resultado::Hecho)
}

/// Al cancelar, el motivo se guarda en `complicaciones` con su prefijo —es donde
/// el Python lo pone y donde el frontal lo lee—. En las demás acciones se
/// respeta el campo tal cual venga.
fn motivo_o_complicacion(accion: Accion, d: &CierreCirugia) -> Option<String> {
    match (accion, d.motivo.as_deref()) {
        (Accion::Cancelar, Some(m)) if !m.trim().is_empty() => Some(format!("Cancelada: {m}")),
        _ => d.complicaciones.clone(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos instituciones, cada una con su sala, su paciente y su cirujano.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (5,'Cirujano Uno','c1@x.co','h','medico',1,1),
                    (6,'Anestesia Uno','a1@x.co','h','anestesiologo',1,1),
                    (7,'Cirujano Dos','c2@x.co','h','medico',2,1),
                    (8,'Farmacia Uno','f1@x.co','h','regente_farmacia',1,1),
                    (9,'Retirado Uno','r1@x.co','h','medico',1,0)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,
                                   fecha_nacimiento,sexo,institucion_id)
             VALUES (10,'CC','doc1','Ana','Ruiz','1990-01-01','F',1),
                    (20,'CC','doc2','Eva','Sol','1990-01-01','F',2)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO quirofano (id,institucion_id,nombre) VALUES (1,1,'Sala Uno'),(2,2,'Sala Dos')",
        ).execute(&pool).await.unwrap();
        pool
    }

    fn nueva() -> ProgramacionNueva {
        ProgramacionNueva {
            quirofano_id: 1,
            paciente_id: 10,
            cirujano_id: 5,
            anestesiologo_id: Some(6),
            consentimiento_id: None,
            fecha_programada: "2026-09-01T13:00:00Z".into(),
            duracion_estimada_min: 90,
            procedimiento: "Apendicectomía".into(),
            codigo_cups: Some("470100".into()),
            tipo_anestesia: Some("GENERAL".into()),
        }
    }

    const UNO: Ambito = Ambito::Institucion(1);
    const DOS: Ambito = Ambito::Institucion(2);

    async fn programar(pool: &crate::Pool) -> i64 {
        crear(pool, UNO, &nueva()).await.unwrap().expect("no se creó")
    }

    // ── Aislamiento ──────────────────────────────────────────────────────────

    /// **La fuga que este módulo vino a cerrar.** El listado del Python devolvía
    /// las cirugías de la otra institución CON el documento del paciente.
    #[tokio::test]
    async fn el_listado_no_cruza_instituciones() {
        let pool = base().await;
        programar(&pool).await;

        let mias = listar_programaciones(&pool, UNO, None).await.unwrap();
        assert_eq!(mias.len(), 1);
        assert_eq!(mias[0].paciente_nombre.as_deref(), Some("Ana Ruiz"));
        assert_eq!(mias[0].paciente_identificacion.as_deref(), Some("doc1"));

        let ajenas = listar_programaciones(&pool, DOS, None).await.unwrap();
        assert!(
            ajenas.is_empty(),
            "la institución 2 ve la cirugía de la 1: {:?}",
            ajenas.iter().map(|p| &p.paciente_identificacion).collect::<Vec<_>>()
        );
        assert!(listar_programaciones(&pool, Ambito::Ninguno, None).await.unwrap().is_empty());
        assert_eq!(
            listar_programaciones(&pool, Ambito::Superadmin, None).await.unwrap().len(),
            1,
            "el superadmin sí la ve: sin esto, un filtro que niegue a todos pasaría igual"
        );
    }

    #[tokio::test]
    async fn una_programacion_ajena_no_existe() {
        let pool = base().await;
        let id = programar(&pool).await;
        assert!(obtener(&pool, UNO, id).await.unwrap().is_some());
        assert!(
            obtener(&pool, DOS, id).await.unwrap().is_none(),
            "404 y nunca 403: la ajena y la inexistente se responden igual"
        );
        assert!(obtener(&pool, Ambito::Ninguno, id).await.unwrap().is_none());
    }

    /// El equipamiento sale como OBJETO, no como cadena — y un JSON roto no
    /// tumba el listado.
    #[tokio::test]
    async fn el_equipamiento_sale_como_objeto_json() {
        let pool = base().await;
        sqlx::query(
            "UPDATE quirofano SET equipamiento = '{\"torre_laparoscopia\": true}' WHERE id = 1",
        ).execute(&pool).await.unwrap();
        let s = &listar_salas(&pool, UNO).await.unwrap()[0];
        assert!(s.equipamiento.is_object(), "salió como {}", s.equipamiento);
        assert_eq!(s.equipamiento["torre_laparoscopia"], serde_json::json!(true));
        let json = serde_json::to_string(&s.equipamiento).unwrap();
        assert!(!json.starts_with('"'), "se serializó como cadena: {json}");

        // Y la pareja: un contenido roto da objeto vacío, no un 500.
        sqlx::query("UPDATE quirofano SET equipamiento = 'esto no es json' WHERE id = 1")
            .execute(&pool).await.unwrap();
        let s = &listar_salas(&pool, UNO).await.unwrap()[0];
        assert_eq!(s.equipamiento, serde_json::json!({}));
        assert_eq!(s.nombre, "Sala Uno", "la sala sigue listándose");
    }

    #[tokio::test]
    async fn las_salas_y_los_medicos_se_acotan() {
        let pool = base().await;
        let salas = listar_salas(&pool, UNO).await.unwrap();
        assert_eq!(salas.len(), 1);
        assert_eq!(salas[0].nombre, "Sala Uno");
        assert_eq!(listar_salas(&pool, Ambito::Ninguno).await.unwrap().len(), 0);
        assert_eq!(listar_salas(&pool, Ambito::Superadmin).await.unwrap().len(), 2);

        let ms = listar_medicos(&pool, UNO).await.unwrap();
        let nombres: Vec<&str> = ms.iter().map(|m| m.nombre_completo.as_str()).collect();
        assert!(nombres.contains(&"Cirujano Uno") && nombres.contains(&"Anestesia Uno"));
        assert!(!nombres.contains(&"Cirujano Dos"), "el de la otra institución");
        assert!(!nombres.contains(&"Farmacia Uno"), "el regente no opera");
        assert!(!nombres.contains(&"Retirado Uno"), "el dado de baja no se agenda");
    }

    // ── Los cuatro id del cuerpo ─────────────────────────────────────────────

    /// **Ninguno de los cuatro id se acepta a ciegas.** Es el mismo agujero que
    /// se cazó en el detalle de factura: la clave foránea no protege, es global.
    #[tokio::test]
    async fn no_se_programa_con_los_datos_del_vecino() {
        let pool = base().await;

        let mut n = nueva();
        n.paciente_id = 20;
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("paciente")
        );

        let mut n = nueva();
        n.cirujano_id = 7;
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("cirujano")
        );

        let mut n = nueva();
        n.anestesiologo_id = Some(7);
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("anestesiólogo")
        );

        let mut n = nueva();
        n.quirofano_id = 2;
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("sala de quirófano")
        );

        // Y la pareja: con los cuatro propios, se crea.
        assert!(crear(&pool, UNO, &nueva()).await.unwrap().is_ok());
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM programacion_quirurgica")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "los cuatro rechazos NO dejaron filas a medias");
    }

    /// El rol se exige, y el Python no lo exigía: nombrar cirujano al regente de
    /// farmacia deja la cirugía con un responsable que no puede serlo.
    #[tokio::test]
    async fn el_cirujano_tiene_que_poder_operar() {
        let pool = base().await;
        let mut n = nueva();
        n.cirujano_id = 8; // regente de farmacia, misma institución
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("cirujano")
        );
        n.cirujano_id = 9; // médico, pero dado de baja
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("cirujano")
        );
    }

    /// El consentimiento tiene que ser DEL MISMO paciente. En el Python el campo
    /// existe y nadie lo comprueba.
    #[tokio::test]
    async fn el_consentimiento_es_del_mismo_paciente() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO consentimiento_informado
               (id,paciente_id,medico_id,tipo_procedimiento,descripcion_procedimiento,
                riesgos_explicados,beneficios_esperados,alternativas_explicadas,
                paciente_comprende,acepta_procedimiento,firma_paciente_path,firma_medico_path)
             VALUES (1,10,5,'Cirugía','apendicectomía','s','c','no',1,1,'/p.png','/m.png'),
                    (2,20,7,'Cirugía','otra','s','c','no',1,1,'/p.png','/m.png')",
        ).execute(&pool).await.unwrap();

        let mut n = nueva();
        // El de OTRO paciente y el INEXISTENTE se responden IGUAL. Distinguirlos
        // era un oráculo —débil, pero contradecía la regla que este módulo
        // declara en su cabecera— y lo cazó la revisión de la rama.
        n.consentimiento_id = Some(2); // el del paciente de la OTRA institución
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("consentimiento")
        );
        n.consentimiento_id = Some(999); // no existe
        assert_eq!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("consentimiento"),
            "el ajeno y el inexistente tienen que dar exactamente lo mismo"
        );

        // Y la pareja: el suyo sí entra.
        n.consentimiento_id = Some(1);
        assert!(crear(&pool, UNO, &n).await.unwrap().is_ok(), "el suyo sí");
    }

    /// **Dos cirugías no pueden estar EN_CURSO en la misma sala.**
    ///
    /// Medido por la revisión de la rama: la versión anterior lanzaba el
    /// `UPDATE` de la sala y descartaba su `rows_affected`, así que la segunda
    /// entraba igual — y al terminar la primera, la sala volvía a DISPONIBLE con
    /// la segunda operando dentro.
    #[tokio::test]
    async fn una_sala_no_admite_dos_cirugias_a_la_vez() {
        let pool = base().await;
        let a = programar(&pool).await;
        let b = programar(&pool).await;

        assert_eq!(
            mover(&pool, UNO, a, Accion::Iniciar, &CierreCirugia::default()).await.unwrap(),
            Resultado::Hecho
        );
        let r = mover(&pool, UNO, b, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();
        assert!(
            matches!(r, Resultado::Rechazado(_)),
            "la segunda cirugía entró en una sala ocupada: {r:?}"
        );
        // Y el rechazo NO dejó la segunda a medias.
        assert_eq!(obtener(&pool, UNO, b).await.unwrap().unwrap().estado, "PROGRAMADA");
        assert!(obtener(&pool, UNO, b).await.unwrap().unwrap().hora_inicio_real.is_none());

        // La pareja: liberada la sala, la segunda sí entra.
        mover(&pool, UNO, a, Accion::Completar, &CierreCirugia::default()).await.unwrap();
        assert_eq!(
            mover(&pool, UNO, b, Accion::Iniciar, &CierreCirugia::default()).await.unwrap(),
            Resultado::Hecho
        );
    }

    /// La fecha se parsea, y el resto del cuerpo también: son entrada del
    /// cliente, así que se rechazan aquí y no reventando un `CHECK`.
    #[tokio::test]
    async fn el_cuerpo_se_valida_antes_de_tocar_la_base() {
        let pool = base().await;
        let mut n = nueva();
        n.fecha_programada = "mañana a las tres".into();
        assert!(matches!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::FechaInvalida(_)
        ));

        let mut n = nueva();
        n.duracion_estimada_min = 0;
        assert!(matches!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::Invalido(_)
        ));

        let mut n = nueva();
        n.tipo_anestesia = Some("EPIDURAL".into());
        assert!(matches!(
            crear(&pool, UNO, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::Invalido(_)
        ));

        // Y ninguno dejó fila.
        let filas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM programacion_quirurgica")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(filas, 0);
        assert!(crear(&pool, UNO, &nueva()).await.unwrap().is_ok(), "y la buena entra");
    }

    /// **Ni el superadmin puede mezclar instituciones en una programación.**
    ///
    /// Todo se comprueba contra la institución DEL PACIENTE. Antes se
    /// comprobaba contra la de quien llama, y `puede_ver` le dice que sí a todo
    /// al superadmin: podía dejar una fila con paciente de una institución y
    /// sala de otra, que ningún filtro alcanza después.
    #[tokio::test]
    async fn el_superadmin_tampoco_mezcla_instituciones() {
        let pool = base().await;
        let mut n = nueva();
        n.quirofano_id = 2; // sala de la institución 2, paciente de la 1
        assert_eq!(
            crear(&pool, Ambito::Superadmin, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("sala de quirófano")
        );

        let mut n = nueva();
        n.cirujano_id = 7; // cirujano de la institución 2
        assert_eq!(
            crear(&pool, Ambito::Superadmin, &n).await.unwrap().unwrap_err(),
            FalloAlCrear::NoVisible("cirujano")
        );

        // La pareja: coherente, el superadmin sí puede.
        assert!(crear(&pool, Ambito::Superadmin, &nueva()).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn una_sala_en_mantenimiento_no_recibe_cirugias() {
        let pool = base().await;
        for estado in ["MANTENIMIENTO", "BLOQUEADO"] {
            sqlx::query("UPDATE quirofano SET estado = ? WHERE id = 1")
                .bind(estado).execute(&pool).await.unwrap();
            assert_eq!(
                crear(&pool, UNO, &nueva()).await.unwrap().unwrap_err(),
                FalloAlCrear::SalaNoDisponible(estado.into())
            );
        }
        // Y la pareja: EN_USO sí acepta programar para más tarde. Una sala
        // ocupada ahora no es una sala fuera de servicio.
        sqlx::query("UPDATE quirofano SET estado = 'EN_USO' WHERE id = 1")
            .execute(&pool).await.unwrap();
        assert!(crear(&pool, UNO, &nueva()).await.unwrap().is_ok());
    }

    // ── La máquina de estados, contra la base ────────────────────────────────

    #[tokio::test]
    async fn iniciar_ocupa_la_sala_y_completar_la_libera() {
        let pool = base().await;
        let id = programar(&pool).await;
        let sala = || async {
            sqlx::query_scalar::<_, String>("SELECT estado FROM quirofano WHERE id = 1")
                .fetch_one(&pool).await.unwrap()
        };
        assert_eq!(sala().await, "DISPONIBLE");

        assert_eq!(
            mover(&pool, UNO, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap(),
            Resultado::Hecho
        );
        assert_eq!(sala().await, "EN_USO");
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert_eq!(p.estado, "EN_CURSO");
        assert!(p.hora_inicio_real.is_some(), "la hora de inicio la pone el servidor");
        assert!(p.hora_fin_real.is_none());

        let cierre = CierreCirugia {
            nota_quirurgica: Some("sin incidentes".into()),
            sangrado_estimado_ml: Some(50),
            ..Default::default()
        };
        assert_eq!(
            mover(&pool, UNO, id, Accion::Completar, &cierre).await.unwrap(),
            Resultado::Hecho
        );
        assert_eq!(sala().await, "DISPONIBLE");
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert_eq!(p.estado, "COMPLETADA");
        assert_eq!(p.nota_quirurgica.as_deref(), Some("sin incidentes"));
        assert_eq!(p.sangrado_estimado_ml, Some(50));
        assert!(p.hora_fin_real.is_some());
    }

    /// **El verbo `suspender` alcanza el estado que antes era inalcanzable**
    /// (#537): desde EN_CURSO libera la sala, y desde SUSPENDIDA el dominio ya
    /// deja las dos salidas —aquí, cancelar—.
    #[tokio::test]
    async fn suspender_alcanza_el_estado_y_libera_la_sala() {
        let pool = base().await;
        let id = programar(&pool).await;
        let sala = || async {
            sqlx::query_scalar::<_, String>("SELECT estado FROM quirofano WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap()
        };
        mover(&pool, UNO, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();
        assert_eq!(sala().await, "EN_USO");

        assert_eq!(
            mover(&pool, UNO, id, Accion::Suspender, &CierreCirugia::default()).await.unwrap(),
            Resultado::Hecho
        );
        assert_eq!(
            obtener(&pool, UNO, id).await.unwrap().unwrap().estado,
            "SUSPENDIDA",
            "el verbo alcanza el estado"
        );
        assert_eq!(sala().await, "DISPONIBLE", "suspender desde EN_CURSO libera la sala");

        assert_eq!(
            mover(&pool, UNO, id, Accion::Cancelar, &CierreCirugia::default()).await.unwrap(),
            Resultado::Hecho
        );
        assert_eq!(obtener(&pool, UNO, id).await.unwrap().unwrap().estado, "CANCELADA");
    }

    /// **Reanudar (iniciar sobre una suspendida) NO pisa la hora del primer
    /// inicio** y anota la del reinicio aparte (directiva 32, #537). Para que
    /// discrimine, la hora del primer inicio se fija a un valor VIEJO y conocido:
    /// un `mover` que la pisara con `ahora` la cambiaría y la prueba lo vería.
    #[tokio::test]
    async fn reanudar_conserva_la_hora_del_primer_inicio_y_anota_el_reinicio() {
        let pool = base().await;
        let id = programar(&pool).await;
        mover(&pool, UNO, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert!(p.hora_inicio_real.is_some());
        assert!(p.hora_reinicio.is_none(), "sin reanudar, no hay hora de reinicio");

        // La hora del primer inicio, fijada a un valor viejo y distinto para que
        // pisarla se note.
        sqlx::query(
            "UPDATE programacion_quirurgica SET hora_inicio_real = '2020-01-01 08:00:00' WHERE id = ?",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        mover(&pool, UNO, id, Accion::Suspender, &CierreCirugia::default()).await.unwrap();
        // Reanudar: iniciar sobre la suspendida.
        mover(&pool, UNO, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();

        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert_eq!(p.estado, "EN_CURSO", "reanudar vuelve a EN_CURSO");
        assert_eq!(
            p.hora_inicio_real.as_deref(),
            Some("2020-01-01 08:00:00"),
            "reanudar NO puede pisar la hora del primer inicio"
        );
        assert!(p.hora_reinicio.is_some(), "el reinicio se anota en su propia columna");
        assert_ne!(
            p.hora_reinicio.as_deref(),
            Some("2020-01-01 08:00:00"),
            "el reinicio es una hora distinta de la del primer inicio"
        );
    }

    /// Terminar una cirugía NO saca la sala de mantenimiento. Si lo hiciera,
    /// bastaría una cirugía vieja para devolver al servicio una sala que alguien
    /// bloqueó por un motivo.
    #[tokio::test]
    async fn liberar_no_resucita_una_sala_bloqueada() {
        let pool = base().await;
        let id = programar(&pool).await;
        mover(&pool, UNO, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();
        sqlx::query("UPDATE quirofano SET estado = 'MANTENIMIENTO' WHERE id = 1")
            .execute(&pool).await.unwrap();

        mover(&pool, UNO, id, Accion::Completar, &CierreCirugia::default()).await.unwrap();
        let estado: String = sqlx::query_scalar("SELECT estado FROM quirofano WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(estado, "MANTENIMIENTO", "la sala salió de mantenimiento sola");
    }

    #[tokio::test]
    async fn cancelar_sin_empezar_no_libera_la_sala_de_otra_cirugia() {
        let pool = base().await;
        let en_curso = programar(&pool).await;
        let futura = programar(&pool).await;
        mover(&pool, UNO, en_curso, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();

        let motivo = CierreCirugia { motivo: Some("el paciente no llegó".into()), ..Default::default() };
        assert_eq!(
            mover(&pool, UNO, futura, Accion::Cancelar, &motivo).await.unwrap(),
            Resultado::Hecho
        );
        let estado: String = sqlx::query_scalar("SELECT estado FROM quirofano WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(
            estado, "EN_USO",
            "cancelar una cirugía FUTURA liberó la sala donde otra está operando"
        );
        let p = obtener(&pool, UNO, futura).await.unwrap().unwrap();
        assert_eq!(p.complicaciones.as_deref(), Some("Cancelada: el paciente no llegó"));
    }

    #[tokio::test]
    async fn una_transicion_imposible_se_rechaza_sin_escribir() {
        let pool = base().await;
        let id = programar(&pool).await;
        let r = mover(&pool, UNO, id, Accion::Completar, &CierreCirugia::default()).await.unwrap();
        assert!(matches!(r, Resultado::Rechazado(_)), "{r:?}");
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert_eq!(p.estado, "PROGRAMADA", "el rechazo escribió igual");
        assert!(p.hora_fin_real.is_none());
    }

    /// Mover la cirugía de otra institución responde `NoExiste`, igual que una
    /// que no existe — y no escribe.
    #[tokio::test]
    async fn no_se_mueve_la_cirugia_del_vecino() {
        let pool = base().await;
        let id = programar(&pool).await;
        assert_eq!(
            mover(&pool, DOS, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap(),
            Resultado::NoExiste
        );
        assert_eq!(
            mover(&pool, Ambito::Ninguno, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap(),
            Resultado::NoExiste
        );
        assert_eq!(obtener(&pool, UNO, id).await.unwrap().unwrap().estado, "PROGRAMADA");
        assert_eq!(
            mover(&pool, DOS, 9999, Accion::Iniciar, &CierreCirugia::default()).await.unwrap(),
            Resultado::NoExiste,
            "la ajena y la inexistente dan lo mismo"
        );
    }

    /// Una fila con el estado corrupto no se trata como programada: se dice.
    #[tokio::test]
    async fn un_estado_corrupto_no_se_adivina() {
        let pool = base().await;
        let id = programar(&pool).await;
        // Por SQL crudo, que es la única forma de saltarse el CHECK... y no lo
        // es: el CHECK lo impide. Se comprueba que la base misma lo cierra.
        let e = sqlx::query("UPDATE programacion_quirurgica SET estado = 'RARO' WHERE id = ?")
            .bind(id).execute(&pool).await;
        assert!(e.is_err(), "el CHECK de la migración tiene que rechazar un estado inventado");
    }

    // ── Checklist ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn el_checklist_solo_cambia_lo_que_viene() {
        let pool = base().await;
        let id = programar(&pool).await;

        let c = ChecklistCambio { ayuno_verificado: Some(true), ..Default::default() };
        assert_eq!(actualizar_checklist(&pool, UNO, id, &c).await.unwrap(), Resultado::Hecho);
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert!(p.ayuno_verificado);
        assert!(!p.labs_prequirurgicos, "lo que no viene no se toca");

        let c = ChecklistCambio { hemoclasificacion: Some("O+".into()), ..Default::default() };
        actualizar_checklist(&pool, UNO, id, &c).await.unwrap();
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert!(p.ayuno_verificado, "el ayuno sigue marcado");
        assert_eq!(p.hemoclasificacion.as_deref(), Some("O+"));

        // **Y se puede BORRAR**, que es distinto de sustituir. Un grupo
        // sanguíneo mal capturado tiene que poder volver a «no lo sé».
        let c = ChecklistCambio { hemoclasificacion: Some("  ".into()), ..Default::default() };
        actualizar_checklist(&pool, UNO, id, &c).await.unwrap();
        let p = obtener(&pool, UNO, id).await.unwrap().unwrap();
        assert_eq!(p.hemoclasificacion, None, "no se pudo vaciar el grupo sanguíneo");
        assert!(p.ayuno_verificado, "y borrar uno no tocó los demás");

        // La pareja: NO mandar el campo lo deja como estaba.
        actualizar_checklist(&pool, UNO, id, &ChecklistCambio {
            hemoclasificacion: Some("A-".into()), ..Default::default()
        }).await.unwrap();
        actualizar_checklist(&pool, UNO, id, &ChecklistCambio::default()).await.unwrap();
        assert_eq!(
            obtener(&pool, UNO, id).await.unwrap().unwrap().hemoclasificacion.as_deref(),
            Some("A-"),
            "el campo ausente no debe borrar nada"
        );
    }

    /// La preparación de una cirugía que ya ocurrió no se retoca.
    #[tokio::test]
    async fn no_se_edita_el_checklist_de_una_cirugia_cerrada() {
        let pool = base().await;
        let id = programar(&pool).await;
        mover(&pool, UNO, id, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();
        mover(&pool, UNO, id, Accion::Completar, &CierreCirugia::default()).await.unwrap();

        let c = ChecklistCambio { ayuno_verificado: Some(true), ..Default::default() };
        let r = actualizar_checklist(&pool, UNO, id, &c).await.unwrap();
        assert!(matches!(r, Resultado::Rechazado(_)), "{r:?}");
        assert!(!obtener(&pool, UNO, id).await.unwrap().unwrap().ayuno_verificado);

        // Y la pareja: EN_CURSO sí se puede corregir — el checklist se completa
        // en la sala, con el paciente ya dentro.
        let id2 = programar(&pool).await;
        mover(&pool, UNO, id2, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();
        assert_eq!(actualizar_checklist(&pool, UNO, id2, &c).await.unwrap(), Resultado::Hecho);
    }

    #[tokio::test]
    async fn el_checklist_ajeno_no_existe() {
        let pool = base().await;
        let id = programar(&pool).await;
        let c = ChecklistCambio { ayuno_verificado: Some(true), ..Default::default() };
        assert_eq!(
            actualizar_checklist(&pool, DOS, id, &c).await.unwrap(),
            Resultado::NoExiste
        );
        assert!(!obtener(&pool, UNO, id).await.unwrap().unwrap().ayuno_verificado);
    }

    // ── Filtro por estado ────────────────────────────────────────────────────

    #[tokio::test]
    async fn el_filtro_por_estado_discrimina() {
        let pool = base().await;
        let a = programar(&pool).await;
        programar(&pool).await;
        mover(&pool, UNO, a, Accion::Iniciar, &CierreCirugia::default()).await.unwrap();

        assert_eq!(listar_programaciones(&pool, UNO, Some("EN_CURSO")).await.unwrap().len(), 1);
        assert_eq!(listar_programaciones(&pool, UNO, Some("PROGRAMADA")).await.unwrap().len(), 1);
        assert_eq!(listar_programaciones(&pool, UNO, None).await.unwrap().len(), 2);
        assert_eq!(listar_programaciones(&pool, UNO, Some("COMPLETADA")).await.unwrap().len(), 0);
    }
}
