// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Los ocho paneles de estadística. **Todo agregado, todo de solo lectura.**
//!
//! # Por qué un agregado necesita el mismo aislamiento que un listado
//!
//! Porque el número TAMBIÉN es el dato. Este módulo se escribe después de que
//! el lado Python perdiera la misma pelea cinco veces: `/financiero` devolvía el
//! valor facturado de TODAS las instituciones —medido con dos prestadores, los
//! dos leían `total_valor = 3000` habiendo facturado 1000 y 2000—, y el contador
//! de glosas de `/facturacion/stats` fue «la fuga que menos se veía, porque un
//! contador no parece un dato».
//!
//! Aquí no hay filtro opcional: cada consulta arranca de [`filtro_ambito`], que
//! para [`Ambito::Ninguno`] devuelve `None` y obliga al `let … else` a cortar.
//! El equivalente Python era una función que devolvía `None` con dos
//! significados distintos —«soy el superadmin» y «no tengo institución»— y las
//! cinco consultas que la usaban trataban los dos igual.
//!
//! # Las tres formas de acotar, según de qué cuelgue cada tabla
//!
//! | Tabla | Cómo se acota |
//! |---|---|
//! | `cita`, `paciente`, `factura` | tienen `institucion_id` propio |
//! | `historia`, `atencion_urgencias`, `hospitalizacion` | `JOIN paciente` |
//! | `glosa` | `JOIN factura` |
//! | `cama` | **no se acota**: ver [`Resumen`] |
//!
//! # Las fechas, que es donde estaba el defecto
//!
//! `cita.fecha` es una fecha local (`YYYY-MM-DD`) y se compara tal cual. Los
//! demás son INSTANTES en UTC, y ahí hay dos trampas encadenadas:
//!
//! 1. **El día del prestador no es el día UTC.** Una urgencia de las 20:00 en
//!    Bogotá se guarda como el día siguiente. Los límites se convierten con
//!    [`medico_dominio::zona::a_utc`], igual que el cierre del RIPS.
//! 2. **La base guarda DOS formatos.** `atencion_urgencias.fecha_ingreso` se
//!    escribe RFC 3339 (`2026-03-01T04:00:00Z`) y `historia.fecha` con el
//!    `DEFAULT` de SQLite (`2026-03-01 04:00:00`). La comparación de texto es
//!    lexicográfica y `'T'` (0x54) va DESPUÉS del espacio (0x20), así que
//!    comparar en crudo mete filas de fuera del rango. Medido: con la ventana de
//!    marzo, `2026-03-01T04:00:00Z` —que es el 28 de febrero a las 23:00 en
//!    Bogotá— entraba como si fuera de marzo.
//!
//! Por eso todo instante se normaliza con `replace(col,'T',' ')` antes de
//! comparar. Lo prueba `una_fecha_rfc3339_y_una_con_espacio_se_filtran_igual`,
//! con su pareja: una fila dentro y otra a cuatro horas del borde, en los dos
//! formatos.
//!
//! **`rips.rs` tiene esta misma comparación SIN normalizar** y por eso reporta a
//! MinSalud urgencias del mes anterior. No se arregla aquí —es otro módulo y
//! otra corrida (directiva 22)— sino en su propia tarea; queda dicho para que no
//! se copie de allí el patrón roto.

use medico_dominio::{zona, Ambito};
use serde::{Serialize, Serializer};

use crate::{filtro_ambito, hoy};

// ─────────────────────────────────────────────────────────────────────────────
// El mapa que conserva su orden
// ─────────────────────────────────────────────────────────────────────────────

/// Un `clave -> número` que se serializa como objeto JSON **en el orden en que
/// se insertó**.
///
/// No es capricho: en varios paneles el orden ES información. El triage va de
/// Rojo a Azul, que es la gravedad; alfabéticamente sería Amarillo, Azul,
/// Naranja, Rojo, Verde, o sea el orden de nada. Un `BTreeMap` ordenaría
/// alfabéticamente y un `HashMap` no ordena: produciría un JSON distinto en cada
/// arranque del proceso, que además hace irrepetible cualquier prueba.
///
/// Se sirve como objeto y no como lista porque el frontal lo lee con
/// `Object.keys(d)` y busca el color por la clave exacta.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MapaOrdenado(Vec<(String, i64)>);

impl MapaOrdenado {
    pub fn nuevo(pares: Vec<(String, i64)>) -> Self {
        Self(pares)
    }

    pub fn total(&self) -> i64 {
        self.0.iter().map(|(_, n)| n).sum()
    }

    pub fn get(&self, clave: &str) -> Option<i64> {
        self.0.iter().find(|(k, _)| k == clave).map(|(_, v)| *v)
    }

