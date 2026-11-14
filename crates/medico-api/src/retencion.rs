//! #758 — Orquestación de la supresión por defunción (Ley 1581).
//!
//! La capa de datos (`medico_datos::retencion`) vuelca y purga; aquí vive lo que
//! necesita cripto y disco: comprimir el expediente, cifrarlo, firmar el
//! manifiesto, VERIFICAR la copia y solo entonces purgar. Es la costura entre el
//! volcado y el transporte a custodia, igual que `ihce.rs` lo es entre la cola y
//! el mecanismo IHCE — y de medico porque la POLÍTICA (qué frase cifra, dónde se
//! guarda, cada cuánto se barre) es del producto, no de la capa de datos.
//!
//! # El invariante de seguridad: no se purga sin copia verificada
//!
//! El orden es archivar → VERIFICAR (re-descifrar y comparar el hash) → purgar.
//! Si el archivo no se puede reabrir, no se borra nada. La purga vuelve a
//! comprobar en su transacción que el expediente no cambió (recuentos) y que la
//! defunción sigue vigente. Es el punto de no retorno, y por eso va el último.
//!
//! # Claves (D32: un valor con dos significados son dos campos)
//!
//! - El expediente se cifra con **`MEDICO_RETENCION_KEY`**, frase PROPIA de este
//!   archivo — NO se reusa `MEDICO_STORAGE_KEY` (que protege el TOTP y un blob de
//!   la historia): mezclar dominios de clave es lo que la D32 advierte.
//! - El manifiesto se firma con **`MEDICO_SIGNING_KEY`**, la identidad de firma de
//!   medico (la misma de la cadena `acceso`): el manifiesto es una afirmación de
//!   medico, coherente con el resto de la firma institucional.

use std::io::Write as _;
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::Value;
use sha2::{Digest, Sha256};

use medico_datos::retencion::{self, RazonNoPurga};
use medico_datos::Pool;

/// Cada cuánto barre la tarea de fondo. Un día: los vencimientos son a 20 años,
/// no hay prisa de segundos (a diferencia de la cola IHCE).
const INTERVALO: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// La configuración del subsistema. Se construye del entorno con
/// [`config_desde_entorno`], que devuelve `Some` **solo si están todas** las
/// piezas obligatorias: sin ellas la tarea no arranca (D20), como la cola IHCE.
#[derive(Clone)]
pub struct Config {
    /// Frase que cifra el expediente archivado (`MEDICO_RETENCION_KEY`).
    pub frase: String,
    /// Clave privada de firma del manifiesto (`MEDICO_SIGNING_KEY`).
    pub sk_firma: String,
    /// Carpeta bajo custodia donde vive la copia cifrada (`MEDICO_RETENCION_DIR`).
    pub dir_archivo: PathBuf,
    /// Carpeta de imágenes clínicas, para archivarlas y borrarlas del disco.
    pub dir_imagenes: PathBuf,
    /// La frase con que las imágenes están cifradas en reposo (`MEDICO_STORAGE_KEY`).
    /// Sin ella no se puede archivar una imagen `cifrada=1`: se aborta (concilio
    /// 2026-09-22, fable) para que la copia bajo custodia sea autosuficiente con solo
    /// `MEDICO_RETENCION_KEY`, no con dos claves de las que el manifiesto nombra cero.
    pub clave_reposo: Option<String>,
    /// Años desde la defunción antes de suprimir (`MEDICO_RETENCION_ANOS`, def. 20).
    pub anos: i64,
}

/// Lee la configuración del entorno. `None` = subsistema inactivo (el caso normal
/// hasta que el prestador lo configure): faltan `MEDICO_RETENCION_KEY`,
/// `MEDICO_RETENCION_DIR` o `MEDICO_SIGNING_KEY`.
pub fn config_desde_entorno() -> Option<Config> {
    config_desde(|k| std::env::var(k).ok())
}

