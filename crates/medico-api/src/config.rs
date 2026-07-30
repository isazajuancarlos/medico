//! La configuración se lee una vez y, si falta o es insegura, el binario **no
//! arranca**. No hay valores por defecto para los secretos y no debe haberlos:
//! un token firmado con una clave conocida se falsifica, y con eso todo el
//! aislamiento entre instituciones deja de proteger nada. Es el mismo principio
//! que `backend/core/config.py`: fallar al arrancar, no degradarse.

use std::env::{self, VarError};

#[derive(Debug, thiserror::Error)]
pub enum ErrorConfig {
    #[error(
        "falta SECRET_KEY. No hay valor por defecto a propósito: firma las sesiones, y una \
         clave conocida deja falsificar la de cualquier usuario. Genere una de 32+ bytes."
    )]
    SecretoAusente,
    #[error("SECRET_KEY tiene {0} caracteres; hacen falta al menos {1}")]
    SecretoCorto(usize, usize),
    #[error(
        "MEDICO_ESCUCHA no es una dirección IP válida: «{0}». Ponga una dirección \
         (por ejemplo 127.0.0.1 o 0.0.0.0), no un nombre de máquina ni un puerto."
    )]
    NoEsDireccion(String),
    #[error(
        "SECRET_KEY es un valor conocido e inseguro ({0:?}). Ese es exactamente el camino por \
         el que «changeme» llega a producción. Genere una clave propia."
    )]
    SecretoInseguro(String),
    #[error("{0} no es un número válido: {1}")]
    NoEsNumero(&'static str, String),
}

/// Mínimo del secreto. 32 bytes es la salida de SHA-256: por debajo, la firma es
/// más débil que el algoritmo que la produce.
const MINIMO_SECRETO: usize = 32;

/// Valores que NO pueden ser el secreto: los defectos que la gente deja puestos.
const INSEGUROS: &[&str] = &[
    "changeme", "secret", "secret_key", "clave", "password", "cambiame",
    "SECRET_KEY_FINAL_V6", "dev", "test", "0000000000000000000000000000000000",
];

#[derive(Debug, Clone)]
pub struct Config {
    pub url_bd: String,
    pub puerto: u16,
    pub secreto_sesion: Vec<u8>,
    pub minutos_sesion: i64,
    /// Dónde se guardan las imágenes diagnósticas. La ruta de un archivo la
    /// construye el servidor desde el id; nada que venga del cliente entra en ella.
    pub dir_imagenes: String,
    /// Clave PRIVADA de firma (hex) para sellar la historia al firmarla. `None`
    /// desactiva la firma con gracia: la historia se cierra sin firma criptográfica.
    pub clave_firma: Option<String>,
    /// Clave PÚBLICA de verificación (hex), para comprobar firmas. `None` si no hay.
    pub clave_verificacion: Option<String>,
    /// Frase para cifrar en reposo los archivos (imágenes). `None` los guarda en
    /// claro (histórico / sin cifrado). Es un secreto de despliegue.
    pub clave_reposo: Option<String>,
    /// `MEDICO_TRAS_PROXY=true` cuando hay nginx delante. Ver `Estado::tras_proxy`.
    pub tras_proxy: bool,
    /// En qué dirección escucha el servidor. Ver [`direccion_de_escucha`].
    pub escucha: std::net::IpAddr,
}

fn opcional(clave: &str) -> Option<String> {
    match env::var(clave) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        Ok(_) | Err(VarError::NotPresent) | Err(VarError::NotUnicode(_)) => None,
    }
}

fn numero<T: std::str::FromStr>(clave: &'static str, defecto: T) -> Result<T, ErrorConfig> {
    match opcional(clave) {
        None => Ok(defecto),
        Some(v) => v.trim().parse().map_err(|_| ErrorConfig::NoEsNumero(clave, v)),
    }
}

/// Un secreto sirve si es largo y no es un valor conocido. Función pura para
/// poder probarla sin tocar el entorno (que es global y hace las pruebas
/// dependientes del orden).
fn validar_secreto(s: &str) -> Result<(), ErrorConfig> {
    if s.len() < MINIMO_SECRETO {
        return Err(ErrorConfig::SecretoCorto(s.len(), MINIMO_SECRETO));
    }
    if INSEGUROS.iter().any(|m| m.eq_ignore_ascii_case(s.trim())) {
        return Err(ErrorConfig::SecretoInseguro(s.to_string()));
    }
    Ok(())
}

/// La dirección en la que liga el servidor, en cascada.
///
/// | `MEDICO_ESCUCHA` | `tras_proxy` | resultado |
/// |---|---|---|
/// | puesta | cualquiera | lo que diga el operador |
/// | ausente | sí | `127.0.0.1` |
/// | ausente | no | `0.0.0.0` |
///
/// # Por qué el defecto depende de la otra bandera
///
/// Declarar `MEDICO_TRAS_PROXY=true` es decir «a mí me habla el proxy». Si además
/// el binario sigue escuchando en `0.0.0.0`, cualquiera que alcance el puerto se
/// salta el proxy — y entonces **elige su propio `X-Real-IP`**, con lo que el
/// freno del login no engancha nunca: basta rotar la cabecera en cada intento.
/// El control existiría y no frenaría, que es exactamente lo que ya pasaba aquí
/// mientras se leía el primer salto de `X-Forwarded-For`.
///
/// # Y por qué es un defecto y no una regla
///
/// Cerrarlo a `127.0.0.1` sin salida rompería una topología legítima: nginx en
/// otra máquina, o un contenedor cuyo proxy vive fuera de su red de bucle —y
/// `docker-compose.yml` levanta justo eso—. Ahí el operador pone
/// `MEDICO_ESCUCHA=0.0.0.0` y lo dice a propósito. Se cierra el descuido, no el
/// caso.
pub fn direccion_de_escucha(
    valor: Option<&str>,
    tras_proxy: bool,
) -> Result<std::net::IpAddr, ErrorConfig> {
    match valor.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => v.parse().map_err(|_| ErrorConfig::NoEsDireccion(v.to_string())),
        None if tras_proxy => Ok(std::net::IpAddr::from([127, 0, 0, 1])),
        None => Ok(std::net::IpAddr::from([0, 0, 0, 0])),
    }
}

