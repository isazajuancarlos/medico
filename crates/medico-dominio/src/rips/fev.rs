// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **FEV-RIPS: el documento ÚNICO que se envía al Mecanismo Único de Validación.**
//!
//! Port de `backend/fev_rips.py`, y es **la condición del ADR para apagar el
//! Python**: sin FEV-RIPS no hay facturación electrónica, que es por lo que le
//! pagan al prestador. En Rust ya estaban el recolector y los CINCO documentos;
//! lo que faltaba era esto, y era lo único que quedaba fuera del estándar de la
//! directiva 26.
//!
//! # Qué añade sobre los cinco documentos
//!
//! [`super::ac`] … [`super::ah`] producen **seis documentos separados**, cada uno
//! con su propia lista de usuarios: un paciente con consulta y medicamento
//! aparece dos veces. FEV-RIPS es **UN documento** donde cada usuario lleva
//! TODOS sus servicios juntos. Además:
//!
//! 1. el sobre dice qué tipo de documento es —RIPS sin factura o FEV en salud—;
//! 2. `numAutorizacion` y `numFEVPagoModerador` van `null` y no `""`: enviar
//!    cadena vacía donde el esquema declara nulable es causal de rechazo;
//! 3. las fechas van sin segundos, que aquí ya salen así de
//!    [`super::documentos`] — el Python las recorta porque su extracción las
//!    produce con `%S`.
//!
//! # El CT no existe en FEV-RIPS
//!
//! Es un archivo de control del formato plano de la Resolución 3374. Aquí se
//! descarta, igual que en el Python.
//!
//! # Adaptable al tipo de prestador, y el CUERPO no cambia
//!
//! Lo único que cambia entre un consultorio y un hospital es el SOBRE:
//!
//! | tipo | `numFactura` | `tipoNota` | `xmlFevFile` | endpoint |
//! |---|---|---|---|---|
//! | RIPS sin factura | `null` | `"RS"` | vacío | `CargarRipsSinFactura` |
//! | FEV en salud | el número | `null` | AttachedDocument | `CargarFevRips` |
//!
//! Un profesional independiente que factura menos de 3 500 UVT va por la
//! primera; una IPS, clínica u hospital por la segunda, con el XML **ya
//! aprobado por la DIAN**. Por eso el sobre es una capa aparte: añadir un tipo
//! es una entrada más, no un módulo nuevo.
//!
//! Referencias: MANUAL DE CONSUMO API-DOCKER-FEV-RIPS V 4.3 (FEVRM001 v2.0),
//! Resolución 2275 de 2023, Documento Técnico 1, y el MANUAL DE USUARIO
//! CONVERTIDOR A JSON, que es el que declara la estructura contra la 948.

use std::collections::HashMap;

use serde::Serialize;

use super::documentos::{Documento, Servicios, Usuario};

/// El `tipoNota` de un RIPS sin factura.
pub const TIPO_RIPS_SIN_FACTURA: &str = "RS";

/// El objeto RIPS pelado: lo que va **en el archivo**.
///
/// Hay DOS formas y confundirlas cuesta un ciclo de validación: el ARCHIVO que
/// se sube a la aplicación de escritorio —y el que produce el conversor web— es
/// este objeto; el CUERPO DE LA API lo envuelve en [`Paquete`]. Verificado
/// contra un archivo real del prestador (jul-2026): sus claves raíz son las
/// cinco de aquí, sin `rips` ni `xmlFevFile`.
#[derive(Debug, Clone, Serialize)]
pub struct Rips {
    #[serde(rename = "numDocumentoIdObligado")]
    pub num_documento_id_obligado: String,
    #[serde(rename = "numFactura")]
    pub num_factura: Option<String>,
    #[serde(rename = "tipoNota")]
    pub tipo_nota: Option<String>,
    #[serde(rename = "numNota")]
    pub num_nota: Option<String>,
    pub usuarios: Vec<Usuario>,
}

/// El cuerpo de la petición a la API del MUV.
#[derive(Debug, Clone, Serialize)]
pub struct Paquete {
    pub rips: Rips,
    #[serde(rename = "xmlFevFile")]
    pub xml_fev_file: String,
}

