//! Transporte y autenticación con el mecanismo IHCE (RDA) del MinSalud.
//!
//! Aquí vive la MITAD de #603 que no depende de la Guía de Implementación FHIR:
//! el flujo OAuth2, los encabezados, las URLs de operación y el mapeo de errores.
//! El CONTENIDO del `Bundle` —qué recursos, con qué perfiles— lo define el paquete
//! `minsalud.fhir.co.rda`, que aún no está; por eso este crate recibe el bundle ya
//! ensamblado como `serde_json::Value` y solo se encarga de LLEVARLO.
//!
//! La especificación completa, transcrita del Manual de Operaciones IHCE v01
//! (MinSalud, oct-2025), está en `crates/medico-dominio/datos/ihce/TRANSPORTE.md`.
//!
//! # Lo que este crate NO hace todavía, a propósito
//!
//! - ~~**No habla HTTPS**~~ — **ya sí, desde el 2026-08-10**, y sin haberlo pedido:
//!   las features de Cargo son UNIÓN, y `medico-muv` activó `rustls-tls` sobre el
//!   `reqwest` que los dos comparten (decisión de Juan, con ISC y
//!   CDLA-Permissive-2.0 entrando en `deny.toml` con su motivo). Apuntar `api_base`
//!   a la URL HTTPS real de Azure ya no falla al conectar. Lo que sigue sin estar
//!   probado es el envío contra el mecanismo de verdad: hace falta el sandbox.
//! - **No encola ni reintenta**: el envío asíncrono con backoff que pide el manual
//!   es la otra mitad de la fase 4, y va sobre este cliente, no dentro de él.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Margen antes de que el token venza en el que ya se pide uno nuevo: un token
/// que caduca a mitad de una petición en vuelo da un 401 evitable.
const MARGEN_SEGUNDOS: u64 = 60;

/// Las credenciales y URLs del mecanismo, TODO como configuración.
///
/// La URL base y las credenciales las entrega MinSalud al onboarding (Manual de
/// Gestión de Credenciales); no se fijan en código (directiva 20) y los secretos
/// no se guardan en el repo (art. 6.2 Res. 1888/2025: bóveda, nunca texto plano).
#[derive(Debug, Clone)]
pub struct Config {
    /// El endpoint de token, con el tenant ya interpolado. En producción:
    /// `https://login.microsoftonline.com/<tenant>/oauth2/v2.0/token`.
    pub token_url: String,
    /// La raíz de la API IHCE, sin barra final. Las operaciones se le pegan.
    pub api_base: String,
    pub client_id: String,
    pub client_secret: String,
    pub scope: String,
    /// La clave de suscripción de Azure API Management (`Ocp-Apim-Subscription-Key`).
    pub subscription_key: String,
}

impl Config {
    /// Construye la `token_url` de Azure AD a partir del tenant, que es la única
    /// pieza que se arma y no se recibe entera.
    pub fn azure(
        tenant_id: &str,
        api_base: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        scope: impl Into<String>,
        subscription_key: impl Into<String>,
    ) -> Self {
        Self {
            token_url: format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token"),
            api_base: api_base.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            scope: scope.into(),
            subscription_key: subscription_key.into(),
        }
    }
}

/// Cuál de los cuatro RDA se envía. Cada uno es una operación distinta de la API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperacionRda {
    Paciente,
    Hospitalizacion,
    ConsultaExterna,
    Urgencias,
}

