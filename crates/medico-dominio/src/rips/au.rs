// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! **AU — Archivo de Urgencias.** Port de `generar_archivo_au` (#67).
//!
//! Obligatorio para toda IPS con servicio de urgencias habilitado (Res.
//! 2275/2023). Es el documento que la ficha #67 mandaba no implementar sin
//! contrastar contra el PDF de la norma — y al hacerlo aparecieron dos cosas.
//!
//! # No lleva ni documento del profesional ni bloque económico
//!
//! El MUV lo dice literal: «Propiedades no permitidas en la estructura de
//! urgencias: tipoDocumentoIdentificacion, numDocumentoIdentificacion». Los
//! campos P11/P12 existen en el AC, el AP y el AM, y aquí **no**. Tampoco
//! `vrServicio`, `conceptoRecaudo`, `valorPagoModerador` ni
//! `numFEVPagoModerador`: el ejemplo de la 948 para el AU termina en
//! `fechaEgreso`, `consecutivo` y los CIE-11. Un campo que SOBRA hace rechazar
//! el archivo completo, igual que uno que falta.
//!
//! # La condición y destino del egreso, que estaba mal en el sistema vivo
//!
//! La 948 no trae esa tabla: remite a `web.sispro.gov.co`. El Python llevaba un
//! mapa escrito a mano donde `FALLECIDO` era el `04` y `HOSPITALIZADO` el `02`,
//! y en el catálogo del validador el **02 es «paciente muerto»** y el **04
//! «referido a otra institución»** — cruzados. Medido el 2026-08-04: de los seis
//! destinos que el sistema guarda, cuatro no casaban con ninguna clave del mapa
//! y los dos que sí casaban emitían el código equivocado, así que **una muerte
//! en urgencias se reportaba como un traslado**.
//!
//! Aquí sale de [`muv::condicion_destino_de_urgencias`], que se genera del
//! maestro. Y la **fuga se anota como falta**: el catálogo no tiene código para
//! «el paciente se fue sin que nadie le diera salida», y el más parecido por su
//! forma —el `01`— afirma un alta a domicilio.
//!
//! # Las atenciones sin egreso no entran
//!
//! No han terminado y no se pueden facturar. Antes se les ponía
//! `fecha_egreso = now()`, o sea que un paciente todavía en urgencias se
//! reportaba como dado de alta en el instante en que alguien generó el archivo.

use serde::Serialize;

use super::documentos::{
    fecha_dt, usuario_base, Documento, FuenteInstitucion, FuentePaciente, Usuario,
};
use super::recolector::{Donde, Recolector};
use crate::{cie11, muv};

/// Una atención de urgencias ya traída de la base, con su paciente.
///
/// **No lleva profesional**: el AU no admite los campos P11/P12.
#[derive(Debug, Clone)]
pub struct FuenteUrgencia {
    /// El id de la atención, para señalar dónde corregir.
    pub id: i64,
    pub fecha_ingreso: chrono::NaiveDateTime,
    /// Las atenciones sin egreso no llegan hasta aquí: las descarta la consulta.
    pub fecha_egreso: chrono::NaiveDateTime,
    pub causa_motivo: Option<String>,
    pub diagnostico_ingreso: Option<String>,
    pub diagnostico_egreso: Option<String>,
    /// El vocabulario del sistema (`DOMICILIO`, `FALLECIDO`…), no el código del
    /// RIPS: la traducción es de [`muv::condicion_destino_de_urgencias`].
    pub destino_salida: Option<String>,
    pub diagnostico_causa_muerte: Option<String>,
    pub paciente: FuentePaciente,
}

/// Una atención de urgencias tal y como sale al JSON de la 948.
#[derive(Debug, Clone, Serialize)]
pub struct Urgencia {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "fechaInicioAtencion")]
    pub fecha_inicio_atencion: String,
    #[serde(rename = "causaMotivoAtencion")]
    pub causa_motivo_atencion: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipal")]
    pub cod_diagnostico_principal: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipalE")]
    pub cod_diagnostico_principal_e: Option<String>,
    #[serde(rename = "codDiagnosticoRelacionadoE1")]
    pub cod_diagnostico_relacionado_e1: Option<String>,
    #[serde(rename = "condicionDestinoUsuarioEgreso")]
    pub condicion_destino_usuario_egreso: Option<String>,
    #[serde(rename = "codDiagnosticoCausaMuerte")]
    pub cod_diagnostico_causa_muerte: Option<String>,
    #[serde(rename = "fechaEgreso")]
    pub fecha_egreso: String,
    pub consecutivo: i64,
    #[serde(rename = "codDiagnosticoPrincipalCIE11")]
    pub cod_diagnostico_principal_cie11: String,
    #[serde(rename = "nomCodDiagnosticoPrincipalCIE11")]
    pub nom_cod_diagnostico_principal_cie11: String,
    #[serde(rename = "codDiagnosticoPrincipalECIE11")]
    pub cod_diagnostico_principal_e_cie11: String,
    #[serde(rename = "nomCodDiagnosticoPrincipalECIE11")]
    pub nom_cod_diagnostico_principal_e_cie11: String,
    #[serde(rename = "codDiagnosticoRelacionadoE1CIE11")]
    pub cod_diagnostico_relacionado_e1_cie11: String,
    #[serde(rename = "nomCodDiagnosticoRelacionadoE1CIE11")]
    pub nom_cod_diagnostico_relacionado_e1_cie11: String,
    #[serde(rename = "codDiagnosticoCausaMuerteCIE11")]
    pub cod_diagnostico_causa_muerte_cie11: String,
    #[serde(rename = "nomCodDiagnosticoCausaMuerteCIE11")]
    pub nom_cod_diagnostico_causa_muerte_cie11: String,
}

