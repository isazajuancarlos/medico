//! Farmacia: inventario, movimientos y dispensación. Port a Rust de
//! `backend/routers/farmacia.py`.
//!
//! El item de inventario lleva `institucion_id` propio y se filtra por él
//! (ajeno == inexistente). La fórmula y el paciente NO lo llevan: cuelgan de la
//! cadena historia → paciente → institución, igual que en el resto del clínico.
//!
//! # Lo que este módulo NO deja hacer, y por qué está aquí y no en la ruta
//!
//! Dispensar es la operación con efectos jurídicos y económicos del módulo: baja
//! existencias, deja constancia de una entrega a un paciente y cierra una
//! prescripción. Las cinco negativas —lote vencido, lote que no es el del item,
//! más de lo prescrito, más de lo que hay, cantidad no positiva— viven en la capa
//! de datos y no en el manejador HTTP, porque la ruta se puede duplicar y la
//! comprobación de la copia es la que se olvida. Cada una tiene su variante en
//! [`ResultadoDispensa`]: quien llama no puede confundir «no hay existencias» con
//! «el lote venció», y el usuario recibe el motivo real en vez de un «error».

use chrono::{Duration, NaiveDate, SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

/// Cuántos días antes del vencimiento se enciende la alerta.
///
/// 90 días es el plazo que da margen para devolver al proveedor o redistribuir
/// antes de que el lote deje de servir; con 30 ya no da tiempo a nada, y con 180
/// media farmacia queda en alerta permanente y la alerta deja de mirarse.
const DIAS_AVISO_VENCIMIENTO: i64 = 90;

/// Filas por página en los históricos (movimientos y dispensaciones).
const POR_PAGINA: i64 = 50;

// --- Item de inventario ------------------------------------------------------

/// Un item tal como lo ve la interfaz.
///
/// `valor_total`, `alerta_stock` y `alerta_vencimiento` NO son columnas: se
/// calculan en la consulta a partir de las que sí lo son. Guardarlos sería tener
/// dos respuestas para la misma pregunta y ninguna con autoridad.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Item {
    pub id: i64,
    pub nombre: String,
    pub tipo: String,
    pub categoria: Option<String>,
    pub presentacion: Option<String>,
    pub lote: String,
    pub fecha_vencimiento: Option<String>,
    pub registro_invima: Option<String>,
    pub cantidad_actual: i64,
    pub cantidad_minima: i64,
    pub unidad_medida: String,
    pub ubicacion: String,
    pub costo_unitario: f64,
    pub valor_total: f64,
    pub requiere_refrigeracion: bool,
    pub controlado: bool,
    pub alerta_stock: bool,
    pub alerta_vencimiento: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemNuevo {
    pub nombre: String,
    pub tipo: Option<String>,
    pub categoria: Option<String>,
    pub presentacion: Option<String>,
    pub lote: Option<String>,
    pub fecha_vencimiento: Option<String>,
    pub registro_invima: Option<String>,
    pub cantidad_actual: Option<i64>,
    pub cantidad_minima: Option<i64>,
    pub unidad_medida: Option<String>,
    pub ubicacion: Option<String>,
    pub costo_unitario: Option<f64>,
    pub requiere_refrigeracion: Option<bool>,
    pub controlado: Option<bool>,
}

/// Edición parcial: lo ausente se queda como estaba (`COALESCE`).
///
/// **`cantidad_actual` no está y no debe estar.** Las existencias solo cambian
/// por una entrada, una salida o un ajuste, y las tres escriben su movimiento en
/// la misma transacción. Un `PUT` que pudiera fijar el stock a mano sería una
/// puerta trasera al histórico: el número cambiaría y no habría fila que dijera
/// quién, cuándo ni por qué.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ItemEdicion {
    pub nombre: Option<String>,
    pub tipo: Option<String>,
    pub categoria: Option<String>,
    pub presentacion: Option<String>,
    pub fecha_vencimiento: Option<String>,
    pub registro_invima: Option<String>,
    pub cantidad_minima: Option<i64>,
    pub unidad_medida: Option<String>,
    pub ubicacion: Option<String>,
    pub costo_unitario: Option<f64>,
    pub requiere_refrigeracion: Option<bool>,
    pub controlado: Option<bool>,
}

/// Filtros del listado. Todos opcionales y combinables.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Filtros {
    /// Búsqueda por nombre o lote.
    pub q: Option<String>,
    pub tipo: Option<String>,
    pub ubicacion: Option<String>,
    /// Solo los que están en o por debajo del mínimo.
    pub stock_bajo: Option<bool>,
    /// Solo los que vencen dentro del plazo de aviso, o ya vencieron.
    pub por_vencer: Option<bool>,
    /// Lo que pide el botón «Ver solo alertas» de la pantalla: la UNIÓN de los
    /// dos anteriores. Es su propio filtro y no la suma de dos banderas porque
    /// combinarlas con AND daría solo lo que está bajo mínimo Y además vence —
    /// casi nada— justo cuando el usuario pide ver todo lo que reclama atención.
    pub solo_alertas: Option<bool>,
}

/// Resumen de la farmacia para la cabecera de la pantalla.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Stats {
    pub total_items: i64,
    pub stock_bajo: i64,
    pub por_vencer: i64,
    pub vencidos: i64,
    pub formulas_pendientes: i64,
    pub valor_total_inventario: f64,
}

// --- Movimientos y dispensaciones --------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Movimiento {
    pub id: i64,
    pub item_inventario_id: i64,
    pub usuario_id: i64,
    pub fecha: String,
    pub tipo_movimiento: String,
    pub cantidad: i64,
    pub motivo: String,
    pub paciente_id: Option<i64>,
    pub formula_id: Option<i64>,
    pub usuario_nombre: Option<String>,
    pub item_nombre: Option<String>,
    pub proveedor_id: Option<i64>,
    pub proveedor_nombre: Option<String>,
    pub documento_soporte: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entrada {
    pub cantidad: i64,
    pub motivo: String,
    /// Lote de la remesa que entra. Ver [`ResultadoEntrada::LoteDistinto`].
    pub lote: Option<String>,
    pub fecha_vencimiento: Option<String>,
    pub costo_unitario: Option<f64>,
    /// De quién viene la remesa. **Opcional a propósito**: hay entradas que no
    /// son una compra —una donación, un traslado entre sedes, un ajuste de
    /// conteo— y obligar a inventar un proveedor para esas produciría justo el
    /// dato falso que este módulo evita en todo lo demás.
    pub proveedor_id: Option<i64>,
    /// Factura o remisión con la que llegó. Es el documento que un auditor pide
    /// primero y el que cierra el rastro hacia atrás.
    pub documento_soporte: Option<String>,
}