impl OperacionRda {
    /// El nombre de la operación FHIR, tal como lo publica el manual (§5.5).
    pub fn ruta(self) -> &'static str {
        match self {
            OperacionRda::Paciente => "$enviar-rda-paciente",
            OperacionRda::Hospitalizacion => "$enviar-rda-hospitalizacion",
            // `$enviar-rda-consulta`, NO `-externa`. Corregido el 2026-08-14
            // contra la colección Postman oficial del sandbox: el NOMBRE de la
            // petición es «enviar-rda-consulta-externa» y su URL es
            // «$enviar-rda-consulta». Escrito de memoria, esto habría dado 404
            // en el 25 % de los envíos — y un 404 en una operación FHIR se lee
            // como «el servidor no la tiene», no como «la escribimos mal».
            OperacionRda::ConsultaExterna => "$enviar-rda-consulta",
            OperacionRda::Urgencias => "$enviar-rda-urgencias",
        }
    }

    /// El nombre CORTO, estable, con el que la cola de envíos guarda la operación
    /// en la base (`cola_rda`). Se separa de [`ruta`] a propósito: la ruta es
    /// parte del contrato HTTP y podría cambiar; esto es la clave persistida, y
    /// cambiarla obligaría a migrar filas encoladas.
    pub fn nombre(self) -> &'static str {
        match self {
            OperacionRda::Paciente => "paciente",
            OperacionRda::Hospitalizacion => "hospitalizacion",
            OperacionRda::ConsultaExterna => "consulta-externa",
            OperacionRda::Urgencias => "urgencias",
        }
    }

    /// Lee la operación de una fila de la cola. Devuelve `None` ante un valor
    /// desconocido —no cae a un defecto (directiva 20)—: una fila con una
    /// operación que este binario no reconoce la escribió algo que no es este
    /// código, y enviarla como «paciente» por descarte sería peor que pararla.
    pub fn desde_nombre(s: &str) -> Option<Self> {
        match s {
            "paciente" => Some(OperacionRda::Paciente),
            "hospitalizacion" => Some(OperacionRda::Hospitalizacion),
            "consulta-externa" => Some(OperacionRda::ConsultaExterna),
            "urgencias" => Some(OperacionRda::Urgencias),
            _ => None,
        }
    }
}

/// Un `issue` de un `OperationOutcome` de HL7 FHIR (§5.6).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Issue {
    pub severity: Option<String>,
    pub code: Option<String>,
    pub diagnostics: Option<String>,
    #[serde(default)]
    pub location: Vec<String>,
}

/// El recurso de error del mecanismo. Se conserva entero para que el llamante
/// pueda mirar cada `issue`; `diagnostico()` es el atajo para el mensaje.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OperationOutcome {
    #[serde(default)]
    pub issue: Vec<Issue>,
}

impl OperationOutcome {
    /// Interpreta el cuerpo de una respuesta de error. Si NO es un
    /// `OperationOutcome` legible, no se pierde el dato ni se revienta
    /// (directiva 20): se envuelve el texto crudo en un `issue` sintético.
    fn desde(cuerpo: &str) -> Self {
        serde_json::from_str(cuerpo).unwrap_or_else(|_| OperationOutcome {
            issue: vec![Issue {
                severity: Some("error".into()),
                code: Some("invalid".into()),
                diagnostics: Some(cuerpo.to_string()),
                location: Vec::new(),
            }],
        })
    }