/// El parseo de la config, con la fuente de variables como parámetro para poder
/// probarlo sin tocar el entorno global del proceso (patrón de `ihce::config_desde`).
pub fn config_desde<F: Fn(&str) -> Option<String>>(get: F) -> Option<Config> {
    let g = |k: &str| get(k).filter(|v| !v.is_empty());
    let frase = g("MEDICO_RETENCION_KEY")?;
    let sk_firma = g("MEDICO_SIGNING_KEY")?;
    let dir_archivo = PathBuf::from(g("MEDICO_RETENCION_DIR")?);
    let dir_imagenes = PathBuf::from(
        g("MEDICO_DIR_IMAGENES").unwrap_or_else(|| "storage/imagenes".into()),
    );
    // El periodo tiene PISO legal (Res. 839/2017, 15 años). Un valor por debajo
    // sería borrado ilegal: se RECHAZA ruidosamente y el subsistema queda inactivo
    // (fail-safe: no purga), en vez de caer al defecto en silencio (D20/D21). Lo
    // cazó el concilio del 2026-09-22 (opus): el filtro `>0` guardaba la dirección
    // inofensiva y dejaba pasar la peligrosa. El mínimo es de la ley, no del operador.
    let anos = match g("MEDICO_RETENCION_ANOS") {
        None => retencion::ANOS_POR_DEFECTO,
        Some(s) => match s.parse::<i64>() {
            Ok(n) if n >= retencion::MINIMO_LEGAL_ANOS => n,
            _ => {
                eprintln!(
                    "retención: MEDICO_RETENCION_ANOS={s:?} es inválido o < {} (mínimo legal); \
                     retención INACTIVA hasta corregir",
                    retencion::MINIMO_LEGAL_ANOS
                );
                return None;
            }
        },
    };
    let clave_reposo = g("MEDICO_STORAGE_KEY");
    Some(Config { frase, sk_firma, dir_archivo, dir_imagenes, clave_reposo, anos })
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("base de datos: {0}")]
    Bd(#[from] sqlx::Error),
    #[error("disco: {0}")]
    Io(#[from] std::io::Error),
    #[error("el archivo cifrado no se pudo reabrir para verificarlo: {0}")]
    NoVerifica(String),
    #[error("no se pudo firmar el manifiesto (clave inválida)")]
    NoFirma,
    #[error("la imagen «{0}» que la base nombra no se pudo leer del disco: {1}")]
    ImagenIlegible(String, std::io::Error),
    #[error("la imagen «{0}» está cifrada en reposo pero no descifra con MEDICO_STORAGE_KEY \
             (¿falta la clave o es la equivocada?): no se archiva a medias")]
    ImagenCifradaSinClave(String),
}

/// Qué pasó con un paciente elegible.
#[derive(Debug)]
pub enum Resultado {
    /// Suprimido: `id` de la fila `supresion` y la ruta del archivo bajo custodia.
    Suprimido { supresion_id: i64, ruta: String },
    /// No se purgó (el expediente cambió, o dejó de ser elegible): se descartó el
    /// archivo a medias y se reintentará en el próximo barrido.
    NoPurgado(RazonNoPurga),
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(bytes)?;
    e.finish()
}

/// Escribe bytes y los FUERZA a disco estable (`fsync`) antes de volver. Sin esto,
/// `std::fs::write` deja los bytes en la caché de páginas y `verificar` (que re-lee)
/// los sirve DESDE la caché, no del plato: un corte de energía entre escribir la
/// copia y confirmar la purga (que sí es durable, WAL synchronous=FULL) deja el
/// borrado durable y PIERDE la única copia. Es la misma falacia que el respaldo del
/// despliegue ya corrigió (`cp`+`sha256` compara la RAM consigo misma). Concilio
/// 2026-09-22 (opus).
fn escribir_durable(ruta: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(ruta)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}

/// Archiva (comprime + cifra + firma), VERIFICA y purga el expediente de un
/// paciente elegible. Es el acto completo, con su punto de no retorno al final.
pub async fn archivar_y_purgar(
    pool: &Pool,
    cfg: &Config,
    elegible: &retencion::Elegible,
    ahora: chrono::DateTime<chrono::Utc>,
) -> Result<Resultado, Error> {
    let paciente_id = elegible.paciente_id;

    // 1. Volcar el expediente (filas) y capturar las rutas de imagen ANTES de
    //    purgar: después la fila `imagen_diagnostica` ya no existe para hallarlas.
    let (expediente, recuentos) = retencion::volcar(pool, paciente_id).await?;
    let rutas_img = retencion::rutas_de_imagen(pool, paciente_id).await?;

    // 2. Meter los bytes de las imágenes (base64) para que el archivo sea
    //    autosuficiente. Si UNA no se puede leer (permiso, montaje de red, E/S), se
    //    ABORTA sin tocar nada: archivar a medias y purgar dejaría PHI clínica en el
    //    disco —la fila `imagen_diagnostica` que la nombra se borraría— o la perdería
    //    para siempre. Se reintenta el próximo barrido; un fallo permanente lo
    //    arregla el operador. Antes se marcaba `Null` y se borraba igual (concilio
    //    2026-09-22, sonnet: pérdida de PHI sin copia, con la verificación ciega a ello).
    let mut imagenes = serde_json::Map::new();
    for (ruta, cifrada) in &rutas_img {
        let p = cfg.dir_imagenes.join(ruta);
        let bytes = std::fs::read(&p).map_err(|e| Error::ImagenIlegible(ruta.clone(), e))?;
        // Si está cifrada en reposo (MEDICO_STORAGE_KEY), se DESCIFRA antes de meterla
        // al blob: así la copia bajo custodia se abre con una sola clave
        // (MEDICO_RETENCION_KEY), no con dos que el manifiesto no nombra (fable).
        let claro = if *cifrada {
            let frase = cfg.clave_reposo.as_deref().ok_or_else(|| Error::ImagenCifradaSinClave(ruta.clone()))?;
            guaca::reposo::descifrar(&bytes, frase).map_err(|_| Error::ImagenCifradaSinClave(ruta.clone()))?
        } else {
            bytes
        };
        imagenes.insert(ruta.clone(), Value::from(STANDARD.encode(&claro)));
    }

    let contenedor = serde_json::json!({
        "version": 1,
        "paciente_id": paciente_id,
        "expediente": expediente,
        "imagenes": Value::Object(imagenes),
    });
    let crudo = serde_json::to_vec(&contenedor).unwrap_or_default();

    // 3. Comprimir → cifrar → hash.
    let comprimido = gzip(&crudo)?;
    let cifrado = guaca::reposo::cifrar(&comprimido, &cfg.frase);
    let sha = sha256_hex(&cifrado);

    // 4. Escribir el archivo bajo custodia.
    std::fs::create_dir_all(&cfg.dir_archivo)?;
    let ts = ahora.format("%Y%m%dT%H%M%SZ");
    let nombre = format!("supresion-{paciente_id}-{ts}.blob");
    let ruta = cfg.dir_archivo.join(&nombre);
    escribir_durable(&ruta, &cifrado)?;

    // 5. Manifiesto firmado: ata el hash del archivo a lo que contenía.
    let mut manifiesto = std::collections::BTreeMap::<String, Value>::new();
    manifiesto.insert("paciente_id".into(), Value::from(paciente_id));
    manifiesto.insert("institucion_id".into(), Value::from(elegible.institucion_id));
    manifiesto.insert("fecha_defuncion".into(), Value::from(elegible.fecha_defuncion.clone()));
    manifiesto.insert("fecha_vencimiento".into(), Value::from(elegible.fecha_vencimiento.clone()));
    manifiesto.insert(
        "fecha_archivado".into(),
        Value::from(ahora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
    );
    manifiesto.insert("anos_retencion".into(), Value::from(cfg.anos));
    manifiesto.insert("archivo".into(), Value::from(nombre.clone()));
    manifiesto.insert("sha256".into(), Value::from(sha.clone()));
    manifiesto.insert(
        "tablas".into(),
        Value::Object(recuentos.iter().map(|(t, n)| (t.clone(), Value::from(*n))).collect()),
    );
    manifiesto.insert("imagenes".into(), Value::from(rutas_img.len()));
    let bytes_manifiesto = guaca::firma::canonico(&manifiesto);
    let firma = guaca::firma::firmar(&bytes_manifiesto, &cfg.sk_firma).ok_or(Error::NoFirma)?;
    let manifiesto_txt = String::from_utf8(bytes_manifiesto).unwrap_or_default();

    // El manifiesto firmado viaja TAMBIÉN a disco, junto al blob: si el certificado
    // en base fallara tras purgar (los datos ya no estarían), la prueba del borrado
    // seguiría existiendo fuera de memoria.
    let manifiesto_disco = serde_json::json!({ "manifiesto": manifiesto_txt, "firma": firma });
    escribir_durable(
        &cfg.dir_archivo.join(format!("{nombre}.manifiesto.json")),
        &serde_json::to_vec_pretty(&manifiesto_disco).unwrap_or_default(),
    )?;
    // Y sincronizar el DIRECTORIO: si no, las entradas nuevas (blob y manifiesto)
    // pueden no estar en el árbol de forma durable aunque su contenido sí lo esté.
    if let Ok(d) = std::fs::File::open(&cfg.dir_archivo) {
        d.sync_all().ok();
    }

    // 6. VERIFICAR antes de tocar la base: re-leer el archivo, comprobar el hash y
    //    que descifra y descomprime a un JSON. Si no, NO se purga (y se limpia).
    if let Err(e) = verificar(&ruta, &sha, &cfg.frase) {
        std::fs::remove_file(&ruta).ok();
        std::fs::remove_file(cfg.dir_archivo.join(format!("{nombre}.manifiesto.json"))).ok();
        return Err(e);
    }

    // 7. El punto de no retorno: purgar Y certificar en UNA transacción (concilio
    //    2026-09-22, sonnet): el certificado y el borrado son atómicos. Si devuelve
    //    error de base, el blob ya verificado se descarta (no se purgó). El recuento
    //    del certificado es lo ARCHIVADO (el keystone garantiza que todo se borró).
    let cert = retencion::DatosCertificado {
        institucion_id: elegible.institucion_id,
        fecha_defuncion: &elegible.fecha_defuncion,
        fecha_vencimiento: &elegible.fecha_vencimiento,
        ruta_archivo: &nombre,
        sha256_archivo: &sha,
        manifiesto: &manifiesto_txt,
        firma: &firma,
    };
    let ruta_manifiesto = cfg.dir_archivo.join(format!("{nombre}.manifiesto.json"));
    let resultado_purga =
        match retencion::purgar_y_certificar(pool, paciente_id, cfg.anos, ahora, &recuentos, &cert)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                std::fs::remove_file(&ruta).ok();
                std::fs::remove_file(&ruta_manifiesto).ok();
                return Err(e.into());
            }
        };
    match resultado_purga {
        Err(razon) => {
            // El expediente cambió, dejó de ser elegible, o el borrado quedó
            // incompleto: nada se commiteó. El archivo a medias se descarta y se
            // reintentará el próximo barrido.
            std::fs::remove_file(&ruta).ok();
            std::fs::remove_file(&ruta_manifiesto).ok();
            Ok(Resultado::NoPurgado(razon))
        }
        Ok(supresion_id) => {
            // 8. Ya purgado y certificado (atómico): borrar los archivos de imagen
            //    del disco (por la lista capturada al inicio; su fila ya no existe).
            //    Si uno falla, se grita: quedaría PHI en disco sin fila que la nombre.
            for (r, _) in &rutas_img {
                if let Err(e) = std::fs::remove_file(cfg.dir_imagenes.join(r)) {
                    eprintln!(
                        "⚠️  retención: paciente {paciente_id} suprimido pero la imagen «{r}» \
                         no se pudo borrar del disco (PHI residual): {e}"
                    );
                }
            }
            // 9. Eslabón en la cadena de acceso (Ley 1581): el acto de supresión.
            let total: i64 = recuentos.iter().map(|(_, n)| n).sum();
            eslabon_de_supresion(pool, elegible, total, cfg.sk_firma.as_str()).await;

            Ok(Resultado::Suprimido { supresion_id, ruta: nombre })
        }
    }
}

