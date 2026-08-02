// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Los maestros del Mecanismo Único de Validación, empotrados (#250).
//!
//! Salen del contenedor del propio validador de MinSalud —`Rips.REFs_*`—, que
//! es la tabla contra la que el MUV juzga lo que se le envía.
//!
//! **Ningún código de catálogo se escribe a mano.** El 2026-07-26 se inventó
//! aquí al lado un mapa de presentaciones (`tableta = 1`, cuando el catálogo
//! dice 66 y el 1 es AMPOLLA); el MUV lo aceptó y devolvió CUV, porque ese campo
//! lo valida por rango y no contra la tabla. Un dato falso que pasa la
//! validación no lo corrige nadie, porque nadie lo ve.

use serde::Deserialize;
use std::sync::OnceLock;

const TIPO_DOC_JSON: &str = include_str!("../datos/muv_tipo_doc.json");
const FINALIDAD_JSON: &str = include_str!("../datos/muv_finalidad.json");

#[derive(Debug, Deserialize)]
struct FilaTipoDoc {
    #[serde(rename = "TipoDocumentoIdentificacion")]
    tipo: String,
    #[serde(rename = "DocumentoIdentificacion")]
    nombre: String,
    /// Si el MUV lo admite para quien REALIZA la atención. Es la columna que
    /// separa los ocho de los trece.
    #[serde(rename = "RealizaAtencionRips")]
    realiza_atencion: bool,
}

#[derive(Debug, Deserialize)]
struct ArchivoTipoDoc {
    filas: Vec<FilaTipoDoc>,
}

/// Los tipos de documento que el MUV admite para el profesional que atendió,
/// con su nombre. Ninguno es de menor ni de «sin identificar»: quien atiende es
/// un adulto identificado.
///
/// Es la regla que el MUV aplicó al rechazarnos las consultas de niños, cuando
/// el documento del PACIENTE iba en el campo del médico. Lo grave no fue el
/// rechazo: las atenciones de adultos pasaban, llevando el documento del
/// paciente como si fuera el del profesional.
pub fn tipos_documento_profesional() -> &'static [(String, String)] {
    static CACHE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let archivo: ArchivoTipoDoc = serde_json::from_str(TIPO_DOC_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan");
        archivo
            .filas
            .into_iter()
            .filter(|f| f.realiza_atencion)
            .map(|f| (f.tipo, f.nombre))
            .collect()
    })
}

/// Si un tipo de documento puede ser el de quien realizó la atención.
pub fn admite_como_profesional(tipo: &str) -> bool {
    tipos_documento_profesional().iter().any(|(t, _)| t == tipo)
}

/// Los códigos admitidos, para el mensaje que ve quien tiene que corregirlo.
pub fn tipos_documento_profesional_listados() -> String {
    tipos_documento_profesional()
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Deserialize)]
struct FilaFinalidad {
    #[serde(rename = "Codigo")]
    codigo: String,
    /// Si aplica al archivo de consultas. El maestro la trae por archivo.
    #[serde(rename = "AplicaAC")]
    aplica_ac: bool,
}

#[derive(Debug, Deserialize)]
struct ArchivoFinalidad {
    filas: Vec<FilaFinalidad>,
}

fn finalidades() -> &'static [FilaFinalidad] {
    static CACHE: OnceLock<Vec<FilaFinalidad>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<ArchivoFinalidad>(FINALIDAD_JSON)
            .expect("el maestro va empotrado y las pruebas lo validan")
            .filas
    })
}

