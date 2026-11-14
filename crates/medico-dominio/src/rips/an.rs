// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **AN — el arreglo `recienNacidos`.** Sexto de los siete de la 948 (#67).
//!
//! La 948 define SIETE arreglos y medico emitía CINCO. Éste faltaba por una razón
//! que no era de código: **no había dónde capturar el dato**. La tabla
//! `recien_nacido` (migración `0062`, #249) es la que lo desbloquea.
//!
//! # La forma sale del VALIDADOR, no de la memoria
//!
//! Las 18 claves y su orden salen de `backend/datos/normativa/campos_fev948.json`
//! —el mismo artefacto que ata `campos_de_la_948.rs`— y los tipos y longitudes de
//! `Rips.RecienNacidos` del contenedor `fevrips-db`, leídos el 2026-08-16.
//!
//! # El recién nacido cuelga de la MADRE, y eso no es un detalle de modelado
//!
//! El arreglo viaja dentro de `usuarios[].servicios` de la madre, no como un
//! usuario propio, y el validador lo ata con `FK_RecienNacidos_Usuarios_UsuarioId`.
//! Un recién nacido no tiene aún identificación propia obligatoria: sus dos campos
//! de documento son NULABLES en el validador y aquí también.
//!
//! # Un nacimiento NO abre usuario por su cuenta, y se une por DOCUMENTO
//!
//! A diferencia del AC o del AH, esto **no crea usuarios**: se cuelga de los que
//! ya existen por sus otras atenciones, y la llave es la misma con que el FEV
//! funde los cinco documentos — el par (tipo, número) del documento de la madre,
//! no su `paciente.id`, que después de fundir ya no existe.
//!
//! **Qué pasa si la madre no tiene ninguna otra atención en el periodo**: no se
//! emite, y se DEVUELVE su id para que quien cierra el mes lo vea. No es que el
//! MUV lo rechace —eso no está medido y no se afirma—: es que un usuario cuyo
//! único servicio fuese el arreglo de recién nacidos es una forma de paquete que
//! este producto nunca ha enviado, y estrenarla en el cierre de un cliente sin
//! haberla probado contra el validador es apostar su facturación. Callarlo sería
//! peor todavía (directiva 20), y por eso se reporta.
//!
//! # `peso` va en GRAMOS
//!
//! El validador lo declara `float` y la norma lo llama `peso` a secas. El RIPS lo
//! define en gramos desde la 3374. Aquí el nombre de la columna lleva la unidad
//! (`peso_gramos`) precisamente para que este punto no haya que recordarlo.

use serde::Serialize;

use super::documentos::{fecha_dt, FuenteInstitucion, Usuario};
use super::recolector::{Donde, Recolector};
use crate::cie11;

/// Un recién nacido ya traído de la base, con la madre a la que se une.
#[derive(Debug, Clone)]
pub struct FuenteRecienNacido {
    pub id: i64,
    /// El `paciente.id` de la MADRE. **No es la llave de unión** —después de
    /// fundir el FEV los ids ya no están—; sirve para el rastro de la Ley 1581 y
    /// para poder señalar la fila que se quedó fuera.
    pub madre_paciente_id: i64,
    /// El par que SÍ une: el mismo `(tipoDocumentoIdentificacion,
    /// numDocumentoIdentificacion)` con que `fev::fundir` agrupa a los usuarios.
    pub madre_tipo_documento: String,
    pub madre_numero_documento: String,
    pub tipo_documento: Option<String>,
    pub numero_documento: Option<String>,
    pub fecha_nacimiento: chrono::NaiveDateTime,
    pub edad_gestacional: i64,
    pub num_consultas_prenatal: i64,
    /// `M`/`F`/`I`, el mismo vocabulario que `codSexo` del usuario.
    pub sexo_biologico: String,
    pub peso_gramos: i64,
    pub diagnostico_principal: String,
    pub condicion_destino: Option<String>,
    pub diagnostico_causa_muerte: Option<String>,
    /// Nula mientras el recién nacido siga en el servicio.
    pub fecha_egreso: Option<chrono::NaiveDateTime>,
    pub codigo_vida: Option<String>,
}

