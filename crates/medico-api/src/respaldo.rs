//! Respaldo y restauración de medico. **Lo que se restaura es lo que se probó.**
//!
//! Medido en #609 (B4.a): medico no tenía respaldo/restauración de la base —una
//! de las cinco condiciones de «plena operación»—, mientras informes sí
//! (`siger-api/src/respaldo.rs`, del que esto es la adaptación). Un producto
//! clínico instalado en casa del cliente sin forma de respaldar su base es un
//! riesgo operativo directo.
//!
//! # Por qué esto no es «copiar el archivo»
//!
//! La base puede ir en modo WAL: las escrituras confirmadas viven en
//! `medico.db-wal` hasta que alguien las integra. Copiar `medico.db` con el
//! servidor vivo produce un archivo que se abre, que pasa por bueno, y al que le
//! faltan las últimas transacciones —o que trae una a medias—. El respaldo parece
//! bueno hasta el día en que hace falta, que es el único día en que se mira. Aquí
//! se usa `VACUUM INTO`, que toma la instantánea bajo una transacción de lectura:
//! consistente, en caliente, y sin parar el servicio.
//!
//! # Y por qué la base sola no es un respaldo
//!
//! Las imágenes diagnósticas viven FUERA de la base, en `dir_imagenes`
//! (`imagen_diagnostica.ruta_archivo`). Un respaldo que solo guarda `medico.db`
//! restaura un sistema donde cada placa dice existir y el disco no la tiene: la
//! fila promete y el disco no cumple. El respaldo lleva los dos, y se niega a
//! declararse bueno si algún archivo que la base nombra no llegó a la copia.
//!
//! **Almacenes en disco (revisar esta lista al añadir uno):** `dir_imagenes` y,
//! desde el 2026-09-22 (#758), `MEDICO_RETENCION_DIR` —el archivo cifrado bajo
//! custodia de cada supresión, que es la ÚNICA copia del expediente de un paciente
//! ya purgado—. Los dos entran en `respaldar`/`restaurar`; `verificar` exige que
//! cada blob que la tabla `supresion` nombra esté en `custodia/` con su sha256.
//! Verificado el 2026-08-14 (imágenes): el único `tokio::fs::write` de contenido
//! clínico es el de la subida de imágenes (`rutas.rs`, bajo `dir_imagenes`). Las
//! columnas de ruta `consentimiento_informado.firma_*_path` e
//! `informe_imagen.imagenes_paths` existen en el esquema pero **están dormidas**:
//! se rellenan con cadenas (en pruebas, `/f/p.png`) y NADA escribe esos archivos
//! a disco todavía —como estuvo `unidad_dispensacion`—. El día que la captura de
//! firmas o de imágenes de informe se cablee a disco, su raíz de almacenamiento
//! entra en este módulo; hasta entonces, respaldar `dir_imagenes` es completo.
//!
//! # La verificación no es opcional
//!
//! «Hay un cron» no es tener respaldos. Cada respaldo se ABRE al terminar, se le
//! pasa `PRAGMA integrity_check`, se comparan los recuentos de las tablas con el
//! origen y se comprueba imagen por imagen. Si algo no cuadra, sale con error y lo
//! dice; no deja una carpeta que parezca un respaldo.

use std::path::{Path, PathBuf};

use sqlx::{Connection, Row, SqliteConnection};

/// sha256 en hex, para verificar el archivo bajo custodia contra `supresion.sha256_archivo`.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::new().chain_update(bytes).finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Las tablas cuyo recuento se compara entre origen y copia.
///
/// Es la lista de lo que constituye el estado del sistema. Si mañana nace una
/// tabla nueva y no entra aquí, el respaldo seguiría diciendo «verificado»
/// habiéndola ignorado, así que `las_tablas_del_esquema_estan_todas` compara esta
/// lista contra el esquema real y falla si alguien añade una tabla sin pasar por
/// aquí (o a `TABLAS_EXENTAS` con su razón).
const TABLAS: &[&str] = &[
    // Catálogos y estructura institucional.
    "institucion",
    "usuario",
    "paciente",
    "entidad",
    "contrato",
    "licencia",
    "configuracion_institucion",
    "config_campo",
    "capacidad_rol",
    "politica_seguridad",
    "especialidad",
    "proveedor",
    // Maestros sembrados (van en la copia como estado; su recuento delata una
    // siembra a medias).
    "departamento",
    "municipio",
    "cie10",
    "cups",
    "medicamento",
    "unidad_dispensacion",
    // Acto clínico.
    "historia",
    "documento_clinico",
    "consentimiento_informado",
    "contrarreferencia",
    "formula",
    "orden",
    "imagen_diagnostica",
    "informe_imagen",
    "dispensacion",
    // Agenda y atención.
    "cita",
    "agenda_medico",
    "bloque_horario",
    "bloqueo_agenda",
    "atencion_urgencias",
    "hospitalizacion",
    // El registro de nacimiento (#249). Va aquí y no en las exentas: es dato
    // clínico del recién nacido y es el insumo del arreglo `recienNacidos` del
    // RIPS — perderlo en una restauración es perder la facturación del parto.
    "recien_nacido",
    // El MAR. Es prueba de qué se le puso a quién: perderlo en una restauración
    // es perder la defensa del prestador ante un evento adverso.
    "administracion_medicamento",
    // El certificado de defunción. Alimenta el registro civil y la estadística
    // vital: perderlo en una restauración no se puede reconstruir de ningún sitio.
    "defuncion",
    // El acto anestésico. La valoración dice si el paciente podía entrar y la
    // recuperación si podía salir: son las dos piezas de la defensa del
    // prestador ante un evento adverso perioperatorio.
    "valoracion_preanestesica",
    "recuperacion_postanestesica",
    // P&P. `vacuna_aplicada` lleva el LOTE, que es lo que permite encontrar a
    // quién se le aplicó un biológico retirado: perderlo es perder la
    // trazabilidad de una campaña entera.
    "inscripcion_pyp",
    "control_pyp",
    "vacuna_aplicada",
    // Habilitación. La hoja de vida de un equipo y las actas de los comités son
    // lo que se enseña en una visita: perderlas no se reconstruye de ningún lado.
    "equipo_biomedico",
    "mantenimiento_equipo",
    "comite",
    "sesion_comite",
    "evento_iaas",
    // Los últimos. `transfusion` y `unidad_sangre` son la trazabilidad de una
    // bolsa: perderlas es no poder responder a quién se le puso un lote.
    "teleconsulta",
    "turno_personal",
    "unidad_sangre",
    "transfusion",
    "cama",
    "quirofano",
    "programacion_quirurgica",
    "traslado",
    "ambulancia",
    // Apoyo diagnóstico y terapéutico.
    "perfil_laboratorio",
    "resultado_laboratorio",
    "plan_alimentacion",
    "valoracion_nutricional",
    "plan_rehabilitacion",
    "sesion_terapia",
    // Inventario.
    "item_inventario",
    "movimiento_inventario",
    // Facturación y RIPS.
    "factura",
    "detalle_factura",
    "glosa",
    "envio_muv",
    "envio_rda",
    // El aval del `nilknown` del RDA (#603 F4.5). Cada fila es la afirmación de un
    // clínico de que una sección quedó revisada «sin nada conocido», con quién y
    // cuándo: perderla en una restauración es perder la atribución de una
    // afirmación clínica que viaja al visor nacional, y con ella la defensa de por
    // qué se emitió ese `nilknown`.
    "aval_seccion_rda",
    // Seguridad de cuenta.
    "codigo_recuperacion",
    // El certificado de supresión por defunción (#758). Es la PRUEBA (Ley 1581) de
    // qué expediente se archivó y purgó, con su manifiesto firmado: perderlo en una
    // restauración es perder la constancia del borrado, que no se reconstruye.
    "supresion",
    // La identidad de instancia (#758, migración 0077). El respaldo la LLEVA a
    // propósito: un respaldo restaurado es la MISMA instancia lógica, así que conserva su
    // uuid y su carpeta de imágenes `dir/{uuid}/` (namespace por instancia, fail-safe #2).
    // Perderla en la restauración desvincularía la base de sus imágenes: la reconciliación
    // buscaría en la subcarpeta de un uuid nuevo y las daría por inexistentes.
    "instancia",
];

