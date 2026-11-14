//! Imágenes diagnósticas de una historia. El archivo vive en disco; aquí solo
//! va la fila con su referencia. Aislamiento por la cadena
//! historia → paciente → institución, igual que órdenes y fórmulas.
//!
//! La `ruta_archivo` la fija SIEMPRE este módulo desde el id de la fila —nunca
//! el cliente—, así que el nombre original que sube el usuario no puede escapar
//! de la carpeta ni pisar otro archivo.

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Imagen {
    pub id: i64,
    pub historia_id: i64,
    pub ruta_archivo: String,
    pub nombre_original: String,
    pub tipo_mime: String,
    pub tamano_bytes: i64,
    pub descripcion: String,
    pub fecha_subida: String,
    /// El archivo en disco está cifrado (blob de guaca) y hay que descifrarlo
    /// para servirlo. `false` = archivo en claro (histórico o sin clave de reposo).
    pub cifrada: bool,
    /// `adjunto_historia` o `resultado_imagenologia` (migración `0056`).
    ///
    /// **Sale en la lectura porque la pantalla tiene que poder distinguirlos**:
    /// el primero es contenido del documento y entra en lo que sella la firma;
    /// el segundo llega después y no. Sin este campo los dos se veían idénticos
    /// bajo «Adjuntos de la atención», así que ni el clínico ni un auditor
    /// podían saber qué cubre la firma de esa historia.
    pub origen: String,
}

impl Imagen {
    /// ¿Este archivo entra en lo que sella la firma de la historia?
    pub fn sellado_con_la_firma(&self) -> bool {
        self.origen == "adjunto_historia"
    }
}

/// Lo que aporta el cliente. El tamaño y el mime los mide el servidor sobre los
/// bytes recibidos, no se los cree a la petición.
#[derive(Debug, Clone, Deserialize)]
pub struct ImagenNueva {
    pub nombre_original: String,
    pub tipo_mime: String,
    pub tamano_bytes: i64,
    #[serde(default)]
    pub descripcion: String,
}

const COLUMNAS: &str = "id, historia_id, ruta_archivo, nombre_original, tipo_mime, \
                        tamano_bytes, descripcion, fecha_subida, cifrada, origen";

async fn historia_visible(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM historia h JOIN paciente p ON p.id = h.paciente_id WHERE h.id = ?",
    )
    .bind(historia_id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

async fn imagen_visible(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<Imagen>, sqlx::Error> {
    let img: Option<Imagen> = sqlx::query_as::<_, Imagen>(&format!(
        "SELECT {COLUMNAS} FROM imagen_diagnostica WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some(img) = img else { return Ok(None) };
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM historia h JOIN paciente p ON p.id = h.paciente_id WHERE h.id = ?",
    )
    .bind(img.historia_id)
    .fetch_optional(pool)
    .await?;
    if fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si) {
        Ok(Some(img))
    } else {
        Ok(None)
    }
}

pub async fn listar_por_historia(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<Vec<Imagen>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Imagen>(&format!(
        "SELECT {COLUMNAS} FROM imagen_diagnostica WHERE historia_id = ? ORDER BY id"
    ))
    .bind(historia_id)
    .fetch_all(pool)
    .await
}

/// Registra la fila y le fija la ruta desde su propio id. Devuelve la `Imagen`
/// completa —con la `ruta_archivo` que el llamante usará para escribir los
/// bytes— o `None` si la historia no es del ámbito. Escribir el archivo es
/// responsabilidad del llamante; esta función solo toca la base.
/// De dónde viene el archivo. **Son dos cosas distintas** y la diferencia
/// decide dos comportamientos: si se puede subir a una historia firmada, y si
/// entra en lo que la firma sella (migración `0056`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    /// El médico lo añade mientras escribe la historia. Es contenido del
    /// documento: se sella con la firma y no se toca una historia firmada.
    AdjuntoDeLaHistoria,
    /// El servicio de imagenología lo sube DESPUÉS, contra una orden. No es
    /// contenido del documento firmado: llega por el flujo operativo posterior,
    /// igual que cerrar una orden de laboratorio, y lo normal es que la consulta
    /// ya esté firmada cuando llega (directiva 21).
    ResultadoDeImagenologia,
}

impl Origen {
    pub fn como_str(self) -> &'static str {
        match self {
            Origen::AdjuntoDeLaHistoria => "adjunto_historia",
            Origen::ResultadoDeImagenologia => "resultado_imagenologia",
        }
    }
}

