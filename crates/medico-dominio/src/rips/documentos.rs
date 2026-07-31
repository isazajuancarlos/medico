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
}
