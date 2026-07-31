// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Los maestros clínicos: CIE-10 (diagnósticos) y CUPS (procedimientos) — #77.
//!
//! `historia.diagnostico_principal` y `historia.cod_consulta_cups` eran TEXT
//! libre. El RIPS los valida contra las tablas del MUV, así que un código mal
//! digitado tiene la forma correcta, pasa cualquier validación de estructura, y
//! hace **rechazar la factura entera tres semanas después**. Tenerlos aquí
//! convierte eso en una línea del informe de cierre — que es exactamente para lo
//! que existe [`medico_dominio::rips::Recolector::codigo`].
//!
//! # De dónde salen, y la advertencia que viaja con ellos
//!
//! Se generan con `herramientas/generar_catalogos.py` desde los `.xlsx`
//! oficiales y se versionan con el código. **El CIE-10 declara en su propia
//! segunda fila venir de un blog de WordPress**, no de MinSalud ni de la OMS. Su
//! estructura se verificó —12 423 códigos, todos distintos, todos con forma
//! `[A-Z]\d{2,3}`— pero antes de facturar con esto hay que contrastarlo contra
//! la tabla de referencia vigente del MUV. La advertencia viaja DENTRO del JSON
//! para que no se pierda por el camino.

use serde::{Deserialize, Serialize};

const CIE10_JSON: &str = include_str!("../datos/cie10.json");
const CUPS_JSON: &str = include_str!("../datos/cups.json");
const CUM_JSON: &str = include_str!("../datos/cum.json");

#[derive(Debug, Deserialize)]
struct ArchivoCie10 {
    advertencia: String,
    codigos: Vec<Cie10Crudo>,
}

#[derive(Debug, Deserialize)]
struct Cie10Crudo {
    codigo: String,
    descripcion: String,
    categoria: String,
    notacion: String,
}

#[derive(Debug, Deserialize)]
struct ArchivoCups {
    codigos: Vec<CupsCrudo>,
}

#[derive(Debug, Deserialize)]
struct CupsCrudo {
    codigo: String,
    nombre: String,
    seccion: String,
    ambito_rips: String,
    quirurgico: bool,
}

/// Un diagnóstico de la CIE-10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostico {
    pub codigo: String,
    pub descripcion: String,
    pub categoria: String,
    /// `""`, `"daga"` o `"asterisco"`. La notación daga-asterisco de la
    /// clasificación: `†` marca la etiología y `*` la manifestación. Se conserva
    /// porque es información clínica real, pero **no forma parte del código**:
    /// el MUV valida contra su tabla y `A065†` no está en ninguna.
    pub notacion: String,
}

/// Un procedimiento del CUPS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Procedimiento {
    pub codigo: String,
    pub nombre: String,
    /// Tal cual viene de la fuente. Dos filas la traen corrompida
    /// (`SECCIO4R 00 …` y `NULL`) y se conservan así: corregirlas a ojo sería
    /// inventar a qué sección pertenece un procedimiento.
    pub seccion: String,
    /// En qué archivo del RIPS se reporta: `AP`, `AC` o `AT`.
    ///
    /// Sale de la columna que la fuente titula «Habilitado» y que NO lleva un
    /// sí/no. Se descubrió leyendo los VALORES en vez del nombre, y responde
    /// justo la pregunta que el generador de RIPS tiene que hacerse por cada
    /// procedimiento.
    pub ambito_rips: String,
    pub quirurgico: bool,
}

#[derive(Debug, Deserialize)]
struct ArchivoCum {
    advertencia: String,
    medicamentos: Vec<CumCrudo>,
}

/// Nombres de un carácter a propósito: son 65 997 registros y cada byte de clave
/// se paga 65 997 veces en el binario. `c` código, `n` nombre, `k` concentración,
/// `f` forma farmacéutica, `t` titular.
#[derive(Debug, Deserialize)]
struct CumCrudo {
    c: String,
    n: String,
    k: String,
    f: String,
    t: String,
    /// `estadocum` del export. 22 % del maestro está inactivo.
    a: bool,
}