#[derive(Debug, thiserror::Error)]
pub enum ErrorRespaldo {
    #[error("no se pudo abrir «{0}»: {1}")]
    NoAbre(String, sqlx::Error),
    #[error("la copia no se pudo escribir en «{0}»: {1}")]
    NoEscribe(String, sqlx::Error),
    #[error("problema con los archivos: {0}")]
    Archivos(#[from] std::io::Error),
    #[error("el destino «{0}» ya tiene contenido; elija una carpeta nueva")]
    DestinoOcupado(String),
    #[error("«{0}» no parece un respaldo de medico: falta {1}")]
    NoEsRespaldo(String, &'static str),
    #[error(
        "la copia NO es fiel al origen y por eso no se declara buena:\n{}",
        .0.join("\n")
    )]
    NoVerifica(Vec<String>),
    #[error(
        "la base de destino ya tiene datos ({0} pacientes, {1} historias). \
         Restaurar los borraría. Pare el servicio y use --forzar si es lo que quiere."
    )]
    DestinoConDatos(i64, i64),
    #[error(
        "la carpeta de imágenes de destino está sellada por OTRA instancia \
         ({existente} ≠ {entrante}): restaurar sobre ella borraría imágenes vivas de esa \
         instancia. ¿MEDICO_DIR_IMAGENES cruzado? Elija una carpeta de imágenes vacía o \
         la de esta instancia."
    )]
    DestinoImagenesDeOtraInstancia { existente: String, entrante: String },
}

/// Qué se guardó, para poder decirlo y para poder compararlo.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resumen {
    pub filas: Vec<(String, i64)>,
    pub imagenes: usize,
    pub bytes_bd: u64,
}

impl Resumen {
    pub fn total_filas(&self) -> i64 {
        self.filas.iter().map(|(_, n)| n).sum()
    }
}

fn ruta_de_url(url: &str) -> PathBuf {
    // `sqlite://medico.db`, `sqlite:medico.db` o una ruta pelada. Los parámetros
    // (`?mode=rwc`) se descartan: no forman parte del nombre del archivo.
    let sin_esquema = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .unwrap_or(url);
    PathBuf::from(sin_esquema.split('?').next().unwrap_or(sin_esquema))
}

async fn conectar(ruta: &Path) -> Result<SqliteConnection, ErrorRespaldo> {
    let url = format!("sqlite://{}", ruta.display());
    SqliteConnection::connect(&url)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(ruta.display().to_string(), e))
}

async fn recuentos(cx: &mut SqliteConnection) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let mut salida = Vec::new();
    for tabla in TABLAS {
        // El nombre de la tabla viene de una constante del código, nunca de
        // fuera: no hay parámetro que ligar porque no hay entrada del usuario.
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {tabla}"))
            .fetch_one(&mut *cx)
            .await?;
        salida.push(((*tabla).to_string(), n));
    }
    Ok(salida)
}

/// Las imágenes que la base DICE tener en disco, con su id.
async fn imagenes_declaradas(cx: &mut SqliteConnection) -> Result<Vec<(i64, String)>, sqlx::Error> {
    let filas = sqlx::query(
        "SELECT id, ruta_archivo FROM imagen_diagnostica \
         WHERE ruta_archivo IS NOT NULL AND ruta_archivo <> ''",
    )
    .fetch_all(&mut *cx)
    .await?;
    Ok(filas
        .into_iter()
        .map(|f| (f.get::<i64, _>("id"), f.get::<String, _>("ruta_archivo")))
        .collect())
}

/// El uuid de `instancia` (migración 0077) de una base SQLite, o `None` si la base no
/// la tiene (respaldo pre-0077) o no se puede abrir/leer. Se usa para verificar, ANTES
/// de copiar, que un respaldo y la carpeta de imágenes de destino son la MISMA instancia
/// (guarda H3 de `restaurar`).
async fn uuid_de_base(bd: &Path) -> Option<String> {
    let mut cx = conectar(bd).await.ok()?;
    let uuid = sqlx::query_scalar::<_, String>("SELECT uuid FROM instancia WHERE id = 1")
        .fetch_optional(&mut cx)
        .await
        .ok()
        .flatten();
    cx.close().await.ok();
    uuid
}

