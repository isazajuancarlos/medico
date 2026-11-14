//! La cola de envío de los RDA al mecanismo IHCE (#603 fase 4b): aquí las filas
//! y la política de reintento; el transporte, en `medico-ihce`.
//!
//! El guardado clínico ENCOLA con [`encolar`]; una tarea de fondo llama a
//! [`drenar`] en un bucle. `drenar` no sabe hablar HTTP: recibe el envío como un
//! *closure* que devuelve un [`Resultado`] ya clasificado, igual que
//! `guaca::auditoria` recibe el firmante. Así este módulo no depende de
//! `medico-ihce` —la app cablea el `ClienteIhce` real y traduce sus errores— y se
//! prueba entero sin red, con un closure de mentira.
//!
//! # La política, en una frase
//!
//! Un `Aceptado` o un `Duplicado` son terminales. Un `Reintentar` empuja el
//! siguiente intento con backoff exponencial hasta [`MAX_INTENTOS`], y entonces
//! pasa a `fallido`. Un `Permanente` para en seco: reintentar un Bundle mal
//! formado solo gasta.

use std::future::Future;

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

use crate::Pool;

/// La base del backoff: la primera espera tras un fallo transitorio.
pub const BASE_SEGUNDOS: i64 = 30;
/// El techo de la espera: un mecanismo caído no se reintenta más lento que esto.
pub const TOPE_SEGUNDOS: i64 = 3600;
/// Cuántos intentos transitorios antes de rendirse y marcar `fallido`.
pub const MAX_INTENTOS: i64 = 8;

/// El desenlace de intentar entregar UN RDA, tal como lo clasifica el llamante
/// —que es quien conoce el transporte—. La cola solo aplica la política.
#[derive(Debug, Clone, PartialEq)]
pub enum Resultado {
    /// 200: el mecanismo lo recibió. Trae el identificador del RDA si vino.
    Aceptado(Option<String>),
    /// 409: ya existía. Terminal y NO es alarma —el dato está en la plataforma—.
    Duplicado,
    /// Fallo transitorio (red, token, 5xx): se reintenta con backoff. El texto es
    /// el motivo, para la trazabilidad.
    Reintentar(String),
    /// Fallo permanente (400 de estructura, Bundle ilegible): no se reintenta.
    Permanente(String),
}

/// La espera antes del intento número `intentos` (1 = el primero tras un fallo):
/// exponencial desde [`BASE_SEGUNDOS`], con techo en [`TOPE_SEGUNDOS`].
pub fn espera_de(intentos: i64) -> Duration {
    if intentos <= 1 {
        return Duration::seconds(BASE_SEGUNDOS);
    }
    // `saturating` y el techo antes de convertir: con muchos intentos `1 << n`
    // desborda, y un desbordamiento aquí daría una espera negativa —martillear
    // justo cuando más conviene esperar—.
    let exp = (intentos - 1).min(32) as u32;
    let seg = BASE_SEGUNDOS.saturating_mul(1i64 << exp).min(TOPE_SEGUNDOS);
    Duration::seconds(seg)
}

/// Encola un RDA ya ensamblado para envío. Nace `pendiente` con su turno AHORA.
pub async fn encolar(
    pool: &Pool,
    institucion_id: i64,
    operacion: &str,
    bundle: &Value,
    ahora: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    let t = ahora.to_rfc3339();
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO envio_rda
           (institucion_id, operacion, bundle, estado, intentos, proximo_intento, creado, actualizado)
         VALUES (?, ?, ?, 'pendiente', 0, ?, ?, ?)
         RETURNING id",
    )
    .bind(institucion_id)
    .bind(operacion)
    .bind(bundle.to_string())
    .bind(&t)
    .bind(&t)
    .bind(&t)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Lo que dejó una pasada de la cola. Para el log de la tarea de fondo.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Resumen {
    pub aceptados: u32,
    pub duplicados: u32,
    pub reintentados: u32,
    pub fallidos: u32,
}

