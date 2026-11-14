//! Cliente del Mecanismo Único de Validación (MUV) de MinSalud.
//!
//! Habla con el contenedor Docker que el PRESTADOR corre en su propia máquina
//! (`https://localhost:9443` por defecto). Sustituye los tres pasos manuales de
//! hoy: subir el archivo a la aplicación de escritorio, esperar la validación y
//! copiar el CUV a mano.
//!
//! ```text
//! login  ->  cargar  ->  CUV
//! ```
//!
//! Portado de `backend/muv_client.py` el 2026-08-10, al verificar #274: el
//! binario GENERABA el FEV-RIPS (`medico_dominio::rips::fev`) y no lo SUBÍA, así
//! que la cadena por la que le pagan al prestador terminaba fuera del producto.
//! Las 22 pruebas del banco de Python están portadas una a una.
//!
//! # Las credenciales NO son nuestras
//!
//! MinSalud confirmó (caso 2026403888) que el acceso al MUV está atado a una
//! PERSONA registrada en Mi Seguridad Social y SISPRO y vinculada a la entidad:
//! **no existe credencial de integrador**. Son del prestador, viven en su
//! despliegue y no pueden aparecer en el código, en un log ni en una excepción.
//! Por eso [`Credenciales`] implementa `Debug` **a mano**, censurado — derivarlo
//! habría bastado para que un `tracing::error!("{cfg:?}")` publicara la clave.
//!
//! # Lo que aporta sobre una llamada HTTP pelada
//!
//! Separar NOTIFICACIONES de RECHAZOS. El validador devuelve las dos cosas en la
//! misma lista `ResultadosValidacion`, y desde el 1-jun-2026 (validador 1.4.8.0)
//! trece reglas que antes solo notificaban son causal de rechazo. Confundirlas
//! significa creer que un archivo pasó cuando no pasó. Cada hallazgo trae además
//! `PathFuente` —`usuarios[1].servicios.consultas[0].codConsulta`—, que es lo que
//! permite señalar el campo a corregir en vez de enseñar un volcado.
//!
//! # Un rechazo NO es una avería, y el código HTTP no lo decide
//!
//! Un archivo rechazado viaja con **HTTP 400 y el resultado completo dentro**
//! (observado el 2026-07-26 contra el contenedor de stage). Tratarlo como avería
//! tira justo los hallazgos que se fueron a buscar y le dice al usuario «el
//! contenedor no respondió» cuando respondió perfectamente para decir que el
//! archivo está mal. Lo que separa un resultado de una avería no es el código:
//! es si el cuerpo **es** un resultado, y por eso se exige su forma.
//!
//! # TLS: `rustls`, y el certificado del contenedor se DECLARA
//!
//! El backend es `rustls` —Rust puro, sin depender de la OpenSSL del cliente— y
//! la elección está medida y escrita en el `Cargo.toml`. Con él entraron dos
//! licencias a `deny.toml` (ISC y CDLA-Permissive-2.0), con su motivo.
//!
//! El contenedor se sirve por HTTPS con un certificado **autofirmado**, así que
//! la verificación normal falla contra `localhost`. Hay dos salidas honestas y el
//! despliegue elige una — [`Tls`] las hace explícitas, y no hay defecto: pasar
//! [`Tls::SinVerificar`] tiene que escribirse.
//!
//! Referencia: MANUAL DE CONSUMO API-DOCKER-FEV-RIPS V 4.3 (FEVRM001 v2.0).

use std::io::Write as _;
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;

pub const RUTA_LOGIN: &str = "/api/Auth/LoginSISPRO";
pub const RUTA_CARGAR_SIN_FACTURA: &str = "/api/PaquetesFevRips/CargarRipsSinFactura";
pub const RUTA_CARGAR_FEV: &str = "/api/PaquetesFevRips/CargarFevRips";
pub const RUTA_CONSULTAR_CUV: &str = "/api/ConsultasFevRips/ConsultarCUV";

/// `Clase` de un hallazgo que NO impide obtener el CUV.
const CLASE_NOTIFICACION: &str = "NOTIFICACION";

