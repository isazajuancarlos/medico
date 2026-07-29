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

impl Config {
    pub fn desde_entorno() -> Result<Self, ErrorConfig> {
        let secreto = opcional("SECRET_KEY").ok_or(ErrorConfig::SecretoAusente)?;
        validar_secreto(&secreto)?;

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
}