/// Por qué no se puede armar el paquete.
///
/// Se falla ANTES de enviar y no después: un rechazo del MUV cuesta un ciclo
/// completo de reenvío, y aquí el llamante todavía puede corregir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSePuede {
    /// RIPS sin factura sin `numNota`.
    FaltaNumNota,
    /// FEV en salud sin el AttachedDocument en Base64.
    FaltaXmlFev,
    /// Hay recién nacidos en el periodo cuya madre no tiene ninguna OTRA atención
    /// en el mismo periodo, así que no hay usuario del que colgar el arreglo.
    ///
    /// Lleva los `id` de las filas, y por eso `mensaje` devuelve `String` y no
    /// `&'static str`: quien cierra el mes necesita saber CUÁLES, no que «hay
    /// alguno». Un mensaje sin la lista obliga a buscarlos a mano en la tabla.
    RecienNacidosSinAtencionDeLaMadre(Vec<i64>),
}

impl NoSePuede {
    pub fn mensaje(&self) -> String {
        match self {
            NoSePuede::FaltaNumNota => {
                "numNota es obligatorio en un RIPS sin factura, debe ser consecutivo y no \
                 repetirse entre envíos: repetirlo produce rechazo en el MUV"
                    .to_string()
            }
            NoSePuede::FaltaXmlFev => {
                "una FEV en salud exige el AttachedDocument de la factura electrónica en \
                 Base64; si el prestador no factura electrónicamente, la ruta correcta es el \
                 RIPS sin factura"
                    .to_string()
            }
            NoSePuede::RecienNacidosSinAtencionDeLaMadre(ids) => {
                let lista: Vec<String> = ids.iter().map(i64::to_string).collect();
                format!(
                    "hay {} registro(s) de nacimiento (id {}) cuya madre no tiene ninguna otra \
                     atención en el periodo, así que el arreglo recienNacidos no tiene usuario \
                     del que colgar. Revise que la atención del parto esté registrada y \
                     facturada en el mismo periodo, o excluya esos nacimientos del cierre.",
                    ids.len(),
                    lista.join(", ")
                )
            }
        }
    }
}

/// Identidad de un usuario entre documentos distintos.
///
/// **NO se usa el `consecutivo`**: cada documento numera el suyo desde 1, así
/// que el 1 del AC y el 1 del AM son personas distintas. Fusionar por
/// consecutivo mezclaría los servicios de dos pacientes — y en un sistema
/// clínico eso es un dato falso, no una imprecisión.
fn clave(u: &Usuario) -> (String, String) {
    (u.tipo_documento.clone(), u.num_documento.clone())
}

/// Cadena vacía -> `None`, que es lo que el esquema declara.
fn a_null(v: &Option<String>) -> Option<String> {
    v.as_ref().filter(|s| !s.is_empty()).cloned()
}

/// Fusiona los documentos en una sola lista de usuarios.
///
/// Cada usuario queda una vez, con todos sus servicios agrupados, y se renumera
/// el `consecutivo` de forma global. Los de cada servicio se renumeran **dentro
/// de su propio tipo**, que es como los referencia el validador cuando devuelve
/// un error (`usuarios[0].servicios.consultas[2]`).
///
/// El orden de salida es el de PRIMERA APARICIÓN, no el de un mapa: un `HashMap`
/// recorrido daría un orden distinto en cada corrida y el archivo dejaría de
/// poder compararse con el del Python —y con el de ayer—.
pub fn consolidar(documentos: &[Documento]) -> Vec<Usuario> {
    let mut orden: Vec<Usuario> = Vec::new();
    let mut donde: HashMap<(String, String), usize> = HashMap::new();

    for d in documentos {
        for u in &d.usuarios {
            let k = clave(u);
            let pos = match donde.get(&k) {
                Some(i) => {
                    // Un paciente incapacitado en CUALQUIERA de sus atenciones
                    // lo está en el documento: el «SI» gana sobre el «NO».
                    if u.incapacidad == "SI" {
                        orden[*i].incapacidad = "SI".to_string();
                    }
                    *i
                }
                None => {
                    let mut base = u.clone();
                    base.servicios = Servicios::default();
                    orden.push(base);
                    donde.insert(k, orden.len() - 1);
                    orden.len() - 1
                }
            };
            let destino = &mut orden[pos].servicios;
            destino.consultas.extend(u.servicios.consultas.iter().cloned());
            destino
                .procedimientos
                .extend(u.servicios.procedimientos.iter().cloned());
            destino
                .medicamentos
                .extend(u.servicios.medicamentos.iter().cloned());
            destino.urgencias.extend(u.servicios.urgencias.iter().cloned());
            destino
                .hospitalizacion
                .extend(u.servicios.hospitalizacion.iter().cloned());
            destino
                .otros_servicios
                .extend(u.servicios.otros_servicios.iter().cloned());
        }
    }

    for (i, u) in orden.iter_mut().enumerate() {
        u.consecutivo = i as i64 + 1;
        for (j, c) in u.servicios.consultas.iter_mut().enumerate() {
            c.consecutivo = j as i64 + 1;
            c.num_autorizacion = a_null(&c.num_autorizacion);
            c.num_fev_pago_moderador = a_null(&c.num_fev_pago_moderador);
        }
        for (j, p) in u.servicios.procedimientos.iter_mut().enumerate() {
            p.consecutivo = j as i64 + 1;
            p.num_autorizacion = a_null(&p.num_autorizacion);
            p.num_fev_pago_moderador = a_null(&p.num_fev_pago_moderador);
        }
        for (j, m) in u.servicios.medicamentos.iter_mut().enumerate() {
            m.consecutivo = j as i64 + 1;
            m.num_fev_pago_moderador = a_null(&m.num_fev_pago_moderador);
        }
        for (j, x) in u.servicios.urgencias.iter_mut().enumerate() {
            x.consecutivo = j as i64 + 1;
        }
        for (j, h) in u.servicios.hospitalizacion.iter_mut().enumerate() {
            h.consecutivo = j as i64 + 1;
            h.num_autorizacion = a_null(&h.num_autorizacion);
        }
        for (j, x) in u.servicios.otros_servicios.iter_mut().enumerate() {
            x.consecutivo = j as i64 + 1;
            x.num_autorizacion = a_null(&x.num_autorizacion);
            x.num_fev_pago_moderador = a_null(&x.num_fev_pago_moderador);
        }
    }
    orden
}