/// Cuál de los dos métodos de carga se usa.
///
/// Es un enum y no la ruta en crudo por lo mismo que `OperacionRda::nombre` en
/// `medico-ihce`: una cadena admite una errata que el compilador no ve, y aquí
/// una errata en la ruta es un 404 que se leería como «el MUV no respondió».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// RIPS sin factura — profesional independiente (< 3 500 UVT).
    SinFactura,
    /// FEV en salud — IPS, clínica, hospital.
    Fev,
}

impl Destino {
    pub fn ruta(self) -> &'static str {
        match self {
            Destino::SinFactura => RUTA_CARGAR_SIN_FACTURA,
            Destino::Fev => RUTA_CARGAR_FEV,
        }
    }
}

/// Qué se hace con el certificado del contenedor.
///
/// Es un parámetro OBLIGATORIO del constructor, no un `Option` con defecto: las
/// tres posturas tienen consecuencias distintas y elegir por omisión es cómo se
/// llega a producción sin verificar nada (directiva 20).
#[derive(Debug, Clone)]
pub enum Tls {
    /// Verifica contra el juego de CAs raíz **empotrado en el binario**
    /// (`webpki-roots`), no contra el almacén del sistema operativo: la feature
    /// es `rustls-tls`, no `rustls-tls-native-roots`.
    ///
    /// La diferencia importa y por eso está escrita: una CA **interna** instalada
    /// en la máquina del prestador NO se confiaría por aquí, y el envío fallaría
    /// al conectar. Falla cerrado, que es el lado correcto, pero quien tenga ese
    /// caso necesita `Certificado` — no le sirve esta variante por parecerse su
    /// nombre a lo que quiere.
    DelSistema,
    /// Verifica contra el certificado del propio contenedor, en PEM. Es lo
    /// correcto para el autofirmado que trae de fábrica: sigue habiendo
    /// verificación, solo que contra el certificado que se espera.
    Certificado(Vec<u8>),
    /// NO verifica. Solo es defendible porque la conexión **no sale de la
    /// máquina**: el contenedor corre en el mismo equipo y no hay red por donde
    /// interceptarla. Si algún día se mueve a otro host, deja de serlo — y por
    /// eso hay que escribirlo, no heredarlo.
    SinVerificar,
}

/// Credenciales SISPRO **del prestador**. No se registran nunca.
#[derive(Clone)]
pub struct Credenciales {
    pub tipo_documento: String,
    pub numero_documento: String,
    pub clave: String,
    pub nit: String,
    /// Solo para profesionales independientes; una IPS no lo informa. Los cuatro
    /// valores de la sección 8.1 «LoginSispro» del manual, y lo que los separa:
    ///
    /// - `RE`   no es facturador electrónico (sea cual sea su NIT)
    /// - `PIN`  factura electrónicamente y el NIT COINCIDE con el documento
    /// - `PINx` factura electrónicamente y el NIT de la DIAN NO coincide
    /// - `PIE`  documento distinto de cédula de ciudadanía (extranjero)
    ///
    /// El discriminante primero es si factura electrónicamente, y solo después
    /// la igualdad NIT/documento: quien lea «Nacional» contra «de Excepción» sin
    /// la tabla delante elige por el adjetivo y se equivoca.
    ///
    /// No confundirlo con el `tipoUsuario` del RIPS («01».«12», tipo de
    /// afiliado): mismo nombre, documento distinto, valores sin relación.
    pub tipo_usuario: Option<String>,
}

/// `Debug` **escrito a mano y censurado**, no derivado. Es la única defensa real:
/// con `#[derive(Debug)]`, un solo `{:?}` en un log publica la clave del
/// prestador, y eso no se deshace.
impl std::fmt::Debug for Credenciales {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Credenciales(tipo_documento={:?}, numero_documento='***', clave='***', nit='***')",
            self.tipo_documento
        )
    }
}

