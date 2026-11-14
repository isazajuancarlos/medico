//! Datos que no hay que pedir porque ya están en otro sitio (#84). Port de
//! `backend/derivaciones.py`.
//!
//! Es la contrapartida de haber quitado los defectos. Cada uno que se quitó hizo
//! la captura más lenta, y bajo la presión de tiempo de una consulta lo lento se
//! rodea: se escribe cualquier cosa con tal de cerrar la nota, y se cambiaría un
//! dato falso por uno basura, con más fricción. La salida no es exigir más sino
//! pedir menos: **derivar lo que ya se puede saber**. Un valor derivado cuesta
//! cero segundos al clínico, es reproducible y —esto es lo que lo separa de un
//! defecto— queda marcado como [`Procedencia::Derivado`], así que cualquiera puede
//! ver de dónde salió y desconfiar de él si desconfía del origen.
//!
//! Cada derivación devuelve `None` cuando no puede derivar. **No aproxima**:
//! derivar mal es exactamente el problema que se está resolviendo. Elegir el
//! primero de varios «San Juan» sería inventar, y eso es peor que dejar el hueco,
//! porque un hueco se ve y un municipio inventado no (directiva 20).
//!
//! # Puro a propósito
//!
//! Este módulo NO toca la base. La búsqueda de candidatos por nombre vive en
//! `medico_datos::divipola`; aquí llega ya resuelta como `&[Candidato]`, y la
//! lógica de desambiguación se prueba sin un pool. El que orquesta —leer el
//! paciente, no pisar lo confirmado, marcar la procedencia y guardar— es
//! `medico_datos::paciente::derivar_municipio`.

use crate::procedencia::Procedencia;

/// La procedencia con la que nace TODO lo que sale de este módulo: fue la máquina
/// quien lo calculó, no un humano quien lo afirmó.
pub const PROCEDENCIA: Procedencia = Procedencia::Derivado;

/// Un municipio candidato para resolver por nombre. Es lo mínimo que la
/// desambiguación necesita; lo llena el llamante desde `divipola::Municipio`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidato {
    /// El código DIVIPOLA de cinco dígitos (con el cero a la izquierda).
    pub codigo: String,
    /// El nombre del municipio, tal cual el catálogo.
    pub nombre: String,
    /// El nombre del departamento, para desambiguar cuando la captura lo trae.
    pub nombre_departamento: String,
}

/// Código DIVIPOLA del municipio a partir del nombre de la ciudad ya capturada.
///
/// `Paciente` guardaba `ciudad_residencia`/`departamento_residencia` como texto
/// libre desde el sistema viejo. Si el nombre identifica un municipio **sin
/// ambigüedad**, no hay por qué pedir el código.
///
/// `municipios` es el catálogo DIVIPOLA (o cualquier superconjunto de los
/// candidatos); esta función hace TODO el trabajo, incluido el filtro por
/// subcadena, para que el plegado de tildes viva en un solo sitio y no dependa
/// del `LIKE` de la base —que no pliega, y con él «Medellin» no encontraría
/// «Medellín»—. La regla, calcada del Python:
///
///   1. sin ciudad → `None` (no hay de dónde derivar);
///   2. los municipios cuyo nombre CONTIENE la ciudad (plegado), como
///      `buscar_por_nombre`;
///   3. si se dio el departamento, se filtra a los de ese departamento;
///   4. si hay **exactamente un** candidato cuyo nombre COINCIDE (no solo
///      contiene) → ese;
///   5. si tras el filtro queda **exactamente un** candidato → ese;
///   6. en cualquier otro caso → `None`. Varios «San Juan» no se resuelven
///      eligiendo el primero.
///
/// Compara plegando tildes y caja: en Colombia los nombres las llevan y la
/// captura vieja las teclea sin ellas.
pub fn municipio_desde_texto(
    ciudad: Option<&str>,
    departamento: Option<&str>,
    municipios: &[Candidato],
) -> Option<String> {
    let ciudad = ciudad.map(str::trim).filter(|c| !c.is_empty())?;
    let ciudad_plegada = plegar(ciudad);

    // Paso 2: los que CONTIENEN el texto (plegado). Es lo que hacía
    // `buscar_por_nombre` en la base, hecho aquí para no perder las tildes.
    let contienen = municipios.iter().filter(|c| plegar(&c.nombre).contains(&ciudad_plegada));

    // Paso 3: si la captura trae departamento, solo cuentan los de ese
    // departamento. Si no trae, cuentan todos los que contienen.
    let dep = departamento.map(str::trim).filter(|d| !d.is_empty());
    let filtrados: Vec<&Candidato> = match dep {
        Some(dep) => contienen.filter(|c| plegar(&c.nombre_departamento) == plegar(dep)).collect(),
        None => contienen.collect(),
    };

    // Paso 4: un único nombre que COINCIDE exactamente (plegado) manda sobre las
    // subcadenas —«Santa Rosa» no debe resolverse a «Santa Rosa de Osos» solo
    // porque lo contenga, pero sí si es el único que casa exacto—.
    let exactos: Vec<&&Candidato> =
        filtrados.iter().filter(|c| plegar(&c.nombre) == ciudad_plegada).collect();
    if let [uno] = exactos.as_slice() {
        return Some(uno.codigo.clone());
    }

    // Paso 5: sin coincidencia exacta única, solo se deriva si NO hay ambigüedad
    // —un solo candidato que contiene—.
    if let [uno] = filtrados.as_slice() {
        return Some(uno.codigo.clone());
    }

    // Paso 6: 0 o >1 → no se puede saber. No se aproxima.
    None
}

