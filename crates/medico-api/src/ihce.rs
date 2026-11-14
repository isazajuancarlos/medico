//! El cableado entre la cola de envíos (`medico_datos::cola_rda`) y el transporte
//! (`medico_ihce`): aquí vive la POLÍTICA de reintento —qué error del mecanismo
//! se reintenta y cuál para en seco— y el paso que drena una vez con el cliente
//! real.
//!
//! La cola no conoce el transporte y el transporte no conoce la cola; este módulo
//! es la costura, y es de medico porque la decisión «un 400 no se reintenta, un
//! 5xx sí» es de producto, no de ninguna de las dos piezas.

use medico_datos::cola_rda::{self, Resultado, Resumen};
use medico_datos::Pool;
use medico_ihce::{ClienteIhce, Config, ErrorIhce, OperacionRda};

/// Lee la configuración del mecanismo IHCE de una fuente de variables. Devuelve
/// `Some` **solo si están TODAS**: una configuración a medias no arranca una cola
/// que enviaría a ninguna parte (directiva 20). Toma la fuente como parámetro
/// para poder probar el parseo sin tocar el entorno global del proceso.
pub fn config_desde<F: Fn(&str) -> Option<String>>(get: F) -> Option<Config> {
    Some(Config::azure(
        &get("MEDICO_IHCE_TENANT")?,
        get("MEDICO_IHCE_BASE_URL")?,
        get("MEDICO_IHCE_CLIENT_ID")?,
        get("MEDICO_IHCE_CLIENT_SECRET")?,
        get("MEDICO_IHCE_SCOPE")?,
        get("MEDICO_IHCE_SUBSCRIPTION_KEY")?,
    ))
}

/// La configuración desde el entorno del proceso. `None` = la cola IHCE no está
/// configurada, que es el caso normal hasta tener las credenciales de sandbox.
/// Los secretos (`CLIENT_SECRET`) viven en el `.env` del servidor / una bóveda,
/// nunca en el repo (art. 6.2 Res. 1888/2025 y la regla de secretos general).
pub fn config_desde_entorno() -> Option<Config> {
    config_desde(|k| std::env::var(k).ok())
}

/// El bucle de la tarea de fondo: drena la cola, espera, repite. Un fallo de una
/// pasada no la mata —el mecanismo puede caerse y volver—; se anota y se sigue.
pub async fn correr_cola(pool: Pool, cliente: ClienteIhce) {
    const INTERVALO: std::time::Duration = std::time::Duration::from_secs(30);
    loop {
        match drenar_una_vez(&pool, &cliente).await {
            Ok(r) if r != Resumen::default() => println!(
                "cola IHCE: {} aceptados · {} duplicados · {} reintentos · {} fallidos",
                r.aceptados, r.duplicados, r.reintentados, r.fallidos
            ),
            Ok(_) => {}
            Err(e) => eprintln!("cola IHCE: error drenando la cola: {e}"),
        }
        tokio::time::sleep(INTERVALO).await;
    }
}

/// Traduce el fallo del transporte a la decisión de la cola.
///
/// **Permanente** lo que no cambia por reintentar: un 400 de estructura (el
/// Bundle está mal formado) y un Bundle que ni siquiera serializa. **Duplicado**
/// el 409, que es terminal y no una alarma. Todo lo demás —red caída, token
/// vencido (401/403), un 5xx, algo inesperado— es transitorio y **se reintenta**
/// con backoff; si de verdad es persistente, la cola se rinde a los `MAX_INTENTOS`
/// y lo marca `fallido`, así que reintentar de más no es un bucle infinito.
pub fn clasificar(e: ErrorIhce) -> Resultado {
    match e {
        ErrorIhce::Duplicado(_) => Resultado::Duplicado,
        ErrorIhce::Estructura(oo) => {
            Resultado::Permanente(format!("400 estructura: {}", oo.diagnostico()))
        }
        ErrorIhce::Serializacion(err) => {
            Resultado::Permanente(format!("bundle no serializable: {err}"))
        }
        otro => Resultado::Reintentar(otro.to_string()),
    }
}

