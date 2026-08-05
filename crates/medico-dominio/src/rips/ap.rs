// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! **AP — Archivo de Procedimientos.** Port de `generar_archivo_ap` (#67).
//!
//! Lleva laboratorios, imágenes diagnósticas, terapias y cirugías: todo lo que
//! en este sistema es una `orden` colgada de una historia.
//!
//! # Lo que se corrige al portar, en vez de replicarlo
//!
//! **Los seis campos CIE-11 salían siempre vacíos.** El Python los deriva de
//! `getattr(orden, 'diagnostico_principal', None)`, y `Orden` **no tiene** ese
//! campo —los diagnósticos viven en la historia—, así que el `getattr` devuelve
//! `None` en toda ejecución y el par sale `("", "")`. Mientras tanto,
//! `codDiagnosticoPrincipal` (el CIE-10) sí se toma de la historia. O sea que el
//! archivo decía el diagnóstico en un campo y lo callaba en el de al lado.
//!
//! No es una decisión que replicar: es un `getattr` sobre el objeto equivocado.
//! Aquí el CIE-11 se deriva **del mismo diagnóstico que se emite** como CIE-10,
//! que es lo único que puede significar ese par. Derivar no es inventar:
//! [`cie11::par`] solo devuelve algo cuando la equivalencia es exacta y única, y
//! deja el campo vacío en cuanto hay dos destinos posibles.
//!
//! El Python se corrigió en la misma pasada (`backend/rips_completo.py`), para
//! que los dos sigan pudiendo compararse byte a byte — que es la condición del
//! ADR para apagarlo.
//!
//! # La modalidad se DERIVA del tipo de orden, y eso es legítimo
//!
//! Un laboratorio es apoyo diagnóstico por definición, así que
//! `modalidadGrupoServicioTecSal` sale de `orden.tipo` sin preguntarle a nadie.
//! Lo que no se puede es rellenar el que no encaje: un tipo que no esté en el
//! mapa se anota como falta, no cae al `02` por descarte.

use serde::Serialize;

use super::documentos::{
    documento_profesional, fecha_dt, usuario_base, Documento, FuenteInstitucion, FuentePaciente,
    FuenteProfesional, Usuario,
};
use super::recolector::{Donde, Recolector};
use crate::{cie11, muv};

/// Un procedimiento ya traído de la base, con su paciente y su profesional.
///
/// Los campos que la `orden` todavía no captura llegan en `None` y el recolector
/// los anota: es la misma postura del AC, y la razón por la que este módulo
/// existe. Ninguno se rellena aquí.
#[derive(Debug, Clone)]
pub struct FuenteProcedimiento {
    /// El id de la ORDEN, que es donde hay que ir a corregir.
    pub id: i64,
    /// La fecha de la HISTORIA, no la de la orden: es la de la atención, que es
    /// lo que el RIPS reporta y lo que el Python emite.
    pub fecha: chrono::NaiveDateTime,
    pub cod_procedimiento_cups: Option<String>,
    /// `Laboratorio`, `Imagenología`, `Cirugía`… De él sale la modalidad.
    pub tipo: String,
    pub via_ingreso_servicio: Option<String>,
    pub grupo_servicios: Option<String>,
    pub cod_servicio: Option<i64>,
    /// De la historia: la orden no tiene finalidad propia.
    pub finalidad: Option<String>,
    pub diagnostico_principal: Option<String>,
    pub diagnostico_relacionado: Option<String>,
    pub valor_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    pub valor_pago_moderador: Option<i64>,
    pub num_fev_pago_moderador: Option<String>,
    pub paciente: FuentePaciente,
    pub profesional: FuenteProfesional,
}