    /// Los `diagnostics` de todos los `issue`, unidos. Para logs y mensajes.
    pub fn diagnostico(&self) -> String {
        self.issue
            .iter()
            .filter_map(|i| i.diagnostics.as_deref())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Lo que devuelve un envío aceptado: el `Bundle`/`OperationOutcome` con el
/// identificador del RDA que asignó el mecanismo. Se entrega crudo porque
/// DÓNDE vive el id dentro del recurso lo fija el IG, que aún no está.
#[derive(Debug, Clone, PartialEq)]
pub struct RespuestaEnvio {
    pub cuerpo: serde_json::Value,
}

/// Todo lo que puede salir mal al hablar con el mecanismo.
#[derive(Debug, thiserror::Error)]
pub enum ErrorIhce {
    #[error("fallo de red hablando con IHCE: {0}")]
    Red(reqwest::Error),
    #[error("no se pudo serializar el bundle: {0}")]
    Serializacion(serde_json::Error),
    #[error("no se pudo autenticar contra Azure AD (HTTP {0})")]
    AuthFallida(u16),
    /// 400: el bundle no cumple la estructura. El `OperationOutcome` dice qué campo.
    #[error("el mecanismo rechazó el RDA por estructura (400)")]
    Estructura(OperationOutcome),
    /// 401/403: token inválido o sin permiso para la operación.
    #[error("el mecanismo rechazó la autorización (HTTP {0})")]
    AutorizacionRechazada(u16),
    /// 409: ya existe un RDA con el mismo Encounter (subject+period+serviceProvider+participant).
    #[error("el mecanismo reporta un RDA duplicado (409)")]
    Duplicado(OperationOutcome),
    #[error("respuesta inesperada del mecanismo (HTTP {status})")]
    Inesperado { status: u16, cuerpo: String },
}

#[derive(Deserialize)]
struct RespuestaToken {
    access_token: String,
    expires_in: u64,
}

struct TokenEnCache {
    valor: String,
    /// Instante a partir del cual hay que volver a pedir uno (ya con el margen
    /// descontado): se compara con `Instant::now()`, no con un reloj de pared,
    /// para que un ajuste de hora no lo dé por vivo ni por muerto.
    expira: Instant,
}

/// El cliente del mecanismo IHCE. Cachea el token y reutiliza la conexión.
pub struct ClienteIhce {
    http: reqwest::Client,
    cfg: Config,
    token: Mutex<Option<TokenEnCache>>,
}

impl ClienteIhce {
    pub fn nuevo(cfg: Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            cfg,
            token: Mutex::new(None),
        }
    }

    /// Envía un RDA ya ensamblado. Consigue el token (cacheado), pone los tres
    /// encabezados y traduce el código HTTP a un resultado o a un [`ErrorIhce`].
    pub async fn enviar_rda(
        &self,
        operacion: OperacionRda,
        bundle: &serde_json::Value,
    ) -> Result<RespuestaEnvio, ErrorIhce> {
        let token = self.token().await?;
        let url = format!("{}/Composition/{}", self.cfg.api_base, operacion.ruta());
        // El cuerpo se serializa a mano y el `Content-Type` se pone explícito:
        // `.json()` de reqwest estamparía `application/json`, y el mecanismo
        // exige `application/fhir+json`.
        let cuerpo = serde_json::to_vec(bundle).map_err(ErrorIhce::Serializacion)?;
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Ocp-Apim-Subscription-Key", &self.cfg.subscription_key)
            .header("Content-Type", "application/fhir+json")
            .body(cuerpo)
            .send()
            .await
            .map_err(ErrorIhce::Red)?;
        self.interpretar(resp).await
    }

    async fn interpretar(&self, resp: reqwest::Response) -> Result<RespuestaEnvio, ErrorIhce> {
        let status = resp.status().as_u16();
        let cuerpo = resp.text().await.map_err(ErrorIhce::Red)?;
        match status {
            200 | 201 => Ok(RespuestaEnvio {
                cuerpo: serde_json::from_str(&cuerpo).unwrap_or(serde_json::Value::Null),
            }),
            400 => Err(ErrorIhce::Estructura(OperationOutcome::desde(&cuerpo))),
            401 | 403 => Err(ErrorIhce::AutorizacionRechazada(status)),
            409 => Err(ErrorIhce::Duplicado(OperationOutcome::desde(&cuerpo))),
            otro => Err(ErrorIhce::Inesperado {
                status: otro,
                cuerpo,
            }),
        }
    }

    /// El token vigente, del caché si sirve o pedido de nuevo si no. El lock NO
    /// se sostiene a través del `await` de la petición: se lee/clona y se suelta,
    /// se pide fuera del lock, y se vuelve a tomar para guardar.
    async fn token(&self) -> Result<String, ErrorIhce> {
        if let Some(v) = self.token_del_cache() {
            return Ok(v);
        }
        let (valor, expires_in) = self.pedir_token().await?;
        let expira = Instant::now() + Duration::from_secs(expires_in.saturating_sub(MARGEN_SEGUNDOS));
        *self.token.lock().unwrap() = Some(TokenEnCache {
            valor: valor.clone(),
            expira,
        });
        Ok(valor)
    }

    fn token_del_cache(&self) -> Option<String> {
        let g = self.token.lock().unwrap();
        match &*g {
            Some(t) if t.expira > Instant::now() => Some(t.valor.clone()),
            _ => None,
        }
    }

    async fn pedir_token(&self) -> Result<(String, u64), ErrorIhce> {
        let resp = self
            .http
            .post(&self.cfg.token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.cfg.client_id.as_str()),
                ("client_secret", self.cfg.client_secret.as_str()),
                ("scope", self.cfg.scope.as_str()),
            ])
            .send()
            .await
            .map_err(ErrorIhce::Red)?;
        if !resp.status().is_success() {
            return Err(ErrorIhce::AuthFallida(resp.status().as_u16()));
        }
        let tok: RespuestaToken = resp.json().await.map_err(ErrorIhce::Red)?;
        Ok((tok.access_token, tok.expires_in))
    }
}