/// Drena la cola UNA vez con el cliente real. La tarea de fondo la llama en un
/// bucle; se deja como una función para poder probarla contra el mock.
///
/// El `id` del RDA aceptado se extrae de `cuerpo["id"]` a la ligera: DÓNDE vive de
/// verdad lo fija el IG (aún no traído), así que hoy se guarda lo que haya sin
/// depender de una forma que todavía no está fijada.
pub async fn drenar_una_vez(pool: &Pool, cliente: &ClienteIhce) -> Result<Resumen, sqlx::Error> {
    cola_rda::drenar(pool, chrono::Utc::now(), |op_nombre, bundle| async move {
        let Some(op) = OperacionRda::desde_nombre(&op_nombre) else {
            return Resultado::Permanente(format!("operación desconocida en la cola: {op_nombre}"));
        };
        match cliente.enviar_rda(op, &bundle).await {
            Ok(resp) => {
                Resultado::Aceptado(resp.cuerpo.get("id").and_then(|v| v.as_str()).map(str::to_string))
            }
            Err(e) => clasificar(e),
        }
    })
    .await
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use medico_ihce::{Config, OperationOutcome};

    fn oo(msg: &str) -> OperationOutcome {
        // El `issue` con su diagnóstico, construido a mano: es la forma que el
        // mecanismo devuelve en un 400/409.
        serde_json::from_value(serde_json::json!({
            "issue": [{ "diagnostics": msg }]
        }))
        .unwrap()
    }

    /// **El 409 no se reintenta: es duplicado, no fallo.**
    #[test]
    fn el_409_es_duplicado() {
        assert_eq!(clasificar(ErrorIhce::Duplicado(oo("Encounter repetido"))), Resultado::Duplicado);
    }

    /// **El 400 de estructura NO se reintenta**: el Bundle está mal y reintentarlo
    /// no lo arregla. La pareja es el bucket de al lado: lo transitorio SÍ se
    /// reintenta —sin esta mitad, un `clasificar` que devolviera siempre
    /// `Permanente` pasaría igual—.
    #[test]
    fn el_400_es_permanente_pero_lo_transitorio_se_reintenta() {
        assert!(matches!(clasificar(ErrorIhce::Estructura(oo("falta Composition"))), Resultado::Permanente(_)));
        let err = serde_json::from_str::<i32>("no-numero").unwrap_err();
        assert!(matches!(clasificar(ErrorIhce::Serializacion(err)), Resultado::Permanente(_)));

        // Transitorios: token, autorización, algo inesperado.
        assert!(matches!(clasificar(ErrorIhce::AuthFallida(500)), Resultado::Reintentar(_)));
        assert!(matches!(clasificar(ErrorIhce::AutorizacionRechazada(401)), Resultado::Reintentar(_)));
        assert!(matches!(
            clasificar(ErrorIhce::Inesperado { status: 503, cuerpo: "gateway".into() }),
            Resultado::Reintentar(_)
        ));
    }

    /// **La configuración se arma solo si están TODAS las variables.** La pareja
    /// es la segunda mitad: quitar cualquiera devuelve `None` —una cola a medias
    /// no arranca (directiva 20)—; sin ella, un `config_desde` que ignorara una
    /// variable pasaría igual.
    #[test]
    fn la_config_exige_todas_las_variables() {
        use std::collections::HashMap;
        let completo: HashMap<&str, &str> = [
            ("MEDICO_IHCE_TENANT", "t"),
            ("MEDICO_IHCE_BASE_URL", "https://api"),
            ("MEDICO_IHCE_CLIENT_ID", "c"),
            ("MEDICO_IHCE_CLIENT_SECRET", "s"),
            ("MEDICO_IHCE_SCOPE", "x"),
            ("MEDICO_IHCE_SUBSCRIPTION_KEY", "k"),
        ]
        .into_iter()
        .collect();

        let cfg = config_desde(|k| completo.get(k).map(|v| v.to_string())).unwrap();
        assert!(cfg.token_url.contains("/t/oauth2"), "el tenant no entró en la URL: {}", cfg.token_url);
        assert_eq!(cfg.api_base, "https://api");

        for faltante in completo.keys() {
            let get =
                |k: &str| if k == *faltante { None } else { completo.get(k).map(|v| v.to_string()) };
            assert!(config_desde(get).is_none(), "arrancó sin {faltante}");
        }
    }

    // ── E2E: la cola + el cliente REAL + un mecanismo de mentira ──────────────
    //
    // Cruza la frontera (directiva 28): levanta un servidor HTTP real y le habla
    // por reqwest, encadenado con la cola. Un `assert` de estructura a estructura
    // mediría el mapeo, no el envío.

    async fn mecanismo(status_envio: u16, cuerpo: &'static str) -> String {
        use axum::extract::Path;
        use axum::http::StatusCode;
        use axum::routing::post;
        use axum::{Json, Router};

        let app = Router::new()
            .route(
                "/tenant/oauth2/v2.0/token",
                post(|| async {
                    Json(serde_json::json!({
                        "access_token": "tok", "token_type": "Bearer", "expires_in": 3599
                    }))
                }),
            )
            .route(
                "/Composition/{op}",
                post(move |Path(_op): Path<String>| async move {
                    (StatusCode::from_u16(status_envio).unwrap(), cuerpo.to_string())
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    fn cliente(base: &str) -> ClienteIhce {
        ClienteIhce::nuevo(Config {
            token_url: format!("{base}/tenant/oauth2/v2.0/token"),
            api_base: base.to_string(),
            client_id: "c".into(),
            client_secret: "s".into(),
            scope: "x".into(),
            subscription_key: "k".into(),
        })
    }

    async fn encolar_uno(pool: &Pool) -> i64 {
        cola_rda::encolar(
            pool,
            1,
            "paciente",
            &serde_json::json!({ "resourceType": "Bundle", "type": "document" }),
            chrono::Utc::now(),
        )
        .await
        .unwrap()
    }

    async fn estado_de(pool: &Pool, id: i64) -> String {
        sqlx::query_scalar("SELECT estado FROM envio_rda WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Un encolado que el mecanismo acepta (200) sale de la cola como `aceptado`,
    /// con el id que devolvió — todo el camino: cola → cliente → HTTP → cola.
    #[tokio::test]
    async fn un_encolado_se_drena_y_queda_aceptado() {
        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        let id = encolar_uno(&pool).await;
        let cli = cliente(&mecanismo(200, r#"{"resourceType":"Bundle","id":"RDA-1"}"#).await);

        let r = drenar_una_vez(&pool, &cli).await.unwrap();
        assert_eq!(r.aceptados, 1);
        assert_eq!(estado_de(&pool, id).await, "aceptado");
        let rda: Option<String> = sqlx::query_scalar("SELECT rda_id FROM envio_rda WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rda.as_deref(), Some("RDA-1"));
    }

    /// La pareja por la vía real: un 409 del mecanismo no se reintenta —queda
    /// `duplicado`—, y así se comprueba que `clasificar` está de verdad enchufado
    /// entre el cliente y la cola, no solo probado a solas.
    #[tokio::test]
    async fn un_409_del_mecanismo_deja_el_encolado_como_duplicado() {
        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        let id = encolar_uno(&pool).await;
        let cli = cliente(&mecanismo(409, r#"{"issue":[{"diagnostics":"Encounter repetido"}]}"#).await);

        let r = drenar_una_vez(&pool, &cli).await.unwrap();
        assert_eq!(r.duplicados, 1);
        assert_eq!(estado_de(&pool, id).await, "duplicado");
    }

    /// **EL AMARRE entre el que encola y el que envía** (#603 F4.6). `medico-datos`
    /// encola la consulta externa con el literal
    /// `rda_encuentro::OPERACION_CONSULTA_EXTERNA` porque no puede depender de
    /// `medico-ihce`; el transporte la vuelve a decodificar con
    /// `OperacionRda::desde_nombre`. Si los dos literales se separan, una fila
    /// encolada saldría del drenaje como `Permanente` («operación desconocida») sin
    /// llegar nunca al visor — un fallo que no se ve al escribir el encolado. Esta
    /// prueba vive AQUÍ, el único crate que ve los dos lados, y cierra el lazo por
    /// ambas direcciones (el nombre canónico es el del transporte; el encolado lo
    /// copia; el decodificador lo reconoce).
    #[test]
    fn la_operacion_encolada_la_decodifica_el_transporte() {
        use medico_datos::rda_encuentro::{
            OPERACION_CONSULTA_EXTERNA, OPERACION_HOSPITALIZACION, OPERACION_URGENCIAS,
        };
        // Los TRES literales con que `rda_encuentro`/`rda_aval` encolan y llavean el
        // aval tienen que ser EXACTAMENTE los nombres canónicos del transporte, en
        // los dos sentidos (nombre ↔ desde_nombre). Sin este amarre para
        // hospitalización y urgencias (añadido tras el concilio del 2026-09-17), una
        // deriva se drenaría como `Permanente` («operación desconocida») sin que nada
        // se pusiera rojo — y los docstrings de esas constantes PROMETÍAN este caso rojo.
        for (op, nombre) in [
            (OperacionRda::ConsultaExterna, OPERACION_CONSULTA_EXTERNA),
            (OperacionRda::Hospitalizacion, OPERACION_HOSPITALIZACION),
            (OperacionRda::Urgencias, OPERACION_URGENCIAS),
        ] {
            assert_eq!(op.nombre(), nombre, "el literal de la cola no es el nombre canónico del transporte");
            assert_eq!(
                OperacionRda::desde_nombre(nombre),
                Some(op),
                "el transporte no reconoce la operación que la cola persiste: se drenaría como desconocida"
            );
        }
    }
}
