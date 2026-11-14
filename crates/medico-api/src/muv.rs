//! El cableado entre el FEV-RIPS que genera este binario y el cliente del
//! Mecanismo Único de Validación (`medico_muv`): aquí vive la CONFIGURACIÓN
//! —dónde está el contenedor y con qué credenciales se entra— y nada más.
//!
//! # Dónde viven las credenciales del prestador: en el ENTORNO del proceso
//!
//! No es una decisión nueva: es la que ya tomó `ihce.rs` dos días antes para el
//! mismo problema —credenciales de MinSalud, por prestador—, y tener dos sitios
//! donde buscar el mismo tipo de secreto sería el mecanismo de más que la
//! directiva 1 manda quitar. Concretamente:
//!
//! - **NO van a la base.** `institucion` y `configuracion_institucion` se editan
//!   desde una pantalla de administración, y un secreto que un administrador
//!   puede leer de vuelta deja de ser secreto. Además la base viaja en los
//!   respaldos.
//! - **NO van al repositorio**, obviamente, ni a un valor por defecto: sin las
//!   variables no hay envío, y eso es lo correcto (directiva 20).
//! - **Van al entorno del proceso** —el `.env` del servidor, o una bóveda que lo
//!   alimente—, que es donde ya está `SECRET_KEY` y donde el despliegue las pone
//!   con permisos 0640 root:medico, igual que hoy.
//!
//! **La limitación que eso trae, dicha antes de que sorprenda**: es UNA
//! configuración por PROCESO. El binario es multi-institucional, así que un
//! despliegue que sirva a dos prestadores que facturan por separado necesitaría
//! credenciales por institución, y esto no se lo daría. No se resuelve hoy
//! porque hoy no existe ese despliegue —una base por instancia, directiva 26— y
//! porque la solución (columna cifrada, con su pantalla y su rotación) es un
//! trabajo con decisiones propias. Lo que sí hace `enviar` es **negarse** si la
//! institución que pide el envío no es la que las variables declaran: mejor
//! pararse que facturarle a la EPS con el NIT del vecino.

use medico_muv::{ClienteMuv, Credenciales, Destino, Tls};
use std::time::Duration;

/// Cuánto se espera por petición. Validar un paquete de un mes entero tarda, y
/// el defecto de `reqwest` es no tener ningún tope: una petición colgada dejaría
/// al usuario mirando una pantalla que no vuelve.
const TIMEOUT: Duration = Duration::from_secs(120);

/// Todo lo que hace falta para hablar con el contenedor del prestador.
pub struct Config {
    pub base_url: String,
    pub credenciales: Credenciales,
    pub tls: Tls,
    /// El NIT que las credenciales declaran. Se guarda aparte para poder
    /// comprobar que coincide con el de la institución que pide el envío.
    pub nit: String,
    /// Qué método de carga usa este prestador. Ver [`Destino`].
    pub destino: Destino,
}

