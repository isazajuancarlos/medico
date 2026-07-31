// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Los documentos del RIPS de la Resolución 948 de 2026, armados sin tocar la
//! base (#67/#250).
//!
//! Cada documento recibe **filas ya leídas** y un [`Recolector`], y devuelve la
//! estructura que se serializa a JSON. La consulta vive en
//! `medico_datos::rips`; aquí solo están las reglas, que es lo que hay que poder
//! probar sin servidor y contrastar contra el Python línea a línea.
//!
//! # El orden de los campos NO es estético
//!
//! Las estructuras declaran los campos en el MISMO orden que los diccionarios de
//! `backend/rips_completo.py`, porque `serde` serializa en orden de declaración y
//! el ADR del port exige que la salida sea **byte a byte idéntica** a la del
//! Python antes de poder apagarlo. Reordenarlos «para agrupar mejor» rompe esa
//! comparación sin cambiar nada semántico, que es la peor clase de diferencia:
//! parece que el port falla cuando no falla.
//!
//! # Los números salen como números
//!
//! `vrServicio`, `codServicio` y `valorPagoModerador` son enteros en la base y
//! enteros en el JSON. El [`Recolector`] trabaja con cadenas —es lo que le
//! permite decir «vacío» sin confundirlo con cero—, así que [`entero`] hace el
//! viaje de ida y vuelta: comprueba la presencia como texto y emite el número.
//! Sin eso, un `0` legítimo saldría como `"0"` y el MUV rechazaría el archivo
//! por formato.

use serde::Serialize;

use super::{Dato, Donde, Recolector};

/// Un dato de texto obligatorio, resuelto según lo que el recolector permita.
///
/// Es el equivalente del `exigir(...)` que el Python repite dentro de cada
/// generador. Aquí es una función y no un cierre por generador para que la
/// decisión —«qué se hace con la ausencia»— esté escrita UNA vez.
pub fn texto(rec: &mut Recolector, valor: Option<&str>, donde: &Donde) -> Option<String> {
    let d = rec.dato(valor, donde);
    if rec.permite_supuestos() {
        d.o_el_supuesto(Recolector::supuesto_de(donde))
    } else {
        d.o_nada()
    }
}

/// Un dato ENTERO obligatorio. Ver la nota del módulo sobre por qué existe.
///
/// **`0` no es ausente.** Un servicio puede valer cero de verdad —una atención
/// gratuita, una capitación—, y confundir «no se registró» con «vale cero» es el
/// error que el recolector existe para no cometer. La conversión a texto se hace
/// aquí, después de que el llamante haya decidido que su cero es un dato.
pub fn entero(rec: &mut Recolector, valor: Option<i64>, donde: &Donde) -> Option<i64> {
    let texto = valor.map(|v| v.to_string());
    match rec.dato(texto.as_deref(), donde) {
        Dato::Hay(_) => valor,
        Dato::Falta if rec.permite_supuestos() => {
            Recolector::supuesto_de(donde).and_then(|s| s.parse().ok())
        }
        Dato::Falta => None,
    }
}

/// Los ocho tipos de documento que la Resolución 948 admite para un profesional.
///
/// **No incluye `RC`** (registro civil), y esa ausencia tiene historia: hasta el
/// 2026-07-26 el Python mandaba en P11/P12 el documento del PACIENTE. Lo destapó
/// el MUV rechazando las atenciones de menores —«valores permitidos
/// CC,CE,CD,PA,SC,PE,DE,PT, valor enviado: RC»—, y lo grave no fue el rechazo:
/// las de adultos PASABAN, atribuyendo el acto médico a un documento que no era
/// el del médico. Un dato falso que nadie habría visto.
pub const TIPOS_DOCUMENTO_PROFESIONAL: &[&str] =
    &["CC", "CE", "CD", "PA", "SC", "PE", "DE", "PT"];