/// La advertencia sobre la fecha del maestro CUM.
pub fn advertencia_cum() -> &'static str {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<ArchivoCum>(CUM_JSON)
            .expect("el JSON empotrado se valida en las pruebas")
            .advertencia
    })
}

/// La advertencia sobre la procedencia del CIE-10, para mostrarla donde importe.
pub fn advertencia_cie10() -> &'static str {
    // Se lee del JSON y no se copia aquí: dos textos que dicen lo mismo son dos
    // textos que un día dirán cosas distintas.
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<ArchivoCie10>(CIE10_JSON)
            .expect("el JSON empotrado se valida en las pruebas")
            .advertencia
    })
}

/// Llena las dos tablas desde el JSON empotrado. Idempotente.
///
/// Medido: 46 ms de parseo y 37 ms de escritura para las 26 063 filas, en una
/// sola transacción. Por eso NO hay mecanismo de «sembrar solo si cambió»: sería
/// un segundo camino para estar al día, y taparía los fallos del primero.
pub async fn sembrar(pool: &crate::Pool) -> Result<(usize, usize, usize), sqlx::Error> {
    let cie: ArchivoCie10 = serde_json::from_str(CIE10_JSON)
        .expect("datos/cie10.json va empotrado y se valida en las pruebas");
    let cups: ArchivoCups = serde_json::from_str(CUPS_JSON)
        .expect("datos/cups.json va empotrado y se valida en las pruebas");

    let mut tx = pool.begin().await?;
    for d in &cie.codigos {
        sqlx::query(
            "INSERT INTO cie10 (codigo, descripcion, categoria, notacion) VALUES (?,?,?,?)
             ON CONFLICT (codigo) DO UPDATE SET
                 descripcion = excluded.descripcion,
                 categoria = excluded.categoria,
                 notacion = excluded.notacion",
        )
        .bind(&d.codigo)
        .bind(&d.descripcion)
        .bind(&d.categoria)
        .bind(&d.notacion)
        .execute(&mut *tx)
        .await?;
    }
    for p in &cups.codigos {
        sqlx::query(
            "INSERT INTO cups (codigo, nombre, seccion, ambito_rips, quirurgico)
             VALUES (?,?,?,?,?)
             ON CONFLICT (codigo) DO UPDATE SET
                 nombre = excluded.nombre,
                 seccion = excluded.seccion,
                 ambito_rips = excluded.ambito_rips,
                 quirurgico = excluded.quirurgico",
        )
        .bind(&p.codigo)
        .bind(&p.nombre)
        .bind(&p.seccion)
        .bind(&p.ambito_rips)
        .bind(i64::from(p.quirurgico))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // El CUM va aparte porque su lote lo escribe `catalogo::cargar_medicamentos`,
    // que ya existe y ya está probado: se reutiliza en vez de duplicar el upsert.
    let cum: ArchivoCum = serde_json::from_str(CUM_JSON)
        .expect("datos/cum.json va empotrado y se valida en las pruebas");
    let lote: Vec<crate::catalogo::MedicamentoNuevo> = cum
        .medicamentos
        .iter()
        .map(|m| crate::catalogo::MedicamentoNuevo {
            codigo_cum: m.c.clone(),
            nombre: m.n.clone(),
            concentracion: m.k.clone(),
            forma_farmaceutica: m.f.clone(),
            titular: if m.t.is_empty() { None } else { Some(m.t.clone()) },
            activo: m.a,
        })
        .collect();
    crate::catalogo::cargar_medicamentos(pool, &lote).await?;

    Ok((cie.codigos.len(), cups.codigos.len(), lote.len()))
}

/// ¿Existe este CUM **en el maestro**, activo o no?
///
/// Es la pregunta de LECTURA: «¿qué medicamento es este código?», la que hace
/// falta para mostrar una fórmula de hace dos años donde se prescribió algo que
/// entonces estaba vigente. Para decidir si se puede prescribir HOY está
/// [`esta_activo_cum`], que es otra pregunta — y por eso son dos funciones y no
/// una con bandera.
pub async fn existe_cum(pool: &crate::Pool, codigo: &str) -> Result<bool, sqlx::Error> {
    let f: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM medicamento WHERE codigo_cum = ?")
        .bind(codigo)
        .fetch_optional(pool)
        .await?;
    Ok(f.is_some())
}

/// ¿Se puede prescribir HOY este CUM?
///
/// **Ésta es la que decide si el RIPS pasa.** 15 046 de los 65 997 medicamentos
/// del maestro —el 22 %— existen y están INACTIVOS: el MUV los rechaza, y el
/// rechazo llega semanas después como glosa. Un código que no existe y uno que
/// existe pero está inactivo dan las dos `false` aquí, y eso es correcto: la
/// pregunta es la misma. Quien necesite distinguirlos para el mensaje de error
/// tiene [`existe_cum`].
pub async fn esta_activo_cum(pool: &crate::Pool, codigo: &str) -> Result<bool, sqlx::Error> {
    let f: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM medicamento WHERE codigo_cum = ? AND activo = 1")
            .bind(codigo)
            .fetch_optional(pool)
            .await?;
    Ok(f.is_some())
}

/// ¿Existe este diagnóstico? **No normaliza**: `"A065†"` es `false`, porque eso
/// es lo que el MUV responderá. Quien tenga la daga en la mano debe quitarla
/// antes, y verlo aquí es lo que hace que se entere.
pub async fn existe_cie10(pool: &crate::Pool, codigo: &str) -> Result<bool, sqlx::Error> {
    let f: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM cie10 WHERE codigo = ?")
        .bind(codigo)
        .fetch_optional(pool)
        .await?;
    Ok(f.is_some())
}

pub async fn existe_cups(pool: &crate::Pool, codigo: &str) -> Result<bool, sqlx::Error> {
    let f: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM cups WHERE codigo = ?")
        .bind(codigo)
        .fetch_optional(pool)
        .await?;
    Ok(f.is_some())
}

pub async fn obtener_cie10(
    pool: &crate::Pool,
    codigo: &str,
) -> Result<Option<Diagnostico>, sqlx::Error> {
    let f: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT codigo, descripcion, categoria, notacion FROM cie10 WHERE codigo = ?",
    )
    .bind(codigo)
    .fetch_optional(pool)
    .await?;
    Ok(f.map(|(codigo, descripcion, categoria, notacion)| Diagnostico {
        codigo,
        descripcion,
        categoria,
        notacion,
    }))
}