/// El uuid de ESTA base (tabla `instancia`, migración 0077): identidad estable entre
/// arranques, sembrada con `randomblob` al migrar. Desde el rediseño del fail-safe #2 de
/// #758 (2026-09-27) es el NAMESPACE de la carpeta de imágenes —cada archivo vive en
/// `{uuid}/{id}.dat`—, lo que aísla el disco por instancia sin sello ni ventana (D1).
pub async fn uuid_de_instancia(pool: &crate::Pool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT uuid FROM instancia WHERE id = 1")
        .fetch_one(pool)
        .await
}

/// Migra los archivos de imagen del layout plano legado (`dir/{id}.dat`) al namespace
/// por instancia (`dir/{uuid}/{id}.dat`, #758). Idempotente: una fila ya migrada
/// (`ruta_archivo` con `/`) se salta. La ruta nueva se reconstruye del `id` (no del texto
/// viejo), y `ruta_archivo` se ACTUALIZA solo si el archivo pudo moverse —o si ya no había
/// archivo físico que mover (subida fallida histórica: re-apuntar es inocuo)—; si el `move`
/// falla, la fila queda en el layout viejo y se reintenta el próximo arranque. Corre al
/// arranque, ANTES de servir o reconciliar. Devuelve cuántas filas migró.
///
/// Solo toca `{id}.dat` que la BASE conoce como propios (por fila); NO barre el directorio,
/// así que no puede mover un archivo ajeno. Un huérfano legado en la raíz (sin fila, de una
/// purga interrumpida ANTES de este cambio) no se toca aquí: es PHI ya no referenciada, un
/// caso muy raro que se limpia a mano —moverlo a ciegas es justo el borrado cruzado que el
/// namespace evita (D20: no adivinar de quién es un archivo sin fila en la raíz compartida)—.
pub async fn migrar_a_namespace_por_instancia(
    pool: &crate::Pool,
    dir_imagenes: &std::path::Path,
) -> Result<usize, sqlx::Error> {
    let uuid = uuid_de_instancia(pool).await?;
    // Layout legado = `ruta_archivo` sin `/` (los nuevos son "{uuid}/{id}.dat").
    let legadas: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, ruta_archivo FROM imagen_diagnostica \
         WHERE ruta_archivo <> '' AND ruta_archivo NOT LIKE '%/%'",
    )
    .fetch_all(pool)
    .await?;
    if legadas.is_empty() {
        return Ok(0);
    }
    let subdir = dir_imagenes.join(&uuid);
    let mut migradas = 0;
    for (id, ruta_vieja) in legadas {
        let nueva_ruta = format!("{uuid}/{id}.dat");
        let origen = dir_imagenes.join(&ruta_vieja);
        let destino = dir_imagenes.join(&nueva_ruta);
        if std::fs::create_dir_all(&subdir).is_err() {
            continue; // sin subcarpeta no se puede mover; se reintenta el próximo arranque
        }
        if origen.exists() && !destino.exists() && std::fs::rename(&origen, &destino).is_err() {
            continue; // el move falló: NO se re-apunta la fila (sigue sirviendo del layout viejo)
        }
        sqlx::query("UPDATE imagen_diagnostica SET ruta_archivo = ? WHERE id = ?")
            .bind(&nueva_ruta)
            .bind(id)
            .execute(pool)
            .await?;
        migradas += 1;
    }
    Ok(migradas)
}

