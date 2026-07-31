//! Catálogos de referencia: especialidades, medicamentos (CUM) y unidades de
//! dispensación (M13). Son datos compartidos por todas las instituciones, así
//! que aquí no hay `Ambito`: no hay nada que aislar. La autorización de leerlos
//! («estar autenticado») vive en la capa de rutas.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Especialidad {
    pub id: i64,
    pub nombre: String,
}

pub async fn especialidades(pool: &crate::Pool) -> Result<Vec<Especialidad>, sqlx::Error> {
    sqlx::query_as::<_, Especialidad>("SELECT id, nombre FROM especialidad ORDER BY nombre")
        .fetch_all(pool)
        .await
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Medicamento {
    pub id: i64,
    pub codigo_cum: String,
    pub nombre: String,
    pub concentracion: String,
    pub forma_farmaceutica: String,
    pub titular: Option<String>,
}

/// Busca medicamentos por nombre o por código CUM. Devuelve como máximo 50: es un
/// autocompletar, no un volcado del maestro (que tiene decenas de miles de filas).
/// Una consulta vacía devuelve vacío en vez de todo el catálogo.
pub async fn buscar_medicamentos(pool: &crate::Pool, q: &str) -> Result<Vec<Medicamento>, sqlx::Error> {
    let q = q.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // El patrón se arma con parámetro (`?`), nunca interpolando `q` en el SQL.
    // Se escapan los comodines de LIKE para que un `%` escrito por el usuario
    // busque un `%` literal y no «cualquier cosa».
    let patron = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    sqlx::query_as::<_, Medicamento>(
        "SELECT id, codigo_cum, nombre, concentracion, forma_farmaceutica, titular \
         FROM medicamento \
         WHERE nombre LIKE ?1 ESCAPE '\\' OR codigo_cum LIKE ?1 ESCAPE '\\' \
         ORDER BY nombre LIMIT 50",
    )
    .bind(&patron)
    .fetch_all(pool)
    .await
}

/// Una fila lista para cargar en el maestro de medicamentos. `codigo_cum` es la
/// clave: recargar el mismo CUM ACTUALIZA la fila, no la duplica ni falla, para
/// que volver a correr un maestro corregido sea idempotente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedicamentoNuevo {
    pub codigo_cum: String,
    pub nombre: String,
    pub concentracion: String,
    pub forma_farmaceutica: String,
    pub titular: Option<String>,
    /// `estadocum` del export: 22 % del maestro está INACTIVO. Se carga igual
    /// —hace falta para leer una fórmula antigua— pero marcado, porque
    /// prescribirlo hoy es un rechazo del MUV.
    pub activo: bool,
}

/// Carga (o actualiza) un lote de medicamentos en UNA sola transacción: el
/// maestro entra entero o no entra. Si una fila falla, se revierte todo y el
/// catálogo queda como estaba, nunca a medio cargar. Devuelve cuántas filas se
/// insertaron o actualizaron.
pub async fn cargar_medicamentos(
    pool: &crate::Pool,
    lote: &[MedicamentoNuevo],
) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let mut n = 0u64;
    for m in lote {
        let r = sqlx::query(
            "INSERT INTO medicamento (codigo_cum, nombre, concentracion, forma_farmaceutica, titular, activo) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(codigo_cum) DO UPDATE SET \
               nombre = excluded.nombre, \
               concentracion = excluded.concentracion, \
               forma_farmaceutica = excluded.forma_farmaceutica, \
               titular = excluded.titular, \
               activo = excluded.activo",
        )
        .bind(&m.codigo_cum)
        .bind(&m.nombre)
        .bind(&m.concentracion)
        .bind(&m.forma_farmaceutica)
        .bind(&m.titular)
        .bind(i64::from(m.activo))
        .execute(&mut *tx)
        .await?;
        n += r.rows_affected();
    }
    tx.commit().await?;
    Ok(n)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UnidadDispensacion {
    pub codigo: i64,
    pub nombre: String,
}

