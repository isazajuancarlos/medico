//! Proveedores de la farmacia. Es la mitad que le faltaba a la entrada de stock:
//! sin ellos, una remesa dice cuánto entró y cuándo, pero no de quién.
//!
//! No existe en el sistema Python ni en el original de Visual FoxPro; nace aquí.
//! Lleva `institucion_id` propio y se filtra por él, como el inventario.
//!
//! **Un proveedor no se borra si ya trajo algo: se desactiva.** Sus entradas
//! anteriores tienen que seguir diciendo de dónde vinieron, y una fila borrada
//! deja el `proveedor_id` de aquellos movimientos apuntando a nada.

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Proveedor {
    pub id: i64,
    pub razon_social: String,
    pub nit: Option<String>,
    pub contacto: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub direccion: Option<String>,
    pub ciudad: Option<String>,
    pub suministra: Option<String>,
    pub observaciones: Option<String>,
    pub activo: bool,
    /// Cuántas remesas ha traído. La interfaz lo usa para decir por qué un
    /// proveedor no se puede borrar, antes de que el usuario lo intente.
    pub entradas: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProveedorNuevo {
    pub razon_social: String,
    pub nit: Option<String>,
    pub contacto: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub direccion: Option<String>,
    pub ciudad: Option<String>,
    pub suministra: Option<String>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProveedorEdicion {
    pub razon_social: Option<String>,
    pub nit: Option<String>,
    pub contacto: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub direccion: Option<String>,
    pub ciudad: Option<String>,
    pub suministra: Option<String>,
    pub observaciones: Option<String>,
    pub activo: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltrosProveedor {
    /// Busca en razón social, NIT y contacto.
    pub q: Option<String>,
    /// Sin decir nada se devuelven **todos**, activos e inactivos: la pantalla de
    /// gestión los administra. El selector de la entrada pide `activo=true`.
    pub activo: Option<bool>,
}

/// Qué pasó al dar de alta o editar. La razón social vacía es un error del
/// llamante, no una fila con nombre en blanco que después nadie sabe qué es.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoAlta {
    Creado(i64),
    NoExiste,
    SinRazonSocial,
    NitRepetido(String),
}

/// Qué pasó al borrar.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoBorrado {
    Hecho,
    NoExiste,
    /// Ya trajo remesas: se desactiva, no se borra.
    TieneEntradas { entradas: i64 },
}

fn filtro_ambito(ambito: Ambito, col: &str) -> Option<(String, Option<i64>)> {
    match ambito {
        Ambito::Superadmin => Some(("1 = 1".into(), None)),
        Ambito::Institucion(i) => Some((format!("{col} = ?"), Some(i))),
        Ambito::Ninguno => None,
    }
}

const COLUMNAS: &str = "p.id, p.razon_social, p.nit, p.contacto, p.telefono, p.email, \
    p.direccion, p.ciudad, p.suministra, p.observaciones, p.activo, \
    (SELECT COUNT(*) FROM movimiento_inventario m WHERE m.proveedor_id = p.id) AS entradas";

pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &FiltrosProveedor,
) -> Result<Vec<Proveedor>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!("SELECT {COLUMNAS} FROM proveedor p WHERE {filtro}");
    if f.q.is_some() {
        sql.push_str(" AND (p.razon_social LIKE ? OR p.nit LIKE ? OR p.contacto LIKE ?)");
    }
    if f.activo.is_some() {
        sql.push_str(" AND p.activo = ?");
    }
    sql.push_str(" ORDER BY p.activo DESC, p.razon_social");

    let mut q = sqlx::query_as::<_, Proveedor>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(texto) = &f.q {
        let patron = format!("%{texto}%");
        q = q.bind(patron.clone()).bind(patron.clone()).bind(patron);
    }
    if let Some(a) = f.activo {
        q = q.bind(a);
    }
    q.fetch_all(pool).await
}

pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Proveedor>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "p.institucion_id") else {
        return Ok(None);
    };
    let sql = format!("SELECT {COLUMNAS} FROM proveedor p WHERE p.id = ? AND {filtro}");
    let mut q = sqlx::query_as::<_, Proveedor>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// El NIT en blanco se guarda como `NULL`, no como cadena vacía: el índice único
/// parcial trata la cadena vacía como un valor, y dos proveedores «sin NIT»
/// chocarían entre sí por no tenerlo.
fn nit_normalizado(nit: Option<&str>) -> Option<String> {
    nit.map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned)
}

pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    institucion_id: i64,
    p: &ProveedorNuevo,
) -> Result<ResultadoAlta, sqlx::Error> {
    if ambito.puede_ver(Some(institucion_id)) != Visibilidad::Si {
        return Ok(ResultadoAlta::NoExiste);
    }
    if p.razon_social.trim().is_empty() {
        return Ok(ResultadoAlta::SinRazonSocial);
    }
    let nit = nit_normalizado(p.nit.as_deref());
    let r = sqlx::query(
        "INSERT INTO proveedor \
         (institucion_id, razon_social, nit, contacto, telefono, email, direccion, ciudad, \
          suministra, observaciones) VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(p.razon_social.trim())
    .bind(&nit)
    .bind(&p.contacto)
    .bind(&p.telefono)
    .bind(&p.email)
    .bind(&p.direccion)
    .bind(&p.ciudad)
    .bind(&p.suministra)
    .bind(&p.observaciones)
    .execute(pool)
    .await;
    match r {
        Ok(r) => Ok(ResultadoAlta::Creado(r.last_insert_rowid())),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Ok(ResultadoAlta::NitRepetido(nit.unwrap_or_default()))
        }
        Err(e) => Err(e),
    }
}

pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    e: &ProveedorEdicion,
) -> Result<ResultadoAlta, sqlx::Error> {
    if obtener(pool, ambito, id).await?.is_none() {
        return Ok(ResultadoAlta::NoExiste);
    }
    if e.razon_social.as_deref().is_some_and(|r| r.trim().is_empty()) {
        return Ok(ResultadoAlta::SinRazonSocial);
    }
    let nit = nit_normalizado(e.nit.as_deref());
    let r = sqlx::query(
        "UPDATE proveedor SET \
         razon_social=COALESCE(?,razon_social), nit=COALESCE(?,nit), \
         contacto=COALESCE(?,contacto), telefono=COALESCE(?,telefono), \
         email=COALESCE(?,email), direccion=COALESCE(?,direccion), ciudad=COALESCE(?,ciudad), \
         suministra=COALESCE(?,suministra), observaciones=COALESCE(?,observaciones), \
         activo=COALESCE(?,activo) WHERE id = ?",
    )
    .bind(e.razon_social.as_deref().map(str::trim))
    .bind(&nit)
    .bind(&e.contacto)
    .bind(&e.telefono)
    .bind(&e.email)
    .bind(&e.direccion)
    .bind(&e.ciudad)
    .bind(&e.suministra)
    .bind(&e.observaciones)
    .bind(e.activo)
    .bind(id)
    .execute(pool)
    .await;
    match r {
        Ok(_) => Ok(ResultadoAlta::Creado(id)),
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            Ok(ResultadoAlta::NitRepetido(nit.unwrap_or_default()))
        }
        Err(err) => Err(err),
    }
}

pub async fn eliminar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<ResultadoBorrado, sqlx::Error> {
    let Some(p) = obtener(pool, ambito, id).await? else {
        return Ok(ResultadoBorrado::NoExiste);
    };
    if p.entradas > 0 {
        return Ok(ResultadoBorrado::TieneEntradas { entradas: p.entradas });
    }
    sqlx::query("DELETE FROM proveedor WHERE id = ?").bind(id).execute(pool).await?;
    Ok(ResultadoBorrado::Hecho)
}

