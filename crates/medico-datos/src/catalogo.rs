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
/// Busca medicamentos **para prescribir**: solo los ACTIVOS.
///
/// De los 65 997 del maestro, 15 046 están retirados. Ofrecerlos en el buscador
/// de la fórmula sería poner delante del médico exactamente lo que el MUV va a
/// rechazar — y el rechazo llega semanas después como glosa, cuando ya nadie
/// recuerda esa consulta.
///
/// Leer una fórmula ANTIGUA no pasa por aquí: tiene el código y lo busca directo
/// con [`crate::clinico::existe_cum`], que sí encuentra los retirados. Si algún
/// día hace falta un buscador que los incluya, será otra función con otro
/// nombre; una bandera en ésta invitaría a pasar la que toque y equivocarse en
/// silencio.
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
         WHERE activo = 1 \
           AND (nombre LIKE ?1 ESCAPE '\\' OR codigo_cum LIKE ?1 ESCAPE '\\') \
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

/// Un procedimiento del CUPS, como lo necesita un buscador de pantalla.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Cups {
    pub codigo: String,
    pub nombre: String,
    pub seccion: String,
    /// En qué archivo del RIPS se reporta: `AP`, `AC` o `AT`.
    pub ambito_rips: String,
    pub quirurgico: bool,
}

/// **El ámbito del RIPS en el que se busca un procedimiento.**
///
/// Es un enum y no una cadena a propósito: la columna lleva un `CHECK` con tres
/// valores, y pasar `"ac"` en minúscula —o `"CONSULTA"`— habría devuelto vacío
/// sin un solo error, que en un buscador se lee como «no existe ese código».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbitoRips {
    /// Consultas. Es el que pide `historia.cod_consulta_cups`.
    Ac,
    /// Procedimientos.
    Ap,
    /// Otros servicios.
    At,
}

impl AmbitoRips {
    fn como_texto(self) -> &'static str {
        match self {
            AmbitoRips::Ac => "AC",
            AmbitoRips::Ap => "AP",
            AmbitoRips::At => "AT",
        }
    }

    /// Del texto que llega por la URL. `None` si no es uno de los tres — y el
    /// llamante decide, en vez de caer a un defecto que buscaría en el ámbito
    /// equivocado sin decirlo (directiva 20).
    pub fn desde(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "AC" => Some(AmbitoRips::Ac),
            "AP" => Some(AmbitoRips::Ap),
            "AT" => Some(AmbitoRips::At),
            _ => None,
        }
    }
}

/// Busca procedimientos del CUPS por código o por nombre, **acotados al ámbito
/// del RIPS en el que se van a reportar**.
///
/// # Por qué el ámbito no es opcional
///
/// El maestro son 13 640 procedimientos y la columna `ambito_rips` dice en qué
/// archivo se reporta cada uno. Un buscador que los ofreciera todos pondría
/// delante de quien captura una consulta códigos que solo valen en el archivo de
/// procedimientos — y eso el MUV lo rechaza, semanas después y sobre la factura
/// del mes. Es exactamente el mismo argumento por el que
/// [`buscar_medicamentos`] no ofrece los CUM retirados.
///
/// Se pide como parámetro obligatorio y no como filtro opcional: el control que
/// se puede omitir, se omite — es la lección del `institucion_id: Optional` que
/// ya causó una fuga en este proyecto.
///
/// Devuelve como máximo 50: es un autocompletar, no un volcado. Una consulta
/// vacía devuelve vacío en vez del catálogo entero.
pub async fn buscar_cups(
    pool: &crate::Pool,
    q: &str,
    ambito: AmbitoRips,
) -> Result<Vec<Cups>, sqlx::Error> {
    let q = q.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // Mismo tratamiento que en `buscar_medicamentos`: patrón por parámetro,
    // nunca interpolado, y comodines de LIKE escapados para que un `%` escrito
    // por el usuario busque un `%` y no «cualquier cosa».
    let patron = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    sqlx::query_as::<_, Cups>(
        "SELECT codigo, nombre, seccion, ambito_rips, quirurgico \
         FROM cups \
         WHERE ambito_rips = ?2 \
           AND (codigo LIKE ?1 ESCAPE '\\' OR nombre LIKE ?1 ESCAPE '\\') \
         ORDER BY codigo LIMIT 50",
    )
    .bind(&patron)
    .bind(ambito.como_texto())
    .fetch_all(pool)
    .await
}

