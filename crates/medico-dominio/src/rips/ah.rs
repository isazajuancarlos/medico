// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! **AH — Archivo de Hospitalización.** Port de `generar_archivo_ah` (#67).
//!
//! Obligatorio para toda IPS con servicio de hospitalización habilitado (Res.
//! 2275/2023). Cierra los cinco documentos del RIPS en Rust.
//!
//! # Dos mapas escritos a mano, y los dos emitían
//!
//! A diferencia del AU —donde cuatro de seis claves no casaban y salían como
//! falta—, aquí las claves **sí casan** (`via_ingreso` y `estado` se guardan en
//! mayúsculas), así que el archivo salía con los códigos equivocados puestos.
//! Medido el 2026-08-04 contra los maestros del validador:
//!
//! | campo | lo que emitía | lo que es |
//! |---|---|---|
//! | vía `URGENCIAS` | `02` | DERIVADO DE CONSULTA EXTERNA (urgencias es el 03) |
//! | vía `PROGRAMADA` | `03` | DERIVADO DE URGENCIAS (programada es el 02) |
//! | vía `REMISION` | `04` | DERIVADO DE HOSPITALIZACION, y **no aplica al AH** |
//! | egreso `REMISION` | `03` | DERIVADO A OTRO SERVICIO, y **no aplica al AH** |
//! | egreso `DEFUNCION` | `04` | REFERIDO A OTRA INSTITUCION |
//!
//! Las dos primeras están **intercambiadas**: quien entró por urgencias se
//! reportaba como ingreso programado y al revés. Las dos marcadas «no aplica»
//! hacen que el MUV rechace el archivo. Y la última es la misma inversión del
//! AU: **una defunción reportada como un traslado**.
//!
//! # Ni documento del profesional ni bloque económico
//!
//! Igual que el AU, y por lo mismo: «Propiedades no permitidas en la estructura
//! de hospitalización». Un campo que sobra hace rechazar el archivo entero.
//!
//! # Las hospitalizaciones ACTIVAS no entran
//!
//! No hay estancia que facturar todavía. El código viejo lo sabía —su mapa de
//! estados tenía un `ACTIVA: "00"` con el comentario «no debería reportarse»— y
//! las reportaba igual, con el egreso puesto en el instante de la generación.

use serde::Serialize;

use super::documentos::{
    fecha_dt, usuario_base, Documento, FuenteInstitucion, FuentePaciente, Usuario,
};
use super::recolector::{Donde, Recolector};
use crate::{cie11, muv};

/// Una hospitalización ya traída de la base, con su paciente.
///
/// **No lleva profesional**: el AH no admite los campos P11/P12.
#[derive(Debug, Clone)]
pub struct FuenteHospitalizacion {
    pub id: i64,
    pub fecha_ingreso: chrono::NaiveDateTime,
    /// Las abiertas no llegan hasta aquí: las descarta la consulta.
    pub fecha_egreso: chrono::NaiveDateTime,
    /// El vocabulario del sistema (`URGENCIAS`, `PROGRAMADA`, `REMISION`).
    pub via_ingreso: Option<String>,
    /// El vocabulario del sistema (`ALTA_MEDICA`, `DEFUNCION`…).
    pub estado: Option<String>,
    pub causa_motivo: Option<String>,
    pub diagnostico_ingreso: Option<String>,
    pub diagnostico_egreso: Option<String>,
    pub diagnostico_causa_muerte: Option<String>,
    pub paciente: FuentePaciente,
}

