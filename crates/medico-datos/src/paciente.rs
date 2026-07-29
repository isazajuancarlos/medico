//! Pacientes. Toda consulta lleva el ámbito, y el ámbito no se puede omitir.

use medico_dominio::{Ambito, Visibilidad};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Paciente {
    pub id: i64,
    pub tipo_documento: String,
    pub numero_documento: String,
    pub nombre1: String,
    pub nombre2: Option<String>,
    pub apellido1: String,
    pub apellido2: Option<String>,
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub cod_municipio: Option<String>,
    pub institucion_id: i64,
}

const COLUMNAS: &str = "id, tipo_documento, numero_documento, nombre1, nombre2, \
                        apellido1, apellido2, fecha_nacimiento, sexo, cod_municipio, \
                        institucion_id";

/// El `Ambito` es un parámetro OBLIGATORIO, no opcional.
///
/// En Python el equivalente era `institucion_id: Optional[int] = None`, que al
/// omitirse dejaba de filtrar. Eso ya causó una fuga real: `rips_completo.py`
/// filtraba solo si se lo pedían y `routers/rips.py` no se lo pedía, y nadie lo
/// notó. Aquí omitirlo no compila — que es la única versión de esa regla que no
/// depende de que alguien se acuerde.
pub async fn listar(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Paciente>, sqlx::Error> {
    match ambito {
        Ambito::Superadmin => {
            sqlx::query_as::<_, Paciente>(&format!("SELECT {COLUMNAS} FROM paciente ORDER BY id"))
                .fetch_all(pool)
                .await
        }
        Ambito::Institucion(id) => {
            sqlx::query_as::<_, Paciente>(&format!(
                "SELECT {COLUMNAS} FROM paciente WHERE institucion_id = ? ORDER BY id"
            ))
            .bind(id)
            .fetch_all(pool)
            .await
        }
        // No es una consulta que devuelve vacío: es no preguntar. Un usuario sin
        // institución y sin ser superadmin no tiene ningún conjunto legítimo.
        Ambito::Ninguno => Ok(Vec::new()),
    }
}

/// `None` significa «no existe **o** no es suyo». No se distingue a propósito:
/// ver `medico_dominio::Visibilidad`.
pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Paciente>, sqlx::Error> {
    let fila = sqlx::query_as::<_, Paciente>(&format!(
        "SELECT {COLUMNAS} FROM paciente WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(fila.filter(|p| ambito.puede_ver(Some(p.institucion_id)) == Visibilidad::Si))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Base en memoria con dos instituciones y un paciente en cada una.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900'),(2,'Dos','901')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, cod_municipio, institucion_id)
             VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F','05001',1),
                    (20,'CC','222','Beto','Paz','1985-05-05','M','08001',2)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn listar_solo_devuelve_lo_de_la_institucion() {
        let pool = base().await;
        let ps = listar(&pool, Ambito::institucion(1)).await.unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].id, 10);
    }

    /// LA PRUEBA QUE JUSTIFICA ESTE DISEÑO.
    ///
    /// Pedir por identificador el paciente de otra institución tiene que ser
    /// indistinguible de pedir uno que no existe. En Python esto se escapó una
    /// vez —`db.get` no pasaba por el filtro— y bastaba incrementar un número
    /// en la URL para leer, y con `PUT` modificar, la historia de otra
    /// institución.
    #[tokio::test]
    async fn obtener_el_de_otra_institucion_es_igual_que_uno_inexistente() {
        let pool = base().await;
        let ajeno = obtener(&pool, Ambito::institucion(1), 20).await.unwrap();
        let inexistente = obtener(&pool, Ambito::institucion(1), 9999).await.unwrap();
        assert!(ajeno.is_none(), "vio un paciente de otra institución");
        assert!(inexistente.is_none());
    }

    #[tokio::test]
    async fn obtener_el_propio_funciona() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.nombre1, "Ana");
    }

    #[tokio::test]
    async fn el_superadmin_ve_las_dos_instituciones() {
        let pool = base().await;
        assert_eq!(listar(&pool, Ambito::Superadmin).await.unwrap().len(), 2);
        assert!(obtener(&pool, Ambito::Superadmin, 20).await.unwrap().is_some());
    }

    /// Un usuario sin institución no ve nada, y en particular NO lo ve todo.
    #[tokio::test]
    async fn sin_ambito_no_se_ve_nada() {
        let pool = base().await;
        assert!(listar(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::Ninguno, 10).await.unwrap().is_none());
    }

    /// El cero a la izquierda del código de municipio sobrevive al viaje.
    #[tokio::test]
    async fn el_codigo_de_municipio_conserva_el_cero() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.cod_municipio.as_deref(), Some("05001"));
    }

    /// El mismo documento en dos instituciones son dos filas legítimas.
    #[tokio::test]
    async fn el_mismo_documento_puede_estar_en_dos_instituciones() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, institucion_id)
             VALUES (30,'CC','111','Ana','Ruiz','1990-01-01','F',2)",
        )
        .execute(&pool)
        .await
        .expect("el mismo documento en otra institución debe poder guardarse");

        // …y repetirlo DENTRO de la misma sí es un error.
        let repetido = sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, institucion_id)
             VALUES (31,'CC','111','Ana','Ruiz','1990-01-01','F',1)",
        )
        .execute(&pool)
        .await;
        assert!(repetido.is_err(), "se aceptó un documento duplicado en la misma institución");
    }

    /// Las claves foráneas están ACTIVAS. En SQLite vienen apagadas por defecto,
    /// y con ellas apagadas un paciente puede apuntar a una institución que no
    /// existe: la fila queda fuera del alcance de todo filtro por institución y
    /// no la ve nadie, ni siquiera para corregirla.
    #[tokio::test]
    async fn las_claves_foraneas_estan_activas() {
        let pool = base().await;
        let huerfano = sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, institucion_id)
             VALUES (40,'CC','999','Nadie','Nadie','1990-01-01','F',777)",
        )
        .execute(&pool)
        .await;
        assert!(huerfano.is_err(), "se aceptó un paciente de una institución inexistente");
    }
}