/// Si ese código existe **y se reporta en ese ámbito**.
///
/// Lo usa la escritura de la historia: un código que el buscador no habría
/// ofrecido tampoco puede entrar por el cuerpo de la petición. Sin esto, la
/// acotación del buscador sería una cortesía de pantalla y no una regla.
pub async fn cups_es_de_ambito(
    pool: &crate::Pool,
    codigo: &str,
    ambito: AmbitoRips,
) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cups WHERE codigo = ?1 AND ambito_rips = ?2",
    )
    .bind(codigo)
    .bind(ambito.como_texto())
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

/// El maestro de unidades mínimas de dispensación (M13 del RIPS), **copiado
/// verbatim** de `backend/datos/maestros_muv/upr.json`, que lo extrae de la
/// tabla `Rips.REFs_UPR` del contenedor del validador FEV-RIPS.
///
/// # Por qué verbatim y no reformateado
///
/// El archivo trae su propia procedencia dentro (`fuente`, `filtro`, `total`),
/// y ésa es la única prueba de que estos 73 códigos son los del MUV y no unos
/// inventados. Transformarlo a un formato propio crearía un segundo archivo que
/// mantener al día y borraría justo el dato que lo hace creíble.
///
/// # El defecto que este maestro cierra
///
/// El 2026-07-26 se escribió a mano un mapa de presentaciones con `tableta = 1`.
/// En el catálogo real el **1 es AMPOLLA** y la **TABLETA es la 66**, y el MUV
/// devolvió CUV igual: valida ese campo por RANGO («valores permitidos entre 1 y
/// 999»), no contra la tabla. Un dato falso con la forma correcta, aceptado.
/// `la_tableta_no_es_la_ampolla` deja eso en rojo si alguien vuelve a inventarlo.
pub(crate) const UPR_JSON: &str = include_str!("../datos/unidad_dispensacion.json");

#[derive(Debug, serde::Deserialize)]
struct ArchivoUpr {
    fuente: String,
    filas: Vec<UprCrudo>,
}

#[derive(Debug, serde::Deserialize)]
struct UprCrudo {
    #[serde(rename = "Codigo")]
    codigo: i64,
    #[serde(rename = "Nombre")]
    nombre: String,
}

/// De dónde salió el maestro, para poder enseñarlo donde importe — igual que la
/// advertencia del CIE-10 y la del CUM.
pub fn fuente_unidades_dispensacion() -> &'static str {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<ArchivoUpr>(UPR_JSON)
            .expect("el JSON empotrado se valida en las pruebas")
            .fuente
    })
}

