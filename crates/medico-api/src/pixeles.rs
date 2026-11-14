//! Render de PÍXELES DICOM a PNG — el «módulo de píxeles».
//!
//! medico ya recibe, cifra, guarda y DESCARGA archivos DICOM (`tipos::tipo_real`
//! casa la magia `DICM`). Lo que no hacía era MOSTRARLOS: el navegador no pinta un
//! DICOM, así que una placa llegaba solo como «descargar». Este módulo decodifica
//! el pixel data del primer fotograma a un PNG —en escala de grises o color— que el
//! navegador sí pinta, aplicando el *windowing* VOI/LUT que el propio DICOM declara
//! (sin él, un CT o una RM se ven casi negros).
//!
//! # El archivo lo sube un TERCERO: los bytes NO son de fiar
//!
//! El pixel data de una placa lo manda imagenología por un `POST`. Decodificar bytes
//! no confiables con un parser complejo es superficie de ataque —la lección del
//! validador de contenido XBRL de informes—. Dos defensas, y las dos son del diseño:
//!
//! 1. **Tope ANTES de asignar** (`Limites::max_muestras`): las dimensiones se leen
//!    del ENCABEZADO (`Rows`/`Columns`/`NumberOfFrames`/`SamplesPerPixel`) y se
//!    rechaza lo que desbordaría memoria ANTES de `decode_pixel_data`, que es quien
//!    expande un pixel data comprimido (RLE/JPEG) a su tamaño real. Comprobar el
//!    tamaño del buffer YA decodificado llegaría tarde: la bomba ya habría asignado.
//! 2. **Fallo RUIDOSO, nunca una imagen inventada** (directiva 20): un códec no
//!    soportado (JPEG 2000, JPEG-LS), un DICOM corrupto, o uno sin pixel data
//!    devuelven `Err`, y el llamante DEGRADA a «descargar» el original — jamás se
//!    sirve un PNG en blanco como si fuera la placa.
//!
//! # Lo que soporta y lo que no (honesto, D20)
//!
//! Solo se previsualiza lo cuya salida FIJA el encabezado (ALLOWLIST de transfer
//! syntax, `transfer_syntax_acotable`, comprobado sobre el file-meta ANTES de parsear):
//! sin comprimir (Implicit/Explicit VR LE/BE) y RLE lossless —todos producen
//! exactamente `Rows×Columns`, que es lo que el tope anti-bomba acota—. **Se DESCARGA
//! todo lo demás, a propósito**: la familia JPEG (dimensiones propias en el SOF, que
//! `jpeg-decoder` asigna sin cota), el **Deflated Explicit VR LE** (infla el dataset
//! entero al parsear — bomba de descompresión) y cualquier otra. Son la misma clase de
//! bomba que la de `BitsStored`, y re-habilitar JPEG con una cota del SOF es tarea
//! aparte. Todo lo no renderizable → `NoRenderizable`, y el original se descarga.
//!
//! Dos espacios de color, y la frontera es DELIBERADA:
//!
//! - **Escala de grises** (MONOCHROME1/2) —el grueso del diagnóstico: radiografía,
//!   TAC, RM, ecografía en gris—, de 8 **y 16 bits**, con el windowing aplicado.
//! - **RGB verdadero** (3 canales, photometric `RGB`): las muestras ya salen
//!   `R,G,B,R,G,B…` de `to_vec_frame_with_options` y se empaquetan en un PNG a color
//!   (eco Doppler, endoscopia, captura secundaria a color).
//!
//! **YBR_* y paleta devuelven `NoRenderizable` y se descargan**, a propósito: sin la
//! feature `image` (que arrastra `paste` no mantenido + licencias extra, por eso NO
//! se activa) `to_vec_frame_with_options` **no convierte el espacio de color** —copia
//! las muestras tal cual—, así que pintar un YBR como RGB daría un **color falso**.
//! Entre un color equivocado y «descargar», se descarga (D20). Convertir YBR a RGB a
//! mano sobre bytes no confiables es trabajo aparte, con su caso rojo. Es una vista
//! PREVIA para leer la placa en el navegador, no el archivo clínico —ese sigue siendo
//! el DICOM original, cifrado e intacto—.
//!
//! El windowing (solo gris) lo hace `dicom-pixeldata`: se intenta el **VOI window que
//! el DICOM declara** (lo que fija el radiólogo) y, si no lo trae, se cae a
//! **normalización min–max** (auto-contraste) —sin ese fallback, una placa de 16 bits
//! sin window no cabía en `u8` y se quedaba en «descargar»—. Aquí solo se invierte
//! MONOCHROME1 y se empaqueta el PNG. El render es CPU-bound y síncrono; el manejador
//! axum lo corre en `spawn_blocking`.
//!
//! # El tope anti-bomba cubre los DOS drivers de asignación
//!
//! Los bytes los sube un tercero, así que antes de decodificar se acota (1) la
//! GEOMETRÍA —`Rows·Columns·Frames·SamplesPerPixel` ≤ `max_muestras`, el buffer de
//! salida— y (2) `BitsAllocated`/`BitsStored` —que fijan una LUT de `1 << BitsStored`
//! en el camino de 16 bits; sin acotarlos, un DICOM de 1×1 con `BitsStored=30` forzaba
//! ~1 GiB (concilio 2026-11-03)—. Los dos topes van ANTES de `decode_pixel_data`.

