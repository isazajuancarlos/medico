//! **AT — el arreglo `otrosServicios`.** Séptimo y último de los siete de la 948
//! (#67).
//!
//! El Python lo llevaba cableado a cero (`rips_completo.py:1417`) y medico no lo
//! emitía: esto no es portar, es construir el dato. ALCANCE (Juan, 2026-09-08):
//! solo insumos/dispositivos, `tipoOS = "01"` (Dispositivos médicos e insumos).
//!
//! La fuente es la `dispensacion` de un ítem NO medicamento (INSUMO/DISPOSITIVO/
//! REACTIVO) colgada de una ATENCIÓN (`historia_id`), no de una fórmula — ver la
//! migración 0073. Un medicamento va en el arreglo `medicamentos` (AM), nunca
//! aquí: contarlo dos veces sería un doble cobro a la EPS.
//!
//! Estructuralmente es casi gemelo del AM: los mismos campos de facturación
//! (`vrUnitOS`/`vrDispensacion`/`vrServicio`/`conceptoRecaudo`/pago moderador) y
//! el mismo trato de las faltas por el [`Recolector`] —nada se supone salvo en
//! modo permisivo, y solo donde la norma manda—. De `vrUnitOS` depende lo que se
//! cobra, así que un valor ausente se anota como falta, no se inventa (#67, D20).

use serde::Serialize;

use super::documentos::{
    documento_profesional, fecha_d, usuario_base, Documento, FuenteInstitucion, FuentePaciente,
    FuenteProfesional, Usuario,
};
use super::recolector::{Donde, Recolector};

/// `tipoOS = "01"` — Dispositivos médicos e insumos. Único valor del alcance
/// decidido. La transición de la 948 admite el código propio del facturador como
/// `codTecnologiaSalud`, y por eso se acepta `registro_invima` a falta de IUM.
const TIPO_OS_DISPOSITIVOS_E_INSUMOS: &str = "01";

/// Una dispensación de insumo/dispositivo tal como sale de la base, lista para
/// armar su `otrosServicios`.
#[derive(Debug, Clone)]
pub struct FuenteOtroServicio {
    /// El id de la fila de `dispensacion`, que es donde hay que ir a corregir.
    pub id: i64,
    /// La fecha del suministro (`dispensacion.fecha_dispensacion`), que es la del
    /// evento que el RIPS reporta.
    pub fecha: chrono::NaiveDate,
    /// IUM (`item_inventario.ium`) o, en su defecto, `registro_invima` como
    /// código propio durante la transición. `None` si el ítem no tiene ninguno.
    pub codigo_tecnologia: Option<String>,
    /// `item_inventario.nombre`.
    pub nombre_tecnologia: String,
    pub cantidad: Option<i64>,
    pub num_autorizacion: Option<String>,
    pub id_mipres: Option<String>,
    pub valor_unitario: Option<i64>,
    pub valor_dispensacion: Option<i64>,
    pub valor_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    pub valor_pago_moderador: Option<i64>,
    pub num_fev_pago_moderador: Option<String>,
    /// El id de la atención en el sistema IHCE (#603). Fuera a propósito mientras
    /// el transporte no esté enchufado; sale `null` (falta con motivo), igual que
    /// en el AN.
    pub codigo_vida: Option<String>,
    pub paciente: FuentePaciente,
    /// El profesional de la atención en que se suministró (P que ordena/entrega).
    pub profesional: FuenteProfesional,
    /// Procedencia de cada dato (JSON `{campo: procedencia}`), para la auditoría
    /// de #84. `None` mientras la captura no la escriba.
    pub procedencias: Option<String>,
}

/// Un `otrosServicios` tal y como sale al JSON de la 948.
///
/// El orden de los campos es el de `campos_fev948.json` (clave `otrosServicios`),
/// campo a campo: es lo que permite comparar el archivo con el del validador.
#[derive(Debug, Clone, Serialize)]
pub struct OtroServicio {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "numAutorizacion")]
    pub num_autorizacion: Option<String>,
    #[serde(rename = "idMIPRES")]
    pub id_mipres: Option<String>,
    #[serde(rename = "fechaSuministroTecnologia")]
    pub fecha_suministro_tecnologia: String,
    #[serde(rename = "tipoOS")]
    pub tipo_os: String,
    #[serde(rename = "codTecnologiaSalud")]
    pub cod_tecnologia_salud: Option<String>,
    #[serde(rename = "nomTecnologiaSalud")]
    pub nom_tecnologia_salud: String,
    #[serde(rename = "cantidadOS")]
    pub cantidad_os: Option<i64>,
    #[serde(rename = "tipoDocumentoIdentificacion")]
    pub tipo_documento_identificacion: Option<String>,
    #[serde(rename = "numDocumentoIdentificacion")]
    pub num_documento_identificacion: Option<String>,
    #[serde(rename = "vrUnitOS")]
    pub vr_unit_os: Option<i64>,
    #[serde(rename = "vrDispensacion")]
    pub vr_dispensacion: Option<i64>,
    #[serde(rename = "vrServicio")]
    pub vr_servicio: Option<i64>,
    #[serde(rename = "conceptoRecaudo")]
    pub concepto_recaudo: Option<String>,
    #[serde(rename = "valorPagoModerador")]
    pub valor_pago_moderador: i64,
    #[serde(rename = "numFEVPagoModerador")]
    pub num_fev_pago_moderador: Option<String>,
    #[serde(rename = "codigoVIDA")]
    pub codigo_vida: Option<String>,
    pub consecutivo: i64,
}

