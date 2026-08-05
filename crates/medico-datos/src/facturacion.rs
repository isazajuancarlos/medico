//! Facturación: contratos, facturas, sus líneas y las glosas.
//!
//! # Esto NO es una traducción del router de Python
//!
//! El módulo vivo (`backend/routers/facturacion.py`, 17 endpoints) **no está
//! aislado entre instituciones**, y no en un sitio sino en cinco. Van
//! enumerados porque cada uno es una decisión distinta al portarlo, no un
//! descuido repetido:
//!
//! | dónde | qué hace el Python | qué se hace aquí |
//! |---|---|---|
//! | `/stats` y `/facturas` | llama a `filtrar_por_institucion(Factura)`, que es un **no-op**: solo añade el `WHERE` `if hasattr(model, "institucion_id")`, y `Factura` no la tiene | la columna existe (migración 0044) y el filtro se aplica siempre |
//! | `/stats`, las glosas | `select(Glosa).where(estado=="PENDIENTE")` **sin filtro ninguno** | se acota por la factura de la que cuelga |
//! | `/glosas` | `select(Glosa)` **sin filtro**, y encima enriquece con el `numero_factura` ajeno | `JOIN factura` y filtro por su institución |
//! | `/contratos` | `select(Contrato)` **sin filtro** | `contrato` tiene dueño y se filtra |
//! | numeración | `FAC-{año}-{n}` con una secuencia **global** | por institución |
//!
//! Lo que se expone en el Python no son números sueltos: `detalle_factura` lleva
//! `paciente_id`, `historia_id` y el CUPS del procedimiento. Es qué se le
//! facturó a qué paciente en la clínica de al lado.
//!
//! # La numeración es POR INSTITUCIÓN, y esto no es cosmético
//!
//! El generador del Python busca el último `FAC-2026-%` de **toda la base**. Con
//! dos prestadores dentro, eso hace tres cosas malas a la vez: los números
//! saltan sin explicación (el 00007 va después del 00004 porque los tres de en
//! medio son del vecino), **el propio número revela cuántas facturas emitió el
//! otro**, y el `UNIQUE(institucion_id, numero_factura)` de la 0044 deja de
//! tener sentido. Aquí la secuencia se calcula dentro del ámbito y dentro de la
//! misma transacción que inserta.
//!
//! # Los importes van en `f64`, y es deuda conocida
//!
//! Dinero en coma flotante. Se conserva porque el Python usa `float` y el port
//! tiene que poder compararse contra él fila a fila —es la regla de fidelidad
//! del proyecto—, y cambiarlo ahora rompería la comparación que autoriza a
//! apagar el Python. Está dicho también en la migración 0044. No se «mejora» de
//! paso.

use medico_dominio::facturacion::EstadoFactura;
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

use crate::{filtro_ambito, hoy};

// ─────────────────────────────────────────────────────────────────────────────
// Tipos que salen
// ─────────────────────────────────────────────────────────────────────────────