/// Pliega tildes y caja para comparar nombres. Igual criterio que
/// `muv::normalizar_nombre` y `paciente::plegar`, local aquí porque el dominio no
/// depende de `medico-datos` y la de `muv` es privada; su pareja
/// [`pruebas::plegar_iguala_tildes_y_caja`] la ata.
fn plegar(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            'á' | 'Á' | 'à' | 'À' | 'ä' | 'Ä' | 'â' | 'Â' => 'a',
            'é' | 'É' | 'è' | 'È' | 'ë' | 'Ë' | 'ê' | 'Ê' => 'e',
            'í' | 'Í' | 'ì' | 'Ì' | 'ï' | 'Ï' | 'î' | 'Î' => 'i',
            'ó' | 'Ó' | 'ò' | 'Ò' | 'ö' | 'Ö' | 'ô' | 'Ô' => 'o',
            'ú' | 'Ú' | 'ù' | 'Ù' | 'ü' | 'Ü' | 'û' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            otro => otro.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(codigo: &str, nombre: &str, depto: &str) -> Candidato {
        Candidato {
            codigo: codigo.into(),
            nombre: nombre.into(),
            nombre_departamento: depto.into(),
        }
    }

    #[test]
    fn lo_que_sale_de_aqui_nace_derivado() {
        // El punto entero de #84: no es un defecto ni un confirmado.
        assert_eq!(PROCEDENCIA, Procedencia::Derivado);
        assert!(!PROCEDENCIA.avalado_por_humano(), "lo derivó la máquina");
    }

    /// El corazón de la derivación y su pareja (directiva 33): un nombre único
    /// resuelve; varios homónimos NO —y ahí está el caso rojo que descarta un
    /// «elige el primero» ciego—.
    #[test]
    fn un_nombre_unico_resuelve_y_los_homonimos_no() {
        // VERDE: un solo Medellín.
        let uno = [c("05001", "Medellín", "Antioquia")];
        assert_eq!(
            municipio_desde_texto(Some("Medellin"), None, &uno),
            Some("05001".into()),
            "sin tilde tiene que casar igual"
        );

        // ROJO: tres «San Juan» sin departamento → no se puede saber.
        let varios = [
            c("70678", "San Juan", "Sucre"),
            c("20710", "San Juan", "Cesar"),
            c("44650", "San Juan", "La Guajira"),
        ];
        assert_eq!(
            municipio_desde_texto(Some("San Juan"), None, &varios),
            None,
            "elegir el primero de tres homónimos sería inventar"
        );
    }

    /// El departamento desambigua: el mismo «San Juan», con departamento, sí
    /// resuelve. Y su pareja: un departamento que no casa con ninguno → None.
    #[test]
    fn el_departamento_desambigua() {
        let varios = [
            c("70678", "San Juan", "Sucre"),
            c("20710", "San Juan", "Cesar"),
            c("44650", "San Juan", "La Guajira"),
        ];
        assert_eq!(
            municipio_desde_texto(Some("San Juan"), Some("Cesar"), &varios),
            Some("20710".into()),
            "con departamento hay un único candidato"
        );
        assert_eq!(
            municipio_desde_texto(Some("San Juan"), Some("Cesár"), &varios),
            Some("20710".into()),
            "el departamento también se pliega"
        );
        assert_eq!(
            municipio_desde_texto(Some("San Juan"), Some("Nariño"), &varios),
            None,
            "ningún San Juan en Nariño → no se inventa"
        );
    }

    /// La coincidencia EXACTA manda sobre la subcadena: «Santa Rosa» no se
    /// resuelve a «Santa Rosa de Osos», pero si el único exacto está entre las
    /// subcadenas, ese gana.
    #[test]
    fn el_nombre_exacto_manda_sobre_la_subcadena() {
        let candidatos = [
            c("05686", "Santa Rosa de Osos", "Antioquia"),
            c("66682", "Santa Rosa de Cabal", "Risaralda"),
            c("13683", "Santa Rosa del Sur", "Bolívar"),
        ];
        // «Santa Rosa» no coincide exactamente con ninguno y hay tres → None.
        assert_eq!(municipio_desde_texto(Some("Santa Rosa"), None, &candidatos), None);

        // Con el exacto presente entre las subcadenas, ese único exacto gana.
        let con_exacto = [
            c("13683", "Santa Rosa", "Bolívar"),
            c("05686", "Santa Rosa de Osos", "Antioquia"),
        ];
        assert_eq!(
            municipio_desde_texto(Some("Santa Rosa"), None, &con_exacto),
            Some("13683".into()),
            "el exacto único manda aunque haya una subcadena más larga"
        );
    }

    #[test]
    fn sin_ciudad_o_sin_candidatos_no_deriva() {
        let uno = [c("05001", "Medellín", "Antioquia")];
        assert_eq!(municipio_desde_texto(None, None, &uno), None);
        assert_eq!(municipio_desde_texto(Some("   "), None, &uno), None, "en blanco es como ausente");
        assert_eq!(municipio_desde_texto(Some("Medellín"), None, &[]), None, "nadie casó → None");
    }

    /// Se le pasa el CATÁLOGO entero (con ruido) y la función filtra por
    /// subcadena ella misma, plegando: «Medellin» sin tilde encuentra «Medellín»
    /// entre municipios que no tienen nada que ver. Su pareja: un texto que no es
    /// subcadena de ninguno → None, no el primero de la lista.
    #[test]
    fn filtra_el_catalogo_por_subcadena_plegada() {
        let catalogo = [
            c("05001", "Medellín", "Antioquia"),
            c("11001", "Bogotá, D.C.", "Bogotá D.C."),
            c("76001", "Cali", "Valle del Cauca"),
        ];
        assert_eq!(
            municipio_desde_texto(Some("Medellin"), None, &catalogo),
            Some("05001".into()),
            "sin tilde y con ruido alrededor, la subcadena la caza igual"
        );
        // «Bogota» (sin tilde ni «, D.C.») es subcadena de «bogota, d.c.» → único.
        assert_eq!(
            municipio_desde_texto(Some("Bogota"), None, &catalogo),
            Some("11001".into()),
            "subcadena única aunque el nombre guardado traiga más cola"
        );
        assert_eq!(
            municipio_desde_texto(Some("Leticia"), None, &catalogo),
            None,
            "no es subcadena de ninguno → no se inventa el primero"
        );
    }

    #[test]
    fn plegar_iguala_tildes_y_caja() {
        assert_eq!(plegar("Medellín"), plegar("MEDELLIN"));
        assert_eq!(plegar("  Bogotá  D.C. "), "bogota d.c.");
        assert_eq!(plegar("Nariño"), "narino");
        assert_ne!(plegar("Cali"), plegar("Calima"), "plegar no borra letras");
    }
}
