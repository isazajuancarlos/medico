// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! DIVIPOLA: los municipios y departamentos de Colombia, para el RIPS.
//!
//! `paciente.cod_municipio` era TEXT sin validación: cualquier cosa entraba y
//! el envío se rompía después, lejos de donde se tecleó. Peor todavía era el
//! defecto que traía el sistema viejo —`"05001"`—, que pasaba la validación de
//! estructura y **reportaba a todos los pacientes del país como residentes de
//! Medellín**. Un dato ausente que se rellena con uno plausible no se nota
//! nunca; por eso aquí no se sustituye nada (directiva 20).
//!
//! # Por qué viene empaquetado y no se consulta por red
//!
//! `datos/divipola.json` se genera de las publicaciones oficiales y se versiona
//! con el código. Un consultorio en zona rural no puede depender de que haya
//! internet para saber si un código de municipio es válido, y menos de que
//! datos.gov.co esté arriba el día del cierre de mes. La lista cambia cada
//! varios años: regenerarla es un paso deliberado, no una llamada en caliente.
//!
//! # El cero a la izquierda no es cosmético
//!
//! `codMunicipioResidencia` va de CINCO dígitos. Antioquia es el departamento
//! 05 y Atlántico el 08: **148 municipios —el 13 % del país— tienen código que
//! empieza por cero**, Medellín entre ellos. El conjunto de MinSalud los publica
//! SIN ese cero (`5001` en vez de `05001`), porque en algún punto pasaron por
//! una hoja de cálculo que los leyó como números.
//!
//! # Las dos fuentes oficiales no coinciden, y no es un error
//!
//! ```text
//! 27086 Belén de Bajirá (MinSalud)  <->  27493 Nuevo Belén de Bajirá (DANE)
//! 94663 Mapiripana (MinSalud)       <->  94885 La Guadalupe (DANE)
//! ```
//!
//! La primera es el pleito de límites entre Chocó y Antioquia, que lleva
//! décadas; la segunda, una antigua área no municipalizada del Guainía. Se
//! aceptan **los cuatro** códigos: rechazar los de MinSalud rompería el envío al
//! MUV, que valida contra su propia lista, y rechazar los del DANE rompería el
//! cruce con cualquier estadística oficial. Un prestador no tiene por qué
//! arbitrar un diferendo territorial para poder facturar.

use serde::Deserialize;

/// La fuente, empotrada en el binario. Ver el porqué en la cabecera del módulo.
pub(crate) const DIVIPOLA_JSON: &str = include_str!("../datos/divipola.json");

/// Longitud del código de municipio en el RIPS (Resolución 2275 de 2023).
pub const LARGO_CODIGO: usize = 5;

#[derive(Debug, Deserialize)]
struct Archivo {
    /// `"05" -> "ANTIOQUIA"`.
    departamentos: std::collections::BTreeMap<String, String>,
    municipios: Vec<MunicipioCrudo>,
}

#[derive(Debug, Deserialize)]
struct MunicipioCrudo {
    codigo: String,
    nombre: String,
    departamento: String,
    tipo: String,
    en_minsalud: bool,
}

/// Un municipio, ya resuelto con el nombre de su departamento.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Municipio {
    pub codigo: String,
    pub nombre: String,
    pub departamento: String,
    pub nombre_departamento: String,
    pub tipo: String,
    /// Está en la lista de MinSalud, que es contra la que valida el MUV.
    pub en_minsalud: bool,
}