/// Qué pasó al registrar una entrada de stock.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoEntrada {
    Hecha(Box<Item>),
    /// El item no existe o no es del ámbito.
    NoExiste,
    /// Cantidad cero o negativa: una «entrada» que no entra nada.
    CantidadInvalida,
    /// **Llega un lote distinto del que la fila ya tiene, y todavía queda stock
    /// de aquel.** Sumar las dos remesas en una fila las vuelve indistinguibles:
    /// el sistema diría que hay 25 unidades del lote nuevo cuando 5 son del
    /// viejo, con otra fecha de vencimiento y otra trazabilidad. El día de un
    /// retiro sanitario del INVIMA, eso es no saber a quién se le entregó qué.
    ///
    /// Un lote nuevo es un item nuevo. Si la fila está en cero, en cambio, sí se
    /// reutiliza: no hay nada con qué mezclar.
    LoteDistinto { actual: String, quedan: i64 },
    /// Se indicó un proveedor que no es de esta institución o está inactivo.
    /// Guardar la entrada sin él «porque el id no valía» dejaría una remesa cuya
    /// procedencia el usuario CREE haber registrado.
    ProveedorNoValido,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DispensarNueva {
    pub formula_id: i64,
    pub item_inventario_id: i64,
    pub cantidad_dispensada: i64,
    /// Puede venir vacío: entonces se toma el del item. Si viene con valor y no
    /// es el del item, se rechaza — ver [`ResultadoDispensa::LoteNoCoincide`].
    #[serde(default)]
    pub lote_dispensado: String,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Dispensacion {
    pub id: i64,
    pub formula_id: i64,
    pub item_inventario_id: i64,
    pub paciente_id: i64,
    pub regente_farmacia_id: i64,
    pub fecha_dispensacion: String,
    pub cantidad_dispensada: i64,
    pub lote_dispensado: String,
    pub observaciones: Option<String>,
    pub paciente_nombre: Option<String>,
    pub medicamento_nombre: Option<String>,
    pub item_nombre: Option<String>,
    pub regente_nombre: Option<String>,
}

/// Qué pasó al intentar dispensar. Cada negativa es su propia variante: quien
/// llama no puede responder «error» a cinco preguntas distintas.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoDispensa {
    Hecha(Box<Dispensacion>),
    /// La fórmula o el item no existen, o no son del ámbito. Una sola variante
    /// para los dos casos: distinguirlos diría si el identificador existe en la
    /// institución de al lado.
    NoExiste,
    CantidadInvalida,
    /// Ya se entregó todo lo prescrito.
    YaCompleta,
    /// Se pidió más de lo que falta por entregar de esa fórmula.
    ExcedePrescrito { pendiente: f64 },
    SinExistencias { disponible: i64 },
    /// El lote venció. No es una advertencia: dispensar un lote vencido está
    /// prohibido, y el sistema no es quien para dejarlo pasar «por esta vez».
    LoteVencido { vencio: String },
    /// El cliente afirmó un lote que no es el de la fila de la que se descuenta.
    /// Aceptarlo escribiría una trazabilidad falsa, que es el único motivo por el
    /// que el campo existe.
    LoteNoCoincide { del_item: String },
}

/// Qué pasó al borrar un item.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoBorrado {
    Hecho,
    NoExiste,
    /// Tiene movimientos o dispensaciones: su histórico es el rastro de a quién
    /// se le entregó qué. Se da de baja poniéndolo en cero, no borrándolo.
    TieneHistorial { movimientos: i64, dispensaciones: i64 },
}

/// Una fórmula esperando entrega, con el paciente y el médico ya resueltos.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FormulaPendiente {
    pub id: i64,
    pub historia_id: i64,
    pub medicamento: String,
    pub codigo_cum: Option<String>,
    pub dosis: String,
    pub cantidad: f64,
    pub unidad: Option<String>,
    pub observaciones: Option<String>,
    pub fecha_formula: String,
    pub dispensada: bool,
    pub paciente_nombre: Option<String>,
    pub paciente_id: Option<i64>,
    pub medico_nombre: Option<String>,
}

// --- Consultas ---------------------------------------------------------------

use crate::{filtro_ambito, hoy};


/// `hoy` + [`DIAS_AVISO_VENCIMIENTO`]. Si `hoy` no es una fecha, devuelve `hoy`:
/// el efecto es que no se marca nada como «por vencer», que es lo prudente
/// cuando no se sabe qué día es — nunca marcar de más.
fn limite_aviso(hoy: &str) -> String {
    match NaiveDate::parse_from_str(hoy, "%Y-%m-%d") {
        Ok(d) => (d + Duration::days(DIAS_AVISO_VENCIMIENTO)).to_string(),
        Err(_) => hoy.to_string(),
    }
}

/// Las columnas del item más las tres derivadas. `?` de aquí toma `hoy` dos
/// veces y el límite de aviso una: se enlazan en ese orden.
fn columnas_item() -> String {
    "id, nombre, tipo, categoria, presentacion, lote, fecha_vencimiento, registro_invima, \
     cantidad_actual, cantidad_minima, unidad_medida, ubicacion, costo_unitario, \
     (cantidad_actual * costo_unitario) AS valor_total, \
     requiere_refrigeracion, controlado, \
     (cantidad_actual <= cantidad_minima) AS alerta_stock, \
     (fecha_vencimiento IS NOT NULL AND fecha_vencimiento <= ?) AS alerta_vencimiento"
        .to_string()
}

/// Items del ámbito, filtrados. `hoy` en `YYYY-MM-DD` decide las alertas de
/// vencimiento; se pasa en vez de leerlo del reloj para que las pruebas puedan
/// situarse antes y después de una fecha sin esperar tres meses.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &Filtros,
    hoy: &str,
) -> Result<Vec<Item>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Vec::new());
    };
    let aviso = limite_aviso(hoy);
    let mut sql = format!("SELECT {} FROM item_inventario WHERE {filtro}", columnas_item());
    if f.q.is_some() {
        sql.push_str(" AND (nombre LIKE ? OR lote LIKE ?)");
    }
    if f.tipo.is_some() {
        sql.push_str(" AND tipo = ?");
    }
    if f.ubicacion.is_some() {
        sql.push_str(" AND ubicacion = ?");
    }
    if f.stock_bajo == Some(true) {
        sql.push_str(" AND cantidad_actual <= cantidad_minima");
    }
    if f.por_vencer == Some(true) {
        sql.push_str(" AND fecha_vencimiento IS NOT NULL AND fecha_vencimiento <= ?");
    }
    if f.solo_alertas == Some(true) {
        sql.push_str(
            " AND (cantidad_actual <= cantidad_minima \
               OR (fecha_vencimiento IS NOT NULL AND fecha_vencimiento <= ?))",
        );
    }
    sql.push_str(" ORDER BY nombre, lote");

    let mut q = sqlx::query_as::<_, Item>(&sql).bind(&aviso);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(texto) = &f.q {
        let patron = format!("%{texto}%");
        q = q.bind(patron.clone()).bind(patron);
    }
    if let Some(t) = &f.tipo {
        q = q.bind(t.clone());
    }
    if let Some(u) = &f.ubicacion {
        q = q.bind(u.clone());
    }
    if f.por_vencer == Some(true) {
        q = q.bind(aviso.clone());
    }
    if f.solo_alertas == Some(true) {
        q = q.bind(aviso.clone());
    }
    q.fetch_all(pool).await
}

/// Un item por id, si es del ámbito.
pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    hoy: &str,
) -> Result<Option<Item>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT {} FROM item_inventario WHERE id = ? AND {filtro}",
        columnas_item()
    );
    let mut q = sqlx::query_as::<_, Item>(&sql).bind(limite_aviso(hoy)).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Da de alta un item en `institucion_id`, que debe ser visible para el ámbito.
