// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **El FEV-RIPS de Rust tiene que ser BYTE A BYTE el del Python** (#250).
//!
//! Es la condición que el ADR pone para apagar el Python, y por eso esta prueba
//! no compara «lo mismo más o menos»: compara las cadenas.
//!
//! # Qué compara exactamente, y por qué así
//!
//! Lo portado es la **fusión y el sobre**: tomar los cinco documentos y
//! convertirlos en uno solo. Así que la comparación se hace sobre esa costura:
//! los cinco documentos los produce **Rust**, se serializan, y se le pasan al
//! `fev_rips.py` del backend para que él los funda con SU código. Si los dos
//! producen la misma cadena, el port es fiel.
//!
//! Comparar de punta a punta —cada uno leyendo la base con su capa de datos—
//! mediría otra cosa: si `medico-datos` y SQLModel traen las mismas filas. Eso
//! es una propiedad aparte y ya tiene sus pruebas; mezclarlas aquí dejaría un
//! rojo que no dice cuál de las dos falló.
//!
//! # Si falta el Python, esto FALLA; no se salta
//!
//! Una comprobación que se desactiva sola cuando falta su insumo es la que nunca
//! vuelve a decir nada (directivas 20 y 30). Necesita `python3` y el árbol
//! `backend/` en su sitio.

use std::process::Command;

/// El guion que funde los documentos CON EL CÓDIGO DEL PYTHON. `@RAIZ@` y
/// `@SOBRE@` se sustituyen antes de ejecutarlo.
const GUION: &str = r#"
import json, sys
sys.path.insert(0, "@RAIZ@")
import fev_rips

docs = json.loads(sys.stdin.read())
usuarios = fev_rips.consolidar(docs)
obligado = next((d["numDocumentoIdObligado"] for d in docs if d.get("numDocumentoIdObligado")), "")
if "@SOBRE@" == "rs":
    paquete = fev_rips.sobre_rips_sin_factura(obligado, usuarios, "NOTA-001")
else:
    paquete = fev_rips.sobre_fev(obligado, usuarios, "FAC-2026-00001", "UEsDBBQA")
sys.stdout.write(json.dumps(paquete, ensure_ascii=False, separators=(",", ":")))
"#;

use chrono::NaiveDate;
use medico_dominio::rips::{
    documento_ac, documento_ah, documento_am, fev, FuenteConsulta, FuenteHospitalizacion,
    FuenteMedicamento,
};
use medico_dominio::rips::documentos::{FuenteInstitucion, FuentePaciente, FuenteProfesional};
use medico_dominio::rips::recolector::Recolector;

fn institucion() -> FuenteInstitucion {
    FuenteInstitucion {
        nit: "900123456".into(),
        codigo_habilitacion: "050010123401".into(),
    }
}

