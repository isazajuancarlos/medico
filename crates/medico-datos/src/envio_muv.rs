//! El registro de lo enviado al Mecanismo Único de Validación: el CUV y los
//! hallazgos de cada envío, para que el comprobante no viva solo en una pantalla.
//!
//! Aquí están las filas y las dos consultas que la pantalla necesita; el
//! transporte está en `medico-muv` y la política —cuándo se envía— en la vista.
//!
//! # La regla del número de nota, que es lo único que este módulo DECIDE
//!
//! El consecutivo no se puede repetir entre envíos: repetirlo produce rechazo.
//! Pero **la puerta se cierra solo sobre los ACEPTADOS**, y esa elección es la
//! regla, no un detalle. Un número que ya obtuvo un CUV no se puede reutilizar
//! jamás — eso es inequívoco. Un número que se usó en un envío RECHAZADO es otra
//! cosa: el archivo no se radicó, y lo normal es corregir y reenviar; cerrar
//! también ese caso sería rechazar lo inusual en vez de lo imposible
//! (directiva 21), y obligaría a quemar un consecutivo por cada errata.

use chrono::Utc;
use medico_dominio::aislamiento::Ambito;

use crate::Pool;

/// Un envío que LLEGÓ a MinSalud, aceptado o rechazado.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Envio {
    pub id: i64,
    pub institucion_id: i64,
    pub usuario_id: i64,
    pub periodo_inicio: String,
    pub periodo_fin: String,
    pub num_nota: String,
    pub destino: String,
    pub aceptado: bool,
    pub cuv: Option<String>,
    pub proceso_id: Option<i64>,
    pub fecha_radicacion: Option<String>,
    pub ambiente: Option<String>,
    pub hallazgos: String,
    pub registros: i64,
    pub enviado: String,
}

const COLUMNAS: &str = "id, institucion_id, usuario_id, periodo_inicio, periodo_fin, num_nota, \
                        destino, aceptado, cuv, proceso_id, fecha_radicacion, ambiente, \
                        hallazgos, registros, enviado";

/// Lo que hay que anotar de un envío. Se separa de [`Envio`] porque el id y la
/// marca de tiempo los pone esta capa: quien registra no los conoce.
pub struct Nuevo<'a> {
    pub institucion_id: i64,
    pub usuario_id: i64,
    pub periodo_inicio: &'a str,
    pub periodo_fin: &'a str,
    pub num_nota: &'a str,
    pub destino: &'a str,
    pub aceptado: bool,
    pub cuv: Option<&'a str>,
    pub proceso_id: Option<i64>,
    pub fecha_radicacion: Option<&'a str>,
    pub ambiente: Option<&'a str>,
    pub hallazgos: &'a str,
    pub registros: i64,
}