/// ¿Es del ámbito y está activo? Lo pregunta la entrada de stock antes de
/// apuntarle una remesa: un proveedor ajeno no es un proveedor.
pub async fn activo_del_ambito(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<bool, sqlx::Error> {
    Ok(obtener(pool, ambito, id).await?.is_some_and(|p| p.activo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nuevo(razon: &str, nit: Option<&str>) -> ProveedorNuevo {
        ProveedorNuevo {
            razon_social: razon.into(),
            nit: nit.map(str::to_owned),
            contacto: Some("Compras".into()),
            telefono: Some("6042000000".into()),
            email: Some("ventas@x.co".into()),
            direccion: Some("Cra 1 #1-1".into()),
            ciudad: Some("Medellín".into()),
            suministra: Some("Medicamentos".into()),
            observaciones: None,
        }
    }

    #[tokio::test]
    async fn crear_y_listar() {
        let pool = base().await;
        let ResultadoAlta::Creado(id) =
            crear(&pool, Ambito::institucion(1), 1, &nuevo("Distribuidora Andina", Some("900123456-1"))).await.unwrap()
        else { panic!("debía crearse") };
        let ps = listar(&pool, Ambito::institucion(1), &FiltrosProveedor::default()).await.unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].id, id);
        assert!(ps[0].activo);
        assert_eq!(ps[0].entradas, 0);
    }

    #[tokio::test]
    async fn el_proveedor_ajeno_es_indistinguible_del_inexistente() {
        let pool = base().await;
        let ResultadoAlta::Creado(id) =
            crear(&pool, Ambito::institucion(1), 1, &nuevo("Andina", None)).await.unwrap()
        else { panic!() };
        assert!(listar(&pool, Ambito::institucion(2), &FiltrosProveedor::default()).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_none());
        assert_eq!(
            actualizar(&pool, Ambito::institucion(2), id, &ProveedorEdicion {
                razon_social: Some("Robado".into()), ..Default::default()
            }).await.unwrap(),
            ResultadoAlta::NoExiste
        );
        assert_eq!(eliminar(&pool, Ambito::institucion(2), id).await.unwrap(), ResultadoBorrado::NoExiste);
        assert!(!activo_del_ambito(&pool, Ambito::institucion(2), id).await.unwrap());
        // Y crear en institución ajena tampoco.
        assert_eq!(
            crear(&pool, Ambito::institucion(1), 2, &nuevo("Ajeno", None)).await.unwrap(),
            ResultadoAlta::NoExiste
        );
    }

    /// El NIT es único dentro de la institución **solo cuando se declara**: dos
    /// proveedores sin NIT conviven, y el mismo NIT en OTRA institución también.
    #[tokio::test]
    async fn el_nit_se_repite_donde_debe_y_no_donde_no() {
        let pool = base().await;
        assert!(matches!(
            crear(&pool, Ambito::institucion(1), 1, &nuevo("Andina", Some("900123456-1"))).await.unwrap(),
            ResultadoAlta::Creado(_)
        ));
        assert_eq!(
            crear(&pool, Ambito::institucion(1), 1, &nuevo("Andina S.A.S.", Some("900123456-1"))).await.unwrap(),
            ResultadoAlta::NitRepetido("900123456-1".into()),
            "el mismo NIT dos veces en la misma institución es un error de digitación"
        );
        // La institución 2 puede tener al mismo proveedor: es su propia relación.
        assert!(matches!(
            crear(&pool, Ambito::institucion(2), 2, &nuevo("Andina", Some("900123456-1"))).await.unwrap(),
            ResultadoAlta::Creado(_)
        ));
        // Y varios sin NIT conviven: el vacío se guarda como NULL, no como "".
        assert!(matches!(crear(&pool, Ambito::institucion(1), 1, &nuevo("Sin NIT A", None)).await.unwrap(), ResultadoAlta::Creado(_)));
        assert!(matches!(crear(&pool, Ambito::institucion(1), 1, &nuevo("Sin NIT B", Some("   "))).await.unwrap(), ResultadoAlta::Creado(_)));
        assert_eq!(listar(&pool, Ambito::institucion(1), &FiltrosProveedor::default()).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn sin_razon_social_no_hay_proveedor() {
        let pool = base().await;
        assert_eq!(
            crear(&pool, Ambito::institucion(1), 1, &nuevo("   ", None)).await.unwrap(),
            ResultadoAlta::SinRazonSocial
        );
        assert!(listar(&pool, Ambito::institucion(1), &FiltrosProveedor::default()).await.unwrap().is_empty());
    }

    /// El que ya trajo remesas se desactiva, no se borra: sus entradas tienen que
    /// seguir diciendo de quién vinieron.
    #[tokio::test]
    async fn el_proveedor_con_entradas_no_se_borra_pero_si_se_desactiva() {
        let pool = base().await;
        let ResultadoAlta::Creado(id) =
            crear(&pool, Ambito::institucion(1), 1, &nuevo("Andina", None)).await.unwrap()
        else { panic!() };
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'R','r@x.co','h','regente_farmacia',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO item_inventario (id,institucion_id,nombre,lote,cantidad_actual) VALUES (9,1,'Gasa','L-1',10)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO movimiento_inventario (item_inventario_id,usuario_id,tipo_movimiento,cantidad,motivo,proveedor_id) VALUES (9,5,'ENTRADA',10,'Compra',?)")
            .bind(id).execute(&pool).await.unwrap();

        assert_eq!(
            eliminar(&pool, Ambito::institucion(1), id).await.unwrap(),
            ResultadoBorrado::TieneEntradas { entradas: 1 }
        );
        // Desactivarlo sí, y entonces deja de salir en el selector.
        assert!(matches!(
            actualizar(&pool, Ambito::institucion(1), id, &ProveedorEdicion { activo: Some(false), ..Default::default() }).await.unwrap(),
            ResultadoAlta::Creado(_)
        ));
        assert!(!activo_del_ambito(&pool, Ambito::institucion(1), id).await.unwrap());
        let activos = listar(&pool, Ambito::institucion(1), &FiltrosProveedor { activo: Some(true), ..Default::default() }).await.unwrap();
        assert!(activos.is_empty());
        // Pero sigue existiendo para la pantalla de gestión, con su cuenta.
        let todos = listar(&pool, Ambito::institucion(1), &FiltrosProveedor::default()).await.unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].entradas, 1);
        assert!(!todos[0].activo);
    }

    #[tokio::test]
    async fn sin_ambito_no_se_ve_ni_se_crea() {
        let pool = base().await;
        assert!(listar(&pool, Ambito::Ninguno, &FiltrosProveedor::default()).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::Ninguno, 1).await.unwrap().is_none());
        assert_eq!(
            crear(&pool, Ambito::Ninguno, 1, &nuevo("Andina", None)).await.unwrap(),
            ResultadoAlta::NoExiste
        );
    }

    #[tokio::test]
    async fn la_busqueda_mira_razon_social_nit_y_contacto() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), 1, &nuevo("Distribuidora Andina", Some("900123456-1"))).await.unwrap();
        let mut otro = nuevo("Insumos del Valle", Some("800999888-2"));
        otro.contacto = Some("Marta Gómez".into());
        crear(&pool, Ambito::institucion(1), 1, &otro).await.unwrap();

        let buscar = |t: &str| {
            let f = FiltrosProveedor { q: Some(t.into()), activo: None };
            let pool = pool.clone();
            async move { listar(&pool, Ambito::institucion(1), &f).await.unwrap() }
        };
        assert_eq!(buscar("Andina").await.len(), 1);
        assert_eq!(buscar("800999").await.len(), 1);
        assert_eq!(buscar("Marta").await.len(), 1);
        assert_eq!(buscar("Nadie").await.len(), 0);
    }
}