/// El identificador de una instancia cuya PHI vive en `dir` y que NO es la del respaldo
/// entrante (`entrante`), o `None` si el destino no tiene PHI ajena que restaurar pondría
/// en riesgo. Es la guarda de `restaurar` rebasada al **namespace por instancia** (#758):
/// la PHI vive en `dir/{uuid}/{id}.dat`. `restaurar` reemplaza `medico.db` (la identidad
/// de la instancia) y copia el árbol del respaldo; si el destino tiene la PHI de OTRA base,
/// esa base quedaría sin su identidad y su PHI huérfana. Se considera ajena:
///   · una subcarpeta `{uuid}/` NO vacía cuyo `uuid` ≠ `entrante` (otra instancia);
///   · un `.dat` PLANO en la raíz (PHI LEGADA sin migrar, de origen no atribuible →
///     fail-closed: no se puede probar que sea de la instancia entrante);
///   · un SYMLINK (no se sigue con `file_type()`: no atribuible → fail-closed);
///   · un directorio o entrada ilegible (fail-closed: no se pisa lo que no se inspecciona).
/// La subcarpeta REAL de la PROPIA instancia entrante NO cuenta (re-restore legítimo).
///
/// **Límite conocido y documentado (5º concilio 2026-09-27):** el discriminante es el uuid
/// de `instancia`, que se CLONA con la base (un respaldo restaurado comparte identidad con
/// su origen). Dos CLONES vivos compartiendo `MEDICO_DIR_IMAGENES` no se distinguen aquí
/// —comparten subcarpeta—; eso queda fuera de cobertura por diseño (viola «una base por
/// instancia», D26: restaurar REEMPLAZA, no crea una réplica para correr en paralelo).
fn phi_de_otra_instancia(dir: &Path, entrante: Option<&str>) -> Option<String> {
    let entradas = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => return Some("(carpeta de imágenes ilegible)".into()),
    };
    for entrada in entradas {
        let entrada = match entrada {
            Ok(e) => e,
            Err(_) => return Some("(entrada de imágenes ilegible)".into()),
        };
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        let tipo = match entrada.file_type() {
            Ok(t) => t,
            Err(_) => return Some("(tipo de entrada ilegible)".into()),
        };
        // El orden es fail-closed: se enumera lo SEGURO (subcarpeta propia real, archivo
        // que no es PHI) y TODO lo demás cuenta como ajeno (D23). `file_type()` NO sigue
        // symlinks, así que un symlink es un tipo aparte que hay que tratar explícitamente
        // —si se cayera en `else` se ignoraría, que es el hueco que el 6º concilio halló
        // (sonnet, por ejecución)—.
        if tipo.is_symlink() {
            // No se puede atribuir a qué instancia pertenece ni qué contiene sin resolverlo,
            // y el código normal (`imagen::registrar`) nunca crea symlinks en esta carpeta.
            // Fail-closed → ajeno. Cierra el bypass por symlink a una subcarpeta ajena.
            return Some(format!("(entrada symlink no atribuible: {nombre})"));
        } else if tipo.is_dir() {
            // Subcarpeta de instancia: cuenta si es de OTRO uuid y NO está vacía. Se mira
            // cualquier contenido, no solo `.dat`: una subcarpeta nombrada por uuid solo la
            // crea otra instancia, y su contenido es PHI ajena aunque un formato futuro no
            // sea `.dat` (fable, 6º concilio: no atar la guarda al invariante «todo es .dat»).
            if entrante != Some(nombre.as_str()) && subcarpeta_no_vacia(&entrada.path()) {
                return Some(nombre);
            }
        } else if nombre.ends_with(".dat") {
            // PHI legada plana en la raíz: no atribuible → ajena (fail-closed).
            return Some("(imágenes legadas sin migrar, origen desconocido)".into());
        }
    }
    None
}

/// ¿La subcarpeta de una instancia contiene ALGO? Cuenta cualquier entrada, no solo `.dat`
/// (ver `phi_de_otra_instancia`). Un `read_dir` que no abre, o un error de iteración a mitad,
/// → `true` (fail-closed, coherente con `phi_de_otra_instancia`: no se pisa lo que no se
/// puede inspeccionar por completo). Solo una carpeta que se lee entera y sale vacía → `false`.
fn subcarpeta_no_vacia(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut entradas) => match entradas.next() {
            Some(Ok(_)) => true,  // al menos una entrada
            Some(Err(_)) => true, // error de iteración → fail-closed
            None => false,        // se leyó entera y está vacía
        },
        Err(_) => true,
    }
}

fn copiar_arbol(origen: &Path, destino: &Path) -> std::io::Result<usize> {
    if !origen.exists() {
        // Que no haya ni una imagen subida es un estado legítimo de una
        // instalación recién puesta, no un error.
        return Ok(0);
    }
    let mut copiados = 0;
    std::fs::create_dir_all(destino)?;
    for entrada in std::fs::read_dir(origen)? {
        let entrada = entrada?;
        let destino_hijo = destino.join(entrada.file_name());
        if entrada.file_type()?.is_dir() {
            copiados += copiar_arbol(&entrada.path(), &destino_hijo)?;
        } else {
            std::fs::copy(entrada.path(), &destino_hijo)?;
            copiados += 1;
        }
    }
    Ok(copiados)
}