/// Lee la configuración de una fuente de variables. Devuelve `Some` **solo si
/// están TODAS** las obligatorias: una configuración a medias no envía a ninguna
/// parte, y media credencial no identifica a nadie (directiva 20). Toma la
/// fuente como parámetro para poder probar el parseo sin tocar el entorno global
/// del proceso — igual que `ihce::config_desde`.
pub fn config_desde<F: Fn(&str) -> Option<String>>(get: F) -> Option<Config> {
    // Una variable **vacía** no es una variable puesta. `std::env::var` devuelve
    // `Ok("")` para `MEDICO_MUV_NIT=`, y sin este filtro esa configuración se
    // daba por completa: el guardián del NIT compararía dos cadenas vacías y
    // dejaría pasar el envío de una institución cuyo NIT tampoco esté escrito.
    // Es la directiva 20 —ausencia de dato, fallo ruidoso— aplicada a la forma
    // en que un `.env` se escribe de verdad: la línea existe y el valor no.
    let get = |k: &str| get(k).filter(|v| !v.trim().is_empty());
    let nit = get("MEDICO_MUV_NIT")?;
    Some(Config {
        base_url: get("MEDICO_MUV_URL")?,
        credenciales: Credenciales {
            tipo_documento: get("MEDICO_MUV_TIPO_DOCUMENTO")?,
            numero_documento: get("MEDICO_MUV_DOCUMENTO")?,
            clave: get("MEDICO_MUV_CLAVE")?,
            nit: nit.clone(),
            // Opcional de verdad: una IPS NO lo informa, y mandarlo vacío no es
            // lo mismo que no mandarlo.
            tipo_usuario: get("MEDICO_MUV_TIPO_USUARIO"),
        },
        tls: tls_desde(&get),
        nit,
        // El defecto NO es relleno: es la ruta que sirve a quien no factura
        // electrónicamente, y la otra exige además el XML de la DIAN aprobado,
        // que este sistema todavía no guarda. Quien facture electrónicamente lo
        // declara.
        destino: match get("MEDICO_MUV_DESTINO").as_deref() {
            Some("fev") => Destino::Fev,
            _ => Destino::SinFactura,
        },
    })
}

/// Cómo se verifica el certificado del contenedor. El **defecto verifica**: la
/// postura insegura hay que escribirla, y se llama por su nombre en el log.
fn tls_desde<F: Fn(&str) -> Option<String>>(get: &F) -> Tls {
    if get("MEDICO_MUV_TLS_INSEGURO").as_deref() == Some("true") {
        return Tls::SinVerificar;
    }
    match get("MEDICO_MUV_CA") {
        Some(pem) => Tls::Certificado(pem.into_bytes()),
        None => Tls::DelSistema,
    }
}

/// La configuración desde el entorno del proceso. `None` = el envío al MUV no
/// está configurado, que es el caso normal de una instalación que todavía sube
/// el archivo a mano.
pub fn config_desde_entorno() -> Option<Config> {
    config_desde(|k| std::env::var(k).ok())
}