pub async fn unidades_dispensacion(pool: &crate::Pool) -> Result<Vec<UnidadDispensacion>, sqlx::Error> {
    sqlx::query_as::<_, UnidadDispensacion>(
        "SELECT codigo, nombre FROM unidad_dispensacion ORDER BY nombre",
    )
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[tokio::test]
    async fn las_especialidades_vienen_sembradas_y_ordenadas() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        let es = especialidades(&pool).await.unwrap();
        assert!(es.len() >= 39, "la migración siembra las especialidades base");
        // Orden alfabético: ANESTESIOLOGIA antes que MEDICINA GENERAL.
        let anest = es.iter().position(|e| e.nombre == "ANESTESIOLOGIA").unwrap();
        let mgen = es.iter().position(|e| e.nombre == "MEDICINA GENERAL").unwrap();
        assert!(anest < mgen);
    }

    #[tokio::test]
    async fn el_medicamento_esta_vacio_hasta_que_se_cargue_el_maestro() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        // Sin datos, una búsqueda válida devuelve vacío, no un error ni un relleno.
        assert!(buscar_medicamentos(&pool, "acetaminofen").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn una_busqueda_vacia_no_devuelve_todo() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO medicamento (codigo_cum, nombre) VALUES ('19900001-1','ACETAMINOFEN 500MG'),('19900002-1','IBUPROFENO 400MG')",
        )
        .execute(&pool).await.unwrap();
        assert!(buscar_medicamentos(&pool, "   ").await.unwrap().is_empty());
        // Y una búsqueda con texto sí encuentra por nombre.
        let r = buscar_medicamentos(&pool, "ibupro").await.unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].nombre, "IBUPROFENO 400MG");
    }

    #[tokio::test]
    async fn el_comodin_del_usuario_se_toma_literal() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO medicamento (codigo_cum, nombre) VALUES ('1-1','ACETAMINOFEN'),('2-1','IBUPROFENO')",
        )
        .execute(&pool).await.unwrap();
        // Un '%' escrito por el usuario busca un '%' real: no debe traer todo.
        assert!(buscar_medicamentos(&pool, "%").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cargar_medicamentos_no_duplica_y_actualiza_el_mismo_cum() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        let lote = vec![MedicamentoNuevo {
            codigo_cum: "20048021-41".into(),
            nombre: "CARDIK 3".into(),
            concentracion: String::new(),
            forma_farmaceutica: "TABLETA".into(),
            titular: Some("LEGRAND".into()),
            activo: true,
        }];
        assert_eq!(cargar_medicamentos(&pool, &lote).await.unwrap(), 1);

        // Recargar el MISMO CUM con datos corregidos: no crea una segunda fila,
        // actualiza la que hay (upsert idempotente).
        let corregido = vec![MedicamentoNuevo {
            codigo_cum: "20048021-41".into(),
            nombre: "CARDIK 3 CORREGIDO".into(),
            concentracion: String::new(),
            forma_farmaceutica: "TABLETA RECUBIERTA".into(),
            titular: None,
            // Y de paso pasa a INACTIVO: es lo que ocurre de verdad cuando el
            // INVIMA retira un registro. El upsert tiene que reflejarlo, o el
            // sistema seguiría dejando prescribir algo que ya no se factura.
            activo: false,
        }];
        cargar_medicamentos(&pool, &corregido).await.unwrap();

        let r = buscar_medicamentos(&pool, "cardik").await.unwrap();
        assert_eq!(r.len(), 1, "el mismo CUM no se duplica al recargar");
        assert_eq!(r[0].nombre, "CARDIK 3 CORREGIDO");
        assert_eq!(r[0].forma_farmaceutica, "TABLETA RECUBIERTA");
        // El estado también se actualiza: era activo y dejó de serlo.
        let (activo,): (i64,) =
            sqlx::query_as("SELECT activo FROM medicamento WHERE codigo_cum = '20048021-41'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(activo, 0, "un medicamento retirado debe quedar marcado al recargar");
        // Y se encuentra también tecleando el propio código CUM.
        let por_cum = buscar_medicamentos(&pool, "20048021-41").await.unwrap();
        assert_eq!(por_cum.len(), 1);
    }

    #[tokio::test]
    async fn las_unidades_estan_vacias_hasta_que_se_cargue_el_maestro() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        assert!(unidades_dispensacion(&pool).await.unwrap().is_empty());
    }
}