use std::io::Cursor;

use dicom_core::Tag;
use dicom_object::{from_reader, FileMetaTable};
use dicom_pixeldata::{ConvertOptions, PhotometricInterpretation, PixelDecoder, VoiLutOption};

// Tags del grupo 0028 (Image Pixel) que fijan el tamaño del buffer decodificado.
// Se escriben a mano (no se trae `dicom-dictionary-std`) porque son cuatro y
// estables desde PS3.3: Rows, Columns, NumberOfFrames, SamplesPerPixel.
const ROWS: Tag = Tag(0x0028, 0x0010);
const COLUMNS: Tag = Tag(0x0028, 0x0011);
const NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
const SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
// BitsAllocated/BitsStored: NO fijan el buffer de salida, pero SÍ una tabla LUT de
// `1 << BitsStored` entradas que `dicom-pixeldata` construye al aplicar el windowing
// de 16 bits — el segundo *driver* de asignación, invisible al tope de muestras.
const BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const BITS_STORED: Tag = Tag(0x0028, 0x0101);

/// ¿La Transfer Syntax decodifica a un buffer cuyo tamaño FIJA el encabezado?
///
/// El tope anti-bomba mide `Rows×Columns×…`; eso acota la asignación SOLO cuando la
/// salida la fija el encabezado. Lo cumplen las NO comprimidas (Implicit/Explicit VR
/// LE/BE) y RLE lossless (descomprime a exactamente `Rows×Columns`, con `guarded_alloc`).
/// Es un ALLOWLIST (D23: se enumera lo provablemente acotado y el resto va a «no se
/// puede previsualizar»), porque las peligrosas no se adivinan de una en una:
///  - familia JPEG (`…1.2.4.*`): dimensiones propias en el SOF, `jpeg-decoder` asigna
///    sin cota (máximo por defecto `usize::MAX`);
///  - Deflated Explicit VR LE (`…1.2.1.99`): infla el DATASET ENTERO al parsear, sin
///    relación con `Rows×Columns` — bomba de descompresión ANTES incluso de los topes;
///  - cualquier otra nueva (JPIP, MPEG…).
///
/// Se comprueba sobre el FILE-META (siempre sin comprimir) ANTES de `from_reader`, para
/// que un Deflated ni se llegue a inflar (concilio 2026-11-03, sonnet/fable/haiku).
/// La comparación es por IGUALDAD, no por prefijo: Deflated (`…1.2.1.99`) comparte el
/// prefijo de Explicit VR LE (`…1.2.1`) y un `starts_with` lo dejaría colar.
fn transfer_syntax_acotable(ts: &str) -> bool {
    matches!(
        ts,
        "1.2.840.10008.1.2"          // Implicit VR LE
            | "1.2.840.10008.1.2.1"  // Explicit VR LE
            | "1.2.840.10008.1.2.2"  // Explicit VR BE
            | "1.2.840.10008.1.2.5" // RLE Lossless
    )
}

/// Topes para decodificar un DICOM no confiable.
#[derive(Debug, Clone, Copy)]
pub struct Limites {
    /// Máximo de MUESTRAS de píxel —filas × columnas × fotogramas × muestras por
    /// píxel— que se acepta decodificar. Por encima: `DemasiadoGrande`, sin asignar.
    pub max_muestras: u64,
}

impl Default for Limites {
    fn default() -> Self {
        // 96 millones de muestras: cubre una placa CR grande (≈ 4000×5000 = 20 Mpx)
        // y varias de una serie, sin dejar que un encabezado declare gigabytes. A 2
        // bytes por muestra son ~192 MB de buffer decodificado, el techo duro.
        Self { max_muestras: 96_000_000 }
    }
}

/// Por qué no se pudo producir la vista previa. TODAS significan lo mismo para el
/// llamante —degradar a «descargar»—; se distinguen para el log, no para el flujo
/// (nunca se inventa una imagen, directiva 20).
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Los bytes no son un DICOM legible (encabezado o estructura inválidos).
    NoLegible,
    /// El pixel data usa un códec no soportado en Rust puro (JPEG 2000, JPEG-LS),
    /// está corrupto, o el objeto no trae pixel data.
    NoRenderizable,
    /// El tamaño DECLARADO en el encabezado excede el tope, antes de asignar.
    DemasiadoGrande { muestras: u64, tope: u64 },
    /// La codificación del PNG de salida falló, o el buffer decodificado no tuvo el
    /// tamaño esperado para las dimensiones del encabezado.
    Codificacion,
}

/// Lee un entero del encabezado por su `Tag`; `por_defecto` si el elemento falta
/// (p. ej. `NumberOfFrames`, ausente en una imagen de un solo fotograma).
fn entero(
    obj: &dicom_object::DefaultDicomObject,
    tag: Tag,
    por_defecto: u64,
) -> Result<u64, Error> {
    match obj.element_opt(tag) {
        Ok(Some(e)) => e.to_int::<u64>().map_err(|_| Error::NoLegible),
        Ok(None) => Ok(por_defecto),
        Err(_) => Err(Error::NoLegible),
    }
}