/// Construye el cliente y hace el login. Se hace en cada envío a propósito y no
/// se cachea el token: un envío al MUV ocurre una vez al mes por periodo, así
/// que un token cacheado solo serviría para vencer entre usos — que es
/// exactamente el 401 que cuesta un ciclo de validación entenderlo.
pub async fn entrar(cfg: &Config) -> Result<ClienteMuv, medico_muv::ErrorMuv> {
    let cliente = ClienteMuv::nuevo(&cfg.base_url, TIMEOUT, cfg.tls.clone())?;
    cliente.login(&cfg.credenciales).await?;
    Ok(cliente)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::HashMap;

    fn completo() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("MEDICO_MUV_URL", "https://localhost:9443"),
            ("MEDICO_MUV_TIPO_DOCUMENTO", "CC"),
            ("MEDICO_MUV_DOCUMENTO", "1234567890"),
            ("MEDICO_MUV_CLAVE", "secreta"),
            ("MEDICO_MUV_NIT", "900123456"),
        ])
    }

    fn leer(m: &HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> =
            m.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn con_las_cinco_obligatorias_configura() {
        let cfg = config_desde(leer(&completo())).expect("con todas debe configurar");
        assert_eq!(cfg.nit, "900123456");
        assert!(matches!(cfg.destino, Destino::SinFactura));
        assert!(cfg.credenciales.tipo_usuario.is_none());
    }

    #[test]
    fn una_variable_en_blanco_no_cuenta_como_puesta() {
        // `MEDICO_MUV_CLAVE=` en el `.env` es la forma normal de dejar una
        // variable «para rellenar luego», y `env::var` la devuelve como `Ok("")`.
        // Lo señaló la revisión de seguridad del 2026-08-10.
        for clave in completo().keys() {
            let mut m = completo();
            m.insert(clave, "   ");
            assert!(
                config_desde(leer(&m)).is_none(),
                "{clave} en blanco no puede dar por configurado el envío"
            );
        }
    }

    #[test]
    fn a_la_que_le_falte_una_sola_no_configura() {
        // La pareja de la anterior, y la que hace que la comprobación signifique
        // algo: sin ella, un `Some` que ignorara la mitad de las variables
        // pasaría igual. Se prueba quitando CADA una, no una cualquiera.
        for clave in completo().keys() {
            let mut m = completo();
            m.remove(clave);
            assert!(
                config_desde(leer(&m)).is_none(),
                "faltando {clave} NO debería configurar: media credencial no identifica a nadie"
            );
        }
    }

    #[test]
    fn el_defecto_de_tls_verifica() {
        let cfg = config_desde(leer(&completo())).unwrap();
        assert!(
            matches!(cfg.tls, Tls::DelSistema),
            "no verificar tiene que ESCRIBIRSE, nunca heredarse"
        );
    }

    #[test]
    fn la_postura_insegura_hay_que_pedirla_por_su_nombre() {
        let mut m = completo();
        m.insert("MEDICO_MUV_TLS_INSEGURO", "true");
        assert!(matches!(config_desde(leer(&m)).unwrap().tls, Tls::SinVerificar));
    }

    #[test]
    fn cualquier_otro_valor_de_la_bandera_insegura_sigue_verificando() {
        // El caso rojo de la anterior. `"1"`, `"si"` o `"TRUE"` son intentos
        // razonables de encenderla, y NINGUNO la enciende: se elige el lado
        // seguro, y quien la quiera tiene que escribir exactamente `true`.
        for valor in ["1", "si", "TRUE", "yes", ""] {
            let mut m = completo();
            m.insert("MEDICO_MUV_TLS_INSEGURO", valor);
            assert!(
                matches!(config_desde(leer(&m)).unwrap().tls, Tls::DelSistema),
                "{valor:?} no puede apagar la verificación"
            );
        }
    }

    #[test]
    fn el_certificado_del_contenedor_se_toma_si_se_da() {
        let mut m = completo();
        m.insert("MEDICO_MUV_CA", "-----BEGIN CERTIFICATE-----");
        assert!(matches!(config_desde(leer(&m)).unwrap().tls, Tls::Certificado(_)));
    }

    #[test]
    fn la_bandera_insegura_gana_al_certificado() {
        // Que el orden esté fijado importa: con las dos puestas, la intención
        // más explícita es la bandera, y dejarlo indefinido haría que el mismo
        // despliegue verificara o no según el orden del código.
        let mut m = completo();
        m.insert("MEDICO_MUV_CA", "-----BEGIN CERTIFICATE-----");
        m.insert("MEDICO_MUV_TLS_INSEGURO", "true");
        assert!(matches!(config_desde(leer(&m)).unwrap().tls, Tls::SinVerificar));
    }

    #[test]
    fn el_destino_por_defecto_es_el_que_no_exige_la_factura_de_la_dian() {
        let mut m = completo();
        m.insert("MEDICO_MUV_DESTINO", "fev");
        assert!(matches!(config_desde(leer(&m)).unwrap().destino, Destino::Fev));
        // Un valor desconocido NO se convierte en `Fev` por descuido: mandar por
        // la ruta de facturación electrónica un paquete sin XML de la DIAN es un
        // rechazo seguro.
        m.insert("MEDICO_MUV_DESTINO", "loquesea");
        assert!(matches!(config_desde(leer(&m)).unwrap().destino, Destino::SinFactura));
    }

    #[test]
    fn el_independiente_declara_su_tipo_de_usuario() {
        let mut m = completo();
        m.insert("MEDICO_MUV_TIPO_USUARIO", "PIN");
        let cfg = config_desde(leer(&m)).unwrap();
        assert_eq!(cfg.credenciales.tipo_usuario.as_deref(), Some("PIN"));
    }
}