/// Quién ATENDIÓ: los campos P11/P12 de un bloque de servicios.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Profesional {
    pub tipo_documento_identificacion: Option<String>,
    pub num_documento_identificacion: Option<String>,
}

/// Lo que se sabe del profesional que atendió, tal como está en la base.
#[derive(Debug, Clone, Default)]
pub struct DatosProfesional {
    pub tipo_documento: Option<String>,
    pub numero_documento: Option<String>,
}

/// Arma P11/P12 y **no inventa nada**: no hay supuesto posible, porque inventar
/// el documento de un profesional es atribuirle un acto médico a alguien.
pub fn profesional(
    rec: &mut Recolector,
    p: &DatosProfesional,
    donde_tipo: &Donde,
    donde_numero: &Donde,
) -> Profesional {
    let tipo = rec.codigo(p.tipo_documento.as_deref(), donde_tipo, |v| {
        if TIPOS_DOCUMENTO_PROFESIONAL.contains(&v) {
            Ok(())
        } else {
            Err(format!(
                "«{v}» no puede ser el documento de un profesional. La Resolución 948 \
                 solo admite {}.",
                TIPOS_DOCUMENTO_PROFESIONAL.join(", ")
            ))
        }
    });
    let numero = rec.dato(p.numero_documento.as_deref(), donde_numero);
    Profesional {
        // Presente pero imposible se devuelve TAL CUAL, igual que hace `codigo`
        // con un CUM inválido: sustituirlo escondería lo que hay en la base, que
        // es justo el dato que hay que corregir.
        tipo_documento_identificacion: tipo.o_nada(),
        num_documento_identificacion: numero.o_nada(),
    }
}

/// La identificación del usuario en el RIPS, en el orden de la norma.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsuarioRips {
    pub tipo_documento_identificacion: String,
    pub num_documento_identificacion: String,
    pub tipo_usuario: Option<String>,
    pub fecha_nacimiento: String,
    pub cod_sexo: String,
    pub cod_pais_residencia: Option<String>,
    pub cod_municipio_residencia: Option<String>,
    pub cod_zona_territorial_residencia: Option<String>,
    pub cod_pais_origen: Option<String>,
    pub consecutivo: i64,
}

/// El paciente tal como está en la base, para armar su bloque.
#[derive(Debug, Clone)]
pub struct DatosPaciente {
    pub id: i64,
    pub tipo_documento: String,
    pub numero_documento: String,
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub tipo_usuario: Option<String>,
    pub cod_pais: Option<String>,
    pub cod_municipio: Option<String>,
    pub zona: Option<String>,
    pub cod_pais_origen: Option<String>,
}

/// Arma el bloque de identificación del usuario.
///
/// Los supuestos van DECLARADOS y no se emiten salvo que el llamante pida
/// `permitir_supuestos`. El de `codMunicipioResidencia` es el que más daño hacía:
/// `"05001"` reportaba como residente de Medellín a **todo paciente sin
/// municipio, en el país entero**. Tiene la forma correcta, así que el validador
/// lo acepta y el dato falso entra en la estadística de residencia.
pub fn usuario(rec: &mut Recolector, p: &DatosPaciente, consecutivo: i64) -> UsuarioRips {
    let en = |campo, etiqueta| Donde::usuarios(p.id, campo, etiqueta);
    UsuarioRips {
        tipo_documento_identificacion: p.tipo_documento.clone(),
        num_documento_identificacion: p.numero_documento.clone(),
        tipo_usuario: texto(
            rec,
            p.tipo_usuario.as_deref(),
            &en("tipo_usuario", "tipo de usuario").supuesto("01"),
        ),
        fecha_nacimiento: p.fecha_nacimiento.clone(),
        cod_sexo: p.sexo.clone(),
        cod_pais_residencia: texto(
            rec,
            p.cod_pais.as_deref(),
            &en("cod_pais", "país de residencia").supuesto("170"),
        ),
        cod_municipio_residencia: texto(
            rec,
            p.cod_municipio.as_deref(),
            &en("cod_municipio", "municipio de residencia").supuesto("05001"),
        ),
        cod_zona_territorial_residencia: texto(
            rec,
            p.zona.as_deref(),
            &en("zona", "zona territorial").supuesto("01"),
        ),
        cod_pais_origen: texto(
            rec,
            p.cod_pais_origen.as_deref(),
            &en("cod_pais_origen", "país de origen").supuesto("170"),
        ),
        consecutivo,
    }
}