impl Credenciales {
    /// El cuerpo del login, con la forma anidada que pide el manual (§8.1).
    pub fn cuerpo_login(&self) -> serde_json::Value {
        let mut cuerpo = serde_json::json!({
            "persona": {
                "identificacion": {
                    "tipo": self.tipo_documento,
                    "numero": self.numero_documento,
                }
            },
            "clave": self.clave,
            "nit": self.nit,
        });
        // Una IPS NO informa `tipoUsuario`: mandarlo vacío no es lo mismo que no
        // mandarlo, y el manual solo lo pide al independiente (directiva 20).
        if let Some(t) = &self.tipo_usuario {
            cuerpo["tipoUsuario"] = serde_json::Value::String(t.clone());
        }
        cuerpo
    }
}

/// Un resultado de validación, con la ruta exacta del campo señalado.
///
/// `Serialize` además de `Deserialize` porque el consumidor GUARDA los hallazgos
/// tal como vinieron —es lo que permite volver a mirar qué se rechazó sin
/// reenviar—, y con los mismos nombres del validador: releerlos de la base tiene
/// que dar exactamente el mismo tipo que releerlos de la respuesta.
#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
pub struct Hallazgo {
    #[serde(rename = "Clase", default)]
    pub clase: String,
    #[serde(rename = "Codigo", default)]
    pub codigo: String,
    #[serde(rename = "Descripcion", default)]
    pub descripcion: String,
    #[serde(rename = "Observaciones", default, deserialize_with = "texto")]
    pub observaciones: String,
    #[serde(rename = "PathFuente", default, deserialize_with = "texto")]
    pub path_fuente: String,
    #[serde(rename = "Fuente", default, deserialize_with = "texto")]
    pub fuente: String,
}

/// Un `null` en estos campos es «no vino», no un error: el MUV los manda nulos
/// con frecuencia. Se leen como cadena vacía en vez de reventar el hallazgo
/// entero, que es lo que se fue a buscar.
fn texto<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

impl Hallazgo {
    pub fn es_notificacion(&self) -> bool {
        self.clase.to_uppercase() == CLASE_NOTIFICACION
    }

    /// Todo lo que no sea notificación bloquea.
    ///
    /// Se decide por EXCLUSIÓN a propósito: si mañana aparece una clase nueva,
    /// conviene tratarla como bloqueante y equivocarse hacia el lado seguro,
    /// antes que darla por inocua y creer que un archivo pasó cuando no.
    pub fn impide_el_cuv(&self) -> bool {
        !self.es_notificacion()
    }
}

/// Lo que devuelve el MUV tras procesar un paquete.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoCarga {
    pub aceptado: bool,
    pub cuv: Option<String>,
    pub proceso_id: Option<i64>,
    pub fecha_radicacion: Option<String>,
    pub ambiente: Option<String>,
    pub hallazgos: Vec<Hallazgo>,
    /// La respuesta entera, para no perder lo que este tipo todavía no modela.
    pub crudo: serde_json::Value,
}

impl ResultadoCarga {
    /// Avisos que NO impidieron el CUV. Conviene revisarlos igual: desde el
    /// 1-jun-2026 varias reglas de este grupo pasaron a causal de rechazo.
    pub fn notificaciones(&self) -> Vec<&Hallazgo> {
        self.hallazgos.iter().filter(|h| h.es_notificacion()).collect()
    }

    pub fn bloqueantes(&self) -> Vec<&Hallazgo> {
        self.hallazgos.iter().filter(|h| h.impide_el_cuv()).collect()
    }

    /// Hallazgos agrupados por la ruta del campo que los produjo. Es lo que
    /// permite marcar el campo en pantalla en vez de enseñar una lista plana.
    ///
    /// Los que vienen SIN `PathFuente` se quedan fuera a propósito: no señalan
    /// un campo, y meterlos bajo una clave vacía inventaría un campo que no
    /// existe. Se siguen viendo en `hallazgos`.
    pub fn campos_a_corregir(&self) -> std::collections::BTreeMap<&str, Vec<&Hallazgo>> {
        let mut por_campo: std::collections::BTreeMap<&str, Vec<&Hallazgo>> = Default::default();
        for h in &self.hallazgos {
            if !h.path_fuente.is_empty() {
                por_campo.entry(h.path_fuente.as_str()).or_default().push(h);
            }
        }
        por_campo
    }
}

