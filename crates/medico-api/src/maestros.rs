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
use std::collections::HashMap;
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
    /// Filas insertadas o actualizadas en la base.
    pub cargadas: u64,
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

    // Deduplicado por CUM: el mismo par expediente-consecutivo aparece en varias
    // filas (fabricante, importador…); nos quedamos con una, la última vista.
    let mut mapa: HashMap<String, MedicamentoNuevo> = HashMap::new();
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
            // Deliberadamente vacía: el valor por fila es por principio activo y
            // en las combinaciones daría una concentración parcial y engañosa. La
            // concentración ya viaja dentro del nombre del producto.
            concentracion: String::new(),
            forma_farmaceutica: i_forma.map(|i| campo(i).to_string()).unwrap_or_default(),
            titular: i_titular
                .map(campo)
                .filter(|t| !t.is_empty())
                .map(str::to_string),
        };
        mapa.insert(codigo_cum, m);
    }

    let lote: Vec<MedicamentoNuevo> = mapa.into_values().collect();
    let resumen = Resumen {
        filas,
        medicamentos: lote.len(),
        omitidas,
        cargadas: 0,
    };
    Ok((lote, resumen))
}

/// Lee el CSV y carga el maestro en la base en una sola transacción.
pub async fn cargar(pool: &medico_datos::Pool, ruta: &Path) -> Result<Resumen, String> {
    let (lote, mut resumen) = leer(ruta)?;
    let n = medico_datos::catalogo::cargar_medicamentos(pool, &lote)
        .await
        .map_err(|e| format!("error cargando en la base: {e}"))?;
    resumen.cargadas = n;
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
        assert_eq!(cardik.concentracion, "", "la concentración se deja al nombre");
        assert!(lote.iter().any(|m| m.codigo_cum == "20077715-48"));
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

    #[tokio::test]
    async fn carga_de_punta_a_punta_en_memoria() {
        let csv = "\
producto,titular,consecutivocum,expedientecum,formafarmaceutica\n\
\"ACETAMINOFEN 500MG\",\"GENFAR\",\"1\",\"19900001\",\"TABLETA\"\n";
        let ruta = csv_temporal("e2e", csv);

        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        let resumen = cargar(&pool, &ruta).await.unwrap();
        std::fs::remove_file(&ruta).ok();

        assert_eq!(resumen.medicamentos, 1);
        assert_eq!(resumen.cargadas, 1);
        let r = medico_datos::catalogo::buscar_medicamentos(&pool, "acetaminofen")
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].codigo_cum, "19900001-1");
    }
}