/// Una fila lista para enviar.
struct Porenviar {
    id: i64,
    operacion: String,
    bundle: String,
    intentos: i64,
}

/// Drena las filas cuyo turno llegó: por cada `pendiente` con
/// `proximo_intento <= ahora`, llama a `enviar(operacion, bundle)` y aplica el
/// [`Resultado`]. Se asume UN drenador a la vez (la tarea de fondo es una): no
/// marca `enviando`, así que dos drenadores concurrentes podrían enviar la misma
/// fila dos veces —lo cual el mecanismo, de todos modos, atrapa con su 409—.
pub async fn drenar<F, Fut>(
    pool: &Pool,
    ahora: DateTime<Utc>,
    enviar: F,
) -> Result<Resumen, sqlx::Error>
where
    F: Fn(String, Value) -> Fut,
    Fut: Future<Output = Resultado>,
{
    let filas: Vec<Porenviar> = sqlx::query_as::<_, (i64, String, String, i64)>(
        "SELECT id, operacion, bundle, intentos
           FROM envio_rda
          WHERE estado = 'pendiente' AND proximo_intento <= ?
          ORDER BY proximo_intento, id",
    )
    .bind(ahora.to_rfc3339())
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, operacion, bundle, intentos)| Porenviar { id, operacion, bundle, intentos })
    .collect();

    let mut r = Resumen::default();
    let t = ahora.to_rfc3339();
    for f in filas {
        // Un bundle que no se puede releer de la base no se envía como vacío: se
        // para como permanente (directiva 20). Que llegara aquí significa que la
        // fila se corrompió, no que el envío deba adivinar.
        let bundle: Value = match serde_json::from_str(&f.bundle) {
            Ok(v) => v,
            Err(e) => {
                marcar_fallido(pool, f.id, &format!("bundle ilegible en la cola: {e}"), &t).await?;
                r.fallidos += 1;
                continue;
            }
        };

        match enviar(f.operacion, bundle).await {
            Resultado::Aceptado(rda_id) => {
                sqlx::query(
                    "UPDATE envio_rda SET estado='aceptado', rda_id=?, ultimo_error=NULL, actualizado=? WHERE id=?",
                )
                .bind(rda_id)
                .bind(&t)
                .bind(f.id)
                .execute(pool)
                .await?;
                r.aceptados += 1;
            }
            Resultado::Duplicado => {
                sqlx::query("UPDATE envio_rda SET estado='duplicado', actualizado=? WHERE id=?")
                    .bind(&t)
                    .bind(f.id)
                    .execute(pool)
                    .await?;
                r.duplicados += 1;
            }
            Resultado::Permanente(motivo) => {
                marcar_fallido(pool, f.id, &motivo, &t).await?;
                r.fallidos += 1;
            }
            Resultado::Reintentar(motivo) => {
                let intentos = f.intentos + 1;
                if intentos >= MAX_INTENTOS {
                    // Se registra el conteo final además del estado: `marcar_fallido`
                    // no toca `intentos` (lo usan el permanente y el bundle
                    // ilegible, donde no debe cambiar), pero agotar los reintentos
                    // SÍ consumió este intento y la fila debe decir cuántos hubo.
                    sqlx::query(
                        "UPDATE envio_rda SET estado='fallido', intentos=?, ultimo_error=?, actualizado=? WHERE id=?",
                    )
                    .bind(intentos)
                    .bind(format!("agotó {MAX_INTENTOS} reintentos; último: {motivo}"))
                    .bind(&t)
                    .bind(f.id)
                    .execute(pool)
                    .await?;
                    r.fallidos += 1;
                } else {
                    let proximo = (ahora + espera_de(intentos)).to_rfc3339();
                    sqlx::query(
                        "UPDATE envio_rda SET intentos=?, proximo_intento=?, ultimo_error=?, actualizado=? WHERE id=?",
                    )
                    .bind(intentos)
                    .bind(&proximo)
                    .bind(&motivo)
                    .bind(&t)
                    .bind(f.id)
                    .execute(pool)
                    .await?;
                    r.reintentados += 1;
                }
            }
        }
    }
    Ok(r)
}

