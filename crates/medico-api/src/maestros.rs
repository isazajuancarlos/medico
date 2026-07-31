//! Carga del maestro CUM del INVIMA (medicamentos) desde su export CSV.
//!
//! El CUM de un medicamento es el par expediente-consecutivo: las columnas
//! `expedientecum` y `consecutivocum` juntas ("20048021-41"). El export trae
//! además el rol del titular, así que el MISMO CUM aparece repetido en varias
//! filas; el cargador las colapsa por su clave (la última gana), igual que el
//! upsert de la base.
//!
//! Se mapea por NOMBRE de columna, nunca por posición: el INVIMA reordena y
//! agrega columnas entre versiones. Si una columna requerida falta, el comando
//! FALLA e imprime los encabezados que sí encontró — jamás carga con una columna
//! adivinada (directiva 20: ante el dato ausente, ruido, no un relleno).

use medico_datos::catalogo::MedicamentoNuevo;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Cuentas de una carga, para que el operador vea qué entró y qué no.
#[derive(Debug)]
pub struct Resumen {
    /// Filas de datos leídas del CSV (sin contar el encabezado).
    pub filas: u64,
    /// CUM distintos preparados tras colapsar los repetidos.
    pub medicamentos: usize,
    /// Filas descartadas por venir sin CUM o sin nombre.
    pub omitidas: u64,
}

/// Normaliza un encabezado para compararlo sin depender de mayúsculas, acentos ni
/// separadores: `"Forma_Farmacéutica"` y `formafarmaceutica` son la misma columna.
fn normalizar(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => Some('a'),
            'é' | 'è' | 'ë' | 'ê' => Some('e'),
            'í' | 'ì' | 'ï' | 'î' => Some('i'),
            'ó' | 'ò' | 'ö' | 'ô' => Some('o'),
            'ú' | 'ù' | 'ü' | 'û' => Some('u'),
            'ñ' => Some('n'),
            ' ' | '_' | '-' | '.' | '"' => None,
            c => Some(c),
        })
        .collect()
}

