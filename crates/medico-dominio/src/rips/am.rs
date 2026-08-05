// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! **AM — Archivo de Medicamentos.** Port de `generar_archivo_am` (#67).
//!
//! Lleva las fórmulas del periodo. Es, junto con el AC, el documento que `#67`
//! da por verificado contra dos fuentes, así que la forma que emite el Python es
//! segura de portar.
//!
//! # Los tres campos que no se rellenan, y por qué
//!
//! - **`codTecnologiaSalud` (el CUM)** se valida contra el CatalogoCUMs del
//!   INVIMA ([`crate::cum`]), no solo «que no esté vacío». Un CUM mal digitado
//!   tiene la forma correcta y hace rechazar la factura entera tres semanas
//!   después. `"00000000"` no es un CUM: es un dato falso con forma de dato.
//! - **`unidadMinDispensa` (M13)** sale del catálogo `REFs_UPR`
//!   ([`muv::codigo_presentacion`]). `formula.unidad` es texto libre, así que
//!   cuando lo escrito no es una presentación conocida **no se aproxima a la más
//!   parecida**: se anota. De este campo dependen el valor unitario y la
//!   cantidad, y el MUV solo lo valida por rango (1 a 999) — es el sitio más
//!   fácil para que se cuele un dato falso sin que nadie se entere. Ya pasó:
//!   `tableta = 1` cuando el 1 del catálogo es AMPOLLA.
//! - **`diasTratamiento` (M15)** no se deduce de `dosis`, que es texto libre:
//!   «1 cada 8 horas» no dice cuántos días. Se anota como falta en vez de mandar
//!   un 1, porque los días de tratamiento salen de la prescripción.
//!
//! # La cantidad no se trunca
//!
//! `cantidad` se guarda como número real y el RIPS solo admite enteros. Truncar
//! `2.5` a `2` perdería media dosis en silencio, que en un medicamento es peor
//! que un rechazo. Se emite **tal cual** y se anota con su motivo, para que
//! quien cierra el mes decida si el dato estaba mal registrado.

use serde::Serialize;

use super::documentos::{
    documento_profesional, fecha_d, usuario_base, Documento, FuenteInstitucion, FuentePaciente,
    FuenteProfesional, Usuario,
};
use super::recolector::{Donde, Recolector};
use crate::{cum, muv};

/// Una fórmula ya traída de la base, con su paciente y su profesional.
#[derive(Debug, Clone)]
pub struct FuenteMedicamento {
    /// El id de la FÓRMULA, que es donde hay que ir a corregir.
    pub id: i64,
    /// La fecha de la HISTORIA en que se formuló: es la de la atención, que es
    /// lo que el RIPS reporta.
    pub fecha: chrono::NaiveDate,
    pub codigo_cum: Option<String>,
    pub tipo_medicamento: Option<String>,
    pub medicamento: String,
    /// De la historia: la fórmula no tiene diagnóstico propio, y el de la
    /// atención **es** el contexto clínico de la prescripción (M05).
    pub diagnostico_principal: Option<String>,
    /// Texto libre (`tableta`, `ampolla`…). Se traduce contra `REFs_UPR`.
    pub unidad: Option<String>,
    pub cantidad: Option<f64>,
    pub dias_tratamiento: Option<i64>,
    pub valor_unitario: Option<i64>,
    pub valor_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    pub valor_pago_moderador: Option<i64>,
    pub num_fev_pago_moderador: Option<String>,
    pub paciente: FuentePaciente,
    pub profesional: FuenteProfesional,
}

/// La cantidad dispensada tal y como sale al JSON.
///
/// El MUV rechaza `7.0` donde espera `7` («Existen errores en el formato JSON»),
/// así que el entero tiene que llegar entero al serializador. Y una fracción
/// **no se convierte**: sale como está, para que se vea qué hay que corregir.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Cantidad {
    Entera(i64),
    /// Lo que había, sin truncar. Va acompañada de su falta.
    SinConvertir(f64),
}

