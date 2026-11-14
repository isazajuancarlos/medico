// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **El FEV-RIPS de EXTREMO A EXTREMO es el mismo en Rust y en Python** (#598).
//!
//! Es la última medición que el ADR pide antes de apagar el Python. Ya está
//! probado que, dados los MISMOS cinco documentos, la fusión y el sobre de Rust
//! son byte a byte los del Python (`fev_rips_paridad_con_python.rs`). Lo que esa
//! prueba NO cruza es la frontera **base → documentos**: que `medico-datos` con
//! SQLx traiga las mismas filas que `rips_completo` con SQLModel. Aquí se cruza.
//!
//! # Por qué no es «una sola base que ambos leen»
//!
//! Los dos esquemas divergen (medido el 2026-08-09): el documento del paciente
//! es `tipo_documento`/`numero_documento` en Rust y `tipo_id`/`identificacion`
//! en Python; el destino de urgencias es «DOMICILIO» en Rust y «Domicilio» en
//! Python; hay columnas reubicadas. Así que el enfoque es **sembrar datos
//! EQUIVALENTES en cada esquema** —desde UN mismo `spec` neutral, que es la
//! única fuente de la verdad— y comparar las dos cadenas de salida. El riesgo se
//! traslada al sembrado, y por eso el `spec` es único: si las dos siembras no
//! son equivalentes, la culpa es de una de las dos traducciones y se ve en el
//! diff, no de la generación.
//!
//! # La zona horaria se fija en UTC a propósito
//!
//! Rust convierte la fecha guardada (UTC) a la zona del prestador antes de
//! emitirla; Python emite el naive tal cual. Con la zona del prestador en UTC la
//! conversión de Rust es la identidad, así que los dos parten del mismo instante
//! de pared y `fechaInicioAtencion` coincide sin aritmética de husos. La
//! conversión de zona es comportamiento SOLO de Rust —Python no la hace—, así
//! que neutralizarla en los dos lados es lo correcto para una prueba de
//! EQUIVALENCIA; su corrección tiene sus propias pruebas en `zona.rs`.
//!
//! # Si falta el Python, esto FALLA; no se salta (directivas 20 y 30).

use std::path::PathBuf;
use std::process::Command;

use chrono::NaiveDate;
use medico_dominio::rips::fev;
use medico_dominio::rips::recolector::Recolector;
use medico_dominio::rips::{documento_ac, documento_ah, documento_am, documento_ap, documento_au};
use serde_json::{json, Value};