/// Arma el AT a partir de las dispensaciones de insumo/dispositivo del periodo,
/// ya traídas de la base. Un usuario por paciente, con sus servicios agrupados.
pub fn documento_aos(
    institucion: &FuenteInstitucion,
    servicios: &[FuenteOtroServicio],
    rec: &mut Recolector,
) -> Documento {
    let mut usuarios: Vec<Usuario> = Vec::new();
    let mut indice: Vec<i64> = Vec::new();

    for f in servicios {
        let pos = match indice.iter().position(|id| *id == f.paciente.id) {
            Some(i) => i,
            None => {
                let u = usuario_base(&f.paciente, usuarios.len() as i64 + 1, rec);
                usuarios.push(u);
                indice.push(f.paciente.id);
                usuarios.len() - 1
            }
        };

        let consecutivo = usuarios[pos].servicios.otros_servicios.len() as i64 + 1;
        let os = otro_servicio_de(institucion, f, consecutivo, rec);
        usuarios[pos].servicios.otros_servicios.push(os);
    }

    Documento {
        num_documento_id_obligado: institucion.nit.clone(),
        num_factura: String::new(),
        tipo_nota: String::new(),
        num_nota: String::new(),
        usuarios,
    }
}

fn otro_servicio_de(
    institucion: &FuenteInstitucion,
    f: &FuenteOtroServicio,
    consecutivo: i64,
    rec: &mut Recolector,
) -> OtroServicio {
    rec.en_registro(f.procedencias.as_deref()); // #84: audita la procedencia
    let permisivo = rec.permite_supuestos();

    // El código de la tecnología (IUM o, en transición, el registro INVIMA). Sin
    // supuesto: un código inventado tiene la forma correcta y hace rechazar la
    // factura entera, así que a falta de dato el campo queda vacío y se anota.
    let cod_tecnologia = {
        let donde = Donde::aos(
            f.id,
            "item_inventario.ium/registro_invima",
            "código de la tecnología en salud (IUM o registro INVIMA)",
        );
        rec.dato(f.codigo_tecnologia.as_deref(), &donde).o_nada()
    };

    let cantidad = {
        let donde = Donde::aos(f.id, "cantidad_dispensada", "cantidad suministrada");
        rec.numero(f.cantidad, &donde).o_nada()
    };

    let (tipo_doc, num_doc) = documento_profesional(
        &f.profesional,
        rec,
        Donde::aos(
            f.id,
            "profesional.tipo_documento",
            "tipo de documento del profesional que suministró",
        ),
        Donde::aos(
            f.id,
            "profesional.numero_documento",
            "número de documento del profesional que suministró",
        ),
    );

    let vr_unitario = {
        let donde =
            Donde::aos(f.id, "valor_unitario", "valor unitario de la tecnología").supuesto("0");
        let d = rec.numero(f.valor_unitario, &donde);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };

    let vr_dispensacion = {
        let donde = Donde::aos(f.id, "valor_dispensacion", "valor de la dispensación").supuesto("0");
        let d = rec.numero(f.valor_dispensacion, &donde);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };

    let vr_servicio = {
        let donde = Donde::aos(f.id, "valor_servicio", "valor del servicio").supuesto("0");
        let d = rec.numero(f.valor_servicio, &donde);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };

    let concepto = {
        let donde = Donde::aos(f.id, "concepto_recaudo", "concepto de recaudo").supuesto("05");
        let d = rec.dato(f.concepto_recaudo.as_deref(), &donde);
        if permisivo {
            d.o_el_supuesto(Some("05"))
        } else {
            d.o_nada()
        }
    };

    OtroServicio {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        num_autorizacion: vacio_a_nulo(&f.num_autorizacion),
        id_mipres: vacio_a_nulo(&f.id_mipres),
        fecha_suministro_tecnologia: fecha_d(&f.fecha),
        tipo_os: TIPO_OS_DISPOSITIVOS_E_INSUMOS.to_string(),
        cod_tecnologia_salud: cod_tecnologia,
        nom_tecnologia_salud: f.nombre_tecnologia.clone(),
        cantidad_os: cantidad,
        tipo_documento_identificacion: tipo_doc,
        num_documento_identificacion: num_doc,
        vr_unit_os: vr_unitario,
        vr_dispensacion,
        vr_servicio,
        concepto_recaudo: concepto,
        valor_pago_moderador: f.valor_pago_moderador.unwrap_or(0),
        num_fev_pago_moderador: vacio_a_nulo(&f.num_fev_pago_moderador),
        codigo_vida: vacio_a_nulo(&f.codigo_vida),
        consecutivo,
    }
}