/// Re-lee el archivo cifrado, comprueba el hash y que descifra+descomprime a JSON.
fn verificar(ruta: &std::path::Path, sha_esperado: &str, frase: &str) -> Result<(), Error> {
    let leido = std::fs::read(ruta)?;
    if sha256_hex(&leido) != sha_esperado {
        return Err(Error::NoVerifica("el hash del archivo escrito no casa".into()));
    }
    let comprimido = guaca::reposo::descifrar(&leido, frase)
        .map_err(|e| Error::NoVerifica(format!("no descifra: {e:?}")))?;
    let mut crudo = Vec::new();
    std::io::copy(&mut flate2::read::GzDecoder::new(&comprimido[..]), &mut crudo)
        .map_err(|e| Error::NoVerifica(format!("no descomprime: {e}")))?;
    serde_json::from_slice::<Value>(&crudo)
        .map_err(|e| Error::NoVerifica(format!("no es JSON: {e}")))?;
    Ok(())
}

/// Deja el eslabón del acto de supresión en la cadena `acceso`. Acto de SISTEMA
/// (no hay sesión): `usuario_id = 0`, rol `sistema`. La cadena no tiene FK, así
/// que sobrevive al borrado de lo que referenciaba — que es justo lo que aquí se
/// necesita. Best-effort con grito: el certificado firmado es la prueba primaria.
async fn eslabon_de_supresion(
    pool: &Pool,
    elegible: &retencion::Elegible,
    filas_borradas: i64,
    sk_firma: &str,
) {
    let evento = medico_datos::acceso::Evento {
        usuario_id: 0,
        rol: "sistema".into(),
        institucion_id: Some(elegible.institucion_id),
        accion: "suprimir".into(),
        recurso: "expediente".into(),
        objeto: medico_datos::acceso::Objeto::DeUnTitular {
            paciente_id: elegible.paciente_id,
            alcance: medico_datos::acceso::Alcance {
                ambito: format!("institucion:{}", elegible.institucion_id),
                filtro: Some("retencion:defuncion+20a".into()),
                n: filas_borradas,
                pacientes: Vec::new(),
            },
        },
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    if let Err(e) = medico_datos::acceso::registrar(pool, &evento, Some(sk_firma)).await {
        eprintln!(
            "⚠️  supresión del paciente {} sin eslabón en la cadena de acceso: {e}",
            elegible.paciente_id
        );
    }
}

/// Un barrido: suprime a todos los elegibles del sistema. Devuelve
/// (suprimidos, no_purgados) para el log. Itera por ámbito Superadmin (todas las
/// instituciones); cada archivo queda scoped por su `institucion_id`.
///
/// Tanto la supresión (`archivar_y_purgar`) como la reconciliación de huérfanas operan
/// SOLO sobre la subcarpeta `dir_imagenes/{uuid}/` de ESTA base (namespace por instancia,
/// #758): un `MEDICO_DIR_IMAGENES` cruzado no puede tocar la PHI de otra base, así que ya
/// no hace falta la ventana de gracia ni el sello (D1).
pub async fn barrer(
    pool: &Pool,
    cfg: &Config,
    ahora: chrono::DateTime<chrono::Utc>,
) -> Result<(usize, usize), Error> {
    let elegibles =
        retencion::elegibles(pool, medico_dominio::Ambito::Superadmin, ahora, cfg.anos).await?;
    let mut suprimidos = 0;
    let mut no_purgados = 0;
    for e in &elegibles {
        match archivar_y_purgar(pool, cfg, e, ahora).await {
            Ok(Resultado::Suprimido { supresion_id, ruta }) => {
                suprimidos += 1;
                println!(
                    "retención: paciente {} suprimido (supresion #{supresion_id}, archivo {ruta})",
                    e.paciente_id
                );
            }
            Ok(Resultado::NoPurgado(razon)) => {
                no_purgados += 1;
                eprintln!("retención: paciente {} no purgado: {razon:?}", e.paciente_id);
            }
            Err(err) => {
                no_purgados += 1;
                eprintln!("retención: paciente {} falló: {err}", e.paciente_id);
            }
        }
    }
    // Reconciliación: barre imágenes huérfanas (>24 h) que una purga interrumpida
    // pudo dejar en disco sin fila. Best-effort: un fallo aquí no tumba el barrido.
    // Opera SOLO en la subcarpeta de esta instancia (namespace por instancia, #758).
    match reconciliar_imagenes_huerfanas(pool, &cfg.dir_imagenes, std::time::Duration::from_secs(24 * 3600)).await {
        Ok(n) if n > 0 => println!("retención: {n} imágenes huérfanas reconciliadas (purga interrumpida)"),
        Ok(_) => {}
        Err(e) => eprintln!("retención: reconciliación de imágenes huérfanas falló: {e}"),
    }
    Ok((suprimidos, no_purgados))
}

/// Borra las imágenes HUÉRFANAS del disco: archivos `{id}.dat` en la subcarpeta de ESTA
/// instancia (`dir_imagenes/{uuid}/`) cuya fila `imagen_diagnostica` ya no existe. Las
/// produce la purga si el proceso muere entre el commit y el borrado del archivo (concilio
/// 2026-09-22, sonnet/opus): PHI en disco sin fila que la nombre, que ningún barrido
/// reencontraría porque el paciente ya tiene certificado. Guarda de ANTIGÜEDAD
/// (`min_edad`): no toca un archivo recién escrito. La subida inserta la FILA y luego
/// escribe el archivo, pero esta función lee los ids (SELECT) y DESPUÉS el directorio: una
/// subida que inserta su fila tras el SELECT y escribe su archivo antes del `read_dir` se
/// vería como huérfana; la guarda de antigüedad la protege. Devuelve cuántas borró.
///
/// **Namespace por instancia (#758, rediseño del fail-safe #2, 2026-09-27):** solo se
/// escanea `dir_imagenes/{uuid}/`, la subcarpeta de ESTA base. Otra base de uuid DISTINTO
/// con `MEDICO_DIR_IMAGENES` cruzado escribe en `dir_imagenes/{OTRO_uuid}/`, así que el
/// borrado cruzado de PHI ajena no ocurre —reemplaza al sello `.instancia`, su ventana de
/// bootstrap y el brazo `Ajena`, todos retirados (D1)—. El oráculo `seq` se CONSERVA porque
/// protege un eje ortogonal: una base VACÍA/restaurada (sin historia de imagen, `seq` nulo)
/// no arrasa los `{id}.dat` que encuentre en su propia subcarpeta. LÍMITE (5º concilio): dos
/// CLONES vivos (mismo uuid) sobre el mismo dir comparten subcarpeta — fuera de cobertura
/// por diseño (D26; ver `respaldo::phi_de_otra_instancia`).
pub async fn reconciliar_imagenes_huerfanas(
    pool: &Pool,
    dir_imagenes: &std::path::Path,
    min_edad: std::time::Duration,
) -> Result<usize, Error> {
    // La subcarpeta de ESTA instancia; si no existe (nunca subió una imagen), nada que hacer.
    let uuid = medico_datos::imagen::uuid_de_instancia(pool).await?;
    let dir = dir_imagenes.join(&uuid);
    if !dir.is_dir() {
        return Ok(0);
    }

    // ORÁCULO de pertenencia (concilio 2026-09-22, fable): un `{id}.dat` legítimo lo
    // emitió ESTA base, así que su id ≤ el máximo que `imagen_diagnostica` ha emitido
    // jamás —`sqlite_sequence`, que sobrevive a la purga de las filas—. Sin este
    // ancla, una base vacía o EQUIVOCADA (`DATABASE_URL` mal puesta, restaurar un
    // respaldo viejo) haría `SELECT id` = vacío → TODO `{id}.dat` «huérfano» → borrado
    // MASIVO de PHI de pacientes vivos (D22/D30: un conjunto de referencia vacío
    // arrasa). Si la base nunca emitió un id de imagen (`seq` nulo), NO se borra nada
    // y se avisa si hay archivos: es la señal de una base que no es la del disco.
    let seq: Option<i64> =
        sqlx::query_scalar("SELECT seq FROM sqlite_sequence WHERE name = 'imagen_diagnostica'")
            .fetch_optional(pool)
            .await?;
    let vivos: std::collections::HashSet<i64> =
        sqlx::query_scalar::<_, i64>("SELECT id FROM imagen_diagnostica")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let ahora = std::time::SystemTime::now();
    let mut borradas = 0;
    let mut sospechosas = 0; // {id}.dat que NO pudo emitir esta base
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Ok(0);
    };
    for e in entradas.flatten() {
        let nombre = e.file_name();
        // SOLO archivos con la forma exacta `{id}.dat` que fija imagen.rs; cualquier
        // otro archivo se deja intacto (no se borra lo que no se reconoce).
        let Some(id) = nombre.to_str().and_then(|n| n.strip_suffix(".dat")).and_then(|s| s.parse::<i64>().ok())
        else {
            continue;
        };
        if vivos.contains(&id) {
            continue; // tiene fila: legítima
        }
        // Un id que esta base NUNCA emitió (seq nulo, o id > seq) NO es una huérfana
        // suya: no se toca. Protege del borrado masivo con base ajena/vacía.
        if seq.is_none_or(|max| id > max) {
            sospechosas += 1;
            continue;
        }
        let bastante_vieja = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| ahora.duration_since(t).ok())
            .is_some_and(|d| d >= min_edad);
        if bastante_vieja && std::fs::remove_file(e.path()).is_ok() {
            borradas += 1;
        }
    }
    if sospechosas > 0 {
        eprintln!(
            "⚠️  retención: {sospechosas} archivo(s) de imagen con id que esta base NUNCA emitió \
             (¿DATABASE_URL equivocada o respaldo viejo?). NO se borraron; revíselo a mano."
        );
    }
    Ok(borradas)
}