/// Una hospitalización tal y como sale al JSON de la 948.
#[derive(Debug, Clone, Serialize)]
pub struct Hospitalizacion {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "viaIngresoServicioSalud")]
    pub via_ingreso_servicio_salud: Option<String>,
    #[serde(rename = "fechaInicioAtencion")]
    pub fecha_inicio_atencion: String,
    #[serde(rename = "numAutorizacion")]
    pub num_autorizacion: String,
    #[serde(rename = "causaMotivoAtencion")]
    pub causa_motivo_atencion: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipal")]
    pub cod_diagnostico_principal: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipalE")]
    pub cod_diagnostico_principal_e: Option<String>,
    #[serde(rename = "codDiagnosticoRelacionadoE1")]
    pub cod_diagnostico_relacionado_e1: Option<String>,
    #[serde(rename = "codComplicacion")]
    pub cod_complicacion: Option<String>,
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
    #[serde(rename = "codComplicacionCIE11")]
    pub cod_complicacion_cie11: String,
    #[serde(rename = "nomCodComplicacionCIE11")]
    pub nom_cod_complicacion_cie11: String,
    #[serde(rename = "codDiagnosticoCausaMuerteCIE11")]
    pub cod_diagnostico_causa_muerte_cie11: String,
    #[serde(rename = "nomCodDiagnosticoCausaMuerteCIE11")]
    pub nom_cod_diagnostico_causa_muerte_cie11: String,
}

/// Arma el AH a partir de las hospitalizaciones del periodo.
pub fn documento_ah(
    institucion: &FuenteInstitucion,
    hospitalizaciones: &[FuenteHospitalizacion],
    rec: &mut Recolector,
) -> Documento {
    let mut usuarios: Vec<Usuario> = Vec::new();
    let mut indice: Vec<i64> = Vec::new();

    for h in hospitalizaciones {
        let pos = match indice.iter().position(|id| *id == h.paciente.id) {
            Some(i) => i,
            None => {
                let u = usuario_base(&h.paciente, usuarios.len() as i64 + 1, rec);
                usuarios.push(u);
                indice.push(h.paciente.id);
                usuarios.len() - 1
            }
        };

        let consecutivo = usuarios[pos].servicios.hospitalizacion.len() as i64 + 1;
        let fila = hospitalizacion_de(institucion, h, consecutivo, rec);
        usuarios[pos].servicios.hospitalizacion.push(fila);
    }

    Documento {
        num_documento_id_obligado: institucion.nit.clone(),
        num_factura: String::new(),
        tipo_nota: String::new(),
        num_nota: String::new(),
        usuarios,
    }
}