pub async fn registrar(
    pool: &crate::Pool,
    ambito: Ambito,
    historia_id: i64,
    m: &ImagenNueva,
    cifrada: bool,
    origen: Origen,
) -> Result<Option<Imagen>, sqlx::Error> {
    // **El adjunto de la historia se niega sobre una firmada**: añadírselo es
    // editarla por al lado. El resultado de imagenología NO, y esa exención es
    // la misma que la de `orden::cambiar_estado` — bloquearla dejaría al
    // radiólogo sin poder subir la placa de una consulta ya firmada, que es lo
    // normal.
    //
    // `None` cubre los dos negativos —ajena y firmada— porque el llamante ya los
    // dibuja igual: en ambos casos no se adjunta.
    let veredicto = crate::historia::admite_escritura(pool, ambito, historia_id).await?;
    let admitida = match origen {
        Origen::AdjuntoDeLaHistoria => veredicto == crate::Resultado::Hecho,
        Origen::ResultadoDeImagenologia => veredicto != crate::Resultado::NoExiste,
    };
    if !admitida {
        return Ok(None);
    }
    // El uuid de ESTA base, leído antes de la transacción (la tabla `instancia` es
    // inmutable): entra en el nombre del archivo para aislar el namespace por instancia.
    let uuid = uuid_de_instancia(pool).await?;
    let mut tx = pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO imagen_diagnostica (historia_id, ruta_archivo, nombre_original, tipo_mime, tamano_bytes, descripcion, cifrada, origen) \
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(historia_id)
    .bind("") // se fija enseguida desde el id
    .bind(&m.nombre_original)
    .bind(&m.tipo_mime)
    .bind(m.tamano_bytes)
    .bind(&m.descripcion)
    .bind(cifrada)
    .bind(origen.como_str())
    .execute(&mut *tx)
    .await?;
    let id = r.last_insert_rowid();
    // Namespace por INSTANCIA (#758, rediseño del fail-safe #2, 2026-09-27): el archivo
    // vive en `{uuid}/{id}.dat`. Dos bases de uuid DISTINTO que compartan
    // `MEDICO_DIR_IMAGENES` (un cruce accidental de `DATABASE_URL`/`MEDICO_DIR_IMAGENES`)
    // no comparten namespace de archivos: cada una escribe, sirve, reconcilia y purga SOLO
    // en `dir/{su_uuid}/`, así que ninguna lee, exfiltra ni borra la PHI de otra. Reemplaza
    // de raíz al sello `.instancia` + su ventana (D1: quita la causa, no la contiene).
    // LÍMITE conocido (5º concilio 2026-09-27): el uuid se CLONA con la base, así que dos
    // CLONES vivos sobre el mismo dir SÍ comparten subcarpeta — fuera de cobertura por
    // diseño (viola «una base por instancia», D26); ver `respaldo::phi_de_otra_instancia`.
    let ruta = format!("{uuid}/{id}.dat");
    sqlx::query("UPDATE imagen_diagnostica SET ruta_archivo = ? WHERE id = ?")
        .bind(&ruta)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let img: Imagen = sqlx::query_as::<_, Imagen>(&format!(
        "SELECT {COLUMNAS} FROM imagen_diagnostica WHERE id = ?"
    ))
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Some(img))
}