/// Un medicamento tal y como sale al JSON de la 948.
///
/// El orden de los campos es el del Python, campo a campo.
#[derive(Debug, Clone, Serialize)]
pub struct Medicamento {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "fechaDispensAdmon")]
    pub fecha_dispens_admon: String,
    #[serde(rename = "idMIPRES")]
    pub id_mipres: String,
    #[serde(rename = "codTecnologiaSalud")]
    pub cod_tecnologia_salud: Option<String>,
    #[serde(rename = "tipoMedicamento")]
    pub tipo_medicamento: Option<String>,
    #[serde(rename = "nomTecnologiaSalud")]
    pub nom_tecnologia_salud: String,
    #[serde(rename = "codDiagnosticoPrincipal")]
    pub cod_diagnostico_principal: Option<String>,
    #[serde(rename = "unidadMinDispensa")]
    pub unidad_min_dispensa: Option<i64>,
    #[serde(rename = "cantidadMedicamento")]
    pub cantidad_medicamento: Option<Cantidad>,
    #[serde(rename = "diasTratamiento")]
    pub dias_tratamiento: Option<i64>,
    // P11/P12: el PROFESIONAL que formuló, no el paciente.
    #[serde(rename = "tipoDocumentoIdentificacion")]
    pub tipo_documento_identificacion: Option<String>,
    #[serde(rename = "numDocumentoIdentificacion")]
    pub num_documento_identificacion: Option<String>,
    #[serde(rename = "vrUnitMedicamento")]
    pub vr_unit_medicamento: Option<i64>,
    #[serde(rename = "vrServicio")]
    pub vr_servicio: Option<i64>,
    #[serde(rename = "conceptoRecaudo")]
    pub concepto_recaudo: Option<String>,
    #[serde(rename = "valorPagoModerador")]
    pub valor_pago_moderador: i64,
    #[serde(rename = "numFEVPagoModerador")]
    pub num_fev_pago_moderador: String,
    pub consecutivo: i64,
}

/// Arma el AM a partir de las fórmulas del periodo, ya traídas de la base.
pub fn documento_am(
    institucion: &FuenteInstitucion,
    formulas: &[FuenteMedicamento],
    rec: &mut Recolector,
) -> Documento {
    let mut usuarios: Vec<Usuario> = Vec::new();
    let mut indice: Vec<i64> = Vec::new();

    for f in formulas {
        let pos = match indice.iter().position(|id| *id == f.paciente.id) {
            Some(i) => i,
            None => {
                let u = usuario_base(&f.paciente, usuarios.len() as i64 + 1, rec);
                usuarios.push(u);
                indice.push(f.paciente.id);
                usuarios.len() - 1
            }
        };

        let consecutivo = usuarios[pos].servicios.medicamentos.len() as i64 + 1;
        let medicamento = medicamento_de(institucion, f, consecutivo, rec);
        usuarios[pos].servicios.medicamentos.push(medicamento);
    }

    Documento {
        num_documento_id_obligado: institucion.nit.clone(),
        num_factura: String::new(),
        tipo_nota: String::new(),
        num_nota: String::new(),
        usuarios,
    }
}

