// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Las piezas que los cinco documentos del RIPS comparten (#250).
//!
//! Port de los ayudantes de `backend/rips_completo.py`: el bloque de
//! identificación del usuario, el documento del profesional que atendió y las
//! dos formas de fecha.
//!
//! # Aquí no se usa `#[serde(flatten)]`, y es a propósito
//!
//! `flatten` obliga a serde a pasar por un mapa intermedio, y `serde_json` lo
//! ordena ALFABÉTICAMENTE salvo que se compile con `preserve_order`. El JSON
//! seguiría siendo válido, pero dejaría de poder compararse byte a byte con el
//! del Python — que es la condición que el ADR pone para apagarlo. Por eso los
//! dos campos del profesional se declaran a mano en cada documento, en su sitio.

use serde::Serialize;

use super::ac::Consulta;
use super::recolector::{Donde, Recolector};
use crate::muv;

/// La institución que reporta. `codigo_habilitacion` es el `codPrestador` de
/// cada servicio; el NIT, el `numDocumentoIdObligado` del documento.
#[derive(Debug, Clone)]
pub struct FuenteInstitucion {
    pub nit: String,
    pub codigo_habilitacion: String,
}

/// La fila del paciente, ya traída de la base.
#[derive(Debug, Clone)]
pub struct FuentePaciente {
    /// El id de la fila, para poder señalar dónde corregir lo que falte.
    pub id: i64,
    pub tipo_documento: String,
    pub numero_documento: String,
    pub tipo_usuario: Option<String>,
    /// Ya en la forma del RIPS (`AAAA-MM-DD`): la trae `medico-datos`, que es
    /// quien habla con la base.
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub cod_pais: Option<String>,
    pub cod_municipio: Option<String>,
    pub zona: Option<String>,
    pub cod_pais_origen: Option<String>,
}

/// El profesional que atendió. **No es el paciente** — ver
/// [`documento_profesional`].
#[derive(Debug, Clone, Default)]
pub struct FuenteProfesional {
    pub tipo_documento: Option<String>,
    pub numero_documento: Option<String>,
}

/// Los servicios de un usuario dentro de un documento.
///
/// Cada archivo llena el suyo; los vacíos no se emiten, porque el AC de un
/// paciente no lleva una lista vacía de medicamentos — lleva solo consultas.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Servicios {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub consultas: Vec<Consulta>,
}

/// El usuario tal y como sale al JSON.
///
/// El orden de los campos es el del Python, y no es cosmético: es lo que
/// permite comparar los dos archivos byte a byte.
#[derive(Debug, Clone, Serialize)]
pub struct Usuario {
    #[serde(rename = "tipoDocumentoIdentificacion")]
    pub tipo_documento: String,
    #[serde(rename = "numDocumentoIdentificacion")]
    pub num_documento: String,
    #[serde(rename = "tipoUsuario")]
    pub tipo_usuario: Option<String>,
    #[serde(rename = "fechaNacimiento")]
    pub fecha_nacimiento: String,
    #[serde(rename = "codSexo")]
    pub cod_sexo: String,
    #[serde(rename = "codPaisResidencia")]
    pub cod_pais_residencia: Option<String>,
    #[serde(rename = "codMunicipioResidencia")]
    pub cod_municipio_residencia: Option<String>,
    #[serde(rename = "codZonaTerritorialResidencia")]
    pub cod_zona_territorial_residencia: Option<String>,
    #[serde(rename = "codPaisOrigen")]
    pub cod_pais_origen: Option<String>,
    pub consecutivo: i64,
    /// `"SI"` o `"NO"`. Lo pone el documento, no el bloque de identificación.
    pub incapacidad: String,
    pub servicios: Servicios,
}

/// El documento del RIPS: la envoltura común a los cinco archivos.
#[derive(Debug, Clone, Serialize)]
pub struct Documento {
    #[serde(rename = "numDocumentoIdObligado")]
    pub num_documento_id_obligado: String,
    #[serde(rename = "numFactura")]
    pub num_factura: String,
    #[serde(rename = "tipoNota")]
    pub tipo_nota: String,
    #[serde(rename = "numNota")]
    pub num_nota: String,
    pub usuarios: Vec<Usuario>,
}