/// El obligado: el primero que alguno de los documentos declare.
fn obligado(documentos: &[Documento]) -> String {
    documentos
        .iter()
        .map(|d| d.num_documento_id_obligado.as_str())
        .find(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// Sobre para profesional independiente o entidad de objeto social diferente.
///
/// `xmlFevFile` va VACÍO: esta ruta no exige factura electrónica de la DIAN.
/// `numNota` es OBLIGATORIO y no se puede repetir entre envíos — el manual del
/// convertidor advierte que repetirlo «generará rechazo».
pub fn sobre_rips_sin_factura(
    documentos: &[Documento],
    num_nota: &str,
) -> Result<Paquete, NoSePuede> {
    if num_nota.trim().is_empty() {
        return Err(NoSePuede::FaltaNumNota);
    }
    Ok(Paquete {
        rips: Rips {
            num_documento_id_obligado: obligado(documentos),
            num_factura: None,
            tipo_nota: Some(TIPO_RIPS_SIN_FACTURA.to_string()),
            num_nota: Some(num_nota.to_string()),
            usuarios: consolidar(documentos),
        },
        xml_fev_file: String::new(),
    })
}

/// Sobre para prestador que factura electrónicamente: IPS, clínica, hospital.
///
/// `xmlFevFile` es el AttachedDocument de la factura electrónica en Base64, y
/// debe estar **ya aprobado por la DIAN**: el MUV valida después de esa
/// aprobación.
pub fn sobre_fev(
    documentos: &[Documento],
    num_factura: &str,
    xml_fev_base64: &str,
) -> Result<Paquete, NoSePuede> {
    if xml_fev_base64.is_empty() {
        return Err(NoSePuede::FaltaXmlFev);
    }
    Ok(Paquete {
        rips: Rips {
            num_documento_id_obligado: obligado(documentos),
            num_factura: Some(num_factura.to_string()),
            tipo_nota: None,
            num_nota: None,
            usuarios: consolidar(documentos),
        },
        xml_fev_file: xml_fev_base64.to_string(),
    })
}

/// El archivo tal como lo espera la aplicación de escritorio: el objeto pelado,
/// con sangría de **un** espacio y sin escapar los no-ASCII.
///
/// La sangría de uno no es capricho: es la que produce `json.dump(..., indent=1)`
/// del Python, y compararlos byte a byte es la condición del ADR.
///
/// **No lleva BOM.** El conversor oficial escribe UTF-8 con BOM y un archivo
/// real del prestador lo trae; quien escriba el archivo decide si lo pone —ver
/// [`con_bom`]—, porque una cadena con BOM dentro es una trampa para cualquier
/// otro consumidor.
pub fn archivo(p: &Paquete) -> String {
    let mut salida = Vec::new();
    let sangria = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut salida, sangria);
    p.rips
        .serialize(&mut ser)
        .expect("el RIPS siempre serializa: no hay mapas con claves no textuales");
    String::from_utf8(salida).expect("serde_json emite UTF-8 válido")
}

/// El archivo con la marca de orden de bytes que escribe el conversor oficial.
///
/// No está confirmado que el validador la EXIJA; parecerse al archivo que ya le
/// funciona al prestador es la apuesta segura, y por eso va aparte y no dentro
/// de [`archivo`].
pub fn con_bom(texto: &str) -> Vec<u8> {
    let mut v = vec![0xEF, 0xBB, 0xBF];
    v.extend_from_slice(texto.as_bytes());
    v
}

/// Atenciones por tipo. Sirve para cuadrar contra lo que devuelve `ConsultarCUV`.
pub fn contar_atenciones(p: &Paquete) -> Conteo {
    let mut c = Conteo::default();
    for u in &p.rips.usuarios {
        c.consultas += u.servicios.consultas.len();
        c.procedimientos += u.servicios.procedimientos.len();
        c.medicamentos += u.servicios.medicamentos.len();
        c.urgencias += u.servicios.urgencias.len();
        c.hospitalizacion += u.servicios.hospitalizacion.len();
    }
    c
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Conteo {
    pub consultas: usize,
    pub procedimientos: usize,
    pub medicamentos: usize,
    pub urgencias: usize,
    pub hospitalizacion: usize,
}

impl Conteo {
    pub fn total(&self) -> usize {
        self.consultas
            + self.procedimientos
            + self.medicamentos
            + self.urgencias
            + self.hospitalizacion
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Las reglas de la fusión que la fixture realista NO puede ejercer.**
    ///
    /// `fev_rips_paridad_con_python.rs` compara documentos hechos por los
    /// generadores de verdad, y ahí el `incapacidad` de un paciente lo decide su
    /// PRIMERA atención dentro de cada archivo: nunca llega un «SI» por un
    /// documento posterior, así que la regla del «SI gana» no se recorre. Lo
    /// destapó un mutante que sobrevivió cambiándola por «NO».
    ///
    /// Aquí los documentos se escriben a mano, que es donde ese caso existe.
    fn usuario(documento: &str, incapacidad: &str, consecutivo: i64) -> Usuario {
        Usuario {
            tipo_documento: "CC".into(),
            num_documento: documento.into(),
            tipo_usuario: Some("01".into()),
            fecha_nacimiento: "1980-05-04".into(),
            cod_sexo: "M".into(),
            cod_pais_residencia: Some("170".into()),
            cod_municipio_residencia: Some("05001".into()),
            cod_zona_territorial_residencia: Some("01".into()),
            cod_pais_origen: Some("170".into()),
            consecutivo,
            incapacidad: incapacidad.into(),
            servicios: Servicios::default(),
        }
    }

    fn doc(usuarios: Vec<Usuario>) -> Documento {
        Documento {
            num_documento_id_obligado: "900123456".into(),
            num_factura: String::new(),
            tipo_nota: String::new(),
            num_nota: String::new(),
            usuarios,
        }
    }

    /// Un paciente incapacitado en CUALQUIERA de sus atenciones lo está en el
    /// documento: el «SI» gana sobre el «NO», venga en el orden que venga.
    #[test]
    fn el_si_de_incapacidad_gana_llegue_cuando_llegue() {
        // El «SI» llega DESPUÉS.
        let u = consolidar(&[
            doc(vec![usuario("111", "NO", 1)]),
            doc(vec![usuario("111", "SI", 1)]),
        ]);
        assert_eq!(u.len(), 1, "no fusionó al mismo paciente");
        assert_eq!(u[0].incapacidad, "SI", "el «SI» posterior no ganó");

        // Y llegando ANTES tampoco se pierde: un «NO» posterior no degrada.
        let u = consolidar(&[
            doc(vec![usuario("111", "SI", 1)]),
            doc(vec![usuario("111", "NO", 1)]),
        ]);
        assert_eq!(u[0].incapacidad, "SI", "un «NO» posterior degradó el «SI»");

        // LA PAREJA: sin ningún «SI», sigue siendo «NO». Sin esto, una regla que
        // pusiera «SI» a todo el mundo pasaría las dos de arriba.
        let u = consolidar(&[
            doc(vec![usuario("111", "NO", 1)]),
            doc(vec![usuario("111", "NO", 1)]),
        ]);
        assert_eq!(u[0].incapacidad, "NO");
    }

    /// **Dos personas distintas no se fusionan**, y la identidad NO es el
    /// consecutivo: cada documento numera el suyo desde 1, así que el 1 del AC y
    /// el 1 del AM son personas distintas. Fusionar por consecutivo mezclaría
    /// los servicios de dos pacientes — en un sistema clínico eso es un dato
    /// falso, no una imprecisión.
    #[test]
    fn la_identidad_es_el_documento_y_no_el_consecutivo() {
        let u = consolidar(&[
            doc(vec![usuario("111", "NO", 1)]),
            doc(vec![usuario("222", "NO", 1)]),
        ]);
        assert_eq!(u.len(), 2, "dos personas con el mismo consecutivo se fusionaron");
        assert_eq!(u[0].num_documento, "111");
        assert_eq!(u[1].num_documento, "222");
        // Y se renumeran de forma global.
        assert_eq!((u[0].consecutivo, u[1].consecutivo), (1, 2));

        // La pareja: el MISMO documento con consecutivos distintos SÍ es una
        // persona sola.
        let u = consolidar(&[
            doc(vec![usuario("111", "NO", 5)]),
            doc(vec![usuario("111", "NO", 9)]),
        ]);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].consecutivo, 1);
    }

    fn otro_servicio(consecutivo: i64, num_autorizacion: Option<&str>) -> super::super::aos::OtroServicio {
        super::super::aos::OtroServicio {
            cod_prestador: "050010123401".into(),
            num_autorizacion: num_autorizacion.map(str::to_string),
            id_mipres: None,
            fecha_suministro_tecnologia: "2026-03-10".into(),
            tipo_os: "01".into(),
            cod_tecnologia_salud: Some("INV-1".into()),
            nom_tecnologia_salud: "Catéter".into(),
            cantidad_os: Some(1),
            tipo_documento_identificacion: Some("CC".into()),
            num_documento_identificacion: Some("111".into()),
            vr_unit_os: Some(1000),
            vr_dispensacion: Some(0),
            vr_servicio: Some(1000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: 0,
            num_fev_pago_moderador: None,
            codigo_vida: None,
            consecutivo,
        }
    }

    /// `consolidar` funde los `otrosServicios` del AT en el usuario, los renumera
    /// dentro de su tipo y convierte `numAutorizacion` vacío en null. El banco de
    /// campos llama a `documento_aos` directo y NO pasa por aquí, así que sin esta
    /// prueba revertir el `extend` o el renumerado del AT no rompía nada
    /// (`sin-caso-rojo`, 2026-09-08). Dos suministros del mismo paciente:
    #[test]
    fn consolidar_funde_renumera_y_anula_el_at() {
        let mut u = usuario("111", "NO", 1);
        // Consecutivos «crudos» y desordenados, y una autorización vacía que debe
        // salir null; la fusión debe reescribir el orden a 1,2.
        u.servicios.otros_servicios = vec![otro_servicio(9, Some("")), otro_servicio(4, Some("AUT-2"))];
        let out = consolidar(&[doc(vec![u])]);
        assert_eq!(out.len(), 1);
        let at = &out[0].servicios.otros_servicios;
        assert_eq!(at.len(), 2, "el AT no se fundió en el usuario");
        assert_eq!((at[0].consecutivo, at[1].consecutivo), (1, 2), "no se renumeró dentro del tipo");
        assert_eq!(at[0].num_autorizacion, None, "la autorización vacía no se anuló (a_null)");
        assert_eq!(at[1].num_autorizacion.as_deref(), Some("AUT-2"), "se perdió una autorización real");
    }

    /// El obligado sale del primer documento que lo declare, y una lista vacía
    /// no revienta: devuelve un paquete sin usuarios, que es lo que hay.
    #[test]
    fn sin_documentos_el_paquete_sale_vacio_y_no_revienta() {
        let p = sobre_rips_sin_factura(&[], "NOTA-1").unwrap();
        assert!(p.rips.usuarios.is_empty());
        assert_eq!(p.rips.num_documento_id_obligado, "");
        assert_eq!(contar_atenciones(&p).total(), 0);
    }
}