fn medicamento_de(
    institucion: &FuenteInstitucion,
    f: &FuenteMedicamento,
    consecutivo: i64,
    rec: &mut Recolector,
) -> Medicamento {
    let permisivo = rec.permite_supuestos();

    macro_rules! exigir {
        ($valor:expr, $campo:literal, $etiqueta:literal, $supuesto:literal) => {{
            let donde = Donde::am(f.id, $campo, $etiqueta).supuesto($supuesto);
            let d = rec.dato($valor, &donde);
            if permisivo {
                d.o_el_supuesto(Some($supuesto))
            } else {
                d.o_nada()
            }
        }};
        ($valor:expr, $campo:literal, $etiqueta:literal) => {{
            let donde = Donde::am(f.id, $campo, $etiqueta);
            rec.dato($valor, &donde).o_nada()
        }};
    }

    // El CUM se pide con `codigo`, no con `dato`: el campo que la 948 contrasta
    // contra el listado del INVIMA. Un código presente pero inexistente tiene la
    // forma correcta y hace rechazar la factura completa.
    let donde_cum = Donde::am(f.id, "codigo_cum", "CUM del medicamento").supuesto("00000000");
    let cum_dato = rec.codigo(f.codigo_cum.as_deref(), &donde_cum, |v| {
        if cum::existe(Some(v)) {
            return Ok(());
        }
        Err(format!(
            "el código «{v}» no está en el CatalogoCUMs del INVIMA. Revise el \
             dato: el MUV lo valida contra ese listado."
        ))
    });
    // Con supuestos permitidos sale el `00000000`, que es lo que hace el Python.
    // No es un CUM y por eso el MUV lo rechazará — que es exactamente lo que
    // tiene que pasar cuando alguien fuerza un archivo sin el dato.
    let cod_tecnologia = if permisivo {
        cum_dato.o_el_supuesto(Some("00000000"))
    } else {
        cum_dato.o_nada()
    };

    let tipo_medicamento = exigir!(
        f.tipo_medicamento.as_deref(),
        "tipo_medicamento",
        "tipo de medicamento",
        "01"
    );

    // M05. Se toma el de la historia en que se formuló; si tampoco está, se
    // anota. No es un valor de relleno.
    let diagnostico = exigir!(
        f.diagnostico_principal.as_deref(),
        "historia.diagnostico_principal",
        "diagnóstico que motivó la fórmula"
    );

    // M13. `unidad` es texto libre y el catálogo es el que manda: si lo escrito
    // no es una presentación conocida, el campo queda vacío y se anota. Sin
    // supuesto a propósito — de la presentación dependen el valor unitario y la
    // cantidad, y elegir una es afirmar en qué se dispensó.
    let donde_unidad = Donde::am(
        f.id,
        "unidad",
        "unidad mínima de dispensación (código de la 948)",
    );
    let unidad_min = rec
        .numero(muv::codigo_presentacion(f.unidad.as_deref()), &donde_unidad)
        .o_nada();

    let cantidad = cantidad_entera(f, rec);

    let dias = {
        let donde = Donde::am(
            f.id,
            "dias_tratamiento",
            "días de tratamiento de la fórmula",
        );
        rec.numero(f.dias_tratamiento, &donde).o_nada()
    };

    let (tipo_doc, num_doc) = documento_profesional(
        &f.profesional,
        rec,
        Donde::am(
            f.id,
            "medico.tipo_documento",
            "tipo de documento del profesional que atendió",
        ),
        Donde::am(
            f.id,
            "medico.numero_documento",
            "número de documento del profesional que atendió",
        ),
    );

    let donde_unitario = Donde::am(
        f.id,
        "valor_unitario",
        "valor unitario del medicamento",
    )
    .supuesto("0");
    let vr_unitario = {
        let d = rec.numero(f.valor_unitario, &donde_unitario);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };

    let donde_valor = Donde::am(f.id, "valor_servicio", "valor del servicio").supuesto("0");
    let vr_servicio = {
        let d = rec.numero(f.valor_servicio, &donde_valor);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };
    let concepto = exigir!(
        f.concepto_recaudo.as_deref(),
        "concepto_recaudo",
        "concepto de recaudo",
        "05"
    );

    Medicamento {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        fecha_dispens_admon: fecha_d(&f.fecha),
        id_mipres: String::new(),
        cod_tecnologia_salud: cod_tecnologia,
        tipo_medicamento,
        nom_tecnologia_salud: f.medicamento.clone(),
        cod_diagnostico_principal: diagnostico,
        unidad_min_dispensa: unidad_min,
        cantidad_medicamento: cantidad,
        dias_tratamiento: dias,
        tipo_documento_identificacion: tipo_doc,
        num_documento_identificacion: num_doc,
        vr_unit_medicamento: vr_unitario,
        vr_servicio,
        concepto_recaudo: concepto,
        valor_pago_moderador: f.valor_pago_moderador.unwrap_or(0),
        num_fev_pago_moderador: f.num_fev_pago_moderador.clone().unwrap_or_default(),
        consecutivo,
    }
}