    pub fn claves(&self) -> Vec<&str> {
        self.0.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for MapaOrdenado {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.iter().map(|(k, v)| (k, v)))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fechas
// ─────────────────────────────────────────────────────────────────────────────

/// La columna de instante, normalizada para poder compararse con texto.
///
/// Ver la nota de fechas en la cabecera del módulo: sin esto, la mitad de la
/// base se compara mal en el borde del periodo.
fn instante(col: &str) -> String {
    format!("replace({col},'T',' ')")
}

/// Los dos extremos del periodo LOCAL, como el texto UTC que guarda la base.
///
/// El superior es **exclusivo** y es el día siguiente a las 00:00: así el último
/// día entra entero sin depender de si los segundos se guardaron o no. Es la
/// misma decisión que `< fin + 1 día` del lado Python.
fn limites_utc(inicio: &str, fin: &str) -> (String, String) {
    (texto_utc(inicio, 0), texto_utc(fin, 1))
}

fn texto_utc(dia: &str, mas_dias: i64) -> String {
    use chrono::NaiveDate;
    // Una fecha que no se puede leer NO se sustituye por hoy: se deja tal cual y
    // la comparación la trata como texto, que es lo mismo que hacía antes de
    // convertir. Rellenarla movería el periodo sin decirlo (directiva 20).
    let Ok(d) = NaiveDate::parse_from_str(dia, "%Y-%m-%d") else {
        return dia.to_string();
    };
    let d = d + chrono::Duration::days(mas_dias);
    let local = d.and_hms_opt(0, 0, 0).expect("medianoche siempre existe como hora");
    match zona::a_utc(local) {
        Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
        // La hora inexistente del cambio de horario: se interpreta como UTC en
        // vez de inventar un desplazamiento. Colombia no la tiene; el binario
        // puede acabar en otro sitio.
        None => local.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

/// El ÚLTIMO día del mes que empieza en `inicio` (`YYYY-MM-01`).
///
/// Se calcula saltando al día 1 del mes siguiente y restando uno, en vez de con
/// una tabla de longitudes: febrero y los bisiestos se resuelven solos.
fn fin_de_mes(inicio: &str) -> String {
    use chrono::{Datelike, NaiveDate};
    let Ok(d) = NaiveDate::parse_from_str(inicio, "%Y-%m-%d") else {
        return inicio.to_string();
    };
    let (anio, mes) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
    NaiveDate::from_ymd_opt(anio, mes, 1)
        .map(|siguiente| (siguiente - chrono::Duration::days(1)).to_string())
        .unwrap_or_else(|| inicio.to_string())
}

/// El primer día del mes de `fecha` (`YYYY-MM-DD`).
fn inicio_de_mes(fecha: &str) -> String {
    format!("{}-01", &fecha[..7.min(fecha.len())])
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipos de respuesta
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Periodo {
    pub inicio: String,
    pub fin: String,
}

/// Los KPI de portada.
///
/// # Las camas SÍ se acotan, y aquí el port se separa del original
///
/// El lado Python declara `Cama` en `core.aislamiento.COMPARTIDOS`, así que sus
/// tres cifras de camas y la tasa de ocupación son las del EDIFICIO: con dos
/// prestadores en la misma instalación, los dos leen las mismas camas y la misma
/// ocupación.
///
/// Este esquema ya decidió lo contrario y hace tiempo: la migración
/// `0023_camas_por_institucion` le puso `institucion_id NOT NULL` a `cama` y
/// movió el `UNIQUE` del número a `(institucion_id, numero)`, precisamente para
/// que dos prestadores puedan tener cada uno su «101-A». Copiar aquí el
/// `COMPARTIDOS` de Python sería contradecir el esquema sobre el que se consulta.
#[derive(Debug, Serialize)]
pub struct Resumen {
    pub fecha: String,
    pub total_pacientes: i64,
    pub citas_hoy: i64,
    pub citas_mes: i64,
    pub citas_atendidas_mes: i64,
    pub tasa_asistencia: f64,
    pub consultas_mes: i64,
    pub urgencias_activas: i64,
    pub hospitalizados: i64,
    pub total_camas: i64,
    pub camas_disponibles: i64,
    pub camas_ocupadas: i64,
    pub tasa_ocupacion: f64,
    pub facturas_pendientes: i64,
    pub valor_pendiente: f64,
}

#[derive(Debug, Serialize)]
pub struct Kpis {
    pub citas_hoy: i64,
    pub citas_mes: i64,
    pub total_pacientes: i64,
    pub fecha: String,
}

#[derive(Debug, Serialize)]
pub struct PuntoTendencia {
    pub fecha: String,
    pub cantidad: i64,
}

#[derive(Debug, Serialize)]
pub struct CitasPorMedico {
    pub medico: String,
    pub cantidad: i64,
}

#[derive(Debug, Serialize)]
pub struct Citas {
    pub periodo: Periodo,
    pub total: i64,
    pub by_estado: MapaOrdenado,
    pub by_tipo: MapaOrdenado,
    pub tendencia: Vec<PuntoTendencia>,
    pub by_medico: Vec<CitasPorMedico>,
}

#[derive(Debug, Serialize)]
pub struct PacientesPorEps {
    pub eps: String,
    pub cantidad: i64,
}

#[derive(Debug, Serialize)]
pub struct Pacientes {
    pub total: i64,
    pub by_sexo: MapaOrdenado,
    pub by_grupo_edad: MapaOrdenado,
    pub by_tipo_usuario: MapaOrdenado,
    pub by_eps: Vec<PacientesPorEps>,
    pub by_zona: MapaOrdenado,
}

#[derive(Debug, Serialize)]
pub struct Diagnostico {
    pub codigo: String,
    pub descripcion: String,
    pub cantidad: i64,
}

#[derive(Debug, Serialize)]
pub struct Diagnosticos {
    pub periodo: Periodo,
    pub diagnosticos: Vec<Diagnostico>,
}

#[derive(Debug, Serialize)]
pub struct ProduccionMedico {
    pub medico_id: i64,
    pub nombre: String,
    /// **Ausente, no inventada.** `usuario` no tiene columna de especialidad en
    /// este esquema —llega con el maestro de talento humano—, y es la misma
    /// decisión ya escrita en `InfoMedico` del contexto de historia. El frontal
    /// pinta «—» cuando viene nula.
    pub especialidad: Option<String>,
    pub citas_atendidas: i64,
    pub historias_creadas: i64,
}

#[derive(Debug, Serialize)]
pub struct Produccion {
    pub periodo: Periodo,
    pub medicos: Vec<ProduccionMedico>,
}

#[derive(Debug, Serialize)]
pub struct GrupoFacturas {
    pub estado: String,
    pub cantidad: i64,
    pub valor: f64,
}

#[derive(Debug, Serialize)]
pub struct PuntoMensual {
    pub periodo: String,
    pub cantidad: i64,
    pub valor: f64,
}

#[derive(Debug, Serialize)]
pub struct Financiero {
    pub periodo: Periodo,
    pub total_valor: f64,
    pub by_estado: Vec<GrupoFacturas>,
    pub tendencia_mensual: Vec<PuntoMensual>,
    pub glosas: Vec<GrupoFacturas>,
    pub valor_glosado_pendiente: f64,
}

#[derive(Debug, Serialize)]
pub struct Urgencias {
    pub periodo: Periodo,
    pub total: i64,
    pub by_triage: MapaOrdenado,
    pub by_destino: MapaOrdenado,
    /// Minutos medios entre el ingreso y el egreso. **Es la ESTANCIA, no la
    /// espera**, y el nombre lo dice porque son dos magnitudes distintas.
    ///
    /// El modelo Python tiene una columna propia `tiempo_espera_minutos`
    /// —«indicador de calidad»— que mide del triage a la atención. Este esquema
    /// no la tiene, así que la primera versión de este campo derivaba la
    /// estancia de las dos marcas que sí existen y la servía bajo la etiqueta
    /// «Tiempo espera promedio». Lo cazó la revisión de esta rama, y el fallo no
    /// era el cálculo sino el nombre: **ningún router del Python escribe esa
    /// columna**, así que allí el panel mostraba 0 siempre. O sea que el cambio
    /// no iba de 25 a 360, iba de un cero honesto a un número plausible y
    /// equivocado — la familia del `cod_consulta_cups='890201'`.
    ///
    /// La espera de verdad volverá cuando haya de dónde sacarla: hace falta la
    /// hora del triage y la del primer contacto médico, y ninguna de las dos se
    /// registra hoy.
    pub estancia_promedio_min: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Etiquetas
// ─────────────────────────────────────────────────────────────────────────────

/// Los cinco niveles, **en orden de gravedad**, con la etiqueta que el frontal
/// usa para buscar el color.
///
/// Los miembros del enum son `TRIAGE_1..TRIAGE_5`, no los colores: el lado
/// Python escribió `TipoTriage.ROJO` —que no existe— y ese panel respondió 500 a
/// todo el mundo durante meses sin que nadie lo notara, porque ninguna prueba lo
/// invocaba.
const TRIAGE: &[(&str, &str)] = &[
    ("TRIAGE_1", "Rojo (I)"),
    ("TRIAGE_2", "Naranja (II)"),
    ("TRIAGE_3", "Amarillo (III)"),
    ("TRIAGE_4", "Verde (IV)"),
    ("TRIAGE_5", "Azul (V)"),
];

/// Régimen de afiliación, tabla del MUV.
const TIPO_USUARIO: &[(&str, &str)] = &[
    ("01", "Contributivo"),
    ("02", "Subsidiado"),
    ("03", "Vinculado"),
    ("04", "Particular"),
    ("05", "Otro"),
];

const ZONA: &[(&str, &str)] = &[("01", "Urbana"), ("02", "Rural")];

/// Cómo se nombra la ausencia de un dato en un panel.
///
/// No es un valor por defecto —no sustituye a nada— sino la etiqueta de «aquí no
/// hay dato», que es justo lo que la directiva 20 pide que se VEA. El lado
/// Python emitía la cadena `"None"`, que parece un valor.
const SIN_DATO: &str = "(sin dato)";

fn etiquetar(tabla: &[(&str, &str)], valor: Option<&str>) -> String {
    match valor {
        None => SIN_DATO.to_string(),
        Some(v) => tabla
            .iter()
            .find(|(k, _)| *k == v)
            .map(|(_, e)| (*e).to_string())
            .unwrap_or_else(|| v.to_string()),
    }
}

/// Ordena los pares según la tabla de etiquetas, y deja al final lo que no esté
/// en ella. Es lo que convierte un `GROUP BY` en un orden con sentido clínico.
fn en_orden_de(tabla: &[(&str, &str)], mut pares: Vec<(String, i64)>) -> MapaOrdenado {
    let posicion = |e: &str| tabla.iter().position(|(_, x)| *x == e).unwrap_or(usize::MAX);
    pares.sort_by_key(|(k, _)| posicion(k));
    MapaOrdenado::nuevo(pares)
}

// ─────────────────────────────────────────────────────────────────────────────
// Los cuatro ejecutores
// ─────────────────────────────────────────────────────────────────────────────
//
// Toda consulta de este módulo pasa por uno de estos cinco, y ninguno acepta SQL
// sin `inst`. Los `valores` van SIEMPRE después del filtro, porque el filtro se
// escribe siempre primero en el `WHERE`.
//
// **LO QUE ESTA FIRMA GARANTIZA ES LA MITAD, Y CONVIENE QUE ESTÉ ESCRITO.** La
// primera versión de este comentario decía que impide «que el `?` del filtro y
// su valor se separen», y eso es cierto en UNA dirección: no se puede escribir
// la consulta acotada y olvidar el `bind`. Al revés no protege — lo midió la
// revisión de esta rama: una consulta con `1 = 1` y un `inst` enlazado se
// ejecuta limpiamente, porque SQLite ignora el parámetro sobrante, y devuelve
// las filas de las dos instituciones sin un solo error.
//
// Por eso el filtro NO se fabrica sustituyendo texto: cada tabla llama a
// `filtro_ambito` con SU columna. Antes había un `filtro_pac.replace("p.", "c.")`
// que funcionaba y fallaba ruidoso —si el patrón no casaba, SQLite se quejaba de
// una columna inexistente—, pero era una cadena decidiendo quién ve los datos de
// quién, y eso vuelve al sistema de tipos.

async fn escalar_i64(
    pool: &crate::Pool,
    sql: &str,
    inst: Option<i64>,
    valores: &[&str],
) -> Result<i64, sqlx::Error> {
    let mut q = sqlx::query_scalar::<_, i64>(sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for v in valores {
        q = q.bind(*v);
    }
    q.fetch_one(pool).await
}

async fn escalar_f64_opt(
    pool: &crate::Pool,
    sql: &str,
    inst: Option<i64>,
    valores: &[&str],
) -> Result<Option<f64>, sqlx::Error> {
    let mut q = sqlx::query_scalar::<_, Option<f64>>(sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for v in valores {
        q = q.bind(*v);
    }
    q.fetch_one(pool).await
}

/// Un `GROUP BY` de una columna que puede ser nula.
async fn agrupado(
    pool: &crate::Pool,
    sql: &str,
    inst: Option<i64>,
    valores: &[&str],
) -> Result<Vec<(Option<String>, i64)>, sqlx::Error> {
    let mut q = sqlx::query_as::<_, (Option<String>, i64)>(sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for v in valores {
        q = q.bind(*v);
    }
    q.fetch_all(pool).await
}

/// Como [`filas`], pero con un `LIMIT` que se enlaza como NÚMERO.
///
/// Existe porque el tope viajaba como cadena —los demás ejecutores toman
/// `&[&str]`— y `LIMIT '10'` funciona en SQLite por coerción del motor, no
/// porque el código lo diga. Un motor más estricto lo rechazaría, y una
/// coerción implícita en la que nadie repara es la clase de cosa que se
/// descubre el día de la migración.
async fn filas_con_tope<T>(
    pool: &crate::Pool,
    sql: &str,
    inst: Option<i64>,
    valores: &[&str],
    tope: i64,
) -> Result<Vec<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    let mut q = sqlx::query_as::<_, T>(sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for v in valores {
        q = q.bind(*v);
    }
    q.bind(tope).fetch_all(pool).await
}

/// Una consulta cuya forma de fila la decide quien llama.
async fn filas<T>(
    pool: &crate::Pool,
    sql: &str,
    inst: Option<i64>,
    valores: &[&str],
) -> Result<Vec<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    let mut q = sqlx::query_as::<_, T>(sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for v in valores {
        q = q.bind(*v);
    }
    q.fetch_all(pool).await
}

fn con_etiquetas(
    tabla: &[(&str, &str)],
    filas: Vec<(Option<String>, i64)>,
) -> Vec<(String, i64)> {
    filas
        .into_iter()
        .map(|(k, n)| (etiquetar(tabla, k.as_deref()), n))
        .collect()
}

fn sin_nulos(filas: Vec<(Option<String>, i64)>) -> Vec<(String, i64)> {
    filas
        .into_iter()
        .map(|(k, n)| (k.unwrap_or_else(|| SIN_DATO.to_string()), n))
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Resumen y KPI
// ─────────────────────────────────────────────────────────────────────────────

pub async fn resumen(pool: &crate::Pool, ambito: Ambito) -> Result<Resumen, sqlx::Error> {
    let hoy = hoy();
    let inicio_mes = inicio_de_mes(&hoy);

    // Un filtro por TABLA, y los cuatro salen de `filtro_ambito`. El `let … else`
    // sobre la tupla corta los cuatro a la vez: para `Ambito::Ninguno` los cuatro
    // son `None` y no se consulta nada.
    let (
        Some((filtro_pac, inst)),
        Some((filtro_cita, _)),
        Some((filtro_fact, _)),
        Some((filtro_cama, _)),
    ) = (
        filtro_ambito(ambito, "p.institucion_id"),
        filtro_ambito(ambito, "c.institucion_id"),
        filtro_ambito(ambito, "f.institucion_id"),
        filtro_ambito(ambito, "m.institucion_id"),
    )
    else {
        return Ok(Resumen {
            fecha: hoy,
            total_pacientes: 0,
            citas_hoy: 0,
            citas_mes: 0,
            citas_atendidas_mes: 0,
            tasa_asistencia: 0.0,
            consultas_mes: 0,
            urgencias_activas: 0,
            hospitalizados: 0,
            total_camas: 0,
            camas_disponibles: 0,
            camas_ocupadas: 0,
            tasa_ocupacion: 0.0,
            facturas_pendientes: 0,
            valor_pendiente: 0.0,
        });
    };
    let (total_camas, camas_ocupadas, camas_disponibles) =
        camas(pool, &filtro_cama, inst).await?;
    let tasa_ocupacion = porcentaje(camas_ocupadas, total_camas);
    let sql = format!("SELECT COUNT(*) FROM paciente p WHERE {filtro_pac}");
    let total_pacientes = escalar_i64(pool, &sql, inst, &[]).await?;

    let sql = format!("SELECT COUNT(*) FROM cita c WHERE {filtro_cita} AND c.fecha = ?");
    let citas_hoy = escalar_i64(pool, &sql, inst, &[&hoy]).await?;

    // EL MES TIENE DOS EXTREMOS, y aquí el port se separa del original.
    //
    // El Python cuenta `fecha >= inicio_mes` y nada más, así que una cita
    // agendada para diciembre entra en las «Citas Este Mes» de marzo. Como
    // `citas_atendidas_mes` solo puede ser pasado, la tasa de asistencia
    // —`atendidas / citas_mes`, la tarjeta de portada— **baja cuanto más se
    // agende con antelación**: el indicador castiga justo la buena práctica.
    // Lo cazó la revisión de esta rama.
    let fin_mes = fin_de_mes(&inicio_mes);
    let sql = format!(
        "SELECT COUNT(*) FROM cita c WHERE {filtro_cita} AND c.fecha >= ? AND c.fecha <= ?"
    );
    let citas_mes = escalar_i64(pool, &sql, inst, &[&inicio_mes, &fin_mes]).await?;

    let sql = format!(
        "SELECT COUNT(*) FROM cita c WHERE {filtro_cita} AND c.fecha >= ? AND c.fecha <= ? \
         AND c.estado = 'ATENDIDA'"
    );
    let citas_atendidas_mes = escalar_i64(pool, &sql, inst, &[&inicio_mes, &fin_mes]).await?;

    let desde_mes = texto_utc(&inicio_mes, 0);
    let col_hist = instante("h.fecha");
    let sql = format!(
        "SELECT COUNT(*) FROM historia h JOIN paciente p ON p.id = h.paciente_id \
         WHERE {filtro_pac} AND {col_hist} >= ?"
    );
    let consultas_mes = escalar_i64(pool, &sql, inst, &[&desde_mes]).await?;

    // «Activa» es lo mismo que en la cola de urgencias: sin destino de salida.
    // El esquema Rust no tiene la bandera `en_atencion` del modelo Python.
    let sql = format!(
        "SELECT COUNT(*) FROM atencion_urgencias u JOIN paciente p ON p.id = u.paciente_id \
         WHERE {filtro_pac} AND u.destino_salida IS NULL"
    );
    let urgencias_activas = escalar_i64(pool, &sql, inst, &[]).await?;

    let sql = format!(
        "SELECT COUNT(*) FROM hospitalizacion i JOIN paciente p ON p.id = i.paciente_id \
         WHERE {filtro_pac} AND i.estado = 'ACTIVA'"
    );
    let hospitalizados = escalar_i64(pool, &sql, inst, &[]).await?;

    let sql = format!(
        "SELECT COUNT(*) FROM factura f WHERE {filtro_fact} \
         AND f.estado IN ('BORRADOR','RADICADA')"
    );
    let facturas_pendientes = escalar_i64(pool, &sql, inst, &[]).await?;

    let sql = format!(
        "SELECT CAST(COALESCE(SUM(f.valor_neto),0) AS REAL) FROM factura f WHERE {filtro_fact} \
         AND f.estado IN ('BORRADOR','RADICADA')"
    );
    let valor_pendiente = escalar_f64_opt(pool, &sql, inst, &[]).await?.unwrap_or(0.0);

    Ok(Resumen {
        fecha: hoy,
        total_pacientes,
        citas_hoy,
        citas_mes,
        citas_atendidas_mes,
        tasa_asistencia: porcentaje(citas_atendidas_mes, citas_mes),
        consultas_mes,
        urgencias_activas,
        hospitalizados,
        total_camas,
        camas_disponibles,
        camas_ocupadas,
        tasa_ocupacion,
        facturas_pendientes,
        valor_pendiente,
    })
}

pub async fn kpis(pool: &crate::Pool, ambito: Ambito) -> Result<Kpis, sqlx::Error> {
    let r = resumen(pool, ambito).await?;
    Ok(Kpis {
        citas_hoy: r.citas_hoy,
        citas_mes: r.citas_mes,
        total_pacientes: r.total_pacientes,
        fecha: r.fecha,
    })
}

/// Camas totales, ocupadas y disponibles del ámbito. `cama` lleva
/// `institucion_id` desde la migración 0023, así que se acota como todo lo demás.
async fn camas(
    pool: &crate::Pool,
    filtro: &str,
    inst: Option<i64>,
) -> Result<(i64, i64, i64), sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*), \
         COALESCE(SUM(m.estado = 'OCUPADA'),0), \
         COALESCE(SUM(m.estado = 'DISPONIBLE'),0) FROM cama m WHERE {filtro}"
    );
    let mut q = sqlx::query_as::<_, (i64, i64, i64)>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_one(pool).await
}

/// Un porcentaje con un decimal. `total = 0` da cero y no NaN: la tasa de un
/// hospital sin camas es cero, y un NaN se serializa como `null` y rompe la
/// gráfica del frontal.
fn porcentaje(parte: i64, total: i64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((parte as f64 / total as f64) * 1000.0).round() / 10.0
}

// ─────────────────────────────────────────────────────────────────────────────
// Citas
// ─────────────────────────────────────────────────────────────────────────────

pub async fn citas(
    pool: &crate::Pool,
    ambito: Ambito,
    inicio: &str,
    fin: &str,
) -> Result<Citas, sqlx::Error> {
    let periodo = Periodo { inicio: inicio.to_string(), fin: fin.to_string() };
    let Some((filtro, inst)) = filtro_ambito(ambito, "c.institucion_id") else {
        return Ok(Citas {
            periodo,
            total: 0,
            by_estado: MapaOrdenado::default(),
            by_tipo: MapaOrdenado::default(),
            tendencia: Vec::new(),
            by_medico: Vec::new(),
        });
    };
    // `cita.fecha` es una fecha LOCAL en `YYYY-MM-DD`: se compara tal cual, sin
    // conversión de huso y sin `CAST`. El `CAST(x AS DATE)` del lado Python
    // devuelve **2026** en SQLite (afinidad numérica), y por eso aquel panel
    // contaba cero en cualquier base SQLite.
    let rango = format!("{filtro} AND c.fecha >= ? AND c.fecha <= ?");
    let fechas: [&str; 2] = [inicio, fin];

    let sql = format!(
        "SELECT c.estado, COUNT(*) AS n FROM cita c WHERE {rango} \
         GROUP BY c.estado ORDER BY n DESC, c.estado"
    );
    let by_estado = MapaOrdenado::nuevo(sin_nulos(agrupado(pool, &sql, inst, &fechas).await?));

    let sql = format!(
        "SELECT c.tipo, COUNT(*) AS n FROM cita c WHERE {rango} \
         GROUP BY c.tipo ORDER BY n DESC, c.tipo"
    );
    let by_tipo = MapaOrdenado::nuevo(sin_nulos(agrupado(pool, &sql, inst, &fechas).await?));

    // Diaria hasta 60 días, mensual por encima: una gráfica de 400 puntos no se
    // lee. El corte va sobre las fechas pedidas, no sobre el número de filas.
    let expr = if dias_entre(inicio, fin) <= 60 {
        "c.fecha"
    } else {
        "substr(c.fecha,1,7)"
    };
    let sql = format!(
        "SELECT {expr}, COUNT(*) FROM cita c WHERE {rango} GROUP BY {expr} ORDER BY 1"
    );
    let tendencia: Vec<PuntoTendencia> = filas::<(String, i64)>(pool, &sql, inst, &fechas)
        .await?
        .into_iter()
        .map(|(fecha, cantidad)| PuntoTendencia { fecha, cantidad })
        .collect();

    // El JOIN a `usuario` hace dos cosas a la vez: pone el nombre y deja fuera
    // las citas sin médico asignado, que no son de ningún profesional. El lado
    // Python las agrupaba bajo la etiqueta «ID None».
    let sql = format!(
        "SELECT u.nombre, COUNT(*) AS n FROM cita c JOIN usuario u ON u.id = c.medico_id \
         WHERE {rango} GROUP BY u.id, u.nombre ORDER BY n DESC, u.nombre LIMIT 10"
    );
    let by_medico: Vec<CitasPorMedico> = filas::<(String, i64)>(pool, &sql, inst, &fechas)
        .await?
        .into_iter()
        .map(|(medico, cantidad)| CitasPorMedico { medico, cantidad })
        .collect();

    Ok(Citas {
        periodo,
        total: by_estado.total(),
        by_estado,
        by_tipo,
        tendencia,
        by_medico,
    })
}

fn dias_entre(inicio: &str, fin: &str) -> i64 {
    use chrono::NaiveDate;
    match (
        NaiveDate::parse_from_str(inicio, "%Y-%m-%d"),
        NaiveDate::parse_from_str(fin, "%Y-%m-%d"),
    ) {
        (Ok(a), Ok(b)) => (b - a).num_days(),
        _ => 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pacientes
// ─────────────────────────────────────────────────────────────────────────────

pub async fn pacientes(pool: &crate::Pool, ambito: Ambito) -> Result<Pacientes, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Pacientes {
            total: 0,
            by_sexo: MapaOrdenado::default(),
            by_grupo_edad: grupos_de_edad(std::iter::empty()),
            by_tipo_usuario: MapaOrdenado::default(),
            by_eps: Vec::new(),
            by_zona: MapaOrdenado::default(),
        });
    };

    let sql = format!(
        "SELECT p.sexo, COUNT(*) AS n FROM paciente p WHERE {filtro} \
         GROUP BY p.sexo ORDER BY n DESC, p.sexo"
    );
    let by_sexo = MapaOrdenado::nuevo(sin_nulos(agrupado(pool, &sql, inst, &[]).await?));

    let sql = format!(
        "SELECT p.tipo_usuario, COUNT(*) FROM paciente p WHERE {filtro} GROUP BY p.tipo_usuario"
    );
    let by_tipo_usuario = en_orden_de(
        TIPO_USUARIO,
        con_etiquetas(TIPO_USUARIO, agrupado(pool, &sql, inst, &[]).await?),
    );

    let sql =
        format!("SELECT p.zona, COUNT(*) FROM paciente p WHERE {filtro} GROUP BY p.zona");
    let by_zona = en_orden_de(ZONA, con_etiquetas(ZONA, agrupado(pool, &sql, inst, &[]).await?));

    // La edad se calcula en Rust y no en SQL: `zona::hoy()` es la fecha del
    // PRESTADOR, y `date('now')` de SQLite es UTC. Entre las 19:00 y la
    // medianoche de Bogotá esas dos no son el mismo día, y ahí es donde un
    // paciente cambia de grupo.
    let sql = format!("SELECT p.fecha_nacimiento FROM paciente p WHERE {filtro}");
    let nacimientos: Vec<(String,)> = filas(pool, &sql, inst, &[]).await?;
    let by_grupo_edad = grupos_de_edad(nacimientos.iter().map(|(f,)| f.as_str()));

    let sql = format!("SELECT COUNT(*) FROM paciente p WHERE {filtro}");
    let total = escalar_i64(pool, &sql, inst, &[]).await?;

    let sql = format!(
        "SELECT e.nombre, COUNT(*) AS n FROM paciente p JOIN entidad e ON e.id = p.entidad_id \
         WHERE {filtro} GROUP BY e.id, e.nombre ORDER BY n DESC, e.nombre LIMIT 8"
    );
    let by_eps: Vec<PacientesPorEps> = filas::<(String, i64)>(pool, &sql, inst, &[])
        .await?
        .into_iter()
        .map(|(eps, cantidad)| PacientesPorEps { eps, cantidad })
        .collect();

    Ok(Pacientes {
        total,
        by_sexo,
        by_grupo_edad,
        by_tipo_usuario,
        by_eps,
        by_zona,
    })
}

/// Los seis tramos, en orden de edad. Se emiten TODOS aunque estén a cero: a una
/// gráfica a la que le falta el tramo «60+» se le lee «aquí no hay mayores», y
/// eso es distinto de «hay cero».
fn grupos_de_edad<'a>(nacimientos: impl Iterator<Item = &'a str>) -> MapaOrdenado {
    use chrono::NaiveDate;
    const TRAMOS: [(&str, i64); 6] = [
        ("0-10", 10),
        ("11-17", 17),
        ("18-29", 29),
        ("30-44", 44),
        ("45-59", 59),
        ("60+", i64::MAX),
    ];
    let hoy = zona::hoy();
    let mut cuenta = [0i64; 6];
    for texto in nacimientos {
        let Ok(nacimiento) = NaiveDate::parse_from_str(texto, "%Y-%m-%d") else {
            continue;
        };
        let edad = edad_en(nacimiento, hoy);
        // Una fecha de nacimiento en el FUTURO no es un recién nacido: es un
        // error de digitación. El lado Python la mete en el tramo «0-10» —su
        // cadena de `if edad <= 10` acierta con el primero— y ahí queda
        // indistinguible de un bebé de verdad. Aquí se descarta, igual que una
        // fecha ilegible: se ve en que el total no cuadra con el censo, que es
        // exactamente lo que hay que ver.
        if edad < 0 {
            continue;
        }
        let i = TRAMOS.iter().position(|(_, tope)| edad <= *tope).unwrap_or(5);
        cuenta[i] += 1;
    }
    MapaOrdenado::nuevo(
        TRAMOS
            .iter()
            .zip(cuenta)
            .map(|((nombre, _), n)| ((*nombre).to_string(), n))
            .collect(),
    )
}

fn edad_en(nacimiento: chrono::NaiveDate, hoy: chrono::NaiveDate) -> i64 {
    use chrono::Datelike;
    let mut edad = i64::from(hoy.year()) - i64::from(nacimiento.year());
    if (hoy.month(), hoy.day()) < (nacimiento.month(), nacimiento.day()) {
        edad -= 1;
    }
    edad
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnósticos
// ─────────────────────────────────────────────────────────────────────────────

pub async fn diagnosticos(
    pool: &crate::Pool,
    ambito: Ambito,
    inicio: &str,
    fin: &str,
    top: i64,
) -> Result<Diagnosticos, sqlx::Error> {
    let periodo = Periodo { inicio: inicio.to_string(), fin: fin.to_string() };
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Diagnosticos { periodo, diagnosticos: Vec::new() });
    };
    let (desde, hasta) = limites_utc(inicio, fin);
    let col = instante("h.fecha");

    let sql = format!(
        "SELECT h.diagnostico_principal, c.descripcion, COUNT(*) AS n \
         FROM historia h JOIN paciente p ON p.id = h.paciente_id \
         LEFT JOIN cie10 c ON c.codigo = h.diagnostico_principal \
         WHERE {filtro} AND {col} >= ? AND {col} < ? \
         GROUP BY h.diagnostico_principal, c.descripcion \
         ORDER BY n DESC, h.diagnostico_principal LIMIT ?"
    );
    let filas: Vec<(String, Option<String>, i64)> =
        filas_con_tope(pool, &sql, inst, &[&desde, &hasta], top.clamp(1, 30)).await?;

    Ok(Diagnosticos {
        periodo,
        diagnosticos: filas
            .into_iter()
            .map(|(codigo, descripcion, cantidad)| Diagnostico {
                // Sin descripción en el maestro se muestra el CÓDIGO, que es lo
                // que el profesional escribió: no se inventa un texto.
                descripcion: descripcion.unwrap_or_else(|| codigo.clone()),
                codigo,
                cantidad,
            })
            .collect(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Producción
// ─────────────────────────────────────────────────────────────────────────────

pub async fn produccion(
    pool: &crate::Pool,
    ambito: Ambito,
    inicio: &str,
    fin: &str,
) -> Result<Produccion, sqlx::Error> {
    let periodo = Periodo { inicio: inicio.to_string(), fin: fin.to_string() };
    let (Some((filtro_pac, inst)), Some((filtro_cita, _))) = (
        filtro_ambito(ambito, "p.institucion_id"),
        filtro_ambito(ambito, "c.institucion_id"),
    ) else {
        return Ok(Produccion { periodo, medicos: Vec::new() });
    };
    let (desde, hasta) = limites_utc(inicio, fin);
    let col = instante("h.fecha");

    let sql = format!(
        "SELECT u.id, u.nombre, COUNT(*) FROM cita c JOIN usuario u ON u.id = c.medico_id \
         WHERE {filtro_cita} AND c.fecha >= ? AND c.fecha <= ? AND c.estado = 'ATENDIDA' \
         GROUP BY u.id, u.nombre"
    );
    let citas: Vec<(i64, String, i64)> = filas(pool, &sql, inst, &[inicio, fin]).await?;

    let sql = format!(
        "SELECT u.id, u.nombre, COUNT(*) FROM historia h \
         JOIN paciente p ON p.id = h.paciente_id \
         JOIN usuario  u ON u.id = h.medico_id \
         WHERE {filtro_pac} AND {col} >= ? AND {col} < ? \
         GROUP BY u.id, u.nombre"
    );
    let historias: Vec<(i64, String, i64)> = filas(pool, &sql, inst, &[&desde, &hasta]).await?;

    let mut medicos: Vec<ProduccionMedico> = citas
        .into_iter()
        .map(|(id, nombre, n)| ProduccionMedico {
            medico_id: id,
            nombre,
            especialidad: None,
            citas_atendidas: n,
            historias_creadas: 0,
        })
        .collect();
    for (id, nombre, n) in historias {
        match medicos.iter_mut().find(|m| m.medico_id == id) {
            Some(m) => m.historias_creadas = n,
            None => medicos.push(ProduccionMedico {
                medico_id: id,
                nombre,
                especialidad: None,
                citas_atendidas: 0,
                historias_creadas: n,
            }),
        }
    }
    // Por citas atendidas, y a igualdad por nombre: sin el segundo criterio el
    // orden de dos médicos empatados cambia entre corridas.
    medicos.sort_by(|a, b| {
        b.citas_atendidas
            .cmp(&a.citas_atendidas)
            .then_with(|| a.nombre.cmp(&b.nombre))
    });

    Ok(Produccion { periodo, medicos })
}

// ─────────────────────────────────────────────────────────────────────────────
// Financiero
// ─────────────────────────────────────────────────────────────────────────────

pub async fn financiero(
    pool: &crate::Pool,
    ambito: Ambito,
    inicio: &str,
    fin: &str,
) -> Result<Financiero, sqlx::Error> {
    let periodo = Periodo { inicio: inicio.to_string(), fin: fin.to_string() };
    let Some((filtro, inst)) = filtro_ambito(ambito, "f.institucion_id") else {
        return Ok(Financiero {
            periodo,
            total_valor: 0.0,
            by_estado: Vec::new(),
            tendencia_mensual: Vec::new(),
            glosas: Vec::new(),
            valor_glosado_pendiente: 0.0,
        });
    };
    // `factura.fecha_factura` es una fecha, no un instante: se compara directa.
    let rango = format!("{filtro} AND f.fecha_factura >= ? AND f.fecha_factura <= ?");
    let fechas: [&str; 2] = [inicio, fin];

    let sql = format!(
        "SELECT f.estado, COUNT(*), CAST(COALESCE(SUM(f.valor_neto),0) AS REAL) \
         FROM factura f WHERE {rango} GROUP BY f.estado ORDER BY f.estado"
    );
    let by_estado: Vec<GrupoFacturas> = filas::<(String, i64, f64)>(pool, &sql, inst, &fechas)
        .await?
        .into_iter()
        .map(|(estado, cantidad, valor)| GrupoFacturas { estado, cantidad, valor })
        .collect();

    let sql = format!(
        "SELECT substr(f.fecha_factura,1,7), COUNT(*), \
         CAST(COALESCE(SUM(f.valor_neto),0) AS REAL) FROM factura f WHERE {rango} \
         GROUP BY substr(f.fecha_factura,1,7) ORDER BY 1"
    );
    let tendencia_mensual: Vec<PuntoMensual> = filas::<(String, i64, f64)>(pool, &sql, inst, &fechas)
        .await?
        .into_iter()
        .map(|(periodo, cantidad, valor)| PuntoMensual { periodo, cantidad, valor })
        .collect();

    // LA GLOSA NO SE FILTRA POR FECHA, y esto ya se decidió una vez.
    //
    // Cuelga de una factura con `factura_id` obligatorio, así que el JOIN es lo
    // que la acota — y SOLO el JOIN. En el lado Python hubo aquí un filtro por
    // `Factura.fecha_factura` que duró un commit: las glosas llegan meses
    // después de radicar, así que una glosa VIVA sobre una factura del año
    // anterior desaparecía del panel. Medido entonces: 100 aquí frente a 877 en
    // `/facturacion/stats`, con los mismos datos y la misma sesión.
    let sql = format!(
        "SELECT g.estado, COUNT(*), CAST(COALESCE(SUM(g.valor_glosado),0) AS REAL) \
         FROM glosa g JOIN factura f ON f.id = g.factura_id \
         WHERE {filtro} GROUP BY g.estado ORDER BY g.estado"
    );
    let glosas: Vec<GrupoFacturas> = filas::<(String, i64, f64)>(pool, &sql, inst, &[])
        .await?
        .into_iter()
        .map(|(estado, cantidad, valor)| GrupoFacturas { estado, cantidad, valor })
        .collect();

    Ok(Financiero {
        total_valor: by_estado.iter().map(|g| g.valor).sum(),
        valor_glosado_pendiente: glosas
            .iter()
            .filter(|g| g.estado == "PENDIENTE")
            .map(|g| g.valor)
            .sum(),
        periodo,
        by_estado,
        tendencia_mensual,
        glosas,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Urgencias
// ─────────────────────────────────────────────────────────────────────────────

pub async fn urgencias(
    pool: &crate::Pool,
    ambito: Ambito,
    inicio: &str,
    fin: &str,
) -> Result<Urgencias, sqlx::Error> {
    let periodo = Periodo { inicio: inicio.to_string(), fin: fin.to_string() };
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Urgencias {
            periodo,
            total: 0,
            by_triage: MapaOrdenado::default(),
            by_destino: MapaOrdenado::default(),
            estancia_promedio_min: 0.0,
        });
    };
    let (desde, hasta) = limites_utc(inicio, fin);
    let col = instante("u.fecha_ingreso");
    let rango = format!("{filtro} AND {col} >= ? AND {col} < ?");
    let limites: [&str; 2] = [&desde, &hasta];

    let sql = format!(
        "SELECT u.triage, COUNT(*) FROM atencion_urgencias u \
         JOIN paciente p ON p.id = u.paciente_id WHERE {rango} GROUP BY u.triage"
    );
    let by_triage = en_orden_de(
        TRIAGE,
        con_etiquetas(TRIAGE, agrupado(pool, &sql, inst, &limites).await?),
    );

    // El destino sí se filtra: una atención sin destino es una que sigue
    // abierta, no un destino llamado «ninguno».
    let sql = format!(
        "SELECT u.destino_salida, COUNT(*) AS n FROM atencion_urgencias u \
         JOIN paciente p ON p.id = u.paciente_id WHERE {rango} \
         AND u.destino_salida IS NOT NULL GROUP BY u.destino_salida ORDER BY n DESC, 1"
    );
    let by_destino = MapaOrdenado::nuevo(sin_nulos(agrupado(pool, &sql, inst, &limites).await?));

    let sql = format!(
        "SELECT COUNT(*) FROM atencion_urgencias u JOIN paciente p ON p.id = u.paciente_id \
         WHERE {rango}"
    );
    let total = escalar_i64(pool, &sql, inst, &limites).await?;

    // La ESTANCIA: de ingreso a egreso. No es el tiempo de espera y no se llama
    // así — ver el doc del campo.
    let sql = format!(
        "SELECT AVG((julianday(replace(u.fecha_egreso,'T',' ')) - julianday({col})) * 1440.0) \
         FROM atencion_urgencias u JOIN paciente p ON p.id = u.paciente_id \
         WHERE {rango} AND u.fecha_egreso IS NOT NULL"
    );
    let estancia = escalar_f64_opt(pool, &sql, inst, &limites).await?;

    Ok(Urgencias {
        periodo,
        total,
        by_triage,
        by_destino,
        estancia_promedio_min: estancia.map(|m| (m * 10.0).round() / 10.0).unwrap_or(0.0),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos instituciones con datos EQUIVALENTES y distintos.
    ///
    /// La forma importa: la 1 y la 2 tienen el mismo tipo de filas con valores
    /// distintos, para que una fuga se vea como un número mayor y no como un
    /// campo raro. Los volúmenes son distintos a propósito (1 tiene más citas
    /// que 2) porque un aislamiento roto sumaría los dos.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO entidad (id,nit,nombre,codigo_eps)
             VALUES (1,'800','EPS Uno','E1'),(2,'801','EPS Dos','E2')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (5,'Ana Uno','a@x.co','h','medico',1,1),
                    (6,'Beto Dos','b@x.co','h','medico',2,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,
                                   fecha_nacimiento,sexo,institucion_id,tipo_usuario,zona,entidad_id)
             VALUES (10,'CC','111','Ana','Ruiz','1990-05-05','F',1,'01','01',1),
                    (11,'CC','112','Luis','Paz','1950-05-05','M',1,'02','02',1),
                    (20,'CC','221','Eva','Sol','2000-05-05','F',2,'01','01',2)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cita (id,institucion_id,paciente_id,medico_id,fecha,hora,estado,tipo)
             VALUES (100,1,10,5,'2026-03-10','08:00','ATENDIDA','PRIMERA_VEZ'),
                    (101,1,10,5,'2026-03-11','09:00','PROGRAMADA','CONTROL'),
                    (102,1,11,5,'2026-03-12','10:00','ATENDIDA','CONTROL'),
                    (200,2,20,6,'2026-03-10','08:00','ATENDIDA','PRIMERA_VEZ')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cie10 (codigo,descripcion,categoria) VALUES ('A09','Diarrea','A09')",
        ).execute(&pool).await.unwrap();
        // Las historias van con las DOS formas de fecha que existen en la base:
        // la del DEFAULT de SQLite (espacio) y la RFC 3339 que escribe el código.
        sqlx::query(
            "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,
                                   motivo_consulta,enfermedad_actual,diagnostico_principal,
                                   plan_tratamiento)
             VALUES (300,'2026-03-10 14:00:00','HC-1',10,5,'m','e','A09','p'),
                    (301,'2026-03-11T14:00:00Z','HC-2',11,5,'m','e','A09','p'),
                    (400,'2026-03-10 14:00:00','HC-3',20,6,'m','e','J00','p')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cama (id,institucion_id,numero,piso,servicio,estado)
             VALUES (1,1,'101','1','GENERAL','OCUPADA'),(2,1,'102','1','GENERAL','DISPONIBLE'),
                    (3,2,'101','1','GENERAL','OCUPADA')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,
                                          fecha_ingreso,estado)
             VALUES (1,10,1,5,'2026-03-10T12:00:00Z','ACTIVA')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO factura (id,institucion_id,numero_factura,entidad_id,fecha_factura,
                                  periodo_inicio,periodo_fin,valor_neto,estado)
             VALUES (1,1,'F-1',1,'2026-03-15','2026-03-01','2026-03-31',1000,'RADICADA'),
                    (2,2,'F-2',2,'2026-03-15','2026-03-01','2026-03-31',2000,'RADICADA')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO glosa (id,factura_id,item_glosado,motivo,valor_glosado,fecha_glosa,estado)
             VALUES (1,1,'x','y',100,'2026-05-01','PENDIENTE'),
                    (2,2,'x','y',700,'2026-05-01','PENDIENTE')",
        ).execute(&pool).await.unwrap();
        pool
    }

    const MARZO: (&str, &str) = ("2026-03-01", "2026-03-31");
    const UNO: Ambito = Ambito::Institucion(1);
    const DOS: Ambito = Ambito::Institucion(2);

    // ── El aislamiento, panel por panel ──────────────────────────────────────

    /// **La prueba que tiene que ponerse roja si alguien quita un filtro.**
    ///
    /// No comprueba «se ve algo»: comprueba que cada prestador ve LO SUYO y que
    /// la suma de los dos es mayor que lo que ve cada uno. Ese segundo asserto
    /// es el que discrimina — un panel que devolviera todo para todos pasaría
    /// cualquier prueba que solo mirase «el número es > 0».
    #[tokio::test]
    async fn ningun_panel_suma_las_dos_instituciones() {
        let pool = base().await;
        let (i, f) = MARZO;

        let c1 = citas(&pool, UNO, i, f).await.unwrap();
        let c2 = citas(&pool, DOS, i, f).await.unwrap();
        assert_eq!(c1.total, 3, "la 1 tiene tres citas en marzo");
        assert_eq!(c2.total, 1, "la 2 tiene una");
        assert!(
            !c1.by_medico.iter().any(|m| m.medico == "Beto Dos"),
            "el médico de la otra institución aparece en el panel de citas: {:?}",
            c1.by_medico
        );

        let p1 = pacientes(&pool, UNO).await.unwrap();
        let p2 = pacientes(&pool, DOS).await.unwrap();
        assert_eq!((p1.total, p2.total), (2, 1));
        assert!(
            !p1.by_eps.iter().any(|e| e.eps == "EPS Dos"),
            "la EPS de la otra institución se cuela: {:?}", p1.by_eps
        );

        let f1 = financiero(&pool, UNO, i, f).await.unwrap();
        let f2 = financiero(&pool, DOS, i, f).await.unwrap();
        assert_eq!(f1.total_valor, 1000.0, "la 1 facturó 1000, no 3000");
        assert_eq!(f2.total_valor, 2000.0, "la 2 facturó 2000, no 3000");
        assert_eq!(f1.valor_glosado_pendiente, 100.0, "las glosas se acotan por su factura");
        assert_eq!(f2.valor_glosado_pendiente, 700.0);

        let d1 = diagnosticos(&pool, UNO, i, f, 10).await.unwrap();
        assert_eq!(d1.diagnosticos.len(), 1, "solo el A09 de la institución 1");
        assert_eq!(d1.diagnosticos[0].cantidad, 2);
        assert!(
            !d1.diagnosticos.iter().any(|d| d.codigo == "J00"),
            "el diagnóstico de la otra institución se cuela"
        );

        let pr1 = produccion(&pool, UNO, i, f).await.unwrap();
        assert_eq!(pr1.medicos.len(), 1);
        assert_eq!(pr1.medicos[0].nombre, "Ana Uno");

        let r1 = resumen(&pool, UNO).await.unwrap();
        let r2 = resumen(&pool, DOS).await.unwrap();
        assert_eq!((r1.total_pacientes, r2.total_pacientes), (2, 1));
        assert_eq!(r1.hospitalizados, 1);
        assert_eq!(r2.hospitalizados, 0, "la hospitalización es de un paciente de la 1");
        assert_eq!(r1.facturas_pendientes, 1);
        assert_eq!(r1.valor_pendiente, 1000.0, "no 3000");
    }

    /// El superadmin SÍ ve la suma. Es la pareja del caso de arriba: sin ella,
    /// un filtro que negara a todo el mundo pasaría la prueba anterior.
    #[tokio::test]
    async fn el_superadmin_ve_las_dos() {
        let pool = base().await;
        let (i, f) = MARZO;
        assert_eq!(citas(&pool, Ambito::Superadmin, i, f).await.unwrap().total, 4);
        assert_eq!(pacientes(&pool, Ambito::Superadmin).await.unwrap().total, 3);
        assert_eq!(
            financiero(&pool, Ambito::Superadmin, i, f).await.unwrap().total_valor,
            3000.0
        );
        assert_eq!(resumen(&pool, Ambito::Superadmin).await.unwrap().total_pacientes, 3);
    }

    /// `Ambito::Ninguno` no ve NADA, en los ocho paneles.
    ///
    /// Es el caso que el lado Python trataba igual que al superadmin: los dos
    /// eran `institucion_id = None`, y las cinco consultas hacían
    /// `if sub is not None`, así que un usuario sin institución veía a todos los
    /// pacientes del sistema.
    #[tokio::test]
    async fn el_ambito_ninguno_no_ve_nada() {
        let pool = base().await;
        let (i, f) = MARZO;
        let n = Ambito::Ninguno;

        assert_eq!(citas(&pool, n, i, f).await.unwrap().total, 0);
        assert!(citas(&pool, n, i, f).await.unwrap().by_medico.is_empty());
        assert_eq!(pacientes(&pool, n).await.unwrap().total, 0);
        assert!(pacientes(&pool, n).await.unwrap().by_eps.is_empty());
        assert!(diagnosticos(&pool, n, i, f, 10).await.unwrap().diagnosticos.is_empty());
        assert!(produccion(&pool, n, i, f).await.unwrap().medicos.is_empty());
        assert_eq!(financiero(&pool, n, i, f).await.unwrap().total_valor, 0.0);
        assert!(financiero(&pool, n, i, f).await.unwrap().glosas.is_empty());
        assert_eq!(urgencias(&pool, n, i, f).await.unwrap().total, 0);

        let r = resumen(&pool, n).await.unwrap();
        assert_eq!(r.total_pacientes, 0);
        assert_eq!(r.citas_mes, 0);
        assert_eq!(r.valor_pendiente, 0.0);
        assert_eq!(r.hospitalizados, 0);
        assert_eq!(kpis(&pool, n).await.unwrap().total_pacientes, 0);
    }

    // ── Las fechas ───────────────────────────────────────────────────────────

    /// **Los dos formatos de la base se filtran igual, y el borde se respeta.**
    ///
    /// `atencion_urgencias.fecha_ingreso` se escribe RFC 3339 (`…T04:00:00Z`) y
    /// `historia.fecha` con el `DEFAULT` de SQLite (`… 04:00:00`). Comparando en
    /// crudo, `'T'` (0x54) va después del espacio (0x20) y una fila de fuera del
    /// rango entra igual.
    ///
    /// Los cuatro casos son la pareja: DOS que tienen que verse y DOS que no,
    /// uno de cada formato. Sin `replace(col,'T',' ')` el primero se cuela.
    #[tokio::test]
    async fn una_fecha_rfc3339_y_una_con_espacio_se_filtran_igual() {
        let pool = base().await;
        // Ventana: marzo en Bogotá = 2026-03-01 05:00 UTC .. 2026-04-01 05:00 UTC.
        sqlx::query(
            "INSERT INTO atencion_urgencias (id,paciente_id,triage,motivo_consulta,
                                             enfermera_triage_id,fecha_ingreso)
             VALUES (1,10,'TRIAGE_1','m',5,'2026-03-01T04:00:00Z'),
                    (2,10,'TRIAGE_2','m',5,'2026-03-01T06:00:00Z'),
                    (3,10,'TRIAGE_3','m',5,'2026-03-31 20:00:00'),
                    (4,10,'TRIAGE_4','m',5,'2026-04-01T06:00:00Z')",
        ).execute(&pool).await.unwrap();

        let u = urgencias(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(
            u.total, 2,
            "esperadas la 2 (RFC 3339 dentro) y la 3 (espacio dentro). \
             La 1 es el 28-FEB a las 23:00 en Bogotá y la 4 es de abril: \
             si salen 3, la comparación no está normalizando la 'T'. by_triage={:?}",
            u.by_triage.claves()
        );
        assert_eq!(u.by_triage.get("Naranja (II)"), Some(1));
        assert_eq!(u.by_triage.get("Amarillo (III)"), Some(1));
        assert_eq!(u.by_triage.get("Rojo (I)"), None, "la del 28 de febrero no entra");
        assert_eq!(u.by_triage.get("Verde (IV)"), None, "la de abril tampoco");
    }

    /// Y el mismo par sobre `historia`, que es la otra columna de instante: el
    /// panel de diagnósticos cuenta las dos formas.
    #[tokio::test]
    async fn el_panel_de_diagnosticos_ve_las_dos_formas_de_fecha() {
        let pool = base().await;
        let d = diagnosticos(&pool, UNO, MARZO.0, MARZO.1, 10).await.unwrap();
        assert_eq!(
            d.diagnosticos[0].cantidad, 2,
            "una historia se guardó con espacio y la otra en RFC 3339: cuentan las dos"
        );
        assert_eq!(d.diagnosticos[0].descripcion, "Diarrea", "la descripción sale del CIE-10");
    }

    /// Un diagnóstico que no está en el maestro muestra su CÓDIGO, no un hueco.
    #[tokio::test]
    async fn un_diagnostico_sin_maestro_muestra_su_codigo() {
        let pool = base().await;
        let d = diagnosticos(&pool, DOS, MARZO.0, MARZO.1, 10).await.unwrap();
        assert_eq!(d.diagnosticos.len(), 1);
        assert_eq!(d.diagnosticos[0].codigo, "J00");
        assert_eq!(d.diagnosticos[0].descripcion, "J00", "sin CIE-10 se muestra el código");
    }

    /// La ventana se pide en fecha LOCAL y la base guarda UTC: los límites se
    /// convierten, no se comparan crudos.
    #[test]
    fn los_limites_del_periodo_van_en_utc() {
        let (desde, hasta) = limites_utc("2026-03-01", "2026-03-31");
        assert_eq!(desde, "2026-03-01 05:00:00", "medianoche en Bogotá son las 05:00 UTC");
        assert_eq!(
            hasta, "2026-04-01 05:00:00",
            "el límite superior es EXCLUSIVO y cae en la medianoche del día siguiente, \
             para que el 31 entre entero"
        );
    }

    #[test]
    fn una_fecha_ilegible_no_se_sustituye_por_hoy() {
        assert_eq!(texto_utc("no-es-fecha", 0), "no-es-fecha");
    }

    // ── El mapa ordenado ─────────────────────────────────────────────────────

    /// **El orden del triage es clínico, no alfabético.**
    ///
    /// Con un `BTreeMap` esta prueba se pone roja: daría Amarillo, Azul,
    /// Naranja, Rojo, Verde. Con un `HashMap`, daría un orden distinto en cada
    /// corrida. Es lo que justifica el tipo `MapaOrdenado`.
    #[tokio::test]
    async fn el_triage_sale_en_orden_de_gravedad() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO atencion_urgencias (id,paciente_id,triage,motivo_consulta,
                                             enfermera_triage_id,fecha_ingreso)
             VALUES (1,10,'TRIAGE_5','m',5,'2026-03-10T14:00:00Z'),
                    (2,10,'TRIAGE_1','m',5,'2026-03-10T14:00:00Z'),
                    (3,10,'TRIAGE_3','m',5,'2026-03-10T14:00:00Z')",
        ).execute(&pool).await.unwrap();
        let u = urgencias(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(
            u.by_triage.claves(),
            vec!["Rojo (I)", "Amarillo (III)", "Azul (V)"],
            "de más grave a menos; alfabéticamente sería Amarillo, Azul, Rojo"
        );
    }

    /// Y el JSON conserva ese orden. Es la mitad que cruza la frontera: el
    /// `Vec` interno podría estar bien ordenado y `Serialize` emitirlo ordenado
    /// alfabéticamente igual.
    #[test]
    fn el_json_del_mapa_conserva_el_orden_de_insercion() {
        let m = MapaOrdenado::nuevo(vec![
            ("Rojo (I)".into(), 3),
            ("Azul (V)".into(), 1),
            ("Amarillo (III)".into(), 2),
        ]);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, r#"{"Rojo (I)":3,"Azul (V)":1,"Amarillo (III)":2}"#);
        assert!(
            json.find("Rojo").unwrap() < json.find("Amarillo").unwrap(),
            "si sale alfabético, el serializador está ordenando por su cuenta: {json}"
        );
    }

    /// Los nombres del enum son `TRIAGE_1..5`, no los colores.
    ///
    /// El lado Python escribió `TipoTriage.ROJO`, que no existe, y ese panel
    /// respondió 500 a todo el mundo hasta el 2026-08-06 sin que nada fallara:
    /// ninguna prueba lo invocaba. El color va en la ETIQUETA, no en la clave.
    #[test]
    fn las_claves_del_triage_son_las_del_esquema() {
        for (clave, _) in TRIAGE {
            assert!(
                clave.starts_with("TRIAGE_"),
                "«{clave}» no es un valor que la tabla acepte: el CHECK de \
                 atencion_urgencias solo admite TRIAGE_1..TRIAGE_5"
            );
        }
        assert_eq!(etiquetar(TRIAGE, Some("TRIAGE_1")), "Rojo (I)");
        assert_eq!(etiquetar(TRIAGE, Some("ROJO")), "ROJO", "lo desconocido se muestra tal cual");
        assert_eq!(etiquetar(TRIAGE, None), SIN_DATO);
    }

    // ── Edades ───────────────────────────────────────────────────────────────

    /// Los seis tramos salen SIEMPRE, y el reparto respeta los bordes.
    #[test]
    fn los_tramos_de_edad_reparten_por_los_bordes() {
        use chrono::Datelike;
        let hoy = zona::hoy();
        // El cumpleaños EXACTO de hace `anios` años: así la edad es `anios` y no
        // «casi». Con `365 * anios + bisiestos` el redondeo movía un caso de
        // tramo, que es justo lo que esta prueba mide.
        let hace = |anios: i32| {
            hoy.with_year(hoy.year() - anios)
                // 29 de febrero: el año no bisiesto no lo tiene. Se usa el 28,
                // que es el día que la ley civil toma para cumplir años.
                .unwrap_or_else(|| hoy.with_day(28).unwrap().with_year(hoy.year() - anios).unwrap())
                .format("%Y-%m-%d")
                .to_string()
        };
        let fechas = [hace(5), hace(11), hace(17), hace(18), hace(59), hace(60), hace(90)];
        let m = grupos_de_edad(fechas.iter().map(|s| s.as_str()));
        assert_eq!(m.claves(), vec!["0-10", "11-17", "18-29", "30-44", "45-59", "60+"]);
        assert_eq!(m.get("0-10"), Some(1));
        assert_eq!(m.get("11-17"), Some(2), "11 y 17 caen en el mismo tramo");
        assert_eq!(m.get("18-29"), Some(1));
        assert_eq!(m.get("30-44"), Some(0), "el tramo vacío se emite igual");
        assert_eq!(m.get("45-59"), Some(1));
        assert_eq!(m.get("60+"), Some(2));
        assert_eq!(m.total(), 7);
    }

    /// El cumpleaños de mañana todavía no cuenta.
    #[test]
    fn la_edad_no_se_adelanta_al_cumpleanos() {
        use chrono::NaiveDate;
        let d = |a, m, x| NaiveDate::from_ymd_opt(a, m, x).unwrap();
        assert_eq!(edad_en(d(2000, 6, 15), d(2026, 6, 14)), 25, "un día antes: 25");
        assert_eq!(edad_en(d(2000, 6, 15), d(2026, 6, 15)), 26, "el mismo día: 26");
    }

    /// Una fecha de nacimiento ilegible NO se cuenta en ningún tramo, en vez de
    /// caer en uno por defecto: un paciente con el dato roto no es un recién
    /// nacido ni un anciano.
    #[test]
    fn una_fecha_de_nacimiento_rota_no_cae_en_ningun_tramo() {
        let m = grupos_de_edad(["1990-01-01", "vacío", ""].into_iter());
        assert_eq!(m.total(), 1);
    }

    /// Una fecha de nacimiento FUTURA tampoco. En el Python cae en «0-10» y
    /// queda indistinguible de un recién nacido.
    #[test]
    fn un_nacimiento_en_el_futuro_no_es_un_recien_nacido() {
        let futuro = (zona::hoy() + chrono::Duration::days(400)).to_string();
        let m = grupos_de_edad(["1990-01-01", futuro.as_str()].into_iter());
        assert_eq!(m.get("0-10"), Some(0), "el de 2027 se contó como bebé");
        assert_eq!(m.total(), 1);
        // Y la pareja: el de HOY sí es un recién nacido de verdad.
        let hoy = zona::hoy().to_string();
        let m = grupos_de_edad([hoy.as_str()].into_iter());
        assert_eq!(m.get("0-10"), Some(1));
    }

    // ── Aritmética ───────────────────────────────────────────────────────────

    #[test]
    fn un_porcentaje_sin_denominador_es_cero_y_no_nan() {
        assert_eq!(porcentaje(0, 0), 0.0);
        assert_eq!(porcentaje(5, 0), 0.0, "y tampoco es infinito");
        assert_eq!(porcentaje(1, 3), 33.3, "un decimal");
        assert_eq!(porcentaje(2, 2), 100.0);
    }

    #[test]
    fn el_inicio_de_mes_sale_de_la_fecha_del_prestador() {
        assert_eq!(inicio_de_mes("2026-03-17"), "2026-03-01");
        assert_eq!(inicio_de_mes("2026-12-31"), "2026-12-01");
    }

    // ── Contenido de los paneles ─────────────────────────────────────────────

    /// El panel de citas trae sus cuatro cortes con datos reales. Es el positivo
    /// de control de todos los assertos de aislamiento: sin él, un módulo que
    /// devolviera vacío siempre pasaría la mitad de este banco.
    #[tokio::test]
    async fn el_panel_de_citas_trae_sus_cuatro_cortes() {
        let pool = base().await;
        let c = citas(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(c.periodo, Periodo { inicio: "2026-03-01".into(), fin: "2026-03-31".into() });
        assert_eq!(c.by_estado.get("ATENDIDA"), Some(2));
        assert_eq!(c.by_estado.get("PROGRAMADA"), Some(1));
        assert_eq!(c.by_tipo.get("CONTROL"), Some(2));
        assert_eq!(c.tendencia.len(), 3, "tres días distintos");
        assert_eq!(c.tendencia[0].fecha, "2026-03-10", "y en orden");
        assert_eq!(c.by_medico.len(), 1);
        assert_eq!(c.by_medico[0].cantidad, 3);
    }

    /// Un periodo de más de 60 días agrupa por mes en vez de por día.
    #[tokio::test]
    async fn una_ventana_larga_agrupa_por_mes() {
        let pool = base().await;
        let c = citas(&pool, UNO, "2026-01-01", "2026-12-31").await.unwrap();
        assert_eq!(c.tendencia.len(), 1, "las tres citas son del mismo mes");
        assert_eq!(c.tendencia[0].fecha, "2026-03", "y el punto se llama por el mes");
        assert_eq!(c.tendencia[0].cantidad, 3);
    }

    #[tokio::test]
    async fn el_panel_de_pacientes_reparte_por_sexo_regimen_y_zona() {
        let pool = base().await;
        let p = pacientes(&pool, UNO).await.unwrap();
        assert_eq!(p.by_sexo.get("F"), Some(1));
        assert_eq!(p.by_sexo.get("M"), Some(1));
        assert_eq!(p.by_tipo_usuario.get("Contributivo"), Some(1));
        assert_eq!(p.by_tipo_usuario.get("Subsidiado"), Some(1));
        assert_eq!(p.by_zona.get("Urbana"), Some(1));
        assert_eq!(p.by_zona.get("Rural"), Some(1));
        assert_eq!(p.by_eps.len(), 1);
        assert_eq!(p.by_eps[0].eps, "EPS Uno");
    }

    /// Un régimen que no está en la tabla del MUV se muestra tal cual, y la
    /// AUSENCIA se nombra en vez de inventarse.
    #[tokio::test]
    async fn el_regimen_ausente_se_nombra_y_el_desconocido_se_muestra() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,
                                   fecha_nacimiento,sexo,institucion_id,tipo_usuario)
             VALUES (12,'CC','113','Sin','Regimen','1990-01-01','M',1,NULL),
                    (13,'CC','114','Raro','Codigo','1990-01-01','M',1,'99')",
        ).execute(&pool).await.unwrap();
        let p = pacientes(&pool, UNO).await.unwrap();
        assert_eq!(p.by_tipo_usuario.get(SIN_DATO), Some(1));
        assert_eq!(p.by_tipo_usuario.get("99"), Some(1), "el código crudo, no «Otro»");
        // Y lo desconocido va al final, después de los cinco del MUV.
        let claves = p.by_tipo_usuario.claves();
        assert!(
            claves.iter().position(|c| *c == "Contributivo")
                < claves.iter().position(|c| *c == "99"),
            "lo que no está en la tabla va después: {claves:?}"
        );
    }

    #[tokio::test]
    async fn la_produccion_junta_citas_e_historias_del_mismo_medico() {
        let pool = base().await;
        let p = produccion(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(p.medicos.len(), 1);
        let m = &p.medicos[0];
        assert_eq!(m.medico_id, 5);
        assert_eq!(m.citas_atendidas, 2, "dos citas ATENDIDA");
        assert_eq!(m.historias_creadas, 2, "dos historias, una de cada formato de fecha");
        assert_eq!(m.especialidad, None, "ausente, no inventada");
    }

    /// Un médico que solo escribió historias aparece igual, con cero citas.
    #[tokio::test]
    async fn un_medico_sin_citas_pero_con_historias_aparece() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (7,'Zoe Uno','z@x.co','h','medico',1,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,
                                   motivo_consulta,enfermedad_actual,diagnostico_principal,
                                   plan_tratamiento)
             VALUES (302,'2026-03-12T10:00:00Z','HC-9',10,7,'m','e','A09','p')",
        ).execute(&pool).await.unwrap();
        let p = produccion(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(p.medicos.len(), 2);
        let zoe = p.medicos.iter().find(|m| m.medico_id == 7).expect("Zoe no aparece");
        assert_eq!((zoe.citas_atendidas, zoe.historias_creadas), (0, 1));
        assert_eq!(p.medicos[0].medico_id, 5, "ordenados por citas atendidas");
    }

    #[tokio::test]
    async fn el_financiero_trae_estados_tendencia_y_glosas() {
        let pool = base().await;
        let f = financiero(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(f.by_estado.len(), 1);
        assert_eq!(f.by_estado[0].estado, "RADICADA");
        assert_eq!(f.by_estado[0].valor, 1000.0);
        assert_eq!(f.tendencia_mensual.len(), 1);
        assert_eq!(f.tendencia_mensual[0].periodo, "2026-03");
        assert_eq!(f.glosas.len(), 1);
        assert_eq!(f.glosas[0].valor, 100.0);
    }

    /// **La glosa de una factura VIEJA sigue contando.**
    ///
    /// Es la regresión que el lado Python tuvo durante un commit: acotar las
    /// glosas por `fecha_factura` las hace desaparecer, porque llegan meses
    /// después de radicar. La ventana es marzo y la factura es de enero.
    #[tokio::test]
    async fn una_glosa_sobre_una_factura_vieja_no_desaparece() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO factura (id,institucion_id,numero_factura,entidad_id,fecha_factura,
                                  periodo_inicio,periodo_fin,valor_neto,estado)
             VALUES (3,1,'F-3',1,'2025-01-15','2025-01-01','2025-01-31',500,'GLOSADA')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO glosa (id,factura_id,item_glosado,motivo,valor_glosado,fecha_glosa,estado)
             VALUES (3,3,'x','y',450,'2026-03-01','PENDIENTE')",
        ).execute(&pool).await.unwrap();

        let f = financiero(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(
            f.valor_glosado_pendiente, 550.0,
            "la glosa de la factura de enero de 2025 tiene que seguir contando: \
             una glosa viva no se cae del panel porque su factura sea vieja"
        );
        assert_eq!(
            f.total_valor, 1000.0,
            "y la factura de 2025 NO entra en el valor del periodo: eso sí va por fecha"
        );
    }

    /// **Una cita de diciembre NO es una cita «de este mes».**
    ///
    /// El Python cuenta `fecha >= inicio_mes` sin tope, así que la tarjeta de
    /// portada mete el futuro entero. Y como las atendidas solo pueden ser
    /// pasado, la tasa de asistencia BAJA cuanto más se agende con antelación.
    /// Con el código anterior esta prueba se pone roja: `citas_mes` sale 2 y la
    /// tasa, 50 %.
    #[tokio::test]
    async fn una_cita_del_mes_que_viene_no_cuenta_como_de_este_mes() {
        let pool = base().await;
        let hoy = hoy();
        let mes = &hoy[..7];
        sqlx::query(
            "INSERT INTO cita (id,institucion_id,paciente_id,medico_id,fecha,hora,estado)
             VALUES (300,1,10,5,?,'08:00','ATENDIDA'),
                    (301,1,10,5,'2099-12-31','09:00','PROGRAMADA')",
        )
        .bind(format!("{mes}-01"))
        .execute(&pool).await.unwrap();

        let r = resumen(&pool, UNO).await.unwrap();
        assert_eq!(
            r.citas_mes, 1,
            "la cita de 2099 se coló en «este mes»: el filtro no tiene tope superior"
        );
        assert_eq!(r.citas_atendidas_mes, 1);
        assert_eq!(r.tasa_asistencia, 100.0, "una de una atendida es el 100 %, no el 50 %");
    }

    /// El último día del mes se calcula, no se tabula: febrero y los bisiestos.
    #[test]
    fn el_fin_de_mes_acierta_en_febrero_y_en_diciembre() {
        assert_eq!(fin_de_mes("2026-02-01"), "2026-02-28");
        assert_eq!(fin_de_mes("2024-02-01"), "2024-02-29", "2024 es bisiesto");
        assert_eq!(fin_de_mes("2026-12-01"), "2026-12-31", "diciembre salta de año");
        assert_eq!(fin_de_mes("2026-04-01"), "2026-04-30");
        assert_eq!(fin_de_mes("no-es-fecha"), "no-es-fecha", "no se inventa un mes");
    }

    /// El tope del top-N se enlaza como NÚMERO y de verdad recorta.
    #[tokio::test]
    async fn el_tope_de_diagnosticos_recorta_y_se_acota() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,
                                   motivo_consulta,enfermedad_actual,diagnostico_principal,
                                   plan_tratamiento)
             VALUES (310,'2026-03-12 10:00:00','HC-A',10,5,'m','e','B01','p'),
                    (311,'2026-03-13 10:00:00','HC-B',10,5,'m','e','C02','p')",
        ).execute(&pool).await.unwrap();

        let d = diagnosticos(&pool, UNO, MARZO.0, MARZO.1, 1).await.unwrap();
        assert_eq!(d.diagnosticos.len(), 1, "top=1 devuelve uno");
        assert_eq!(d.diagnosticos[0].codigo, "A09", "y es el más frecuente");
        assert_eq!(diagnosticos(&pool, UNO, MARZO.0, MARZO.1, 10).await.unwrap().diagnosticos.len(), 3);
        // El tope se acota a [1,30]: un cero o un negativo no vacían el panel.
        assert_eq!(diagnosticos(&pool, UNO, MARZO.0, MARZO.1, 0).await.unwrap().diagnosticos.len(), 1);
        assert_eq!(diagnosticos(&pool, UNO, MARZO.0, MARZO.1, -5).await.unwrap().diagnosticos.len(), 1);
    }

    /// **Las tres cifras de camas NO suman, y es correcto.** `total_camas` son
    /// las camas INSTALADAS —los cinco estados—, mientras que ocupadas y
    /// disponibles son dos de ellos: una cama en aseo no es ninguna de las dos.
    /// La tasa de ocupación es sobre las instaladas, que es como se mide en un
    /// hospital. Se fija con una prueba para que sea una decisión y no una
    /// casualidad de la base de ejemplo, donde solo había dos estados.
    #[tokio::test]
    async fn una_cama_en_aseo_cuenta_en_el_total_y_en_ninguna_de_las_dos() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cama (id,institucion_id,numero,piso,servicio,estado)
             VALUES (4,1,'103','1','GENERAL','EN_ASEO'),
                    (5,1,'104','1','GENERAL','MANTENIMIENTO')",
        ).execute(&pool).await.unwrap();
        let r = resumen(&pool, UNO).await.unwrap();
        assert_eq!(r.total_camas, 4, "las cuatro instaladas de la institución 1");
        assert_eq!(r.camas_ocupadas, 1);
        assert_eq!(r.camas_disponibles, 1);
        assert_eq!(
            r.camas_ocupadas + r.camas_disponibles, 2,
            "y NO suman el total: dos camas están en aseo y en mantenimiento"
        );
        assert_eq!(r.tasa_ocupacion, 25.0, "1 de 4 instaladas");
    }

    #[tokio::test]
    async fn el_resumen_cuenta_camas_y_ocupacion() {
        let pool = base().await;
        let r = resumen(&pool, UNO).await.unwrap();
        assert_eq!((r.total_camas, r.camas_ocupadas, r.camas_disponibles), (2, 1, 1));
        assert_eq!(r.tasa_ocupacion, 50.0);
        assert_eq!(r.fecha, hoy(), "la fecha es la del prestador");
    }

    /// El tiempo de estancia sale de las dos marcas, y las dos formas de fecha
    /// se restan igual.
    #[tokio::test]
    async fn la_estancia_se_deriva_de_ingreso_y_egreso() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO atencion_urgencias (id,paciente_id,triage,motivo_consulta,
                                             enfermera_triage_id,fecha_ingreso,
                                             destino_salida,fecha_egreso)
             VALUES (1,10,'TRIAGE_2','m',5,'2026-03-10T10:00:00Z','DOMICILIO','2026-03-10T12:00:00Z'),
                    (2,10,'TRIAGE_3','m',5,'2026-03-11 10:00:00','REMISION','2026-03-11T11:00:00Z'),
                    (3,10,'TRIAGE_4','m',5,'2026-03-12T10:00:00Z',NULL,NULL)",
        ).execute(&pool).await.unwrap();
        let u = urgencias(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(u.total, 3);
        assert_eq!(
            u.estancia_promedio_min, 90.0,
            "media de 120 y 60 minutos; la tercera no tiene egreso y no entra"
        );
        assert_eq!(u.by_destino.get("DOMICILIO"), Some(1));
        assert_eq!(u.by_destino.get("REMISION"), Some(1));
        assert_eq!(u.by_destino.len(), 2, "la atención abierta no es un destino");
    }

    /// Sin ninguna atención cerrada la media es 0.0 y no `null`: el frontal la
    /// pinta como número.
    #[tokio::test]
    async fn sin_egresos_la_estancia_media_es_cero() {
        let pool = base().await;
        let u = urgencias(&pool, UNO, MARZO.0, MARZO.1).await.unwrap();
        assert_eq!(u.total, 0);
        assert_eq!(u.estancia_promedio_min, 0.0);
    }

    #[tokio::test]
    async fn los_kpis_son_los_del_resumen() {
        let pool = base().await;
        let r = resumen(&pool, UNO).await.unwrap();
        let k = kpis(&pool, UNO).await.unwrap();
        assert_eq!(
            (k.citas_hoy, k.citas_mes, k.total_pacientes, k.fecha),
            (r.citas_hoy, r.citas_mes, r.total_pacientes, r.fecha)
        );
    }
}