/// `medico-api respaldar <carpeta>`: la base en caliente y sus imágenes, con la
/// copia verificada contra el origen antes de decir que existe.
pub async fn respaldar(
    url_bd: &str,
    dir_imagenes: &str,
    dir_archivo: Option<&str>,
    destino: &Path,
) -> Result<Resumen, ErrorRespaldo> {
    if destino.exists() && std::fs::read_dir(destino)?.next().is_some() {
        // Escribir dentro de una carpeta que ya tiene cosas mezcla dos respaldos
        // y produce uno que no es ninguno de los dos.
        return Err(ErrorRespaldo::DestinoOcupado(destino.display().to_string()));
    }
    std::fs::create_dir_all(destino)?;

    let origen_bd = ruta_de_url(url_bd);
    let copia_bd = destino.join("medico.db");

    let mut cx = conectar(&origen_bd).await?;
    let filas_origen = recuentos(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(origen_bd.display().to_string(), e))?;
    let declaradas = imagenes_declaradas(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(origen_bd.display().to_string(), e))?;

    // `VACUUM INTO` toma la instantánea bajo una transacción de lectura: no para
    // al servidor, no arrastra el WAL a medias y deja la copia ya compactada.
    // El destino lo escribe SQLite, así que la ruta va en la propia orden; se
    // rechaza cualquier comilla en ella antes de llegar aquí.
    let ruta_copia = copia_bd.display().to_string();
    if ruta_copia.contains('\'') {
        return Err(ErrorRespaldo::DestinoOcupado(format!(
            "{ruta_copia} (la ruta no puede llevar comillas simples)"
        )));
    }
    sqlx::query(&format!("VACUUM INTO '{ruta_copia}'"))
        .execute(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoEscribe(ruta_copia.clone(), e))?;
    cx.close().await.ok();

    let copiadas = copiar_arbol(Path::new(dir_imagenes), &destino.join("imagenes"))?;

    // El ARCHIVO BAJO CUSTODIA de las supresiones (#758): es la ÚNICA copia del
    // expediente de un paciente ya purgado, así que un respaldo sin él restauraría
    // certificados que apuntan a datos irrecuperables (concilio 2026-09-22). Se copia
    // como una superficie de disco más; `verificar` comprueba que cada archivo que la
    // tabla `supresion` nombra esté aquí con su sha256.
    if let Some(da) = dir_archivo {
        copiar_arbol(Path::new(da), &destino.join("custodia"))?;
    }

    // Y ahora se comprueba. Un respaldo que no se abre no es un respaldo.
    let resumen = verificar(destino, Some(&filas_origen), Some(&declaradas)).await?;

    std::fs::write(
        destino.join("RESPALDO.txt"),
        format!(
            "Respaldo de medico\n\
             fecha (UTC): {}\n\
             origen: {}\n\
             imágenes en disco: {}\n\
             {}\n\
             Verificado al crearse: integrity_check, recuentos por tabla y \n\
             presencia de cada imagen que la base nombra.\n\n\
             Restaurar:  medico-api restaurar {}\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            origen_bd.display(),
            copiadas,
            resumen
                .filas
                .iter()
                .map(|(t, n)| format!("{t}: {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
            destino.display(),
        ),
    )?;

    Ok(Resumen { imagenes: copiadas, ..resumen })
}

/// Abre un respaldo y comprueba que es fiel. Si se le pasan los recuentos y las
/// imágenes del origen, además compara; si no, verifica lo que puede por sí solo
/// (integridad y que cada imagen nombrada esté presente).
///
/// Se usa en dos momentos: al crear el respaldo y antes de restaurarlo. Es a
/// propósito la MISMA función: comprobar la restauración con un criterio más
/// flojo que el de la copia dejaría pasar justo lo que se degradó en el medio.
pub async fn verificar(
    carpeta: &Path,
    filas_origen: Option<&[(String, i64)]>,
    imagenes_origen: Option<&[(i64, String)]>,
) -> Result<Resumen, ErrorRespaldo> {
    let bd = carpeta.join("medico.db");
    if !bd.exists() {
        return Err(ErrorRespaldo::NoEsRespaldo(
            carpeta.display().to_string(),
            "medico.db",
        ));
    }

    let mut fallos = Vec::new();
    let mut cx = conectar(&bd).await?;

    let integridad: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(bd.display().to_string(), e))?;
    if integridad != "ok" {
        fallos.push(format!("  integrity_check dice: {integridad}"));
    }

    let filas = recuentos(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(bd.display().to_string(), e))?;
    if let Some(esperadas) = filas_origen {
        for ((tabla, copia), (_, origen)) in filas.iter().zip(esperadas.iter()) {
            if copia != origen {
                fallos.push(format!(
                    "  {tabla}: el origen tiene {origen} filas y la copia {copia}"
                ));
            }
        }
    }

    // Cada imagen que la base NOMBRA tiene que estar. Se mira en la copia, y se
    // comprueba contra lo que declaraba el origen cuando se sabe: si una fila
    // perdió su ruta por el camino, comparar solo la copia consigo misma no lo
    // vería.
    let declaradas_copia = imagenes_declaradas(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(bd.display().to_string(), e))?;
    if let Some(origen) = imagenes_origen
        && origen.len() != declaradas_copia.len()
    {
        fallos.push(format!(
            "  el origen declaraba {} imágenes y la copia declara {}",
            origen.len(),
            declaradas_copia.len()
        ));
    }
    let mut ausentes = Vec::new();
    for (id, ruta) in &declaradas_copia {
        if !carpeta.join("imagenes").join(ruta).exists() {
            ausentes.push(format!("imagen {id}: {ruta}"));
        }
    }
    if !ausentes.is_empty() {
        fallos.push(format!(
            "  {} imágenes que la base nombra NO están en el respaldo:\n    {}",
            ausentes.len(),
            ausentes.join("\n    ")
        ));
    }

    // El ARCHIVO BAJO CUSTODIA de cada supresión (#758): la fila `supresion` nombra
    // `ruta_archivo` y su `sha256_archivo`, y ese blob es la ÚNICA copia del
    // expediente purgado. Tiene que estar en `custodia/` con su hash intacto, o el
    // respaldo restauraría un certificado que apunta a datos irrecuperables.
    let hay_supresion: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='supresion'",
    )
    .fetch_one(&mut cx)
    .await
    .map_err(|e| ErrorRespaldo::NoAbre(bd.display().to_string(), e))?;
    if hay_supresion > 0 {
        let sups: Vec<(String, String)> =
            sqlx::query_as("SELECT ruta_archivo, sha256_archivo FROM supresion")
                .fetch_all(&mut cx)
                .await
                .map_err(|e| ErrorRespaldo::NoAbre(bd.display().to_string(), e))?;
        let mut malos = Vec::new();
        for (ruta, sha) in &sups {
            match std::fs::read(carpeta.join("custodia").join(ruta)) {
                Ok(bytes) if sha256_hex(&bytes) == *sha => {}
                Ok(_) => malos.push(format!("{ruta}: sha256 no casa")),
                Err(_) => malos.push(format!("{ruta}: ausente en la custodia del respaldo")),
            }
        }
        if !malos.is_empty() {
            fallos.push(format!(
                "  {} archivo(s) bajo custodia que la base nombra faltan o cambiaron:\n    {}",
                malos.len(),
                malos.join("\n    ")
            ));
        }
    }
    cx.close().await.ok();

    if !fallos.is_empty() {
        return Err(ErrorRespaldo::NoVerifica(fallos));
    }

    Ok(Resumen {
        filas,
        imagenes: declaradas_copia.len(),
        bytes_bd: std::fs::metadata(&bd)?.len(),
    })
}