/// ¿El destino está en ESTA máquina?
///
/// Es la premisa que hace defendibles las dos posturas flojas —hablar sin TLS y
/// no verificar el certificado—: si la conexión no sale del equipo, no hay red
/// por donde interceptarla. Hasta el 2026-08-10 esa premisa vivía en un
/// comentario y no la comprobaba nadie (lo dijo la revisión de seguridad).
///
/// Se parsea la URL de verdad, con `reqwest::Url`, y no a mano: en
/// `http://localhost@validador.ajeno.com/` el host es **el de la derecha**, y un
/// `starts_with("http://localhost")` lo daría por local.
fn destino_en_esta_maquina(url: &reqwest::Url) -> bool {
    // Se mira el HOST ya parseado, no la cadena. `host_str` devuelve el host y
    // solo el host: el `usuario@` y el puerto quedan fuera.
    match url.host_str() {
        Some(h) if h.eq_ignore_ascii_case("localhost") => true,
        Some(h) => {
            // Un IPv6 llega entre corchetes.
            let h = h.trim_start_matches('[').trim_end_matches(']');
            h.parse::<std::net::IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
        }
        None => false,
    }
}

/// Todo lo que puede salir mal hablando con el MUV.
///
/// Las tres primeras variantes no son matices: cada una manda a una persona
/// distinta a hacer una cosa distinta. Colapsarlas en «error del MUV» pondría a
/// revisar el archivo a quien tiene el contenedor apagado.
#[derive(Debug, thiserror::Error)]
pub enum ErrorMuv {
    /// El contenedor no respondió, o respondió algo ininteligible. Es
    /// RECUPERABLE: reintentar más tarde puede funcionar, y **no** significa que
    /// el archivo esté mal.
    #[error("no se pudo contactar el MUV: {0}")]
    NoDisponible(String),
    /// SISPRO rechazó las credenciales del prestador. No es un problema del
    /// archivo ni de la red: alguien debe revisar el registro en Mi Seguridad
    /// Social o la vinculación a la entidad.
    #[error(
        "SISPRO rechazó las credenciales del prestador (HTTP {0}). Revisar el registro en \
         Mi Seguridad Social y la vinculación a la entidad; no es un problema del archivo."
    )]
    CredencialesInvalidas(u16),
    /// Se intentó cargar sin haber hecho `login` antes.
    #[error("hay que autenticarse antes de cargar")]
    SinToken,
    #[error("no se pudo serializar el cuerpo: {0}")]
    Serializacion(#[from] serde_json::Error),
    /// La configuración pide una postura que solo vale dentro de la máquina, y
    /// el destino NO está dentro de la máquina. No es un fallo de red: es que la
    /// premisa escrita no se cumple, y seguir adelante mandaría las credenciales
    /// del prestador y la historia clínica del mes por una red ajena.
    #[error("{0}")]
    ConfiguracionInsegura(String),
}

/// El cliente del contenedor MUV del prestador.
pub struct ClienteMuv {
    http: reqwest::Client,
    base_url: String,
    token: Mutex<Option<String>>,
}

