//! Lo que el binario emite tiene que llamarse como la Resolución 948 dice.
//!
//! El lado Python ya tenía este banco (`backend/tests/test_campos_fev948.py`);
//! el lado Rust no, y esa asimetría no era teórica: los documentos de Rust
//! comprobaban sus nombres contra **listas escritas a mano dentro de la propia
//! prueba**, así que un campo que faltara en el código faltaba también en la
//! lista y las dos coincidían en callar. Es la forma disfrazada del banco que
//! solo mira lo que sobra.
//!
//! Aquí la aguja no se escribe: sale de `campos_fev948.json`, el artefacto que
//! `herramientas/extraer_campos_fev948.py` extrae del Documento Técnico 1. Se
//! lee con `include_str!` para que el día que alguien regenere el JSON, esta
//! prueba mida la norma nueva sin que nadie se acuerde de tocarla.
//!
//! ## Las dos mitades, y por qué hacen falta las dos
//!
//! Un campo que **sobra** hace rechazar el archivo entero, y con él la factura:
//! el MUV contesta «Propiedades no permitidas en la estructura». Un campo que
//! **falta** puede ser opcional —y por eso el banco del Python llevaba meses
//! mirando solo la primera mitad—. Pero «puede ser opcional» no es «lo es»: con
//! esa mitad sin vigilar, urgencias y hospitalización emitían el diagnóstico
//! relacionado de egreso 1 y callaban el 2 y el 3, y el AM callaba nueve campos
//! —su par CIE-11 entre ellos—. Doce más nueve campos que la 948 nombra, con
//! todo el banco en verde.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use medico_dominio::rips::{
    ac::{documento_ac, FuenteConsulta},
    ah::{documento_ah, FuenteHospitalizacion},
    am::{documento_am, FuenteMedicamento},
    an::{anexar_recien_nacidos, FuenteRecienNacido},
    aos::{documento_aos, FuenteOtroServicio},
    ap::{documento_ap, FuenteProcedimiento},
    au::{documento_au, FuenteUrgencia},
    documentos::{usuario_base, FuenteInstitucion, FuentePaciente, FuenteProfesional},
    recolector::Recolector,
};

/// El artefacto, no una copia. Ver la cabecera del módulo.
const NORMA: &str = include_str!("../../../backend/datos/normativa/campos_fev948.json");

/// Campos que la 948 nombra y medico todavía NO emite, con el motivo de cada uno.
///
/// Es la pareja de la lista de tolerados: aquélla declara lo que sobra con
/// permiso, ésta lo que falta con permiso. Vacío no es la meta; la meta es que
/// nada esté aquí sin una razón que se pueda leer.
///
/// `codigoVIDA` es «el id único de la atención en el sistema IHCE», y la propia
/// norma dice que «será obligatorio una vez entre en operación»: hoy su tamaño
/// es `0, 1-256`, o sea opcional. Emitirlo vacío sería inventar un identificador
/// que nadie asignó. Sale de aquí cuando el transporte IHCE/RDA esté enchufado
/// —ficha #603—, y ése es el amarre entre las dos: el RIPS es donde el id del
/// IHCE se ve.
const FALTAN_CON_MOTIVO: &[&str] = &["codigoVIDA"];

fn norma_de(arreglo: &str) -> BTreeSet<String> {
    let v: serde_json::Value = serde_json::from_str(NORMA).expect("el JSON de la norma no es JSON");
    v["arreglos"][arreglo]
        .as_array()
        .unwrap_or_else(|| panic!("la norma no tiene el arreglo «{arreglo}»"))
        .iter()
        .map(|c| c.as_str().expect("campo que no es cadena").to_string())
        .collect()
}

/// Las claves de primer nivel del objeto de servicio, tal y como salen al JSON.
fn claves(valor: &serde_json::Value) -> BTreeSet<String> {
    valor
        .as_object()
        .expect("el servicio no serializó como objeto")
        .keys()
        .cloned()
        .collect()
}

fn comparar(arreglo: &str, emitidos: BTreeSet<String>) {
    let norma = norma_de(arreglo);
    let excepciones: BTreeSet<String> = FALTAN_CON_MOTIVO.iter().map(|s| s.to_string()).collect();

    let faltan: Vec<_> = norma
        .difference(&emitidos)
        .filter(|c| !excepciones.contains(*c))
        .cloned()
        .collect();
    assert!(
        faltan.is_empty(),
        "{arreglo} no emite campos que la Resolución 948 sí nombra: {faltan:?}. Si la \
         ausencia es deliberada, va a FALTAN_CON_MOTIVO con su razón; si no, es un campo \
         que se perdió."
    );

    let sobran: Vec<_> = emitidos.difference(&norma).cloned().collect();
    assert!(
        sobran.is_empty(),
        "{arreglo} emite campos que la Resolución 948 no nombra: {sobran:?}. El MUV \
         rechaza el archivo COMPLETO con «Propiedades no permitidas en la estructura»."
    );
}