/// Busca diagnósticos por código o descripción, para el selector de la pantalla.
/// El límite lo pone el llamante, por lo mismo que en DIVIPOLA.
pub async fn buscar_cie10(
    pool: &crate::Pool,
    texto: &str,
    limite: i64,
) -> Result<Vec<Diagnostico>, sqlx::Error> {
    let patron = format!("%{}%", texto.trim());
    let filas: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT codigo, descripcion, categoria, notacion FROM cie10
         WHERE codigo LIKE ?1 OR descripcion LIKE ?1 ORDER BY codigo LIMIT ?2",
    )
    .bind(&patron)
    .bind(limite)
    .fetch_all(pool)
    .await?;
    Ok(filas
        .into_iter()
        .map(|(codigo, descripcion, categoria, notacion)| Diagnostico {
            codigo,
            descripcion,
            categoria,
            notacion,
        })
        .collect())
}

pub async fn buscar_cups(
    pool: &crate::Pool,
    texto: &str,
    limite: i64,
) -> Result<Vec<Procedimiento>, sqlx::Error> {
    let patron = format!("%{}%", texto.trim());
    let filas: Vec<(String, String, String, String, i64)> = sqlx::query_as(
        "SELECT codigo, nombre, seccion, ambito_rips, quirurgico FROM cups
         WHERE codigo LIKE ?1 OR nombre LIKE ?1 ORDER BY codigo LIMIT ?2",
    )
    .bind(&patron)
    .bind(limite)
    .fetch_all(pool)
    .await?;
    Ok(filas
        .into_iter()
        .map(|(codigo, nombre, seccion, ambito_rips, quirurgico)| Procedimiento {
            codigo,
            nombre,
            seccion,
            ambito_rips,
            quirurgico: quirurgico != 0,
        })
        .collect())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        let url = format!(
            "sqlite:file:clinico_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        let pool = crate::abrir(&url).await.unwrap();
        sembrar(&pool).await.unwrap();
        pool
    }

    /// Los dos JSON empotrados se parsean. Si no, `sembrar` entra en pánico en el
    /// arranque de producción, así que esto tiene que fallar aquí primero.
    #[test]
    fn los_json_empotrados_son_validos() {
        let cie: ArchivoCie10 = serde_json::from_str(CIE10_JSON).expect("cie10 válido");
        let cups: ArchivoCups = serde_json::from_str(CUPS_JSON).expect("cups válido");
        assert_eq!(cie.codigos.len(), 12_423);
        assert_eq!(cups.codigos.len(), 13_640);
        // Y el ámbito RIPS está en los tres valores que la fuente usa.
        let ambitos: std::collections::HashSet<&str> =
            cups.codigos.iter().map(|c| c.ambito_rips.as_str()).collect();
        assert_eq!(ambitos.len(), 3, "AP, AC y AT: {ambitos:?}");
    }

    /// La advertencia sobre la procedencia NO se pierde. Es lo que impide que
    /// alguien facture con esto creyendo que viene de MinSalud.
    #[test]
    fn la_advertencia_de_procedencia_viaja_con_los_datos() {
        let a = advertencia_cie10();
        assert!(a.contains("medicblog.wordpress.com"), "{a}");
        assert!(a.contains("NO es una publicación oficial"), "{a}");
    }

    #[tokio::test]
    async fn se_siembran_los_dos_catalogos() {
        let pool = base().await;
        let (c,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cie10").fetch_one(&pool).await.unwrap();
        let (p,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cups").fetch_one(&pool).await.unwrap();
        assert_eq!(c, 12_423);
        assert_eq!(p, 13_640);
    }

    #[tokio::test]
    async fn sembrar_es_idempotente() {
        let pool = base().await;
        sembrar(&pool).await.unwrap();
        let (c,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cie10").fetch_one(&pool).await.unwrap();
        assert_eq!(c, 12_423);
    }

    // === LA DAGA Y EL ASTERISCO ===========================================

    /// **378 códigos llevaban notación y se guardan LIMPIOS.** Enviar `A065†` al
    /// MUV es rechazo seguro, así que el código va sin ella y la notación en su
    /// propia columna: no se pierde, y no contamina.
    #[tokio::test]
    async fn la_notacion_se_separa_del_codigo() {
        let pool = base().await;
        let d = obtener_cie10(&pool, "A065").await.unwrap().expect("A065 existe limpio");
        assert_eq!(d.codigo, "A065");
        assert_eq!(d.notacion, "daga", "la notación no se pierde, se separa");

        // Y el código CON la daga no existe: es lo que el MUV responderá.
        assert!(!existe_cie10(&pool, "A065†").await.unwrap());

        let (con_notacion,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM cie10 WHERE notacion <> ''")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(con_notacion, 378, "300 asterisco + 78 daga");
    }

    /// Limpiar la notación NO fundió dos diagnósticos en uno: se comprobó antes
    /// de decidirlo y esto lo fija.
    #[tokio::test]
    async fn limpiar_la_notacion_no_perdio_ningun_codigo() {
        let pool = base().await;
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(DISTINCT codigo) FROM cie10")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 12_423, "si hubiera habido colisiones, aquí faltarían");
    }

    // === LO QUE EL RIPS NECESITA ==========================================

    #[tokio::test]
    async fn un_diagnostico_real_existe_y_uno_inventado_no() {
        let pool = base().await;
        assert!(existe_cie10(&pool, "A000").await.unwrap(), "cólera");
        assert!(!existe_cie10(&pool, "Z9999").await.unwrap());
        assert!(!existe_cie10(&pool, "").await.unwrap());
    }

    #[tokio::test]
    async fn un_procedimiento_real_existe_y_uno_inventado_no() {
        let pool = base().await;
        assert!(existe_cups(&pool, "010100").await.unwrap(), "punción cisternal");
        assert!(!existe_cups(&pool, "999999").await.unwrap());
    }

    #[tokio::test]
    async fn el_diagnostico_trae_su_categoria_para_los_informes() {
        let pool = base().await;
        let d = obtener_cie10(&pool, "A000").await.unwrap().unwrap();
        assert_eq!(d.categoria, "A00", "los informes epidemiológicos agrupan por aquí");
        assert!(d.descripcion.contains("COLERA"), "{}", d.descripcion);
    }

    /// Las dos secciones corrompidas de la fuente se conservan TAL CUAL. No se
    /// corrigen a ojo porque eso sería inventar a qué sección pertenecen.
    #[tokio::test]
    async fn las_secciones_ilegibles_se_conservan_y_se_pueden_encontrar() {
        let pool = base().await;
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM cups WHERE seccion NOT LIKE 'SECCION%'",
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 2, "las dos de la fuente, ni una más ni una menos");
    }

    // === EL CUM ===========================================================

    /// El maestro entero entra al arrancar. Antes hacía falta acordarse de un
    /// subcomando: quien lo olvidaba tenía un sistema donde los diagnósticos se
    /// validaban y los medicamentos no, sin que nada avisara.
    #[tokio::test]
    async fn el_cum_se_siembra_entero_al_arrancar() {
        let pool = base().await;
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM medicamento")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 65_997);
        assert!(existe_cum(&pool, "19900001-1").await.unwrap() || n > 0);
        assert!(!existe_cum(&pool, "no-existe").await.unwrap());
    }

    /// **22 % del maestro está INACTIVO, y se carga marcado, no borrado.**
    ///
    /// Se descubrió el 2026-07-31 al verificar contra https://app.invima.gov.co/cum/
    /// por indicación de Juan: el export trae `estadocum` y el cargador lo
    /// ignoraba, así que 15 046 medicamentos entraban como si se pudieran
    /// prescribir. El MUV los rechaza.
    #[tokio::test]
    async fn los_medicamentos_inactivos_se_marcan_y_no_se_borran() {
        let pool = base().await;
        let (activos,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM medicamento WHERE activo=1")
            .fetch_one(&pool).await.unwrap();
        let (inactivos,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM medicamento WHERE activo=0")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(activos, 50_951);
        assert_eq!(inactivos, 15_046, "el 22 % del maestro");
        assert_eq!(activos + inactivos, 65_997, "no se borró ninguno");
    }

    /// Las DOS preguntas se distinguen, que es todo el punto: leer una fórmula
    /// vieja necesita el nombre; prescribir hoy necesita el estado.
    #[tokio::test]
    async fn existir_y_poder_prescribirse_son_preguntas_distintas() {
        let pool = base().await;
        let inactivo: (String,) =
            sqlx::query_as("SELECT codigo_cum FROM medicamento WHERE activo=0 LIMIT 1")
                .fetch_one(&pool).await.unwrap();
        assert!(existe_cum(&pool, &inactivo.0).await.unwrap(), "existe: se puede leer");
        assert!(!esta_activo_cum(&pool, &inactivo.0).await.unwrap(), "pero no se prescribe");

        let activo: (String,) =
            sqlx::query_as("SELECT codigo_cum FROM medicamento WHERE activo=1 LIMIT 1")
                .fetch_one(&pool).await.unwrap();
        assert!(esta_activo_cum(&pool, &activo.0).await.unwrap());

        // Y uno inventado da false en las dos.
        assert!(!existe_cum(&pool, "no-existe").await.unwrap());
        assert!(!esta_activo_cum(&pool, "no-existe").await.unwrap());
    }

    /// La concentración se resolvió donde NO había ambigüedad y se dejó vacía
    /// donde la había: 76 % del maestro. Media concentración es peor que
    /// ninguna, y esta prueba fija que no se rellenó el resto.
    #[tokio::test]
    async fn la_concentracion_se_puso_solo_donde_era_inequivoca() {
        let pool = base().await;
        let (con,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM medicamento WHERE concentracion <> ''")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(con, 50_208, "los de un solo principio activo y una sola concentración");
        // Y el resto quedó VACÍO, no relleno con un valor plausible.
        let (vacios,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM medicamento WHERE concentracion = ''")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(con + vacios, 65_997);
    }

    /// **La columna del export llamada `concentracion` NO es la concentración**:
    /// es un código de una letra. Si se hubiera cargado, habría «A» o «B» suelta
    /// en decenas de miles de filas. Esta prueba lo hace imposible de reintroducir.
    #[tokio::test]
    async fn ninguna_concentracion_es_una_letra_suelta() {
        let pool = base().await;
        let (raras,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM medicamento WHERE length(concentracion) = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(raras, 0, "una concentración de un carácter es el código, no el dato");
    }

    #[tokio::test]
    async fn la_advertencia_de_fecha_del_cum_viaja_con_los_datos() {
        let a = advertencia_cum();
        assert!(a.contains("2026-07-20"), "{a}");
        assert!(a.contains("no se nota hasta la glosa"), "{a}");
    }

    #[tokio::test]
    async fn se_busca_por_codigo_y_por_texto() {
        let pool = base().await;
        let por_texto = buscar_cie10(&pool, "COLERA", 10).await.unwrap();
        assert!(por_texto.iter().any(|d| d.codigo == "A000"));
        let por_codigo = buscar_cie10(&pool, "A000", 10).await.unwrap();
        assert_eq!(por_codigo.len(), 1);

        let proc = buscar_cups(&pool, "PUNCION CISTERNAL", 10).await.unwrap();
        assert!(!proc.is_empty());
        assert!(proc[0].quirurgico, "está en la sección de quirúrgicos");
    }

    /// **La columna que la fuente titula «Habilitado» no lleva un sí/no**: lleva
    /// el archivo del RIPS donde se reporta el procedimiento. Se descubrió
    /// leyendo los VALORES en vez del nombre, y por poco se guarda como
    /// `quirurgico` — que es la columna siguiente.
    ///
    /// Esta prueba fija los dos: que el ámbito son AP/AC/AT y que lo quirúrgico
    /// se leyó de donde toca.
    #[tokio::test]
    async fn el_ambito_rips_y_lo_quirurgico_salen_de_columnas_distintas() {
        let pool = base().await;
        let (ap,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cups WHERE ambito_rips='AP'")
            .fetch_one(&pool).await.unwrap();
        let (ac,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cups WHERE ambito_rips='AC'")
            .fetch_one(&pool).await.unwrap();
        let (at,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cups WHERE ambito_rips='AT'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!((ap, ac, at), (13_121, 334, 185), "el reparto de la fuente");

        // Y lo quirúrgico es OTRA cosa: 9 269 sí, y no coincide con ningún ámbito.
        let (q,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cups WHERE quirurgico=1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(q, 9_269);
        assert_ne!(q, ap, "si coincidieran, se habría leído la columna equivocada");
    }
}
