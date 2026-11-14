// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **AC — Archivo de Consultas.** Port de `generar_archivo_ac` (#250).
//!
//! Es el documento obligatorio y el más usado: toda atención ambulatoria y toda
//! consulta de control. `#67` lo da por verificado contra dos fuentes, así que
//! la forma que emite el Python es segura de portar.
//!
//! # Lo que se corrige al portar, en vez de replicarlo
//!
//! `finalidadTecnologiaSalud` sale con `10` en toda consulta donde nadie eligió
//! una finalidad —lo pone el defecto del modelo en Python y el
//! `NOT NULL DEFAULT '10'` de SQLite—, y **`10` no existe en la tabla del MUV**,
//! que va del 11 al 44. Como el campo nunca venía vacío, el recolector no lo
//! veía: era un dato falso con la forma correcta, que es la peor clase.
//!
//! Aquí pasa por [`Recolector::codigo`], así que sale en el informe de cierre
//! como lo que es. **No se sustituye por `11`**: elegir la finalidad de un acto
//! médico en nombre de quien lo hizo es exactamente lo que este módulo existe
//! para no hacer.

use serde::Serialize;

use super::documentos::{
    documento_profesional, fecha_dt, usuario_base, Documento, FuenteInstitucion, FuentePaciente,
    FuenteProfesional, Usuario,
};
use super::recolector::{Donde, Recolector};
use crate::{cie11, muv};

/// Una consulta ya traída de la base, con su paciente y su profesional.
#[derive(Debug, Clone)]
pub struct FuenteConsulta {
    /// El id de la historia, para señalar dónde corregir.
    pub id: i64,
    pub fecha: chrono::NaiveDateTime,
    /// De la cita, si la hubo.
    pub num_autorizacion: Option<String>,
    pub cod_consulta_cups: Option<String>,
    pub modalidad_grupo_servicio: Option<String>,
    pub grupo_servicios: Option<String>,
    pub cod_servicio: Option<i64>,
    pub finalidad: Option<String>,
    pub causa_externa: Option<String>,
    pub diagnostico_principal: Option<String>,
    pub diagnostico_relacionado1: Option<String>,
    pub diagnostico_relacionado2: Option<String>,
    pub diagnostico_relacionado3: Option<String>,
    pub tipo_diagnostico: Option<String>,
    pub valor_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    pub valor_pago_moderador: Option<i64>,
    pub num_fev_pago_moderador: Option<String>,
    pub genera_incapacidad: bool,
    pub paciente: FuentePaciente,
    pub profesional: FuenteProfesional,
    /// Procedencia de cada dato clínico (JSON `{campo: procedencia}`), para la
    /// auditoría de #84. `None` mientras la captura no la escriba.
    pub procedencias: Option<String>,
}

/// Una consulta tal y como sale al JSON de la 948.
///
/// El orden de los campos es el del Python, campo a campo: es lo que permite
/// comparar los dos archivos byte a byte.
#[derive(Debug, Clone, Serialize)]
pub struct Consulta {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "fechaInicioAtencion")]
    pub fecha_inicio_atencion: String,
    #[serde(rename = "numAutorizacion")]
    pub num_autorizacion: Option<String>,
    #[serde(rename = "codConsulta")]
    pub cod_consulta: Option<String>,
    #[serde(rename = "modalidadGrupoServicioTecSal")]
    pub modalidad_grupo_servicio_tec_sal: Option<String>,
    #[serde(rename = "grupoServicios")]
    pub grupo_servicios: Option<String>,
    #[serde(rename = "codServicio")]
    pub cod_servicio: Option<i64>,
    #[serde(rename = "finalidadTecnologiaSalud")]
    pub finalidad_tecnologia_salud: Option<String>,
    #[serde(rename = "causaMotivoAtencion")]
    pub causa_motivo_atencion: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipal")]
    pub cod_diagnostico_principal: Option<String>,
    // Los relacionados SÍ pueden no estar: son opcionales. Pero «no hay» se
    // escribe `null`, no `""`: el MUV contrasta el campo contra la tabla de la
    // CIE y la cadena vacía no está en ninguna tabla.
    #[serde(rename = "codDiagnosticoRelacionado1")]
    pub cod_diagnostico_relacionado1: Option<String>,
    #[serde(rename = "codDiagnosticoRelacionado2")]
    pub cod_diagnostico_relacionado2: Option<String>,
    #[serde(rename = "codDiagnosticoRelacionado3")]
    pub cod_diagnostico_relacionado3: Option<String>,
    #[serde(rename = "codDiagnosticoPrincipalCIE11")]
    pub cod_diagnostico_principal_cie11: String,
    #[serde(rename = "nomCodDiagnosticoPrincipalCIE11")]
    pub nom_cod_diagnostico_principal_cie11: String,
    #[serde(rename = "codDiagnosticoRelacionado1CIE11")]
    pub cod_diagnostico_relacionado1_cie11: String,
    #[serde(rename = "nomCodDiagnosticoRelacionado1CIE11")]
    pub nom_cod_diagnostico_relacionado1_cie11: String,
    #[serde(rename = "codDiagnosticoRelacionado2CIE11")]
    pub cod_diagnostico_relacionado2_cie11: String,
    #[serde(rename = "nomCodDiagnosticoRelacionado2CIE11")]
    pub nom_cod_diagnostico_relacionado2_cie11: String,
    #[serde(rename = "codDiagnosticoRelacionado3CIE11")]
    pub cod_diagnostico_relacionado3_cie11: String,
    #[serde(rename = "nomCodDiagnosticoRelacionado3CIE11")]
    pub nom_cod_diagnostico_relacionado3_cie11: String,
    #[serde(rename = "tipoDiagnosticoPrincipal")]
    pub tipo_diagnostico_principal: Option<String>,
    // P11/P12: el PROFESIONAL que atendió, no el paciente. Van declarados aquí
    // y no con `#[serde(flatten)]`, que reordenaría el objeto entero.
    #[serde(rename = "tipoDocumentoIdentificacion")]
    pub tipo_documento_identificacion: Option<String>,
    #[serde(rename = "numDocumentoIdentificacion")]
    pub num_documento_identificacion: Option<String>,
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