/// Llena `unidad_dispensacion` desde el JSON empotrado. Idempotente.
///
/// La migración 0010 creó la tabla VACÍA a propósito —«un catálogo vacío da un
/// selector vacío, honesto, y no un valor inventado»—, y así estuvo hasta hoy:
/// el selector de la fórmula (`vista/historias.rs`) llevaba un año ofreciendo
/// una lista de cero elementos. Honesto sí, pero inservible: quien tiene que
/// prescribir no puede elegir nada, y el campo se rellena por otra vía o se
/// queda en blanco. La salida no era inventar el catálogo: era traer el oficial.
pub async fn sembrar_unidades_dispensacion(pool: &crate::Pool) -> Result<usize, sqlx::Error> {
    let archivo: ArchivoUpr = serde_json::from_str(UPR_JSON)
        .expect("datos/unidad_dispensacion.json va empotrado y se valida en las pruebas");

    let mut tx = pool.begin().await?;
    for u in &archivo.filas {
        sqlx::query(
            "INSERT INTO unidad_dispensacion (codigo, nombre) VALUES (?1, ?2)
             ON CONFLICT (codigo) DO UPDATE SET nombre = excluded.nombre",
        )
        .bind(u.codigo)
        .bind(&u.nombre)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(archivo.filas.len())
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

        // Se consulta la TABLA y no el buscador: éste ya solo ofrece activos
        // —correctamente— y aquí se recargó como retirado. Son dos conductas
        // distintas y ambas ciertas; la prueba del upsert mira la tabla.
        let filas: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT nombre, forma_farmaceutica, activo FROM medicamento WHERE codigo_cum = ?",
        )
        .bind("20048021-41")
        .fetch_all(&pool).await.unwrap();
        assert_eq!(filas.len(), 1, "el mismo CUM no se duplica al recargar");
        assert_eq!(filas[0].0, "CARDIK 3 CORREGIDO");
        assert_eq!(filas[0].1, "TABLETA RECUBIERTA");
        assert_eq!(filas[0].2, 0, "un medicamento retirado queda marcado al recargar");

        // Y como quedó retirado, el buscador de la fórmula ya NO lo ofrece:
        // el arreglo llega hasta donde el médico lo toca.
        assert!(buscar_medicamentos(&pool, "cardik").await.unwrap().is_empty());
        assert!(buscar_medicamentos(&pool, "20048021-41").await.unwrap().is_empty());
    }

    /// **El buscador de la fórmula no ofrece lo retirado.** Arreglar la columna
    /// sin arreglar la búsqueda habría dejado el defecto justo donde el médico
    /// lo toca: eligiendo de una lista un medicamento que el MUV rechaza.
    #[tokio::test]
    async fn el_buscador_no_ofrece_medicamentos_retirados() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO medicamento (codigo_cum, nombre, activo) VALUES \
             ('1-1','ASPIRINA VIGENTE',1), ('2-1','ASPIRINA RETIRADA',0)",
        )
        .execute(&pool).await.unwrap();

        let r = buscar_medicamentos(&pool, "aspirina").await.unwrap();
        assert_eq!(r.len(), 1, "solo el vigente");
        assert_eq!(r[0].nombre, "ASPIRINA VIGENTE");

        // Y tampoco se cuela buscándolo por su código exacto.
        assert!(buscar_medicamentos(&pool, "2-1").await.unwrap().is_empty());
        // Pero SIGUE EXISTIENDO para leer una fórmula antigua.
        assert!(crate::clinico::existe_cum(&pool, "2-1").await.unwrap());
    }

    #[tokio::test]
    async fn las_unidades_estan_vacias_hasta_que_se_siembre_el_maestro() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        // La migración crea la tabla vacía; quien la llena es `sembrar`, que
        // corre en cada arranque del binario. Abrir la base no siembra nada.
        assert!(unidades_dispensacion(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn el_maestro_de_unidades_se_siembra_entero_y_ordenado() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        let n = sembrar_unidades_dispensacion(&pool).await.unwrap();
        assert_eq!(n, 73, "el maestro del validador trae 73 unidades habilitadas");

        let us = unidades_dispensacion(&pool).await.unwrap();
        assert_eq!(us.len(), 73);
        // Ordenadas por nombre, que es como las pide el selector de la fórmula.
        let mut por_nombre: Vec<&str> = us.iter().map(|u| u.nombre.as_str()).collect();
        let copia = por_nombre.clone();
        por_nombre.sort_unstable();
        assert_eq!(copia, por_nombre);
    }

    /// **El caso rojo, y viene de un defecto real.**
    ///
    /// El 2026-07-26 se escribió a mano `tableta = 1` y el MUV devolvió CUV:
    /// valida este campo por RANGO, no contra su tabla, así que un código
    /// inventado con la forma correcta pasa. En el maestro de verdad el 1 es
    /// AMPOLLA y la TABLETA es la 66.
    ///
    /// Es la PAREJA de la prueba de arriba: aquélla pasa con cualquier lista de
    /// 73 filas ordenadas —una inventada también—, y ésta no.
    #[tokio::test]
    async fn la_tableta_no_es_la_ampolla() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sembrar_unidades_dispensacion(&pool).await.unwrap();
        let us = unidades_dispensacion(&pool).await.unwrap();
        let codigo = |n: &str| us.iter().find(|u| u.nombre == n).map(|u| u.codigo);

        assert_eq!(codigo("AMPOLLA"), Some(1));
        assert_eq!(codigo("TABLETA"), Some(66), "la TABLETA es la 66, no la 1");
        assert_eq!(codigo("CAPSULA"), Some(14));

        // Y la procedencia viaja con el archivo: es la única prueba de que estos
        // códigos son los del MUV y no los que le parecieron bien a alguien.
        assert!(
            fuente_unidades_dispensacion().contains("REFs_UPR"),
            "el maestro tiene que declarar de qué tabla del validador salió: {}",
            fuente_unidades_dispensacion()
        );
    }

    #[tokio::test]
    async fn sembrar_las_unidades_es_idempotente() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sembrar_unidades_dispensacion(&pool).await.unwrap();
        sembrar_unidades_dispensacion(&pool).await.unwrap();
        assert_eq!(unidades_dispensacion(&pool).await.unwrap().len(), 73);
    }

    // ── El buscador de CUPS (#331, prerrequisito) ────────────────────────────

    async fn con_cups() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO cups (codigo, nombre, seccion, ambito_rips, quirurgico) VALUES
             ('890201','CONSULTA DE PRIMERA VEZ MEDICINA GENERAL','CONSULTA','AC',0),
             ('890301','CONSULTA DE CONTROL MEDICINA GENERAL','CONSULTA','AC',0),
             ('471001','ARTROSCOPIA DE RODILLA','QUIRURGICO','AP',1),
             ('999999','TRASLADO ASISTENCIAL','OTROS','AT',0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    /// **El ámbito acota de verdad**, que es el punto entero: quien captura una
    /// consulta no puede recibir un código que solo vale en el archivo de
    /// procedimientos — el MUV lo rechaza semanas después.
    #[tokio::test]
    async fn el_buscador_acota_por_el_ambito_del_rips() {
        let pool = con_cups().await;

        let ac = buscar_cups(&pool, "consulta", AmbitoRips::Ac).await.unwrap();
        assert_eq!(ac.len(), 2, "las dos consultas");
        assert!(ac.iter().all(|c| c.ambito_rips == "AC"));

        // LA PAREJA: el quirúrgico existe y NO sale al buscar para consultas.
        // Sin esta mitad, un buscador que no filtrara pasaría la de arriba.
        assert!(
            buscar_cups(&pool, "artroscopia", AmbitoRips::Ac).await.unwrap().is_empty(),
            "un procedimiento quirúrgico se ofreció para una CONSULTA"
        );
        // Y sí sale en el suyo, así que no es que la fila no exista.
        assert_eq!(buscar_cups(&pool, "artroscopia", AmbitoRips::Ap).await.unwrap().len(), 1);
    }

    /// Busca por código además de por nombre: es como se teclea un CUPS conocido.
    #[tokio::test]
    async fn se_busca_por_codigo_y_por_nombre() {
        let pool = con_cups().await;
        assert_eq!(buscar_cups(&pool, "890201", AmbitoRips::Ac).await.unwrap().len(), 1);
        assert_eq!(buscar_cups(&pool, "GENERAL", AmbitoRips::Ac).await.unwrap().len(), 2);
    }

    /// Una consulta vacía devuelve VACÍO, no el catálogo entero: son 13 640
    /// filas y esto alimenta un autocompletar.
    #[tokio::test]
    async fn la_busqueda_vacia_no_vuelca_el_catalogo() {
        let pool = con_cups().await;
        assert!(buscar_cups(&pool, "", AmbitoRips::Ac).await.unwrap().is_empty());
        assert!(buscar_cups(&pool, "   ", AmbitoRips::Ac).await.unwrap().is_empty());
    }

    /// Los comodines de LIKE escritos por el usuario buscan el carácter
    /// LITERAL. Con `%` sin escapar, teclearlo devolvería el catálogo entero —
    /// que es la fuga de rendimiento y de datos que el escape existe para evitar.
    #[tokio::test]
    async fn los_comodines_no_se_interpretan() {
        let pool = con_cups().await;
        assert!(
            buscar_cups(&pool, "%", AmbitoRips::Ac).await.unwrap().is_empty(),
            "un `%` se interpretó como comodín y devolvió todo"
        );
        assert!(buscar_cups(&pool, "_", AmbitoRips::Ac).await.unwrap().is_empty());
    }

    /// El ámbito se lee del texto **y un valor raro no cae a un defecto**: caer
    /// a `AC` haría que `?ambito=procedimientos` buscara consultas y devolviera
    /// una lista que parece correcta y es de otro archivo (directiva 20).
    #[test]
    fn el_ambito_no_se_adivina() {
        assert_eq!(AmbitoRips::desde("AC"), Some(AmbitoRips::Ac));
        assert_eq!(AmbitoRips::desde("ac"), Some(AmbitoRips::Ac), "se tolera la caja");
        assert_eq!(AmbitoRips::desde(" ap "), Some(AmbitoRips::Ap));
        assert_eq!(AmbitoRips::desde("procedimientos"), None);
        assert_eq!(AmbitoRips::desde(""), None);
    }

    /// `cups_es_de_ambito` es la puerta de la ESCRITURA, y tiene que decir que
    /// no al código que el buscador no habría ofrecido.
    #[tokio::test]
    async fn la_escritura_rechaza_el_cups_de_otro_ambito() {
        let pool = con_cups().await;
        assert!(cups_es_de_ambito(&pool, "890201", AmbitoRips::Ac).await.unwrap());
        assert!(
            !cups_es_de_ambito(&pool, "471001", AmbitoRips::Ac).await.unwrap(),
            "un CUPS de procedimientos entró como consulta"
        );
        assert!(!cups_es_de_ambito(&pool, "NOEXISTE", AmbitoRips::Ac).await.unwrap());
    }
}