/// Catálogo nacional: una EPS es la misma persona jurídica para todos los
/// prestadores, así que no lleva institución (ver la 0044).
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Entidad {
    pub id: i64,
    pub nit: String,
    pub nombre: String,
    pub codigo_eps: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Contrato {
    pub id: i64,
    pub entidad_id: i64,
    pub entidad_nombre: String,
    pub nombre: String,
    pub numero_contrato: String,
    pub fecha_inicio: String,
    pub fecha_fin: String,
    pub activo: bool,
    pub tipo_tarifa: String,
    pub porcentaje_incremento: f64,
    pub observaciones: Option<String>,
}

/// Lo que va en el listado. **Sin líneas ni glosas**: el listado del Python las
/// omite a propósito (`include_detalles=False`) y traerlas por cada fila serían
/// dos consultas por factura para pintar una tabla.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct FacturaResumen {
    pub id: i64,
    pub numero_factura: String,
    pub entidad_id: i64,
    pub entidad_nombre: String,
    pub contrato_id: Option<i64>,
    pub contrato_nombre: Option<String>,
    pub fecha_factura: String,
    pub fecha_radicado: Option<String>,
    pub num_radicado: Option<String>,
    pub periodo_inicio: String,
    pub periodo_fin: String,
    pub valor_bruto: f64,
    pub descuentos: f64,
    pub valor_neto: f64,
    pub estado: String,
    pub observaciones: Option<String>,
    pub glosada: bool,
    pub total_detalles: i64,
    pub glosas_pendientes: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Detalle {
    pub id: i64,
    pub servicio: String,
    pub codigo_cups: Option<String>,
    pub cantidad: i64,
    pub valor_unitario: f64,
    pub valor_total: f64,
    pub paciente_id: Option<i64>,
    pub paciente_nombre: Option<String>,
    pub historia_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Glosa {
    pub id: i64,
    pub factura_id: i64,
    pub numero_factura: String,
    pub item_glosado: String,
    pub motivo: String,
    pub valor_glosado: f64,
    pub fecha_glosa: String,
    pub respuesta: Option<String>,
    pub fecha_respuesta: Option<String>,
    pub estado: String,
}

/// La factura con todo lo que cuelga de ella. Es lo que devuelve el detalle y
/// todas las rutas que la modifican.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FacturaCompleta {
    #[serde(flatten)]
    pub cabecera: FacturaResumen,
    pub detalles: Vec<Detalle>,
    pub glosas: Vec<Glosa>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub total_facturas: i64,
    pub borradores: i64,
    pub radicadas: i64,
    pub aceptadas: i64,
    pub pagadas: i64,
    pub glosadas: i64,
    pub anuladas: i64,
    pub valor_total_radicado: f64,
    pub valor_total_pagado: f64,
    pub glosas_pendientes: i64,
    pub valor_glosado_pendiente: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipos que entran
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltrosFactura {
    pub estado: Option<String>,
    pub entidad_id: Option<i64>,
    /// Busca en el número de factura.
    pub q: Option<String>,
    #[serde(default = "pagina_por_defecto")]
    pub page: i64,
    #[serde(default = "tamanio_por_defecto")]
    pub size: i64,
}

fn pagina_por_defecto() -> i64 {
    1
}
fn tamanio_por_defecto() -> i64 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetalleNuevo {
    pub servicio: String,
    /// Sin defecto, y es deliberado: el Python trae `default="890201"`, un CUPS
    /// con la forma correcta que nadie afirmó. Si no se sabe, va vacío y el
    /// recolector de faltantes lo anota.
    pub codigo_cups: Option<String>,
    #[serde(default = "cantidad_por_defecto")]
    pub cantidad: i64,
    #[serde(default)]
    pub valor_unitario: f64,
    pub paciente_id: Option<i64>,
    pub historia_id: Option<i64>,
}

fn cantidad_por_defecto() -> i64 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct FacturaNueva {
    pub entidad_id: i64,
    pub contrato_id: Option<i64>,
    pub periodo_inicio: String,
    pub periodo_fin: String,
    #[serde(default)]
    pub descuentos: f64,
    pub observaciones: Option<String>,
    #[serde(default)]
    pub detalles: Vec<DetalleNuevo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FacturaEdicion {
    pub entidad_id: Option<i64>,
    pub contrato_id: Option<i64>,
    pub periodo_inicio: Option<String>,
    pub periodo_fin: Option<String>,
    pub descuentos: Option<f64>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Radicacion {
    pub num_radicado: Option<String>,
    /// Sin fecha se toma la de hoy **en la zona del prestador**, no la del reloj
    /// de la máquina: entre las 19:00 y la medianoche de Bogotá, `Utc` daría el
    /// día siguiente y la factura quedaría radicada mañana.
    pub fecha_radicado: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlosaNueva {
    pub item_glosado: String,
    pub motivo: String,
    #[serde(default)]
    pub valor_glosado: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RespuestaGlosa {
    pub respuesta: String,
    pub aceptar: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContratoNuevo {
    pub entidad_id: i64,
    pub nombre: String,
    pub numero_contrato: String,
    pub fecha_inicio: String,
    pub fecha_fin: String,
    pub tipo_tarifa: String,
    #[serde(default)]
    pub porcentaje_incremento: f64,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContratoEdicion {
    pub nombre: Option<String>,
    pub fecha_inicio: Option<String>,
    pub fecha_fin: Option<String>,
    pub activo: Option<bool>,
    pub tipo_tarifa: Option<String>,
    pub porcentaje_incremento: Option<f64>,
    pub observaciones: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Resultados
// ─────────────────────────────────────────────────────────────────────────────

/// Qué pasó al escribir.
///
/// **`NoExiste` cubre también «es de otra institución»**, y esa fusión es el
/// diseño, no un atajo: un 403 confirmaría que la factura existe, y con eso se
/// enumera. Es la misma razón por la que [`medico_dominio::Visibilidad`] no
/// tiene variante `Prohibido`.
#[derive(Debug, Clone, PartialEq)]
pub enum Resultado {
    Hecho,
    NoExiste,
    /// Existe y es del ámbito, pero su estado no admite la operación.
    EstadoNoAdmite {
        estado: String,
    },
    /// La transición pedida no está en la máquina de estados.
    TransicionInvalida {
        desde: String,
        hacia: String,
        permitidas: Vec<String>,
    },
    /// Radicar una factura sin valor.
    SinValor,
    /// La entidad o el contrato que se nombra no existe, o el contrato es de
    /// otra institución.
    ReferenciaInvalida {
        que: &'static str,
    },
    /// Ya hay un contrato con ese número **en esta institución**.
    NumeroRepetido {
        numero: String,
    },
    /// Un texto obligatorio llegó vacío.
    FaltaTexto {
        campo: &'static str,
    },
}

/// Igual que [`Resultado`], pero llevando el identificador de lo creado.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoAlta {
    Creado(i64),
    Fallo(Resultado),
}

// ─────────────────────────────────────────────────────────────────────────────
// Consultas
// ─────────────────────────────────────────────────────────────────────────────

const COLS_FACTURA: &str = "f.id, f.numero_factura, f.entidad_id, e.nombre AS entidad_nombre, \
    f.contrato_id, c.nombre AS contrato_nombre, f.fecha_factura, f.fecha_radicado, \
    f.num_radicado, f.periodo_inicio, f.periodo_fin, f.valor_bruto, f.descuentos, \
    f.valor_neto, f.estado, f.observaciones, f.glosada, \
    (SELECT COUNT(*) FROM detalle_factura d WHERE d.factura_id = f.id) AS total_detalles, \
    (SELECT COUNT(*) FROM glosa g WHERE g.factura_id = f.id AND g.estado = 'PENDIENTE') \
        AS glosas_pendientes";

const DE_FACTURA: &str = "FROM factura f \
    JOIN entidad e ON e.id = f.entidad_id \
    LEFT JOIN contrato c ON c.id = f.contrato_id";

pub async fn listar_facturas(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &FiltrosFactura,
) -> Result<Vec<FacturaResumen>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "f.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!("SELECT {COLS_FACTURA} {DE_FACTURA} WHERE {filtro}");
    if f.estado.is_some() {
        sql.push_str(" AND f.estado = ?");
    }
    if f.entidad_id.is_some() {
        sql.push_str(" AND f.entidad_id = ?");
    }
    if f.q.is_some() {
        sql.push_str(" AND f.numero_factura LIKE ?");
    }
    sql.push_str(" ORDER BY f.fecha_factura DESC, f.id DESC LIMIT ? OFFSET ?");

    // El tamaño se acota aquí y no solo en la ruta: una consulta sin techo es
    // una forma de sacar la base entera de un tirón, y este módulo tiene que
    // poder llamarse desde otro sitio sin heredar el descuido.
    let size = f.size.clamp(1, 100);
    let page = f.page.max(1);

    let mut q = sqlx::query_as::<_, FacturaResumen>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(e) = &f.estado {
        q = q.bind(e.clone());
    }
    if let Some(e) = f.entidad_id {
        q = q.bind(e);
    }
    if let Some(t) = &f.q {
        q = q.bind(format!("%{t}%"));
    }
    q.bind(size).bind((page - 1) * size).fetch_all(pool).await
}

async fn cabecera(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<FacturaResumen>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "f.institucion_id") else {
        return Ok(None);
    };
    let sql = format!("SELECT {COLS_FACTURA} {DE_FACTURA} WHERE f.id = ? AND {filtro}");
    let mut q = sqlx::query_as::<_, FacturaResumen>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

const COLS_DETALLE: &str = "d.id, d.servicio, d.codigo_cups, d.cantidad, d.valor_unitario, \
    d.valor_total, d.paciente_id, \
    TRIM(COALESCE(p.nombre1,'') || ' ' || COALESCE(p.nombre2,'') || ' ' || \
         COALESCE(p.apellido1,'') || ' ' || COALESCE(p.apellido2,'')) AS paciente_nombre, \
    d.historia_id";

const COLS_GLOSA: &str = "g.id, g.factura_id, f.numero_factura, g.item_glosado, g.motivo, \
    g.valor_glosado, g.fecha_glosa, g.respuesta, g.fecha_respuesta, g.estado";

/// La factura con sus líneas y sus glosas, o `None` si no existe **o no es de
/// esta institución**.
pub async fn obtener_factura(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<FacturaCompleta>, sqlx::Error> {
    let Some(cab) = cabecera(pool, ambito, id).await? else {
        return Ok(None);
    };
    // Las dos consultas de abajo NO repiten el filtro por institución, y es
    // correcto: cuelgan de `factura_id`, y esa factura ya pasó por `cabecera`,
    // que sí lo aplica. Es la misma regla que la 0044 escribe para explicar por
    // qué `detalle_factura` y `glosa` no llevan `institucion_id` — su dueño es
    // demostrable en un salto.
    let detalles = sqlx::query_as::<_, Detalle>(&format!(
        "SELECT {COLS_DETALLE} FROM detalle_factura d \
         LEFT JOIN paciente p ON p.id = d.paciente_id \
         WHERE d.factura_id = ? ORDER BY d.id"
    ))
    .bind(id)
    .fetch_all(pool)
    .await?;

    let glosas = sqlx::query_as::<_, Glosa>(&format!(
        "SELECT {COLS_GLOSA} FROM glosa g JOIN factura f ON f.id = g.factura_id \
         WHERE g.factura_id = ? ORDER BY g.fecha_glosa DESC, g.id DESC"
    ))
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(Some(FacturaCompleta {
        cabecera: cab,
        detalles,
        glosas,
    }))
}

pub async fn stats(pool: &crate::Pool, ambito: Ambito) -> Result<Stats, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "f.institucion_id") else {
        return Ok(Stats {
            total_facturas: 0,
            borradores: 0,
            radicadas: 0,
            aceptadas: 0,
            pagadas: 0,
            glosadas: 0,
            anuladas: 0,
            valor_total_radicado: 0.0,
            valor_total_pagado: 0.0,
            glosas_pendientes: 0,
            valor_glosado_pendiente: 0.0,
        });
    };

    #[derive(sqlx::FromRow)]
    struct FilaFacturas {
        total: i64,
        borradores: i64,
        radicadas: i64,
        aceptadas: i64,
        pagadas: i64,
        glosadas: i64,
        anuladas: i64,
        valor_radicado: f64,
        valor_pagado: f64,
    }

    let sql_facturas = format!(
        "SELECT COUNT(*) AS total, \
         COALESCE(SUM(estado = 'BORRADOR'),0) AS borradores, \
         COALESCE(SUM(estado = 'RADICADA'),0) AS radicadas, \
         COALESCE(SUM(estado = 'ACEPTADA'),0) AS aceptadas, \
         COALESCE(SUM(estado = 'PAGADA'),0)   AS pagadas, \
         COALESCE(SUM(estado = 'GLOSADA'),0)  AS glosadas, \
         COALESCE(SUM(estado = 'ANULADA'),0)  AS anuladas, \
         CAST(COALESCE(SUM(CASE WHEN estado IN ('RADICADA','ACEPTADA','GLOSADA') \
                                THEN valor_neto ELSE 0 END),0) AS REAL) AS valor_radicado, \
         CAST(COALESCE(SUM(CASE WHEN estado = 'PAGADA' THEN valor_neto ELSE 0 END),0) \
              AS REAL) AS valor_pagado \
         FROM factura f WHERE {filtro}"
    );
    let f = sqlx::query_as::<_, FilaFacturas>(&sql_facturas);
    let f = match inst {
        Some(i) => f.bind(i),
        None => f,
    }
    .fetch_one(pool)
    .await?;

    // Las glosas pendientes se acotan POR SU FACTURA. En el Python esta consulta
    // no lleva filtro ninguno, así que el panel de un prestador contaba —y
    // sumaba en dinero— las glosas de todos los demás.
    #[derive(sqlx::FromRow)]
    struct FilaGlosas {
        pendientes: i64,
        valor: f64,
    }
    let sql_glosas = format!(
        "SELECT COUNT(*) AS pendientes, CAST(COALESCE(SUM(g.valor_glosado),0) AS REAL) AS valor \
         FROM glosa g JOIN factura f ON f.id = g.factura_id \
         WHERE g.estado = 'PENDIENTE' AND {filtro}"
    );
    let g = sqlx::query_as::<_, FilaGlosas>(&sql_glosas);
    let g = match inst {
        Some(i) => g.bind(i),
        None => g,
    }
    .fetch_one(pool)
    .await?;

    Ok(Stats {
        total_facturas: f.total,
        borradores: f.borradores,
        radicadas: f.radicadas,
        aceptadas: f.aceptadas,
        pagadas: f.pagadas,
        glosadas: f.glosadas,
        anuladas: f.anuladas,
        valor_total_radicado: f.valor_radicado,
        valor_total_pagado: f.valor_pagado,
        glosas_pendientes: g.pendientes,
        valor_glosado_pendiente: g.valor,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Escrituras
// ─────────────────────────────────────────────────────────────────────────────

/// El siguiente `FAC-{año}-{n}` **de esta institución**.
///
/// Va dentro de la misma transacción que el `INSERT`, y por eso recibe el
/// ejecutor en vez del pool: calcularlo fuera dejaría la ventana en la que dos
/// altas simultáneas eligen el mismo número. Si aun así coincidieran, el
/// `UNIQUE(institucion_id, numero_factura)` de la 0044 es la red de abajo.
async fn siguiente_numero(
    tx: &mut sqlx::SqliteConnection,
    institucion_id: i64,
    anio: i32,
) -> Result<String, sqlx::Error> {
    let prefijo = format!("FAC-{anio}-");
    let ultimo: Option<String> = sqlx::query_scalar(
        "SELECT numero_factura FROM factura \
         WHERE institucion_id = ? AND numero_factura LIKE ? \
         ORDER BY numero_factura DESC LIMIT 1",
    )
    .bind(institucion_id)
    .bind(format!("{prefijo}%"))
    .fetch_optional(&mut *tx)
    .await?;

    let n = ultimo
        .and_then(|u| u.rsplit('-').next().and_then(|s| s.parse::<u32>().ok()))
        .map(|n| n + 1)
        .unwrap_or(1);
    Ok(format!("{prefijo}{n:05}"))
}

/// `bruto` es la suma de las líneas; `neto` nunca baja de cero.
async fn recalcular(
    tx: &mut sqlx::SqliteConnection,
    factura_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE factura SET \
           valor_bruto = COALESCE((SELECT SUM(valor_total) FROM detalle_factura \
                                   WHERE factura_id = ?), 0), \
           valor_neto  = MAX(0, COALESCE((SELECT SUM(valor_total) FROM detalle_factura \
                                          WHERE factura_id = ?), 0) - descuentos) \
         WHERE id = ?",
    )
    .bind(factura_id)
    .bind(factura_id)
    .bind(factura_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// El estado de una factura del ámbito, o `None` si no la ve.
async fn estado_de(
    tx: &mut sqlx::SqliteConnection,
    ambito: Ambito,
    id: i64,
) -> Result<Option<EstadoFactura>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(None);
    };
    let sql = format!("SELECT estado FROM factura WHERE id = ? AND {filtro}");
    let q = sqlx::query_scalar::<_, String>(&sql).bind(id);
    let q = match inst {
        Some(i) => q.bind(i),
        None => q,
    };
    // Un estado que no reconocemos NO se rellena con `Borrador`: se trata como
    // si la factura no estuviera, que es ruidoso y seguro. Lo escribió algo que
    // no es este código.
    Ok(q.fetch_optional(&mut *tx)
        .await?
        .and_then(|s| EstadoFactura::desde_texto(&s)))
}

/// Comprueba que la entidad exista y que el contrato —si se nombra— sea **de
/// esta institución**. Lo segundo el Python no lo mira: bastaba nombrar el
/// contrato de otro prestador para colgarle la factura.
async fn referencias_validas(
    tx: &mut sqlx::SqliteConnection,
    institucion_id: i64,
    entidad_id: i64,
    contrato_id: Option<i64>,
) -> Result<Option<&'static str>, sqlx::Error> {
    let hay_entidad: Option<i64> = sqlx::query_scalar("SELECT id FROM entidad WHERE id = ?")
        .bind(entidad_id)
        .fetch_optional(&mut *tx)
        .await?;
    if hay_entidad.is_none() {
        return Ok(Some("entidad"));
    }
    if let Some(c) = contrato_id {
        let hay_contrato: Option<i64> =
            sqlx::query_scalar("SELECT id FROM contrato WHERE id = ? AND institucion_id = ?")
                .bind(c)
                .bind(institucion_id)
                .fetch_optional(&mut *tx)
                .await?;
        if hay_contrato.is_none() {
            return Ok(Some("contrato"));
        }
    }
    Ok(None)
}

pub async fn crear_factura(
    pool: &crate::Pool,
    ambito: Ambito,
    institucion_id: i64,
    n: &FacturaNueva,
) -> Result<ResultadoAlta, sqlx::Error> {
    if ambito.puede_ver(Some(institucion_id)) != Visibilidad::Si {
        return Ok(ResultadoAlta::Fallo(Resultado::NoExiste));
    }
    let mut tx = pool.begin().await?;

    if let Some(que) = referencias_validas(&mut tx, institucion_id, n.entidad_id, n.contrato_id).await?
    {
        return Ok(ResultadoAlta::Fallo(Resultado::ReferenciaInvalida { que }));
    }

    let hoy = hoy();
    let anio: i32 = hoy[..4].parse().unwrap_or(1970);
    let numero = siguiente_numero(&mut tx, institucion_id, anio).await?;

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO factura \
         (institucion_id, numero_factura, entidad_id, contrato_id, fecha_factura, \
          periodo_inicio, periodo_fin, descuentos, observaciones, estado) \
         VALUES (?,?,?,?,?,?,?,?,?, 'BORRADOR') RETURNING id",
    )
    .bind(institucion_id)
    .bind(&numero)
    .bind(n.entidad_id)
    .bind(n.contrato_id)
    .bind(&hoy)
    .bind(&n.periodo_inicio)
    .bind(&n.periodo_fin)
    .bind(n.descuentos)
    .bind(&n.observaciones)
    .fetch_one(&mut *tx)
    .await?;

    for d in &n.detalles {
        insertar_detalle(&mut tx, id, d).await?;
    }
    recalcular(&mut tx, id).await?;
    tx.commit().await?;
    Ok(ResultadoAlta::Creado(id))
}

async fn insertar_detalle(
    tx: &mut sqlx::SqliteConnection,
    factura_id: i64,
    d: &DetalleNuevo,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO detalle_factura \
         (factura_id, servicio, codigo_cups, cantidad, valor_unitario, valor_total, \
          paciente_id, historia_id) VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(factura_id)
    .bind(&d.servicio)
    .bind(&d.codigo_cups)
    .bind(d.cantidad)
    .bind(d.valor_unitario)
    .bind(d.valor_unitario * d.cantidad as f64)
    .bind(d.paciente_id)
    .bind(d.historia_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

pub async fn actualizar_factura(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    e: &FacturaEdicion,
) -> Result<Resultado, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if !estado.admite_edicion() {
        return Ok(Resultado::EstadoNoAdmite {
            estado: estado.como_texto().into(),
        });
    }
    // La institución no se puede cambiar y no está en `FacturaEdicion`: una
    // factura no se muda de prestador. Lo que sí hay que validar es la pareja
    // entidad/contrato si alguna de las dos se toca.
    let institucion_id: i64 = sqlx::query_scalar("SELECT institucion_id FROM factura WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if e.entidad_id.is_some() || e.contrato_id.is_some() {
        let (ent, con): (i64, Option<i64>) =
            sqlx::query_as("SELECT entidad_id, contrato_id FROM factura WHERE id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let ent = e.entidad_id.unwrap_or(ent);
        let con = e.contrato_id.or(con);
        if let Some(que) = referencias_validas(&mut tx, institucion_id, ent, con).await? {
            return Ok(Resultado::ReferenciaInvalida { que });
        }
    }

    sqlx::query(
        "UPDATE factura SET \
           entidad_id     = COALESCE(?, entidad_id), \
           contrato_id    = COALESCE(?, contrato_id), \
           periodo_inicio = COALESCE(?, periodo_inicio), \
           periodo_fin    = COALESCE(?, periodo_fin), \
           descuentos     = COALESCE(?, descuentos), \
           observaciones  = COALESCE(?, observaciones) \
         WHERE id = ?",
    )
    .bind(e.entidad_id)
    .bind(e.contrato_id)
    .bind(&e.periodo_inicio)
    .bind(&e.periodo_fin)
    .bind(e.descuentos)
    .bind(&e.observaciones)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    recalcular(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Resultado::Hecho)
}

pub async fn eliminar_factura(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Resultado, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    // Solo un BORRADOR se borra. Lo que ya se radicó se ANULA: existe fuera de
    // este sistema y borrar la fila no lo borra de la EPS.
    if !estado.admite_edicion() {
        return Ok(Resultado::EstadoNoAdmite {
            estado: estado.como_texto().into(),
        });
    }
    // Las líneas y las glosas caen solas: `ON DELETE CASCADE` en la 0044.
    sqlx::query("DELETE FROM factura WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Resultado::Hecho)
}

pub async fn radicar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    r: &Radicacion,
) -> Result<Resultado, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if estado != EstadoFactura::Borrador {
        return Ok(Resultado::EstadoNoAdmite {
            estado: estado.como_texto().into(),
        });
    }
    let neto: f64 = sqlx::query_scalar("SELECT valor_neto FROM factura WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if neto <= 0.0 {
        return Ok(Resultado::SinValor);
    }
    sqlx::query(
        "UPDATE factura SET estado = 'RADICADA', num_radicado = ?, fecha_radicado = ? WHERE id = ?",
    )
    .bind(&r.num_radicado)
    .bind(r.fecha_radicado.clone().unwrap_or_else(hoy))
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Resultado::Hecho)
}

pub async fn cambiar_estado(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    destino: EstadoFactura,
) -> Result<Resultado, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if !estado.puede_pasar_a(destino) {
        return Ok(Resultado::TransicionInvalida {
            desde: estado.como_texto().into(),
            hacia: destino.como_texto().into(),
            permitidas: estado
                .siguientes()
                .iter()
                .map(|e| e.como_texto().to_string())
                .collect(),
        });
    }
    // `glosada` es DERIVADO y se escribe en la misma transacción, como pide la
    // 0044. No se apaga al salir de GLOSADA: la factura FUE glosada, y ese hecho
    // no se deshace por responder la glosa.
    sqlx::query("UPDATE factura SET estado = ?, glosada = (glosada OR ? = 'GLOSADA') WHERE id = ?")
        .bind(destino.como_texto())
        .bind(destino.como_texto())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Resultado::Hecho)
}

pub async fn agregar_detalle(
    pool: &crate::Pool,
    ambito: Ambito,
    factura_id: i64,
    d: &DetalleNuevo,
) -> Result<Resultado, sqlx::Error> {
    if d.servicio.trim().is_empty() {
        return Ok(Resultado::FaltaTexto { campo: "servicio" });
    }
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, factura_id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if !estado.admite_edicion() {
        return Ok(Resultado::EstadoNoAdmite {
            estado: estado.como_texto().into(),
        });
    }
    insertar_detalle(&mut tx, factura_id, d).await?;
    recalcular(&mut tx, factura_id).await?;
    tx.commit().await?;
    Ok(Resultado::Hecho)
}

pub async fn eliminar_detalle(
    pool: &crate::Pool,
    ambito: Ambito,
    factura_id: i64,
    detalle_id: i64,
) -> Result<Resultado, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, factura_id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if !estado.admite_edicion() {
        return Ok(Resultado::EstadoNoAdmite {
            estado: estado.como_texto().into(),
        });
    }
    // El `AND factura_id = ?` es lo que impide borrar la línea de OTRA factura
    // pasando su id: sin él, bastaba nombrar una factura propia y el detalle
    // ajeno.
    let filas = sqlx::query("DELETE FROM detalle_factura WHERE id = ? AND factura_id = ?")
        .bind(detalle_id)
        .bind(factura_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if filas == 0 {
        return Ok(Resultado::NoExiste);
    }
    recalcular(&mut tx, factura_id).await?;
    tx.commit().await?;
    Ok(Resultado::Hecho)
}

// ─────────────────────────────────────────────────────────────────────────────
// Glosas
// ─────────────────────────────────────────────────────────────────────────────

pub async fn listar_glosas(
    pool: &crate::Pool,
    ambito: Ambito,
    estado: Option<&str>,
) -> Result<Vec<Glosa>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "f.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT {COLS_GLOSA} FROM glosa g JOIN factura f ON f.id = g.factura_id WHERE {filtro}"
    );
    if estado.is_some() {
        sql.push_str(" AND g.estado = ?");
    }
    sql.push_str(" ORDER BY g.fecha_glosa DESC, g.id DESC");
    let mut q = sqlx::query_as::<_, Glosa>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(e) = estado {
        q = q.bind(e.to_string());
    }
    q.fetch_all(pool).await
}

pub async fn agregar_glosa(
    pool: &crate::Pool,
    ambito: Ambito,
    factura_id: i64,
    g: &GlosaNueva,
) -> Result<ResultadoAlta, sqlx::Error> {
    if g.item_glosado.trim().is_empty() {
        return Ok(ResultadoAlta::Fallo(Resultado::FaltaTexto {
            campo: "item_glosado",
        }));
    }
    if g.motivo.trim().is_empty() {
        return Ok(ResultadoAlta::Fallo(Resultado::FaltaTexto {
            campo: "motivo",
        }));
    }
    let mut tx = pool.begin().await?;
    let Some(estado) = estado_de(&mut tx, ambito, factura_id).await? else {
        return Ok(ResultadoAlta::Fallo(Resultado::NoExiste));
    };
    // Glosar un BORRADOR no significa nada: la EPS glosa lo que recibió. Y una
    // factura anulada o pagada ya no admite discusión. El Python no lo mira.
    if !matches!(
        estado,
        EstadoFactura::Radicada | EstadoFactura::Aceptada | EstadoFactura::Glosada
    ) {
        return Ok(ResultadoAlta::Fallo(Resultado::EstadoNoAdmite {
            estado: estado.como_texto().into(),
        }));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO glosa (factura_id, item_glosado, motivo, valor_glosado, fecha_glosa, estado) \
         VALUES (?,?,?,?,?, 'PENDIENTE') RETURNING id",
    )
    .bind(factura_id)
    .bind(g.item_glosado.trim())
    .bind(g.motivo.trim())
    .bind(g.valor_glosado)
    .bind(hoy())
    .fetch_one(&mut *tx)
    .await?;

    // La marca y el paso a GLOSADA van en la MISMA transacción que la glosa,
    // que es lo que la 0044 exige de quien escriba la columna derivada.
    sqlx::query(
        "UPDATE factura SET glosada = 1, \
           estado = CASE WHEN estado = 'RADICADA' THEN 'GLOSADA' ELSE estado END \
         WHERE id = ?",
    )
    .bind(factura_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ResultadoAlta::Creado(id))
}

pub async fn responder_glosa(
    pool: &crate::Pool,
    ambito: Ambito,
    glosa_id: i64,
    r: &RespuestaGlosa,
) -> Result<Resultado, sqlx::Error> {
    if r.respuesta.trim().is_empty() {
        return Ok(Resultado::FaltaTexto { campo: "respuesta" });
    }
    let Some((filtro, inst)) = filtro_ambito(ambito, "f.institucion_id") else {
        return Ok(Resultado::NoExiste);
    };
    let mut tx = pool.begin().await?;

    // El estado se lee CON el JOIN a la factura: una glosa no lleva institución
    // propia, su dueño es el de su factura.
    let sql = format!(
        "SELECT g.estado FROM glosa g JOIN factura f ON f.id = g.factura_id \
         WHERE g.id = ? AND {filtro}"
    );
    let q = sqlx::query_scalar::<_, String>(&sql).bind(glosa_id);
    let q = match inst {
        Some(i) => q.bind(i),
        None => q,
    };
    let Some(estado) = q.fetch_optional(&mut *tx).await? else {
        return Ok(Resultado::NoExiste);
    };
    if estado != "PENDIENTE" {
        return Ok(Resultado::EstadoNoAdmite { estado });
    }

    sqlx::query(
        "UPDATE glosa SET respuesta = ?, fecha_respuesta = ?, estado = ? WHERE id = ?",
    )
    .bind(r.respuesta.trim())
    .bind(hoy())
    .bind(if r.aceptar { "ACEPTADA" } else { "RECHAZADA" })
    .bind(glosa_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Resultado::Hecho)
}

// ─────────────────────────────────────────────────────────────────────────────
// Contratos y entidades
// ─────────────────────────────────────────────────────────────────────────────

const COLS_CONTRATO: &str = "c.id, c.entidad_id, e.nombre AS entidad_nombre, c.nombre, \
    c.numero_contrato, c.fecha_inicio, c.fecha_fin, c.activo, c.tipo_tarifa, \
    c.porcentaje_incremento, c.observaciones";

pub async fn listar_contratos(
    pool: &crate::Pool,
    ambito: Ambito,
    activo: Option<bool>,
) -> Result<Vec<Contrato>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "c.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT {COLS_CONTRATO} FROM contrato c JOIN entidad e ON e.id = c.entidad_id \
         WHERE {filtro}"
    );
    if activo.is_some() {
        sql.push_str(" AND c.activo = ?");
    }
    sql.push_str(" ORDER BY c.nombre");
    let mut q = sqlx::query_as::<_, Contrato>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(a) = activo {
        q = q.bind(a);
    }
    q.fetch_all(pool).await
}