/// La fila si es del ámbito, para servir sus bytes. `None` si no existe o es ajena.
pub async fn obtener(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<Imagen>, sqlx::Error> {
    imagen_visible(pool, ambito, id).await
}

/// Borra la fila si es del ámbito y devuelve su `ruta_archivo` para que el
/// llamante borre el archivo. `None` si no existe o es ajena.
/// Deshace una fila que ACABA de crearse y cuyo archivo no se pudo escribir.
///
/// NO pasa por la puerta de la historia firmada, y esa excepción es la regla:
/// esto no edita el documento, deshace una escritura propia que falló a medias.
/// Con la puerta puesta —como quedó al cerrar `eliminar` el 2026-08-10— el
/// rollback de `guardar_imagen_de` se negaba sobre una historia firmada y dejaba
/// una fila apuntando a un archivo que no existe. Lo encontró la revisión.
///
/// Toma el id que devolvió `registrar` y nada más: no hay entrada de usuario por
/// medio, así que no hay decisión de autorización que tomar aquí.
pub async fn deshacer_registro(pool: &crate::Pool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM imagen_diagnostica WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Borra un adjunto. **Se niega sobre una historia FIRMADA.**
pub async fn eliminar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<String>, sqlx::Error> {
    let Some(img) = imagen_visible(pool, ambito, id).await? else {
        return Ok(None);
    };
    let padre: Option<i64> =
        sqlx::query_scalar("SELECT historia_id FROM imagen_diagnostica WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    if crate::historia::admite_escritura_de(pool, ambito, padre).await?
        == crate::Resultado::Firmada
    {
        return Ok(None);
    }
    sqlx::query("DELETE FROM imagen_diagnostica WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Some(img.ruta_archivo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// LA MIGRACIÓN 0056 REETIQUETA LAS PLACAS VIEJAS.
    ///
    /// Su primera versión afirmaba que el defecto `adjunto_historia` describía
    /// bien lo que había, porque «el adjunto de la historia era el único camino
    /// documentado». Era falso: la placa del radiólogo ya escribía en esta misma
    /// tabla antes de la rama. Sin el `UPDATE`, toda placa ya subida quedaba
    /// dentro de lo que sella la firma — al revés de lo que la columna existe
    /// para distinguir.
    ///
    /// Se simula una base «vieja»: filas SIN origen explícito (el defecto las
    /// pone en `adjunto_historia`) y un informe que cita una de ellas.
    #[tokio::test]
    async fn el_relleno_de_la_0056_marca_las_placas_y_deja_los_adjuntos() {
        let pool = base().await;
        // Dos imágenes: la 1 la cita un informe (es placa), la 2 no (es adjunto).
        for i in 1..=2 {
            sqlx::query(
                "INSERT INTO imagen_diagnostica (id, historia_id, ruta_archivo, nombre_original, \
                 tipo_mime, tamano_bytes) VALUES (?, 100, ?, 'x.jpg', 'image/jpeg', 10)",
            )
            .bind(i)
            .bind(format!("{i}.dat"))
            .execute(&pool)
            .await
            .unwrap();
        }
        // `informe_imagen` exige orden, paciente, radiólogo y los tres textos.
        sqlx::query(
            "INSERT INTO orden (id, historia_id, tipo, descripcion, justificacion) \
             VALUES (1, 100, 'Imagenología', 'Rx tórax', 'control')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO informe_imagen (orden_id, paciente_id, radiologo_id, tecnica, \
             hallazgos, impresion_diagnostica, imagenes_paths) \
             VALUES (1, 10, 5, 't', 'h', 'i', '[\"/api/imagenes/1/archivo\"]')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // El mismo UPDATE que la migración.
        sqlx::query(
            "UPDATE imagen_diagnostica SET origen = 'resultado_imagenologia' \
             WHERE EXISTS (SELECT 1 FROM informe_imagen i \
                 WHERE i.imagenes_paths LIKE '%/api/imagenes/' || imagen_diagnostica.id || '/archivo%')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let o: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, origen FROM imagen_diagnostica ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(o[0].1, "resultado_imagenologia", "la placa citada no se reetiquetó");
        // La pareja, y sin ella un UPDATE que marcara TODO pasaría igual.
        assert_eq!(o[1].1, "adjunto_historia", "se reetiquetó un adjunto que nadie cita");
    }

    /// El `LIKE` no confunde la imagen 1 con la 17 ni con la 71: la ruta lleva el
    /// id entre dos separadores fijos. Sin esta prueba, un patrón sin las barras
    /// —`%imagenes/1%`— pasaría la de arriba y marcaría de más.
    #[tokio::test]
    async fn el_relleno_no_confunde_un_id_con_otro_que_lo_contenga() {
        let pool = base().await;
        for i in [1_i64, 17, 71] {
            sqlx::query(
                "INSERT INTO imagen_diagnostica (id, historia_id, ruta_archivo, nombre_original, \
                 tipo_mime, tamano_bytes) VALUES (?, 100, ?, 'x.jpg', 'image/jpeg', 10)",
            )
            .bind(i)
            .bind(format!("{i}.dat"))
            .execute(&pool)
            .await
            .unwrap();
        }
        // `informe_imagen` exige orden, paciente, radiólogo y los tres textos.
        sqlx::query(
            "INSERT INTO orden (id, historia_id, tipo, descripcion, justificacion) \
             VALUES (1, 100, 'Imagenología', 'Rx tórax', 'control')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO informe_imagen (orden_id, paciente_id, radiologo_id, tecnica, \
             hallazgos, impresion_diagnostica, imagenes_paths) \
             VALUES (1, 10, 5, 't', 'h', 'i', '[\"/api/imagenes/1/archivo\"]')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE imagen_diagnostica SET origen = 'resultado_imagenologia' \
             WHERE EXISTS (SELECT 1 FROM informe_imagen i \
                 WHERE i.imagenes_paths LIKE '%/api/imagenes/' || imagen_diagnostica.id || '/archivo%')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let marcadas: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM imagen_diagnostica WHERE origen = 'resultado_imagenologia' ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(marcadas.len(), 1, "marcó de más: {marcadas:?}");
        assert_eq!(marcadas[0].0, 1);
    }

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dra','d@x.co','h','medico',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento) VALUES (100,'HC1',10,5,'m','e','J00','plan'),(200,'HC2',20,5,'m','e','J00','plan')")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nueva() -> ImagenNueva {
        ImagenNueva {
            nombre_original: "rx-torax.jpg".into(),
            tipo_mime: "image/jpeg".into(),
            tamano_bytes: 12345,
            descripcion: "Radiografía de tórax".into(),
        }
    }

    #[tokio::test]
    async fn registrar_fija_la_ruta_con_namespace_de_instancia() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::institucion(1), 100, &nueva(), false, Origen::AdjuntoDeLaHistoria).await.unwrap().unwrap();
        // Namespace por instancia (#758): la ruta es `{uuid}/{id}.dat`, no `{id}.dat` a secas.
        let uuid = uuid_de_instancia(&pool).await.unwrap();
        assert_eq!(img.ruta_archivo, format!("{uuid}/{}.dat", img.id));
        assert_eq!(img.nombre_original, "rx-torax.jpg");
        let lista = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, img.id);
    }

    /// La migración al namespace por instancia mueve el `{id}.dat` legado a `{uuid}/{id}.dat`,
    /// actualiza `ruta_archivo`, y es IDEMPOTENTE (una 2ª corrida no mueve nada). Discrimina:
    /// tras migrar, el archivo vive en la subcarpeta y NO en la raíz.
    #[tokio::test]
    async fn migrar_mueve_el_layout_legado_y_es_idempotente() {
        let pool = base().await;
        // Siembra una fila con el layout LEGADO (`{id}.dat`, sin subcarpeta) y su archivo.
        let id: i64 = sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id,ruta_archivo,nombre_original,tipo_mime,tamano_bytes) \
             VALUES (100,'','rx.jpg','image/jpeg',3)",
        ).execute(&pool).await.unwrap().last_insert_rowid();
        sqlx::query("UPDATE imagen_diagnostica SET ruta_archivo = ? WHERE id = ?")
            .bind(format!("{id}.dat")).bind(id).execute(&pool).await.unwrap();
        let tmp = std::env::temp_dir().join(format!("medico-mig-{}", std::process::id()));
        let dir = tmp.join("imagenes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{id}.dat")), b"PHI").unwrap();

        let uuid = uuid_de_instancia(&pool).await.unwrap();
        let n = migrar_a_namespace_por_instancia(&pool, &dir).await.unwrap();
        assert_eq!(n, 1, "una fila legada migrada");
        // El archivo se movió a la subcarpeta; la raíz ya no lo tiene.
        assert!(dir.join(&uuid).join(format!("{id}.dat")).exists(), "el archivo vive en dir/{{uuid}}/");
        assert!(!dir.join(format!("{id}.dat")).exists(), "ya no está en la raíz");
        // La fila apunta al nuevo layout.
        let ruta: String = sqlx::query_scalar("SELECT ruta_archivo FROM imagen_diagnostica WHERE id = ?")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(ruta, format!("{uuid}/{id}.dat"));
        // Idempotente: una 2ª corrida no encuentra nada legado.
        let n2 = migrar_a_namespace_por_instancia(&pool, &dir).await.unwrap();
        assert_eq!(n2, 0, "la 2ª corrida no migra nada (ya está en el nuevo layout)");
        assert_eq!(std::fs::read(dir.join(&uuid).join(format!("{id}.dat"))).unwrap(), b"PHI");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[tokio::test]
    async fn la_marca_cifrada_persiste() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::institucion(1), 100, &nueva(), true, Origen::AdjuntoDeLaHistoria).await.unwrap().unwrap();
        assert!(img.cifrada);
        assert!(obtener(&pool, Ambito::institucion(1), img.id).await.unwrap().unwrap().cifrada);
    }

    #[tokio::test]
    async fn la_historia_ajena_no_deja_subir_ni_listar() {
        let pool = base().await;
        assert!(registrar(&pool, Ambito::institucion(1), 200, &nueva(), false, Origen::AdjuntoDeLaHistoria).await.unwrap().is_none());
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 200).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn obtener_y_eliminar_respetan_el_ambito() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::institucion(1), 100, &nueva(), false, Origen::AdjuntoDeLaHistoria).await.unwrap().unwrap();
        // Ajena: ni se ve ni se borra.
        assert!(obtener(&pool, Ambito::institucion(2), img.id).await.unwrap().is_none());
        assert!(eliminar(&pool, Ambito::institucion(2), img.id).await.unwrap().is_none());
        // Propia: se ve, y al borrar devuelve la ruta para limpiar el archivo.
        assert!(obtener(&pool, Ambito::institucion(1), img.id).await.unwrap().is_some());
        let ruta = eliminar(&pool, Ambito::institucion(1), img.id).await.unwrap().unwrap();
        let uuid = uuid_de_instancia(&pool).await.unwrap();
        assert_eq!(ruta, format!("{uuid}/{}.dat", img.id));
        assert!(obtener(&pool, Ambito::institucion(1), img.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn el_superadmin_ve_de_cualquier_institucion() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::Superadmin, 200, &nueva(), false, Origen::AdjuntoDeLaHistoria).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::Superadmin, img.id).await.unwrap().is_some());
    }
}