// ————————————————————————— las fuentes de prueba —————————————————————————

fn institucion() -> FuenteInstitucion {
    FuenteInstitucion {
        nit: "900123456".into(),
        codigo_habilitacion: "050010123401".into(),
    }
}

fn profesional() -> FuenteProfesional {
    FuenteProfesional {
        tipo_documento: Some("CC".into()),
        numero_documento: Some("98765432".into()),
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

fn cuando() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 3, 16)
        .unwrap()
        .and_hms_opt(9, 30, 0)
        .unwrap()
}

// ——————————————————————————— las cinco pruebas ———————————————————————————

#[test]
fn el_ac_emite_los_campos_de_la_948() {
    let mut rec = Recolector::permitiendo_supuestos();
    let doc = documento_ac(
        &institucion(),
        &[FuenteConsulta {
            procedencias: None,
            id: 1,
            fecha: cuando(),
            num_autorizacion: None,
            cod_consulta_cups: Some("890201".into()),
            modalidad_grupo_servicio: Some("01".into()),
            grupo_servicios: Some("01".into()),
            cod_servicio: Some(334),
            finalidad: Some("10".into()),
            causa_externa: Some("38".into()),
            diagnostico_principal: Some("A000".into()),
            diagnostico_relacionado1: None,
            diagnostico_relacionado2: None,
            diagnostico_relacionado3: None,
            tipo_diagnostico: Some("01".into()),
            valor_servicio: Some(50000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: Some(0),
            num_fev_pago_moderador: None,
            genera_incapacidad: false,
            paciente: paciente(),
            profesional: profesional(),
        }],
        &mut rec,
    );
    let v = serde_json::to_value(&doc.usuarios[0].servicios.consultas[0]).unwrap();
    comparar("consultas", claves(&v));
}

#[test]
fn el_ap_emite_los_campos_de_la_948() {
    let mut rec = Recolector::permitiendo_supuestos();
    let doc = documento_ap(
        &institucion(),
        &[FuenteProcedimiento {
            procedencias: None,
            id: 1,
            fecha: cuando(),
            cod_procedimiento_cups: Some("871121".into()),
            tipo: "PROCEDIMIENTO".into(),
            via_ingreso_servicio: Some("02".into()),
            grupo_servicios: Some("01".into()),
            cod_servicio: Some(334),
            finalidad: Some("10".into()),
            diagnostico_principal: Some("A000".into()),
            diagnostico_relacionado: None,
            valor_servicio: Some(80000),
            concepto_recaudo: Some("05".into()),
            valor_pago_moderador: Some(0),
            num_fev_pago_moderador: None,
            paciente: paciente(),
            profesional: profesional(),
        }],
        &mut rec,
    );
    let v = serde_json::to_value(&doc.usuarios[0].servicios.procedimientos[0]).unwrap();
    comparar("procedimientos", claves(&v));
}

/// Dos casos, porque el AM tiene dos ramas y la 948 les da valores distintos:
/// la preparación MAGISTRAL lleva su concentración, unidad y forma, y los demás
/// tipos llevan `0`, `0` y `null` por mandato de la norma. Con un solo caso, una
/// rama podría dejar de emitir los tres campos sin que nadie se enterara.
#[test]
fn el_am_emite_los_campos_de_la_948_sea_o_no_magistral() {
    for tipo in ["01", "03"] {
        let mut rec = Recolector::permitiendo_supuestos();
        let doc = documento_am(
            &institucion(),
            &[FuenteMedicamento {
            procedencias: None,
                id: 1,
                fecha: NaiveDate::from_ymd_opt(2026, 3, 16).unwrap(),
                codigo_cum: Some("19900001-1".into()),
                tipo_medicamento: Some(tipo.into()),
                medicamento: "ACETAMINOFEN 500 MG".into(),
                diagnostico_principal: Some("A000".into()),
                unidad: Some("tableta".into()),
                cantidad: Some(10.0),
                dias_tratamiento: Some(5),
                valor_unitario: Some(300),
                valor_servicio: Some(3000),
                concepto_recaudo: Some("05".into()),
                valor_pago_moderador: Some(0),
                num_fev_pago_moderador: None,
                paciente: paciente(),
                profesional: profesional(),
            }],
            &mut rec,
        );
        let v = serde_json::to_value(&doc.usuarios[0].servicios.medicamentos[0]).unwrap();
        comparar("medicamentos", claves(&v));
    }
}

#[test]
fn el_au_emite_los_campos_de_la_948() {
    let mut rec = Recolector::permitiendo_supuestos();
    let doc = documento_au(
        &institucion(),
        &[FuenteUrgencia {
            procedencias: None,
            id: 1,
            fecha_ingreso: cuando(),
            fecha_egreso: cuando() + chrono::Duration::hours(4),
            causa_motivo: Some("38".into()),
            diagnostico_ingreso: Some("A000".into()),
            diagnostico_egreso: Some("A000".into()),
            destino_salida: Some("DOMICILIO".into()),
            diagnostico_causa_muerte: None,
            paciente: paciente(),
        }],
        &mut rec,
    );
    let v = serde_json::to_value(&doc.usuarios[0].servicios.urgencias[0]).unwrap();
    comparar("urgencias", claves(&v));
}

#[test]
fn el_ah_emite_los_campos_de_la_948() {
    let mut rec = Recolector::permitiendo_supuestos();
    let doc = documento_ah(
        &institucion(),
        &[FuenteHospitalizacion {
            procedencias: None,
            id: 1,
            fecha_ingreso: cuando(),
            fecha_egreso: cuando() + chrono::Duration::days(2),
            via_ingreso: Some("URGENCIAS".into()),
            estado: Some("ALTA_MEDICA".into()),
            causa_motivo: Some("38".into()),
            diagnostico_ingreso: Some("A000".into()),
            diagnostico_egreso: Some("A000".into()),
            diagnostico_causa_muerte: None,
            paciente: paciente(),
        }],
        &mut rec,
    );
    let v = serde_json::to_value(&doc.usuarios[0].servicios.hospitalizacion[0]).unwrap();
    comparar("hospitalizacion", claves(&v));
}

/// El AN es el SEXTO arreglo, y el primero que no viene de un `documento_*`:
/// se cuelga del usuario ya armado de la MADRE. Por eso su caso monta el
/// usuario a mano en vez de llamar a un generador.
#[test]
fn el_an_emite_los_campos_de_la_948() {
    let mut rec = Recolector::permitiendo_supuestos();
    let madre = paciente();
    let mut usuarios = vec![usuario_base(&madre, 1, &mut rec)];
    let huerfanos = anexar_recien_nacidos(
        &institucion(),
        &mut usuarios,
        &[FuenteRecienNacido {
            id: 1,
            madre_paciente_id: madre.id,
            madre_tipo_documento: madre.tipo_documento.clone(),
            madre_numero_documento: madre.numero_documento.clone(),
            tipo_documento: Some("CN".into()),
            numero_documento: Some("9900123456".into()),
            fecha_nacimiento: cuando(),
            edad_gestacional: 39,
            num_consultas_prenatal: 7,
            sexo_biologico: "F".into(),
            peso_gramos: 3250,
            diagnostico_principal: "Z380".into(),
            condicion_destino: Some("01".into()),
            diagnostico_causa_muerte: None,
            fecha_egreso: Some(cuando() + chrono::Duration::days(2)),
            codigo_vida: Some("VIDA-2026-0001".into()),
        }],
        &mut rec,
    );
    assert!(
        huerfanos.is_empty(),
        "la madre estaba en el documento: si sale huérfano, el caso no midió nada"
    );
    let v = serde_json::to_value(&usuarios[0].servicios.recien_nacidos[0]).unwrap();
    comparar("recienNacidos", claves(&v));
}

#[test]
fn el_at_emite_los_campos_de_la_948() {
    let mut rec = Recolector::permitiendo_supuestos();
    let doc = documento_aos(
        &institucion(),
        &[FuenteOtroServicio {
            id: 1,
            fecha: cuando().date(),
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
        }],
        &mut rec,
    );
    let v = serde_json::to_value(&doc.usuarios[0].servicios.otros_servicios[0]).unwrap();
    comparar("otrosServicios", claves(&v));
}

/// El positivo de control del banco entero: si la ruta al artefacto se rompiera
/// —un `include_str!` que apunte a otro sitio, un JSON vacío— las cinco pruebas
/// de arriba compararían contra el conjunto vacío y saldrían VERDES sin haber
/// medido nada. Aquí se exige que la norma cargada tenga los siete arreglos y
/// las cuentas del documento.
#[test]
fn la_norma_cargada_es_la_del_documento_tecnico_1() {
    let v: serde_json::Value = serde_json::from_str(NORMA).unwrap();
    let arreglos = v["arreglos"].as_object().expect("sin arreglos");
    assert_eq!(arreglos.len(), 7, "la 948 tiene siete arreglos");
    for (arreglo, cuantos) in [
        ("consultas", 30),
        ("procedimientos", 27),
        ("urgencias", 25),
        ("hospitalizacion", 30),
        ("recienNacidos", 18),
        ("medicamentos", 28),
        ("otrosServicios", 18),
    ] {
        assert_eq!(
            norma_de(arreglo).len(),
            cuantos,
            "{arreglo} cambió de tamaño: si alguien regeneró el JSON, se entera aquí"
        );
    }
}