pub async fn obtener_contrato(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Contrato>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "c.institucion_id") else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {COLS_CONTRATO} FROM contrato c JOIN entidad e ON e.id = c.entidad_id \
         WHERE c.id = ? AND {filtro}"
    );
    let mut q = sqlx::query_as::<_, Contrato>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Los cuatro tipos que admite el CHECK de la 0044. Se comprueba aquí para poder
/// devolver un mensaje que diga cuáles son, en vez de dejar que SQLite conteste
/// con una violación de restricción que el usuario no puede interpretar.
const TIPOS_TARIFA: [&str; 4] = ["ISS", "SOAT", "CUPS", "PERSONALIZADA"];

pub async fn crear_contrato(
    pool: &crate::Pool,
    ambito: Ambito,
    institucion_id: i64,
    c: &ContratoNuevo,
) -> Result<ResultadoAlta, sqlx::Error> {
    if ambito.puede_ver(Some(institucion_id)) != Visibilidad::Si {
        return Ok(ResultadoAlta::Fallo(Resultado::NoExiste));
    }
    if c.nombre.trim().is_empty() {
        return Ok(ResultadoAlta::Fallo(Resultado::FaltaTexto {
            campo: "nombre",
        }));
    }
    if c.numero_contrato.trim().is_empty() {
        return Ok(ResultadoAlta::Fallo(Resultado::FaltaTexto {
            campo: "numero_contrato",
        }));
    }
    if !TIPOS_TARIFA.contains(&c.tipo_tarifa.as_str()) {
        return Ok(ResultadoAlta::Fallo(Resultado::ReferenciaInvalida {
            que: "tipo_tarifa",
        }));
    }

    let mut tx = pool.begin().await?;
    let hay_entidad: Option<i64> = sqlx::query_scalar("SELECT id FROM entidad WHERE id = ?")
        .bind(c.entidad_id)
        .fetch_optional(&mut *tx)
        .await?;
    if hay_entidad.is_none() {
        return Ok(ResultadoAlta::Fallo(Resultado::ReferenciaInvalida {
            que: "entidad",
        }));
    }

    // La unicidad se mira DENTRO de la institución. En el Python es global, así
    // que el segundo prestador que numere sus contratos «001» recibe un 400 —y
    // con él, la confirmación de que el vecino ya tiene ese número.
    let repetido: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM contrato WHERE institucion_id = ? AND numero_contrato = ?",
    )
    .bind(institucion_id)
    .bind(c.numero_contrato.trim())
    .fetch_optional(&mut *tx)
    .await?;
    if repetido.is_some() {
        return Ok(ResultadoAlta::Fallo(Resultado::NumeroRepetido {
            numero: c.numero_contrato.trim().to_string(),
        }));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contrato \
         (institucion_id, entidad_id, nombre, numero_contrato, fecha_inicio, fecha_fin, \
          activo, tipo_tarifa, porcentaje_incremento, observaciones) \
         VALUES (?,?,?,?,?,?,1,?,?,?) RETURNING id",
    )
    .bind(institucion_id)
    .bind(c.entidad_id)
    .bind(c.nombre.trim())
    .bind(c.numero_contrato.trim())
    .bind(&c.fecha_inicio)
    .bind(&c.fecha_fin)
    .bind(&c.tipo_tarifa)
    .bind(c.porcentaje_incremento)
    .bind(&c.observaciones)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ResultadoAlta::Creado(id))
}

