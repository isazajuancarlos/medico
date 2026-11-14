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
    /// El número siguiente se saldría del rango que la resolución de la DIAN
    /// autoriza (`configuracion_institucion.rango_hasta`): emitirlo daría una
    /// factura inválida. Se rechaza en vez de numerar fuera del rango.
    FueraDelRango {
        siguiente: i64,
        hasta: i64,
    },
    /// Hay un tope de rango declarado pero falta el número de la resolución que lo
    /// autoriza. Una resolución a medias no numera —un número que no se puede
    /// citar es tan inválido como uno fuera de rango—; se completa la
    /// configuración antes de facturar (directiva 20).
    ResolucionIncompleta,
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
    prefijo_institucion: &str,
    resolucion: Option<&str>,
    rango_desde: Option<i64>,
    rango_hasta: Option<i64>,
) -> Result<Result<String, Resultado>, sqlx::Error> {
    // **El prefijo lo pone la institución.** Hasta el 2026-08-08 estaba escrito
    // aquí como `FAC-` mientras `configuracion_institucion.prefijo_factura`
    // existía, se editaba desde el panel y no la leía nadie: el administrador
    // escribía «FE» y el binario seguía numerando «FAC». Es un descarte
    // silencioso de lo que el cliente afirmó, que es la directiva 20 al revés.
    //
    // El contador sigue siendo POR PREFIJO y POR AÑO, porque la consulta de
    // abajo busca el último con ese prefijo. Cambiar el prefijo empieza una
    // serie nueva desde el 1 — que es lo correcto: una serie de numeración es
    // justo lo que un prefijo distinto declara.
    let prefijo = format!("{prefijo_institucion}-{anio}-");
    let ultimo: Option<String> = sqlx::query_scalar(
        "SELECT numero_factura FROM factura \
         WHERE institucion_id = ? AND numero_factura LIKE ? \
         ORDER BY numero_factura DESC LIMIT 1",
    )
    .bind(institucion_id)
    .bind(format!("{prefijo}%"))
    .fetch_optional(&mut *tx)
    .await?;

    let previo: Option<i64> =
        ultimo.and_then(|u| u.rsplit('-').next().and_then(|s| s.parse::<i64>().ok()));

    // **La resolución de la DIAN sólo restringe si tiene TOPE** (`rango_hasta`).
    // Es el único límite cuya violación invalida la factura, y su ausencia es la
    // señal de «esta IPS todavía no cargó su resolución», que tiene que poder
    // seguir facturando (directiva 21). Con tope declarado:
    if let Some(hasta) = rango_hasta {
        // Un tope autorizado sin su número de resolución es una resolución a
        // medias: no se numera hasta completarla, en vez de emitir un número que
        // no se puede citar ante la DIAN.
        if resolucion.map(|r| r.trim().is_empty()).unwrap_or(true) {
            return Ok(Err(Resultado::ResolucionIncompleta));
        }
        // El arranque de la serie lo fija `rango_desde` (la resolución autoriza
        // DESDE un número); si no se declaró, el 1. Una serie ya empezada
        // continúa en el siguiente, no se renumera.
        let n = previo.map_or_else(|| rango_desde.unwrap_or(1), |p| p + 1);
        if n > hasta {
            return Ok(Err(Resultado::FueraDelRango { siguiente: n, hasta }));
        }
        return Ok(Ok(format!("{prefijo}{n:05}")));
    }

    // Sin tope: el arranque puede fijarlo `rango_desde` si está (una resolución
    // que solo declara el inicio), y si no, el 1 de siempre.
    let n = previo.map_or_else(|| rango_desde.unwrap_or(1), |p| p + 1);
    Ok(Ok(format!("{prefijo}{n:05}")))
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
    // ANTES de abrir la transacción: `configuracion` escribe la fila si falta, y
    // una escritura dentro de otra transacción sobre el mismo pool de SQLite es
    // cómo se fabrica un bloqueo.
    let cfg = crate::administracion::configuracion(pool, institucion_id).await?;
    // Sin institución no hay factura: el `puede_ver` de arriba ya salió.
    let prefijo = cfg
        .as_ref()
        .map(|c| c.prefijo_factura.clone())
        .unwrap_or_else(|| "FAC".into());
    let resolucion = cfg.as_ref().and_then(|c| c.resolucion_facturacion.clone());
    let rango_desde = cfg.as_ref().and_then(|c| c.rango_desde);
    let rango_hasta = cfg.as_ref().and_then(|c| c.rango_hasta);
    let mut tx = pool.begin().await?;

    if let Some(que) = referencias_validas(&mut tx, institucion_id, n.entidad_id, n.contrato_id).await?
    {
        return Ok(ResultadoAlta::Fallo(Resultado::ReferenciaInvalida { que }));
    }

    let hoy = hoy();
    let anio: i32 = hoy[..4].parse().unwrap_or(1970);
    let numero = match siguiente_numero(
        &mut tx,
        institucion_id,
        anio,
        &prefijo,
        resolucion.as_deref(),
        rango_desde,
        rango_hasta,
    )
    .await?
    {
        Ok(s) => s,
        // Sale sin commit: la transacción revierte al soltarse, así que la
        // factura fuera de rango no llega a existir.
        Err(r) => return Ok(ResultadoAlta::Fallo(r)),
    };

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
        if let Some(que) = insertar_detalle(&mut tx, institucion_id, id, d).await? {
            // Sale sin commit: la transacción revierte al soltarse, así que la
            // factura entera no llega a existir. Una factura a medias sería peor
            // que ninguna.
            return Ok(ResultadoAlta::Fallo(Resultado::ReferenciaInvalida { que }));
        }
    }
    recalcular(&mut tx, id).await?;
    tx.commit().await?;
    Ok(ResultadoAlta::Creado(id))
}