impl Config {
    pub fn desde_entorno() -> Result<Self, ErrorConfig> {
        let secreto = opcional("SECRET_KEY").ok_or(ErrorConfig::SecretoAusente)?;
        validar_secreto(&secreto)?;

        // Solo `true` explícito. Cualquier otra cosa —vacío, «1», «si», un
        // dedazo— vale `false`, que es el lado seguro. Se lee antes que el
        // resto porque el defecto de `escucha` depende de él.
        let tras_proxy = opcional("MEDICO_TRAS_PROXY")
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Ok(Self {
            // La base SÍ tiene defecto: no es un secreto, y el perfil de una sola
            // máquina es el que se entrega instalado.
            url_bd: opcional("DATABASE_URL").unwrap_or_else(|| "sqlite://medico.db".into()),
            puerto: numero("PORT", 8000u16)?,
            secreto_sesion: secreto.into_bytes(),
            minutos_sesion: numero("SESSION_TIMEOUT_MINUTES", 60i64)?,
            // Carpeta local, no un secreto: el binario se despliega con su
            // almacenamiento al lado. El nombre del archivo lo pone el servidor
            // desde el id de la fila, nunca el cliente.
            dir_imagenes: opcional("MEDICO_DIR_IMAGENES").unwrap_or_else(|| "storage/imagenes".into()),
            // Claves de firma: opcionales (sin ellas la historia se cierra sin
            // firma criptográfica). La privada es SECRETA; la pública puede
            // publicarse para que cualquiera verifique.
            clave_firma: opcional("MEDICO_SIGNING_KEY"),
            clave_verificacion: opcional("MEDICO_VERIFYING_KEY"),
            clave_reposo: opcional("MEDICO_STORAGE_KEY"),
            // Solo `true` explícito. Cualquier otra cosa —vacío, «1», «si», un
            // dedazo— vale `false`, y `false` es el lado seguro: se ignora
            // `X-Real-IP` y el origen sale del zócalo. Al revés,
            // un valor mal escrito haría creer una cabecera que escribe quien
            // llama.
            tras_proxy,
            escucha: direccion_de_escucha(opcional("MEDICO_ESCUCHA").as_deref(), tras_proxy)?,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_secreto_corto_no_pasa() {
        assert!(matches!(validar_secreto("corto"), Err(ErrorConfig::SecretoCorto(..))));
    }

    #[test]
    fn un_valor_conocido_no_pasa_aunque_sea_largo() {
        // 34 ceros: pasa el largo, pero es un valor por defecto que se prohíbe.
        assert!(matches!(
            validar_secreto("0000000000000000000000000000000000"),
            Err(ErrorConfig::SecretoInseguro(_))
        ));
    }

    #[test]
    fn un_secreto_propio_y_largo_pasa() {
        assert!(validar_secreto("una-clave-propia-de-mas-de-treinta-y-dos-bytes").is_ok());
    }

    fn ip(s: &str) -> std::net::IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn sin_proxy_declarado_se_escucha_en_todas_las_interfaces() {
        // El perfil de una sola máquina: si esto ligara a loopback por defecto,
        // nadie llegaría al sistema.
        assert_eq!(direccion_de_escucha(None, false).unwrap(), ip("0.0.0.0"));
    }

    #[test]
    fn declarar_el_proxy_deja_de_exponer_el_puerto() {
        // LA prueba de este cambio. Con el proxy declarado y el puerto abierto a
        // todas las interfaces, quien alcance el puerto se salta el proxy y elige
        // su propio `X-Real-IP`: el freno del login no engancharía nunca.
        assert_eq!(direccion_de_escucha(None, true).unwrap(), ip("127.0.0.1"));
    }

    #[test]
    fn el_operador_manda_sobre_el_defecto_en_las_dos_direcciones() {
        // nginx en otra máquina —o en otro contenedor— es legítimo, y cerrarlo
        // sin salida sería rechazar lo inusual en vez de lo imposible.
        assert_eq!(direccion_de_escucha(Some("0.0.0.0"), true).unwrap(), ip("0.0.0.0"));
        assert_eq!(direccion_de_escucha(Some("10.0.0.7"), true).unwrap(), ip("10.0.0.7"));
        assert_eq!(direccion_de_escucha(Some("127.0.0.1"), false).unwrap(), ip("127.0.0.1"));
        assert_eq!(direccion_de_escucha(Some("::1"), false).unwrap(), ip("::1"));
    }

    #[test]
    fn una_direccion_ilegible_no_se_sustituye_por_un_defecto() {
        // Ante un dato roto, fallar. Caer al defecto dejaría al operador creyendo
        // que puso una cosa mientras el servidor hace otra — y en la dirección
        // peligrosa, porque el defecto sin proxy es 0.0.0.0.
        for malo in ["localhost", "127.0.0.1:8000", "no"] {
            assert!(
                matches!(direccion_de_escucha(Some(malo), false), Err(ErrorConfig::NoEsDireccion(_))),
                "«{malo}» pasó por dirección válida"
            );
        }
        // Vacío es «no la puso»: cae al defecto, que es correcto.
        assert!(direccion_de_escucha(Some("   "), false).is_ok());
    }
}