/// Lee el CSV a medicamentos ya deduplicados. No toca la base: es la parte
/// probable sin montar un pool.
fn leer(ruta: &Path) -> Result<(Vec<MedicamentoNuevo>, Resumen), String> {
    // El export del INVIMA es por coma; se deja anular con MAESTRO_DELIM por si
    // una versión futura cambia a punto y coma (locale es-CO).
    let delim = std::env::var("MAESTRO_DELIM")
        .ok()
        .and_then(|d| d.bytes().next())
        .unwrap_or(b',');

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_path(ruta)
        .map_err(|e| format!("no pude abrir {}: {e}", ruta.display()))?;

    let encabezado = rdr
        .headers()
        .map_err(|e| format!("no pude leer el encabezado de {}: {e}", ruta.display()))?
        .clone();
    let norm: Vec<String> = encabezado.iter().map(normalizar).collect();
    let crudos: Vec<&str> = encabezado.iter().collect();

    let requerida = |nombre: &str| -> Result<usize, String> {
        norm.iter().position(|h| h == nombre).ok_or_else(|| {
            format!(
                "el CSV no trae la columna «{nombre}», requerida para el maestro CUM. \
                 Columnas encontradas: {crudos:?}"
            )
        })
    };
    let opcional = |nombre: &str| norm.iter().position(|h| h == nombre);

    let i_exp = requerida("expedientecum")?;
    let i_con = requerida("consecutivocum")?;
    let i_prod = requerida("producto")?;
    let i_forma = opcional("formafarmaceutica");
    let i_titular = opcional("titular");
    let i_princ = opcional("principioactivo");
    // `estadocum`: Activo o Inactivo. Ignorarlo cargaba 15 046 medicamentos —el
    // 22 % del maestro— como si se pudieran prescribir. El MUV los rechaza.
    let i_estado = opcional("estadocum");
    // La concentración se COMPONE de tres columnas; la que se llama
    // «concentracion» en el export NO lo es. Ver el bloque que la resuelve.
    let i_cant = opcional("cantidad");
    let i_um = opcional("unidadmedida");
    let i_ur = opcional("unidadreferencia");

    // Deduplicado por CUM: el mismo par expediente-consecutivo aparece en varias
    // filas (fabricante, importador…); nos quedamos con una, la última vista.
    let mut mapa: HashMap<String, MedicamentoNuevo> = HashMap::new();
    // Para decidir la concentración hace falta ver TODAS las filas de un CUM, no
    // la última: un medicamento combinado trae una fila por principio activo, y
    // quedarse con una daría la concentración de uno solo presentada como la del
    // producto. Ver el bloque que las resuelve, más abajo.
    let mut principios: HashMap<String, HashSet<String>> = HashMap::new();
    let mut concentraciones: HashMap<String, HashSet<String>> = HashMap::new();
    let mut filas = 0u64;
    let mut omitidas = 0u64;

    for reg in rdr.records() {
        let reg = reg.map_err(|e| format!("fila {}: {e}", filas + 1))?;
        filas += 1;
        let campo = |i: usize| reg.get(i).unwrap_or("").trim();

        let exp = campo(i_exp);
        let con = campo(i_con);
        let prod = campo(i_prod);
        // Sin CUM o sin nombre la fila no sirve a un maestro: no se rellena, se
        // descarta y se cuenta (directiva 20).
        if exp.is_empty() || con.is_empty() || prod.is_empty() {
            omitidas += 1;
            continue;
        }

        let codigo_cum = format!("{exp}-{con}");
        let m = MedicamentoNuevo {
            codigo_cum: codigo_cum.clone(),
            nombre: prod.to_string(),
            // Se rellena después, cuando se han visto todas las filas del CUM.
            concentracion: String::new(),
            forma_farmaceutica: i_forma.map(|i| campo(i).to_string()).unwrap_or_default(),
            // Sin columna de estado se asume ACTIVO, y es la respuesta correcta
            // para un export que no la traiga: marcarlo todo inactivo dejaría el
            // maestro inservible por una columna que quizá esa versión no usa.
            // Con columna, manda ella.
            activo: i_estado
                .map(|i| !campo(i).eq_ignore_ascii_case("inactivo"))
                .unwrap_or(true),
            titular: i_titular
                .map(campo)
                .filter(|t| !t.is_empty())
                .map(str::to_string),
        };
        if let Some(i) = i_princ {
            let pa = campo(i);
            if !pa.is_empty() {
                principios.entry(codigo_cum.clone()).or_default().insert(pa.to_string());
            }
        }
        // «300» + «mg» + «TABLETA RECUBIERTA» → «300 mg / TABLETA RECUBIERTA».
        // Sin cantidad o sin unidad no hay concentración que componer, y media
        // concentración es peor que ninguna.
        if let (Some(ic), Some(iu)) = (i_cant, i_um) {
            let (cant, um) = (campo(ic), campo(iu));
            if !cant.is_empty() && !um.is_empty() {
                let ur = i_ur.map(campo).unwrap_or("");
                let texto = if ur.is_empty() {
                    format!("{cant} {um}")
                } else {
                    format!("{cant} {um} / {ur}")
                };
                concentraciones.entry(codigo_cum.clone()).or_default().insert(texto);
            }
        }
        mapa.insert(codigo_cum, m);
    }

    // La concentración: se pone cuando NO hay ambigüedad y se deja vacía cuando la
    // hay. No es un relleno ni un defecto (directiva 20): es cargar el dato que
    // existe y abstenerse donde el dato de verdad no es uno solo.
    //
    // **La columna llamada `concentracion` NO es la concentración.** En el export
    // del INVIMA es un código de una letra —A, B, C, S…— y cargarla habría puesto
    // «A» en 45 160 medicamentos: un campo lleno de ruido, peor que vacío, porque
    // lo vacío se ve y lo ruidoso se lee. La concentración real se compone de
    // `cantidad` + `unidadmedida` + `unidadreferencia`: «300 mg / TABLETA
    // RECUBIERTA». Esto se descubrió mirando los valores cargados, no el nombre
    // de la columna.
    //
    // Antes se dejaba vacía SIEMPRE, con el argumento de que en las combinaciones
    // el valor por fila es parcial. El argumento es correcto y cubre el 23 % del
    // maestro: medido sobre el export real del INVIMA (157 789 filas), 50 223 de
    // los 65 997 CUM —el 76 %— tienen UN solo principio activo y UNA sola
    // concentración. En esos no hay nada ambiguo, y vaciarlos perdía el dato que
    // distingue «AMOXICILINA 250 mg» de «AMOXICILINA 500 mg» delante de quien
    // prescribe.
    for (cum, m) in mapa.iter_mut() {
        let un_principio = principios.get(cum).map(|p| p.len() == 1).unwrap_or(false);
        let unica = concentraciones
            .get(cum)
            .filter(|c| c.len() == 1)
            .and_then(|c| c.iter().next());
        if let (true, Some(c)) = (un_principio, unica) {
            m.concentracion = c.clone();
        }
    }

    let lote: Vec<MedicamentoNuevo> = mapa.into_values().collect();
    let resumen = Resumen { filas, medicamentos: lote.len(), omitidas };
    Ok((lote, resumen))
}