/// Decodifica el PRIMER fotograma de un DICOM a PNG. Pura: bytes → bytes, sin red
/// ni disco. `Err` ⇒ no hay vista previa y el llamante descarga el original.
///
/// # Errores
/// Ver [`Error`]: no legible, no renderizable (códec/corrupto), demasiado grande,
/// o fallo al codificar el PNG.
pub fn previa_png(dicom: &[u8], lim: Limites) -> Result<Vec<u8>, Error> {
    // PRIMER tope, ANTES de parsear el dataset: se lee SOLO el file-meta (que va
    // siempre SIN comprimir) y se exige una transfer syntax cuya salida fije el
    // encabezado. Esto corta la familia JPEG (SOF sin cota) y —lo que `from_reader`
    // haría ANTES de cualquier otro tope— el Deflated Explicit VR LE, que infla el
    // DATASET ENTERO al parsear: una bomba de descompresión. `previa_png` es el ÚNICO
    // sitio de medico que parsea un DICOM subido por un tercero, así que este tope
    // nace aquí. El meta empieza en «DICM»; se salta el preámbulo de 128 bytes (Part-10).
    let tras_preambulo = if dicom.len() >= 132 && &dicom[128..132] == b"DICM" {
        &dicom[128..]
    } else {
        dicom
    };
    let meta = FileMetaTable::from_reader(Cursor::new(tras_preambulo)).map_err(|_| Error::NoLegible)?;
    if !transfer_syntax_acotable(meta.transfer_syntax()) {
        return Err(Error::NoRenderizable);
    }

    // Solo ahora se parsea el dataset: la transfer syntax ya está en el allowlist, así
    // que `from_reader` no inflará una bomba Deflated ni abrirá un códec de SOF libre.
    let obj = from_reader(Cursor::new(dicom)).map_err(|_| Error::NoLegible)?;

    // Tope ANTES de decodificar: dimensiones del encabezado, no del buffer expandido.
    let filas = entero(&obj, ROWS, 0)?;
    let columnas = entero(&obj, COLUMNS, 0)?;
    let fotogramas = entero(&obj, NUMBER_OF_FRAMES, 1)?.max(1);
    let muestras_pp = entero(&obj, SAMPLES_PER_PIXEL, 1)?.max(1);
    let muestras = filas
        .saturating_mul(columnas)
        .saturating_mul(fotogramas)
        .saturating_mul(muestras_pp);
    if muestras == 0 {
        // Sin dimensiones no hay imagen que mostrar: se descarga (D20).
        return Err(Error::NoRenderizable);
    }
    if muestras > lim.max_muestras {
        return Err(Error::DemasiadoGrande { muestras, tope: lim.max_muestras });
    }

    // SEGUNDO tope, sobre el OTRO driver de asignación (concilio 2026-11-03,
    // sonnet+fable lo midieron por ejecución): el camino de 16 bits de
    // `dicom-pixeldata` construye una LUT de `1 << BitsStored` entradas para el
    // windowing. `BitsStored` es un u16 del encabezado que el atacante controla y que
    // NADIE cruzaba contra `BitsAllocated`: un DICOM de 1×1 (que pasa el tope de
    // muestras con `muestras=1`) y `BitsStored=30` fuerza una tabla de ~1 GiB, y
    // `BitsStored` 0 o >32 un `panic` dentro de la dependencia. El tope de geometría de
    // arriba NO lo ve. Se acota AQUÍ, antes de decodificar: el estándar DICOM exige
    // `BitsStored ≤ BitsAllocated`, y para el gris/RGB de 8 o 16 bits que sabemos
    // mostrar, `BitsAllocated ∈ {8, 16}` (LUT ≤ 2¹⁶ = 64 Ki). Fuera de ese rango no es
    // una placa que vayamos a previsualizar → se descarga (D20). Rechazar antes de
    // `decode_pixel_data` garantiza que la LUT gigante ni se intenta construir.
    let bits_alloc = entero(&obj, BITS_ALLOCATED, 0)?;
    let bits_stored = entero(&obj, BITS_STORED, 0)?;
    if !matches!(bits_alloc, 8 | 16) || bits_stored == 0 || bits_stored > bits_alloc {
        return Err(Error::NoRenderizable);
    }

    // Solo AQUÍ se decodifica (y se asigna el buffer), ya acotado.
    let pixeles = obj.decode_pixel_data().map_err(|_| Error::NoRenderizable)?;

    // Qué sabemos empaquetar SIN convertir el espacio de color a mano ni la feature
    // `image` (que arrastra `paste` no mantenido): gris (1 canal) y RGB verdadero (3
    // canales con photometric RGB — entonces `to_vec_frame_with_options` ya entrega
    // `R,G,B,R,G,B…`). YBR_* y paleta caen al `else` y se descargan (D20): copiarlas
    // como si fueran RGB daría color falso. Es la frontera deliberada, no un olvido.
    let spp = pixeles.samples_per_pixel();
    let pi = pixeles.photometric_interpretation();
    let (color_png, canales): (png::ColorType, usize) = if spp == 1 && pi.is_monochrome() {
        (png::ColorType::Grayscale, 1)
    } else if spp == 3 && matches!(pi, PhotometricInterpretation::Rgb) {
        (png::ColorType::Rgb, 3)
    } else {
        return Err(Error::NoRenderizable);
    };
    let ancho = pixeles.columns();
    let alto = pixeles.rows();

    // La obtención de muestras de 8 bits difiere por espacio de color, y hay una trampa
    // medida: `force_8bit()` es NO-OP en `to_vec_frame_with_options` (la función ignora
    // `bit_depth`), así que pedir `::<u8>` a una placa de 16 bits SIEMPRE falla —la LUT
    // normaliza al rango de 16 bits y no cabe en `u8`—. Por eso TAC/RM nunca mostraban
    // miniatura (concilio 2026-11-03). La salida robusta:
    //
    // - **GRIS**: pedir las muestras como `u16` (que sí caben) aplicando el windowing que
    //   el DICOM declara (`First`); si no trae window, la propia dependencia cae a
    //   normalización. Luego bajamos a `u8` NOSOTROS con auto-contraste min–max —una sola
    //   vía para 8 y 16 bits, bajo nuestro control, sin depender de `force_8bit`—. Es una
    //   vista PREVIA, no el diagnóstico, así que el auto-contraste es correcto.
    // - **RGB**: las muestras ya vienen `R,G,B` en 8 bits; se copian sin normalizar
    //   (normalizar con un min–max GLOBAL mezclaría los canales y falsearía el color).
    //   El VOI no aplica al color. Un RGB de 16 bits no cabe en `u8` → `NoRenderizable`.
    // Fotograma 0.
    let mut muestras: Vec<u8> = if canales == 1 {
        let gris16 = |voi| {
            pixeles.to_vec_frame_with_options::<u16>(0, &ConvertOptions::new().with_voi_lut(voi))
        };
        let crudo: Vec<u16> = gris16(VoiLutOption::First)
            .or_else(|_| gris16(VoiLutOption::Normalize))
            .map_err(|_| Error::NoRenderizable)?;
        let lo = crudo.iter().copied().min().unwrap_or(0);
        let hi = crudo.iter().copied().max().unwrap_or(0);
        let rango = u32::from(hi - lo); // hi ≥ lo por construcción
        if rango == 0 {
            vec![0u8; crudo.len()] // placa plana: todo negro, sin dividir por cero
        } else {
            crudo
                .iter()
                .map(|&v| ((u32::from(v - lo) * 255) / rango) as u8)
                .collect()
        }
    } else {
        pixeles
            .to_vec_frame_with_options::<u8>(0, &ConvertOptions::new())
            .map_err(|_| Error::NoRenderizable)?
    };

    // MONOCHROME1: el valor ALTO es negro (lo contrario del PNG en grises, donde
    // alto = blanco). `to_vec` «ignora el fotométrico» a propósito, así que la
    // inversión la hacemos aquí para no servir una placa en negativo. Solo gris: un
    // RGB nunca es MONOCHROME1.
    if matches!(pi, PhotometricInterpretation::Monochrome1) {
        for b in &mut muestras {
            *b = 255 - *b;
        }
    }

    // Defensa: el buffer tiene que casar filas×columnas×canales (1 gris, 3 RGB).
    let esperado = (ancho as usize)
        .saturating_mul(alto as usize)
        .saturating_mul(canales);
    if muestras.len() != esperado {
        return Err(Error::Codificacion);
    }

    let mut salida = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut salida, ancho, alto);
        enc.set_color(color_png);
        enc.set_depth(png::BitDepth::Eight);
        let mut escritor = enc.write_header().map_err(|_| Error::Codificacion)?;
        escritor
            .write_image_data(&muestras)
            .map_err(|_| Error::Codificacion)?;
    }
    Ok(salida)
}