impl ClienteMuv {
    /// `base_url` es dónde escucha el contenedor; el puerto lo asigna el
    /// despliegue y el manual usa 9443. `timeout` va por petición: validar un
    /// paquete grande tarda, y el defecto de `reqwest` es no tener ninguno.
    ///
    /// Una `base_url` vacía es un error y no un defecto: apuntar a `localhost`
    /// por nuestra cuenta mandaría el dato clínico del prestador a donde nadie
    /// pidió (directiva 20).
    pub fn nuevo(base_url: &str, timeout: Duration, tls: Tls) -> Result<Self, ErrorMuv> {
        if base_url.trim().is_empty() {
            return Err(ErrorMuv::NoDisponible("base_url es obligatorio".into()));
        }
        // El destino se parsea ANTES de construir nada: las dos posturas flojas
        // solo valen si no se sale de la máquina, y eso hay que comprobarlo, no
        // confiarlo a que quien escribe el `.env` haya leído el comentario.
        let url = reqwest::Url::parse(base_url)
            .map_err(|e| ErrorMuv::NoDisponible(format!("base_url ilegible: {e}")))?;
        let local = destino_en_esta_maquina(&url);
        if url.scheme() == "http" && !local {
            return Err(ErrorMuv::ConfiguracionInsegura(format!(
                "MEDICO_MUV_URL apunta a {} SIN TLS. Por ahí viajan la clave del prestador y \
                 la historia clínica del periodo: en claro solo se admite contra esta misma \
                 máquina. Use https, o el contenedor local.",
                url.host_str().unwrap_or("(sin host)")
            )));
        }
        if matches!(tls, Tls::SinVerificar) && !local {
            return Err(ErrorMuv::ConfiguracionInsegura(format!(
                "MEDICO_MUV_TLS_INSEGURO=true con el validador en {}, que no es esta máquina. \
                 No verificar el certificado solo es defendible cuando la conexión no sale del \
                 equipo; contra otro host es un canal interceptable. Use MEDICO_MUV_CA.",
                url.host_str().unwrap_or("(sin host)")
            )));
        }

        let mut b = reqwest::Client::builder().timeout(timeout);
        b = match tls {
            Tls::DelSistema => b,
            Tls::Certificado(pem) => b.add_root_certificate(
                reqwest::Certificate::from_pem(&pem)
                    .map_err(|e| ErrorMuv::NoDisponible(format!("certificado ilegible: {e}")))?,
            ),
            // El nombre del método es el aviso, y se conserva a propósito.
            Tls::SinVerificar => b.danger_accept_invalid_certs(true),
        };
        let http = b
            .build()
            .map_err(|e| ErrorMuv::NoDisponible(e.to_string()))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            token: Mutex::new(None),
        })
    }

    /// Autentica y guarda el token. **No devuelve nada**: el token no sale del
    /// cliente, para que no acabe en un log ni en una plantilla.
    pub async fn login(&self, credenciales: &Credenciales) -> Result<(), ErrorMuv> {
        let respuesta = self
            .post(RUTA_LOGIN, &credenciales.cuerpo_login(), false, false)
            .await?;

        // Cuatro nombres porque el contenedor ha usado varios entre versiones.
        let token = ["token", "Token", "access_token", "accessToken"]
            .iter()
            .find_map(|c| respuesta.get(*c).and_then(|v| v.as_str()))
            .filter(|t| !t.is_empty());

        match token {
            Some(t) => {
                *self.token.lock().unwrap() = Some(t.to_string());
                Ok(())
            }
            // Un 200 sin token es una avería, no un login bueno: seguir adelante
            // daría un 401 en la carga y mandaría a revisar las credenciales.
            None => Err(ErrorMuv::NoDisponible(
                "el MUV no devolvió token en la respuesta de autenticación".into(),
            )),
        }
    }

    /// Envía un paquete y devuelve el resultado de la validación.
    ///
    /// **No devuelve `Err` cuando el archivo es rechazado**: devuelve un
    /// [`ResultadoCarga`] con `aceptado: false` y los hallazgos. Un rechazo es un
    /// resultado normal del proceso —el usuario corrige y reenvía—, no una
    /// avería, y tratarlo como error obliga a todo llamante a desenterrar los
    /// hallazgos de dentro de un error para poder enseñarlos.
    pub async fn cargar(
        &self,
        paquete: &serde_json::Value,
        destino: Destino,
    ) -> Result<ResultadoCarga, ErrorMuv> {
        let respuesta = self.post(destino.ruta(), paquete, true, true).await?;
        Ok(interpretar(respuesta))
    }

    /// Consulta el estado de un documento ya enviado. No requiere token.
    pub async fn consultar_cuv(
        &self,
        consulta: &serde_json::Value,
    ) -> Result<serde_json::Value, ErrorMuv> {
        self.post(RUTA_CONSULTAR_CUV, consulta, false, false).await
    }

    async fn post(
        &self,
        ruta: &str,
        cuerpo: &serde_json::Value,
        con_token: bool,
        comprimir: bool,
    ) -> Result<serde_json::Value, ErrorMuv> {
        let datos = serde_json::to_vec(cuerpo)?;
        let mut peticion = self
            .http
            .post(format!("{}{ruta}", self.base_url))
            .header("Content-Type", "application/json");

        let datos = if comprimir {
            // Obligatorio en los métodos de carga, según el manual.
            peticion = peticion.header("Content-Encoding", "gzip");
            comprimir_gzip(&datos)?
        } else {
            datos
        };

        if con_token {
            let token = self.token.lock().unwrap().clone();
            let Some(token) = token else {
                return Err(ErrorMuv::SinToken);
            };
            peticion = peticion.header("Authorization", format!("Bearer {token}"));
        }

        let resp = match peticion.body(datos).send().await {
            Ok(r) => r,
            // Red, DNS, TLS, timeout: nada de esto dice que el archivo esté mal.
            Err(e) => return Err(ErrorMuv::NoDisponible(e.to_string())),
        };

        let codigo = resp.status().as_u16();
        let texto = resp
            .text()
            .await
            .map_err(|e| ErrorMuv::NoDisponible(e.to_string()))?;

        if codigo == 401 || codigo == 403 {
            return Err(ErrorMuv::CredencialesInvalidas(codigo));
        }

        let json: Option<serde_json::Value> = serde_json::from_str(&texto).ok();

        // Un RECHAZO viaja con HTTP 400 y el resultado completo dentro. Lo que
        // separa un resultado de una avería no es el código HTTP sino si el
        // cuerpo ES un resultado: un 400 con el HTML de un proxy sigue siendo
        // una avería, y por eso se exige la FORMA y no solo que haya JSON.
        if let Some(v) = &json
            && ["ResultState", "ResultadosValidacion", "CodigoUnicoValidacion"]
                .iter()
                .any(|c| v.get(*c).is_some())
        {
            return Ok(v.clone());
        }

        if codigo >= 400 {
            return Err(ErrorMuv::NoDisponible(format!(
                "el MUV respondió HTTP {codigo}: {}",
                recortar(&texto, 300)
            )));
        }
        json.ok_or_else(|| {
            ErrorMuv::NoDisponible(format!(
                "respuesta ininteligible del MUV (HTTP {codigo}): {}",
                recortar(&texto, 200)
            ))
        })
    }
}