#[cfg(test)]
mod pruebas {
    //! Las pruebas CRUZAN LA FRONTERA (directiva 28): levantan un servidor
    //! `axum` real en un puerto efímero y le hablan por HTTP con el cliente
    //! `reqwest` real. Un `assert` de estructura a estructura mediría el mapeo,
    //! no el transporte —los encabezados en el cable, el código HTTP, el token
    //! cacheado—, que es justo lo único que este crate aporta.

    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::{Json, Router};

    struct EstadoMock {
        hits_token: AtomicUsize,
        token_status: u16,
        expires_in: u64,
        status_envio: u16,
        cuerpo_envio: String,
        auth: Mutex<Option<String>>,
        subkey: Mutex<Option<String>>,
        ctype: Mutex<Option<String>>,
        ruta_envio: Mutex<Option<String>>,
    }

    impl EstadoMock {
        fn nuevo(expires_in: u64, status_envio: u16, cuerpo_envio: &str) -> Self {
            Self {
                hits_token: AtomicUsize::new(0),
                token_status: 200,
                expires_in,
                status_envio,
                cuerpo_envio: cuerpo_envio.to_string(),
                auth: Mutex::new(None),
                subkey: Mutex::new(None),
                ctype: Mutex::new(None),
                ruta_envio: Mutex::new(None),
            }
        }
        fn con_token_status(mut self, s: u16) -> Self {
            self.token_status = s;
            self
        }
    }

    async fn token_handler(State(s): State<Arc<EstadoMock>>) -> impl IntoResponse {
        let n = s.hits_token.fetch_add(1, Ordering::SeqCst) + 1;
        (
            StatusCode::from_u16(s.token_status).unwrap(),
            Json(serde_json::json!({
                "access_token": format!("tok-{n}"),
                "token_type": "Bearer",
                "expires_in": s.expires_in,
            })),
        )
    }