/// Un DICOM MONOCHROME2 de 8 bits, sin comprimir (Explicit VR LE), de
/// `filas`×`columnas`. SINTÉTICO: ni un byte de PHI, y se construye en memoria
/// (sin fixture descargado — offline, directiva 28 sobre el formato real). Lo usan
/// las pruebas de este módulo y las del endpoint `/previa` en `rutas`.
#[cfg(test)]
pub(crate) fn dicom_sintetico(filas: u16, columnas: u16, pixeles: Vec<u8>) -> Vec<u8> {
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

    let us = |t: Tag, v: u16| DataElement::new(t, VR::US, PrimitiveValue::from(v));
    let mut obj = InMemDicomObject::new_empty();
    obj.put(us(SAMPLES_PER_PIXEL, 1));
    obj.put(DataElement::new(
        Tag(0x0028, 0x0004),
        VR::CS,
        PrimitiveValue::from("MONOCHROME2"),
    ));
    obj.put(us(ROWS, filas));
    obj.put(us(COLUMNS, columnas));
    obj.put(us(Tag(0x0028, 0x0100), 8)); // BitsAllocated
    obj.put(us(Tag(0x0028, 0x0101), 8)); // BitsStored
    obj.put(us(Tag(0x0028, 0x0102), 7)); // HighBit
    obj.put(us(Tag(0x0028, 0x0103), 0)); // PixelRepresentation (unsigned)
    obj.put(DataElement::new(
        Tag(0x7FE0, 0x0010),
        VR::OB,
        PrimitiveValue::from(pixeles),
    ));
    let archivo = obj
        .with_meta(
            FileMetaTableBuilder::new()
                // Secondary Capture Image Storage — un SOP válido cualquiera.
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                .media_storage_sop_instance_uid("1.2.3.4.5.6.7.8")
                // Explicit VR Little Endian, sin comprimir.
                .transfer_syntax("1.2.840.10008.1.2.1"),
        )
        .expect("construir meta");
    let mut bytes = Vec::new();
    archivo.write_all(&mut bytes).expect("serializar DICOM");
    bytes
}