/// Llena las tablas desde el JSON empotrado. Idempotente: se llama en cada
/// arranque y no duplica ni pisa nada que no venga de la fuente.
///
/// Va en el arranque y no en la migración porque un `INSERT` de 1 124 filas
/// dentro de un `.sql` sería un archivo generado que nadie puede revisar en un
/// diff, y porque así regenerar el JSON basta para que el siguiente arranque lo
/// aplique.
pub async fn sembrar(pool: &crate::Pool) -> Result<usize, sqlx::Error> {
    let archivo: Archivo = serde_json::from_str(DIVIPOLA_JSON)
        .expect("datos/divipola.json va empotrado y se valida en las pruebas");

    let mut tx = pool.begin().await?;
    for (codigo, nombre) in &archivo.departamentos {
        sqlx::query(
            "INSERT INTO departamento (codigo, nombre) VALUES (?,?)
             ON CONFLICT (codigo) DO UPDATE SET nombre = excluded.nombre",
        )
        .bind(codigo)
        .bind(nombre)
        .execute(&mut *tx)
        .await?;
    }
    for m in &archivo.municipios {
        sqlx::query(
            "INSERT INTO municipio (codigo, nombre, departamento, tipo, en_minsalud)
             VALUES (?,?,?,?,?)
             ON CONFLICT (codigo) DO UPDATE SET
                 nombre = excluded.nombre,
                 departamento = excluded.departamento,
                 tipo = excluded.tipo,
                 en_minsalud = excluded.en_minsalud",
        )
        .bind(&m.codigo)
        .bind(&m.nombre)
        .bind(&m.departamento)
        .bind(&m.tipo)
        .bind(i64::from(m.en_minsalud))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(archivo.municipios.len())
}

/// ¿Existe este código? **No normaliza nada**: `"5001"` es `false`, no
/// `"05001"`. Aceptar el de cuatro dígitos «siendo amables» es justo lo que
/// dejaría entrar el dato que MinSalud publica mal, y entonces no habría manera
/// de saber cuáles se arreglaron y cuáles se adivinaron.
pub async fn existe(pool: &crate::Pool, codigo: &str) -> Result<bool, sqlx::Error> {
    if codigo.len() != LARGO_CODIGO {
        return Ok(false);
    }
    let fila: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM municipio WHERE codigo = ?")
        .bind(codigo)
        .fetch_optional(pool)
        .await?;
    Ok(fila.is_some())
}

/// Un municipio por su código.
pub async fn obtener(pool: &crate::Pool, codigo: &str) -> Result<Option<Municipio>, sqlx::Error> {
    let fila: Option<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT m.codigo, m.nombre, m.departamento, d.nombre, m.tipo, m.en_minsalud
         FROM municipio m JOIN departamento d ON d.codigo = m.departamento
         WHERE m.codigo = ?",
    )
    .bind(codigo)
    .fetch_optional(pool)
    .await?;
    Ok(fila.map(|(codigo, nombre, departamento, nombre_departamento, tipo, en_minsalud)| {
        Municipio { codigo, nombre, departamento, nombre_departamento, tipo, en_minsalud: en_minsalud != 0 }
    }))
}

/// Busca por nombre o por código, para el selector de la pantalla.
///
/// El límite es del llamante y no una constante de aquí: quien pinta un
/// desplegable quiere 20 y quien exporta quiere todos, y una cifra fija aquí
/// obligaría a uno de los dos a pelearse con ella.
pub async fn buscar(
    pool: &crate::Pool,
    texto: &str,
    limite: i64,
) -> Result<Vec<Municipio>, sqlx::Error> {
    let patron = format!("%{}%", texto.trim());
    let filas: Vec<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT m.codigo, m.nombre, m.departamento, d.nombre, m.tipo, m.en_minsalud
         FROM municipio m JOIN departamento d ON d.codigo = m.departamento
         WHERE m.nombre LIKE ?1 OR m.codigo LIKE ?1 OR d.nombre LIKE ?1
         ORDER BY m.nombre LIMIT ?2",
    )
    .bind(&patron)
    .bind(limite)
    .fetch_all(pool)
    .await?;
    Ok(filas
        .into_iter()
        .map(|(codigo, nombre, departamento, nombre_departamento, tipo, en_minsalud)| Municipio {
            codigo,
            nombre,
            departamento,
            nombre_departamento,
            tipo,
            en_minsalud: en_minsalud != 0,
        })
        .collect())
}