pub async fn actualizar_contrato(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    e: &ContratoEdicion,
) -> Result<Resultado, sqlx::Error> {
    if let Some(t) = &e.tipo_tarifa
        && !TIPOS_TARIFA.contains(&t.as_str())
    {
        return Ok(Resultado::ReferenciaInvalida { que: "tipo_tarifa" });
    }
    // El `WHERE` lleva el filtro del ámbito, así que un id ajeno no actualiza
    // nada y sale como NoExiste. Ni el número de contrato ni la entidad se
    // editan: cambiar el número rompería la referencia que la EPS ya tiene, y
    // mudar el contrato de entidad convertiría sus facturas en de otra.
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Resultado::NoExiste);
    };
    let sql = format!(
        "UPDATE contrato SET \
           nombre                = COALESCE(?, nombre), \
           fecha_inicio          = COALESCE(?, fecha_inicio), \
           fecha_fin             = COALESCE(?, fecha_fin), \
           activo                = COALESCE(?, activo), \
           tipo_tarifa           = COALESCE(?, tipo_tarifa), \
           porcentaje_incremento = COALESCE(?, porcentaje_incremento), \
           observaciones         = COALESCE(?, observaciones) \
         WHERE id = ? AND {filtro}"
    );
    let q = sqlx::query(&sql)
        .bind(&e.nombre)
        .bind(&e.fecha_inicio)
        .bind(&e.fecha_fin)
        .bind(e.activo)
        .bind(&e.tipo_tarifa)
        .bind(e.porcentaje_incremento)
        .bind(&e.observaciones)
        .bind(id);
    let q = match inst {
        Some(i) => q.bind(i),
        None => q,
    };
    if q.execute(pool).await?.rows_affected() == 0 {
        return Ok(Resultado::NoExiste);
    }
    Ok(Resultado::Hecho)
}

/// Catálogo nacional: lo lee cualquiera. No lleva institución a propósito (0044).
pub async fn listar_entidades(pool: &crate::Pool) -> Result<Vec<Entidad>, sqlx::Error> {
    sqlx::query_as::<_, Entidad>("SELECT id, nit, nombre, codigo_eps FROM entidad ORDER BY nombre")
        .fetch_all(pool)
        .await
}