/// Un procedimiento tal y como sale al JSON de la 948.
///
/// El orden de los campos es el del Python, campo a campo.
#[derive(Debug, Clone, Serialize)]
pub struct Procedimiento {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "fechaInicioAtencion")]
    pub fecha_inicio_atencion: String,
    #[serde(rename = "idMIPRES")]
    pub id_mipres: String,
    #[serde(rename = "numAutorizacion")]
    pub num_autorizacion: String,
    #[serde(rename = "codProcedimiento")]
    pub cod_procedimiento: Option<String>,
    #[serde(rename = "viaIngresoServicioSalud")]
    pub via_ingreso_servicio_salud: Option<String>,
    #[serde(rename = "modalidadGrupoServicioTecSal")]
    pub modalidad_grupo_servicio_tec_sal: Option<String>,
    #[serde(rename = "grupoServicios")]
    pub grupo_servicios: Option<String>,
    #[serde(rename = "codServicio")]
    pub cod_servicio: Option<i64>,
    #[serde(rename = "finalidadTecnologiaSalud")]
    pub finalidad_tecnologia_salud: Option<String>,
    // P11/P12: el PROFESIONAL que ordenó o realizó, no el paciente.
    #[serde(rename = "tipoDocumentoIdentificacion")]
    pub tipo_documento_identificacion: Option<String>,
    #[serde(rename = "numDocumentoIdentificacion")]
    pub num_documento_identificacion: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipal")]
    pub cod_diagnostico_principal: Option<String>,
    #[serde(rename = "codDiagnosticoRelacionado")]
    pub cod_diagnostico_relacionado: Option<String>,
    #[serde(rename = "codComplicacion")]
    pub cod_complicacion: Option<String>,
    #[serde(rename = "vrServicio")]
    pub vr_servicio: Option<i64>,
    #[serde(rename = "conceptoRecaudo")]
    pub concepto_recaudo: Option<String>,
    #[serde(rename = "valorPagoModerador")]
    pub valor_pago_moderador: i64,
    #[serde(rename = "numFEVPagoModerador")]
    pub num_fev_pago_moderador: String,
    pub consecutivo: i64,
    #[serde(rename = "codDiagnosticoPrincipalCIE11")]
    pub cod_diagnostico_principal_cie11: String,
    #[serde(rename = "nomCodDiagnosticoPrincipalCIE11")]
    pub nom_cod_diagnostico_principal_cie11: String,
    #[serde(rename = "codDiagnosticoRelacionadoCIE11")]
    pub cod_diagnostico_relacionado_cie11: String,
    #[serde(rename = "nomCodDiagnosticoRelacionadoCIE11")]
    pub nom_cod_diagnostico_relacionado_cie11: String,
    #[serde(rename = "codComplicacionCIE11")]
    pub cod_complicacion_cie11: String,
    #[serde(rename = "nomCodComplicacionCIE11")]
    pub nom_cod_complicacion_cie11: String,
}

/// La modalidad que corresponde a cada tipo de orden.
///
/// Va como tabla y no como `match` con brazo comodín a propósito: un tipo nuevo
/// **no** cae al `02` por descarte, se queda sin modalidad y el recolector lo
/// dice. Los seis nombres son los del `CHECK` de la migración `0027`.
fn modalidad_de(tipo: &str) -> Option<&'static str> {
    match tipo {
        "Laboratorio" | "Imagenología" | "Terapia" | "Otro" => Some("02"),
        "Remisión Especialista" | "Cirugía" => Some("01"),
        _ => None,
    }
}

/// Arma el AP a partir de los procedimientos del periodo, ya traídos de la base.
///
/// Agrupa por paciente igual que el AC: un usuario aparece una vez con todos sus
/// procedimientos, y el `consecutivo` es su posición dentro de ese usuario.
pub fn documento_ap(
    institucion: &FuenteInstitucion,
    procedimientos: &[FuenteProcedimiento],
    rec: &mut Recolector,
) -> Documento {
    let mut usuarios: Vec<Usuario> = Vec::new();
    let mut indice: Vec<i64> = Vec::new();

    for p in procedimientos {
        let pos = match indice.iter().position(|id| *id == p.paciente.id) {
            Some(i) => i,
            None => {
                let u = usuario_base(&p.paciente, usuarios.len() as i64 + 1, rec);
                usuarios.push(u);
                indice.push(p.paciente.id);
                usuarios.len() - 1
            }
        };

        let consecutivo = usuarios[pos].servicios.procedimientos.len() as i64 + 1;
        let procedimiento = procedimiento_de(institucion, p, consecutivo, rec);
        usuarios[pos].servicios.procedimientos.push(procedimiento);
    }

    Documento {
        num_documento_id_obligado: institucion.nit.clone(),
        num_factura: String::new(),
        tipo_nota: String::new(),
        num_nota: String::new(),
        usuarios,
    }
}