/// La identificación del usuario, con sus huecos anotados.
///
/// `codMunicipioResidencia` traía `"05001"` de supuesto: reportaba como
/// residente de Medellín a todo paciente sin municipio, en el país entero. El
/// campo tiene la forma correcta, así que el validador lo acepta y el dato falso
/// entra en la estadística de residencia sin que nadie lo note.
pub fn usuario_base(
    p: &FuentePaciente,
    consecutivo: i64,
    rec: &mut Recolector,
) -> Usuario {
    let permisivo = rec.permite_supuestos();
    let mut exigir = |valor: Option<&str>, campo: &'static str, etiqueta: &'static str,
                      supuesto: Option<&'static str>| {
        let mut donde = Donde::usuarios(p.id, campo, etiqueta);
        if let Some(s) = supuesto {
            donde = donde.supuesto(s);
        }
        let d = rec.dato(valor, &donde);
        if permisivo {
            d.o_el_supuesto(supuesto)
        } else {
            d.o_nada()
        }
    };

    Usuario {
        tipo_documento: p.tipo_documento.clone(),
        num_documento: p.numero_documento.clone(),
        tipo_usuario: exigir(
            p.tipo_usuario.as_deref(),
            "tipo_usuario",
            "tipo de usuario",
            Some("01"),
        ),
        fecha_nacimiento: p.fecha_nacimiento.clone(),
        cod_sexo: p.sexo.clone(),
        cod_pais_residencia: exigir(
            p.cod_pais.as_deref(),
            "cod_pais",
            "país de residencia",
            Some("170"),
        ),
        cod_municipio_residencia: exigir(
            p.cod_municipio.as_deref(),
            "cod_municipio",
            "municipio de residencia",
            Some("05001"),
        ),
        cod_zona_territorial_residencia: exigir(
            p.zona.as_deref(),
            "zona",
            "zona territorial",
            Some("01"),
        ),
        cod_pais_origen: exigir(
            p.cod_pais_origen.as_deref(),
            "cod_pais_origen",
            "país de origen",
            Some("170"),
        ),
        consecutivo,
        // Las dos las pone el documento que llama.
        incapacidad: String::from("NO"),
        servicios: Servicios::default(),
    }
}

/// Los campos P11/P12 de un bloque de servicios: **quién atendió**.
///
/// La Resolución 948 los define como tipo y número de documento del profesional
/// de la salud que ordenó o realizó el procedimiento, y solo admite ocho tipos
/// —ninguno de menor de edad—.
///
/// Hasta el 2026-07-26 aquí iba el documento del PACIENTE. El MUV lo destapó
/// rechazando las atenciones de menores («valores permitidos
/// CC,CE,CD,PA,SC,PE,DE,PT, valor enviado: RC»). Lo grave no era el rechazo: las
/// de adultos PASABAN, llevando el documento del paciente como si fuera el del
/// médico. Un dato falso que nunca se habría notado.
///
/// **No hay supuesto.** Inventar el documento de un profesional es atribuir un
/// acto médico a alguien.
pub fn documento_profesional(
    medico: &FuenteProfesional,
    rec: &mut Recolector,
    donde_tipo: Donde,
    donde_numero: Donde,
) -> (Option<String>, Option<String>) {
    let tipo = rec.codigo(medico.tipo_documento.as_deref(), &donde_tipo, |v| {
        if muv::admite_como_profesional(v) {
            Ok(())
        } else {
            // Presente pero imposible. Se devuelve TAL CUAL, como hace `codigo`
            // con un CUM inválido: sustituirlo escondería lo que hay en la base,
            // que es el dato que hay que corregir.
            Err(format!(
                "«{v}» no puede ser el documento de un profesional. La \
                 Resolución 948 solo admite {}.",
                muv::tipos_documento_profesional_listados()
            ))
        }
    });
    let numero = rec.dato(medico.numero_documento.as_deref(), &donde_numero);
    (tipo.o_nada(), numero.o_nada())
}

/// Fecha y hora como las quiere el RIPS: **sin segundos**.
///
/// Iba con `%H:%M:%S` y el MUV lo rechaza —«Fecha no válida, formato
/// (AAAA-MM-DD HH:MM)»— en `fechaEgreso` y `fechaDispensAdmon`. La ficha M04
/// declara el campo de 16 caracteres, que es lo que mide «2021-08-18 08:10»;
/// con segundos son 19.
pub fn fecha_dt(f: &chrono::NaiveDateTime) -> String {
    f.format("%Y-%m-%d %H:%M").to_string()
}