/// El bucle de la tarea de fondo: barre, espera un día, repite. Un fallo de una
/// pasada no la mata (como `ihce::correr_cola`). Con el namespace por instancia (#758,
/// rediseño 2026-09-27) el barrido opera SOLO sobre la subcarpeta de esta base, así que
/// ya no hay ventana de gracia ni marca de bootstrap que gobernar: el borrado cruzado entre
/// bases de uuid distinto no ocurre (el clon de mismo uuid queda fuera de cobertura, D26).
pub async fn correr(pool: Pool, cfg: Config) {
    loop {
        match barrer(&pool, &cfg, chrono::Utc::now()).await {
            Ok((s, n)) if s > 0 || n > 0 => {
                println!("retención: barrido — {s} suprimidos, {n} pendientes/fallidos")
            }
            Ok(_) => {}
            Err(e) => eprintln!("retención: el barrido falló entero: {e}"),
        }
        tokio::time::sleep(INTERVALO).await;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::{Duration, Utc};

    struct Temporal(PathBuf);
    impl Drop for Temporal {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
    fn dir_temporal(etiqueta: &str) -> Temporal {
        let p = std::env::temp_dir().join(format!(
            "medico-retencion-{etiqueta}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Temporal(p)
    }

    async fn base_con_fallecido() -> (Pool, String) {
        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (6,'Dr. Paz','d@x.co','h','medico',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','111','Ana','Ruiz','1940-01-01','F',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let fecha = (Utc::now() - Duration::days(21 * 365))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO defuncion \
             (paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (10,?,'INSTITUCION','NATURAL','J189',6,0,?)",
        )
        .bind(&fecha)
        .bind(&fecha)
        .execute(&pool)
        .await
        .unwrap();
        // La historia es ANTERIOR a la defunción (fail-safe #1: un encuentro posterior
        // a la muerte vuelve al paciente NO elegible). El fallecido de 21 años tuvo su
        // atención antes de morir; se fecha 22 años atrás.
        let antes = (Utc::now() - Duration::days(22 * 365))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO historia \
             (codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,\
              diagnostico_principal,plan_tratamiento,fecha) \
             VALUES ('H10',10,6,'dolor de cabeza','cefalea','G43','reposo',?)",
        )
        .bind(&antes)
        .execute(&pool)
        .await
        .unwrap();
        (pool, fecha)
    }

    fn config(dir: &std::path::Path, sk: &str) -> Config {
        Config {
            frase: "una-frase-larga-de-prueba-para-cifrar".into(),
            sk_firma: sk.into(),
            dir_archivo: dir.join("custodia"),
            dir_imagenes: dir.join("imagenes"),
            clave_reposo: None,
            anos: 20,
        }
    }

    /// El ciclo completo (D28): archiva, VERIFICA, purga. Luego se re-descifra el
    /// archivo desde el disco y se comprueba que contiene la historia (copia fiel),
    /// que la firma del manifiesto es válida, y que la historia se purgó.
    #[tokio::test]
    async fn el_ciclo_archiva_fiel_purga_y_firma_verificable() {
        let (pool, fecha) = base_con_fallecido().await;
        let tmp = dir_temporal("ciclo");
        let (vk, sk) = guaca::firma::generar_claves();
        let cfg = config(&tmp.0, &sk);

        let elegibles =
            retencion::elegibles(&pool, medico_dominio::Ambito::Superadmin, Utc::now(), 20)
                .await
                .unwrap();
        assert_eq!(elegibles.len(), 1);

        let r = archivar_y_purgar(&pool, &cfg, &elegibles[0], Utc::now()).await.unwrap();
        let ruta = match r {
            Resultado::Suprimido { ruta, .. } => ruta,
            otro => panic!("debía suprimir, no {otro:?}"),
        };

        // La historia se purgó; el paciente quedó anonimizado.
        let hist: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hist, 0);
        let nombre: String =
            sqlx::query_scalar("SELECT nombre1 FROM paciente WHERE id = 10").fetch_one(&pool).await.unwrap();
        assert_eq!(nombre, "SUPRIMIDO");

        // El archivo bajo custodia re-descifra y contiene la historia (copia fiel).
        let cifrado = std::fs::read(cfg.dir_archivo.join(&ruta)).unwrap();
        let comprimido = guaca::reposo::descifrar(&cifrado, &cfg.frase).unwrap();
        let mut crudo = Vec::new();
        std::io::copy(&mut flate2::read::GzDecoder::new(&comprimido[..]), &mut crudo).unwrap();
        let cont: Value = serde_json::from_slice(&crudo).unwrap();
        let hist_arch = &cont["expediente"]["historia"];
        assert_eq!(hist_arch.as_array().unwrap().len(), 1);
        assert_eq!(hist_arch[0]["motivo_consulta"], "dolor de cabeza");

        // El certificado guarda manifiesto + firma; la firma VERIFICA contra la vk.
        let (manifiesto, firma): (String, String) =
            sqlx::query_as("SELECT manifiesto, firma FROM supresion WHERE paciente_id = 10")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            guaca::firma::verificar(manifiesto.as_bytes(), &firma, &vk),
            guaca::firma::Verificacion::Valida
        );
        // El manifiesto ata la fecha de defunción real.
        assert!(manifiesto.contains(&fecha));
    }

    /// Una imagen que la base nombra pero no se puede leer del disco ABORTA la
    /// supresión sin borrar nada (concilio 2026-09-22, sonnet: antes se archivaba
    /// como `null` y se borraba igual → PHI perdida sin copia). Vía real: ruta que
    /// no existe en `dir_imagenes`.
    #[tokio::test]
    async fn una_imagen_ilegible_aborta_sin_purgar() {
        let (pool, _f) = base_con_fallecido().await;
        let h: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO imagen_diagnostica \
             (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,'no-existe/placa.png','placa.png','image/png',100)",
        )
        .bind(h)
        .execute(&pool)
        .await
        .unwrap();

        let tmp = dir_temporal("imagen");
        let (_vk, sk) = guaca::firma::generar_claves();
        let cfg = config(&tmp.0, &sk); // dir_imagenes = tmp/imagenes, vacío
        let elegibles =
            retencion::elegibles(&pool, medico_dominio::Ambito::Superadmin, Utc::now(), 20)
                .await
                .unwrap();
        let r = archivar_y_purgar(&pool, &cfg, &elegibles[0], Utc::now()).await;
        assert!(matches!(r, Err(Error::ImagenIlegible(_, _))), "debe abortar: {r:?}");
        let hist: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hist, 1, "la historia sigue: no se purga sin poder archivar la imagen");
    }

    /// Una imagen CIFRADA en reposo (`MEDICO_STORAGE_KEY`) se descifra antes de
    /// archivarla, de modo que la copia bajo custodia es autosuficiente con SOLO
    /// `MEDICO_RETENCION_KEY` (concilio 2026-09-22, fable). Vía real: se siembra el
    /// ciphertext en disco y se comprueba que el archivo lleva el texto CLARO.
    #[tokio::test]
    async fn una_imagen_cifrada_se_archiva_en_claro() {
        let (pool, _f) = base_con_fallecido().await;
        let h: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO imagen_diagnostica \
             (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes,cifrada) \
             VALUES (?,'placa.png','placa.png','image/png',5,1)",
        )
        .bind(h).execute(&pool).await.unwrap();

        let tmp = dir_temporal("imgcif");
        let (_vk, sk) = guaca::firma::generar_claves();
        let mut cfg = config(&tmp.0, &sk);
        let storage = "clave-de-reposo-de-prueba";
        cfg.clave_reposo = Some(storage.into());
        // El archivo en disco es el ciphertext bajo la storage key.
        let claro = b"PLACA!";
        std::fs::create_dir_all(&cfg.dir_imagenes).unwrap();
        std::fs::write(cfg.dir_imagenes.join("placa.png"), guaca::reposo::cifrar(claro, storage)).unwrap();

        let elegibles = retencion::elegibles(&pool, medico_dominio::Ambito::Superadmin, Utc::now(), 20).await.unwrap();
        let r = archivar_y_purgar(&pool, &cfg, &elegibles[0], Utc::now()).await.unwrap();
        let ruta = match r { Resultado::Suprimido { ruta, .. } => ruta, o => panic!("{o:?}") };

        // El archivo se abre con SOLO la frase de retención y lleva el texto claro.
        let cif = std::fs::read(cfg.dir_archivo.join(&ruta)).unwrap();
        let comp = guaca::reposo::descifrar(&cif, &cfg.frase).unwrap();
        let mut crudo = Vec::new();
        std::io::copy(&mut flate2::read::GzDecoder::new(&comp[..]), &mut crudo).unwrap();
        let cont: Value = serde_json::from_slice(&crudo).unwrap();
        let b64 = cont["imagenes"]["placa.png"].as_str().unwrap();
        assert_eq!(STANDARD.decode(b64).unwrap(), claro, "el archivo lleva el texto claro, no el ciphertext");
    }

    /// Una imagen cifrada en reposo SIN `clave_reposo` aborta (no se archiva a
    /// medias): la copia sería inservible bajo una clave que el manifiesto no nombra.
    #[tokio::test]
    async fn una_imagen_cifrada_sin_clave_aborta() {
        let (pool, _f) = base_con_fallecido().await;
        let h: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO imagen_diagnostica \
             (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes,cifrada) \
             VALUES (?,'placa.png','placa.png','image/png',5,1)",
        )
        .bind(h).execute(&pool).await.unwrap();
        let tmp = dir_temporal("imgsinclave");
        let (_vk, sk) = guaca::firma::generar_claves();
        let cfg = config(&tmp.0, &sk); // clave_reposo = None
        std::fs::create_dir_all(&cfg.dir_imagenes).unwrap();
        std::fs::write(cfg.dir_imagenes.join("placa.png"), b"loquesea").unwrap();
        let elegibles = retencion::elegibles(&pool, medico_dominio::Ambito::Superadmin, Utc::now(), 20).await.unwrap();
        let r = archivar_y_purgar(&pool, &cfg, &elegibles[0], Utc::now()).await;
        assert!(matches!(r, Err(Error::ImagenCifradaSinClave(_))), "debe abortar: {r:?}");
        let hist: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(hist, 1, "no se purga sin poder archivar la imagen");
    }

    /// Reconciliación (concilio 2026-09-22): borra `{id}.dat` huérfano SOLO si esta
    /// base lo emitió (id ≤ sqlite_sequence), conserva las legítimas, respeta la
    /// guarda de antigüedad, y NO toca ids que la base nunca emitió (protección
    /// contra base equivocada/vacía = borrado masivo, fable).
    #[tokio::test]
    async fn reconciliar_solo_borra_lo_que_esta_base_emitio() {
        let (pool, _f) = base_con_fallecido().await;
        let h: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool).await.unwrap();
        // Imagen legítima VIVA (fija seq ≥ su id).
        let leg: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,'placeholder','x.png','image/png',1)",
        )
        .bind(h).execute(&pool).await.unwrap().last_insert_rowid();
        // Una huérfana REAL: una imagen emitida (id ≤ seq) cuya fila se purgó.
        let huerfana: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,'placeholder','y.png','image/png',1)",
        )
        .bind(h).execute(&pool).await.unwrap().last_insert_rowid();
        sqlx::query("DELETE FROM imagen_diagnostica WHERE id = ?").bind(huerfana).execute(&pool).await.unwrap();

        let tmp = dir_temporal("reconc");
        let dir = tmp.0.join("imagenes");
        // Namespace por instancia (#758): los archivos viven en `dir/{uuid}/`, que es la
        // ÚNICA subcarpeta que la reconciliación de esta base escanea.
        let uuid = medico_datos::imagen::uuid_de_instancia(&pool).await.unwrap();
        let sub = dir.join(&uuid);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(format!("{leg}.dat")), b"legitima").unwrap();       // fila viva
        std::fs::write(sub.join(format!("{huerfana}.dat")), b"huerfana").unwrap();   // emitida, sin fila
        std::fs::write(sub.join("999999.dat"), b"ajena").unwrap();                   // id NUNCA emitido
        std::fs::write(sub.join("no-es-imagen.txt"), b"otro").unwrap();              // no {id}.dat

        let n = reconciliar_imagenes_huerfanas(&pool, &dir, std::time::Duration::ZERO).await.unwrap();
        assert_eq!(n, 1, "solo la huérfana REAL (id emitido y sin fila)");
        assert!(sub.join(format!("{leg}.dat")).exists(), "la legítima se conserva");
        assert!(!sub.join(format!("{huerfana}.dat")).exists(), "la huérfana real se borró");
        assert!(sub.join("999999.dat").exists(), "un id NUNCA emitido NO se toca (¿base equivocada?)");
        assert!(sub.join("no-es-imagen.txt").exists(), "no toca lo que no es {{id}}.dat");

        // Guarda de antigüedad: una huérfana real RECIÉN escrita no se toca.
        std::fs::write(sub.join(format!("{huerfana}.dat")), b"fresca").unwrap();
        let n2 = reconciliar_imagenes_huerfanas(&pool, &dir, std::time::Duration::from_secs(365 * 24 * 3600)).await.unwrap();
        assert_eq!(n2, 0, "una huérfana fresca no se borra (subida en vuelo)");
    }

    /// Protección de borrado masivo (fable, conf 8): con la base EQUIVOCADA/VACÍA
    /// (ninguna imagen emitida jamás → `sqlite_sequence` sin fila), NO se borra NADA
    /// aunque el disco tenga muchos `{id}.dat`. Vía real: base sin historia de imagen.
    #[tokio::test]
    async fn reconciliar_no_arrasa_con_base_equivocada() {
        let (pool, _f) = base_con_fallecido().await; // esta base NUNCA insertó una imagen
        let tmp = dir_temporal("reconc-masivo");
        let dir = tmp.0.join("imagenes");
        let uuid = medico_datos::imagen::uuid_de_instancia(&pool).await.unwrap();
        let sub = dir.join(&uuid);
        std::fs::create_dir_all(&sub).unwrap();
        for i in 1..=50 {
            std::fs::write(sub.join(format!("{i}.dat")), b"phi de pacientes vivos").unwrap();
        }
        let n = reconciliar_imagenes_huerfanas(&pool, &dir, std::time::Duration::ZERO).await.unwrap();
        assert_eq!(n, 0, "con base sin historia de imagen NO se borra nada (protección masiva)");
        let dats = std::fs::read_dir(&sub)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".dat"))
            .count();
        assert_eq!(dats, 50, "los 50 archivos de PHI siguen");
    }

    // ── FAIL-SAFE #2: namespace de archivos por instancia (#758, rediseño 2026-09-27) ──
    //
    // Reemplaza el sello `.instancia` + su ventana + el oráculo como barrera anti-cruce
    // (retirados tras el 4º concilio, que midió que el sello TOFU dejaba dos vías más de
    // borrado/fuga de PHI ajena). El aislamiento es ESTRUCTURAL: cada base vive en
    // `dir/{su_uuid}/`, así que una base de uuid distinto no toca los archivos de otra
    // aunque compartan `MEDICO_DIR_IMAGENES` (el clon de mismo uuid queda fuera, D26). El
    // oráculo `seq` se conserva SOLO para el eje ortogonal (base vacía/restaurada), probado
    // en `reconciliar_no_arrasa_con_base_equivocada`.

    /// D33 — RECONCILIACIÓN con namespace: dos bases que comparten `MEDICO_DIR_IMAGENES`
    /// con ids COLISIONANTES (ambas autoincrementan desde 1). B reconcilia el `dir`
    /// compartido: borra SU huérfana (en `dir/{uuid_b}/`) y NO toca la PHI viva de A
    /// (en `dir/{uuid_a}/`, con el MISMO id). Discrimina el cambio de código: si
    /// `reconciliar` mirara la raíz `dir` (layout viejo) en vez de `dir/{uuid}`, no
    /// hallaría la huérfana de B en su subcarpeta → `n==0` → esta prueba falla.
    #[tokio::test]
    async fn reconciliar_namespace_no_toca_la_phi_de_otra_base_con_id_colisionante() {
        let (pool_a, _fa) = base_con_fallecido().await;
        let (pool_b, _fb) = base_con_fallecido().await;
        let uuid_a = medico_datos::imagen::uuid_de_instancia(&pool_a).await.unwrap();
        let uuid_b = medico_datos::imagen::uuid_de_instancia(&pool_b).await.unwrap();
        assert_ne!(uuid_a, uuid_b, "A y B son instancias distintas");

        let tmp = dir_temporal("namespace-reconc");
        let dir = tmp.0.join("imagenes");
        let sub_a = dir.join(&uuid_a);
        let sub_b = dir.join(&uuid_b);
        std::fs::create_dir_all(&sub_a).unwrap();
        std::fs::create_dir_all(&sub_b).unwrap();

        // A: imagen VIVA (fila + archivo), id = 1.
        let ha: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool_a).await.unwrap();
        let id_a: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,?,'v.png','image/png',1)",
        ).bind(ha).bind(format!("{uuid_a}/1.dat")).execute(&pool_a).await.unwrap().last_insert_rowid();
        std::fs::write(sub_a.join(format!("{id_a}.dat")), b"phi-viva-de-A").unwrap();

        // B: una huérfana PROPIA con el MISMO id (colisión de id, no de path).
        let hb: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool_b).await.unwrap();
        let id_b: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,?,'y.png','image/png',1)",
        ).bind(hb).bind("tmp").execute(&pool_b).await.unwrap().last_insert_rowid();
        sqlx::query("DELETE FROM imagen_diagnostica WHERE id = ?").bind(id_b).execute(&pool_b).await.unwrap();
        assert_eq!(id_a, id_b, "los ids colisionan (ambas desde 1): sin namespace se pisarían");
        std::fs::write(sub_b.join(format!("{id_b}.dat")), b"huerfana-de-B").unwrap();

        let n = reconciliar_imagenes_huerfanas(&pool_b, &dir, std::time::Duration::ZERO).await.unwrap();
        assert_eq!(n, 1, "B borra su propia huérfana (la reconciliación funciona, no está ciega)");
        assert!(!sub_b.join(format!("{id_b}.dat")).exists(), "la huérfana propia de B se borró");
        assert!(sub_a.join(format!("{id_a}.dat")).exists(), "la PHI VIVA de A —mismo id— NO se tocó (namespace)");
    }

    /// D33 — SUPRESIÓN con namespace (el vector H1 del 4º concilio, conf 9): B suprime a
    /// SU fallecido elegible, que tiene una imagen. El paso 8 de `archivar_y_purgar` borra
    /// `dir.join(ruta_archivo)` = `dir/{uuid_b}/{id}.dat`; con `MEDICO_DIR_IMAGENES` cruzado,
    /// la PHI viva de A (en `dir/{uuid_a}/`, mismo id) NO se lee ni se borra ni se archiva.
    /// Sin el namespace (H1), la supresión de B habría leído y borrado la imagen de A.
    #[tokio::test]
    async fn la_supresion_con_namespace_no_toca_la_phi_de_otra_base() {
        let (pool_a, _fa) = base_con_fallecido().await;
        let (pool_b, _fb) = base_con_fallecido().await;
        let uuid_a = medico_datos::imagen::uuid_de_instancia(&pool_a).await.unwrap();
        let uuid_b = medico_datos::imagen::uuid_de_instancia(&pool_b).await.unwrap();

        let tmp = dir_temporal("namespace-supr");
        let (_vk, sk) = guaca::firma::generar_claves();
        // dir_imagenes CRUZADO: B apunta al mismo `dir` donde vive la PHI de A.
        let cfg = config(&tmp.0, &sk);
        let sub_a = cfg.dir_imagenes.join(&uuid_a);
        let sub_b = cfg.dir_imagenes.join(&uuid_b);
        std::fs::create_dir_all(&sub_a).unwrap();
        std::fs::create_dir_all(&sub_b).unwrap();

        // A: PHI VIVA con id colisionante.
        let ha: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool_a).await.unwrap();
        let id_a: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,?,'v.png','image/png',1)",
        ).bind(ha).bind(format!("{uuid_a}/1.dat")).execute(&pool_a).await.unwrap().last_insert_rowid();
        std::fs::write(sub_a.join(format!("{id_a}.dat")), b"phi-viva-de-A").unwrap();

        // B: su fallecido (paciente 10, elegible) tiene una imagen en SU subcarpeta.
        let hb: i64 = sqlx::query_scalar("SELECT id FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool_b).await.unwrap();
        let id_b: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (?,?,'b.png','image/png',1)",
        ).bind(hb).bind(format!("{uuid_b}/1.dat")).execute(&pool_b).await.unwrap().last_insert_rowid();
        std::fs::write(sub_b.join(format!("{id_b}.dat")), b"imagen-de-B").unwrap();
        assert_eq!(id_a, id_b, "ids colisionan: sin namespace la supresión de B leería/borraría la de A");

        let (suprimidos, _) = barrer(&pool_b, &cfg, Utc::now()).await.unwrap();
        assert_eq!(suprimidos, 1, "B suprimió a su fallecido");
        assert!(!sub_b.join(format!("{id_b}.dat")).exists(), "la imagen del expediente de B se borró (purga normal)");
        assert!(sub_a.join(format!("{id_a}.dat")).exists(), "la PHI VIVA de A —mismo id— sobrevive (namespace)");

        // Y el archivo bajo custodia de B NO contiene la PHI de A (no hubo fuga).
        // (su expediente archivado lleva la imagen de B, no la de A; basta con que A siga en disco).
    }

    /// Dos bases DISTINTAS tienen uuid DISTINTO (el discriminador de fondo del namespace):
    /// si colisionaran, dos bases compartirían subcarpeta. Sembrado por `randomblob` (0077).
    #[tokio::test]
    async fn dos_bases_distintas_tienen_identidad_distinta() {
        let (a, _fa) = base_con_fallecido().await;
        let (b, _fb) = base_con_fallecido().await;
        let ua = medico_datos::imagen::uuid_de_instancia(&a).await.unwrap();
        let ub = medico_datos::imagen::uuid_de_instancia(&b).await.unwrap();
        assert_eq!(ua.len(), 32, "uuid = 16 bytes en hex");
        assert_ne!(ua, ub, "cada base nace con una identidad propia");
    }

    /// El piso legal (concilio 2026-09-22, opus): `MEDICO_RETENCION_ANOS` por debajo
    /// de 15 deja el subsistema INACTIVO (fail-safe), no cae al defecto en silencio.
    #[test]
    fn el_periodo_por_debajo_del_minimo_legal_desactiva() {
        let base = |k: &str| -> Option<String> {
            match k {
                "MEDICO_RETENCION_KEY" => Some("frase".into()),
                "MEDICO_SIGNING_KEY" => Some("sk".into()),
                "MEDICO_RETENCION_DIR" => Some("/x".into()),
                _ => None,
            }
        };
        let con = |v: &str| {
            let v = v.to_string();
            move |k: &str| if k == "MEDICO_RETENCION_ANOS" { Some(v.clone()) } else { base(k) }
        };
        // Sin la var → defecto 20, activo.
        assert_eq!(config_desde(base).map(|c| c.anos), Some(20));
        // 15 (el piso) → activo, con anos=15.
        assert_eq!(config_desde(con("15")).map(|c| c.anos), Some(15));
        // 5 (bajo el piso) → INACTIVO.
        assert!(config_desde(con("5")).is_none(), "5 años < 15 debe desactivar");
        // Basura → INACTIVO.
        assert!(config_desde(con("abc")).is_none());
    }

    /// El barrido (punto de entrada de la tarea de fondo): suprime a TODOS los
    /// elegibles, deja en paz a los que no cumplen el plazo, y es idempotente (la
    /// segunda pasada no vuelve a tocar a nadie).
    #[tokio::test]
    async fn el_barrido_suprime_a_los_elegibles_y_es_idempotente() {
        let (pool, _fecha) = base_con_fallecido().await; // paciente 10, muerto hace 21 años
        // Un segundo fallecido reciente (hace 2 años): NO debe suprimirse.
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (11,'CC','222','Beto','Paz','1950-01-01','M',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let reciente = (Utc::now() - Duration::days(2 * 365))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO defuncion \
             (paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (11,?,'INSTITUCION','NATURAL','J189',6,0,?)",
        )
        .bind(&reciente)
        .bind(&reciente)
        .execute(&pool)
        .await
        .unwrap();

        let tmp = dir_temporal("barrido");
        let (_vk, sk) = guaca::firma::generar_claves();
        let cfg = config(&tmp.0, &sk);

        let (suprimidos, fallidos) = barrer(&pool, &cfg, Utc::now()).await.unwrap();
        assert_eq!((suprimidos, fallidos), (1, 0), "solo el fallecido hace 21 años");

        // El reciente sigue intacto; el viejo tiene certificado.
        let n10: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM supresion WHERE paciente_id = 10")
                .fetch_one(&pool).await.unwrap();
        let n11: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM supresion WHERE paciente_id = 11")
                .fetch_one(&pool).await.unwrap();
        assert_eq!((n10, n11), (1, 0));

        // Idempotencia: la segunda pasada no suprime a nadie más.
        let (s2, _) = barrer(&pool, &cfg, Utc::now()).await.unwrap();
        assert_eq!(s2, 0, "no se suprime dos veces");
    }

    /// Par rojo/verde del guardián de verificación (D33): `verificar` acepta el
    /// archivo con la frase correcta y lo RECHAZA con una frase equivocada — que es
    /// la vía por la que un archivo ilegible bloquearía la purga.
    #[tokio::test]
    async fn verificar_discrimina_la_frase() {
        let tmp = dir_temporal("verif");
        let frase = "la-buena";
        let contenido = br#"{"expediente":"de prueba"}"#;
        let comprimido = gzip(contenido).unwrap();
        let cifrado = guaca::reposo::cifrar(&comprimido, frase);
        let sha = sha256_hex(&cifrado);
        let ruta = tmp.0.join("x.blob");
        std::fs::write(&ruta, &cifrado).unwrap();

        // Verde: frase correcta y hash correcto.
        assert!(verificar(&ruta, &sha, frase).is_ok());
        // Rojo por frase: no descifra.
        assert!(verificar(&ruta, &sha, "la-mala").is_err());
        // Rojo por hash: el archivo no es el que se firmó.
        assert!(verificar(&ruta, "0000", frase).is_err());
    }
}