/// Anota un envío. **El CUV se descarta si el envío no fue aceptado**, para que
/// el invariante de la tabla no dependa de que el llamante se acuerde: un CUV en
/// una fila rechazada sería el comprobante de algo que no pasó. El `CHECK` de la
/// migración lo rechazaría igualmente, pero eso sería un error de base a mitad de
/// un envío ya hecho — y el envío no se puede deshacer para reintentarlo.
pub async fn registrar(pool: &Pool, n: Nuevo<'_>) -> Result<i64, sqlx::Error> {
    let cuv = if n.aceptado { n.cuv } else { None };
    let r = sqlx::query(
        "INSERT INTO envio_muv (institucion_id, usuario_id, periodo_inicio, periodo_fin, \
         num_nota, destino, aceptado, cuv, proceso_id, fecha_radicacion, ambiente, hallazgos, \
         registros, enviado) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(n.institucion_id)
    .bind(n.usuario_id)
    .bind(n.periodo_inicio)
    .bind(n.periodo_fin)
    .bind(n.num_nota)
    .bind(n.destino)
    .bind(n.aceptado)
    .bind(cuv)
    .bind(n.proceso_id)
    .bind(n.fecha_radicacion)
    .bind(n.ambiente)
    .bind(n.hallazgos)
    .bind(n.registros)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(r.last_insert_rowid())
}

/// Los envíos de un periodo, el más reciente primero.
///
/// Va por `Ambito` como todo lo demás: un envío dice qué periodo reportó una
/// institución y con qué resultado, y eso no lo ve el vecino.
pub async fn del_periodo(
    pool: &Pool,
    ambito: Ambito,
    inicio: &str,
    fin: &str,
) -> Result<Vec<Envio>, sqlx::Error> {
    let institucion_id = match ambito {
        Ambito::Institucion(id) => id,
        // El superadmin no tiene periodo propio que consultar, y devolver los de
        // TODAS las instituciones sería la fuga que este repositorio ya cerró en
        // `/financiero`. `Ninguno` tampoco ve nada.
        Ambito::Superadmin | Ambito::Ninguno => return Ok(Vec::new()),
    };
    sqlx::query_as::<_, Envio>(&format!(
        "SELECT {COLUMNAS} FROM envio_muv \
         WHERE institucion_id = ? AND periodo_inicio = ? AND periodo_fin = ? \
         ORDER BY enviado DESC, id DESC"
    ))
    .bind(institucion_id)
    .bind(inicio)
    .bind(fin)
    .fetch_all(pool)
    .await
}

/// ¿Este número de nota ya obtuvo un CUV en esta institución?
///
/// Devuelve la fecha del envío que lo consumió, para poder decirlo en pantalla:
/// «ya se usó el 2026-08-10» es accionable; «ya se usó» manda a buscar dónde.
pub async fn nota_ya_aceptada(
    pool: &Pool,
    institucion_id: i64,
    num_nota: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT enviado FROM envio_muv \
         WHERE institucion_id = ? AND num_nota = ? AND aceptado = 1 \
         ORDER BY enviado DESC LIMIT 1",
    )
    .bind(institucion_id)
    .bind(num_nota)
    .fetch_optional(pool)
    .await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> Pool {
        crate::abrir("sqlite::memory:").await.unwrap()
    }

    fn nuevo<'a>(num_nota: &'a str, aceptado: bool, cuv: Option<&'a str>) -> Nuevo<'a> {
        Nuevo {
            institucion_id: 1,
            usuario_id: 7,
            periodo_inicio: "2026-03-01",
            periodo_fin: "2026-03-31",
            num_nota,
            destino: "sin_factura",
            aceptado,
            cuv,
            proceso_id: Some(42),
            fecha_radicacion: Some("2026-03-31"),
            ambiente: Some("DockerStage"),
            hallazgos: "[]",
            registros: 12,
        }
    }

    #[tokio::test]
    async fn un_envio_aceptado_guarda_su_cuv() {
        let p = base().await;
        registrar(&p, nuevo("RS-1", true, Some("CUV-OK"))).await.unwrap();
        let v = del_periodo(&p, Ambito::Institucion(1), "2026-03-01", "2026-03-31")
            .await
            .unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].aceptado);
        assert_eq!(v[0].cuv.as_deref(), Some("CUV-OK"));
        assert_eq!(v[0].ambiente.as_deref(), Some("DockerStage"));
    }

    #[tokio::test]
    async fn un_rechazado_no_guarda_cuv_aunque_se_lo_den() {
        // La pareja de la anterior, y el caso que el CHECK de la migración
        // convierte en imposible: un CUV en una fila rechazada sería el
        // comprobante de algo que no pasó. Aquí se comprueba que el llamante NO
        // puede meterlo ni equivocándose.
        let p = base().await;
        registrar(&p, nuevo("RS-2", false, Some("CUV-QUE-NO-EXISTE")))
            .await
            .expect("no puede reventar a mitad de un envío ya hecho");
        let v = del_periodo(&p, Ambito::Institucion(1), "2026-03-01", "2026-03-31")
            .await
            .unwrap();
        assert!(!v[0].aceptado);
        assert!(v[0].cuv.is_none(), "un rechazo no tiene comprobante");
    }

    #[tokio::test]
    async fn se_guardan_todos_los_envios_del_periodo_no_solo_el_ultimo() {
        let p = base().await;
        registrar(&p, nuevo("RS-1", false, None)).await.unwrap();
        registrar(&p, nuevo("RS-2", true, Some("CUV-OK"))).await.unwrap();
        let v = del_periodo(&p, Ambito::Institucion(1), "2026-03-01", "2026-03-31")
            .await
            .unwrap();
        assert_eq!(v.len(), 2, "el reenvío tras corregir es la historia del periodo");
    }

    #[tokio::test]
    async fn el_periodo_de_otra_institucion_no_se_ve() {
        let p = base().await;
        registrar(&p, nuevo("RS-1", true, Some("CUV-OK"))).await.unwrap();
        let ajenos = del_periodo(&p, Ambito::Institucion(2), "2026-03-01", "2026-03-31")
            .await
            .unwrap();
        assert!(ajenos.is_empty());
    }

    #[tokio::test]
    async fn el_superadmin_no_se_lleva_los_de_todos() {
        // Devolver los de TODAS las instituciones aquí sería la fuga que este
        // repositorio ya cerró en `/financiero`.
        let p = base().await;
        registrar(&p, nuevo("RS-1", true, Some("CUV-OK"))).await.unwrap();
        assert!(
            del_periodo(&p, Ambito::Superadmin, "2026-03-01", "2026-03-31")
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn un_numero_que_ya_obtuvo_cuv_queda_consumido() {
        let p = base().await;
        registrar(&p, nuevo("RS-1", true, Some("CUV-OK"))).await.unwrap();
        assert!(nota_ya_aceptada(&p, 1, "RS-1").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn un_numero_que_solo_fue_rechazado_se_puede_reusar() {
        // La mitad que hace que la puerta discrimine. Sin este caso, cerrar
        // sobre CUALQUIER envío anterior pasaría igual — y obligaría a quemar un
        // consecutivo por cada errata (directiva 21).
        let p = base().await;
        registrar(&p, nuevo("RS-9", false, None)).await.unwrap();
        assert!(
            nota_ya_aceptada(&p, 1, "RS-9").await.unwrap().is_none(),
            "un archivo rechazado no se radicó: su consecutivo sigue libre"
        );
    }

    #[tokio::test]
    async fn el_numero_consumido_por_otra_institucion_no_estorba() {
        let p = base().await;
        registrar(&p, nuevo("RS-1", true, Some("CUV-OK"))).await.unwrap();
        assert!(
            nota_ya_aceptada(&p, 2, "RS-1").await.unwrap().is_none(),
            "el consecutivo es de cada prestador, no del sistema"
        );
    }
}