/// Un recién nacido tal y como sale al JSON de la 948.
///
/// El orden de los campos ES el orden de emisión de serde, y es el de la norma.
#[derive(Debug, Clone, Serialize)]
pub struct RecienNacido {
    #[serde(rename = "codPrestador")]
    pub cod_prestador: String,
    #[serde(rename = "tipoDocumentoIdentificacion")]
    pub tipo_documento_identificacion: Option<String>,
    #[serde(rename = "numDocumentoIdentificacion")]
    pub num_documento_identificacion: Option<String>,
    #[serde(rename = "fechaNacimiento")]
    pub fecha_nacimiento: String,
    #[serde(rename = "edadGestacional")]
    pub edad_gestacional: i64,
    #[serde(rename = "numConsultasCPrenatal")]
    pub num_consultas_c_prenatal: i64,
    #[serde(rename = "codSexoBiologico")]
    pub cod_sexo_biologico: String,
    pub peso: i64,
    #[serde(rename = "codDiagnosticoPrincipal")]
    pub cod_diagnostico_principal: String,
    #[serde(rename = "codDiagnosticoPrincipalCIE11")]
    pub cod_diagnostico_principal_cie11: String,
    #[serde(rename = "nomCodDiagnosticoPrincipalCIE11")]
    pub nom_cod_diagnostico_principal_cie11: String,
    #[serde(rename = "condicionDestinoUsuarioEgreso")]
    pub condicion_destino_usuario_egreso: Option<String>,
    #[serde(rename = "codDiagnosticoCausaMuerte")]
    pub cod_diagnostico_causa_muerte: Option<String>,
    #[serde(rename = "codDiagnosticoCausaMuerteCIE11")]
    pub cod_diagnostico_causa_muerte_cie11: String,
    #[serde(rename = "nomCodDiagnosticoCausaMuerteCIE11")]
    pub nom_cod_diagnostico_causa_muerte_cie11: String,
    #[serde(rename = "fechaEgreso")]
    pub fecha_egreso: String,
    #[serde(rename = "codigoVIDA")]
    pub codigo_vida: Option<String>,
    pub consecutivo: i64,
}

/// Cuelga los recién nacidos del usuario de su madre, dentro de un documento ya
/// armado.
///
/// Devuelve los `id` de los que **no** encontraron a su madre entre los usuarios.
/// No es un detalle: significa que hubo un parto sin ninguna otra atención de la
/// madre en el periodo, y quien genera el FEV tiene que enterarse en vez de ver
/// un arreglo que falta sin explicación.
pub fn anexar_recien_nacidos(
    institucion: &FuenteInstitucion,
    usuarios: &mut [Usuario],
    recien_nacidos: &[FuenteRecienNacido],
    rec: &mut Recolector,
) -> Vec<i64> {
    let mut huerfanos = Vec::new();
    for rn in recien_nacidos {
        let Some(pos) = usuarios.iter().position(|u| {
            u.tipo_documento == rn.madre_tipo_documento
                && u.num_documento == rn.madre_numero_documento
        }) else {
            huerfanos.push(rn.id);
            continue;
        };
        let consecutivo = usuarios[pos].servicios.recien_nacidos.len() as i64 + 1;
        let fila = recien_nacido_de(institucion, rn, consecutivo, rec);
        usuarios[pos].servicios.recien_nacidos.push(fila);
    }
    huerfanos
}