/// Lee el CSV y escribe el JSON que se EMPOTRA en el binario.
///
/// Sustituye a la antigua `cargar`, que escribía directamente en la base de un
/// cliente. Con el maestro empotrado y sembrado en cada arranque, aquella carga
/// quedaba PISADA por el siguiente reinicio, en silencio. La lectura —y todo lo
/// que costó aprender en ella— no cambia: solo el destino.
///
/// Las claves del JSON son de un carácter a propósito: son 65 997 registros y
/// cada byte de clave se paga 65 997 veces dentro del binario.
pub fn generar_json(csv: &Path, salida: &Path) -> Result<Resumen, String> {
    let (lote, resumen) = leer(csv)?;
    let medicamentos: Vec<serde_json::Value> = lote
        .iter()
        .map(|m| {
            serde_json::json!({
                "c": m.codigo_cum,
                "n": m.nombre,
                "k": m.concentracion,
                "f": m.forma_farmaceutica,
                "t": m.titular.clone().unwrap_or_default(),
                "a": m.activo,
            })
        })
        .collect();
    // La FECHA del archivo fuente, que es lo único que dice si el maestro está
    // caducado. Se toma del sistema de archivos y no se teclea: un dato escrito a
    // mano sobre la frescura de otro dato envejece el doble de rápido.
    let fecha_fuente = std::fs::metadata(csv)
        .and_then(|m| m.modified())
        .map(|t| {
            let secs = t
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // Fecha civil desde el epoch, sin dependencias: el algoritmo de
            // Howard Hinnant. Solo hace falta el día, no la hora.
            let z = (secs / 86_400) as i64 + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = yoe + era * 400 + i64::from(m <= 2);
            format!("{y:04}-{m:02}-{d:02}")
        })
        .unwrap_or_else(|_| "desconocida".into());

    let doc = serde_json::json!({
        "fuente": csv.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        "fecha_fuente": fecha_fuente,
        "generado_por": "medico-api generar-maestro-cum",
        "advertencia": format!(
            "Export del {fecha_fuente}. Un maestro CUM caducado NO falla: deja pasar \
             medicamentos que ya no existen y rechaza los nuevos, y eso no se nota \
             hasta la glosa. Contrastar con el export vigente del INVIMA antes de \
             facturar."
        ),
        "medicamentos": medicamentos,
    });
    std::fs::write(salida, serde_json::to_string(&doc).map_err(|e| e.to_string())?)
        .map_err(|e| format!("no pude escribir {}: {e}", salida.display()))?;
    Ok(resumen)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Escribe un CSV temporal y devuelve su ruta; el test lo borra al terminar.
    fn csv_temporal(nombre: &str, contenido: &str) -> std::path::PathBuf {
        let ruta = std::env::temp_dir().join(format!("medico-maestro-{nombre}-{}.csv", std::process::id()));
        std::fs::write(&ruta, contenido).unwrap();
        ruta
    }

    #[test]
    fn compone_el_cum_deduplica_y_omite_filas_incompletas() {
        // Encabezado con el orden REORDENADO y una columna de más, para probar el
        // mapeo por nombre. Dos filas del mismo CUM (distinto rol) y una tercera
        // con producto vacío que debe omitirse.
        let csv = "\
producto,titular,consecutivocum,expedientecum,formafarmaceutica,tiporol\n\
\"CARDIK 3 160/5/12.5\",\"LEGRAND\",\"41\",\"20048021\",\"TABLETA RECUBIERTA\",\"FABRICANTE\"\n\
\"CARDIK 3 160/5/12.5\",\"LEGRAND\",\"41\",\"20048021\",\"TABLETA RECUBIERTA\",\"IMPORTADOR\"\n\
\"TRIMEBUTINA 200MG\",\"TECNOQUIMICAS\",\"48\",\"20077715\",\"TABLETA\",\"FABRICANTE\"\n\
\"\",\"SIN NOMBRE\",\"9\",\"99999999\",\"JARABE\",\"FABRICANTE\"\n";
        let ruta = csv_temporal("dedup", csv);

        let (lote, resumen) = leer(&ruta).unwrap();
        std::fs::remove_file(&ruta).ok();

        assert_eq!(resumen.filas, 4, "cuatro filas de datos");
        assert_eq!(resumen.omitidas, 1, "la fila sin producto se omite");
        assert_eq!(resumen.medicamentos, 2, "el CUM repetido se colapsa a uno");

        let cardik = lote.iter().find(|m| m.codigo_cum == "20048021-41").unwrap();
        assert_eq!(cardik.nombre, "CARDIK 3 160/5/12.5");
        assert_eq!(cardik.forma_farmaceutica, "TABLETA RECUBIERTA");
        assert_eq!(cardik.titular.as_deref(), Some("LEGRAND"));
        // Ese CSV no trae columna de concentración: sin dato, campo vacío.
        assert_eq!(cardik.concentracion, "", "sin columna no se inventa concentración");
        assert!(lote.iter().any(|m| m.codigo_cum == "20077715-48"));
    }

    #[test]
    fn la_concentracion_entra_cuando_es_inequivoca_y_se_calla_cuando_no() {
        // La regla en un caso de cada clase, y con las columnas DE VERDAD: la
        // concentración se compone de cantidad + unidad de medida + unidad de
        // referencia. La columna que el export llama «concentracion» es un código
        // de una letra y cargarla habría puesto «A» en 45 160 medicamentos.
        //
        // El combinado trae una fila por principio activo con SU cantidad:
        // quedarse con una daría la de un ingrediente presentada como la del
        // producto, así que se deja vacía. El simple no tiene esa ambigüedad, y
        // su concentración es lo que distingue 250 mg de 500 mg delante de quien
        // prescribe.
        let csv = "\
producto,consecutivocum,expedientecum,principioactivo,concentracion,cantidad,unidadmedida,unidadreferencia\n\
\"AMOXICILINA\",\"1\",\"1000\",\"AMOXICILINA\",\"A\",\"500\",\"mg\",\"CAPSULA\"\n\
\"AMOXICILINA\",\"1\",\"1000\",\"AMOXICILINA\",\"A\",\"500\",\"mg\",\"CAPSULA\"\n\
\"CARDIK 3\",\"2\",\"2000\",\"VALSARTAN\",\"A\",\"160\",\"mg\",\"TABLETA\"\n\
\"CARDIK 3\",\"2\",\"2000\",\"AMLODIPINO\",\"A\",\"5\",\"mg\",\"TABLETA\"\n\
\"SOLO CANTIDAD\",\"3\",\"3000\",\"X\",\"A\",\"10\",\"\",\"FRASCO\"\n";
        let ruta = csv_temporal("concentracion", csv);
        let (lote, _) = leer(&ruta).unwrap();
        std::fs::remove_file(&ruta).ok();

        let buscar = |cum: &str| lote.iter().find(|m| m.codigo_cum == cum).unwrap();
        assert_eq!(
            buscar("1000-1").concentracion,
            "500 mg / CAPSULA",
            "un solo principio activo y una sola cantidad: no hay nada ambiguo"
        );
        assert_eq!(
            buscar("2000-2").concentracion,
            "",
            "un combinado no tiene UNA concentración; poner la de un ingrediente engaña"
        );
        assert_eq!(
            buscar("3000-3").concentracion,
            "",
            "sin unidad de medida no hay concentración: media concentración engaña más que ninguna"
        );
        // Y jamás el código de una letra, que es lo que casi se carga.
        for m in &lote {
            assert!(m.concentracion != "A", "se cargó el CÓDIGO en vez de la concentración");
        }
    }

    #[test]
    fn falla_ruidoso_si_falta_una_columna_requerida() {
        // Sin «producto»: no se puede nombrar el medicamento.
        let csv = "expedientecum,consecutivocum,titular\n\"1\",\"2\",\"X\"\n";
        let ruta = csv_temporal("falta-col", csv);

        let err = leer(&ruta).unwrap_err();
        std::fs::remove_file(&ruta).ok();

        assert!(err.contains("producto"), "el error nombra la columna que falta: {err}");
        assert!(err.contains("Columnas encontradas"), "y lista las que sí vio: {err}");
    }

    /// De punta a punta por el camino de VERDAD: CSV -> JSON empotrable.
    ///
    /// Antes esta prueba iba CSV -> base, porque el subcomando cargaba en la
    /// base del cliente. Al empotrar el CUM (#77) ese camino desapareció —una
    /// carga manual quedaba pisada por el siguiente arranque—, así que la prueba
    /// sigue al destino nuevo. Lo que va del JSON a la base lo cubren las
    /// pruebas de `medico_datos::clinico`.
    #[test]
    fn de_punta_a_punta_el_csv_se_vuelve_json_empotrable() {
        let csv = "\
producto,titular,consecutivocum,expedientecum,formafarmaceutica,principioactivo,cantidad,unidadmedida\n\
\"ACETAMINOFEN 500MG\",\"GENFAR\",\"1\",\"19900001\",\"TABLETA\",\"ACETAMINOFEN\",\"500\",\"mg\"\n";
        let ruta = csv_temporal("e2e", csv);
        let salida = std::env::temp_dir().join("cum-prueba.json");

        let resumen = generar_json(&ruta, &salida).unwrap();
        std::fs::remove_file(&ruta).ok();

        assert_eq!(resumen.medicamentos, 1);
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&salida).unwrap()).unwrap();
        std::fs::remove_file(&salida).ok();

        let m = &doc["medicamentos"][0];
        assert_eq!(m["c"], "19900001-1", "el CUM es expediente-consecutivo");
        assert_eq!(m["n"], "ACETAMINOFEN 500MG");
        assert_eq!(m["f"], "TABLETA");
        assert_eq!(m["t"], "GENFAR");
        // Un principio activo y una concentración: se resuelve.
        assert_eq!(m["k"], "500 mg");
        // Y la advertencia sobre la caducidad del maestro viaja con los datos.
        assert!(
            doc["advertencia"].as_str().unwrap().contains("no se nota hasta la glosa"),
            "{}", doc["advertencia"]
        );
    }

    /// La concentración se deja VACÍA cuando el medicamento es combinado: el
    /// valor por fila es de UN principio activo y presentarlo como el del
    /// producto sería falso. Media concentración es peor que ninguna.
    #[test]
    fn un_combinado_se_queda_sin_concentracion() {
        let csv = "\
producto,consecutivocum,expedientecum,principioactivo,cantidad,unidadmedida\n\
\"COMBINADO\",\"1\",\"200\",\"AMOXICILINA\",\"500\",\"mg\"\n\
\"COMBINADO\",\"1\",\"200\",\"CLAVULANICO\",\"125\",\"mg\"\n";
        let ruta = csv_temporal("combi", csv);
        let salida = std::env::temp_dir().join("cum-combi.json");
        generar_json(&ruta, &salida).unwrap();
        std::fs::remove_file(&ruta).ok();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&salida).unwrap()).unwrap();
        std::fs::remove_file(&salida).ok();
        assert_eq!(doc["medicamentos"][0]["k"], "", "dos principios activos: no se elige uno");
    }
}