    async fn envio_handler(
        State(s): State<Arc<EstadoMock>>,
        Path(op): Path<String>,
        headers: HeaderMap,
        _body: String,
    ) -> impl IntoResponse {
        let cap = |h: &str| {
            headers
                .get(h)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        *s.auth.lock().unwrap() = cap("authorization");
        *s.subkey.lock().unwrap() = cap("ocp-apim-subscription-key");
        *s.ctype.lock().unwrap() = cap("content-type");
        *s.ruta_envio.lock().unwrap() = Some(op);
        (
            StatusCode::from_u16(s.status_envio).unwrap(),
            s.cuerpo_envio.clone(),
        )
    }

    async fn arrancar(estado: Arc<EstadoMock>) -> String {
        let app = Router::new()
            .route("/tenant/oauth2/v2.0/token", post(token_handler))
            .route("/Composition/{op}", post(envio_handler))
            .with_state(estado);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    fn config(base: &str) -> Config {
        Config {
            token_url: format!("{base}/tenant/oauth2/v2.0/token"),
            api_base: base.to_string(),
            client_id: "cid".into(),
            client_secret: "secret".into(),
            scope: "api://rda/.default".into(),
            subscription_key: "sub-key-123".into(),
        }
    }

    fn bundle() -> serde_json::Value {
        serde_json::json!({ "resourceType": "Bundle", "type": "document" })
    }

    /// El nombre corto va y vuelve por las cuatro operaciones (lo que persiste la
    /// cola). La pareja es la segunda mitad: lo que NO es una operación devuelve
    /// `None`, no una por descarte (directiva 20).
    #[test]
    fn el_nombre_de_la_operacion_va_y_vuelve() {
        for op in [
            OperacionRda::Paciente,
            OperacionRda::Hospitalizacion,
            OperacionRda::ConsultaExterna,
            OperacionRda::Urgencias,
        ] {
            assert_eq!(OperacionRda::desde_nombre(op.nombre()), Some(op));
        }
        for malo in ["", "paciente ", "PACIENTE", "$enviar-rda-paciente", "otra"] {
            assert_eq!(OperacionRda::desde_nombre(malo), None, "{malo:?} coló");
        }
    }

    #[tokio::test]
    async fn envia_y_devuelve_200_con_el_id_del_rda() {
        let estado = Arc::new(EstadoMock::nuevo(
            3599,
            200,
            r#"{"resourceType":"Bundle","id":"RDA-123"}"#,
        ));
        let base = arrancar(estado.clone()).await;
        let cli = ClienteIhce::nuevo(config(&base));

        let r = cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap();
        assert_eq!(r.cuerpo["id"], "RDA-123");
        // Cruzó la frontera hacia la operación correcta, no otra.
        assert_eq!(
            estado.ruta_envio.lock().unwrap().as_deref(),
            Some("$enviar-rda-paciente")
        );
    }

    /// Cada RDA va a SU operación: la de hospitalización no cae en la de paciente.
    #[tokio::test]
    async fn cada_rda_va_a_su_propia_operacion() {
        // LAS CUATRO, no una. Hasta el 2026-08-14 esta prueba ejercitaba solo
        // `Hospitalizacion`, así que `ConsultaExterna` —la única cuyo nombre y
        // cuya ruta NO coinciden— nunca cruzaba la frontera. Un 404 en el 25 %
        // de los envíos no lo habría visto nadie.
        //
        // Y las rutas esperadas se leen del ARTEFACTO vendorizado, no de
        // literales escritos aquí: `datos/ihce/OPERACIONES.txt` es el extracto
        // de la colección Postman del sandbox. Con literales, esta prueba
        // afirmaría lo mismo que el código y las dos podrían estar equivocadas
        // a la vez.
        const OPERACIONES: &str =
            include_str!("../../medico-dominio/datos/ihce/OPERACIONES.txt");

        fn ruta_oficial(nombre: &str) -> String {
            let l = OPERACIONES
                .lines()
                .find(|l| l.starts_with(&format!("{nombre} |")))
                .unwrap_or_else(|| panic!("{nombre} no está en OPERACIONES.txt"));
            l.rsplit('/').next().unwrap().trim().to_string()
        }

        for (op, nombre) in [
            (OperacionRda::Paciente, "enviar-rda-paciente"),
            (OperacionRda::ConsultaExterna, "enviar-rda-consulta-externa"),
            (OperacionRda::Urgencias, "enviar-rda-urgencias"),
            (OperacionRda::Hospitalizacion, "enviar-rda-hospitalizacion"),
        ] {
            let estado = Arc::new(EstadoMock::nuevo(3599, 200, "{}"));
            let base = arrancar(estado.clone()).await;
            let cli = ClienteIhce::nuevo(config(&base));

            cli.enviar_rda(op, &bundle()).await.unwrap();
            assert_eq!(
                estado.ruta_envio.lock().unwrap().as_deref(),
                Some(ruta_oficial(nombre).as_str()),
                "{op:?} — la ruta tiene que ser la de la colección del sandbox"
            );
        }
    }

    /// EL POSITIVO DE CONTROL del arnés de arriba: si `OPERACIONES.txt` no se
    /// cargara o cambiara de forma, `ruta_oficial` devolvería basura y la prueba
    /// mediría basura contra basura. Esto exige que el artefacto diga lo que
    /// creemos que dice — incluida la discrepancia que motivó todo.
    #[test]
    fn el_artefacto_confirma_que_el_nombre_y_la_ruta_no_coinciden() {
        const OPERACIONES: &str =
            include_str!("../../medico-dominio/datos/ihce/OPERACIONES.txt");
        let linea = OPERACIONES
            .lines()
            .find(|l| l.starts_with("enviar-rda-consulta-externa |"))
            .expect("la operación de consulta externa tiene que estar");
        assert!(
            linea.ends_with("$enviar-rda-consulta"),
            "el nombre lleva `-externa` y la ruta NO; si esto cambia, revisa \
             `ruta()` y TRANSPORTE.md: {linea}"
        );
    }

    /// LA PAREJA del envío exitoso: un 400 es error de estructura, y el
    /// `OperationOutcome` conserva el diagnóstico que dice qué arreglar.
    #[tokio::test]
    async fn un_400_es_error_de_estructura_con_su_diagnostico() {
        let oo = r#"{"resourceType":"OperationOutcome","issue":[{"severity":"error","code":"structure","diagnostics":"falta Composition"}]}"#;
        let estado = Arc::new(EstadoMock::nuevo(3599, 400, oo));
        let base = arrancar(estado).await;
        let cli = ClienteIhce::nuevo(config(&base));

        match cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap_err() {
            ErrorIhce::Estructura(oo) => assert_eq!(oo.diagnostico(), "falta Composition"),
            otro => panic!("se esperaba Estructura, vino {otro:?}"),
        }
    }

    #[tokio::test]
    async fn el_409_es_duplicado() {
        let oo = r#"{"issue":[{"diagnostics":"Encounter repetido"}]}"#;
        let estado = Arc::new(EstadoMock::nuevo(3599, 409, oo));
        let base = arrancar(estado).await;
        let cli = ClienteIhce::nuevo(config(&base));

        match cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap_err() {
            ErrorIhce::Duplicado(oo) => assert_eq!(oo.diagnostico(), "Encounter repetido"),
            otro => panic!("se esperaba Duplicado, vino {otro:?}"),
        }
    }

    #[tokio::test]
    async fn el_403_es_autorizacion_rechazada() {
        let estado = Arc::new(EstadoMock::nuevo(3599, 403, ""));
        let base = arrancar(estado).await;
        let cli = ClienteIhce::nuevo(config(&base));

        match cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap_err() {
            ErrorIhce::AutorizacionRechazada(403) => {}
            otro => panic!("se esperaba AutorizacionRechazada(403), vino {otro:?}"),
        }
    }

    /// Un cuerpo de error que NO es un OperationOutcome no revienta ni se pierde:
    /// el texto crudo queda en el diagnóstico (directiva 20).
    #[tokio::test]
    async fn un_400_con_cuerpo_ilegible_conserva_el_texto_crudo() {
        let estado = Arc::new(EstadoMock::nuevo(3599, 400, "502 Bad Gateway (HTML de un proxy)"));
        let base = arrancar(estado).await;
        let cli = ClienteIhce::nuevo(config(&base));

        match cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap_err() {
            ErrorIhce::Estructura(oo) => {
                assert_eq!(oo.diagnostico(), "502 Bad Gateway (HTML de un proxy)")
            }
            otro => panic!("se esperaba Estructura, vino {otro:?}"),
        }
    }

    #[tokio::test]
    async fn los_tres_encabezados_viajan_en_el_envio() {
        let estado = Arc::new(EstadoMock::nuevo(3599, 200, "{}"));
        let base = arrancar(estado.clone()).await;
        let cli = ClienteIhce::nuevo(config(&base));

        cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap();
        assert_eq!(estado.auth.lock().unwrap().as_deref(), Some("Bearer tok-1"));
        assert_eq!(estado.subkey.lock().unwrap().as_deref(), Some("sub-key-123"));
        assert_eq!(
            estado.ctype.lock().unwrap().as_deref(),
            Some("application/fhir+json")
        );
    }

    /// El token se pide UNA vez y se reutiliza mientras esté vigente.
    #[tokio::test]
    async fn el_token_se_cachea_entre_envios() {
        let estado = Arc::new(EstadoMock::nuevo(3599, 200, "{}"));
        let base = arrancar(estado.clone()).await;
        let cli = ClienteIhce::nuevo(config(&base));

        cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap();
        cli.enviar_rda(OperacionRda::Hospitalizacion, &bundle()).await.unwrap();
        assert_eq!(
            estado.hits_token.load(Ordering::SeqCst),
            1,
            "pidió el token de nuevo teniendo uno vigente"
        );
    }

    /// LA PAREJA del caché: un token vencido (expires_in = 0, ya sin margen) se
    /// vuelve a pedir. Sin esto, un caché que nunca refresca pasaría igual.
    #[tokio::test]
    async fn un_token_vencido_se_vuelve_a_pedir() {
        let estado = Arc::new(EstadoMock::nuevo(0, 200, "{}"));
        let base = arrancar(estado.clone()).await;
        let cli = ClienteIhce::nuevo(config(&base));

        cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap();
        cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap();
        assert_eq!(
            estado.hits_token.load(Ordering::SeqCst),
            2,
            "reutilizó un token ya vencido"
        );
    }

    #[tokio::test]
    async fn una_credencial_mala_del_token_es_auth_fallida() {
        let estado = Arc::new(EstadoMock::nuevo(3599, 200, "{}").con_token_status(401));
        let base = arrancar(estado).await;
        let cli = ClienteIhce::nuevo(config(&base));

        match cli.enviar_rda(OperacionRda::Paciente, &bundle()).await.unwrap_err() {
            ErrorIhce::AuthFallida(401) => {}
            otro => panic!("se esperaba AuthFallida(401), vino {otro:?}"),
        }
    }
}