/// Arma el AC a partir de las consultas del periodo, ya traídas de la base.
///
/// Agrupa por paciente: un usuario aparece **una vez** con todas sus consultas,
/// y el `consecutivo` de cada consulta es su posición dentro de ese usuario.
///
/// Las consultas se recorren en el orden en que llegan, así que ordenarlas es
/// cosa de quien consulta la base — y hay que hacerlo, o dos corridas del mismo
/// periodo darían consecutivos distintos.
pub fn documento_ac(
    institucion: &FuenteInstitucion,
    consultas: &[FuenteConsulta],
    rec: &mut Recolector,
) -> Documento {
    // Se conserva el orden de aparición: un `HashMap` daría un orden distinto en
    // cada corrida y el archivo dejaría de ser reproducible.
    let mut usuarios: Vec<Usuario> = Vec::new();
    let mut indice: Vec<i64> = Vec::new();

    for c in consultas {
        let pos = match indice.iter().position(|id| *id == c.paciente.id) {
            Some(i) => i,
            None => {
                let mut u = usuario_base(&c.paciente, usuarios.len() as i64 + 1, rec);
                // Fidelidad al Python: la incapacidad se fija con la PRIMERA
                // consulta del paciente en el periodo, no con la última ni con
                // «alguna». Se anota porque no es obvio y no es evidentemente
                // correcto: es lo que hoy se reporta.
                u.incapacidad = if c.genera_incapacidad { "SI" } else { "NO" }.into();
                usuarios.push(u);
                indice.push(c.paciente.id);
                usuarios.len() - 1
            }
        };

        let consecutivo = usuarios[pos].servicios.consultas.len() as i64 + 1;
        let consulta = consulta_de(institucion, c, consecutivo, rec);
        usuarios[pos].servicios.consultas.push(consulta);
    }

    Documento {
        num_documento_id_obligado: institucion.nit.clone(),
        num_factura: String::new(),
        tipo_nota: String::new(),
        num_nota: String::new(),
        usuarios,
    }
}