///
/// La institución llega como parámetro y no se lee del cuerpo de la petición:
/// quien crea no elige de quién es el inventario. `None` si la institución no es
/// del ámbito.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    institucion_id: i64,
    i: &ItemNuevo,
) -> Result<Option<i64>, sqlx::Error> {
    if ambito.puede_ver(Some(institucion_id)) != Visibilidad::Si {
        return Ok(None);
    }
    let r = sqlx::query(
        "INSERT INTO item_inventario \
         (institucion_id, nombre, tipo, categoria, presentacion, lote, fecha_vencimiento, \
          registro_invima, cantidad_actual, cantidad_minima, unidad_medida, ubicacion, \
          costo_unitario, requiere_refrigeracion, controlado) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(&i.nombre)
    .bind(i.tipo.as_deref().unwrap_or("MEDICAMENTO"))
    .bind(&i.categoria)
    .bind(&i.presentacion)
    .bind(i.lote.as_deref().unwrap_or(""))
    .bind(&i.fecha_vencimiento)
    .bind(&i.registro_invima)
    .bind(i.cantidad_actual.unwrap_or(0))
    .bind(i.cantidad_minima.unwrap_or(10))
    .bind(i.unidad_medida.as_deref().unwrap_or("UNIDAD"))
    .bind(i.ubicacion.as_deref().unwrap_or("FARMACIA"))
    .bind(i.costo_unitario.unwrap_or(0.0))
    .bind(i.requiere_refrigeracion.unwrap_or(false))
    .bind(i.controlado.unwrap_or(false))
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Actualiza los campos provistos. `false` si el item no es del ámbito.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    e: &ItemEdicion,
) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    let sql = format!(
        "UPDATE item_inventario SET \
         nombre=COALESCE(?,nombre), tipo=COALESCE(?,tipo), categoria=COALESCE(?,categoria), \
         presentacion=COALESCE(?,presentacion), fecha_vencimiento=COALESCE(?,fecha_vencimiento), \
         registro_invima=COALESCE(?,registro_invima), cantidad_minima=COALESCE(?,cantidad_minima), \
         unidad_medida=COALESCE(?,unidad_medida), ubicacion=COALESCE(?,ubicacion), \
         costo_unitario=COALESCE(?,costo_unitario), \
         requiere_refrigeracion=COALESCE(?,requiere_refrigeracion), \
         controlado=COALESCE(?,controlado) \
         WHERE id = ? AND {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(&e.nombre)
        .bind(&e.tipo)
        .bind(&e.categoria)
        .bind(&e.presentacion)
        .bind(&e.fecha_vencimiento)
        .bind(&e.registro_invima)
        .bind(e.cantidad_minima)
        .bind(&e.unidad_medida)
        .bind(&e.ubicacion)
        .bind(e.costo_unitario)
        .bind(e.requiere_refrigeracion)
        .bind(e.controlado)
        .bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

/// Borra un item **que no haya movido nada**. Ver [`ResultadoBorrado`].
pub async fn eliminar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<ResultadoBorrado, sqlx::Error> {
    if obtener(pool, ambito, id, &hoy()).await?.is_none() {
        return Ok(ResultadoBorrado::NoExiste);
    }
    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimiento_inventario WHERE item_inventario_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    let dispensaciones: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM dispensacion WHERE item_inventario_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if movimientos > 0 || dispensaciones > 0 {
        return Ok(ResultadoBorrado::TieneHistorial { movimientos, dispensaciones });
    }
    sqlx::query("DELETE FROM item_inventario WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(ResultadoBorrado::Hecho)
}

/// Registra una entrada de stock: sube existencias y escribe su movimiento, en
/// una sola transacción. Actualiza vencimiento y costo si vienen.
pub async fn entrada(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    usuario_id: i64,
    e: &Entrada,
    hoy: &str,
) -> Result<ResultadoEntrada, sqlx::Error> {
    if e.cantidad <= 0 {
        return Ok(ResultadoEntrada::CantidadInvalida);
    }
    let Some(item) = obtener(pool, ambito, id, hoy).await? else {
        return Ok(ResultadoEntrada::NoExiste);
    };
    // Un lote distinto con existencias del anterior no se mezcla.
    if let Some(nuevo) = e.lote.as_deref().map(str::trim).filter(|l| !l.is_empty())
        && nuevo != item.lote
        && item.cantidad_actual > 0
    {
        return Ok(ResultadoEntrada::LoteDistinto {
            actual: item.lote.clone(),
            quedan: item.cantidad_actual,
        });
    }
    if let Some(proveedor_id) = e.proveedor_id
        && !crate::proveedor::activo_del_ambito(pool, ambito, proveedor_id).await?
    {
        return Ok(ResultadoEntrada::ProveedorNoValido);
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE item_inventario SET cantidad_actual = cantidad_actual + ?, \
         lote = COALESCE(?, lote), fecha_vencimiento = COALESCE(?, fecha_vencimiento), \
         costo_unitario = COALESCE(?, costo_unitario) WHERE id = ?",
    )
    .bind(e.cantidad)
    .bind(e.lote.as_deref().map(str::trim).filter(|l| !l.is_empty()))
    .bind(&e.fecha_vencimiento)
    .bind(e.costo_unitario)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO movimiento_inventario \
         (item_inventario_id, usuario_id, fecha, tipo_movimiento, cantidad, motivo, \
          proveedor_id, documento_soporte) VALUES (?,?,?,'ENTRADA',?,?,?,?)",
    )
    .bind(id)
    .bind(usuario_id)
    .bind(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true))
    .bind(e.cantidad)
    .bind(&e.motivo)
    .bind(e.proveedor_id)
    .bind(&e.documento_soporte)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let actualizado = obtener(pool, ambito, id, hoy).await?.expect("acaba de existir");
    Ok(ResultadoEntrada::Hecha(Box::new(actualizado)))
}

/// Fórmulas del ámbito pendientes de entrega, de la más antigua a la más
/// reciente: la que lleva más esperando se atiende primero.
pub async fn formulas_pendientes(
    pool: &crate::Pool,
    ambito: Ambito,
    q: Option<&str>,
) -> Result<Vec<FormulaPendiente>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT f.id, f.historia_id, f.medicamento, f.codigo_cum, f.dosis, f.cantidad, f.unidad, \
         f.observaciones, f.fecha_formula, f.dispensada, \
         (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS paciente_nombre, \
         p.id AS paciente_id, u.nombre AS medico_nombre \
         FROM formula f \
         JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id \
         LEFT JOIN usuario u ON u.id = h.medico_id \
         WHERE f.dispensada = 0 AND {filtro}"
    );
    if q.is_some() {
        sql.push_str(" AND (f.medicamento LIKE ? OR p.numero_documento LIKE ? OR p.apellido1 LIKE ?)");
    }
    sql.push_str(" ORDER BY f.fecha_formula, f.id");

    let mut consulta = sqlx::query_as::<_, FormulaPendiente>(&sql);
    if let Some(i) = inst {
        consulta = consulta.bind(i);
    }
    if let Some(texto) = q {
        let patron = format!("%{texto}%");
        consulta = consulta.bind(patron.clone()).bind(patron.clone()).bind(patron);
    }
    consulta.fetch_all(pool).await
}

