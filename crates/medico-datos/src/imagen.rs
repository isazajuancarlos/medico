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
                        tamano_bytes, descripcion, fecha_subida";

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
pub async fn registrar(
    pool: &crate::Pool,
    ambito: Ambito,
    historia_id: i64,
    m: &ImagenNueva,
) -> Result<Option<Imagen>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(None);
    }
    let mut tx = pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO imagen_diagnostica (historia_id, ruta_archivo, nombre_original, tipo_mime, tamano_bytes, descripcion) \
         VALUES (?,?,?,?,?,?)",
    )
    .bind(historia_id)
    .bind("") // se fija enseguida desde el id
    .bind(&m.nombre_original)
    .bind(&m.tipo_mime)
    .bind(m.tamano_bytes)
    .bind(&m.descripcion)
    .execute(&mut *tx)
    .await?;
    let id = r.last_insert_rowid();
    let ruta = format!("{id}.dat");
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
pub async fn eliminar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<String>, sqlx::Error> {
    let Some(img) = imagen_visible(pool, ambito, id).await? else {
        return Ok(None);
    };
    sqlx::query("DELETE FROM imagen_diagnostica WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Some(img.ruta_archivo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

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
    async fn registrar_fija_la_ruta_desde_el_id() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        assert_eq!(img.ruta_archivo, format!("{}.dat", img.id));
        assert_eq!(img.nombre_original, "rx-torax.jpg");
        let lista = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, img.id);
    }

    #[tokio::test]
    async fn la_historia_ajena_no_deja_subir_ni_listar() {
        let pool = base().await;
        assert!(registrar(&pool, Ambito::institucion(1), 200, &nueva()).await.unwrap().is_none());
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 200).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn obtener_y_eliminar_respetan_el_ambito() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::institucion(1), 100, &nueva()).await.unwrap().unwrap();
        // Ajena: ni se ve ni se borra.
        assert!(obtener(&pool, Ambito::institucion(2), img.id).await.unwrap().is_none());
        assert!(eliminar(&pool, Ambito::institucion(2), img.id).await.unwrap().is_none());
        // Propia: se ve, y al borrar devuelve la ruta para limpiar el archivo.
        assert!(obtener(&pool, Ambito::institucion(1), img.id).await.unwrap().is_some());
        let ruta = eliminar(&pool, Ambito::institucion(1), img.id).await.unwrap().unwrap();
        assert_eq!(ruta, format!("{}.dat", img.id));
        assert!(obtener(&pool, Ambito::institucion(1), img.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn el_superadmin_ve_de_cualquier_institucion() {
        let pool = base().await;
        let img = registrar(&pool, Ambito::Superadmin, 200, &nueva()).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::Superadmin, img.id).await.unwrap().is_some());
    }
}