/// El MISMO DICOM 8-bit de [`dicom_sintetico`] pero serializado en **Deflated Explicit
/// VR LE** (`1.2.840.10008.1.2.1.99`): un archivo VÁLIDO que `from_reader` inflaría y
/// renderizaría SIN el gate. Sirve de caso rojo de CABLEADO (D33): prueba que
/// `previa_png` rechaza por la transfer syntax, no solo que el predicado lo haría.
/// (La feature `deflate` está activa en el build, así que `write_all` sí deflacta.)
#[cfg(test)]
fn dicom_deflated(filas: u16, columnas: u16, pixeles: Vec<u8>) -> Vec<u8> {
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

    let us = |t: Tag, v: u16| DataElement::new(t, VR::US, PrimitiveValue::from(v));
    let mut obj = InMemDicomObject::new_empty();
    obj.put(us(SAMPLES_PER_PIXEL, 1));
    obj.put(DataElement::new(
        Tag(0x0028, 0x0004),
        VR::CS,
        PrimitiveValue::from("MONOCHROME2"),
    ));
    obj.put(us(ROWS, filas));
    obj.put(us(COLUMNS, columnas));
    obj.put(us(BITS_ALLOCATED, 8));
    obj.put(us(BITS_STORED, 8));
    obj.put(us(Tag(0x0028, 0x0102), 7));
    obj.put(us(Tag(0x0028, 0x0103), 0));
    obj.put(DataElement::new(
        Tag(0x7FE0, 0x0010),
        VR::OB,
        PrimitiveValue::from(pixeles),
    ));
    let archivo = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                .media_storage_sop_instance_uid("1.2.3.4.5.6.7.8")
                .transfer_syntax("1.2.840.10008.1.2.1.99"), // Deflated Explicit VR LE
        )
        .expect("construir meta");
    let mut bytes = Vec::new();
    archivo.write_all(&mut bytes).expect("serializar DICOM deflated");
    bytes
}

/// Un DICOM MONOCHROME2 con `bits_allocated`/`bits_stored` ARBITRARIOS (incluidos los
/// hostiles: `bits_stored > bits_allocated`, 0, o >32), con `RescaleSlope=0`. Sirve
/// para el caso rojo de la bomba de LUT.
///
/// `RescaleSlope=0` es EL TRUCO que hace de esto una bomba de verdad (D33, la vía real):
/// con el rescale identidad (slope=1) la LUT se cortocircuita en el índice 256
/// (`T::from(256.0)` no cabe en `u8`) y `to_vec` falla pronto → `NoRenderizable` AUNQUE
/// no esté el gate, de modo que el control pasaría con una regla ciega. Con slope=0 todo
/// índice mapea a 0 (`T::from(0.0)=Some(0)`), el `collect` NO se corta y asigna las
/// `2^BitsStored` entradas enteras — que es la asignación que el gate debe impedir.
#[cfg(test)]
fn dicom_sintetico_bits(
    filas: u16,
    columnas: u16,
    pixeles: Vec<u8>,
    bits_allocated: u16,
    bits_stored: u16,
) -> Vec<u8> {
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

    let us = |t: Tag, v: u16| DataElement::new(t, VR::US, PrimitiveValue::from(v));
    let mut obj = InMemDicomObject::new_empty();
    obj.put(us(SAMPLES_PER_PIXEL, 1));
    obj.put(DataElement::new(
        Tag(0x0028, 0x0004),
        VR::CS,
        PrimitiveValue::from("MONOCHROME2"),
    ));
    obj.put(us(ROWS, filas));
    obj.put(us(COLUMNS, columnas));
    obj.put(us(BITS_ALLOCATED, bits_allocated));
    obj.put(us(BITS_STORED, bits_stored));
    obj.put(us(Tag(0x0028, 0x0102), bits_stored.saturating_sub(1))); // HighBit
    obj.put(us(Tag(0x0028, 0x0103), 0)); // PixelRepresentation (unsigned)
    // El truco de la bomba: slope=0 → la LUT no se cortocircuita y se asigna entera.
    obj.put(DataElement::new(Tag(0x0028, 0x1052), VR::DS, PrimitiveValue::from("0"))); // Intercept
    obj.put(DataElement::new(Tag(0x0028, 0x1053), VR::DS, PrimitiveValue::from("0"))); // Slope
    obj.put(DataElement::new(
        Tag(0x7FE0, 0x0010),
        VR::OB,
        PrimitiveValue::from(pixeles),
    ));
    let archivo = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                .media_storage_sop_instance_uid("1.2.3.4.5.6.7.8")
                .transfer_syntax("1.2.840.10008.1.2.1"),
        )
        .expect("construir meta");
    let mut bytes = Vec::new();
    archivo.write_all(&mut bytes).expect("serializar DICOM");
    bytes
}