/// Fecha sola, `AAAA-MM-DD`.
pub fn fecha_d(f: &chrono::NaiveDate) -> String {
    f.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveDate;

    pub(super) fn paciente() -> FuentePaciente {
        FuentePaciente {
            id: 7,
            tipo_documento: "CC".into(),
            numero_documento: "1234567".into(),
            tipo_usuario: Some("01".into()),
            fecha_nacimiento: "1980-05-04".into(),
            sexo: "M".into(),
            cod_pais: Some("170".into()),
            cod_municipio: Some("05001".into()),
            zona: Some("01".into()),
            cod_pais_origen: Some("170".into()),
        }
    }

    #[test]
    fn la_fecha_va_sin_segundos() {
        let f = NaiveDate::from_ymd_opt(2021, 8, 18)
            .unwrap()
            .and_hms_opt(8, 10, 45)
            .unwrap();
        assert_eq!(fecha_dt(&f), "2021-08-18 08:10");
        assert_eq!(fecha_dt(&f).len(), 16, "el MUV declara el campo de 16");
        assert_eq!(fecha_d(&f.date()), "2021-08-18");
    }

    #[test]
    fn el_usuario_completo_no_anota_nada() {
        let mut rec = Recolector::estricto();
        let u = usuario_base(&paciente(), 1, &mut rec);
        assert!(rec.faltantes().is_empty());
        assert_eq!(u.cod_municipio_residencia.as_deref(), Some("05001"));
        assert_eq!(u.consecutivo, 1);
    }

    #[test]
    fn sin_municipio_el_campo_va_vacio_y_no_a_medellin() {
        let mut p = paciente();
        p.cod_municipio = None;
        let mut rec = Recolector::estricto();
        let u = usuario_base(&p, 1, &mut rec);
        assert_eq!(u.cod_municipio_residencia, None, "no se rellena con 05001");
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].campo, "cod_municipio");
        assert_eq!(f[0].archivo, "usuarios", "la falta no es de la consulta");
        assert_eq!(f[0].registro_id, 7);
        assert_eq!(f[0].supuesto.as_deref(), Some("05001"),
                   "el supuesto se DECLARA aunque no se use");
    }

    #[test]
    fn con_supuestos_permitidos_el_municipio_sale_pero_queda_anotado() {
        let mut p = paciente();
        p.cod_municipio = None;
        let mut rec = Recolector::permitiendo_supuestos();
        let u = usuario_base(&p, 1, &mut rec);
        assert_eq!(u.cod_municipio_residencia.as_deref(), Some("05001"));
        assert_eq!(rec.faltantes().len(), 1, "sale, pero no en silencio");
    }

    #[test]
    fn el_documento_del_profesional_no_admite_el_de_un_menor() {
        let medico = FuenteProfesional {
            tipo_documento: Some("RC".into()),
            numero_documento: Some("999".into()),
        };
        let mut rec = Recolector::estricto();
        let (tipo, numero) = documento_profesional(
            &medico,
            &mut rec,
            Donde::ac(3, "medico.tipo_documento", "tipo de documento del profesional"),
            Donde::ac(3, "medico.numero_documento", "número de documento del profesional"),
        );
        assert_eq!(tipo.as_deref(), Some("RC"), "se devuelve TAL CUAL, no se sustituye");
        assert_eq!(numero.as_deref(), Some("999"));
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert!(f[0].motivo.as_deref().unwrap().contains("no puede ser el documento"));
        assert!(f[0].motivo.as_deref().unwrap().contains("CC"), "dice cuáles sí");
    }

    #[test]
    fn el_profesional_valido_no_anota_nada() {
        let medico = FuenteProfesional {
            tipo_documento: Some("CC".into()),
            numero_documento: Some("111".into()),
        };
        let mut rec = Recolector::estricto();
        let (tipo, numero) = documento_profesional(
            &medico,
            &mut rec,
            Donde::ac(3, "medico.tipo_documento", "tipo"),
            Donde::ac(3, "medico.numero_documento", "número"),
        );
        assert_eq!(tipo.as_deref(), Some("CC"));
        assert_eq!(numero.as_deref(), Some("111"));
        assert!(rec.faltantes().is_empty());
    }

    #[test]
    fn sin_profesional_se_anota_y_no_se_inventa() {
        let mut rec = Recolector::permitiendo_supuestos();
        let (tipo, numero) = documento_profesional(
            &FuenteProfesional::default(),
            &mut rec,
            Donde::ac(3, "medico.tipo_documento", "tipo"),
            Donde::ac(3, "medico.numero_documento", "número"),
        );
        // Ni siquiera con supuestos permitidos: no hay supuesto que declarar,
        // porque inventarlo sería atribuir el acto médico a alguien.
        assert_eq!(tipo, None);
        assert_eq!(numero, None);
        assert_eq!(rec.faltantes().len(), 2);
    }
}