/// El escenario, en un `spec` NEUTRAL que las dos siembras traducen a su esquema.
///
/// Está construido para EJERCITAR la fusión de verdad, no para pasar fácil:
///
/// - El paciente 7 aparece en los CINCO documentos (dos consultas, un
///   procedimiento, un medicamento, una urgencia y una hospitalización), así que
///   la fusión tiene que juntarlo en un solo usuario con todos sus servicios.
/// - Sus dos consultas obligan a renumerar el consecutivo dentro del tipo.
/// - Sus dos consultas traen `genera_incapacidad` distinto (false y true): la
///   fusión tiene que quedarse con «SI».
/// - El paciente 8 va aparte, para que haya más de un usuario que ordenar.
///
/// El orden de las historias es ascendente por fecha: es el que Rust produce
/// (`ORDER BY h.fecha, h.id`) y el que las cinco consultas SIN `order_by` del
/// Python devuelven por rowid = orden de inserción. Si se rompe, la numeración
/// diverge y la prueba se pone roja.
fn escenario() -> Value {
    let paciente = |doc: &str| {
        json!({
            "tipo_documento": "CC", "numero_documento": doc,
            "fecha_nacimiento": "1980-05-04", "sexo": "M", "tipo_usuario": "01",
            "cod_pais": "170", "cod_municipio": "05001", "zona": "01",
            "cod_pais_origen": "170"
        })
    };
    let mut siete = paciente("111");
    siete["ref"] = json!(7);
    let mut ocho = paciente("222");
    ocho["ref"] = json!(8);

    json!({
        "periodo": { "inicio": "2026-03-01", "fin": "2026-03-31" },
        "institucion": {
            "nit": "9001234560",
            "codigo_habilitacion": "0500101234",
            "codigo_prestador": "0500101234"
        },
        "profesional": { "id": 100, "tipo_documento": "CC", "numero_documento": "79123456" },
        "enfermera": { "id": 101 },
        "cama": { "id": 1, "numero": "101-A", "piso": "1", "servicio": "GENERAL" },
        "pacientes": [ siete, ocho ],
        "historias": [
            {
                "id": 1, "paciente_ref": 7, "fecha": "2026-03-15 08:00:00",
                "cod_consulta_cups": "890201", "modalidad_grupo_servicio": "01",
                "grupo_servicios": "01", "cod_servicio": 328, "finalidad": "11",
                "causa_externa": "38", "tipo_diagnostico": "01",
                "diagnostico_principal": "A000", "valor_servicio": 45000,
                "concepto_recaudo": "05", "genera_incapacidad": false,
                "ordenes": [
                    {
                        "tipo": "Laboratorio", "codigo_cups": "902210",
                        "via_ingreso_servicio": "01", "grupo_servicios": "02",
                        "cod_servicio": 105, "valor_servicio": 30000,
                        "concepto_recaudo": "05"
                    }
                ],
                "formulas": [
                    {
                        "medicamento": "Acetaminofén 500 mg", "codigo_cum": "103795-1",
                        "tipo_medicamento": "01", "unidad": "tableta", "cantidad": 20,
                        "dias_tratamiento": 5, "valor_unitario": 200,
                        "valor_servicio": 4000, "concepto_recaudo": "05"
                    }
                ]
            },
            {
                "id": 2, "paciente_ref": 8, "fecha": "2026-03-15 09:00:00",
                "cod_consulta_cups": "890201", "modalidad_grupo_servicio": "01",
                "grupo_servicios": "01", "cod_servicio": 328, "finalidad": "11",
                "causa_externa": "38", "tipo_diagnostico": "01",
                "diagnostico_principal": "A000", "valor_servicio": 45000,
                "concepto_recaudo": "05", "genera_incapacidad": true,
                "ordenes": [], "formulas": []
            },
            {
                // La SEGUNDA consulta del paciente 7: renumera el consecutivo
                // dentro del tipo. Su incapacidad va IGUAL que la primera (false)
                // a propósito: Python fija la incapacidad del usuario con la
                // PRIMERA historia que ve en el AC, así que un valor distinto
                // aquí mediría esa regla de orden y no la equivalencia base→JSON.
                "id": 3, "paciente_ref": 7, "fecha": "2026-03-15 10:00:00",
                "cod_consulta_cups": "890201", "modalidad_grupo_servicio": "01",
                "grupo_servicios": "01", "cod_servicio": 328, "finalidad": "11",
                "causa_externa": "38", "tipo_diagnostico": "01",
                "diagnostico_principal": "A000", "valor_servicio": 45000,
                "concepto_recaudo": "05", "genera_incapacidad": false,
                "ordenes": [], "formulas": []
            }
        ],
        "urgencias": [
            {
                "id": 1, "paciente_ref": 7,
                "fecha_ingreso": "2026-03-16 22:10:00", "fecha_egreso": "2026-03-17 02:45:00",
                "causa_motivo": "38", "diagnostico_ingreso": "A000",
                "diagnostico_egreso": "A000", "destino_salida": "DOMICILIO",
                "valor_servicio": 80000, "concepto_recaudo": "05"
            }
        ],
        "hospitalizaciones": [
            {
                "id": 1, "paciente_ref": 7,
                "fecha_ingreso": "2026-03-18 08:00:00", "fecha_egreso": "2026-03-20 11:30:00",
                "via_ingreso": "URGENCIAS", "estado": "ALTA_MEDICA", "causa_motivo": "38",
                "diagnostico_ingreso": "A000", "diagnostico_egreso": "A000",
                "valor_servicio": 500000, "concepto_recaudo": "05"
            }
        ]
    })
}