/// Un DICOM de 3 canales, 8 bits, sin comprimir (Explicit VR LE), de
/// `filas`×`columnas` con el `photometric` que se le pida (`"RGB"`, `"YBR_FULL"`…).
/// `pixeles` son las muestras intercaladas, longitud `filas*columnas*3`. SINTÉTICO,
/// sin PHI. Sirve para la pareja aguja (RGB renderiza) / control (YBR se descarga).
#[cfg(test)]
fn dicom_sintetico_color(filas: u16, columnas: u16, pixeles: Vec<u8>, photometric: &str) -> Vec<u8> {
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

    let us = |t: Tag, v: u16| DataElement::new(t, VR::US, PrimitiveValue::from(v));
    let mut obj = InMemDicomObject::new_empty();
    obj.put(us(SAMPLES_PER_PIXEL, 3));
    obj.put(DataElement::new(
        Tag(0x0028, 0x0004),
        VR::CS,
        PrimitiveValue::from(photometric),
    ));
    obj.put(us(Tag(0x0028, 0x0006), 0)); // PlanarConfiguration (0 = intercalado R,G,B)
    obj.put(us(ROWS, filas));
    obj.put(us(COLUMNS, columnas));
    obj.put(us(Tag(0x0028, 0x0100), 8)); // BitsAllocated
    obj.put(us(Tag(0x0028, 0x0101), 8)); // BitsStored
    obj.put(us(Tag(0x0028, 0x0102), 7)); // HighBit
    obj.put(us(Tag(0x0028, 0x0103), 0)); // PixelRepresentation (unsigned)
    obj.put(DataElement::new(
        Tag(0x7FE0, 0x0010),
        VR::OB,
        PrimitiveValue::from(pixeles),
    ));
    let archivo = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                .media_storage_sop_instance_uid("1.2.3.4.5.6.7.8")
                .transfer_syntax("1.2.840.10008.1.2.1"),
        )
        .expect("construir meta");
    let mut bytes = Vec::new();
    archivo.write_all(&mut bytes).expect("serializar DICOM");
    bytes
}