/// Recorta por CARACTERES y no por bytes: `&texto[..300]` entra en pánico si el
/// corte cae dentro de un carácter multibyte, y los mensajes del MUV vienen en
/// castellano con tildes.
fn recortar(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn comprimir_gzip(datos: &[u8]) -> Result<Vec<u8>, ErrorMuv> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(datos)
        .and_then(|()| e.finish())
        .map_err(|e| ErrorMuv::NoDisponible(format!("no se pudo comprimir el paquete: {e}")))
}

fn interpretar(respuesta: serde_json::Value) -> ResultadoCarga {
    let hallazgos: Vec<Hallazgo> = respuesta
        .get("ResultadosValidacion")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let cuv = respuesta
        .get("CodigoUnicoValidacion")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // El CUV manda sobre la bandera: si hay código, el archivo pasó. Se exige
    // ADEMÁS que no haya bloqueantes, para no dar por bueno un resultado
    // contradictorio.
    let estado = respuesta
        .get("ResultState")
        .or_else(|| respuesta.get("EsValido"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let hay_bloqueantes = hallazgos.iter().any(Hallazgo::impide_el_cuv);
    let aceptado = cuv.is_some() && estado && !hay_bloqueantes;

    ResultadoCarga {
        aceptado,
        cuv: if aceptado { cuv } else { None },
        proceso_id: respuesta.get("ProcesoId").and_then(|v| v.as_i64()),
        fecha_radicacion: respuesta
            .get("FechaRadicacion")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        ambiente: respuesta
            .get("Ambiente")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        hallazgos,
        crudo: respuesta,
    }
}

#[cfg(test)]
mod pruebas;