fn dia(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Siembra la base de Rust desde el `spec`, traduciendo cada campo neutral a la
/// columna que el esquema de SQLx tiene. Es la pareja del arnés de Python: los
/// dos leen el MISMO `spec`, así que un valor equivocado aquí sale en el diff.
async fn sembrar_rust(pool: &medico_datos::Pool, spec: &Value) {
    let s = |v: &Value| v.as_str().unwrap().to_string();
    let n = |v: &Value| v.as_i64().unwrap();

    let inst = &spec["institucion"];
    sqlx::query(
        "INSERT INTO institucion (id, nombre, nit, codigo_habilitacion, activa) \
         VALUES (1, 'IPS de prueba', ?, ?, 1)",
    )
    .bind(s(&inst["nit"]))
    .bind(s(&inst["codigo_habilitacion"]))
    .execute(pool)
    .await
    .unwrap();

    let prof = &spec["profesional"];
    let medico_id = n(&prof["id"]);
    sqlx::query(
        "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo, \
                              tipo_documento, numero_documento) \
         VALUES (?, 'Médico', ?, 'x', 'medico', 1, 1, ?, ?)",
    )
    .bind(medico_id)
    .bind(format!("medico{medico_id}@x.test"))
    .bind(s(&prof["tipo_documento"]))
    .bind(s(&prof["numero_documento"]))
    .execute(pool)
    .await
    .unwrap();

    let enfermera_id = n(&spec["enfermera"]["id"]);
    sqlx::query(
        "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo) \
         VALUES (?, 'Enfermera', ?, 'x', 'enfermera', 1, 1)",
    )
    .bind(enfermera_id)
    .bind(format!("enfermera{enfermera_id}@x.test"))
    .execute(pool)
    .await
    .unwrap();

    let cama = &spec["cama"];
    let cama_id = n(&cama["id"]);
    sqlx::query("INSERT INTO cama (id, institucion_id, numero, piso, servicio) VALUES (?, 1, ?, ?, ?)")
        .bind(cama_id)
        .bind(s(&cama["numero"]))
        .bind(s(&cama["piso"]))
        .bind(s(&cama["servicio"]))
        .execute(pool)
        .await
        .unwrap();

    for p in spec["pacientes"].as_array().unwrap() {
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1, \
                                   fecha_nacimiento, sexo, institucion_id, tipo_usuario, cod_pais, \
                                   cod_municipio, zona, cod_pais_origen) \
             VALUES (?, ?, ?, 'Paciente', ?, ?, ?, 1, ?, ?, ?, ?, ?)",
        )
        .bind(n(&p["ref"]))
        .bind(s(&p["tipo_documento"]))
        .bind(s(&p["numero_documento"]))
        .bind(format!("Numero{}", n(&p["ref"])))
        .bind(s(&p["fecha_nacimiento"]))
        .bind(s(&p["sexo"]))
        .bind(s(&p["tipo_usuario"]))
        .bind(s(&p["cod_pais"]))
        .bind(s(&p["cod_municipio"]))
        .bind(s(&p["zona"]))
        .bind(s(&p["cod_pais_origen"]))
        .execute(pool)
        .await
        .unwrap();
    }

    for h in spec["historias"].as_array().unwrap() {
        let hid = n(&h["id"]);
        sqlx::query(
            "INSERT INTO historia (id, codigo_historia, paciente_id, medico_id, fecha, \
                                   motivo_consulta, enfermedad_actual, diagnostico_principal, \
                                   plan_tratamiento, cod_consulta_cups, modalidad_grupo_servicio, \
                                   grupo_servicios, cod_servicio, finalidad, causa_externa, \
                                   tipo_diagnostico, valor_servicio, concepto_recaudo, \
                                   genera_incapacidad) \
             VALUES (?, ?, ?, ?, ?, 'Motivo', 'Enfermedad', ?, 'Plan', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(hid)
        .bind(format!("HC-{hid}"))
        .bind(n(&h["paciente_ref"]))
        .bind(medico_id)
        .bind(s(&h["fecha"]))
        .bind(s(&h["diagnostico_principal"]))
        .bind(s(&h["cod_consulta_cups"]))
        .bind(s(&h["modalidad_grupo_servicio"]))
        .bind(s(&h["grupo_servicios"]))
        .bind(n(&h["cod_servicio"]))
        .bind(s(&h["finalidad"]))
        .bind(s(&h["causa_externa"]))
        .bind(s(&h["tipo_diagnostico"]))
        .bind(n(&h["valor_servicio"]))
        .bind(s(&h["concepto_recaudo"]))
        .bind(h["genera_incapacidad"].as_bool().unwrap() as i64)
        .execute(pool)
        .await
        .unwrap();

        for o in h["ordenes"].as_array().unwrap() {
            sqlx::query(
                "INSERT INTO orden (historia_id, tipo, descripcion, justificacion, codigo_cups, \
                                    via_ingreso_servicio, grupo_servicios, cod_servicio, \
                                    valor_servicio, concepto_recaudo) \
                 VALUES (?, ?, 'Orden', 'Justificación', ?, ?, ?, ?, ?, ?)",
            )
            .bind(hid)
            .bind(s(&o["tipo"]))
            .bind(s(&o["codigo_cups"]))
            .bind(s(&o["via_ingreso_servicio"]))
            .bind(s(&o["grupo_servicios"]))
            .bind(n(&o["cod_servicio"]))
            .bind(n(&o["valor_servicio"]))
            .bind(s(&o["concepto_recaudo"]))
            .execute(pool)
            .await
            .unwrap();
        }

        for m in h["formulas"].as_array().unwrap() {
            sqlx::query(
                "INSERT INTO formula (historia_id, medicamento, dosis, cantidad, codigo_cum, \
                                      tipo_medicamento, unidad, dias_tratamiento, valor_unitario, \
                                      valor_servicio, concepto_recaudo) \
                 VALUES (?, ?, '1 cada 8h', ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(hid)
            .bind(s(&m["medicamento"]))
            .bind(m["cantidad"].as_f64().unwrap())
            .bind(s(&m["codigo_cum"]))
            .bind(s(&m["tipo_medicamento"]))
            .bind(s(&m["unidad"]))
            .bind(n(&m["dias_tratamiento"]))
            .bind(n(&m["valor_unitario"]))
            .bind(n(&m["valor_servicio"]))
            .bind(s(&m["concepto_recaudo"]))
            .execute(pool)
            .await
            .unwrap();
        }
    }

    for u in spec["urgencias"].as_array().unwrap() {
        // El AU no lleva bloque económico ni en el esquema ni en el JSON: la tabla
        // de Rust no tiene `valor_servicio`/`concepto_recaudo`, así que no se
        // insertan (el `spec` los trae para el lado Python, cuyo modelo sí los
        // hereda de `CamposRips`, pero tampoco los emite).
        sqlx::query(
            "INSERT INTO atencion_urgencias (id, paciente_id, triage, motivo_consulta, \
                                             enfermera_triage_id, fecha_ingreso, fecha_egreso, \
                                             causa_motivo, diagnostico_ingreso, diagnostico_egreso, \
                                             destino_salida) \
             VALUES (?, ?, 'TRIAGE_3', 'Motivo', ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(n(&u["id"]))
        .bind(n(&u["paciente_ref"]))
        .bind(enfermera_id)
        .bind(s(&u["fecha_ingreso"]))
        .bind(s(&u["fecha_egreso"]))
        .bind(s(&u["causa_motivo"]))
        .bind(s(&u["diagnostico_ingreso"]))
        .bind(s(&u["diagnostico_egreso"]))
        .bind(s(&u["destino_salida"]))
        .execute(pool)
        .await
        .unwrap();
    }

    for hosp in spec["hospitalizaciones"].as_array().unwrap() {
        // El AH tampoco lleva bloque económico (igual que el AU): la tabla de Rust
        // no tiene esas columnas y el JSON no las emite.
        sqlx::query(
            "INSERT INTO hospitalizacion (id, paciente_id, cama_id, medico_tratante_id, \
                                          fecha_ingreso, fecha_egreso, via_ingreso, estado, \
                                          causa_motivo, diagnostico_ingreso, diagnostico_egreso) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(n(&hosp["id"]))
        .bind(n(&hosp["paciente_ref"]))
        .bind(cama_id)
        .bind(medico_id)
        .bind(s(&hosp["fecha_ingreso"]))
        .bind(s(&hosp["fecha_egreso"]))
        .bind(s(&hosp["via_ingreso"]))
        .bind(s(&hosp["estado"]))
        .bind(s(&hosp["causa_motivo"]))
        .bind(s(&hosp["diagnostico_ingreso"]))
        .bind(s(&hosp["diagnostico_egreso"]))
        .execute(pool)
        .await
        .unwrap();
    }
}

/// El intérprete que puede importar el backend. Igual que la prueba de fusión.
fn interprete() -> String {
    let venv = concat!(env!("CARGO_MANIFEST_DIR"), "/../../backend/.venv/bin/python");
    if std::path::Path::new(venv).exists() {
        return venv.to_string();
    }
    "python3".to_string()
}

/// Siembra la base de Rust desde el `spec` y devuelve el FEV compacto.
///
/// Reproduce la tubería del binario (`medico-api::rips::generar_documentos` +
/// `fev`), con el recolector en modo ESTRICTO: si el escenario deja una sola
/// falta, la prueba se detiene aquí —es el control positivo de que la siembra
/// está completa y de que se está comparando un documento de verdad, no dos
/// vacíos iguales (directiva 30)—.
async fn fev_de_rust(db: &std::path::Path, spec: &Value, num_nota: &str) -> String {
    let url = format!("sqlite://{}", db.display());
    let pool = medico_datos::abrir(&url).await.expect("abrir la base de Rust");
    sembrar_rust(&pool, spec).await;

    let ini = dia(spec["periodo"]["inicio"].as_str().unwrap());
    let fin = dia(spec["periodo"]["fin"].as_str().unwrap());

    let institucion = medico_datos::rips::institucion_que_reporta(&pool, 1)
        .await
        .unwrap()
        .expect("la institución sembrada existe");
    let consultas = medico_datos::rips::consultas_del_periodo(&pool, 1, ini, fin)
        .await
        .unwrap()
        .unwrap();
    let procedimientos = medico_datos::rips::procedimientos_del_periodo(&pool, 1, ini, fin)
        .await
        .unwrap()
        .unwrap();
    let medicamentos = medico_datos::rips::medicamentos_del_periodo(&pool, 1, ini, fin)
        .await
        .unwrap()
        .unwrap();
    let urgencias = medico_datos::rips::urgencias_del_periodo(&pool, 1, ini, fin)
        .await
        .unwrap()
        .unwrap();
    let hospitalizaciones = medico_datos::rips::hospitalizaciones_del_periodo(&pool, 1, ini, fin)
        .await
        .unwrap()
        .unwrap();

    let mut rec = Recolector::estricto();
    let ac = documento_ac(&institucion, &consultas, &mut rec);
    let ap = documento_ap(&institucion, &procedimientos, &mut rec);
    let am = documento_am(&institucion, &medicamentos, &mut rec);
    let au = documento_au(&institucion, &urgencias, &mut rec);
    let ah = documento_ah(&institucion, &hospitalizaciones, &mut rec);
    assert!(
        rec.faltantes().is_empty(),
        "la siembra de Rust dejó faltas —el escenario no está completo, así que \
         la comparación mediría documentos con supuestos: {:#?}",
        rec.faltantes()
    );

    let docs = [ac, ap, am, au, ah];
    let paquete = fev::sobre_rips_sin_factura(&docs, num_nota).expect("el sobre sale");
    serde_json::to_string(&paquete).unwrap()
}

/// Genera el FEV del lado Python: escribe el `spec` y corre el arnés.
fn fev_de_python(spec: &Value, spec_path: &std::path::Path, db: &std::path::Path, num_nota: &str) -> String {
    std::fs::write(spec_path, serde_json::to_vec(spec).unwrap()).unwrap();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/apagar_python/generar_fev_desde_base.py");
    let salida = Command::new(interprete())
        .arg(script)
        .arg(spec_path)
        .arg(db)
        .arg(num_nota)
        .output()
        .expect(
            "no se pudo ejecutar python3: esta prueba compara contra el Python de verdad y sin \
             él no comprueba nada, así que falla en vez de saltarse",
        );
    assert!(
        salida.status.success(),
        "el arnés de Python falló:\n{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    String::from_utf8(salida.stdout).unwrap()
}

/// Un directorio temporal propio para cada prueba, para que no se pisen.
fn temporal(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("medico_fev_e2e_{}_{}", std::process::id(), etiqueta));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// **La medición**: el FEV de extremo a extremo es idéntico en los dos sistemas.
#[tokio::test]
async fn el_fev_de_extremo_a_extremo_es_el_mismo_en_los_dos() {
    medico_dominio::zona::fijar_por_nombre("Etc/UTC").unwrap();
    let spec = escenario();
    let dir = temporal("igual");

    let en_rust = fev_de_rust(&dir.join("rust.db"), &spec, "NOTA-001").await;
    let en_python = fev_de_python(&spec, &dir.join("spec.json"), &dir.join("python.db"), "NOTA-001");

    // Control positivo: el documento comparado es NO TRIVIAL. Sin esto, dos FEV
    // vacíos iguales pasarían por verde (directiva 30). Se comprueba sobre la
    // salida de Rust; como es idéntica a la de Python, vale para las dos.
    let p: Value = serde_json::from_str(&en_rust).unwrap();
    let usuarios = p["rips"]["usuarios"].as_array().unwrap();
    assert_eq!(usuarios.len(), 2, "el escenario tiene dos pacientes distintos");
    let u7 = usuarios.iter().find(|u| u["numDocumentoIdentificacion"] == "111").unwrap();
    let u8 = usuarios.iter().find(|u| u["numDocumentoIdentificacion"] == "222").unwrap();
    assert_eq!(u7["incapacidad"], "NO", "las dos consultas del 7 no generan incapacidad");
    assert_eq!(u8["incapacidad"], "SI", "la consulta del 8 sí");
    let s = &u7["servicios"];
    assert_eq!(s["consultas"].as_array().unwrap().len(), 2, "las dos consultas fusionadas");
    assert_eq!(s["procedimientos"].as_array().unwrap().len(), 1);
    assert_eq!(s["medicamentos"].as_array().unwrap().len(), 1);
    assert_eq!(s["urgencias"].as_array().unwrap().len(), 1);
    assert_eq!(s["hospitalizacion"].as_array().unwrap().len(), 1);

    assert_eq!(
        en_rust, en_python,
        "el FEV-RIPS de extremo a extremo de Rust no es el del Python"
    );
}

/// **El control de que la comparación DISCRIMINA** (directiva 33).
///
/// Si la prueba de arriba pasara con cualquier par de entradas —o si las dos
/// tuberías fueran ciegas a una fila—, no mediría nada. Aquí se le quita al
/// escenario del Python la hospitalización, y se EXIGE que el FEV resultante ya
/// no coincida con el de Rust: la divergencia entra por la vía real —una fila
/// que un sistema trae y el otro no—, que es exactamente la propiedad que la
/// prueba de arriba afirma.
#[tokio::test]
async fn quitar_una_fila_a_un_lado_pone_los_dos_fev_distintos() {
    medico_dominio::zona::fijar_por_nombre("Etc/UTC").unwrap();
    let spec = escenario();
    let dir = temporal("rojo");

    let en_rust = fev_de_rust(&dir.join("rust.db"), &spec, "NOTA-001").await;

    let mut mutado = spec.clone();
    mutado["hospitalizaciones"] = json!([]);
    let en_python = fev_de_python(&mutado, &dir.join("spec.json"), &dir.join("python.db"), "NOTA-001");

    assert_ne!(
        en_rust, en_python,
        "quitarle la hospitalización a un lado tenía que cambiar el FEV; si no, \
         la comparación no está midiendo qué filas trae cada sistema"
    );
}