/// Arma el AU a partir de las atenciones del periodo, ya traídas de la base.
pub fn documento_au(
    institucion: &FuenteInstitucion,
    atenciones: &[FuenteUrgencia],
    rec: &mut Recolector,
) -> Documento {
    let mut usuarios: Vec<Usuario> = Vec::new();
    let mut indice: Vec<i64> = Vec::new();

    for a in atenciones {
        let pos = match indice.iter().position(|id| *id == a.paciente.id) {
            Some(i) => i,
            None => {
                let u = usuario_base(&a.paciente, usuarios.len() as i64 + 1, rec);
                usuarios.push(u);
                indice.push(a.paciente.id);
                usuarios.len() - 1
            }
        };

        let consecutivo = usuarios[pos].servicios.urgencias.len() as i64 + 1;
        let urgencia = urgencia_de(institucion, a, consecutivo, rec);
        usuarios[pos].servicios.urgencias.push(urgencia);
    }

    Documento {
        num_documento_id_obligado: institucion.nit.clone(),
        num_factura: String::new(),
        tipo_nota: String::new(),
        num_nota: String::new(),
        usuarios,
    }
}

fn urgencia_de(
    institucion: &FuenteInstitucion,
    a: &FuenteUrgencia,
    consecutivo: i64,
    rec: &mut Recolector,
) -> Urgencia {
    let permisivo = rec.permite_supuestos();

    // La causa va como en el AC: con `codigo`, porque el enum del sistema es la
    // tabla ANTERIOR a la 948 entera. El supuesto es el `38` («enfermedad
    // general» en la tabla nueva) y es **el más flojo de todos** — para un
    // ingreso por urgencias borra justo la distinción entre un accidente de
    // trabajo, uno de tránsito y una agresión, que tienen responsables de pago
    // distintos. Sale con su etiqueta en el resumen para que quien firma lo vea.
    let donde_causa =
        Donde::au(a.id, "causa_motivo", "causa o motivo de la atención").supuesto("38");
    let causa_dato = rec.codigo(a.causa_motivo.as_deref(), &donde_causa, |v| {
        if muv::causa_motivo_existe(v) {
            return Ok(());
        }
        Err(format!(
            "«{v}» no está en la tabla de causa o motivo del MUV (21 a 49). El \
             sistema traía la tabla anterior a la 948, donde «13» era enfermedad \
             general; hoy eso es el 38."
        ))
    });
    let causa = if permisivo {
        causa_dato.o_el_supuesto(Some("38"))
    } else {
        causa_dato.o_nada()
    };

    // `"R69"` («causas de morbilidad desconocidas») es un diagnóstico DE VERDAD,
    // inventado para un paciente real. Se declara como supuesto y solo sale si el
    // llamante lo pide.
    let donde_dx = Donde::au(a.id, "diagnostico_ingreso", "diagnóstico de ingreso").supuesto("R69");
    let diagnostico = {
        let d = rec.dato(a.diagnostico_ingreso.as_deref(), &donde_dx);
        if permisivo {
            d.o_el_supuesto(Some("R69"))
        } else {
            d.o_nada()
        }
    };

    // La condición y destino sale del maestro del validador. No hay supuesto: el
    // 01 afirma un alta a domicilio, y ponerlo por descarte es lo que convertía
    // una fuga —o una muerte— en una atención terminada con normalidad.
    let donde_destino = Donde::au(
        a.id,
        "destino_salida",
        "condición y destino del usuario al egreso",
    );
    let condicion_destino = rec
        .dato(
            muv::condicion_destino_de_urgencias(a.destino_salida.as_deref()),
            &donde_destino,
        )
        .o_nada();
    // Un destino que el sistema guarda y el catálogo no traduce merece su propio
    // motivo: «falta el destino» y «el destino es FUGA y el RIPS no tiene código
    // para eso» se corrigen en sitios distintos. El primero, capturándolo; el
    // segundo, preguntándole al prestador qué reporta hoy.
    if condicion_destino.is_none()
        && let Some(d) = a.destino_salida.as_deref().filter(|d| !d.trim().is_empty())
    {
        rec.no_vale(
            &donde_destino,
            format!(
                "«{d}» no tiene código en el catálogo CondicionDestino del \
                 validador. El más parecido por su forma sería el 01, y el 01 \
                 dice que el paciente salió con destino a su domicilio, o sea \
                 que hubo un alta."
            ),
        );
    }

    let (cie_i, nom_i) = cie11::par(a.diagnostico_ingreso.as_deref());
    let (cie_e, nom_e) = cie11::par(a.diagnostico_egreso.as_deref());
    let (cie_m, nom_m) = cie11::par(a.diagnostico_causa_muerte.as_deref());

    Urgencia {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        fecha_inicio_atencion: fecha_dt(&a.fecha_ingreso),
        causa_motivo_atencion: causa,
        cod_diagnostico_principal: diagnostico,
        // R05, el principal DE EGRESO. Estuvo en `codDiagnosticoRelacionadoE1`,
        // que es otra cosa, así que el obligatorio salía vacío y el opcional con
        // un dato que no le tocaba. El MUV se quejó de los dos a la vez.
        cod_diagnostico_principal_e: vacio_a_nulo(&a.diagnostico_egreso),
        // «No hay» se escribe `null`: la cadena vacía no cumple el mínimo de
        // cuatro caracteres que declara la ficha.
        cod_diagnostico_relacionado_e1: None,
        condicion_destino_usuario_egreso: condicion_destino,
        cod_diagnostico_causa_muerte: vacio_a_nulo(&a.diagnostico_causa_muerte),
        fecha_egreso: fecha_dt(&a.fecha_egreso),
        consecutivo,
        cod_diagnostico_principal_cie11: cie_i.to_string(),
        nom_cod_diagnostico_principal_cie11: nom_i.to_string(),
        cod_diagnostico_principal_e_cie11: cie_e.to_string(),
        nom_cod_diagnostico_principal_e_cie11: nom_e.to_string(),
        cod_diagnostico_relacionado_e1_cie11: String::new(),
        nom_cod_diagnostico_relacionado_e1_cie11: String::new(),
        cod_diagnostico_causa_muerte_cie11: cie_m.to_string(),
        nom_cod_diagnostico_causa_muerte_cie11: nom_m.to_string(),
    }
}