fn hospitalizacion_de(
    institucion: &FuenteInstitucion,
    h: &FuenteHospitalizacion,
    consecutivo: i64,
    rec: &mut Recolector,
) -> Hospitalizacion {
    let permisivo = rec.permite_supuestos();

    // La vía sale del maestro. **Sin supuesto**: el `03` que el Python ponía por
    // descarte afirma que el paciente entró por urgencias, y de eso dependen el
    // pagador y la tarifa.
    let donde_via = Donde::ah(
        h.id,
        "via_ingreso",
        "vía de ingreso a la hospitalización",
    );
    let via = rec
        .dato(
            muv::via_ingreso_de_hospitalizacion(h.via_ingreso.as_deref()),
            &donde_via,
        )
        .o_nada();
    anotar_si_no_se_traduce(rec, &donde_via, h.via_ingreso.as_deref(), &via, "vía de ingreso");

    // La causa va como en el AC y el AU: con `codigo`, contra la tabla del MUV.
    // Iba QUEMADA en «13», y con eso ninguna hospitalización por accidente de
    // tránsito se cobraba al SOAT.
    let donde_causa =
        Donde::ah(h.id, "causa_motivo", "causa o motivo de la atención").supuesto("38");
    let causa_dato = rec.codigo(h.causa_motivo.as_deref(), &donde_causa, |v| {
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

    // `"Z00"` es «examen general», y además tiene tres caracteres donde el RIPS
    // exige cuatro. Un ingreso hospitalario por examen general no existe.
    let donde_dx = Donde::ah(h.id, "diagnostico_ingreso", "diagnóstico de ingreso");
    let diagnostico = rec.dato(h.diagnostico_ingreso.as_deref(), &donde_dx).o_nada();

    // La condición de egreso, del maestro y con la columna AplicaAH.
    let donde_estado = Donde::ah(h.id, "estado", "condición y destino al egreso");
    let condicion = rec
        .dato(
            muv::condicion_destino_de_hospitalizacion(h.estado.as_deref()),
            &donde_estado,
        )
        .o_nada();
    anotar_si_no_se_traduce(
        rec,
        &donde_estado,
        h.estado.as_deref(),
        &condicion,
        "condición de egreso",
    );

    let (cie_i, nom_i) = cie11::par(h.diagnostico_ingreso.as_deref());
    let (cie_e, nom_e) = cie11::par(h.diagnostico_egreso.as_deref());
    let (cie_m, nom_m) = cie11::par(h.diagnostico_causa_muerte.as_deref());

    Hospitalizacion {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        via_ingreso_servicio_salud: via,
        fecha_inicio_atencion: fecha_dt(&h.fecha_ingreso),
        num_autorizacion: String::new(),
        causa_motivo_atencion: causa,
        cod_diagnostico_principal: diagnostico,
        cod_diagnostico_principal_e: vacio_a_nulo(&h.diagnostico_egreso),
        cod_diagnostico_relacionado_e1: None,
        // Ninguna pantalla registra complicaciones, y la 948 la declara
        // opcional: «no hubo» es la respuesta normal.
        cod_complicacion: None,
        condicion_destino_usuario_egreso: condicion,
        cod_diagnostico_causa_muerte: vacio_a_nulo(&h.diagnostico_causa_muerte),
        fecha_egreso: fecha_dt(&h.fecha_egreso),
        consecutivo,
        cod_diagnostico_principal_cie11: cie_i.to_string(),
        nom_cod_diagnostico_principal_cie11: nom_i.to_string(),
        cod_diagnostico_principal_e_cie11: cie_e.to_string(),
        nom_cod_diagnostico_principal_e_cie11: nom_e.to_string(),
        cod_diagnostico_relacionado_e1_cie11: String::new(),
        nom_cod_diagnostico_relacionado_e1_cie11: String::new(),
        cod_complicacion_cie11: String::new(),
        nom_cod_complicacion_cie11: String::new(),
        cod_diagnostico_causa_muerte_cie11: cie_m.to_string(),
        nom_cod_diagnostico_causa_muerte_cie11: nom_m.to_string(),
    }
}

/// Distingue «falta el dato» de «el dato está y el catálogo no lo traduce».
///
/// Se corrigen en sitios distintos: el primero capturándolo, el segundo
/// preguntando qué se reporta para ese caso. Sin esta separación, un estado que
/// el catálogo no cubre se lee como un olvido de captura.
fn anotar_si_no_se_traduce(
    rec: &mut Recolector,
    donde: &Donde,
    guardado: Option<&str>,
    traducido: &Option<String>,
    que: &str,
) {
    if traducido.is_none()
        && let Some(v) = guardado.filter(|v| !v.trim().is_empty())
    {
        rec.no_vale(
            donde,
            format!("«{v}» no tiene código de {que} en el catálogo del validador."),
        );
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

    fn completa(id: i64, p: FuentePaciente) -> FuenteHospitalizacion {
        FuenteHospitalizacion {
            id,
            fecha_ingreso: NaiveDate::from_ymd_opt(2026, 3, 10)
                .unwrap()
                .and_hms_opt(8, 0, 0)
                .unwrap(),
            fecha_egreso: NaiveDate::from_ymd_opt(2026, 3, 14)
                .unwrap()
                .and_hms_opt(11, 30, 0)
                .unwrap(),
            via_ingreso: Some("URGENCIAS".into()),
            estado: Some("ALTA_MEDICA".into()),
            causa_motivo: Some("38".into()),
            diagnostico_ingreso: Some("A000".into()),
            diagnostico_egreso: Some("A000".into()),
            diagnostico_causa_muerte: None,
            paciente: p,
        }
    }

    #[test]
    fn una_hospitalizacion_completa_no_deja_faltas() {
        let mut rec = Recolector::estricto();
        let doc = documento_ah(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        assert!(
            rec.faltantes().is_empty(),
            "faltas inesperadas: {:?}",
            rec.faltantes()
        );
        let h = &doc.usuarios[0].servicios.hospitalizacion[0];
        assert_eq!(h.fecha_inicio_atencion, "2026-03-10 08:00");
        assert_eq!(h.fecha_egreso, "2026-03-14 11:30");
        assert_eq!(h.consecutivo, 1);
    }

    #[test]
    fn quien_entro_por_urgencias_no_se_reporta_como_programado() {
        // El mapa del Python los tenía INTERCAMBIADOS.
        let mut rec = Recolector::estricto();
        let mut programada = completa(2, paciente(8, "2"));
        programada.via_ingreso = Some("PROGRAMADA".into());
        let doc = documento_ah(
            &institucion(),
            &[completa(1, paciente(7, "1")), programada],
            &mut rec,
        );
        assert_eq!(
            doc.usuarios[0].servicios.hospitalizacion[0]
                .via_ingreso_servicio_salud
                .as_deref(),
            Some("03"),
            "urgencias es DERIVADO DE URGENCIAS"
        );
        assert_eq!(
            doc.usuarios[1].servicios.hospitalizacion[0]
                .via_ingreso_servicio_salud
                .as_deref(),
            Some("02"),
            "programada es DERIVADO DE CONSULTA EXTERNA"
        );
        assert!(rec.faltantes().is_empty());
    }

    #[test]
    fn una_defuncion_se_reporta_como_muerte() {
        let mut h = completa(1, paciente(7, "1"));
        h.estado = Some("DEFUNCION".into());
        h.diagnostico_causa_muerte = Some("I219".into());
        let mut rec = Recolector::estricto();
        let doc = documento_ah(&institucion(), &[h], &mut rec);
        let f = &doc.usuarios[0].servicios.hospitalizacion[0];
        assert_eq!(f.condicion_destino_usuario_egreso.as_deref(), Some("02"));
        assert_eq!(f.cod_diagnostico_causa_muerte.as_deref(), Some("I219"));
        assert!(!f.cod_diagnostico_causa_muerte_cie11.is_empty());
        assert!(rec.faltantes().is_empty());
    }

    #[test]
    fn un_estado_que_el_catalogo_no_cubre_se_anota_con_su_motivo() {
        // La pareja: `ACTIVA` no se traduce (y no debería llegar aquí, porque la
        // consulta la descarta), y se distingue de «falta el estado».
        let mut viva = completa(1, paciente(7, "1"));
        viva.estado = Some("ACTIVA".into());
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ah(&institucion(), &[viva], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.hospitalizacion[0].condicion_destino_usuario_egreso,
            None
        );
        assert_eq!(rec.faltantes().len(), 2, "la ausencia y el motivo");

        let mut sin = completa(1, paciente(7, "1"));
        sin.estado = None;
        let mut rec = Recolector::estricto();
        documento_ah(&institucion(), &[sin], &mut rec);
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert!(f[0].motivo.is_none(), "faltaba, no es que no se traduzca");
    }

    #[test]
    fn el_ah_no_lleva_documento_del_profesional_ni_plata() {
        let mut rec = Recolector::estricto();
        let doc = documento_ah(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.hospitalizacion[0]).unwrap();
        for prohibido in [
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "vrServicio",
            "conceptoRecaudo",
            "valorPagoModerador",
            "numFEVPagoModerador",
            // La 948 no tiene este campo: la estancia se deduce de las dos
            // fechas, que sí van en el archivo.
            "diasEstancia",
        ] {
            assert!(!json.contains(prohibido), "«{prohibido}» no va en el AH: {json}");
        }
    }

    #[test]
    fn el_orden_de_los_campos_es_el_de_la_948() {
        let mut rec = Recolector::estricto();
        let doc = documento_ah(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.hospitalizacion[0]).unwrap();
        let orden = [
            "codPrestador",
            "viaIngresoServicioSalud",
            "fechaInicioAtencion",
            "numAutorizacion",
            "causaMotivoAtencion",
            "codDiagnosticoPrincipal",
            "codDiagnosticoPrincipalE",
            "codDiagnosticoRelacionadoE1",
            "codComplicacion",
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
    fn el_ah_no_emite_listas_vacias_de_los_otros_archivos() {
        let mut rec = Recolector::estricto();
        let doc = documento_ah(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"hospitalizacion\""));
        for otro in ["consultas", "procedimientos", "medicamentos", "urgencias"] {
            assert!(!json.contains(&format!("\"{otro}\"")), "{otro}");
        }
    }
}