/// La cantidad dispensada, como entero. Ver la cabecera del módulo.
fn cantidad_entera(f: &FuenteMedicamento, rec: &mut Recolector) -> Option<Cantidad> {
    let donde = Donde::am(f.id, "cantidad", "cantidad del medicamento");
    let Some(n) = f.cantidad else {
        rec.dato(None, &donde);
        return None;
    };
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        return Some(Cantidad::Entera(n as i64));
    }
    rec.no_vale(
        &donde,
        format!(
            "«{n}» no es entera y el RIPS solo admite enteros. Truncarla perdería \
             parte de la dosis en silencio."
        ),
    );
    Some(Cantidad::SinConvertir(n))
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

    /// Un CUM que SÍ está en el catálogo, leído del propio artefacto: escribir
    /// uno a mano validaría contra una aguja inventada.
    fn cum_del_catalogo() -> String {
        crate::cum::normalizar(Some(
            include_str!("../../datos/cum_codigos.txt")
                .lines()
                .next()
                .unwrap(),
        ))
    }

    fn completa(id: i64, p: FuentePaciente) -> FuenteMedicamento {
        FuenteMedicamento {
            id,
            fecha: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
            codigo_cum: Some(cum_del_catalogo()),
            tipo_medicamento: Some("01".into()),
            medicamento: "Acetaminofén 500 mg".into(),
            diagnostico_principal: Some("A000".into()),
            unidad: Some("tableta".into()),
            cantidad: Some(30.0),
            dias_tratamiento: Some(10),
            valor_unitario: Some(300),
            valor_servicio: Some(9_000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: None,
            num_fev_pago_moderador: None,
            paciente: p,
            profesional: FuenteProfesional {
                tipo_documento: Some("CC".into()),
                numero_documento: Some("71234567".into()),
            },
        }
    }

    #[test]
    fn una_formula_completa_no_deja_faltas() {
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        assert!(
            rec.faltantes().is_empty(),
            "faltas inesperadas: {:?}",
            rec.faltantes()
        );
        let s = &doc.usuarios[0].servicios.medicamentos[0];
        assert_eq!(s.fecha_dispens_admon, "2026-03-15");
        assert_eq!(s.nom_tecnologia_salud, "Acetaminofén 500 mg");
        assert_eq!(s.cantidad_medicamento, Some(Cantidad::Entera(30)));
        assert_eq!(s.consecutivo, 1);
    }

    #[test]
    fn un_cum_que_no_existe_se_devuelve_tal_cual_y_se_anota() {
        // La pareja del catálogo: el bueno pasa (arriba), el inventado no.
        let mut f = completa(1, paciente(7, "1"));
        f.codigo_cum = Some("00000000".into());
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[f], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.medicamentos[0]
                .cod_tecnologia_salud
                .as_deref(),
            Some("00000000"),
            "se devuelve TAL CUAL: sustituirlo escondería lo que hay en la base"
        );
        let faltas = rec.faltantes();
        assert_eq!(faltas.len(), 1);
        assert!(faltas[0].motivo.as_deref().unwrap().contains("CatalogoCUMs"));
    }

    #[test]
    fn sin_cum_el_campo_va_vacio() {
        let mut f = completa(1, paciente(7, "1"));
        f.codigo_cum = None;
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[f], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.medicamentos[0].cod_tecnologia_salud,
            None
        );
        let faltas = rec.faltantes();
        assert_eq!(faltas.len(), 1, "una falta por la ausencia, no dos");
        assert!(faltas[0].motivo.is_none(), "faltaba, no estaba mal");
    }

    #[test]
    fn la_presentacion_desconocida_no_se_aproxima_a_la_parecida() {
        // `unidad` trae «unidad» por defecto en el sistema viejo, y «unidad» no
        // es ninguna presentación del catálogo.
        let mut f = completa(1, paciente(7, "1"));
        f.unidad = Some("unidad".into());
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[f], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.medicamentos[0].unidad_min_dispensa,
            None,
            "no se manda un 1, que en el catálogo es AMPOLLA"
        );
        let faltas = rec.faltantes();
        assert_eq!(faltas.len(), 1);
        assert_eq!(faltas[0].campo, "unidad");
        assert_eq!(faltas[0].supuesto, None, "de este campo no hay supuesto");
    }

    #[test]
    fn la_presentacion_conocida_sale_con_el_codigo_del_catalogo() {
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.medicamentos[0].unidad_min_dispensa,
            muv::codigo_presentacion(Some("tableta")),
        );
        assert!(doc.usuarios[0].servicios.medicamentos[0]
            .unidad_min_dispensa
            .is_some());
    }

    #[test]
    fn media_dosis_no_se_trunca() {
        let mut f = completa(1, paciente(7, "1"));
        f.cantidad = Some(2.5);
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[f], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.medicamentos[0].cantidad_medicamento,
            Some(Cantidad::SinConvertir(2.5)),
            "sale tal cual; truncar a 2 perdería media dosis en silencio"
        );
        let faltas = rec.faltantes();
        assert_eq!(faltas.len(), 1);
        assert!(faltas[0].motivo.as_deref().unwrap().contains("no es entera"));
    }

    #[test]
    fn la_cantidad_entera_va_sin_punto_decimal() {
        // El MUV rechaza `7.0` donde espera `7`.
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.medicamentos[0]).unwrap();
        assert!(json.contains("\"cantidadMedicamento\":30"), "{json}");
        assert!(!json.contains("30.0"));
    }

    #[test]
    fn sin_dias_de_tratamiento_no_se_manda_un_uno() {
        let mut f = completa(1, paciente(7, "1"));
        f.dias_tratamiento = None;
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_am(&institucion(), &[f], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.medicamentos[0].dias_tratamiento, None,
            "ni con supuestos permitidos: los días salen de la prescripción"
        );
        assert_eq!(rec.faltantes().len(), 1);
    }

    #[test]
    fn dos_formulas_del_mismo_paciente_van_en_un_solo_usuario() {
        let mut rec = Recolector::estricto();
        let doc = documento_am(
            &institucion(),
            &[completa(1, paciente(7, "1")), completa(2, paciente(7, "1"))],
            &mut rec,
        );
        assert_eq!(doc.usuarios.len(), 1);
        assert_eq!(doc.usuarios[0].servicios.medicamentos.len(), 2);
        assert_eq!(doc.usuarios[0].servicios.medicamentos[1].consecutivo, 2);
    }

    #[test]
    fn el_am_no_emite_listas_vacias_de_los_otros_archivos() {
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"medicamentos\""));
        assert!(!json.contains("\"consultas\""));
        assert!(!json.contains("\"procedimientos\""));
    }

    #[test]
    fn el_orden_de_los_campos_es_el_de_la_948() {
        let mut rec = Recolector::estricto();
        let doc = documento_am(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.medicamentos[0]).unwrap();
        let orden = [
            "codPrestador",
            "fechaDispensAdmon",
            "idMIPRES",
            "codTecnologiaSalud",
            "tipoMedicamento",
            "nomTecnologiaSalud",
            "codDiagnosticoPrincipal",
            "unidadMinDispensa",
            "cantidadMedicamento",
            "diasTratamiento",
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "vrUnitMedicamento",
            "vrServicio",
            "conceptoRecaudo",
            "valorPagoModerador",
            "numFEVPagoModerador",
            "consecutivo",
        ];
        let mut desde = 0;
        for campo in orden {
            let pos = json[desde..]
                .find(&format!("\"{campo}\""))
                .unwrap_or_else(|| panic!("«{campo}» no está o va fuera de orden en {json}"));
            desde += pos;
        }
    }
}