fn recien_nacido_de(
    institucion: &FuenteInstitucion,
    rn: &FuenteRecienNacido,
    consecutivo: i64,
    rec: &mut Recolector,
) -> RecienNacido {
    // El diagnóstico principal es NOT NULL en la tabla, así que aquí no puede
    // faltar; se pasa por el recolector igual para que un dato vacío que entrara
    // por un importador se anote en vez de emitirse en blanco.
    let donde_dx = Donde::ah(
        rn.id,
        "diagnostico_principal",
        "diagnóstico principal del recién nacido",
    );
    if rn.diagnostico_principal.trim().is_empty() {
        rec.no_vale(
            &donde_dx,
            "el diagnóstico principal del recién nacido está vacío".into(),
        );
    } else {
        rec.presente(&donde_dx);
    }

    // La causa de muerte NO se deduce de la condición de destino: son dos datos.
    // Si murió y no hay causa, se anota como falta — el paquete se rechaza igual,
    // pero aquí se dice cuál fila y por qué.
    let murio = rn.condicion_destino.as_deref() == Some("02");
    let hay_causa = rn
        .diagnostico_causa_muerte
        .as_deref()
        .is_some_and(|c| !c.trim().is_empty());
    if murio && !hay_causa {
        rec.no_vale(
            &Donde::ah(
                rn.id,
                "diagnostico_causa_muerte",
                "causa básica de muerte del recién nacido",
            ),
            "el recién nacido consta como muerto (condición de destino 02) y no \
             tiene causa básica de muerte"
                .into(),
        );
    }

    let (cie_p, nom_p) = cie11::par(Some(rn.diagnostico_principal.as_str()));
    let (cie_m, nom_m) = cie11::par(rn.diagnostico_causa_muerte.as_deref());

    RecienNacido {
        cod_prestador: institucion.codigo_habilitacion.clone(),
        tipo_documento_identificacion: vacio_a_nulo(&rn.tipo_documento),
        num_documento_identificacion: vacio_a_nulo(&rn.numero_documento),
        fecha_nacimiento: fecha_dt(&rn.fecha_nacimiento),
        edad_gestacional: rn.edad_gestacional,
        num_consultas_c_prenatal: rn.num_consultas_prenatal,
        cod_sexo_biologico: rn.sexo_biologico.clone(),
        peso: rn.peso_gramos,
        cod_diagnostico_principal: rn.diagnostico_principal.clone(),
        cod_diagnostico_principal_cie11: cie_p.to_string(),
        nom_cod_diagnostico_principal_cie11: nom_p.to_string(),
        condicion_destino_usuario_egreso: vacio_a_nulo(&rn.condicion_destino),
        cod_diagnostico_causa_muerte: vacio_a_nulo(&rn.diagnostico_causa_muerte),
        cod_diagnostico_causa_muerte_cie11: cie_m.to_string(),
        nom_cod_diagnostico_causa_muerte_cie11: nom_m.to_string(),
        // El que sigue en el servicio no tiene egreso: cadena vacía, que es la
        // forma con que los otros documentos dicen «todavía no» (`fecha_dt` de
        // un `None`), no la fecha de generación.
        fecha_egreso: rn.fecha_egreso.as_ref().map(fecha_dt).unwrap_or_default(),
        codigo_vida: vacio_a_nulo(&rn.codigo_vida),
        consecutivo,
    }
}

