// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

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
use crate::{cie11, cum, muv};

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
    /// Procedencia de cada dato clínico (JSON `{campo: procedencia}`), para la
    /// auditoría de #84. `None` mientras la captura no la escriba.
    pub procedencias: Option<String>,
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
    #[serde(rename = "codDiagnosticoPrincipalCIE11")]
    pub cod_diagnostico_principal_cie11: String,
    #[serde(rename = "nomCodDiagnosticoPrincipalCIE11")]
    pub nom_cod_diagnostico_principal_cie11: String,
    #[serde(rename = "codDiagnosticoRelacionado")]
    pub cod_diagnostico_relacionado: Option<String>,
    #[serde(rename = "codDiagnosticoRelacionadoCIE11")]
    pub cod_diagnostico_relacionado_cie11: String,
    #[serde(rename = "nomCodDiagnosticoRelacionadoCIE11")]
    pub nom_cod_diagnostico_relacionado_cie11: String,
    /// M10, M11 y M12. Los tres son de la PREPARACIÓN MAGISTRAL, y para los
    /// demás tipos la norma no deja el campo fuera: dice qué valor lleva —«0»
    /// los dos numéricos, «null» la forma farmacéutica—. Por eso el 0 de aquí
    /// no es un defecto de relleno sino la regla escrita en el Documento
    /// Técnico 1; lo que sí sería inventarse un dato es mandar ese 0 cuando el
    /// medicamento SÍ es magistral, y ese caso se anota como falta.
    #[serde(rename = "concentracionMedicamento")]
    pub concentracion_medicamento: Option<i64>,
    #[serde(rename = "unidadMedida")]
    pub unidad_medida: Option<i64>,
    #[serde(rename = "formaFarmaceutica")]
    pub forma_farmaceutica: Option<String>,
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
    /// M28: «valor acordado entre el gestor Farmacéutico y la EPS, en caso de
    /// no aplicar diligenciar en 0». medico no registra ese acuerdo —no hay
    /// gestor farmacéutico de por medio—, así que el caso es el de «no
    /// aplica», y el 0 lo dice la norma.
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
    rec.en_registro(f.procedencias.as_deref()); // #84: audita la procedencia de este medicamento
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

    // M10/M11/M12 sirven a la preparación MAGISTRAL (tipoMedicamento 03). Para
    // los otros tipos la norma manda 0, 0 y null; para la magistral manda el
    // dato real, y medico no lo captura todavía — así que ahí se anota como
    // falta y sale vacío, en vez de afirmar una concentración de cero.
    let es_magistral = tipo_medicamento.as_deref() == Some("03");
    let (concentracion, unidad_medida, forma_farmaceutica) = if es_magistral {
        rec.dato(
            None,
            &Donde::am(f.id, "concentracion", "concentración del principio activo"),
        );
        rec.dato(
            None,
            &Donde::am(f.id, "unidad_medida", "unidad de medida de la concentración"),
        );
        rec.dato(
            None,
            &Donde::am(f.id, "forma_farmaceutica", "forma farmacéutica (tabla FFM)"),
        );
        (None, None, None)
    } else {
        (Some(0), Some(0), None)
    };

    let (cie_dx, nom_cie_dx) = cie11::par(diagnostico.as_deref());

    Medicamento {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        fecha_dispens_admon: fecha_d(&f.fecha),
        id_mipres: String::new(),
        cod_tecnologia_salud: cod_tecnologia,
        tipo_medicamento,
        nom_tecnologia_salud: f.medicamento.clone(),
        cod_diagnostico_principal: diagnostico,
        cod_diagnostico_principal_cie11: cie_dx.to_string(),
        nom_cod_diagnostico_principal_cie11: nom_cie_dx.to_string(),
        // La fórmula no lleva un segundo diagnóstico: el único que tiene es el
        // de la historia en que se prescribió, y ése ya va de principal.
        cod_diagnostico_relacionado: None,
        cod_diagnostico_relacionado_cie11: String::new(),
        nom_cod_diagnostico_relacionado_cie11: String::new(),
        concentracion_medicamento: concentracion,
        unidad_medida,
        forma_farmaceutica,
        unidad_min_dispensa: unidad_min,
        cantidad_medicamento: cantidad,
        dias_tratamiento: dias,
        tipo_documento_identificacion: tipo_doc,
        num_documento_identificacion: num_doc,
        vr_unit_medicamento: vr_unitario,
        vr_dispensacion: Some(0),
        vr_servicio,
        concepto_recaudo: concepto,
        valor_pago_moderador: f.valor_pago_moderador.unwrap_or(0),
        num_fev_pago_moderador: Some(f.num_fev_pago_moderador.clone().unwrap_or_default()),
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
    // CERO Y NEGATIVOS SALEN ANTES DEL CAMINO CORTO, y esto es lo que faltaba.
    //
    // El 2026-08-16 se puso una guarda en la ENTRADA (`FormulaNueva.cantidad`,
    // en la frontera de `serde`), y la revisión de la directiva 25 sobre
    // `b42033aa` señaló que cerraba la puerta sin revisar la habitación: aquí
    // `-5` y `0` pasan `fract() == 0.0`, así que salían por el camino corto CON
    // `rec.presente()` — al archivo del AM, y auditados como si alguien los
    // hubiera afirmado. La asimetría lo delataba: `2.5` sí producía falta y
    // `-5` ninguna.
    //
    // Y la guarda de la entrada NO alcanza a lo ya guardado: el panel tuvo hasta
    // `1c570350` un `unwrap_or(0.0)` que grababa ceros con la casilla marcada, y
    // la API no validó nada hasta ese mismo día. Esas filas están en la base y
    // solo las caza esto.
    //
    // Sale como `SinConvertir` y no se emite: un rechazo del MUV llega semanas
    // después sobre el mes entero; una falta en el informe de cierre se corrige
    // en una tarde.
    if n <= 0.0 {
        rec.no_vale(
            &donde,
            format!(
                "«{n}» no es una cantidad dispensable: el RIPS admite de 1 a 999. \
                 Corríjala en la fórmula — el archivo no puede salir con esto."
            ),
        );
        return Some(Cantidad::SinConvertir(n));
    }
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        // #84: hay valor, así que su procedencia SÍ se audita. Este camino corto
        // no pasa por `dato`/`numero`, que son los que auditan.
        rec.presente(&donde);
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
            procedencias: None,
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
            procedencias: None,
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

    /// La procedencia (#84) del AM, con sus DOS listas. Es la prueba de que la
    /// auditoría llegó a la fórmula: hasta la 0060, `rips.rs` le pasaba
    /// `procedencias: None` escrito a mano y este camino no se recorría nunca —
    /// un negativo indistinguible de «está limpio» (directiva 30).
    ///
    /// Y la tercera afirmación es la que descarta un vigilante ciego a la
    /// inversa: `historia.diagnostico_principal` tiene valor y NO está en el
    /// mapa, y aun así no se marca, porque nombra la columna de otra tabla y
    /// quien captura la fórmula no lo ve ni tiene dónde confirmarlo.
    #[test]
    fn la_procedencia_marca_el_cum_sin_respaldo_y_no_lo_ajeno() {
        // 1) Con el CUM confirmado: limpio.
        let mut f = completa(1, paciente(7, "1"));
        f.procedencias = Some(r#"{"codigo_cum":"confirmado"}"#.into());
        let mut rec = Recolector::estricto();
        documento_am(&institucion(), &[f], &mut rec);
        assert!(
            rec.sin_respaldo().iter().all(|x| x.campo != "codigo_cum"),
            "un CUM confirmado no se marca: {:?}",
            rec.sin_respaldo()
        );
        // Y lo ajeno tampoco, aunque tenga valor y no esté en el mapa.
        assert!(
            rec.sin_respaldo().iter().all(|x| !x.campo.contains('.')),
            "un campo de otra tabla no es de esta captura: {:?}",
            rec.sin_respaldo()
        );

        // 1 bis) La CANTIDAD también se audita cuando está, y esa es la mitad que
        //    faltaba: `cantidad_entera` devuelve por el camino corto ante una
        //    cantidad entera, así que no pasaba por `dato` ni por `numero` —los dos
        //    que auditan— y se quedaba fuera justo cuando había algo que respaldar.
        //    Con un mapa que respalda todo MENOS la cantidad, tiene que salir ella.
        let mut h = completa(1, paciente(7, "1"));
        h.procedencias = Some(
            r#"{"codigo_cum":"confirmado","unidad":"confirmado","dias_tratamiento":"confirmado",
                "tipo_medicamento":"confirmado","valor_unitario":"confirmado",
                "valor_servicio":"confirmado","concepto_recaudo":"confirmado"}"#
                .into(),
        );
        let mut rec_c = Recolector::estricto();
        documento_am(&institucion(), &[h], &mut rec_c);
        let solo: Vec<_> = rec_c.sin_respaldo().iter().map(|x| x.campo).collect();
        assert_eq!(
            solo,
            vec!["cantidad"],
            "la cantidad presente tiene que auditarse, y ella sola: {solo:?}"
        );

        // 2) Con el mapa presente pero SIN el CUM: sale marcado. Es el caso que
        //    la fricción graduada produce cuando nadie toca la casilla.
        let mut g = completa(1, paciente(7, "1"));
        g.procedencias = Some(r#"{"dias_tratamiento":"confirmado"}"#.into());
        let mut rec2 = Recolector::estricto();
        documento_am(&institucion(), &[g], &mut rec2);
        let marcados: Vec<_> = rec2.sin_respaldo().iter().map(|x| x.campo).collect();
        assert!(
            marcados.contains(&"codigo_cum"),
            "el CUM sin respaldo tiene que salir: {marcados:?}"
        );
        assert!(
            !marcados.contains(&"dias_tratamiento"),
            "el que sí se confirmó, no: {marcados:?}"
        );

        // 3) Sin mapa (fila anterior a la 0060): no se audita nada.
        let mut rec3 = Recolector::estricto();
        documento_am(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec3);
        assert!(
            rec3.sin_respaldo().is_empty(),
            "sin procedencia declarada no se audita: {:?}",
            rec3.sin_respaldo()
        );
    }

    /// **LA ALARMA SUENA CUANDO NADIE AFIRMÓ NADA**, que es el caso por defecto y
    /// el que #84 existe para cazar. Es la prueba que faltaba, y su ausencia dejó
    /// pasar un mecanismo entero inerte.
    ///
    /// Lo que se mide aquí es el `{}` de `json_de`, no la columna: una fórmula
    /// capturada sin marcar la casilla llega con el mapa VACÍO, y eso tiene que
    /// producir una anotación por cada campo con valor. Con el `NULL` que la
    /// primera versión escribía, `en_registro` salta el medicamento entero y esta
    /// prueba sale con cero anotaciones — que es como estaba.
    ///
    /// Su pareja es la de al lado (`la_procedencia_marca_el_cum_sin_respaldo_y_no_lo_ajeno`,
    /// caso 3): con `None` —fila anterior a la migración— NO se audita. Las dos
    /// juntas son la directiva 32: `{}` y `NULL` significan cosas distintas y por
    /// eso no pueden ser el mismo valor.
    #[test]
    fn el_mapa_vacio_marca_todo_lo_que_tiene_valor() {
        let mut f = completa(1, paciente(7, "1"));
        f.procedencias = Some("{}".into());
        let mut rec = Recolector::estricto();
        documento_am(&institucion(), &[f], &mut rec);

        let marcados: Vec<_> = rec.sin_respaldo().iter().map(|x| x.campo).collect();
        for campo in ["codigo_cum", "cantidad", "unidad", "dias_tratamiento"] {
            assert!(
                marcados.contains(&campo),
                "`{campo}` tiene valor y nadie lo afirmó: tiene que salir marcado. \
                 Marcados: {marcados:?}"
            );
        }
        // Y sigue sin marcar lo ajeno, que no es de esta captura.
        assert!(
            marcados.iter().all(|c| !c.contains('.')),
            "lo de otra tabla no se marca aquí: {marcados:?}"
        );

        // Un JSON ILEGIBLE se audita igual que el mapa vacío: no exime. Antes
        // dejaba el registro fuera de toda auditoría, que es convertir un dato
        // corrupto en un permiso.
        let mut g = completa(1, paciente(7, "1"));
        g.procedencias = Some("{roto".into());
        let mut rec2 = Recolector::estricto();
        documento_am(&institucion(), &[g], &mut rec2);
        assert!(
            rec2.sin_respaldo().iter().any(|x| x.campo == "codigo_cum"),
            "un JSON ilegible no puede eximir: {:?}",
            rec2.sin_respaldo()
        );
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

    /// **CERO Y NEGATIVOS NO SALEN AL AM**, con sus dos listas.
    ///
    /// Lo trajo la segunda ronda de la directiva 25, sobre `b42033aa`: la guarda
    /// nueva de `FormulaNueva.cantidad` cierra la ENTRADA, pero aquí `-5` y `0`
    /// pasaban `fract() == 0.0` y salían por el camino corto **con
    /// `rec.presente()`** — en el archivo que se le cobra a la EPS, y auditados
    /// como afirmados por un humano. Lo delataba la asimetría: `2.5` sí producía
    /// falta y `-5` ninguna.
    ///
    /// **Y esto NO lo cubre la guarda de la entrada**, que es lo que lo hace
    /// necesario: el panel tuvo hasta `1c570350` un `unwrap_or(0.0)` que grababa
    /// ceros con la casilla marcada, y la API no validó nada hasta ese día. Esas
    /// filas están en la base y solo las caza aquí.
    #[test]
    fn una_cantidad_de_cero_o_negativa_no_llega_al_archivo() {
        for mala in [0.0, -5.0, -0.5] {
            let mut f = completa(1, paciente(7, "1"));
            f.cantidad = Some(mala);
            let mut rec = Recolector::estricto();
            let doc = documento_am(&institucion(), &[f], &mut rec);
            assert_eq!(
                doc.usuarios[0].servicios.medicamentos[0].cantidad_medicamento,
                Some(Cantidad::SinConvertir(mala)),
                "«{mala}» no puede salir como cantidad entera"
            );
            let faltas = rec.faltantes();
            assert_eq!(faltas.len(), 1, "«{mala}» tiene que dejar UNA falta: {faltas:?}");
            assert!(
                faltas[0].motivo.as_deref().unwrap().contains("dispensable"),
                "y decir por qué: {:?}",
                faltas[0].motivo
            );
            assert!(
                rec.sin_respaldo().is_empty(),
                "y NO puede quedar auditada como presente: {:?}",
                rec.sin_respaldo()
            );
        }
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