/// Fecha y hora **sin segundos**, que es como las quiere el RIPS.
///
/// El Python iba con `%H:%M:%S` y el MUV lo rechazaba —«Fecha no válida, formato
/// (AAAA-MM-DD HH:MM)»— en `fechaEgreso` y `fechaDispensAdmon`. La ficha M04
/// declara el campo de 16 caracteres, que es lo que mide «2021-08-18 08:10»; con
/// segundos son 19.
///
/// Recibe lo que hay en la base, que es texto, y recorta. Si no tiene la forma
/// esperada se devuelve tal cual: recortar a ciegas una cadena rara daría una
/// fecha inventada, y aquí el dato roto tiene que llegar visible al MUV o al
/// informe, no maquillado.
pub fn fecha_hora(v: &str) -> String {
    let v = v.trim();
    match v.len() {
        // `YYYY-MM-DD HH:MM:SS` → se le quitan los segundos.
        19 if v.as_bytes()[16] == b':' => v[..16].to_string(),
        _ => v.to_string(),
    }
}

/// Solo la fecha, `YYYY-MM-DD`.
pub fn fecha(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 10 { v[..10].to_string() } else { v.to_string() }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un mapeo de juguete: el dominio no carga el maestro de 522 KB para
    /// probar sus reglas.
    fn cie11(c: Option<&str>) -> (String, String) {
        match c.map(|s| s.trim().to_uppercase()) {
            Some(ref s) if s == "J449" => ("CA22".into(), "Chronic obstructive pulmonary disease".into()),
            _ => (String::new(), String::new()),
        }
    }

    fn historia() -> DatosConsulta {
        DatosConsulta {
            id: 7,
            paciente_id: 10,
            fecha: "2026-08-03 09:15:44".into(),
            num_autorizacion: None,
            cod_consulta_cups: Some("890201".into()),
            modalidad_grupo_servicio: Some("01".into()),
            grupo_servicios: Some("01".into()),
            cod_servicio: Some(328),
            finalidad: Some("11".into()),
            causa_externa: Some("13".into()),
            diagnostico_principal: Some("J449".into()),
            diagnostico_relacionado1: None,
            diagnostico_relacionado2: None,
            diagnostico_relacionado3: None,
            tipo_diagnostico: Some("01".into()),
            valor_servicio: Some(45000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: 0,
            num_fev_pago_moderador: None,
            genera_incapacidad: false,
            medico: DatosProfesional {
                tipo_documento: Some("CC".into()),
                numero_documento: Some("71234567".into()),
            },
        }
    }

    fn paciente() -> DatosPaciente {
        DatosPaciente {
            id: 10,
            tipo_documento: "CC".into(),
            numero_documento: "1234".into(),
            fecha_nacimiento: "1990-01-01".into(),
            sexo: "F".into(),
            tipo_usuario: None,
            cod_pais: None,
            cod_municipio: None,
            zona: None,
            cod_pais_origen: None,
        }
    }

    #[test]
    fn la_fecha_pierde_los_segundos_y_nada_mas() {
        assert_eq!(fecha_hora("2021-08-18 08:10:33"), "2021-08-18 08:10");
        assert_eq!(fecha_hora("2021-08-18 08:10"), "2021-08-18 08:10");
        assert_eq!(fecha("2021-08-18 08:10:33"), "2021-08-18");
        // Una cadena rara NO se recorta a ciegas: llega visible.
        assert_eq!(fecha_hora("ayer"), "ayer");
    }

    /// El cero es un DATO, no un hueco: un servicio puede ser gratuito.
    #[test]
    fn el_cero_no_se_confunde_con_ausente() {
        let mut rec = Recolector::estricto();
        let d = Donde::ac(1, "valor_servicio", "valor del servicio").supuesto("0");
        assert_eq!(entero(&mut rec, Some(0), &d), Some(0));
        assert!(rec.faltantes().is_empty(), "un cero legítimo se anotó como falta");

        let mut rec = Recolector::estricto();
        assert_eq!(entero(&mut rec, None, &d), None);
        assert_eq!(rec.faltantes().len(), 1, "la ausencia no se anotó");
    }

    #[test]
    fn el_supuesto_solo_sale_si_el_llamante_lo_pide() {
        let d = Donde::ac(1, "valor_servicio", "valor del servicio").supuesto("0");
        let mut estricto = Recolector::estricto();
        assert_eq!(entero(&mut estricto, None, &d), None);

        let mut permisivo = Recolector::permitiendo_supuestos();
        assert_eq!(entero(&mut permisivo, None, &d), Some(0));
        assert_eq!(permisivo.faltantes().len(), 1, "lo supuesto se emitió y NO se anotó");
    }

    #[test]
    fn el_paciente_sin_datos_no_se_rellena_de_colombia_ni_de_medellin() {
        let mut rec = Recolector::estricto();
        let u = usuario(&mut rec, &paciente(), 1);
        assert_eq!(u.cod_municipio_residencia, None, "se reportó residencia en Medellín");
        assert_eq!(u.cod_pais_residencia, None);
        assert_eq!(u.tipo_usuario, None);
        assert_eq!(u.consecutivo, 1);
        // Cinco faltas, una por campo, todas con dónde buscarlas: son los mismos
        // cinco `exigir` que tiene `_usuario_base` en el Python.
        assert_eq!(rec.faltantes().len(), 5);
        assert!(rec.faltantes().iter().all(|f| f.registro_id == 10));
    }

    #[test]
    fn con_supuestos_el_paciente_sale_completo_y_queda_registrado() {
        let mut rec = Recolector::permitiendo_supuestos();
        let u = usuario(&mut rec, &paciente(), 1);
        assert_eq!(u.cod_municipio_residencia.as_deref(), Some("05001"));
        assert_eq!(u.cod_pais_residencia.as_deref(), Some("170"));
        assert_eq!(
            rec.faltantes().len(),
            5,
            "se emitieron supuestos sin dejar constancia de cuáles"
        );
    }

    /// El documento del PACIENTE en el campo del profesional es el fallo que el
    /// MUV destapó. `RC` es de menor de edad y no puede firmar una atención.
    #[test]
    fn un_documento_imposible_de_profesional_se_anota_y_se_devuelve_tal_cual() {
        let mut rec = Recolector::estricto();
        let p = DatosProfesional {
            tipo_documento: Some("RC".into()),
            numero_documento: Some("999".into()),
        };
        let salida = profesional(
            &mut rec,
            &p,
            &Donde::ac(7, "medico.tipo_documento", "tipo de documento del profesional"),
            &Donde::ac(7, "medico.numero_documento", "número de documento del profesional"),
        );
        assert_eq!(
            salida.tipo_documento_identificacion.as_deref(),
            Some("RC"),
            "se sustituyó el valor y con eso se escondió lo que hay que corregir"
        );
        assert_eq!(rec.faltantes().len(), 1);
        assert!(rec.faltantes()[0].motivo.as_ref().unwrap().contains("Resolución 948"));
    }

    #[test]
    fn sin_documento_del_profesional_no_hay_supuesto() {
        let mut rec = Recolector::permitiendo_supuestos();
        let salida = profesional(
            &mut rec,
            &DatosProfesional::default(),
            &Donde::ac(7, "medico.tipo_documento", "tipo de documento del profesional"),
            &Donde::ac(7, "medico.numero_documento", "número de documento del profesional"),
        );
        assert_eq!(
            salida.tipo_documento_identificacion, None,
            "se inventó el documento de un profesional, o sea la autoría de un acto médico"
        );
        assert_eq!(salida.num_documento_identificacion, None);
        assert_eq!(rec.faltantes().len(), 2);
    }

    #[test]
    fn los_ocho_tipos_de_la_norma_pasan_y_ninguno_mas() {
        for t in TIPOS_DOCUMENTO_PROFESIONAL {
            let mut rec = Recolector::estricto();
            profesional(
                &mut rec,
                &DatosProfesional {
                    tipo_documento: Some((*t).into()),
                    numero_documento: Some("1".into()),
                },
                &Donde::ac(1, "medico.tipo_documento", "tipo"),
                &Donde::ac(1, "medico.numero_documento", "número"),
            );
            assert!(rec.faltantes().is_empty(), "el tipo válido {t} se rechazó");
        }
        assert!(!TIPOS_DOCUMENTO_PROFESIONAL.contains(&"RC"));
        assert!(!TIPOS_DOCUMENTO_PROFESIONAL.contains(&"TI"));
    }

    /// LA PRUEBA QUE SOSTIENE EL «BYTE A BYTE».
    ///
    /// Fija los nombres de los campos Y su orden, que es lo que hay que poder
    /// comparar contra la salida del Python antes de apagarlo. La lista de
    /// abajo se lee al lado del diccionario `consulta` de
    /// `backend/rips_completo.py:488`: si alguien reordena la estructura «para
    /// agrupar mejor», esto se pone rojo — que es justo lo que tiene que pasar,
    /// porque sería una diferencia de bytes sin diferencia semántica, la peor
    /// clase.
    #[test]
    fn los_campos_de_ac_van_en_el_orden_de_la_norma() {
        let mut rec = Recolector::estricto();
        let c = consulta(&mut rec, &historia(), "0500112345", 1, &cie11);
        let json = serde_json::to_string(&c).unwrap();
        let claves: Vec<&str> = json
            .split(",\"")
            .map(|t| t.split('"').next().unwrap_or(""))
            .collect();
        // La primera va sin la coma de delante.
        let mut orden: Vec<&str> = vec!["codPrestador"];
        orden.extend(claves.into_iter().skip(1));

        assert_eq!(
            orden,
            [
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
            ]
        );
    }

    /// Una consulta completa no anota NADA: si esto fallara, las pruebas de
    /// ausencia estarían pasando por la razón equivocada.
    #[test]
    fn una_consulta_completa_no_deja_faltantes() {
        let mut rec = Recolector::estricto();
        let c = consulta(&mut rec, &historia(), "0500112345", 1, &cie11);
        assert!(rec.faltantes().is_empty(), "{:?}", rec.faltantes());
        assert_eq!(c.cod_prestador, "0500112345");
        assert_eq!(c.fecha_inicio_atencion, "2026-08-03 09:15", "no se quitaron los segundos");
        assert_eq!(c.vr_servicio, Some(45000));
        assert_eq!(c.consecutivo, 1);
    }

    /// El diagnóstico relacionado ausente va `null`, NUNCA `""`. El MUV rechaza
    /// la cadena vacía porque la contrasta contra la tabla de la CIE.
    #[test]
    fn el_diagnostico_relacionado_ausente_va_nulo_y_no_cadena_vacia() {
        let mut h = historia();
        h.diagnostico_relacionado1 = Some("   ".into());
        let mut rec = Recolector::estricto();
        let c = consulta(&mut rec, &h, "0500112345", 1, &cie11);
        assert_eq!(c.cod_diagnostico_relacionado1, None);
        let json = serde_json::to_string(&c).unwrap();
        assert!(
            json.contains(r#""codDiagnosticoRelacionado1":null"#),
            "salió cadena vacía en vez de null: {json}"
        );
    }

    #[test]
    fn el_cie11_se_deriva_del_diagnostico_y_lo_no_derivable_va_vacio() {
        let mut rec = Recolector::estricto();
        let c = consulta(&mut rec, &historia(), "0500112345", 1, &cie11);
        assert_eq!(c.cod_diagnostico_principal_cie11, "CA22");
        assert!(!c.nom_cod_diagnostico_principal_cie11.is_empty());
        // Sin relacionados, los seis campos de los relacionados van vacíos.
        assert_eq!(c.cod_diagnostico_relacionado1_cie11, "");
        assert_eq!(c.nom_cod_diagnostico_relacionado3_cie11, "");
    }

    /// El CIE-11 sale del CIE-10 **de origen**, no del que se acaba emitiendo:
    /// si el diagnóstico falta y entra el supuesto, derivar de él daría un
    /// CIE-11 para un diagnóstico que nadie hizo.
    #[test]
    fn sin_diagnostico_no_hay_cie11_aunque_se_permitan_supuestos() {
        let mut h = historia();
        h.diagnostico_principal = None;
        let mut rec = Recolector::permitiendo_supuestos();
        let c = consulta(&mut rec, &h, "0500112345", 1, &cie11);
        assert_eq!(c.cod_diagnostico_principal, None, "el diagnóstico NO tiene supuesto");
        assert_eq!(c.cod_diagnostico_principal_cie11, "");
        assert_eq!(rec.faltantes().len(), 1);
    }

    /// La historia vacía anota todo lo obligatorio y no emite nada inventado.
    #[test]
    fn una_historia_sin_datos_lo_anota_todo_y_no_inventa() {
        let mut rec = Recolector::estricto();
        let vacia = DatosConsulta { id: 7, ..Default::default() };
        let c = consulta(&mut rec, &vacia, "0500112345", 1, &cie11);
        assert_eq!(c.cod_consulta, None, "entró el CUPS 890201 que nadie afirmó");
        assert_eq!(c.vr_servicio, None, "el valor del servicio salió en 0 sin serlo");
        assert_eq!(c.tipo_documento_identificacion, None);
        // 10 obligatorios del AC + los 2 del profesional.
        assert_eq!(rec.faltantes().len(), 12, "{:#?}", rec.faltantes());
        assert!(rec.faltantes().iter().all(|f| f.archivo == "AC"));
        // Y el cierre NIEGA la generación, con la lista completa.
        assert!(rec.cerrar().is_err());
    }

    /// Con supuestos sale entera, y queda escrito qué se supuso: es la
    /// diferencia entre auditar y documentar la mentira.
    #[test]
    fn con_supuestos_sale_entera_y_queda_la_lista_de_lo_supuesto() {
        let mut rec = Recolector::permitiendo_supuestos();
        let vacia = DatosConsulta { id: 7, ..Default::default() };
        let c = consulta(&mut rec, &vacia, "0500112345", 1, &cie11);
        assert_eq!(c.cod_consulta.as_deref(), Some("890201"));
        assert_eq!(c.cod_servicio, Some(328));
        assert_eq!(c.vr_servicio, Some(0));
        // El profesional NO tiene supuesto ni con el permiso puesto.
        assert_eq!(c.tipo_documento_identificacion, None);
        let faltantes = rec.cerrar().expect("con supuestos, cerrar no falla");
        assert_eq!(faltantes.len(), 12);
    }
}

// ─── AC · Consultas ─────────────────────────────────────────────────────────

/// Una consulta del archivo AC, en el orden de campos de la Resolución 948.
///
/// El orden es el del diccionario `consulta` de `rips_completo.py:488`. Ver la
/// nota del módulo sobre por qué no se reordena.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Consulta {
    pub cod_prestador: String,
    pub fecha_inicio_atencion: String,
    pub num_autorizacion: String,
    pub cod_consulta: Option<String>,
    pub modalidad_grupo_servicio_tec_sal: Option<String>,
    pub grupo_servicios: Option<String>,
    pub cod_servicio: Option<i64>,
    pub finalidad_tecnologia_salud: Option<String>,
    pub causa_motivo_atencion: Option<String>,
    pub cod_diagnostico_principal: Option<String>,
    /// Los relacionados SÍ pueden no estar: son opcionales. Pero «no hay» se
    /// escribe `null`, **no `""`**: el MUV rechaza la cadena vacía —«El campo
    /// código diagnóstico relacionado [] no es válido»— porque la contrasta
    /// contra la tabla de la CIE, y `""` no está en ninguna tabla.
    pub cod_diagnostico_relacionado1: Option<String>,
    pub cod_diagnostico_relacionado2: Option<String>,
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
    pub tipo_diagnostico_principal: Option<String>,
    pub tipo_documento_identificacion: Option<String>,
    pub num_documento_identificacion: Option<String>,
    pub vr_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    /// El pago moderador SÍ admite cero de verdad: la mayoría de las atenciones
    /// no lo llevan. Cero aquí es un dato, no un hueco, y por eso NO pasa por el
    /// recolector.
    pub valor_pago_moderador: i64,
    #[serde(rename = "numFEVPagoModerador")]
    pub num_fev_pago_moderador: String,
    pub consecutivo: i64,
}

/// Una historia clínica tal como se lee de la base, para armar su consulta.
#[derive(Debug, Clone, Default)]
pub struct DatosConsulta {
    pub id: i64,
    pub paciente_id: i64,
    pub fecha: String,
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
    pub valor_pago_moderador: i64,
    pub num_fev_pago_moderador: Option<String>,
    pub genera_incapacidad: bool,
    pub medico: DatosProfesional,
}

/// «Vacío» en el sentido del RIPS: `None` o cadena en blanco. Sirve para los
/// campos OPCIONALES, que se escriben `null` y nunca `""`.
fn o_nulo(v: Option<&str>) -> Option<String> {
    match v {
        Some(s) if !s.trim().is_empty() => Some(s.to_string()),
        _ => None,
    }
}

/// Arma la consulta de una historia clínica.
///
/// `derivar_cie11` se recibe como parámetro en vez de llamarse aquí para que el
/// dominio no dependa del maestro empotrado: así estas reglas se prueban con un
/// mapeo de tres códigos en vez de cargar 522 KB, y la capa de datos pasa el
/// real.
///
/// **El CIE-11 se deriva del CIE-10 DE ORIGEN, no del que se acaba emitiendo.**
/// Si el diagnóstico falta y entra un supuesto, derivar de él daría un CIE-11
/// para un diagnóstico que nadie hizo.
pub fn consulta(
    rec: &mut Recolector,
    h: &DatosConsulta,
    cod_habilitacion: &str,
    consecutivo: i64,
    derivar_cie11: &dyn Fn(Option<&str>) -> (String, String),
) -> Consulta {
    let en = |campo, etiqueta| Donde::ac(h.id, campo, etiqueta);
    let (dx_p_c11, dx_p_n11) = derivar_cie11(h.diagnostico_principal.as_deref());
    let (dx_1_c11, dx_1_n11) = derivar_cie11(h.diagnostico_relacionado1.as_deref());
    let (dx_2_c11, dx_2_n11) = derivar_cie11(h.diagnostico_relacionado2.as_deref());
    let (dx_3_c11, dx_3_n11) = derivar_cie11(h.diagnostico_relacionado3.as_deref());

    // El ORDEN de estas tres llamadas es el de `rips_completo.py`, y no es
    // indiferente: define el orden de la lista de faltantes que ve quien cierra
    // el mes. Van aquí, en `let`, porque los campos que las usan están al final
    // de la estructura y Rust evalúa los campos en orden de escritura — dejarlas
    // ahí abajo las movería DESPUÉS del diagnóstico, que no es donde van.
    let prof = profesional(
        rec,
        &h.medico,
        &en("medico.tipo_documento", "tipo de documento del profesional que atendió"),
        &en("medico.numero_documento", "número de documento del profesional que atendió"),
    );
    let vr_servicio = entero(
        rec,
        h.valor_servicio,
        &en("valor_servicio", "valor del servicio").supuesto("0"),
    );
    let concepto_recaudo = texto(
        rec,
        h.concepto_recaudo.as_deref(),
        &en("concepto_recaudo", "concepto de recaudo").supuesto("05"),
    );

    Consulta {
        cod_prestador: cod_habilitacion.to_string(),
        fecha_inicio_atencion: fecha_hora(&h.fecha),
        num_autorizacion: h.num_autorizacion.clone().unwrap_or_default(),
        cod_consulta: texto(
            rec,
            h.cod_consulta_cups.as_deref(),
            &en("cod_consulta_cups", "CUPS de la consulta").supuesto("890201"),
        ),
        modalidad_grupo_servicio_tec_sal: texto(
            rec,
            h.modalidad_grupo_servicio.as_deref(),
            &en("modalidad_grupo_servicio", "modalidad del servicio").supuesto("01"),
        ),
        grupo_servicios: texto(
            rec,
            h.grupo_servicios.as_deref(),
            &en("grupo_servicios", "grupo de servicios").supuesto("01"),
        ),
        cod_servicio: entero(
            rec,
            h.cod_servicio,
            &en("cod_servicio", "código de servicio").supuesto("328"),
        ),
        finalidad_tecnologia_salud: texto(
            rec,
            h.finalidad.as_deref(),
            // «10» no está en la tabla de referencia y el MUV lo rechaza; el
            // ejemplo de la propia norma usa 11 en consultas.
            &en("finalidad", "finalidad de la consulta").supuesto("11"),
        ),
        causa_motivo_atencion: texto(
            rec,
            h.causa_externa.as_deref(),
            &en("causa_externa", "causa o motivo de la atención").supuesto("13"),
        ),
        cod_diagnostico_principal: texto(
            rec,
            h.diagnostico_principal.as_deref(),
            &en("diagnostico_principal", "diagnóstico principal"),
        ),
        cod_diagnostico_relacionado1: o_nulo(h.diagnostico_relacionado1.as_deref()),
        cod_diagnostico_relacionado2: o_nulo(h.diagnostico_relacionado2.as_deref()),
        cod_diagnostico_relacionado3: o_nulo(h.diagnostico_relacionado3.as_deref()),
        cod_diagnostico_principal_cie11: dx_p_c11,
        nom_cod_diagnostico_principal_cie11: dx_p_n11,
        cod_diagnostico_relacionado1_cie11: dx_1_c11,
        nom_cod_diagnostico_relacionado1_cie11: dx_1_n11,
        cod_diagnostico_relacionado2_cie11: dx_2_c11,
        nom_cod_diagnostico_relacionado2_cie11: dx_2_n11,
        cod_diagnostico_relacionado3_cie11: dx_3_c11,
        nom_cod_diagnostico_relacionado3_cie11: dx_3_n11,
        tipo_diagnostico_principal: texto(
            rec,
            h.tipo_diagnostico.as_deref(),
            &en("tipo_diagnostico", "tipo de diagnóstico principal").supuesto("01"),
        ),
        tipo_documento_identificacion: prof.tipo_documento_identificacion,
        num_documento_identificacion: prof.num_documento_identificacion,
        vr_servicio,
        concepto_recaudo,
        valor_pago_moderador: h.valor_pago_moderador,
        num_fev_pago_moderador: h.num_fev_pago_moderador.clone().unwrap_or_default(),
        consecutivo,
    }
}