/// `Some("")` y `Some(" ")` son ausencia, no dato: la 948 quiere `null`, no una
/// cadena vacía (ver `fev.rs`). Igual que el `vacio_a_nulo` del AN y el AH.
fn vacio_a_nulo(v: &Option<String>) -> Option<String> {
    match v {
        Some(s) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn institucion() -> FuenteInstitucion {
        FuenteInstitucion {
            nit: "900123456".into(),
            codigo_habilitacion: "050010123401".into(),
        }
    }

    fn paciente() -> FuentePaciente {
        FuentePaciente {
            id: 7,
            tipo_documento: "CC".into(),
            numero_documento: "111".into(),
            tipo_usuario: Some("01".into()),
            fecha_nacimiento: "1985-04-12".into(),
            sexo: "M".into(),
            cod_pais: Some("170".into()),
            cod_municipio: Some("05001".into()),
            zona: Some("U".into()),
            cod_pais_origen: Some("170".into()),
            procedencias: None,
        }
    }

    fn profesional() -> FuenteProfesional {
        FuenteProfesional {
            tipo_documento: Some("CC".into()),
            numero_documento: Some("98765432".into()),
        }
    }

    fn completa() -> FuenteOtroServicio {
        FuenteOtroServicio {
            id: 1,
            fecha: chrono::NaiveDate::from_ymd_opt(2026, 3, 16).unwrap(),
            codigo_tecnologia: Some("20123456-1".into()),
            nombre_tecnologia: "Catéter venoso central".into(),
            cantidad: Some(2),
            num_autorizacion: Some("AUT-778".into()),
            id_mipres: None,
            valor_unitario: Some(35000),
            valor_dispensacion: Some(0),
            valor_servicio: Some(70000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: Some(0),
            num_fev_pago_moderador: None,
            codigo_vida: None,
            paciente: paciente(),
            profesional: profesional(),
            procedencias: None,
        }
    }

    /// VERDE: una dispensación de insumo con todos sus datos no deja faltas y
    /// emite `tipoOS = "01"` y los valores capturados tal cual.
    #[test]
    fn una_dispensacion_completa_no_deja_faltas() {
        let mut rec = Recolector::estricto();
        let doc = documento_aos(&institucion(), &[completa()], &mut rec);
        assert!(
            rec.faltantes().is_empty(),
            "faltas inesperadas: {:?}",
            rec.faltantes()
        );
        let s = &doc.usuarios[0].servicios.otros_servicios[0];
        assert_eq!(s.tipo_os, "01");
        assert_eq!(s.cod_prestador, "050010123401");
        assert_eq!(s.cod_tecnologia_salud.as_deref(), Some("20123456-1"));
        assert_eq!(s.cantidad_os, Some(2));
        assert_eq!(s.vr_unit_os, Some(35000));
        assert_eq!(s.vr_servicio, Some(70000));
        assert_eq!(s.num_autorizacion.as_deref(), Some("AUT-778"));
        assert_eq!(s.consecutivo, 1);
    }

    /// ROJO discriminante (D20/D33): sin `valor_unitario` y en modo ESTRICTO, el
    /// campo sale `null` Y queda anotado como falta — NO un 0 silencioso. Un
    /// emisor que rellenara con 0 (o con el supuesto en modo estricto) haría
    /// `vr_unit_os == Some(0)` y esta prueba caería: de `vrUnitOS` depende lo que
    /// se le cobra a la EPS, así que un valor inventado es un cobro sin sustento.
    #[test]
    fn sin_valor_unitario_es_falta_no_cero() {
        let mut rec = Recolector::estricto();
        let f = FuenteOtroServicio {
            valor_unitario: None,
            ..completa()
        };
        let doc = documento_aos(&institucion(), &[f], &mut rec);
        let s = &doc.usuarios[0].servicios.otros_servicios[0];
        assert_eq!(s.vr_unit_os, None, "sin dato, el valor unitario NO se inventa");
        assert!(
            rec.faltantes().iter().any(|x| x.campo == "valor_unitario"),
            "el valor unitario ausente tiene que quedar anotado como falta: {:?}",
            rec.faltantes()
        );
    }

    /// El insumo NO medicamento cuelga de la atención, y su documento se funde
    /// con el del mismo paciente: dos dispensaciones del mismo paciente son un
    /// usuario con dos `otrosServicios`, renumerados 1 y 2.
    #[test]
    fn dos_del_mismo_paciente_se_agrupan_en_un_usuario() {
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_aos(&institucion(), &[completa(), completa()], &mut rec);
        assert_eq!(doc.usuarios.len(), 1);
        assert_eq!(doc.usuarios[0].servicios.otros_servicios.len(), 2);
    }
}