fn procedimiento_de(
    institucion: &FuenteInstitucion,
    p: &FuenteProcedimiento,
    consecutivo: i64,
    rec: &mut Recolector,
) -> Procedimiento {
    let permisivo = rec.permite_supuestos();

    macro_rules! exigir {
        ($valor:expr, $campo:literal, $etiqueta:literal, $supuesto:literal) => {{
            let donde = Donde::ap(p.id, $campo, $etiqueta).supuesto($supuesto);
            let d = rec.dato($valor, &donde);
            if permisivo {
                d.o_el_supuesto(Some($supuesto))
            } else {
                d.o_nada()
            }
        }};
        ($valor:expr, $campo:literal, $etiqueta:literal) => {{
            let donde = Donde::ap(p.id, $campo, $etiqueta);
            rec.dato($valor, &donde).o_nada()
        }};
    }

    // `"999999"` no es un CUPS: es un procedimiento que no existe. El validador
    // lo rechaza y, si no lo rechazara, sería peor.
    let cod_procedimiento = exigir!(
        p.cod_procedimiento_cups.as_deref(),
        "codigo_cups",
        "CUPS del procedimiento",
        "999999"
    );
    let via = exigir!(
        p.via_ingreso_servicio.as_deref(),
        "via_ingreso_servicio",
        "vía de ingreso al servicio",
        "01"
    );
    let modalidad = exigir!(
        modalidad_de(&p.tipo),
        "tipo",
        "modalidad del procedimiento (sale del tipo de orden)",
        "02"
    );
    let grupo = exigir!(
        p.grupo_servicios.as_deref(),
        "grupo_servicios",
        "grupo de servicios",
        "02"
    );

    let donde_servicio = Donde::ap(p.id, "cod_servicio", "código de servicio").supuesto("0");
    let cod_servicio = {
        let d = rec.numero(p.cod_servicio, &donde_servicio);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };

    // Igual que en el AC: la finalidad no se pide con `dato` sino con `codigo`,
    // porque el problema no es que falte —el defecto de la base la llenaba— sino
    // que el `10` que ponía no existe en la tabla del MUV. El supuesto del AP es
    // el `44`, que es el que usa el ejemplo de la norma en procedimientos.
    let donde_finalidad =
        Donde::ap(p.id, "finalidad", "finalidad del procedimiento").supuesto("44");
    let finalidad_dato = rec.codigo(p.finalidad.as_deref(), &donde_finalidad, |v| {
        if muv::finalidad_existe(v) {
            return Ok(());
        }
        Err(if v == "10" {
            "«10» es el defecto del sistema, no una finalidad que alguien haya \
             elegido, y la tabla del MUV va de 11 a 44."
                .to_string()
        } else {
            format!(
                "«{v}» es de la tabla de finalidades ANTERIOR a la Resolución 948; \
                 el MUV solo admite de 11 a 44. Hay que convertirlo, no enviarlo."
            )
        })
    });
    let finalidad = if permisivo {
        finalidad_dato.o_el_supuesto(Some("44"))
    } else {
        finalidad_dato.o_nada()
    };

    let (tipo_doc, num_doc) = documento_profesional(
        &p.profesional,
        rec,
        Donde::ap(
            p.id,
            "medico.tipo_documento",
            "tipo de documento del profesional que atendió",
        ),
        Donde::ap(
            p.id,
            "medico.numero_documento",
            "número de documento del profesional que atendió",
        ),
    );

    let diagnostico = exigir!(
        p.diagnostico_principal.as_deref(),
        "diagnostico_principal",
        "diagnóstico principal"
    );

    let donde_valor = Donde::ap(p.id, "valor_servicio", "valor del servicio").supuesto("0");
    let vr_servicio = {
        let d = rec.numero(p.valor_servicio, &donde_valor);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };
    let concepto = exigir!(
        p.concepto_recaudo.as_deref(),
        "concepto_recaudo",
        "concepto de recaudo",
        "05"
    );

    // El CIE-11 se deriva del CIE-10 **de origen**, no del que acabe saliendo: si
    // el diagnóstico faltara y entrara el supuesto, derivar de él daría un CIE-11
    // para un diagnóstico que nadie hizo.
    let (cie_p, nom_p) = cie11::par(p.diagnostico_principal.as_deref());
    let (cie_r, nom_r) = cie11::par(p.diagnostico_relacionado.as_deref());

    Procedimiento {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        fecha_inicio_atencion: fecha_dt(&p.fecha),
        id_mipres: String::new(),
        num_autorizacion: String::new(),
        cod_procedimiento,
        via_ingreso_servicio_salud: via,
        modalidad_grupo_servicio_tec_sal: modalidad,
        grupo_servicios: grupo,
        cod_servicio,
        finalidad_tecnologia_salud: finalidad,
        tipo_documento_identificacion: tipo_doc,
        num_documento_identificacion: num_doc,
        cod_diagnostico_principal: diagnostico,
        cod_diagnostico_relacionado: vacio_a_nulo(&p.diagnostico_relacionado),
        // La complicación no se registra en ninguna pantalla de este sistema, así
        // que va nula. No se anota como falta porque la 948 la declara opcional:
        // «no hubo complicación» es la respuesta normal de una consulta.
        cod_complicacion: None,
        vr_servicio,
        concepto_recaudo: concepto,
        // El copago SÍ admite cero de verdad: la mayoría de las atenciones no lo
        // llevan. Cero aquí es un dato, no un hueco.
        valor_pago_moderador: p.valor_pago_moderador.unwrap_or(0),
        num_fev_pago_moderador: p.num_fev_pago_moderador.clone().unwrap_or_default(),
        consecutivo,
        cod_diagnostico_principal_cie11: cie_p.to_string(),
        nom_cod_diagnostico_principal_cie11: nom_p.to_string(),
        cod_diagnostico_relacionado_cie11: cie_r.to_string(),
        nom_cod_diagnostico_relacionado_cie11: nom_r.to_string(),
        cod_complicacion_cie11: String::new(),
        nom_cod_complicacion_cie11: String::new(),
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

    /// Un procedimiento al que no le falta nada: la línea de base contra la que
    /// se mide cada hueco.
    fn completo(id: i64, p: FuentePaciente) -> FuenteProcedimiento {
        FuenteProcedimiento {
            id,
            fecha: NaiveDate::from_ymd_opt(2026, 3, 15)
                .unwrap()
                .and_hms_opt(9, 30, 15)
                .unwrap(),
            cod_procedimiento_cups: Some("902210".into()),
            tipo: "Laboratorio".into(),
            via_ingreso_servicio: Some("01".into()),
            grupo_servicios: Some("02".into()),
            cod_servicio: Some(105),
            finalidad: Some("44".into()),
            diagnostico_principal: Some("A000".into()),
            diagnostico_relacionado: None,
            valor_servicio: Some(32_000),
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
    fn un_procedimiento_completo_no_deja_faltas() {
        let mut rec = Recolector::estricto();
        let doc = documento_ap(&institucion(), &[completo(1, paciente(7, "1234567"))], &mut rec);
        assert!(
            rec.faltantes().is_empty(),
            "faltas inesperadas: {:?}",
            rec.faltantes()
        );
        assert_eq!(doc.num_documento_id_obligado, "900123456");
        assert_eq!(doc.usuarios.len(), 1);
        let s = &doc.usuarios[0].servicios.procedimientos[0];
        assert_eq!(s.cod_prestador, "050010123401");
        assert_eq!(s.fecha_inicio_atencion, "2026-03-15 09:30", "sin segundos");
        assert_eq!(s.cod_procedimiento.as_deref(), Some("902210"));
        assert_eq!(s.consecutivo, 1);
    }

    #[test]
    fn la_modalidad_sale_del_tipo_y_un_tipo_desconocido_no_cae_al_02() {
        // La pareja. El apoyo diagnóstico es 02 y la cirugía 01…
        let mut rec = Recolector::estricto();
        let mut cirugia = completo(1, paciente(7, "1"));
        cirugia.tipo = "Cirugía".into();
        let doc = documento_ap(&institucion(), &[completo(2, paciente(8, "2")), cirugia], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.procedimientos[0]
                .modalidad_grupo_servicio_tec_sal
                .as_deref(),
            Some("02"),
            "laboratorio es apoyo diagnóstico"
        );
        assert_eq!(
            doc.usuarios[1].servicios.procedimientos[0]
                .modalidad_grupo_servicio_tec_sal
                .as_deref(),
            Some("01"),
            "cirugía es intramural"
        );
        assert!(rec.faltantes().is_empty());

        // …y un tipo que el mapa no nombra se queda SIN modalidad y se anota, en
        // vez de caer al 02 por descarte.
        let mut rec = Recolector::estricto();
        let mut raro = completo(3, paciente(9, "3"));
        raro.tipo = "Consejería astral".into();
        let doc = documento_ap(&institucion(), &[raro], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.procedimientos[0].modalidad_grupo_servicio_tec_sal,
            None
        );
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].campo, "tipo");
        assert_eq!(f[0].archivo, "AP");
    }

    #[test]
    fn el_cie11_sale_del_mismo_diagnostico_que_el_cie10() {
        // El fallo que el Python arrastraba: el CIE-10 salía y el CIE-11 iba
        // vacío SIEMPRE, porque se derivaba de un campo que `Orden` no tiene.
        let mut rec = Recolector::estricto();
        let doc = documento_ap(&institucion(), &[completo(1, paciente(7, "1"))], &mut rec);
        let s = &doc.usuarios[0].servicios.procedimientos[0];
        assert_eq!(s.cod_diagnostico_principal.as_deref(), Some("A000"));
        assert_eq!(
            s.cod_diagnostico_principal_cie11,
            cie11::par(Some("A000")).0,
            "el par se deriva del diagnóstico que se está emitiendo"
        );
        assert!(
            !s.cod_diagnostico_principal_cie11.is_empty(),
            "A000 tiene equivalencia exacta y única; si esto sale vacío, el par \
             no se está derivando"
        );
    }

    #[test]
    fn sin_cups_el_campo_va_vacio_y_no_lleva_un_procedimiento_que_no_existe() {
        let mut p = completo(1, paciente(7, "1"));
        p.cod_procedimiento_cups = None;
        let mut rec = Recolector::estricto();
        let doc = documento_ap(&institucion(), &[p], &mut rec);
        assert_eq!(doc.usuarios[0].servicios.procedimientos[0].cod_procedimiento, None);
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].campo, "codigo_cups");
        assert_eq!(f[0].supuesto.as_deref(), Some("999999"));
    }

    #[test]
    fn con_supuestos_permitidos_sale_pero_queda_anotado() {
        let mut p = completo(1, paciente(7, "1"));
        p.cod_procedimiento_cups = None;
        p.via_ingreso_servicio = None;
        p.cod_servicio = None;
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ap(&institucion(), &[p], &mut rec);
        let s = &doc.usuarios[0].servicios.procedimientos[0];
        assert_eq!(s.cod_procedimiento.as_deref(), Some("999999"));
        assert_eq!(s.via_ingreso_servicio_salud.as_deref(), Some("01"));
        assert_eq!(s.cod_servicio, Some(0));
        assert_eq!(rec.faltantes().len(), 3, "salen, pero no en silencio");
    }

    #[test]
    fn la_finalidad_diez_no_pasa_tampoco_en_procedimientos() {
        let mut p = completo(1, paciente(7, "1"));
        p.finalidad = Some("10".into());
        let mut rec = Recolector::estricto();
        documento_ap(&institucion(), &[p], &mut rec);
        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert!(f[0].motivo.as_deref().unwrap().contains("defecto del sistema"));
    }

    #[test]
    fn dos_procedimientos_del_mismo_paciente_van_en_un_solo_usuario() {
        let mut rec = Recolector::estricto();
        let doc = documento_ap(
            &institucion(),
            &[completo(1, paciente(7, "1")), completo(2, paciente(7, "1"))],
            &mut rec,
        );
        assert_eq!(doc.usuarios.len(), 1, "el usuario no se repite");
        assert_eq!(doc.usuarios[0].servicios.procedimientos.len(), 2);
        assert_eq!(doc.usuarios[0].servicios.procedimientos[1].consecutivo, 2);
    }

    #[test]
    fn el_ap_no_emite_una_lista_vacia_de_consultas() {
        let mut rec = Recolector::estricto();
        let doc = documento_ap(&institucion(), &[completo(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"procedimientos\""));
        assert!(
            !json.contains("\"consultas\""),
            "el AP de un paciente no lleva una lista vacía de consultas"
        );
        assert!(!json.contains("\"medicamentos\""));
    }

    #[test]
    fn el_orden_de_los_campos_es_el_de_la_948() {
        let mut rec = Recolector::estricto();
        let doc = documento_ap(&institucion(), &[completo(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.procedimientos[0]).unwrap();
        let orden = [
            "codPrestador",
            "fechaInicioAtencion",
            "idMIPRES",
            "numAutorizacion",
            "codProcedimiento",
            "viaIngresoServicioSalud",
            "modalidadGrupoServicioTecSal",
            "grupoServicios",
            "codServicio",
            "finalidadTecnologiaSalud",
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "codDiagnosticoPrincipal",
            "codDiagnosticoRelacionado",
            "codComplicacion",
            "vrServicio",
            "conceptoRecaudo",
            "valorPagoModerador",
            "numFEVPagoModerador",
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
}