const DISP_COLS: &str = "d.id, d.formula_id, d.item_inventario_id, d.paciente_id, \
    d.regente_farmacia_id, d.fecha_dispensacion, d.cantidad_dispensada, d.lote_dispensado, \
    d.observaciones, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente_nombre, \
    f.medicamento AS medicamento_nombre, i.nombre AS item_nombre, u.nombre AS regente_nombre";

/// Entrega de un medicamento contra su fórmula. Baja existencias, escribe el
/// movimiento de salida, deja la constancia y cierra la fórmula si ya se entregó
/// todo lo prescrito — las cuatro cosas en una transacción.
pub async fn dispensar(
    pool: &crate::Pool,
    ambito: Ambito,
    usuario_id: i64,
    d: &DispensarNueva,
    hoy: &str,
) -> Result<ResultadoDispensa, sqlx::Error> {
    if d.cantidad_dispensada <= 0 {
        return Ok(ResultadoDispensa::CantidadInvalida);
    }
    // La fórmula, por la cadena historia → paciente → institución.
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(ResultadoDispensa::NoExiste);
    };
    let sql = format!(
        "SELECT f.id, f.cantidad, p.id, f.dispensada FROM formula f \
         JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id \
         WHERE f.id = ? AND {filtro}"
    );
    let mut consulta = sqlx::query_as::<_, (i64, f64, i64, bool)>(&sql).bind(d.formula_id);
    if let Some(i) = inst {
        consulta = consulta.bind(i);
    }
    let Some((formula_id, prescrito, paciente_id, dispensada)) = consulta.fetch_optional(pool).await?
    else {
        return Ok(ResultadoDispensa::NoExiste);
    };
    if dispensada {
        return Ok(ResultadoDispensa::YaCompleta);
    }

    let Some(item) = obtener(pool, ambito, d.item_inventario_id, hoy).await? else {
        return Ok(ResultadoDispensa::NoExiste);
    };

    // Lote vencido: no se entrega, y no hay «por esta vez».
    if let Some(vence) = &item.fecha_vencimiento
        && vence.as_str() < hoy
    {
        return Ok(ResultadoDispensa::LoteVencido { vencio: vence.clone() });
    }
    // El lote que afirma el cliente tiene que ser el de la fila de la que se
    // descuenta; vacío significa «el del item».
    let declarado = d.lote_dispensado.trim();
    if !declarado.is_empty() && declarado != item.lote {
        return Ok(ResultadoDispensa::LoteNoCoincide { del_item: item.lote.clone() });
    }

    // `CAST(... AS REAL)` y no a secas: `SUM` sobre una columna INTEGER devuelve
    // INTEGER, y lo prescrito es REAL (la fórmula admite media tableta). Sin el
    // CAST, SQLx rechaza la fila al decodificar y la entrega falla por un motivo
    // que no tiene nada que ver con la farmacia.
    let ya: f64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(cantidad_dispensada), 0) AS REAL) FROM dispensacion WHERE formula_id = ?",
    )
    .bind(formula_id)
    .fetch_one(pool)
    .await?;
    let pendiente = prescrito - ya;
    if pendiente <= 0.0 {
        return Ok(ResultadoDispensa::YaCompleta);
    }
    if (d.cantidad_dispensada as f64) > pendiente {
        return Ok(ResultadoDispensa::ExcedePrescrito { pendiente });
    }
    if d.cantidad_dispensada > item.cantidad_actual {
        return Ok(ResultadoDispensa::SinExistencias { disponible: item.cantidad_actual });
    }

    let ahora = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut tx = pool.begin().await?;
    // El `AND cantidad_actual >= ?` no es redundante con la comprobación de
    // arriba: entre leer y escribir cabe otra dispensación. Si otra se adelantó,
    // esta no afecta ninguna fila y la transacción se deshace.
    let bajada = sqlx::query(
        "UPDATE item_inventario SET cantidad_actual = cantidad_actual - ? \
         WHERE id = ? AND cantidad_actual >= ?",
    )
    .bind(d.cantidad_dispensada)
    .bind(item.id)
    .bind(d.cantidad_dispensada)
    .execute(&mut *tx)
    .await?;
    if bajada.rows_affected() == 0 {
        tx.rollback().await?;
        let ahora_hay = obtener(pool, ambito, item.id, hoy).await?.map_or(0, |i| i.cantidad_actual);
        return Ok(ResultadoDispensa::SinExistencias { disponible: ahora_hay });
    }
    let r = sqlx::query(
        "INSERT INTO dispensacion \
         (formula_id, item_inventario_id, paciente_id, regente_farmacia_id, fecha_dispensacion, \
          cantidad_dispensada, lote_dispensado, observaciones) VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(formula_id)
    .bind(item.id)
    .bind(paciente_id)
    .bind(usuario_id)
    .bind(&ahora)
    .bind(d.cantidad_dispensada)
    .bind(&item.lote)
    .bind(&d.observaciones)
    .execute(&mut *tx)
    .await?;
    let dispensacion_id = r.last_insert_rowid();
    sqlx::query(
        "INSERT INTO movimiento_inventario \
         (item_inventario_id, usuario_id, fecha, tipo_movimiento, cantidad, motivo, paciente_id, formula_id) \
         VALUES (?,?,?,'SALIDA',?,?,?,?)",
    )
    .bind(item.id)
    .bind(usuario_id)
    .bind(&ahora)
    .bind(d.cantidad_dispensada)
    .bind("Dispensación contra fórmula médica")
    .bind(paciente_id)
    .bind(formula_id)
    .execute(&mut *tx)
    .await?;
    // La fórmula se cierra solo cuando se entregó TODO lo prescrito. Cerrarla en
    // la primera entrega parcial la sacaría de la lista de pendientes con el
    // paciente esperando el resto.
    if ya + d.cantidad_dispensada as f64 >= prescrito {
        sqlx::query("UPDATE formula SET dispensada = 1 WHERE id = ?")
            .bind(formula_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    let vista = sqlx::query_as::<_, Dispensacion>(&format!(
        "SELECT {DISP_COLS} FROM dispensacion d \
         JOIN formula f ON f.id = d.formula_id \
         JOIN paciente p ON p.id = d.paciente_id \
         JOIN item_inventario i ON i.id = d.item_inventario_id \
         LEFT JOIN usuario u ON u.id = d.regente_farmacia_id \
         WHERE d.id = ?"
    ))
    .bind(dispensacion_id)
    .fetch_one(pool)
    .await?;
    Ok(ResultadoDispensa::Hecha(Box::new(vista)))
}

/// Dispensaciones del ámbito, de la más reciente a la más antigua. `pagina` es
/// 1-based; una página fuera de rango devuelve vacío, no error.
pub async fn dispensaciones(
    pool: &crate::Pool,
    ambito: Ambito,
    pagina: i64,
) -> Result<Vec<Dispensacion>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "i.institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT {DISP_COLS} FROM dispensacion d \
         JOIN formula f ON f.id = d.formula_id \
         JOIN paciente p ON p.id = d.paciente_id \
         JOIN item_inventario i ON i.id = d.item_inventario_id \
         LEFT JOIN usuario u ON u.id = d.regente_farmacia_id \
         WHERE {filtro} ORDER BY d.fecha_dispensacion DESC, d.id DESC LIMIT ? OFFSET ?"
    );
    let mut q = sqlx::query_as::<_, Dispensacion>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.bind(POR_PAGINA)
        .bind((pagina.max(1) - 1) * POR_PAGINA)
        .fetch_all(pool)
        .await
}