/// `medico-api restaurar <carpeta>`: pone el respaldo en su sitio.
///
/// Verifica ANTES de tocar nada, y se niega si el destino ya tiene datos salvo
/// que se le insista. Restaurar es lo que se hace el peor día; que ese día haya
/// que descubrir que el respaldo estaba mal es exactamente el fallo que este
/// módulo existe para no tener.
pub async fn restaurar(
    carpeta: &Path,
    url_bd: &str,
    dir_imagenes: &str,
    dir_archivo: Option<&str>,
    forzar: bool,
) -> Result<Resumen, ErrorRespaldo> {
    let resumen = verificar(carpeta, None, None).await?;

    // #758: si el respaldo TRAE custodia (blobs de supresiones) pero no se dio
    // `MEDICO_RETENCION_DIR`, restaurar la dejaría fuera EN SILENCIO — la base
    // restaurada tendría certificados apuntando a blobs no colocados, la única copia
    // del expediente purgado. Simétrico con `respaldar`, que se niega igual (concilio
    // 2026-09-22, fable). Se comprueba ANTES de tocar el destino.
    let custodia = carpeta.join("custodia");
    let hay_custodia = std::fs::read_dir(&custodia).is_ok_and(|mut d| d.next().is_some());
    if hay_custodia && dir_archivo.is_none() {
        return Err(ErrorRespaldo::NoVerifica(vec![
            "  el respaldo trae archivos bajo custodia (#758) pero no se indicó \
             MEDICO_RETENCION_DIR; restaurar los perdería. Configure MEDICO_RETENCION_DIR \
             y reintente."
                .to_string(),
        ]));
    }

    let destino_bd = ruta_de_url(url_bd);
    if destino_bd.exists() && !forzar {
        let mut cx = conectar(&destino_bd).await?;

        // Dos situaciones que se parecen y no lo son:
        //
        //   (a) la base de destino no tiene ni el esquema —máquina nueva, no hay
        //       nada que perder, adelante;
        //   (b) la base existe pero NO SE PUEDE LEER —tomada por otro proceso,
        //       corrupta, sin permisos.
        //
        // «No pude mirar» contabilizado como «no hay nada» es la forma más cara
        // de perder datos, porque parece que funcionó (hallazgo de la revisión de
        // informes). Se distinguen preguntando primero si las tablas EXISTEN. Si
        // no existen, es (a). Si existen, el recuento tiene que salir bien:
        // cualquier error se propaga y no se restaura nada.
        let con_esquema: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name IN ('paciente', 'historia')",
        )
        .fetch_one(&mut cx)
        .await
        .map_err(|e| ErrorRespaldo::NoAbre(destino_bd.display().to_string(), e))?;

        if con_esquema > 0 {
            let pacientes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM paciente")
                .fetch_one(&mut cx)
                .await
                .map_err(|e| ErrorRespaldo::NoAbre(destino_bd.display().to_string(), e))?;
            let historias: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia")
                .fetch_one(&mut cx)
                .await
                .map_err(|e| ErrorRespaldo::NoAbre(destino_bd.display().to_string(), e))?;
            cx.close().await.ok();
            if pacientes > 0 || historias > 0 {
                return Err(ErrorRespaldo::DestinoConDatos(pacientes, historias));
            }
        } else {
            cx.close().await.ok();
        }
    }

    // GUARDA de restauración (concilio 2026-09-26 H3; REBASADA al namespace por instancia,
    // #758, 5º concilio 2026-09-27): `restaurar` reemplaza `medico.db` ENTERA —la identidad
    // de la instancia viaja con ella— y copia el árbol de imágenes. Antes de TOCAR NADA (la
    // base incluida), si el destino de imágenes contiene PHI de OTRA instancia se RECHAZA
    // fail-closed: reemplazar la base dejaría esa PHI ajena huérfana en su subcarpeta
    // `{uuid}/`. La identidad entrante se lee del RESPALDO (aún no se ha copiado nada), y
    // `phi_de_otra_instancia` la compara contra las subcarpetas del destino (y trata la PHI
    // legada plana o ilegible como ajena). Reemplaza el sello `.instancia`, que este mismo
    // rediseño retiró: la guarda ya no depende de un marcador que nadie escribe.
    let imagenes_origen = carpeta.join("imagenes");
    let destino_img = Path::new(dir_imagenes);
    let entrante = uuid_de_base(&carpeta.join("medico.db")).await;
    if let Some(existente) = phi_de_otra_instancia(destino_img, entrante.as_deref()) {
        return Err(ErrorRespaldo::DestinoImagenesDeOtraInstancia {
            existente,
            entrante: entrante.unwrap_or_else(|| "(respaldo sin identidad de instancia)".into()),
        });
    }

    if let Some(padre) = destino_bd.parent()
        && !padre.as_os_str().is_empty()
    {
        std::fs::create_dir_all(padre)?;
    }
    std::fs::copy(carpeta.join("medico.db"), &destino_bd)?;
    // El WAL y el índice del origen NO se conservan: la copia de `VACUUM INTO` ya
    // los tiene integrados, y dejar un WAL viejo junto a una base nueva es pedirle
    // a SQLite que reproduzca transacciones de otra base.
    for sufijo in ["-wal", "-shm"] {
        let mut ruta = destino_bd.clone().into_os_string();
        ruta.push(sufijo);
        std::fs::remove_file(PathBuf::from(ruta)).ok();
    }

    if imagenes_origen.exists() {
        std::fs::create_dir_all(dir_imagenes)?;
        copiar_arbol(&imagenes_origen, destino_img)?;
    }

    // El archivo bajo custodia (#758): se restaura a su carpeta si el respaldo lo
    // trae y hay dónde ponerlo. `verificar` (arriba) ya exigió que cada blob que la
    // base nombra estuviera presente con su sha256, así que aquí solo se coloca.
    let custodia_origen = carpeta.join("custodia");
    if let Some(da) = dir_archivo
        && custodia_origen.exists()
    {
        std::fs::create_dir_all(da)?;
        copiar_arbol(&custodia_origen, Path::new(da))?;
    }

    Ok(resumen)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tablas que el esquema tiene y que a propósito NO se comparan.
    ///
    /// Vive aquí y no junto a `TABLAS` porque solo la usa la prueba que audita
    /// esa lista; puesta arriba sería código muerto en el binario.
    const TABLAS_EXENTAS: &[&str] = &[
        // La lleva sqlx para saber qué migraciones aplicó. Viaja en la copia como
        // parte de la base, pero su recuento no dice nada del estado del cliente.
        "_sqlx_migrations",
        // El rastro del freno de login y su reposición: se escriben en CADA
        // intento de acceso, así que basta con que alguien (o un robot) lo intente
        // entre la lectura de los recuentos y el `VACUUM INTO` para que origen y
        // copia difieran y el respaldo nocturno se declarara malo sin que nada
        // estuviera mal. No hay estado del cliente que restaurar aquí.
        "intento_acceso",
        "reposicion_freno",
        // La bitácora de acceso a dato de paciente (Ley 1581) es append-only y se
        // escribe en cada lectura de historia, el más alto de todos: comparar su
        // recuento haría fallar el respaldo por funcionar. Su integridad no la da
        // el conteo sino su propia cadena por hash (`medico_datos::acceso`), y
        // viaja entera en la copia como parte de la base.
        "acceso",
    ];

    /// Una base con esquema, una institución, un paciente, una historia y una
    /// imagen con su archivo en disco.
    async fn base_de_prueba(dir: &Path) -> (String, String) {
        let url = format!("sqlite://{}?mode=rwc", dir.join("medico.db").display());
        let pool = medico_datos::abrir(&url).await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol) \
             VALUES (1,'Dra','d@x.co','x','MEDICO')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente \
             (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) \
             VALUES (1,'CC','123','Ana','Gómez','1990-01-01','F',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO historia \
             (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,\
              diagnostico_principal,plan_tratamiento) \
             VALUES (10,'H-1',1,1,'dolor','cuadro','J00','reposo')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO imagen_diagnostica \
             (id,historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes,origen) \
             VALUES (5,10,'5.dat','placa.png','image/png',9,'adjunto_historia')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let imagenes = dir.join("storage").join("imagenes");
        std::fs::create_dir_all(&imagenes).unwrap();
        std::fs::write(imagenes.join("5.dat"), b"contenido").unwrap();
        (url, imagenes.display().to_string())
    }

    #[tokio::test]
    async fn el_respaldo_se_verifica_y_trae_las_imagenes() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");

        let r = respaldar(&url, &imagenes, None, &destino).await.unwrap();

        assert_eq!(r.imagenes, 1);
        assert_eq!(r.filas.iter().find(|(t, _)| t == "paciente").unwrap().1, 1);
        assert!(destino.join("imagenes").join("5.dat").exists());
        assert!(destino.join("RESPALDO.txt").exists());
    }

    #[tokio::test]
    async fn una_imagen_que_falta_impide_declarar_bueno_el_respaldo() {
        // La prueba que de verdad importa: el respaldo tiene que DISCRIMINAR. Sin
        // esto, «verificado» sería una palabra escrita en un `println!`.
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        std::fs::remove_file(destino.join("imagenes").join("5.dat")).unwrap();

        let e = verificar(&destino, None, None).await.unwrap_err();
        let texto = e.to_string();
        assert!(texto.contains("5.dat"), "el fallo no nombra el archivo: {texto}");
        assert!(texto.contains("imagen 5"), "el fallo no nombra la imagen: {texto}");
    }

    /// #758: el archivo bajo custodia (única copia del expediente purgado) entra en
    /// el respaldo y se verifica por sha256; sin él, el respaldo SE NIEGA a declararse
    /// bueno (concilio 2026-09-22, fable/sonnet). Vía real por `respaldar`/`verificar`.
    #[tokio::test]
    async fn el_respaldo_incluye_y_verifica_la_custodia() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let custodia = tmp.path().join("custodia");
        std::fs::create_dir_all(&custodia).unwrap();
        let contenido = b"expediente cifrado bajo custodia";
        let sha = sha256_hex(contenido);
        std::fs::write(custodia.join("supresion-1.blob"), contenido).unwrap();
        let pool = medico_datos::abrir(&url).await.unwrap();
        sqlx::query(
            "INSERT INTO supresion \
             (paciente_id,institucion_id,fecha_defuncion,fecha_vencimiento,fecha_supresion,\
              ruta_archivo,sha256_archivo,manifiesto,firma,recuentos) \
             VALUES (1,1,'2005-01-01T00:00:00Z','2025-01-01T00:00:00Z','2025-02-01T00:00:00Z',\
                     'supresion-1.blob',?,'{}','f','{}')",
        )
        .bind(&sha)
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        // Con la custodia: el blob entra en la copia y el respaldo se declara bueno.
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, Some(custodia.to_str().unwrap()), &destino).await.unwrap();
        assert!(
            destino.join("custodia").join("supresion-1.blob").exists(),
            "el blob bajo custodia debe entrar en el respaldo"
        );

        // Negativo (D33): si el blob desaparece de la copia, verificar lo caza por sha256.
        std::fs::remove_file(destino.join("custodia").join("supresion-1.blob")).unwrap();
        let e = verificar(&destino, None, None).await.unwrap_err();
        assert!(e.to_string().contains("supresion-1.blob"), "el fallo debe nombrar el blob: {e}");

        // Y respaldar SIN pasar la custodia, habiendo supresiones, se NIEGA (era el bug).
        let destino2 = tmp.path().join("copia2");
        let err = respaldar(&url, &imagenes, None, &destino2).await.unwrap_err();
        assert!(
            err.to_string().contains("custodia") || err.to_string().contains("supresion-1.blob"),
            "respaldar sin custodia con supresiones debe fallar: {err}"
        );

        // H2-bis (concilio 4b, fable): RESTAURAR un respaldo que trae custodia SIN dar
        // MEDICO_RETENCION_DIR se NIEGA (simétrico con respaldar), en vez de restaurar
        // y dejar la única copia fuera en silencio. Respaldo FRESCO con custodia intacta
        // (`destino` de arriba ya perdió su blob en la prueba negativa).
        let destino3 = tmp.path().join("copia3");
        respaldar(&url, &imagenes, Some(custodia.to_str().unwrap()), &destino3).await.unwrap();
        let destino_bd = tmp.path().join("restaurada.db");
        let url_r = format!("sqlite://{}?mode=rwc", destino_bd.display());
        let imgs_r = tmp.path().join("imgs-restauradas").display().to_string();
        let err2 = restaurar(&destino3, &url_r, &imgs_r, None, false).await.unwrap_err();
        assert!(
            err2.to_string().contains("custodia") || err2.to_string().contains("MEDICO_RETENCION_DIR"),
            "restaurar sin custodia con supresiones debe negarse: {err2}"
        );
        assert!(!destino_bd.exists(), "no debe haber tocado el destino al negarse");
        // Con la custodia, la restauración completa sí procede y coloca los blobs.
        let cust_r = tmp.path().join("custodia-restaurada");
        restaurar(&destino3, &url_r, &imgs_r, Some(cust_r.to_str().unwrap()), false).await.unwrap();
        assert!(cust_r.join("supresion-1.blob").exists(), "el blob se coloca al restaurar con custodia");
    }

    #[tokio::test]
    async fn una_base_corrompida_no_pasa_la_verificacion() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        // Se pisan las páginas del medio: el encabezado sigue diciendo «soy
        // SQLite», así que la base ABRE. Es justo el respaldo que parece bueno.
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(destino.join("medico.db"))
            .unwrap();
        f.seek(SeekFrom::Start(4096)).unwrap();
        f.write_all(&[0xFF; 4096]).unwrap();
        f.sync_all().unwrap();
        drop(f);

        assert!(
            verificar(&destino, None, None).await.is_err(),
            "una base con las páginas pisadas pasó por buena"
        );
    }

    #[tokio::test]
    async fn restaurar_no_pisa_una_base_con_datos() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        let e = restaurar(&destino, &url, &imagenes, None, false).await.unwrap_err();
        assert!(matches!(e, ErrorRespaldo::DestinoConDatos(1, 1)), "{e}");
    }

    /// RED (guarda de restauración, rebasada al namespace #758): restaurar sobre una carpeta
    /// que contiene la PHI de OTRA instancia (subcarpeta `{uuid_ajeno}/` con `.dat`) se
    /// RECHAZA, sin pisarla. `restaurar` reemplaza medico.db (la identidad), así que dejar
    /// esa base ajena sin identidad y su PHI huérfana es el daño. Quitar la guarda (o
    /// hacerla no recursiva) deja pasar la restauración → esta prueba falla.
    #[tokio::test]
    async fn restaurar_no_pisa_las_imagenes_de_otra_instancia() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        // Destino: BD nueva (para no chocar con DestinoConDatos) y una carpeta de imágenes
        // con la PHI VIVA de OTRA instancia en SU subcarpeta (layout de namespace real).
        std::fs::create_dir_all(tmp.path().join("nueva")).unwrap();
        let url_nueva =
            format!("sqlite://{}?mode=rwc", tmp.path().join("nueva/medico.db").display());
        let img_ajena = tmp.path().join("img-de-B");
        let sub_b = img_ajena.join("bbbb1111bbbb2222bbbb3333bbbb4444");
        std::fs::create_dir_all(&sub_b).unwrap();
        std::fs::write(sub_b.join("9.dat"), b"phi-de-B-viva").unwrap();

        let e = restaurar(&destino, &url_nueva, &img_ajena.display().to_string(), None, false)
            .await
            .unwrap_err();
        assert!(matches!(e, ErrorRespaldo::DestinoImagenesDeOtraInstancia { .. }), "{e}");
        assert!(sub_b.join("9.dat").exists(), "la PHI viva de B (subcarpeta ajena) no se tocó");
    }

    /// RED (fail-closed): restaurar sobre una carpeta con PHI LEGADA plana (`{id}.dat` en la
    /// raíz, sin migrar) se RECHAZA — su origen no es atribuible a la instancia entrante, así
    /// que se trata como ajena. **MUTANTE que discrimina (D33):** una `phi_de_otra_instancia`
    /// que ignorara los `.dat` planos de la raíz dejaría pasar la restauración y pisaría la
    /// PHI legada. Verificado por ejecución.
    #[tokio::test]
    async fn restaurar_no_pisa_phi_legada_plana_de_origen_desconocido() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        let dir_nueva = tmp.path().join("nueva3");
        std::fs::create_dir_all(&dir_nueva).unwrap();
        let url_nueva =
            format!("sqlite://{}?mode=rwc", dir_nueva.join("medico.db").display());
        // Carpeta con PHI legada plana (layout viejo, no migrado): origen desconocido.
        let img_viva = tmp.path().join("img-legada-plana");
        std::fs::create_dir_all(&img_viva).unwrap();
        std::fs::write(img_viva.join("9.dat"), b"phi-legada-plana").unwrap();

        let e = restaurar(&destino, &url_nueva, &img_viva.display().to_string(), None, false)
            .await
            .unwrap_err();
        assert!(matches!(e, ErrorRespaldo::DestinoImagenesDeOtraInstancia { .. }), "{e}");
        assert_eq!(
            std::fs::read_to_string(img_viva.join("9.dat")).unwrap(),
            "phi-legada-plana",
            "la PHI legada plana no se pisó",
        );
    }

    /// VERDE: restaurar sobre una carpeta que solo contiene la PHI de la MISMA instancia del
    /// respaldo (su subcarpeta `{uuid}/`) PROCEDE — la guarda no bloquea el re-restore
    /// legítimo sobre la propia carpeta.
    #[tokio::test]
    async fn restaurar_sobre_la_propia_carpeta_procede() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        // uuid del respaldo (viaja en medico.db).
        let mut cx = conectar(&destino.join("medico.db")).await.unwrap();
        let uuid: String = sqlx::query_scalar("SELECT uuid FROM instancia WHERE id = 1")
            .fetch_one(&mut cx)
            .await
            .unwrap();
        cx.close().await.ok();

        std::fs::create_dir_all(tmp.path().join("nueva2")).unwrap();
        let url_nueva =
            format!("sqlite://{}?mode=rwc", tmp.path().join("nueva2/medico.db").display());
        let img = tmp.path().join("img-propia");
        // PHI de la PROPIA instancia entrante, en su subcarpeta {uuid}/ (layout real).
        std::fs::create_dir_all(img.join(&uuid)).unwrap();
        std::fs::write(img.join(&uuid).join("7.dat"), b"phi-propia").unwrap();

        restaurar(&destino, &url_nueva, &img.display().to_string(), None, false)
            .await
            .unwrap();
        assert!(img.join("5.dat").exists(), "la imagen del respaldo se restauró");
    }

    /// RED (6º concilio, sonnet conf 10, por ejecución): una subcarpeta ajena presentada como
    /// SYMLINK no bypasea la guarda. `file_type()` no sigue symlinks, así que sin la rama
    /// explícita se ignoraría y `restaurar` procedería sobre PHI ajena. Fail-closed: rechaza.
    #[tokio::test]
    #[cfg(unix)]
    async fn restaurar_no_pisa_phi_ajena_presentada_como_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        std::fs::create_dir_all(tmp.path().join("nueva4")).unwrap();
        let url_nueva =
            format!("sqlite://{}?mode=rwc", tmp.path().join("nueva4/medico.db").display());
        // PHI ajena real en un dir externo, y en dir_imagenes un SYMLINK (nombrado como un
        // uuid ajeno) que apunta a ella: el bypass que el 6º concilio verificó por ejecución.
        let phi_real = tmp.path().join("phi-de-B-real");
        std::fs::create_dir_all(&phi_real).unwrap();
        std::fs::write(phi_real.join("9.dat"), b"phi-de-B-viva").unwrap();
        let img = tmp.path().join("img-con-symlink");
        std::fs::create_dir_all(&img).unwrap();
        std::os::unix::fs::symlink(&phi_real, img.join("bbbb1111bbbb2222bbbb3333bbbb4444")).unwrap();

        let e = restaurar(&destino, &url_nueva, &img.display().to_string(), None, false)
            .await
            .unwrap_err();
        assert!(matches!(e, ErrorRespaldo::DestinoImagenesDeOtraInstancia { .. }), "{e}");
        assert!(phi_real.join("9.dat").exists(), "la PHI de B (por symlink) no se tocó");
    }

    /// RED (fable, 6º concilio): una subcarpeta ajena con contenido que NO es `.dat` igual es
    /// PHI ajena a proteger — la guarda mira CUALQUIER contenido de una subcarpeta de otro
    /// uuid, no solo `.dat`. Con la versión que solo miraba `.dat` esto procedería (verde falso).
    #[tokio::test]
    async fn restaurar_no_pisa_subcarpeta_ajena_con_contenido_no_dat() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        std::fs::create_dir_all(tmp.path().join("nueva5")).unwrap();
        let url_nueva =
            format!("sqlite://{}?mode=rwc", tmp.path().join("nueva5/medico.db").display());
        let img = tmp.path().join("img-ajena-no-dat");
        let sub = img.join("cccc1111cccc2222cccc3333cccc4444");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("9.dat.tmp"), b"phi-en-vuelo").unwrap(); // no termina en .dat

        let e = restaurar(&destino, &url_nueva, &img.display().to_string(), None, false)
            .await
            .unwrap_err();
        assert!(matches!(e, ErrorRespaldo::DestinoImagenesDeOtraInstancia { .. }), "{e}");
        assert!(sub.join("9.dat.tmp").exists(), "la PHI ajena no-.dat no se tocó");
    }

    #[tokio::test]
    async fn una_base_de_destino_ilegible_no_cuenta_como_vacia() {
        // El hallazgo de la revisión de seguridad de informes. El recuento del
        // destino se leía con `.unwrap_or(0)`, así que una base tomada, corrupta o
        // sin permisos daba «cero filas» y la restauración la pisaba. «No pude
        // mirar» contabilizado como «no hay nada» es la forma más cara de perder
        // datos: parece que funcionó.
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        use std::io::{Seek, SeekFrom, Write};
        let bd = ruta_de_url(&url);
        let mut f = std::fs::OpenOptions::new().write(true).open(&bd).unwrap();
        f.seek(SeekFrom::Start(4096)).unwrap();
        f.write_all(&[0xFF; 8192]).unwrap();
        f.sync_all().unwrap();
        drop(f);

        let r = restaurar(&destino, &url, &imagenes, None, false).await;
        assert!(
            r.is_err(),
            "restauró sobre una base que no se pudo leer: eso la habría borrado"
        );
    }

    #[tokio::test]
    async fn restaurar_en_una_maquina_nueva_sigue_funcionando() {
        // El reverso de la anterior, para que la corrección no cierre de más: una
        // base de destino que existe pero AÚN NO TIENE ESQUEMA es el caso normal
        // de instalar en una máquina nueva, y ahí no hay nada que perder.
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        let bd = ruta_de_url(&url);
        std::fs::remove_file(&bd).unwrap();
        std::fs::File::create(&bd).unwrap();
        // Una máquina NUEVA no tiene imágenes previas: la carpeta de destino está vacía
        // (las de `base_de_prueba` eran un artefacto del arnés). La guarda R2/R3 rechaza
        // pisar una carpeta con PHI ajena SIN sellar; una carpeta vacía no es ese caso.
        std::fs::remove_dir_all(&imagenes).ok();

        restaurar(&destino, &url, &imagenes, None, false)
            .await
            .expect("una base sin esquema debe poder restaurarse sin --forzar");
    }

    #[tokio::test]
    async fn restaurar_deja_el_sistema_como_estaba() {
        // La vuelta completa, que es lo único que prueba que el respaldo SIRVE: se
        // respalda, se destruye el original, se restaura, y se comprueba que están
        // las filas Y la imagen.
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        respaldar(&url, &imagenes, None, &destino).await.unwrap();

        let bd = ruta_de_url(&url);
        std::fs::remove_file(&bd).unwrap();
        std::fs::remove_dir_all(&imagenes).unwrap();

        let r = restaurar(&destino, &url, &imagenes, None, false).await.unwrap();
        assert_eq!(r.filas.iter().find(|(t, _)| t == "paciente").unwrap().1, 1);

        let mut cx = conectar(&bd).await.unwrap();
        let pacientes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM paciente")
            .fetch_one(&mut cx)
            .await
            .unwrap();
        assert_eq!(pacientes, 1);
        assert_eq!(
            std::fs::read_to_string(Path::new(&imagenes).join("5.dat")).unwrap(),
            "contenido"
        );
    }

    #[tokio::test]
    async fn no_se_respalda_sobre_una_carpeta_ocupada() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, imagenes) = base_de_prueba(tmp.path()).await;
        let destino = tmp.path().join("copia");
        std::fs::create_dir_all(&destino).unwrap();
        std::fs::write(destino.join("otra-cosa.txt"), b"x").unwrap();

        assert!(matches!(
            respaldar(&url, &imagenes, None, &destino).await.unwrap_err(),
            ErrorRespaldo::DestinoOcupado(_)
        ));
    }

    #[tokio::test]
    async fn las_tablas_del_esquema_estan_todas() {
        // El vigilante también se audita: si alguien añade una tabla y no la mete
        // en TABLAS, el respaldo diría «verificado» sin haberla mirado.
        let tmp = tempfile::tempdir().unwrap();
        let (url, _) = base_de_prueba(tmp.path()).await;
        let mut cx = conectar(&ruta_de_url(&url)).await.unwrap();
        let filas = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(&mut cx)
        .await
        .unwrap();

        let del_esquema: Vec<String> = filas.iter().map(|f| f.get::<String, _>("name")).collect();
        let sin_comprobar: Vec<&String> = del_esquema
            .iter()
            .filter(|t| !TABLAS.contains(&t.as_str()) && !TABLAS_EXENTAS.contains(&t.as_str()))
            .collect();
        assert!(
            sin_comprobar.is_empty(),
            "tablas que el respaldo no compara: {sin_comprobar:?}. \
             Añádalas a TABLAS o a TABLAS_EXENTAS con su razón."
        );
    }

    #[test]
    fn la_url_se_convierte_en_ruta() {
        assert_eq!(ruta_de_url("sqlite://medico.db"), PathBuf::from("medico.db"));
        assert_eq!(ruta_de_url("sqlite:/tmp/a.db"), PathBuf::from("/tmp/a.db"));
        assert_eq!(ruta_de_url("medico.db?mode=rwc"), PathBuf::from("medico.db"));
    }
}