/// Un DICOM MONOCHROME2 de 16 bits LEGÍTIMO (BitsAllocated=16, BitsStored=16, SIN
/// rescale → identidad), con `muestras` reales (valores que pasan de 255). Es el caso
/// del grueso del diagnóstico (TAC/RM), y sirve de aguja para que el 16 bits SÍ se vea.
#[cfg(test)]
fn dicom_sintetico_16bit(filas: u16, columnas: u16, muestras: Vec<u16>) -> Vec<u8> {
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

    let us = |t: Tag, v: u16| DataElement::new(t, VR::US, PrimitiveValue::from(v));
    let mut obj = InMemDicomObject::new_empty();
    obj.put(us(SAMPLES_PER_PIXEL, 1));
    obj.put(DataElement::new(
        Tag(0x0028, 0x0004),
        VR::CS,
        PrimitiveValue::from("MONOCHROME2"),
    ));
    obj.put(us(ROWS, filas));
    obj.put(us(COLUMNS, columnas));
    obj.put(us(BITS_ALLOCATED, 16));
    obj.put(us(BITS_STORED, 16));
    obj.put(us(Tag(0x0028, 0x0102), 15)); // HighBit
    obj.put(us(Tag(0x0028, 0x0103), 0)); // PixelRepresentation (unsigned)
    // Pixel data: muestras u16 en little-endian (OW).
    let mut bytes_px = Vec::with_capacity(muestras.len() * 2);
    for m in muestras {
        bytes_px.extend_from_slice(&m.to_le_bytes());
    }
    obj.put(DataElement::new(
        Tag(0x7FE0, 0x0010),
        VR::OW,
        PrimitiveValue::from(bytes_px),
    ));
    let archivo = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                .media_storage_sop_instance_uid("1.2.3.4.5.6.7.8")
                .transfer_syntax("1.2.840.10008.1.2.1"),
        )
        .expect("construir meta");
    let mut bytes = Vec::new();
    archivo.write_all(&mut bytes).expect("serializar DICOM");
    bytes
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn degradado(filas: u16, columnas: u16) -> Vec<u8> {
        let n = usize::from(filas) * usize::from(columnas);
        (0..n).map(|i| (i % 256) as u8).collect()
    }

    // AGUJA: una placa DICOM real se decodifica a un PNG de verdad.
    #[test]
    fn una_placa_dicom_se_renderiza_a_png() {
        let dcm = dicom_sintetico(4, 4, degradado(4, 4));
        let png = previa_png(&dcm, Limites::default()).expect("debería renderizar");
        // La firma PNG: lo que salió es una imagen que el navegador pinta.
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "no empieza por la firma PNG");
    }

    // CONTROL (D33) — y la pieza que ENDURECE el borde: la magia `DICM` sola (lo
    // único que `tipos::tipo_real` exige para ACEPTAR el archivo) NO basta para
    // renderizar. Un sello `DICM` sobre basura devuelve `NoLegible`: nunca se sirve
    // un PNG inventado por un archivo que solo finge ser DICOM.
    #[test]
    fn un_sello_dicm_sobre_basura_no_se_renderiza() {
        assert_eq!(previa_png(b"no soy un dicom", Limites::default()), Err(Error::NoLegible));
        let mut falso = vec![0u8; 128];
        falso.extend_from_slice(b"DICM");
        falso.extend_from_slice(b"basura que no es un encabezado");
        assert_eq!(previa_png(&falso, Limites::default()), Err(Error::NoLegible));
    }

    // AGUJA/CONTROL del TOPE (D33), mismo artefacto, solo cambia el tope: el mismo
    // DICOM renderiza con el tope normal y se RECHAZA con uno por debajo de sus
    // muestras. Prueba que es el tope quien decide, no otra cosa.
    #[test]
    fn el_tope_de_muestras_discrimina() {
        let dcm = dicom_sintetico(4, 4, degradado(4, 4)); // 16 muestras
        assert!(previa_png(&dcm, Limites::default()).is_ok());
        let r = previa_png(&dcm, Limites { max_muestras: 8 });
        assert!(
            matches!(r, Err(Error::DemasiadoGrande { muestras: 16, tope: 8 })),
            "esperaba DemasiadoGrande 16/8, fue {r:?}"
        );
    }

    // La bomba de dimensiones se para ANTES de decodificar: un encabezado que
    // declara 50000×50000 (2.5e9 muestras) con pixel data minúsculo se rechaza por
    // el tope, sin llegar a `decode_pixel_data` (que es quien asignaría al expandir
    // un códec comprimido). MUTANTE = quitar el tope: llegaría a decode.
    #[test]
    fn un_encabezado_gigante_se_rechaza_antes_de_asignar() {
        let dcm = dicom_sintetico(50000, 50000, vec![0u8; 16]);
        match previa_png(&dcm, Limites::default()) {
            Err(Error::DemasiadoGrande { muestras, tope }) => {
                assert_eq!(tope, 96_000_000);
                assert!(muestras > tope, "muestras={muestras}");
            }
            otro => panic!("esperaba DemasiadoGrande, fue {otro:?}"),
        }
    }

    // AGUJA color: una placa RGB se renderiza a un PNG A COLOR de verdad. Se cruza la
    // frontera (D28): se DECODIFICA lo que se produjo y se comprueba que el consumidor
    // lo lee como RGB 2×2 —un gris saldría `Grayscale`, así la prueba no pasaría con la
    // regla vieja de solo-gris—.
    #[test]
    fn una_placa_rgb_se_renderiza_a_png_a_color() {
        let pixeles = vec![
            255, 0, 0, // rojo
            0, 255, 0, // verde
            0, 0, 255, // azul
            255, 255, 0, // amarillo
        ];
        let dcm = dicom_sintetico_color(2, 2, pixeles, "RGB");
        let png = previa_png(&dcm, Limites::default()).expect("un RGB debería renderizar");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "no es un PNG");
        let lector = png::Decoder::new(&png[..]).read_info().expect("PNG ilegible");
        let info = lector.info();
        assert_eq!(info.color_type, png::ColorType::Rgb, "no salió RGB");
        assert_eq!((info.width, info.height), (2, 2));
    }

    // CONTROL (D33) que DISCRIMINA la frontera de color: un DICOM de 3 canales pero
    // YBR_FULL NO se renderiza (se descarga), porque sin convertir el espacio de color
    // pintarlo como RGB daría color falso. Mismo `spp=3` que la aguja de arriba: lo
    // que cambia el veredicto es el PHOTOMETRIC, no el número de canales — un gate que
    // solo mirara `spp == 3` serviría esta placa con color inventado.
    #[test]
    fn una_placa_ybr_no_se_renderiza_y_se_descarga() {
        let dcm = dicom_sintetico_color(2, 2, vec![128u8; 2 * 2 * 3], "YBR_FULL");
        assert_eq!(previa_png(&dcm, Limites::default()), Err(Error::NoRenderizable));
    }

    // PAREJA D33 del allowlist de transfer syntax: solo se previsualiza lo cuya salida
    // FIJA el encabezado (no comprimido + RLE); todo lo demás se descarga.
    #[test]
    fn solo_las_transfer_syntax_acotadas_se_previsualizan() {
        // ACOTADAS → se previsualizan.
        for ts in [
            "1.2.840.10008.1.2",   // Implicit VR LE
            "1.2.840.10008.1.2.1", // Explicit VR LE
            "1.2.840.10008.1.2.2", // Explicit VR BE
            "1.2.840.10008.1.2.5", // RLE lossless
        ] {
            assert!(transfer_syntax_acotable(ts), "{ts} debería ser acotable");
        }
        // NO acotadas → se descartan. El control DISCRIMINA dos reglas ciegas distintas:
        // «permite todo» (fallaría con cualquiera de abajo) y «permite por prefijo
        // …1.2.1» (dejaría colar el Deflated …1.2.1.99, la bomba de descompresión).
        for ts in [
            "1.2.840.10008.1.2.1.99",  // Deflated Explicit VR LE (infla el dataset entero)
            "1.2.840.10008.1.2.4.50",  // JPEG baseline
            "1.2.840.10008.1.2.4.70",  // JPEG lossless SV1
            "1.2.840.10008.1.2.4.90",  // JPEG 2000
            "1.2.840.10008.1.2.4.80",  // JPEG-LS
            "1.2.840.10008.1.2.4.100", // MPEG2
        ] {
            assert!(!transfer_syntax_acotable(ts), "{ts} NO debería previsualizarse");
        }
    }

    // CABLEADO (D33) — hueco que cazó el Paso 5 (haiku): el test de arriba prueba el
    // PREDICADO, no que `previa_png` lo USE. Aquí la vía real: un DICOM Deflated VÁLIDO
    // que, SIN el gate, `from_reader` inflaría y renderizaría `Ok` (envuelve el mismo
    // 4×4 8-bit que `dicom_sintetico`, que sí renderiza). `previa_png` debe rechazarlo
    // con `NoRenderizable` por la transfer syntax leída del meta ANTES de parsear.
    // MUTANTE = quitar el `if !transfer_syntax_acotable(...)` → renderizaría → roja.
    #[test]
    fn un_dicom_deflated_lo_rechaza_el_gate_antes_de_inflar() {
        let dcm = dicom_deflated(4, 4, degradado(4, 4));
        // Sanidad: es un Deflated VÁLIDO (from_reader lo infla y parsea), así que el
        // rechazo viene del GATE de transfer syntax, no de un parseo fallido (NoLegible).
        assert!(
            from_reader(Cursor::new(&dcm[..])).is_ok(),
            "el deflated de prueba debería ser parseable; si no, el test no discrimina"
        );
        assert_eq!(previa_png(&dcm, Limites::default()), Err(Error::NoRenderizable));
    }

    // CONTROL (D33) de la BOMBA DE LUT que cazó el concilio (sonnet+fable, medida): el
    // tope de muestras mira la geometría, pero `BitsStored` controla una tabla de
    // `1 << BitsStored` entradas. Un DICOM MINÚSCULO (1×1, `muestras=1`, pasa el tope)
    // con `BitsStored > BitsAllocated` —que DICOM prohíbe y nadie cruzaba— fuerza GiB de
    // asignación (BitsStored 30→~1 GiB, 32→~4 GiB) o un `panic` (0, >32). El gate lo
    // rechaza ANTES de `decode_pixel_data`, así que la tabla NI SE INTENTA: por eso es
    // seguro probar aquí el valor real de la bomba (32) y el del panic (33, 0) sin
    // reventar el test. `16/17` es el borde (pequeño aunque se colara) y es el que uso
    // para la comprobación de mutación: quitar el gate lo deja pasar a decodificar.
    #[test]
    fn un_bitstored_hostil_se_rechaza_antes_de_asignar() {
        let caso = |ba: u16, bs: u16| {
            previa_png(&dicom_sintetico_bits(1, 1, vec![0u8; 2], ba, bs), Limites::default())
        };
        assert_eq!(caso(16, 17), Err(Error::NoRenderizable), "BitsStored>BitsAllocated");
        assert_eq!(caso(16, 32), Err(Error::NoRenderizable), "el valor de la bomba (4 GiB)");
        assert_eq!(caso(16, 33), Err(Error::NoRenderizable), "el valor del panic (>32)");
        assert_eq!(caso(16, 0), Err(Error::NoRenderizable), "BitsStored=0 (panic)");
        assert_eq!(caso(32, 32), Err(Error::NoRenderizable), "BitsAllocated exótico (32)");
        assert_eq!(caso(16, 65535), Err(Error::NoRenderizable), "BitsStored máximo u16");
        // CONTROL POSITIVO que hace que el caso rojo DISCRIMINE por la vía real (D33):
        // `16/16` el gate lo PERMITE (bits_stored ≤ bits_allocated) y —con el mismo
        // slope=0— SÍ renderiza (LUT de 2¹⁶ = 64 Ki, construida entera y mapeada a 0).
        // `16/16` y `16/17` difieren ÚNICAMENTE en la rama `bits_stored > bits_alloc`
        // del gate: que uno pase y el otro no prueba que es el GATE quien decide, no un
        // fallo lateral del decoder. Sin el gate, `16/17` también renderizaría (Ok).
        assert!(caso(16, 16).is_ok(), "el gate NO debe bloquear 16/16 legítimo");
    }

    // AGUJA (D33) del arreglo funcional: una placa de 16 bits LEGÍTIMA (TAC/RM, valores
    // > 255, rescale identidad) SÍ se renderiza. Con el VOI `Default` anterior la LUT
    // identidad desbordaba `u8` en el índice 256 y devolvía `NoRenderizable` → el grueso
    // del diagnóstico nunca mostraba miniatura; el fallback `First→Normalize` lo arregla.
    // Se cruza la frontera (D28): se decodifica el PNG y se comprueba 16×16 gris 2×2.
    #[test]
    fn una_placa_gris_de_16_bits_se_renderiza() {
        let dcm = dicom_sintetico_16bit(2, 2, vec![0u16, 1000, 30000, 65535]);
        let png = previa_png(&dcm, Limites::default()).expect("un 16 bits debería renderizar");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "no es un PNG");
        let mut lector = png::Decoder::new(&png[..]).read_info().expect("PNG ilegible");
        assert_eq!(lector.info().color_type, png::ColorType::Grayscale, "no salió gris");
        assert_eq!((lector.info().width, lector.info().height), (2, 2));
        // Normalización min–max: el valor más alto (65535) sale 255 y el más bajo (0)
        // sale 0, así que la miniatura tiene contraste real y no un rectángulo plano.
        let mut datos = vec![0u8; lector.output_buffer_size()];
        let marco = lector.next_frame(&mut datos).expect("marco");
        let px = &datos[..marco.buffer_size()];
        assert!(
            px.contains(&0) && px.contains(&255),
            "la normalización debería llevar el rango a 0..=255, fue {px:?}"
        );
    }
}