fn profesional() -> FuenteProfesional {
    FuenteProfesional {
        tipo_documento: Some("CC".into()),
        numero_documento: Some("79123456".into()),
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

fn consulta(id: i64, p: FuentePaciente, incapacidad: bool) -> FuenteConsulta {
    FuenteConsulta {
            procedencias: None,
        id,
        fecha: NaiveDate::from_ymd_opt(2026, 3, 15)
            .unwrap()
            .and_hms_opt(9, 30, 15)
            .unwrap(),
        // VACÍO a propósito: es el campo que FEV-RIPS tiene que emitir `null` y
        // los cinco documentos `""`. Es la mitad del port que más fácil se
        // escapa, porque el JSON sigue siendo válido con la cadena vacía.
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
        valor_servicio: Some(45000),
        concepto_recaudo: Some("05".into()),
        valor_pago_moderador: Some(0),
        num_fev_pago_moderador: None,
        genera_incapacidad: incapacidad,
        paciente: p,
        profesional: profesional(),
    }
}

fn medicamento(id: i64, p: FuentePaciente) -> FuenteMedicamento {
    FuenteMedicamento {
            procedencias: None,
        id,
        fecha: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
        codigo_cum: Some("19900001-1".into()),
        tipo_medicamento: Some("01".into()),
        medicamento: "Acetaminofén 500 mg".into(),
        diagnostico_principal: Some("A000".into()),
        unidad: Some("tableta".into()),
        cantidad: Some(20.0),
        dias_tratamiento: Some(5),
        valor_unitario: Some(200),
        valor_servicio: Some(4000),
        concepto_recaudo: Some("05".into()),
        valor_pago_moderador: Some(0),
        num_fev_pago_moderador: None,
        paciente: p,
        profesional: profesional(),
    }
}

fn hospitalizacion(id: i64, p: FuentePaciente) -> FuenteHospitalizacion {
    FuenteHospitalizacion {
            procedencias: None,
        id,
        fecha_ingreso: NaiveDate::from_ymd_opt(2026, 3, 16)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap(),
        fecha_egreso: NaiveDate::from_ymd_opt(2026, 3, 18)
            .unwrap()
            .and_hms_opt(11, 0, 0)
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

/// Los tres documentos con los que se compara.
///
/// **Dos pacientes y uno de ellos en los tres**: es lo único que ejerce la
/// fusión de verdad. Con un paciente por documento, un «consolidar» que no
/// fusionara nada daría el mismo resultado.
fn documentos() -> Vec<medico_dominio::rips::documentos::Documento> {
    let uno = paciente(7, "111");
    let dos = paciente(8, "222");
    // **El tercero aparece SOLO en el AM**, y es el que hace que la renumeración
    // del consecutivo se note: dentro del AM es el número 2, y en el documento
    // fundido tiene que ser el 3. Sin él, los consecutivos de cada documento ya
    // coinciden con los globales y un `consolidar` que no renumerara pasaría
    // esta prueba — lo destapó un mutante que sobrevivió.
    let tres = paciente(9, "333");
    let mut rec = Recolector::permitiendo_supuestos();
    vec![
        documento_ac(
            &institucion(),
            &[
                consulta(1, uno.clone(), false),
                consulta(2, dos, true),
                // Una SEGUNDA consulta del primero: obliga a renumerar el
                // consecutivo dentro de su tipo.
                consulta(3, uno.clone(), true),
            ],
            &mut rec,
        ),
        documento_am(
            &institucion(),
            &[medicamento(1, uno.clone()), medicamento(2, tres)],
            &mut rec,
        ),
        documento_ah(
            &institucion(),
            &[hospitalizacion(1, uno)],
            &mut rec,
        ),
    ]
}

/// El intérprete que puede importar `fev_rips`.
///
/// **El del entorno virtual del backend primero**: `fev_rips` importa
/// `rips_completo`, que arrastra SQLModel, así que el `python3` del sistema no
/// lo puede cargar. Se prefiere el venv y se cae al del sistema —que es lo que
/// hay en el runner, donde las dependencias se instalan en el intérprete
/// principal—; si ninguno de los dos sirve, la prueba FALLA con el error de
/// Python entero, en vez de saltarse.
fn interprete() -> String {
    let venv = concat!(env!("CARGO_MANIFEST_DIR"), "/../../backend/.venv/bin/python");
    if std::path::Path::new(venv).exists() {
        return venv.to_string();
    }
    "python3".to_string()
}

/// Le da los MISMOS documentos al `fev_rips.py` del backend y devuelve su JSON.
fn del_python(documentos_json: &str, sobre: &str) -> String {
    let raiz = concat!(env!("CARGO_MANIFEST_DIR"), "/../../backend");
    // El guion se arma con `replace` y no con `format!`: el cuerpo lleva llaves
    // de Python y `format!` las leería como suyas.
    let guion = GUION
        .replace("@RAIZ@", raiz)
        .replace("@SOBRE@", sobre);
    let mut hijo = Command::new(interprete())
        .arg("-c")
        .arg(&guion)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect(
            "no se pudo ejecutar python3: esta prueba compara contra el Python de verdad y sin \
             él no comprueba nada, así que falla en vez de saltarse",
        );
    {
        use std::io::Write;
        hijo.stdin
            .as_mut()
            .unwrap()
            .write_all(documentos_json.as_bytes())
            .unwrap();
    }
    let salida = hijo.wait_with_output().unwrap();
    assert!(
        salida.status.success(),
        "el Python falló:\n{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    String::from_utf8(salida.stdout).unwrap()
}

/// Serializa lo que Rust produce con el MISMO formato compacto que el guion del
/// Python, para que la comparación sea de cadenas y no de estructuras.
fn compacto<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap()
}

#[test]
fn el_paquete_sin_factura_es_el_mismo_en_los_dos() {
    let docs = documentos();
    let en_python = del_python(&compacto(&docs), "rs");
    let en_rust = compacto(&fev::sobre_rips_sin_factura(&docs, "NOTA-001").unwrap());
    assert_eq!(en_rust, en_python, "el FEV-RIPS de Rust no es el del Python");
}

#[test]
fn el_paquete_con_factura_electronica_es_el_mismo_en_los_dos() {
    let docs = documentos();
    let en_python = del_python(&compacto(&docs), "fev");
    let en_rust =
        compacto(&fev::sobre_fev(&docs, "FAC-2026-00001", "UEsDBBQA").unwrap());
    assert_eq!(en_rust, en_python, "el sobre de la FEV no es el del Python");
}

/// **El control de que la comparación de arriba mide algo.**
///
/// Si el guion del Python devolviera lo mismo para cualquier entrada —o si los
/// documentos de prueba no ejercieran la fusión— las dos pruebas anteriores
/// pasarían con un `consolidar` que no consolidara. Aquí se comprueba que el
/// resultado SÍ depende de la entrada, y en la dirección que importa: el
/// paciente que aparece en los tres documentos sale **una vez** con sus tres
/// clases de servicio juntas.
#[test]
fn la_fusion_junta_los_servicios_del_mismo_paciente() {
    let docs = documentos();
    let p = fev::sobre_rips_sin_factura(&docs, "NOTA-001").unwrap();

    assert_eq!(p.rips.usuarios.len(), 3, "los tres documentos traen tres personas");
    let uno = &p.rips.usuarios[0];
    assert_eq!(uno.num_documento, "111");
    assert_eq!(uno.servicios.consultas.len(), 2);
    assert_eq!(uno.servicios.medicamentos.len(), 1);
    assert_eq!(uno.servicios.hospitalizacion.len(), 1);

    // Los consecutivos se renumeran: el global por usuario y el de cada tipo
    // dentro de su tipo.
    assert_eq!(uno.consecutivo, 1);
    assert_eq!(p.rips.usuarios[1].consecutivo, 2);
    // El tercero: dentro del AM es el 2 y aquí tiene que ser el 3.
    assert_eq!(p.rips.usuarios[2].num_documento, "333");
    assert_eq!(p.rips.usuarios[2].consecutivo, 3);
    assert_eq!(docs[1].usuarios[1].consecutivo, 2, "control: en su documento era el 2");
    assert_eq!(uno.servicios.consultas[0].consecutivo, 1);
    assert_eq!(uno.servicios.consultas[1].consecutivo, 2);
    assert_eq!(uno.servicios.medicamentos[0].consecutivo, 1);

    // Y en los CINCO documentos sueltos ese mismo paciente sale tres veces.
    let sueltos: usize = docs
        .iter()
        .map(|d| d.usuarios.iter().filter(|u| u.num_documento == "111").count())
        .sum();
    assert_eq!(sueltos, 3, "control: sin fusión el paciente aparece tres veces");
}

/// **`numAutorizacion` y `numFEVPagoModerador` van `null`, no `""`.**
///
/// Enviar cadena vacía donde el esquema declara nulable es causal de rechazo, y
/// el JSON sigue siendo válido: no hay error que avise. La pareja es que en los
/// documentos SUELTOS siguen saliendo como `""` — ahí el formato es el otro, y
/// cambiarlo rompería la paridad de los cinco archivos.
#[test]
fn los_campos_nulables_salen_null_en_el_fev_y_vacios_en_los_cinco() {
    let docs = documentos();
    let sueltos = compacto(&docs[0].usuarios[0].servicios.consultas[0]);
    assert!(
        sueltos.contains(r#""numAutorizacion":"""#),
        "el documento suelto dejó de emitir la cadena vacía: {sueltos}"
    );

    let p = fev::sobre_rips_sin_factura(&docs, "NOTA-001").unwrap();
    let fusionado = compacto(&p.rips.usuarios[0].servicios.consultas[0]);
    assert!(
        fusionado.contains(r#""numAutorizacion":null"#),
        "el FEV-RIPS emitió cadena vacía donde el esquema pide null: {fusionado}"
    );
    assert!(
        fusionado.contains(r#""numFEVPagoModerador":null"#),
        "{fusionado}"
    );
}

/// El archivo que se sube a la aplicación de escritorio: sangría de UN espacio,
/// sin escapar los no-ASCII, y sin el envoltorio de la API.
#[test]
fn el_archivo_tiene_la_forma_del_conversor_oficial() {
    let docs = documentos();
    let p = fev::sobre_rips_sin_factura(&docs, "NOTA-001").unwrap();
    let texto = fev::archivo(&p);

    assert!(texto.starts_with("{\n \"numDocumentoIdObligado\""), "{:.80}", texto);
    assert!(!texto.contains("xmlFevFile"), "el archivo no lleva el envoltorio de la API");
    assert!(texto.contains("Acetaminofén"), "se escaparon los no-ASCII");

    // Y el BOM va aparte, porque una cadena con BOM dentro es una trampa para
    // cualquier otro consumidor.
    assert_eq!(&fev::con_bom(&texto)[..3], &[0xEF, 0xBB, 0xBF]);
}

/// Las dos puertas que fallan ANTES de enviar: un rechazo del MUV cuesta un
/// ciclo completo de reenvío, y aquí el llamante todavía puede corregir.
#[test]
fn el_sobre_se_niega_a_salir_incompleto() {
    let docs = documentos();
    assert_eq!(
        fev::sobre_rips_sin_factura(&docs, "  ").unwrap_err(),
        fev::NoSePuede::FaltaNumNota
    );
    assert_eq!(
        fev::sobre_fev(&docs, "FAC-1", "").unwrap_err(),
        fev::NoSePuede::FaltaXmlFev
    );
    // La pareja: con lo que falta puesto, los dos salen.
    assert!(fev::sobre_rips_sin_factura(&docs, "NOTA-001").is_ok());
    assert!(fev::sobre_fev(&docs, "FAC-1", "UEsDBBQA").is_ok());
}