/// Movimientos del ámbito, opcionalmente de un item y de un tipo.
pub async fn movimientos(
    pool: &crate::Pool,
    ambito: Ambito,
    item_id: Option<i64>,
    tipo: Option<&str>,
    pagina: i64,
) -> Result<Vec<Movimiento>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "i.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT m.id, m.item_inventario_id, m.usuario_id, m.fecha, m.tipo_movimiento, \
         m.cantidad, m.motivo, m.paciente_id, m.formula_id, \
         u.nombre AS usuario_nombre, i.nombre AS item_nombre, \
         m.proveedor_id, pr.razon_social AS proveedor_nombre, m.documento_soporte \
         FROM movimiento_inventario m \
         JOIN item_inventario i ON i.id = m.item_inventario_id \
         LEFT JOIN usuario u ON u.id = m.usuario_id \
         LEFT JOIN proveedor pr ON pr.id = m.proveedor_id \
         WHERE {filtro}"
    );
    if item_id.is_some() {
        sql.push_str(" AND m.item_inventario_id = ?");
    }
    if tipo.is_some() {
        sql.push_str(" AND m.tipo_movimiento = ?");
    }
    sql.push_str(" ORDER BY m.fecha DESC, m.id DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, Movimiento>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(it) = item_id {
        q = q.bind(it);
    }
    if let Some(t) = tipo {
        q = q.bind(t.to_string());
    }
    q.bind(POR_PAGINA)
        .bind((pagina.max(1) - 1) * POR_PAGINA)
        .fetch_all(pool)
        .await
}