async fn marcar_fallido(pool: &Pool, id: i64, motivo: &str, ahora: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE envio_rda SET estado='fallido', ultimo_error=?, actualizado=? WHERE id=?")
        .bind(motivo)
        .bind(ahora)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    async fn base() -> Pool {
        crate::abrir("sqlite::memory:").await.unwrap()
    }

    fn bundle() -> Value {
        json!({ "resourceType": "Bundle", "type": "document" })
    }

    async fn estado_de(pool: &Pool, id: i64) -> (String, i64, Option<String>, Option<String>) {
        sqlx::query_as("SELECT estado, intentos, ultimo_error, rda_id FROM envio_rda WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn un_envio_aceptado_queda_aceptado_con_su_id() {
        let pool = base().await;
        let t0 = Utc::now();
        let id = encolar(&pool, 1, "paciente", &bundle(), t0).await.unwrap();

        let r = drenar(&pool, t0, |op, _b| async move {
            assert_eq!(op, "paciente");
            Resultado::Aceptado(Some("RDA-9".into()))
        })
        .await
        .unwrap();

        assert_eq!(r.aceptados, 1);
        let (estado, _, err, rda) = estado_de(&pool, id).await;
        assert_eq!(estado, "aceptado");
        assert_eq!(rda.as_deref(), Some("RDA-9"));
        assert_eq!(err, None);
    }

    #[tokio::test]
    async fn un_409_queda_duplicado_y_no_es_fallido() {
        let pool = base().await;
        let t0 = Utc::now();
        let id = encolar(&pool, 1, "paciente", &bundle(), t0).await.unwrap();

        let r = drenar(&pool, t0, |_o, _b| async { Resultado::Duplicado }).await.unwrap();
        assert_eq!(r.duplicados, 1);
        assert_eq!(estado_de(&pool, id).await.0, "duplicado", "un 409 no debe leerse como fallo");
    }

    #[tokio::test]
    async fn un_permanente_no_se_reintenta() {
        let pool = base().await;
        let t0 = Utc::now();
        let id = encolar(&pool, 1, "paciente", &bundle(), t0).await.unwrap();

        let r = drenar(&pool, t0, |_o, _b| async { Resultado::Permanente("400: falta Composition".into()) })
            .await
            .unwrap();
        assert_eq!(r.fallidos, 1);
        let (estado, intentos, err, _) = estado_de(&pool, id).await;
        assert_eq!(estado, "fallido");
        assert_eq!(intentos, 0, "un permanente no gasta un intento: para en seco");
        assert!(err.unwrap().contains("Composition"));
    }

    /// Un fallo transitorio NO se reintenta EN EL ACTO: se empuja al futuro con
    /// backoff. La pareja de esto es que, pasado ese tiempo, sí vuelve a salir.
    #[tokio::test]
    async fn un_transitorio_espera_su_backoff_antes_de_reintentar() {
        let pool = base().await;
        let t0 = Utc::now();
        let id = encolar(&pool, 1, "paciente", &bundle(), t0).await.unwrap();

        // Primer intento: falla transitorio → queda pendiente, con el turno en el futuro.
        let r = drenar(&pool, t0, |_o, _b| async { Resultado::Reintentar("red".into()) }).await.unwrap();
        assert_eq!(r.reintentados, 1);
        let (estado, intentos, _, _) = estado_de(&pool, id).await;
        assert_eq!(estado, "pendiente");
        assert_eq!(intentos, 1);

        // Vuelto a drenar EN EL MISMO instante: su turno aún no llegó, no sale.
        let r = drenar(&pool, t0, |_o, _b| async { panic!("no debía intentarse antes del backoff") })
            .await
            .unwrap();
        assert_eq!(r, Resumen::default(), "reintentó antes de cumplir el backoff");

        // Pasado el backoff (base + margen): ahora SÍ vuelve a intentarse.
        let luego = t0 + espera_de(1) + Duration::seconds(1);
        let r = drenar(&pool, luego, |_o, _b| async { Resultado::Aceptado(None) }).await.unwrap();
        assert_eq!(r.aceptados, 1);
        assert_eq!(estado_de(&pool, id).await.0, "aceptado");
    }

    /// Tras agotar los reintentos, un transitorio persistente pasa a `fallido`:
    /// una cola que reintenta para siempre esconde un problema en vez de gritarlo.
    #[tokio::test]
    async fn agotados_los_reintentos_pasa_a_fallido() {
        let pool = base().await;
        let mut t = Utc::now();
        let id = encolar(&pool, 1, "paciente", &bundle(), t).await.unwrap();

        // Se drena repetidamente, avanzando el reloj más allá de cada backoff,
        // y SIEMPRE falla transitorio. Debe rendirse a los MAX_INTENTOS.
        for _ in 0..MAX_INTENTOS {
            t += Duration::seconds(TOPE_SEGUNDOS + 1);
            drenar(&pool, t, |_o, _b| async { Resultado::Reintentar("sigue caído".into()) })
                .await
                .unwrap();
        }
        let (estado, intentos, err, _) = estado_de(&pool, id).await;
        assert_eq!(estado, "fallido", "no se rindió tras agotar los reintentos");
        assert_eq!(intentos, MAX_INTENTOS);
        assert!(err.unwrap().contains("agotó"));
    }

    #[tokio::test]
    async fn el_backoff_crece_y_topa() {
        assert_eq!(espera_de(1), Duration::seconds(BASE_SEGUNDOS));
        assert_eq!(espera_de(2), Duration::seconds(BASE_SEGUNDOS * 2));
        assert_eq!(espera_de(3), Duration::seconds(BASE_SEGUNDOS * 4));
        assert!(espera_de(2) > espera_de(1), "el backoff no crece");
        assert_eq!(espera_de(100), Duration::seconds(TOPE_SEGUNDOS), "el backoff no topó");
    }

    /// Un bundle que se corrompió en la base no se envía como vacío: se para como
    /// fallido (directiva 20). El closure ni se llama.
    #[tokio::test]
    async fn un_bundle_ilegible_no_se_envia() {
        let pool = base().await;
        let t0 = Utc::now();
        sqlx::query(
            "INSERT INTO envio_rda (institucion_id,operacion,bundle,estado,intentos,proximo_intento,creado,actualizado)
             VALUES (1,'paciente','esto no es json','pendiente',0,?,?,?)",
        )
        .bind(t0.to_rfc3339()).bind(t0.to_rfc3339()).bind(t0.to_rfc3339())
        .execute(&pool).await.unwrap();

        let r = drenar(&pool, t0, |_o, _b| async { panic!("no debía intentar enviar un bundle ilegible") })
            .await
            .unwrap();
        assert_eq!(r.fallidos, 1);
    }

    /// Directiva 8: 120 encolados se drenan sin perder ninguno.
    #[tokio::test]
    async fn ciento_veinte_encolados_se_drenan() {
        let pool = base().await;
        let t0 = Utc::now();
        for _ in 0..120 {
            encolar(&pool, 1, "paciente", &bundle(), t0).await.unwrap();
        }
        let r = drenar(&pool, t0, |_o, _b| async { Resultado::Aceptado(None) }).await.unwrap();
        assert_eq!(r.aceptados, 120, "se perdieron envíos al drenar");
        let pendientes: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM envio_rda WHERE estado='pendiente'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pendientes, 0);
    }
}