/// Un `Some("")` es un dato ausente disfrazado, y el validador rechaza la cadena
/// vacía donde el esquema declara nulable. Misma regla que en el AH.
fn vacio_a_nulo(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rips::documentos::{usuario_base, FuentePaciente};

    fn institucion() -> FuenteInstitucion {
        FuenteInstitucion {
            nit: "900123456".into(),
            codigo_habilitacion: "050010123401".into(),
        }
    }

    fn madre(id: i64) -> FuentePaciente {
        FuentePaciente {
            id,
            tipo_documento: "CC".into(),
            numero_documento: format!("100{id}"),
            fecha_nacimiento: "1998-04-12".into(),
            sexo: "F".into(),
            cod_pais: Some("170".into()),
            cod_municipio: Some("05001".into()),
            zona: Some("02".into()),
            cod_pais_origen: Some("170".into()),
            procedencias: None,
            tipo_usuario: Some("01".into()),
        }
    }

    fn nacido(id: i64, madre_id: i64) -> FuenteRecienNacido {
        let m = madre(madre_id);
        FuenteRecienNacido {
            id,
            madre_paciente_id: madre_id,
            madre_tipo_documento: m.tipo_documento.clone(),
            madre_numero_documento: m.numero_documento,
            tipo_documento: Some("CN".into()),
            numero_documento: Some("9900123456".into()),
            fecha_nacimiento: "2026-08-16T03:20:00".parse().unwrap(),
            edad_gestacional: 39,
            num_consultas_prenatal: 7,
            sexo_biologico: "F".into(),
            peso_gramos: 3250,
            diagnostico_principal: "Z380".into(),
            condicion_destino: Some("01".into()),
            diagnostico_causa_muerte: None,
            fecha_egreso: Some("2026-08-18T10:00:00".parse().unwrap()),
            codigo_vida: Some("VIDA-2026-0001".into()),
        }
    }

    /// Arma un documento con una madre ya presente y le cuelga su recién nacido.
    fn con_una_madre(rns: &[FuenteRecienNacido]) -> (Vec<Usuario>, Vec<i64>, Vec<i64>) {
        let mut rec = Recolector::estricto();
        let mut usuarios = vec![usuario_base(&madre(7), 1, &mut rec)];
        let indice = vec![7_i64];
        let huerfanos = anexar_recien_nacidos(&institucion(), &mut usuarios, rns, &mut rec);
        (usuarios, indice, huerfanos)
    }

    #[test]
    fn el_recien_nacido_cuelga_del_usuario_de_su_madre() {
        let (usuarios, _, huerfanos) = con_una_madre(&[nacido(1, 7)]);
        assert!(huerfanos.is_empty(), "la madre estaba: no hay huérfanos");
        assert_eq!(usuarios[0].servicios.recien_nacidos.len(), 1);
        let rn = &usuarios[0].servicios.recien_nacidos[0];
        assert_eq!(rn.consecutivo, 1);
        assert_eq!(rn.peso, 3250, "el peso va en gramos, tal cual");
        assert_eq!(rn.cod_prestador, "050010123401");
    }

    /// La pareja del de arriba: **un parto cuya madre no tiene atención en el
    /// periodo no se emite, y se DICE**. Sin esta prueba, `anexar` podría estar
    /// tirando filas en silencio y el arreglo saldría corto sin que nadie lo note.
    #[test]
    fn el_nacimiento_sin_madre_en_el_documento_se_reporta_como_huerfano() {
        let (usuarios, _, huerfanos) = con_una_madre(&[nacido(1, 999)]);
        assert_eq!(huerfanos, vec![1], "hay que decir cuál se quedó fuera");
        assert!(usuarios[0].servicios.recien_nacidos.is_empty());
    }

    #[test]
    fn dos_partos_de_la_misma_madre_numeran_el_consecutivo() {
        let (usuarios, _, _) = con_una_madre(&[nacido(1, 7), nacido(2, 7)]);
        let cs: Vec<i64> = usuarios[0]
            .servicios
            .recien_nacidos
            .iter()
            .map(|r| r.consecutivo)
            .collect();
        assert_eq!(cs, vec![1, 2], "los gemelos van 1 y 2, no 1 y 1");
    }

    /// Las 18 claves en el ORDEN de la norma.
    ///
    /// Se mide sobre `to_string`, no sobre `to_value`: `serde_json` sin
    /// `preserve_order` ordena el mapa ALFABÉTICAMENTE, así que un `to_value`
    /// haría pasar esta prueba con los campos declarados en cualquier orden —y
    /// el orden es justo lo que permite comparar el JSON byte a byte con el del
    /// Python. Serializar la struct directamente sí respeta la declaración.
    ///
    /// El CONJUNTO de claves se contrasta aparte contra el artefacto de la
    /// norma, en `tests/campos_de_la_948.rs`; aquí se mide el orden.
    #[test]
    fn las_dieciocho_claves_de_la_948_en_su_orden() {
        let (usuarios, _, _) = con_una_madre(&[nacido(1, 7)]);
        let json = serde_json::to_string(&usuarios[0].servicios.recien_nacidos[0]).unwrap();
        let esperado = [
            "codPrestador",
            "tipoDocumentoIdentificacion",
            "numDocumentoIdentificacion",
            "fechaNacimiento",
            "edadGestacional",
            "numConsultasCPrenatal",
            "codSexoBiologico",
            "peso",
            "codDiagnosticoPrincipal",
            "codDiagnosticoPrincipalCIE11",
            "nomCodDiagnosticoPrincipalCIE11",
            "condicionDestinoUsuarioEgreso",
            "codDiagnosticoCausaMuerte",
            "codDiagnosticoCausaMuerteCIE11",
            "nomCodDiagnosticoCausaMuerteCIE11",
            "fechaEgreso",
            "codigoVIDA",
            "consecutivo",
        ];
        let mut desde = 0usize;
        for clave in esperado {
            let aguja = format!("\"{clave}\":");
            let pos = json[desde..]
                .find(&aguja)
                .unwrap_or_else(|| panic!("«{clave}» no sale, o sale antes de su sitio"));
            desde += pos + aguja.len();
        }
        assert!(
            json.matches(':').count() >= esperado.len(),
            "salieron menos campos de los que la norma nombra"
        );
    }

    /// Sin documento propio se emite `null`, no `""`: enviar cadena vacía donde
    /// el esquema declara nulable es causal de rechazo, y es la misma regla que
    /// el `numAutorizacion` del FEV.
    #[test]
    fn sin_documento_propio_emite_null_y_no_cadena_vacia() {
        let mut n = nacido(1, 7);
        n.tipo_documento = None;
        n.numero_documento = Some("   ".into());
        let (usuarios, _, _) = con_una_madre(&[n]);
        let v = serde_json::to_value(&usuarios[0].servicios.recien_nacidos[0]).unwrap();
        assert!(v["tipoDocumentoIdentificacion"].is_null());
        assert!(
            v["numDocumentoIdentificacion"].is_null(),
            "un `Some(\"   \")` es ausencia disfrazada"
        );
    }

    /// El que sigue en el servicio no tiene egreso, y **no se le pone la fecha de
    /// generación** — que es justo el defecto que el AH tenía con las
    /// hospitalizaciones activas.
    #[test]
    fn el_que_sigue_en_el_servicio_no_estrena_fecha_de_egreso() {
        let mut n = nacido(1, 7);
        n.fecha_egreso = None;
        n.condicion_destino = Some("08".into());
        let (usuarios, _, _) = con_una_madre(&[n]);
        assert_eq!(usuarios[0].servicios.recien_nacidos[0].fecha_egreso, "");
    }

    /// Murió y no se dijo de qué: el recolector tiene que anotarlo. Es la
    /// segunda barandilla de `MuertoSinCausa` — la primera está en la captura,
    /// pero un importador no pasa por ella.
    #[test]
    fn el_muerto_sin_causa_se_anota_como_falta() {
        let mut rec = Recolector::estricto();
        let mut usuarios = vec![usuario_base(&madre(7), 1, &mut rec)];
        let mut n = nacido(1, 7);
        n.condicion_destino = Some("02".into());
        n.diagnostico_causa_muerte = None;
        let antes = rec.faltantes().len();
        anexar_recien_nacidos(&institucion(), &mut usuarios, &[n], &mut rec);
        assert!(
            rec.faltantes().len() > antes,
            "una defunción sin causa tiene que dejar falta"
        );
    }

    /// La pareja del de arriba: con su causa, NO se anota falta. Sin este caso,
    /// un recolector que anotara siempre pasaría igual.
    #[test]
    fn el_muerto_con_su_causa_no_deja_falta() {
        let mut rec = Recolector::estricto();
        let mut usuarios = vec![usuario_base(&madre(7), 1, &mut rec)];
        let mut n = nacido(1, 7);
        n.condicion_destino = Some("02".into());
        n.diagnostico_causa_muerte = Some("P219".into());
        let antes = rec.faltantes().len();
        anexar_recien_nacidos(&institucion(), &mut usuarios, &[n], &mut rec);
        assert_eq!(rec.faltantes().len(), antes);
    }
}