/// Resumen del inventario del ámbito.
pub async fn stats(pool: &crate::Pool, ambito: Ambito, hoy: &str) -> Result<Stats, sqlx::Error> {
    let vacio = Stats {
        total_items: 0,
        stock_bajo: 0,
        por_vencer: 0,
        vencidos: 0,
        formulas_pendientes: 0,
        valor_total_inventario: 0.0,
    };
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(vacio);
    };
    let aviso = limite_aviso(hoy);
    // `por_vencer` y `vencidos` son conjuntos DISJUNTOS: lo ya vencido no cuenta
    // como «por vencer». Si se solaparan, la suma de las dos cifras engañaría
    // sobre cuántos lotes hay realmente en riesgo.
    let sql = format!(
        "SELECT COUNT(*), \
         COALESCE(SUM(cantidad_actual <= cantidad_minima), 0), \
         COALESCE(SUM(fecha_vencimiento IS NOT NULL AND fecha_vencimiento >= ? AND fecha_vencimiento <= ?), 0), \
         COALESCE(SUM(fecha_vencimiento IS NOT NULL AND fecha_vencimiento < ?), 0), \
         COALESCE(SUM(cantidad_actual * costo_unitario), 0.0) \
         FROM item_inventario WHERE {filtro}"
    );
    let mut q = sqlx::query_as::<_, (i64, i64, i64, i64, f64)>(&sql)
        .bind(hoy)
        .bind(&aviso)
        .bind(hoy);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let (total, bajo, por_vencer, vencidos, valor) = q.fetch_one(pool).await?;

    let (filtro_p, inst_p) = filtro_ambito(ambito, "p.institucion_id").expect("ámbito ya validado");
    let sql_f = format!(
        "SELECT COUNT(*) FROM formula f \
         JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id \
         WHERE f.dispensada = 0 AND {filtro_p}"
    );
    let mut qf = sqlx::query_scalar::<_, i64>(&sql_f);
    if let Some(i) = inst_p {
        qf = qf.bind(i);
    }
    let pendientes = qf.fetch_one(pool).await?;

    Ok(Stats {
        total_items: total,
        stock_bajo: bajo,
        por_vencer,
        vencidos,
        formulas_pendientes: pendientes,
        valor_total_inventario: valor,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const HOY: &str = "2026-08-01";

    /// Dos instituciones, cada una con su paciente, su historia y su fórmula.
    /// El usuario 5 es de la 1 y el 6 de la 2.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES \
             (5,'Regente Uno','r1@x.co','h','regente_farmacia',1,1), \
             (6,'Regente Dos','r2@x.co','h','regente_farmacia',2,1)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento) \
             VALUES (100,'HC1',10,5,'m','e','J00','plan'),(200,'HC2',20,6,'m','e','J00','plan')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO formula (id,historia_id,medicamento,dosis,cantidad,unidad) VALUES \
             (1000,100,'Acetaminofén 500mg','1 cada 8h',30,'tableta'), \
             (2000,200,'Ibuprofeno 400mg','1 cada 12h',20,'tableta')",
        ).execute(&pool).await.unwrap();
        pool
    }

    fn item(nombre: &str, cantidad: i64) -> ItemNuevo {
        ItemNuevo {
            nombre: nombre.into(),
            tipo: None,
            categoria: Some("Analgésico".into()),
            presentacion: Some("Tableta".into()),
            lote: Some("L-001".into()),
            fecha_vencimiento: Some("2027-01-01".into()),
            registro_invima: Some("INVIMA 2020M-1".into()),
            cantidad_actual: Some(cantidad),
            cantidad_minima: Some(10),
            unidad_medida: Some("TABLETA".into()),
            ubicacion: Some("FARMACIA".into()),
            costo_unitario: Some(150.0),
            requiere_refrigeracion: Some(false),
            controlado: Some(false),
        }
    }

    fn dispensa(formula: i64, item: i64, cantidad: i64) -> DispensarNueva {
        DispensarNueva {
            formula_id: formula,
            item_inventario_id: item,
            cantidad_dispensada: cantidad,
            lote_dispensado: String::new(),
            observaciones: None,
        }
    }

    #[tokio::test]
    async fn crear_y_listar_calcula_valor_y_alertas() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100))
            .await.unwrap().unwrap();
        let items = listar(&pool, Ambito::institucion(1), &Filtros::default(), HOY).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, id);
        assert_eq!(items[0].valor_total, 100.0 * 150.0);
        assert!(!items[0].alerta_stock, "100 sobre un mínimo de 10 no es stock bajo");
        assert!(!items[0].alerta_vencimiento, "vence en 2027, no en los próximos 90 días");
    }

    /// El inventario de otra institución no se ve, no se toca y no se borra.
    /// Es LO que la tabla del original no podía cumplir: no tenía dueño.
    #[tokio::test]
    async fn el_inventario_ajeno_es_indistinguible_del_inexistente() {
        let pool = base().await;
        let de_la_uno = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100))
            .await.unwrap().unwrap();

        assert!(listar(&pool, Ambito::institucion(2), &Filtros::default(), HOY).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::institucion(2), de_la_uno, HOY).await.unwrap().is_none());
        assert!(!actualizar(&pool, Ambito::institucion(2), de_la_uno, &ItemEdicion {
            nombre: Some("Robado".into()), ..Default::default()
        }).await.unwrap());
        assert_eq!(
            eliminar(&pool, Ambito::institucion(2), de_la_uno).await.unwrap(),
            ResultadoBorrado::NoExiste
        );
        // Y no puede descontarlo dispensando contra él.
        assert_eq!(
            dispensar(&pool, Ambito::institucion(2), 6, &dispensa(2000, de_la_uno, 5), HOY).await.unwrap(),
            ResultadoDispensa::NoExiste
        );
        // Sigue intacto.
        assert_eq!(obtener(&pool, Ambito::institucion(1), de_la_uno, HOY).await.unwrap().unwrap().cantidad_actual, 100);
    }

    #[tokio::test]
    async fn crear_en_institucion_ajena_no_se_permite() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::institucion(1), 2, &item("Ajeno", 1)).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn la_alerta_de_vencimiento_se_enciende_dentro_del_plazo_y_no_antes() {
        let pool = base().await;
        let mut i = item("Amoxicilina", 50);
        i.fecha_vencimiento = Some("2026-10-01".into()); // a 61 días de HOY
        let id = crear(&pool, Ambito::institucion(1), 1, &i).await.unwrap().unwrap();

        let dentro = obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap();
        assert!(dentro.alerta_vencimiento, "a 61 días debe avisar");
        // Situados un año antes, los mismos datos no avisan.
        let antes = obtener(&pool, Ambito::institucion(1), id, "2025-08-01").await.unwrap().unwrap();
        assert!(!antes.alerta_vencimiento, "a más de 90 días no debe avisar");
    }

    /// «Ver solo alertas» trae lo que está bajo mínimo Y TAMBIÉN lo que vence,
    /// no la intersección de ambos. Con AND habría devuelto uno; con OR, dos.
    #[tokio::test]
    async fn solo_alertas_es_la_union_y_no_la_interseccion() {
        let pool = base().await;
        // Sano: 100 unidades, vence en 2027.
        crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100)).await.unwrap();
        // Bajo mínimo, sin vencimiento.
        let mut bajo = item("Gasa estéril", 2);
        bajo.lote = Some("L-G".into());
        bajo.fecha_vencimiento = None;
        crear(&pool, Ambito::institucion(1), 1, &bajo).await.unwrap();
        // Con stock de sobra, pero vence dentro del plazo.
        let mut pronto = item("Amoxicilina", 500);
        pronto.lote = Some("L-A".into());
        pronto.fecha_vencimiento = Some("2026-09-01".into());
        crear(&pool, Ambito::institucion(1), 1, &pronto).await.unwrap();

        let f = Filtros { solo_alertas: Some(true), ..Default::default() };
        let alertados = listar(&pool, Ambito::institucion(1), &f, HOY).await.unwrap();
        let nombres: Vec<&str> = alertados.iter().map(|i| i.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["Amoxicilina", "Gasa estéril"], "los dos, no su intersección");

        // Y cada bandera por separado sigue trayendo lo suyo.
        let solo_bajo = listar(&pool, Ambito::institucion(1),
            &Filtros { stock_bajo: Some(true), ..Default::default() }, HOY).await.unwrap();
        assert_eq!(solo_bajo.len(), 1);
        assert_eq!(solo_bajo[0].nombre, "Gasa estéril");
    }

    #[tokio::test]
    async fn la_entrada_sube_existencias_y_deja_movimiento() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 10))
            .await.unwrap().unwrap();
        let e = Entrada { cantidad: 40, motivo: "Compra".into(), lote: None, fecha_vencimiento: None, costo_unitario: Some(200.0), proveedor_id: None, documento_soporte: Some("FV-1024".into()) };
        let ResultadoEntrada::Hecha(despues) = entrada(&pool, Ambito::institucion(1), id, 5, &e, HOY).await.unwrap() else {
            panic!("la entrada debía registrarse");
        };
        assert_eq!(despues.cantidad_actual, 50);
        assert_eq!(despues.costo_unitario, 200.0);
        let movs = movimientos(&pool, Ambito::institucion(1), Some(id), None, 1).await.unwrap();
        assert_eq!(movs.len(), 1);
        assert_eq!(movs[0].tipo_movimiento, "ENTRADA");
        assert_eq!(movs[0].cantidad, 40);
        assert_eq!(movs[0].usuario_nombre.as_deref(), Some("Regente Uno"));
    }

    /// La remesa dice DE QUIÉN vino, y un proveedor ajeno o inactivo no cuela.
    ///
    /// Lo segundo importa más de lo que parece: sin la comprobación, un id
    /// inválido se guardaría igual y la entrada quedaría con una procedencia que
    /// el usuario cree haber registrado y que no lleva a ninguna parte.
    #[tokio::test]
    async fn la_entrada_registra_el_proveedor_y_rechaza_el_que_no_vale() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 10))
            .await.unwrap().unwrap();
        let crate::proveedor::ResultadoAlta::Creado(prov) = crate::proveedor::crear(
            &pool, Ambito::institucion(1), 1,
            &crate::proveedor::ProveedorNuevo {
                razon_social: "Distribuidora Andina".into(), nit: Some("900123456-1".into()),
                contacto: None, telefono: None, email: None, direccion: None, ciudad: None,
                suministra: None, observaciones: None,
            }).await.unwrap()
        else { panic!("debía crearse el proveedor") };
        // Uno de la OTRA institución, para probar que no se acepta.
        let crate::proveedor::ResultadoAlta::Creado(ajeno) = crate::proveedor::crear(
            &pool, Ambito::institucion(2), 2,
            &crate::proveedor::ProveedorNuevo {
                razon_social: "Insumos del Valle".into(), nit: None, contacto: None,
                telefono: None, email: None, direccion: None, ciudad: None,
                suministra: None, observaciones: None,
            }).await.unwrap()
        else { panic!() };

        let base_entrada = |p: Option<i64>| Entrada {
            cantidad: 40, motivo: "Compra".into(), lote: None, fecha_vencimiento: None,
            costo_unitario: None, proveedor_id: p, documento_soporte: Some("FV-1024".into()),
        };

        // El ajeno se rechaza, y NADA se escribe.
        assert_eq!(
            entrada(&pool, Ambito::institucion(1), id, 5, &base_entrada(Some(ajeno)), HOY).await.unwrap(),
            ResultadoEntrada::ProveedorNoValido
        );
        assert_eq!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().cantidad_actual, 10);
        assert!(movimientos(&pool, Ambito::institucion(1), None, None, 1).await.unwrap().is_empty());

        // El propio y activo sí, y el movimiento lo lleva con nombre y factura.
        assert!(matches!(
            entrada(&pool, Ambito::institucion(1), id, 5, &base_entrada(Some(prov)), HOY).await.unwrap(),
            ResultadoEntrada::Hecha(_)
        ));
        let movs = movimientos(&pool, Ambito::institucion(1), Some(id), None, 1).await.unwrap();
        assert_eq!(movs[0].proveedor_id, Some(prov));
        assert_eq!(movs[0].proveedor_nombre.as_deref(), Some("Distribuidora Andina"));
        assert_eq!(movs[0].documento_soporte.as_deref(), Some("FV-1024"));

        // Desactivarlo lo saca del juego: la donación siguiente ya no se le apunta.
        crate::proveedor::actualizar(&pool, Ambito::institucion(1), prov,
            &crate::proveedor::ProveedorEdicion { activo: Some(false), ..Default::default() }).await.unwrap();
        assert_eq!(
            entrada(&pool, Ambito::institucion(1), id, 5, &base_entrada(Some(prov)), HOY).await.unwrap(),
            ResultadoEntrada::ProveedorNoValido
        );
        // Y sin proveedor la entrada SÍ pasa: una donación no tiene vendedor.
        assert!(matches!(
            entrada(&pool, Ambito::institucion(1), id, 5, &base_entrada(None), HOY).await.unwrap(),
            ResultadoEntrada::Hecha(_)
        ));
    }

    /// Un lote nuevo NO se suma al que ya hay: se rechaza y el stock no se toca.
    #[tokio::test]
    async fn un_lote_distinto_no_se_mezcla_con_el_que_queda() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 5))
            .await.unwrap().unwrap();
        let e = Entrada { cantidad: 20, motivo: "Compra".into(), lote: Some("L-999".into()), fecha_vencimiento: None, costo_unitario: None, proveedor_id: None, documento_soporte: None };
        assert_eq!(
            entrada(&pool, Ambito::institucion(1), id, 5, &e, HOY).await.unwrap(),
            ResultadoEntrada::LoteDistinto { actual: "L-001".into(), quedan: 5 }
        );
        assert_eq!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().cantidad_actual, 5);

        // Con la fila en cero sí se reutiliza: no hay nada con qué mezclar.
        sqlx::query("UPDATE item_inventario SET cantidad_actual = 0 WHERE id = ?")
            .bind(id).execute(&pool).await.unwrap();
        let ResultadoEntrada::Hecha(despues) = entrada(&pool, Ambito::institucion(1), id, 5, &e, HOY).await.unwrap() else {
            panic!("con la fila vacía el lote nuevo sí entra");
        };
        assert_eq!(despues.lote, "L-999");
        assert_eq!(despues.cantidad_actual, 20);
    }

    #[tokio::test]
    async fn dispensar_baja_stock_cierra_la_formula_y_deja_los_dos_rastros() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100))
            .await.unwrap().unwrap();
        let ResultadoDispensa::Hecha(d) = dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 30), HOY).await.unwrap() else {
            panic!("debía dispensar");
        };
        assert_eq!(d.cantidad_dispensada, 30);
        assert_eq!(d.paciente_id, 10, "el paciente sale de la fórmula, no del cliente");
        assert_eq!(d.lote_dispensado, "L-001");
        assert_eq!(d.regente_nombre.as_deref(), Some("Regente Uno"));

        assert_eq!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().cantidad_actual, 70);
        assert!(formulas_pendientes(&pool, Ambito::institucion(1), None).await.unwrap().is_empty(),
                "entregado todo lo prescrito, la fórmula deja de estar pendiente");
        let movs = movimientos(&pool, Ambito::institucion(1), Some(id), Some("SALIDA"), 1).await.unwrap();
        assert_eq!(movs.len(), 1);
        assert_eq!(movs[0].formula_id, Some(1000));
        assert_eq!(dispensaciones(&pool, Ambito::institucion(1), 1).await.unwrap().len(), 1);
    }

    /// La entrega parcial existe y la fórmula sigue pendiente hasta completarse.
    #[tokio::test]
    async fn la_entrega_parcial_no_cierra_la_formula() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100))
            .await.unwrap().unwrap();
        assert!(matches!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 20), HOY).await.unwrap(),
            ResultadoDispensa::Hecha(_)
        ));
        assert_eq!(formulas_pendientes(&pool, Ambito::institucion(1), None).await.unwrap().len(), 1,
                   "quedan 10 por entregar: sigue pendiente");
        // Y no se puede pedir más de lo que falta.
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 11), HOY).await.unwrap(),
            ResultadoDispensa::ExcedePrescrito { pendiente: 10.0 }
        );
        assert!(matches!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 10), HOY).await.unwrap(),
            ResultadoDispensa::Hecha(_)
        ));
        assert!(formulas_pendientes(&pool, Ambito::institucion(1), None).await.unwrap().is_empty());
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 1), HOY).await.unwrap(),
            ResultadoDispensa::YaCompleta
        );
    }

    /// Las cinco negativas, cada una con su motivo y sin tocar el stock.
    #[tokio::test]
    async fn dispensar_discrimina_los_cinco_motivos_de_rechazo() {
        let pool = base().await;
        let bueno = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 5))
            .await.unwrap().unwrap();
        let mut caducado = item("Amoxicilina", 100);
        caducado.lote = Some("L-VIEJO".into());
        caducado.fecha_vencimiento = Some("2026-01-01".into());
        let caducado = crear(&pool, Ambito::institucion(1), 1, &caducado).await.unwrap().unwrap();

        // 1) cantidad no positiva
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, bueno, 0), HOY).await.unwrap(),
            ResultadoDispensa::CantidadInvalida
        );
        // 2) lote vencido
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, caducado, 1), HOY).await.unwrap(),
            ResultadoDispensa::LoteVencido { vencio: "2026-01-01".into() }
        );
        // 3) lote afirmado que no es el del item
        let mut miente = dispensa(1000, bueno, 1);
        miente.lote_dispensado = "L-OTRO".into();
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &miente, HOY).await.unwrap(),
            ResultadoDispensa::LoteNoCoincide { del_item: "L-001".into() }
        );
        // 4) más de lo prescrito (30)
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, bueno, 31), HOY).await.unwrap(),
            ResultadoDispensa::ExcedePrescrito { pendiente: 30.0 }
        );
        // 5) más de lo que hay (5), pidiendo 10 de las 30 prescritas
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, bueno, 10), HOY).await.unwrap(),
            ResultadoDispensa::SinExistencias { disponible: 5 }
        );
        // Ninguna de las cinco movió nada.
        assert_eq!(obtener(&pool, Ambito::institucion(1), bueno, HOY).await.unwrap().unwrap().cantidad_actual, 5);
        assert!(dispensaciones(&pool, Ambito::institucion(1), 1).await.unwrap().is_empty());
        assert!(movimientos(&pool, Ambito::institucion(1), None, None, 1).await.unwrap().is_empty());
    }

    /// Y DISCRIMINA: con el mismo item y la misma cantidad, situados un día antes
    /// del vencimiento la entrega se hace. El rechazo era por la fecha, no porque
    /// la función rechace siempre.
    #[tokio::test]
    async fn el_lote_vencido_se_acepta_el_dia_antes_de_vencer() {
        let pool = base().await;
        let mut i = item("Amoxicilina", 100);
        i.fecha_vencimiento = Some("2026-01-01".into());
        let id = crear(&pool, Ambito::institucion(1), 1, &i).await.unwrap().unwrap();
        // El día del vencimiento todavía sirve; el siguiente ya no.
        assert!(matches!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 1), "2026-01-01").await.unwrap(),
            ResultadoDispensa::Hecha(_)
        ));
        assert_eq!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 1), "2026-01-02").await.unwrap(),
            ResultadoDispensa::LoteVencido { vencio: "2026-01-01".into() }
        );
    }

    #[tokio::test]
    async fn un_item_con_historial_no_se_borra() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100))
            .await.unwrap().unwrap();
        // Sin historial sí se borra.
        let otro = crear(&pool, Ambito::institucion(1), 1, &item("Gasa estéril", 0)).await.unwrap().unwrap();
        assert_eq!(eliminar(&pool, Ambito::institucion(1), otro).await.unwrap(), ResultadoBorrado::Hecho);

        assert!(matches!(
            dispensar(&pool, Ambito::institucion(1), 5, &dispensa(1000, id, 30), HOY).await.unwrap(),
            ResultadoDispensa::Hecha(_)
        ));
        assert_eq!(
            eliminar(&pool, Ambito::institucion(1), id).await.unwrap(),
            ResultadoBorrado::TieneHistorial { movimientos: 1, dispensaciones: 1 }
        );
        assert!(obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn las_formulas_pendientes_son_solo_las_del_ambito() {
        let pool = base().await;
        let de_uno = formulas_pendientes(&pool, Ambito::institucion(1), None).await.unwrap();
        assert_eq!(de_uno.len(), 1);
        assert_eq!(de_uno[0].id, 1000);
        assert_eq!(de_uno[0].paciente_nombre.as_deref(), Some("Ana Ruiz"));
        assert_eq!(de_uno[0].medico_nombre.as_deref(), Some("Regente Uno"));
        let de_dos = formulas_pendientes(&pool, Ambito::institucion(2), None).await.unwrap();
        assert_eq!(de_dos.len(), 1);
        assert_eq!(de_dos[0].id, 2000);
        // Y la búsqueda filtra.
        assert!(formulas_pendientes(&pool, Ambito::institucion(1), Some("Ibuprofeno")).await.unwrap().is_empty());
        assert_eq!(formulas_pendientes(&pool, Ambito::institucion(1), Some("Acetamin")).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn las_cifras_del_resumen_cuentan_lo_del_ambito_y_no_solapan() {
        let pool = base().await;
        // 100 unidades vigentes, 5 bajo mínimo, uno por vencer y uno vencido.
        crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100)).await.unwrap();
        let mut bajo = item("Gasa estéril", 3);
        bajo.lote = Some("L-G".into());
        bajo.fecha_vencimiento = None;
        crear(&pool, Ambito::institucion(1), 1, &bajo).await.unwrap();
        let mut pronto = item("Amoxicilina", 50);
        pronto.lote = Some("L-A".into());
        pronto.fecha_vencimiento = Some("2026-09-01".into());
        crear(&pool, Ambito::institucion(1), 1, &pronto).await.unwrap();
        let mut caduco = item("Dipirona", 20);
        caduco.lote = Some("L-D".into());
        caduco.fecha_vencimiento = Some("2026-05-01".into());
        crear(&pool, Ambito::institucion(1), 1, &caduco).await.unwrap();

        let s = stats(&pool, Ambito::institucion(1), HOY).await.unwrap();
        assert_eq!(s.total_items, 4);
        assert_eq!(s.stock_bajo, 1, "solo la gasa está en o bajo el mínimo");
        assert_eq!(s.por_vencer, 1, "el vencido NO cuenta como por vencer");
        assert_eq!(s.vencidos, 1);
        assert_eq!(s.formulas_pendientes, 1);
        assert_eq!(s.valor_total_inventario, (100 + 3 + 50 + 20) as f64 * 150.0);

        // La institución 2 no tiene inventario, y su resumen lo dice sin mentir.
        let s2 = stats(&pool, Ambito::institucion(2), HOY).await.unwrap();
        assert_eq!(s2.total_items, 0);
        assert_eq!(s2.valor_total_inventario, 0.0);
        assert_eq!(s2.formulas_pendientes, 1, "la suya, no la de la 1");
    }

    /// `Ambito::Ninguno` no ve nada y no puede hacer nada. Sin esto, un usuario
    /// sin institución y sin ser superadmin caería en el `1 = 1` del superadmin.
    #[tokio::test]
    async fn sin_ambito_no_se_ve_ni_se_dispensa() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 100)).await.unwrap().unwrap();
        assert!(listar(&pool, Ambito::Ninguno, &Filtros::default(), HOY).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::Ninguno, id, HOY).await.unwrap().is_none());
        assert!(formulas_pendientes(&pool, Ambito::Ninguno, None).await.unwrap().is_empty());
        assert!(movimientos(&pool, Ambito::Ninguno, None, None, 1).await.unwrap().is_empty());
        assert!(dispensaciones(&pool, Ambito::Ninguno, 1).await.unwrap().is_empty());
        assert_eq!(stats(&pool, Ambito::Ninguno, HOY).await.unwrap().total_items, 0);
        assert_eq!(
            dispensar(&pool, Ambito::Ninguno, 5, &dispensa(1000, id, 1), HOY).await.unwrap(),
            ResultadoDispensa::NoExiste
        );
        assert!(crear(&pool, Ambito::Ninguno, 1, &item("Nuevo", 1)).await.unwrap().is_none());
    }

    /// Directiva 8. Cien dispensaciones concurrentes de 1 unidad contra un stock
    /// de 40 y una fórmula de 100: no importa el orden ni el entrelazado, tienen
    /// que salir EXACTAMENTE 40 entregas y el stock quedar en 0 — nunca en
    /// negativo, que es el fallo clásico de leer-y-luego-escribir sin condición.
    /// Con timeout, para que un interbloqueo no se confunda con lentitud.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_dispensaciones_concurrentes_no_dejan_el_stock_en_negativo() {
        let pool = base().await;
        sqlx::query("UPDATE formula SET cantidad = 100 WHERE id = 1000")
            .execute(&pool).await.unwrap();
        let id = crear(&pool, Ambito::institucion(1), 1, &item("Acetaminofén", 40))
            .await.unwrap().unwrap();

        let mut tareas = Vec::new();
        for _ in 0..100 {
            let p = pool.clone();
            tareas.push(tokio::spawn(async move {
                dispensar(&p, Ambito::institucion(1), 5, &dispensa(1000, id, 1), HOY).await
            }));
        }
        let plazo = std::time::Duration::from_secs(120);
        let mut hechas = 0;
        let mut sin_existencias = 0;
        let mut otras = 0;
        for t in tareas {
            let r = tokio::time::timeout(plazo, t).await
                .expect("una dispensación no respondió: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic")
                .expect("ninguna debía fallar por SQL");
            match r {
                ResultadoDispensa::Hecha(_) => hechas += 1,
                ResultadoDispensa::SinExistencias { .. } => sin_existencias += 1,
                _ => otras += 1,
            }
        }
        assert_eq!(hechas, 40, "se entregaron {hechas} de 40 existencias");
        assert_eq!(sin_existencias, 60);
        assert_eq!(otras, 0);
        let quedan = obtener(&pool, Ambito::institucion(1), id, HOY).await.unwrap().unwrap().cantidad_actual;
        assert_eq!(quedan, 0);
        assert_eq!(dispensaciones(&pool, Ambito::institucion(1), 1).await.unwrap().len(), 40);
        // Y el histórico cuadra con el stock: 40 salidas de 1.
        let salidas: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(cantidad),0) FROM movimiento_inventario WHERE tipo_movimiento='SALIDA'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(salidas, 40);
    }
}