/// Todos los municipios del país (1 124). Lo usa la derivación del municipio
/// (#84): pasa el catálogo entero a `derivaciones::municipio_desde_texto`, que
/// filtra por subcadena PLEGANDO tildes —cosa que el `LIKE` de `buscar` no hace,
/// así que «Medellin» no encontraría «Medellín»—.
pub async fn todos(pool: &crate::Pool) -> Result<Vec<Municipio>, sqlx::Error> {
    let filas: Vec<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT m.codigo, m.nombre, m.departamento, d.nombre, m.tipo, m.en_minsalud
         FROM municipio m JOIN departamento d ON d.codigo = m.departamento
         ORDER BY m.nombre",
    )
    .fetch_all(pool)
    .await?;
    Ok(filas
        .into_iter()
        .map(|(codigo, nombre, departamento, nombre_departamento, tipo, en_minsalud)| Municipio {
            codigo,
            nombre,
            departamento,
            nombre_departamento,
            tipo,
            en_minsalud: en_minsalud != 0,
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
            "sqlite:file:divipola_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        let pool = crate::abrir(&url).await.unwrap();
        sembrar(&pool).await.unwrap();
        pool
    }

    /// El JSON empotrado se parsea. Si no, `sembrar` entra en pánico en el
    /// arranque de producción, así que esto tiene que fallar aquí primero.
    #[test]
    fn el_json_empotrado_es_valido() {
        let a: Archivo = serde_json::from_str(DIVIPOLA_JSON).expect("JSON válido");
        assert_eq!(a.departamentos.len(), 33);
        assert_eq!(a.municipios.len(), 1124);
    }

    #[tokio::test]
    async fn se_siembra_el_pais_entero() {
        let pool = base().await;
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM municipio")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n.0, 1124);
        let d: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM departamento")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(d.0, 33);
    }

    /// Sembrar dos veces no duplica ni cambia nada: se llama en CADA arranque.
    #[tokio::test]
    async fn sembrar_es_idempotente() {
        let pool = base().await;
        sembrar(&pool).await.unwrap();
        sembrar(&pool).await.unwrap();
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM municipio")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n.0, 1124);
    }

    // === EL CERO A LA IZQUIERDA ==========================================

    /// 148 municipios —el 13 % del país— empiezan por cero. Si el cargador los
    /// hubiera leído como números, este número sería 0 y todo lo demás pasaría
    /// igual.
    #[tokio::test]
    async fn los_codigos_con_cero_delante_sobreviven() {
        let pool = base().await;
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM municipio WHERE codigo LIKE '0%'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n.0, 148, "el 13 % del país empieza por cero");
        assert!(existe(&pool, "05001").await.unwrap(), "Medellín");
    }

    /// Y NO se acepta la forma de cuatro dígitos que publica MinSalud. Ser
    /// amable aquí es dejar entrar el dato mal y no poder distinguir después
    /// cuáles se arreglaron de cuáles se adivinaron.
    #[tokio::test]
    async fn el_codigo_de_cuatro_digitos_no_cuela() {
        let pool = base().await;
        assert!(!existe(&pool, "5001").await.unwrap());
        assert!(!existe(&pool, "").await.unwrap());
        assert!(!existe(&pool, "050010").await.unwrap());
    }

    // === EL DIFERENDO TERRITORIAL ========================================

    /// Los CUATRO códigos del pleito existen. Rechazar los de MinSalud rompe el
    /// envío al MUV; rechazar los del DANE rompe el cruce estadístico. Un
    /// prestador no arbitra un diferendo de límites para poder facturar.
    #[tokio::test]
    async fn los_cuatro_codigos_en_disputa_se_aceptan() {
        let pool = base().await;
        for codigo in ["27086", "27493", "94663", "94885"] {
            assert!(
                existe(&pool, codigo).await.unwrap(),
                "{codigo} debe existir: es uno de los cuatro en disputa"
            );
        }
        // Y se distingue cuál es de cuál lista, sin excluir a ninguno.
        assert!(obtener(&pool, "27086").await.unwrap().unwrap().en_minsalud, "MinSalud");
        assert!(!obtener(&pool, "27493").await.unwrap().unwrap().en_minsalud, "solo DANE");
    }

    #[tokio::test]
    async fn un_municipio_trae_el_nombre_de_su_departamento() {
        let pool = base().await;
        let m = obtener(&pool, "05001").await.unwrap().unwrap();
        assert_eq!(m.nombre, "MEDELLÍN");
        assert_eq!(m.departamento, "05");
        assert_eq!(m.nombre_departamento, "ANTIOQUIA");
    }

    #[tokio::test]
    async fn un_codigo_inventado_no_devuelve_nada() {
        let pool = base().await;
        assert!(obtener(&pool, "99999").await.unwrap().is_none());
        assert!(!existe(&pool, "99999").await.unwrap());
    }

    // === BÚSQUEDA ========================================================

    #[tokio::test]
    async fn se_busca_por_nombre_por_codigo_y_por_departamento() {
        let pool = base().await;
        let por_nombre = buscar(&pool, "MEDELL", 20).await.unwrap();
        assert!(por_nombre.iter().any(|m| m.codigo == "05001"));

        let por_codigo = buscar(&pool, "05001", 20).await.unwrap();
        assert_eq!(por_codigo.len(), 1);
        assert_eq!(por_codigo[0].nombre, "MEDELLÍN");

        // Antioquia tiene 125 municipios; con límite 20 se devuelven 20.
        let por_departamento = buscar(&pool, "ANTIOQUIA", 20).await.unwrap();
        assert_eq!(por_departamento.len(), 20, "el límite lo pone el llamante");
        assert!(por_departamento.iter().all(|m| m.departamento == "05"));
    }

    /// Una búsqueda vacía no devuelve el país entero de golpe ni revienta:
    /// devuelve lo que quepa en el límite.
    #[tokio::test]
    async fn la_busqueda_vacia_respeta_el_limite() {
        let pool = base().await;
        assert_eq!(buscar(&pool, "", 5).await.unwrap().len(), 5);
        assert_eq!(buscar(&pool, "   ", 5).await.unwrap().len(), 5);
    }
}