/// `""` y `None` son lo mismo aquí, y los dos se escriben `null`.
fn vacio_a_nulo(v: &Option<String>) -> Option<String> {
    v.as_ref().filter(|s| !s.trim().is_empty()).cloned()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveDate;

    fn institucion() -> FuenteInstitucion {
        FuenteInstitucion {
            nit: "900123456".into(),
            codigo_habilitacion: "050010123401".into(),
        }
    }

    fn paciente(id: i64, documento: &str) -> FuentePaciente {
        FuentePaciente {
            id,
            tipo_documento: "CC".into(),
            numero_documento: documento.into(),
            tipo_usuario: Some("01".into()),
            fecha_nacimiento: "1980-05-04".into(),
            sexo: "M".into(),
            cod_pais: Some("170".into()),
            cod_municipio: Some("05001".into()),
            zona: Some("01".into()),
            cod_pais_origen: Some("170".into()),
        }
    }

    fn completa(id: i64, p: FuentePaciente) -> FuenteUrgencia {
        FuenteUrgencia {
            id,
            fecha_ingreso: NaiveDate::from_ymd_opt(2026, 3, 15)
                .unwrap()
                .and_hms_opt(22, 10, 30)
                .unwrap(),
            fecha_egreso: NaiveDate::from_ymd_opt(2026, 3, 16)
                .unwrap()
                .and_hms_opt(2, 45, 0)
                .unwrap(),
            causa_motivo: Some("38".into()),
            diagnostico_ingreso: Some("A000".into()),
            diagnostico_egreso: Some("A000".into()),
            destino_salida: Some("DOMICILIO".into()),
            diagnostico_causa_muerte: None,
            paciente: p,
        }
    }

    #[test]
    fn una_atencion_completa_no_deja_faltas() {
        let mut rec = Recolector::estricto();
        let doc = documento_au(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        assert!(
            rec.faltantes().is_empty(),
            "faltas inesperadas: {:?}",
            rec.faltantes()
        );
        let u = &doc.usuarios[0].servicios.urgencias[0];
        assert_eq!(u.fecha_inicio_atencion, "2026-03-15 22:10", "sin segundos");
        assert_eq!(u.fecha_egreso, "2026-03-16 02:45");
        assert_eq!(u.condicion_destino_usuario_egreso.as_deref(), Some("01"));
        assert_eq!(u.consecutivo, 1);
    }

    #[test]
    fn un_muerto_se_reporta_como_muerto() {
        // El fallo que este port destapó: el mapa del Python emitía «04»
        // (referido a otra institución) para un fallecimiento.
        let mut a = completa(1, paciente(7, "1"));
        a.destino_salida = Some("FALLECIDO".into());
        a.diagnostico_causa_muerte = Some("I219".into());
        let mut rec = Recolector::estricto();
        let doc = documento_au(&institucion(), &[a], &mut rec);
        let u = &doc.usuarios[0].servicios.urgencias[0];
        assert_eq!(u.condicion_destino_usuario_egreso.as_deref(), Some("02"));
        assert_eq!(u.cod_diagnostico_causa_muerte.as_deref(), Some("I219"));
        assert!(!u.cod_diagnostico_causa_muerte_cie11.is_empty(), "y su CIE-11");
        assert!(rec.faltantes().is_empty());
    }

    #[test]
    fn una_fuga_no_pasa_por_un_alta_a_domicilio() {
        let mut a = completa(1, paciente(7, "1"));
        a.destino_salida = Some("FUGA".into());
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_au(&institucion(), &[a], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.urgencias[0].condicion_destino_usuario_egreso,
            None,
            "ni siquiera con supuestos permitidos: el 01 afirmaría un alta"
        );
        let f = rec.faltantes();
        assert_eq!(f.len(), 2, "la ausencia y el motivo de por qué no se puede");
        assert!(f.iter().any(|x| x
            .motivo
            .as_deref()
            .is_some_and(|m| m.contains("no tiene código"))));
    }

    #[test]
    fn el_destino_ausente_y_el_intraducible_se_distinguen() {
        // La pareja del caso de arriba: sin destino se anota UNA falta, sin
        // motivo. Con FUGA se anotan dos, y la segunda dice por qué. Se corrigen
        // en sitios distintos.
        let mut a = completa(1, paciente(7, "1"));
        a.destino_salida = None;
        let mut rec = Recolector::estricto();
        documento_au(&institucion(), &[a], &mut rec);
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert!(f[0].motivo.is_none(), "faltaba, no es que no se pueda traducir");
    }

    #[test]
    fn el_au_no_lleva_documento_del_profesional_ni_plata() {
        // El MUV: «Propiedades no permitidas en la estructura de urgencias».
        // Un campo que SOBRA hace rechazar el archivo entero.
        let mut rec = Recolector::estricto();
        let doc = documento_au(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.urgencias[0]).unwrap();
        for prohibido in [
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "vrServicio",
            "conceptoRecaudo",
            "valorPagoModerador",
            "numFEVPagoModerador",
        ] {
            assert!(
                !json.contains(prohibido),
                "«{prohibido}» no puede ir en el AU: {json}"
            );
        }
    }

    #[test]
    fn el_orden_de_los_campos_es_el_de_la_948() {
        let mut rec = Recolector::estricto();
        let doc = documento_au(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.urgencias[0]).unwrap();
        let orden = [
            "codPrestador",
            "fechaInicioAtencion",
            "causaMotivoAtencion",
            "codDiagnosticoPrincipal",
            "codDiagnosticoPrincipalE",
            "codDiagnosticoRelacionadoE1",
            "condicionDestinoUsuarioEgreso",
            "codDiagnosticoCausaMuerte",
            "fechaEgreso",
            "consecutivo",
            "codDiagnosticoPrincipalCIE11",
        ];
        let mut desde = 0;
        for campo in orden {
            let pos = json[desde..]
                .find(&format!("\"{campo}\""))
                .unwrap_or_else(|| panic!("«{campo}» no está o va fuera de orden en {json}"));
            desde += pos;
        }
    }

    #[test]
    fn dos_atenciones_del_mismo_paciente_van_en_un_solo_usuario() {
        let mut rec = Recolector::estricto();
        let doc = documento_au(
            &institucion(),
            &[completa(1, paciente(7, "1")), completa(2, paciente(7, "1"))],
            &mut rec,
        );
        assert_eq!(doc.usuarios.len(), 1);
        assert_eq!(doc.usuarios[0].servicios.urgencias.len(), 2);
        assert_eq!(doc.usuarios[0].servicios.urgencias[1].consecutivo, 2);
    }

    #[test]
    fn el_au_no_emite_listas_vacias_de_los_otros_archivos() {
        let mut rec = Recolector::estricto();
        let doc = documento_au(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"urgencias\""));
        assert!(!json.contains("\"consultas\""));
        assert!(!json.contains("\"procedimientos\""));
        assert!(!json.contains("\"medicamentos\""));
    }
}