fn consulta_de(
    institucion: &FuenteInstitucion,
    c: &FuenteConsulta,
    consecutivo: i64,
    rec: &mut Recolector,
) -> Consulta {
    rec.en_registro(c.procedencias.as_deref()); // #84: audita la procedencia de esta consulta
    let permisivo = rec.permite_supuestos();

    macro_rules! exigir {
        ($valor:expr, $campo:literal, $etiqueta:literal, $supuesto:literal) => {{
            let donde = Donde::ac(c.id, $campo, $etiqueta).supuesto($supuesto);
            let d = rec.dato($valor, &donde);
            if permisivo {
                d.o_el_supuesto(Some($supuesto))
            } else {
                d.o_nada()
            }
        }};
        ($valor:expr, $campo:literal, $etiqueta:literal) => {{
            let donde = Donde::ac(c.id, $campo, $etiqueta);
            rec.dato($valor, &donde).o_nada()
        }};
    }

    let cod_consulta = exigir!(
        c.cod_consulta_cups.as_deref(),
        "cod_consulta_cups",
        "CUPS de la consulta",
        "890201"
    );
    let modalidad = exigir!(
        c.modalidad_grupo_servicio.as_deref(),
        "modalidad_grupo_servicio",
        "modalidad del servicio",
        "01"
    );
    let grupo = exigir!(
        c.grupo_servicios.as_deref(),
        "grupo_servicios",
        "grupo de servicios",
        "01"
    );

    let donde_servicio = Donde::ac(c.id, "cod_servicio", "código de servicio").supuesto("328");
    let cod_servicio = {
        let d = rec.numero(c.cod_servicio, &donde_servicio);
        if permisivo {
            d.o_el_supuesto(Some("328"))
        } else {
            d.o_nada()
        }
    };

    // La finalidad no se pide con `dato` sino con `codigo`: el problema no es
    // que falte —nunca falta— sino que el defecto de la base pone un `10` que
    // el MUV no reconoce.
    let donde_finalidad =
        Donde::ac(c.id, "finalidad", "finalidad de la consulta").supuesto("11");
    let finalidad = rec
        .codigo(c.finalidad.as_deref(), &donde_finalidad, |v| {
            if muv::finalidad_existe(v) {
                return Ok(());
            }
            // Los dos casos mandan a sitios distintos, así que se dicen
            // distintos: el «10» es un defecto que nadie eligió y se corrige
            // eligiendo; los demás son de la tabla vieja y se corrigen
            // convirtiendo. Un solo mensaje para ambos no diría qué hacer.
            Err(if v == "10" {
                "«10» es el defecto del sistema, no una finalidad que alguien \
                 haya elegido, y la tabla del MUV va de 11 a 44."
                    .to_string()
            } else {
                format!(
                    "«{v}» es de la tabla de finalidades ANTERIOR a la Resolución \
                     948; el MUV solo admite de 11 a 44. Hay que convertirlo, no \
                     enviarlo."
                )
            })
        })
        .o_nada();

    // ── La PAREJA (finalidad, CUPS), no cada una por su lado (#331) ──────────
    //
    // Hasta aquí se ha comprobado que la finalidad EXISTE. El MUV comprueba algo
    // más: que valga PARA ESE PROCEDIMIENTO (`REFs_FinalidadCups`). O sea que se
    // podía elegir una finalidad válida y un CUPS válido cuya combinación el
    // validador rechaza, y no enterarse hasta el rechazo — que llega semanas
    // después, sobre el archivo del mes entero.
    //
    // **Solo se juzga cuando la tabla cubre ese CUPS.** Son 308 de 13 640
    // (2,26 %), así que tratar la ausencia como rechazo bloquearía casi todo el
    // catálogo (directiva 21, y la ficha lo avisaba). Sin cobertura no se anota
    // nada: no es que la pareja valga, es que no se sabe, y una falta que no se
    // puede afirmar no se inventa (directiva 20).
    //
    // Va DESPUÉS de comprobar que la finalidad existe, y el orden es la regla:
    // al revés, un `10` —el defecto que nadie eligió— produciría además «esta
    // finalidad no vale para 890201», mandando a cambiar el CUPS cuando lo que
    // falta es que alguien elija la finalidad.
    // El `finalidad_existe` de la guarda NO es redundante con el bloque de
    // arriba, y esto lo cazó `el_defecto_diez_no_produce_ademas_la_queja_de_la_
    // pareja` — que se escribió antes de la condición y la puso roja a la
    // primera. Sin él, un `10` produce DOS faltas sobre el mismo campo: «ese
    // código no existe» y «no vale para 890201». La segunda es cierta y es
    // ruido: manda a cambiar el CUPS cuando lo que falta es que alguien elija la
    // finalidad. Una alarma que a veces sobra es la que a la tercera nadie mira
    // (directiva 23).
    if let (Some(f), Some(cups)) = (finalidad.as_deref(), c.cod_consulta_cups.as_deref())
        && muv::finalidad_existe(f)
        && muv::finalidad_admitida_para(f, cups) == Some(false)
    {
        let admitidas = muv::finalidades_de_cups(cups)
            .map(|v| v.join(", "))
            .unwrap_or_default();
        rec.no_vale(
            &donde_finalidad,
            // El mensaje dice QUÉ admite ese CUPS, no solo que esto no vale: sin
            // la lista, quien lo lee tiene que ir a buscarla a un maestro de
            // MinSalud, y en la práctica no lo hace.
            format!(
                "la finalidad «{f}» no la admite el MUV para el CUPS {cups}. \
                 Para ese procedimiento admite: {admitidas}."
            ),
        );
    }

    // La causa va como la finalidad y por lo mismo: el enum del sistema es la
    // tabla anterior a la 948 entera. El supuesto ya NO es «13» —que no existe—
    // sino «38», que es el mismo «enfermedad general» en la tabla nueva.
    //
    // Traducir aquí sí vale y en la migración no, y la diferencia importa: un
    // supuesto lo pide el llamante, se emite solo con `permitir_supuestos` y
    // queda anotado con su etiqueta. Reescribir la columna, en cambio, deja un
    // código válido que nadie afirmó y que ya no detecta nada.
    let donde_causa = Donde::ac(c.id, "causa_externa", "causa o motivo de la atención")
        .supuesto("38");
    let causa_dato = rec.codigo(c.causa_externa.as_deref(), &donde_causa, |v| {
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
    let diagnostico = exigir!(
        c.diagnostico_principal.as_deref(),
        "diagnostico_principal",
        "diagnóstico principal"
    );
    let tipo_diagnostico = exigir!(
        c.tipo_diagnostico.as_deref(),
        "tipo_diagnostico",
        "tipo de diagnóstico principal",
        "01"
    );

    let (tipo_doc, num_doc) = documento_profesional(
        &c.profesional,
        rec,
        Donde::ac(
            c.id,
            "medico.tipo_documento",
            "tipo de documento del profesional que atendió",
        ),
        Donde::ac(
            c.id,
            "medico.numero_documento",
            "número de documento del profesional que atendió",
        ),
    );

    let donde_valor = Donde::ac(c.id, "valor_servicio", "valor del servicio").supuesto("0");
    let vr_servicio = {
        let d = rec.numero(c.valor_servicio, &donde_valor);
        if permisivo {
            d.o_el_supuesto(Some("0"))
        } else {
            d.o_nada()
        }
    };
    let concepto = exigir!(
        c.concepto_recaudo.as_deref(),
        "concepto_recaudo",
        "concepto de recaudo",
        "05"
    );

    // El CIE-11 se deriva del CIE-10 **de origen**, no del que acabe saliendo:
    // si el diagnóstico faltara y entrara el supuesto, derivar de él daría un
    // CIE-11 para un diagnóstico que nadie hizo.
    // LOS RELACIONADOS PASAN POR EL RECOLECTOR, y hasta el 2026-08-16 no lo
    // hacían: salían con `vacio_a_nulo` directo, así que `auditar_procedencia` no
    // los veía JAMÁS — mientras la pantalla les dibuja su casilla «Confirmo este
    // valor — va a facturación» y los tres están en `CAMPOS_RIPS_DIAGNOSTICO`.
    // Lo trajo la revisión de la directiva 25 sobre `1c570350`.
    //
    // Es la simétrica del arreglo que el commit anterior hizo con `cantidad`: una
    // casilla que promete un efecto que no ocurre es peor que no tenerla, porque
    // el clínico gasta la atención en confirmar algo que nadie va a mirar.
    //
    // Van con `presente` y no con `dato`: son OPCIONALES —«no hay relacionado» es
    // una respuesta legítima y no una falta—, así que solo se auditan cuando
    // TIENEN valor. Con `dato` se exigirían tres diagnósticos en toda consulta,
    // que es el sesgo a cerrar (directiva 21).
    let relacionado = |campo: &'static str, etiqueta: &'static str, v: &Option<String>, rec: &mut Recolector| {
        let valor = vacio_a_nulo(v);
        if valor.is_some() {
            rec.presente(&Donde::ac(c.id, campo, etiqueta));
        }
        valor
    };
    let relacionado_1 = relacionado(
        "diagnostico_relacionado1",
        "diagnóstico relacionado 1",
        &c.diagnostico_relacionado1,
        rec,
    );
    let relacionado_2 = relacionado(
        "diagnostico_relacionado2",
        "diagnóstico relacionado 2",
        &c.diagnostico_relacionado2,
        rec,
    );
    let relacionado_3 = relacionado(
        "diagnostico_relacionado3",
        "diagnóstico relacionado 3",
        &c.diagnostico_relacionado3,
        rec,
    );

    let (cie_p, nom_p) = cie11::par(c.diagnostico_principal.as_deref());
    let (cie_1, nom_1) = cie11::par(c.diagnostico_relacionado1.as_deref());
    let (cie_2, nom_2) = cie11::par(c.diagnostico_relacionado2.as_deref());
    let (cie_3, nom_3) = cie11::par(c.diagnostico_relacionado3.as_deref());

    Consulta {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        fecha_inicio_atencion: fecha_dt(&c.fecha),
        num_autorizacion: Some(c.num_autorizacion.clone().unwrap_or_default()),
        cod_consulta,
        modalidad_grupo_servicio_tec_sal: modalidad,
        grupo_servicios: grupo,
        cod_servicio,
        finalidad_tecnologia_salud: finalidad,
        causa_motivo_atencion: causa,
        cod_diagnostico_principal: diagnostico,
        cod_diagnostico_relacionado1: relacionado_1,
        cod_diagnostico_relacionado2: relacionado_2,
        cod_diagnostico_relacionado3: relacionado_3,
        cod_diagnostico_principal_cie11: cie_p.to_string(),
        nom_cod_diagnostico_principal_cie11: nom_p.to_string(),
        cod_diagnostico_relacionado1_cie11: cie_1.to_string(),
        nom_cod_diagnostico_relacionado1_cie11: nom_1.to_string(),
        cod_diagnostico_relacionado2_cie11: cie_2.to_string(),
        nom_cod_diagnostico_relacionado2_cie11: nom_2.to_string(),
        cod_diagnostico_relacionado3_cie11: cie_3.to_string(),
        nom_cod_diagnostico_relacionado3_cie11: nom_3.to_string(),
        tipo_diagnostico_principal: tipo_diagnostico,
        tipo_documento_identificacion: tipo_doc,
        num_documento_identificacion: num_doc,
        vr_servicio,
        concepto_recaudo: concepto,
        // El copago SÍ admite cero de verdad: la mayoría de las atenciones no lo
        // llevan. Cero aquí es un dato, no un hueco.
        valor_pago_moderador: c.valor_pago_moderador.unwrap_or(0),
        num_fev_pago_moderador: Some(c.num_fev_pago_moderador.clone().unwrap_or_default()),
        consecutivo,
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
            procedencias: None,
        }
    }

    /// Una consulta a la que no le falta nada: la línea de base contra la que se
    /// mide cada hueco.
    fn completa(id: i64, p: FuentePaciente) -> FuenteConsulta {
        FuenteConsulta {
            procedencias: None,
            id,
            fecha: NaiveDate::from_ymd_opt(2026, 3, 15)
                .unwrap()
                .and_hms_opt(9, 30, 15)
                .unwrap(),
            num_autorizacion: None,
            cod_consulta_cups: Some("890201".into()),
            modalidad_grupo_servicio: Some("01".into()),
            grupo_servicios: Some("01".into()),
            cod_servicio: Some(328),
            finalidad: Some("11".into()),
            causa_externa: Some("38".into()),
            diagnostico_principal: Some("A000".into()),
            diagnostico_relacionado1: None,
            diagnostico_relacionado2: None,
            diagnostico_relacionado3: None,
            tipo_diagnostico: Some("01".into()),
            valor_servicio: Some(45_000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: None,
            num_fev_pago_moderador: None,
            genera_incapacidad: false,
            paciente: p,
            profesional: FuenteProfesional {
                tipo_documento: Some("CC".into()),
                numero_documento: Some("71234567".into()),
            },
        }
    }

    #[test]
    fn una_consulta_completa_no_deja_faltas() {
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[completa(1, paciente(7, "1234567"))], &mut rec);
        assert!(
            rec.faltantes().is_empty(),
            "faltas inesperadas: {:?}",
            rec.faltantes()
        );
        assert_eq!(doc.num_documento_id_obligado, "900123456");
        assert_eq!(doc.usuarios.len(), 1);
        let s = &doc.usuarios[0].servicios.consultas[0];
        assert_eq!(s.cod_prestador, "050010123401");
        assert_eq!(s.fecha_inicio_atencion, "2026-03-15 09:30", "sin segundos");
        assert_eq!(s.consecutivo, 1);
        assert_eq!(s.vr_servicio, Some(45_000));
        assert_eq!(s.valor_pago_moderador, 0, "cero es un dato, no un hueco");
        // En los CINCO documentos, ausente se escribe cadena vacía. En el
        // FEV-RIPS se convierte en `null` —lo hace `fev::consolidar`—, y por eso
        // el campo es `Option`: para poder distinguir las dos formas sin dos
        // structs. Ver `rips::fev`.
        assert_eq!(s.num_autorizacion.as_deref(), Some(""), "ausente se escribe cadena vacía");
    }

    // ── La PAREJA (finalidad, CUPS) — #331 ──────────────────────────────────

    /// **El caso que #331 existe para cazar.** Finalidad válida, CUPS válido, y
    /// la combinación que el MUV rechaza. Antes salía sin una sola falta y el
    /// rechazo llegaba semanas después, sobre el archivo del mes entero.
    #[test]
    fn una_finalidad_valida_para_otro_cups_se_anota() {
        let mut c = completa(1, paciente(7, "1"));
        // La 12 existe en el maestro y NO está entre las nueve de 890201.
        c.finalidad = Some("12".into());
        c.cod_consulta_cups = Some("890201".into());

        let mut rec = Recolector::estricto();
        documento_ac(&institucion(), &[c], &mut rec);

        let faltas = rec.faltantes();
        let dicho = format!("{faltas:?}");
        assert!(
            dicho.contains("no la admite el MUV para el CUPS 890201"),
            "no se anotó la pareja inválida: {dicho}"
        );
        // Y DICE cuáles sí valen: sin la lista, quien lo lee tiene que ir a
        // buscarla a un maestro de MinSalud, y en la práctica no lo hace.
        assert!(dicho.contains("11"), "el mensaje no dice qué finalidades admite: {dicho}");
    }

    /// **La pareja de la anterior, y la que impide el desastre.** El CUPS que NO
    /// está en la tabla —el 97,74 % del catálogo— no se juzga: no es que la
    /// combinación valga, es que no se sabe, y una falta que no se puede afirmar
    /// no se inventa.
    ///
    /// Sin esta mitad, la regla habría bloqueado casi todos los procedimientos
    /// legítimos y las pruebas de arriba habrían pasado igual.
    #[test]
    fn un_cups_que_la_tabla_no_cubre_no_se_juzga() {
        let mut c = completa(1, paciente(7, "1"));
        c.finalidad = Some("12".into());
        // 890202 NO está en `REFs_FinalidadCups` (medido el 2026-08-07).
        c.cod_consulta_cups = Some("890202".into());

        let mut rec = Recolector::estricto();
        documento_ac(&institucion(), &[c], &mut rec);

        let dicho = format!("{:?}", rec.faltantes());
        assert!(
            !dicho.contains("no la admite el MUV"),
            "se rechazó una pareja sobre un CUPS que la tabla no cubre: {dicho}"
        );
    }

    /// Y la tercera: la combinación que SÍ vale no anota nada. Sin ella, una
    /// regla que dijera «no vale» a todo pasaría las dos de arriba.
    #[test]
    fn la_pareja_admitida_no_anota_nada() {
        let mut rec = Recolector::estricto();
        // `completa` ya usa 11 + 890201, que es una pareja admitida.
        documento_ac(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let dicho = format!("{:?}", rec.faltantes());
        assert!(!dicho.contains("no la admite el MUV"), "cerró de más: {dicho}");
    }

    /// **El orden importa, y por eso se prueba.** Con el `10` —el defecto que
    /// nadie eligió— la falta tiene que ser «ese código no existe», y NO además
    /// «no vale para 890201»: lo segundo manda a cambiar el CUPS cuando lo que
    /// falta es que alguien elija la finalidad.
    #[test]
    fn el_defecto_diez_no_produce_ademas_la_queja_de_la_pareja() {
        let mut c = completa(1, paciente(7, "1"));
        c.finalidad = Some("10".into());

        let mut rec = Recolector::estricto();
        documento_ac(&institucion(), &[c], &mut rec);

        let dicho = format!("{:?}", rec.faltantes());
        assert!(dicho.contains("defecto del sistema"), "se perdió la queja del 10: {dicho}");
        assert!(
            !dicho.contains("no la admite el MUV"),
            "el 10 produjo además la queja de la pareja, que manda a corregir lo que no es: {dicho}"
        );
    }

    #[test]
    fn el_cie11_se_deriva_del_diagnostico() {
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let s = &doc.usuarios[0].servicios.consultas[0];
        assert_eq!(s.cod_diagnostico_principal_cie11, "1A00&XN8P1&XN8P1");
        assert!(!s.nom_cod_diagnostico_principal_cie11.is_empty());
        // Y los relacionados que no hay quedan vacíos, no con el del principal.
        assert_eq!(s.cod_diagnostico_relacionado1_cie11, "");
        assert_eq!(s.cod_diagnostico_relacionado1, None);
    }

    #[test]
    fn el_diez_de_finalidad_sale_en_el_informe_de_cierre() {
        // El caso que motiva la corrección: el campo viene LLENO —nunca falta—
        // y aun así es un código que el MUV no conoce.
        let mut c = completa(1, paciente(7, "1"));
        c.finalidad = Some("10".into());
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ac(&institucion(), &[c], &mut rec);

        let f = rec.faltantes();
        assert_eq!(f.len(), 1, "una sola línea, no dos");
        assert_eq!(f[0].campo, "finalidad");
        let motivo = f[0].motivo.as_deref().expect("lleva motivo, no es una ausencia");
        assert!(motivo.contains("11 a 44"), "dice cuáles sí valen: {motivo}");
        assert!(motivo.contains("defecto"), "dice que nadie la eligió: {motivo}");
        // Y NO se sustituye por 11: elegir la finalidad es del médico.
        assert_eq!(
            doc.usuarios[0].servicios.consultas[0].finalidad_tecnologia_salud.as_deref(),
            Some("10")
        );
    }

    #[test]
    fn la_tabla_vieja_de_finalidad_se_distingue_del_defecto() {
        // Medido el 2026-08-01: NINGUNO de los nueve valores del enum del
        // sistema (02..10) está en la tabla de la 948, que va de 11 a 44. No es
        // solo el «10»: es que la tabla entera es la de antes de la 948. Los dos
        // casos se corrigen distinto, así que el informe los dice distinto.
        let mut c = completa(1, paciente(7, "1"));
        c.finalidad = Some("06".into()); // ATENCION_PARTO, de la tabla anterior
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ac(&institucion(), &[c], &mut rec);

        let motivo = rec.faltantes()[0].motivo.as_deref().unwrap();
        assert!(motivo.contains("ANTERIOR"), "{motivo}");
        assert!(motivo.contains("convertirlo"), "dice qué hacer: {motivo}");
        assert!(!motivo.contains("defecto"), "este SÍ lo eligió alguien");
        assert_eq!(
            doc.usuarios[0].servicios.consultas[0].finalidad_tecnologia_salud.as_deref(),
            Some("06"),
            "se devuelve tal cual: sustituirlo escondería lo que hay en la base"
        );
    }

    #[test]
    fn la_causa_de_la_tabla_vieja_tambien_sale_en_el_cierre() {
        // El mismo defecto en el campo de al lado. «13» era «enfermedad general»
        // en la tabla anterior y no existe en la de la 948, donde ese concepto
        // es el 38.
        let mut c = completa(1, paciente(7, "1"));
        c.causa_externa = Some("13".into());
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ac(&institucion(), &[c], &mut rec);

        let f = rec.faltantes();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].campo, "causa_externa");
        let motivo = f[0].motivo.as_deref().unwrap();
        assert!(motivo.contains("21 a 49"), "{motivo}");
        assert!(motivo.contains("38"), "dice cuál es hoy: {motivo}");
        assert_eq!(
            doc.usuarios[0].servicios.consultas[0].causa_motivo_atencion.as_deref(),
            Some("13"),
            "se devuelve tal cual, no se traduce por su cuenta"
        );
    }

    #[test]
    fn sin_causa_y_con_supuestos_entra_el_38_y_no_el_13() {
        // El supuesto se actualizó con la tabla. Si hubiera quedado el «13», el
        // camino permisivo produciría un archivo que el MUV rechaza — una salida
        // de emergencia que no salva a nadie.
        let mut c = completa(1, paciente(7, "1"));
        c.causa_externa = None;
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ac(&institucion(), &[c], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.consultas[0].causa_motivo_atencion.as_deref(),
            Some("38")
        );
        assert!(muv::causa_motivo_existe("38"), "y el supuesto es válido");
    }

    #[test]
    fn sin_datos_de_facturacion_el_estricto_no_deja_cerrar() {
        let mut c = completa(1, paciente(7, "1"));
        c.valor_servicio = None;
        c.concepto_recaudo = None;
        c.cod_consulta_cups = None;
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[c], &mut rec);

        let s = &doc.usuarios[0].servicios.consultas[0];
        assert_eq!(s.vr_servicio, None, "no se rellena con 0");
        assert_eq!(s.cod_consulta, None, "no se rellena con 890201");
        assert_eq!(rec.faltantes().len(), 3);
        assert!(rec.cerrar().is_err(), "el cierre estricto tiene que fallar");
    }

    /// #84 y su pareja: el generador ACTIVA la auditoría de procedencia
    /// (`en_registro`) por registro. Con la finalidad (campo DURO) `derivado` por la
    /// máquina, el recolector la marca sin respaldo; sin procedencia declarada, no
    /// se audita. Es el caso rojo de la llamada `en_registro` de `consulta_de`:
    /// quitarla deja los dos asserts sin cumplirse.
    #[test]
    fn el_generador_activa_la_auditoria_de_procedencia() {
        let mut c = completa(1, paciente(7, "1"));
        c.procedencias = Some(r#"{"finalidad":"derivado"}"#.into());
        let mut rec = Recolector::estricto();
        let _ = documento_ac(&institucion(), &[c], &mut rec);
        assert!(
            rec.sin_respaldo().iter().any(|f| f.campo == "finalidad"),
            "la finalidad derivada (dura, sin aval humano) tiene que salir sin respaldo"
        );

        // Un registro sin procedencia declarada no genera ruido (rollout seguro).
        let c2 = completa(2, paciente(8, "2"));
        let mut rec2 = Recolector::estricto();
        let _ = documento_ac(&institucion(), &[c2], &mut rec2);
        assert!(rec2.sin_respaldo().is_empty(), "sin procedencia declarada no se audita");
    }

    /// **EL DIAGNÓSTICO RELACIONADO CON VALOR Y SIN AVAL SALE SIN RESPALDO**, y
    /// su pareja: sin valor, no sale nada.
    ///
    /// Hasta el 2026-08-16 los tres relacionados se emitían con `vacio_a_nulo`
    /// directo, sin pasar por el recolector, así que `auditar_procedencia` no los
    /// veía jamás — y la pantalla les dibuja su casilla «Confirmo este valor — va
    /// a facturación». Una casilla que promete un efecto que no ocurre gasta la
    /// atención del clínico en algo que nadie mira. Lo trajo la revisión de la
    /// directiva 25 sobre `1c570350`.
    ///
    /// Validada contra su MUTANTE —la versión anterior, con `vacio_a_nulo`
    /// directo—: se pone roja con nueve faltas en la lista y **ninguna** es el
    /// relacionado, que es fallar por la vía real y no por el arnés.
    ///
    /// La pareja es lo que impide el sesgo a cerrar: los relacionados son
    /// OPCIONALES, y exigir tres diagnósticos en toda consulta sería peor que el
    /// defecto.
    #[test]
    fn el_diagnostico_relacionado_con_valor_se_audita_y_sin_valor_no() {
        // ROJO: hay relacionado 1 y la procedencia declarada no lo respalda.
        let mut c = completa(1, paciente(7, "1"));
        c.diagnostico_relacionado1 = Some("J069".into());
        c.diagnostico_relacionado2 = None;
        c.diagnostico_relacionado3 = Some("".into());
        c.procedencias = Some(r#"{"diagnostico_principal":"confirmado"}"#.into());
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[c], &mut rec);
        assert_eq!(
            doc.usuarios[0].servicios.consultas[0].cod_diagnostico_relacionado1.as_deref(),
            Some("J069"),
            "y sigue saliendo al archivo: auditar no es bloquear"
        );
        assert!(
            rec.sin_respaldo().iter().any(|f| f.campo == "diagnostico_relacionado1"),
            "un relacionado puesto que nadie afirmó tiene que salir sin respaldo: {:?}",
            rec.sin_respaldo()
        );

        // VERDE: los que no tienen valor no producen NADA. «No hay relacionado»
        // es una respuesta legítima.
        for vacio in ["diagnostico_relacionado2", "diagnostico_relacionado3"] {
            assert!(
                !rec.sin_respaldo().iter().any(|f| f.campo == vacio),
                "`{vacio}` no tiene valor: no hay nada que respaldar"
            );
        }
    }

    #[test]
    fn con_supuestos_permitidos_sale_pero_queda_anotado() {
        let mut c = completa(1, paciente(7, "1"));
        c.valor_servicio = None;
        c.cod_consulta_cups = None;
        c.cod_servicio = None;
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_ac(&institucion(), &[c], &mut rec);

        let s = &doc.usuarios[0].servicios.consultas[0];
        assert_eq!(s.vr_servicio, Some(0));
        assert_eq!(s.cod_consulta.as_deref(), Some("890201"));
        assert_eq!(s.cod_servicio, Some(328), "el supuesto numérico se convierte");
        assert_eq!(rec.faltantes().len(), 3, "sale, pero no en silencio");
        assert!(rec.cerrar().is_ok());
    }

    #[test]
    fn el_valor_cero_es_un_dato_y_no_se_anota() {
        // Un servicio puede ser gratuito. Si `0` se contara como ausente, cada
        // consulta gratuita saldría en el informe de cierre como un error.
        let mut c = completa(1, paciente(7, "1"));
        c.valor_servicio = Some(0);
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[c], &mut rec);
        assert!(rec.faltantes().is_empty());
        assert_eq!(doc.usuarios[0].servicios.consultas[0].vr_servicio, Some(0));
    }

    #[test]
    fn dos_consultas_del_mismo_paciente_van_bajo_un_solo_usuario() {
        let p = paciente(7, "1234567");
        let mut rec = Recolector::estricto();
        let doc = documento_ac(
            &institucion(),
            &[completa(1, p.clone()), completa(2, p)],
            &mut rec,
        );
        assert_eq!(doc.usuarios.len(), 1, "el usuario no se repite");
        let cs = &doc.usuarios[0].servicios.consultas;
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].consecutivo, 1);
        assert_eq!(cs[1].consecutivo, 2, "el consecutivo es por usuario");
    }

    #[test]
    fn dos_pacientes_son_dos_usuarios_numerados_en_orden() {
        let mut rec = Recolector::estricto();
        let doc = documento_ac(
            &institucion(),
            &[
                completa(1, paciente(7, "111")),
                completa(2, paciente(9, "222")),
                completa(3, paciente(7, "111")),
            ],
            &mut rec,
        );
        assert_eq!(doc.usuarios.len(), 2);
        assert_eq!(doc.usuarios[0].consecutivo, 1);
        assert_eq!(doc.usuarios[0].num_documento, "111");
        assert_eq!(doc.usuarios[0].servicios.consultas.len(), 2);
        assert_eq!(doc.usuarios[1].consecutivo, 2);
        assert_eq!(doc.usuarios[1].servicios.consultas.len(), 1);
    }

    #[test]
    fn la_incapacidad_la_fija_la_primera_consulta_del_paciente() {
        // Fidelidad al Python, y queda escrito porque no es obvio: si la primera
        // consulta del periodo no generó incapacidad y la segunda sí, el usuario
        // sale con "NO".
        let p = paciente(7, "1");
        let mut segunda = completa(2, p.clone());
        segunda.genera_incapacidad = true;
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[completa(1, p), segunda], &mut rec);
        assert_eq!(doc.usuarios[0].incapacidad, "NO");
    }

    #[test]
    fn el_orden_de_los_campos_es_el_de_la_948() {
        // No es cosmético: es lo que permite comparar el JSON con el del Python
        // byte a byte, que es la condición del ADR para apagarlo. Un
        // `#[serde(flatten)]` de más reordena el objeto entero en alfabético.
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[completa(1, paciente(7, "1"))], &mut rec);
        let json = serde_json::to_string(&doc).unwrap();

        let esperado = [
            "numDocumentoIdObligado",
            "numFactura",
            "tipoNota",
            "numNota",
            "usuarios",
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "tipoUsuario",
            "fechaNacimiento",
            "codSexo",
            "codPaisResidencia",
            "codMunicipioResidencia",
            "codZonaTerritorialResidencia",
            "codPaisOrigen",
            "consecutivo",
            "incapacidad",
            "servicios",
            "consultas",
            "codPrestador",
            "fechaInicioAtencion",
            "numAutorizacion",
            "codConsulta",
            "modalidadGrupoServicioTecSal",
            "grupoServicios",
            "codServicio",
            "finalidadTecnologiaSalud",
            "causaMotivoAtencion",
            "codDiagnosticoPrincipal",
            "codDiagnosticoRelacionado1",
            "codDiagnosticoRelacionado2",
            "codDiagnosticoRelacionado3",
            "codDiagnosticoPrincipalCIE11",
            "nomCodDiagnosticoPrincipalCIE11",
            "codDiagnosticoRelacionado1CIE11",
            "nomCodDiagnosticoRelacionado1CIE11",
            "codDiagnosticoRelacionado2CIE11",
            "nomCodDiagnosticoRelacionado2CIE11",
            "codDiagnosticoRelacionado3CIE11",
            "nomCodDiagnosticoRelacionado3CIE11",
            "tipoDiagnosticoPrincipal",
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "vrServicio",
            "conceptoRecaudo",
            "valorPagoModerador",
            "numFEVPagoModerador",
            "consecutivo",
        ];
        let mut desde = 0usize;
        for campo in esperado {
            let aguja = format!("\"{campo}\":");
            let pos = json[desde..]
                .find(&aguja)
                .unwrap_or_else(|| panic!("«{campo}» no aparece tras la posición {desde}\n{json}"));
            desde += pos + aguja.len();
        }
    }

    #[test]
    fn un_relacionado_en_blanco_se_escribe_null_y_no_cadena_vacia() {
        // El MUV contrasta el campo contra la tabla de la CIE, y "" no está en
        // ninguna tabla: «El campo código diagnóstico relacionado [] no es válido».
        let mut c = completa(1, paciente(7, "1"));
        c.diagnostico_relacionado1 = Some("   ".into());
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[c], &mut rec);
        let json = serde_json::to_string(&doc.usuarios[0].servicios.consultas[0]).unwrap();
        assert!(json.contains("\"codDiagnosticoRelacionado1\":null"), "{json}");
    }

    #[test]
    fn sin_consultas_el_documento_sale_vacio_y_no_falla() {
        // Un periodo sin atenciones es un periodo sin atenciones, no un error.
        let mut rec = Recolector::estricto();
        let doc = documento_ac(&institucion(), &[], &mut rec);
        assert!(doc.usuarios.is_empty());
        assert!(rec.cerrar().is_ok());
    }
}