/// Inserta una línea, comprobando ANTES que el paciente y la historia que nombra
/// sean de esta institución.
///
/// # La fuga que esto cierra, y que estuvo aquí
///
/// Sin esta comprobación, un admin de la institución A podía crear una factura
/// PROPIA y ponerle una línea con el `paciente_id` de un paciente de la B. La
/// clave foránea de la 0044 es global (`REFERENCES paciente(id)`), así que la
/// acepta; y `COLS_DETALLE` hace `LEFT JOIN paciente` para devolver
/// `paciente_nombre`. Resultado: **la respuesta traía el nombre completo del
/// paciente ajeno**, y repitiendo con `paciente_id = 1..N` se volcaba el padrón
/// entero de todos los prestadores de la base. Era además un oráculo de
/// existencia: id que existe → 201, id que no → 500 por violación de FK.
///
/// Lo cazó la revisión de seguridad de esta misma rama, midiéndolo por la
/// frontera del programa y con su pareja: el `GET /api/pacientes/{id}` legítimo
/// del mismo paciente respondía 404 a esa misma cookie. O sea que la vía buena
/// negaba y esta la regalaba.
///
/// # Por qué va aquí y no en cada ruta
///
/// `insertar_detalle` es el ÚNICO embudo por el que entra una línea: lo llaman
/// `crear_factura` (con la lista del alta) y `agregar_detalle`. Poner la
/// comprobación en las dos rutas habría sido arreglar los dos casos y dejar
/// abierta la tercera que se escriba mañana.
///
/// # El patrón ya existía en el repositorio
///
/// `paciente_es_del_ambito` vive en `historia.rs`, `urgencias.rs`, `documento.rs`
/// y `acceso.rs`, y comprobaciones equivalentes en `nutricion`, `rehabilitacion`,
/// `hospitalizacion`, `cita`, `ambulancia` y `referencia`. Facturación era el
/// único módulo con `paciente_id` que no tenía ninguna. Aquí se hace **dentro de
/// la transacción** —de ahí que no se reutilice la de `acceso`, que toma el
/// pool—: comprobar fuera dejaría una ventana entre la comprobación y el INSERT.
async fn insertar_detalle(
    tx: &mut sqlx::SqliteConnection,
    institucion_id: i64,
    factura_id: i64,
    d: &DetalleNuevo,
) -> Result<Option<&'static str>, sqlx::Error> {
    // El paciente, si se nombra, tiene que ser de esta institución.
    if let Some(p) = d.paciente_id {
        let suyo: Option<i64> =
            sqlx::query_scalar("SELECT id FROM paciente WHERE id = ? AND institucion_id = ?")
                .bind(p)
                .bind(institucion_id)
                .fetch_optional(&mut *tx)
                .await?;
        if suyo.is_none() {
            // Mismo mensaje para «no existe» y «es de otro», por lo mismo que
            // `Visibilidad` no tiene variante `Prohibido`: distinguirlos deja
            // enumerar el padrón del vecino probando identificadores.
            return Ok(Some("paciente"));
        }
    }
    // La historia no lleva institución: cuelga de su paciente, igual que en el
    // resto del sistema.
    if let Some(h) = d.historia_id {
        let suya: Option<i64> = sqlx::query_scalar(
            "SELECT h.id FROM historia h JOIN paciente p ON p.id = h.paciente_id \
             WHERE h.id = ? AND p.institucion_id = ?",
        )
        .bind(h)
        .bind(institucion_id)
        .fetch_optional(&mut *tx)
        .await?;
        if suya.is_none() {
            return Ok(Some("historia"));
        }
    }

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
    Ok(None)
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
    // La institución sale de la FACTURA, no del token: así la línea no puede
    // apuntar a un paciente de otra ni aunque el llamante mintiera.
    let institucion_id: i64 =
        sqlx::query_scalar("SELECT institucion_id FROM factura WHERE id = ?")
            .bind(factura_id)
            .fetch_one(&mut *tx)
            .await?;
    if let Some(que) = insertar_detalle(&mut tx, institucion_id, factura_id, d).await? {
        return Ok(Resultado::ReferenciaInvalida { que });
    }
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
    if !estado.admite_glosa() {
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

#[cfg(test)]
mod pruebas_numeracion {
    //! **La resolución de la DIAN se aplica a la numeración** (#597).
    //!
    //! Hasta el 2026-08-09 `resolucion_facturacion`, `rango_desde` y `rango_hasta`
    //! se guardaban y no las leía nadie: el binario podía emitir un número fuera
    //! del rango autorizado y la factura salía inválida sin que nada avisara.

    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO entidad (id,nit,nombre,codigo_eps) VALUES (1,'800','EPS Uno','EPS001')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn factura() -> FacturaNueva {
        FacturaNueva {
            entidad_id: 1,
            contrato_id: None,
            periodo_inicio: "2026-03-01".into(),
            periodo_fin: "2026-03-31".into(),
            descuentos: 0.0,
            observaciones: None,
            detalles: vec![],
        }
    }

    async fn configurar(
        pool: &crate::Pool,
        resolucion: Option<&str>,
        desde: Option<i64>,
        hasta: Option<i64>,
    ) {
        crate::administracion::actualizar_configuracion(
            pool,
            1,
            &crate::administracion::ConfiguracionCambio {
                resolucion_facturacion: resolucion.map(|s| s.to_string()),
                rango_desde: desde,
                rango_hasta: hasta,
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .unwrap();
    }

    async fn crear(pool: &crate::Pool) -> ResultadoAlta {
        crear_factura(pool, Ambito::institucion(1), 1, &factura())
            .await
            .unwrap()
    }

    async fn creada(pool: &crate::Pool) -> i64 {
        match crear(pool).await {
            ResultadoAlta::Creado(id) => id,
            otro => panic!("no se creó la factura: {otro:?}"),
        }
    }

    /// El sufijo numérico de `PREFIJO-AÑO-NNNNN`, sin depender del año ni del
    /// prefijo (que los pone la institución).
    async fn numero(pool: &crate::Pool, id: i64) -> i64 {
        let n: String = sqlx::query_scalar("SELECT numero_factura FROM factura WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
        n.rsplit('-').next().unwrap().parse().unwrap()
    }

    async fn cuantas(pool: &crate::Pool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM factura")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Sin resolución configurada la serie es la de siempre: arranca en 1 y sigue.
    /// Es el control de que la restricción **no se cuela** cuando no hay rango
    /// (una IPS que aún no cargó su resolución tiene que poder facturar).
    #[tokio::test]
    async fn sin_resolucion_la_serie_arranca_en_uno() {
        let pool = base().await;
        let a = creada(&pool).await;
        let b = creada(&pool).await;
        assert_eq!(numero(&pool, a).await, 1);
        assert_eq!(numero(&pool, b).await, 2);
    }

    /// Con rango declarado, la serie arranca en `rango_desde`. La pareja son DOS
    /// valores distintos: prueba que se LEE la columna, no que coincide con una
    /// constante (directiva 33).
    #[tokio::test]
    async fn con_rango_la_serie_arranca_en_rango_desde() {
        for desde in [1000, 2500] {
            let pool = base().await;
            configurar(&pool, Some("RES-0001"), Some(desde), Some(desde + 10)).await;
            let id = creada(&pool).await;
            assert_eq!(
                numero(&pool, id).await,
                desde,
                "la serie no arrancó en rango_desde"
            );
        }
    }

    /// El número que superaría el tope se RECHAZA, y no llega a existir. El
    /// control positivo es que la factura que cae **justo en `rango_hasta`** SÍ se
    /// emite: el límite es `> hasta`, no `>= hasta`, y sin esa mitad un mutante que
    /// cerrara un número antes pasaría.
    #[tokio::test]
    async fn el_numero_que_supera_el_tope_se_rechaza() {
        let pool = base().await;
        // Rango [1000, 1001]: caben exactamente dos.
        configurar(&pool, Some("RES-0001"), Some(1000), Some(1001)).await;
        assert_eq!(numero(&pool, creada(&pool).await).await, 1000);
        assert_eq!(
            numero(&pool, creada(&pool).await).await,
            1001,
            "la factura que cae en el tope SÍ se emite"
        );
        // La tercera se saldría del rango.
        assert_eq!(
            crear(&pool).await,
            ResultadoAlta::Fallo(Resultado::FueraDelRango {
                siguiente: 1002,
                hasta: 1001,
            })
        );
        // Y no persistió: la transacción revirtió.
        assert_eq!(cuantas(&pool).await, 2, "la factura fuera de rango no existe");
    }

    /// Un rango con tope pero SIN número de resolución no factura: la resolución
    /// está a medias. La pareja: al completar el número, sí factura (y arranca en
    /// `rango_desde`).
    #[tokio::test]
    async fn un_rango_sin_numero_de_resolucion_no_factura() {
        let pool = base().await;
        configurar(&pool, None, Some(1000), Some(2000)).await;
        assert_eq!(
            crear(&pool).await,
            ResultadoAlta::Fallo(Resultado::ResolucionIncompleta)
        );
        assert_eq!(cuantas(&pool).await, 0, "no se emitió nada a medias");

        // Completada la resolución (el rango se conserva por COALESCE):
        configurar(&pool, Some("RES-2026-1"), None, None).await;
        assert_eq!(numero(&pool, creada(&pool).await).await, 1000);
    }

    /// La restricción sólo la impone el TOPE. `rango_desde` a solas fija el
    /// arranque pero no rechaza nada, y por eso tampoco exige el número de
    /// resolución: es la resolución que solo declara desde dónde numerar.
    #[tokio::test]
    async fn solo_rango_desde_fija_el_inicio_y_no_pone_tope() {
        let pool = base().await;
        configurar(&pool, None, Some(5), None).await;
        assert_eq!(
            numero(&pool, creada(&pool).await).await,
            5,
            "arranca en rango_desde aun sin tope"
        );
    }
}