/// Si el código de finalidad existe en la tabla del validador.
///
/// **Existe por el `10`.** El modelo del Python (`FinalidadConsulta.NO_APLICA`)
/// y el esquema de SQLite (`finalidad TEXT NOT NULL DEFAULT '10'`) ponen `10`
/// en toda consulta donde nadie eligió una finalidad — y `10` NO ESTÁ en la
/// tabla del MUV, que va del 11 al 44 (medido el 2026-08-01 sobre
/// `REFs_FinalidadTecnologiaSalud`, 34 filas).
///
/// Como es un defecto de la base y no una ausencia, el recolector no lo veía:
/// el campo siempre venía «lleno». Aquí se convierte en lo que es —un código que
/// no existe— y sale en el informe de cierre con su motivo. No se sustituye por
/// `11`: eso sería elegir la finalidad del acto médico en nombre de quien lo
/// hizo.
pub fn finalidad_existe(codigo: &str) -> bool {
    finalidades().iter().any(|f| f.codigo == codigo)
}

/// Las finalidades que el maestro marca como aplicables al archivo de consultas.
pub fn finalidades_de_consulta() -> Vec<&'static str> {
    finalidades()
        .iter()
        .filter(|f| f.aplica_ac)
        .map(|f| f.codigo.as_str())
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn son_los_ocho_que_el_muv_nombro_al_rechazarnos() {
        // El mensaje literal del MUV: «valores permitidos CC,CE,CD,PA,SC,PE,DE,PT».
        let mut codigos: Vec<&str> = tipos_documento_profesional()
            .iter()
            .map(|(t, _)| t.as_str())
            .collect();
        codigos.sort_unstable();
        assert_eq!(codigos, ["CC", "CD", "CE", "DE", "PA", "PE", "PT", "SC"]);
    }

    #[test]
    fn los_de_menor_no_atienden_a_nadie() {
        // RC (registro civil) y TI (tarjeta de identidad) SÍ están en el maestro
        // —un paciente puede tenerlos— pero no como quien realiza la atención.
        // Es justo el caso que destapó el fallo.
        assert!(!admite_como_profesional("RC"));
        assert!(!admite_como_profesional("TI"));
        assert!(admite_como_profesional("CC"));
    }

    #[test]
    fn el_filtro_descarta_de_verdad_algunas_filas() {
        // Si `RealizaAtencionRips` dejara de leerse, pasarían las trece y esta
        // prueba lo dice. Sin ella, un filtro que no filtra se ve igual de verde.
        let archivo: ArchivoTipoDoc = serde_json::from_str(TIPO_DOC_JSON).unwrap();
        assert_eq!(archivo.filas.len(), 13);
        assert_eq!(tipos_documento_profesional().len(), 8);
    }

    #[test]
    fn el_diez_de_finalidad_no_existe_en_la_tabla_del_muv() {
        // La razón entera de que `finalidad_existe` exista. El defecto de la
        // base pone `10` en toda consulta sin finalidad elegida, y `10` no está.
        assert!(!finalidad_existe("10"));
        assert!(finalidad_existe("11"));
        assert!(finalidad_existe("44"));
        assert!(!finalidad_existe("45"));
        assert!(!finalidad_existe(""));
    }

    #[test]
    fn la_tabla_de_finalidad_va_del_11_al_44() {
        let mut c: Vec<&str> = finalidades().iter().map(|f| f.codigo.as_str()).collect();
        c.sort_unstable();
        assert_eq!(c.len(), 34);
        assert_eq!(*c.first().unwrap(), "11");
        assert_eq!(*c.last().unwrap(), "44");
    }

    #[test]
    fn no_todas_las_finalidades_aplican_a_consultas() {
        // Si `AplicaAC` dejara de leerse, saldrían las 34 y esto lo dice.
        let de_consulta = finalidades_de_consulta();
        assert!(!de_consulta.is_empty());
        assert!(de_consulta.len() < finalidades().len(),
                "el filtro AplicaAC no está filtrando nada");
    }

    #[test]
    fn lo_desconocido_no_se_admite() {
        assert!(!admite_como_profesional(""));
        assert!(!admite_como_profesional("XX"));
        // Y no se acepta por parecido: el MUV compara la cadena exacta.
        assert!(!admite_como_profesional("cc"));
    }
}
