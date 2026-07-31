# medico en Rust — Plan 1: la fundación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un binario `medico` que sirva el Angular ya compilado desde dentro de sí mismo, abra una base SQLite propia, autentique con Argon2id + JWT, y responda `/api/pacientes` con aislamiento por institución comprobado por prueba.

**Architecture:** Workspace de cargo con tres crates —`medico-dominio` (tipos y la regla de aislamiento, sin E/S), `medico-datos` (SQLx y migraciones), `medico-servidor` (axum, el binario y el frontend embebido)—. El frontend Angular/PrimeNG **no se toca**: se compila con `ng build` y se empotra con `rust-embed`. La decisión que gobierna todo esto es `decisiones/0008-medico-en-rust-revision-del-estandar-73.md`.

**Tech Stack:** Rust 1.97 edición 2024 · axum 0.8 · SQLx 0.8 (sqlite, migrate) · rust-embed 8 · argon2 · jsonwebtoken · tokio · serde.

## Global Constraints

- **Edición 2024**, igual que Quipu. `resolver = "2"`, workspace en la raíz con `crates/`.
- **Titularidad desde el primer commit** (directiva 12): cada `Cargo.toml` lleva `authors = ["Juan Carlos Isaza Arenas <isazajuancarlos@gmail.com>"]` y `license`. Xiliux no es persona jurídica y no aparece como autor.
- **Ningún secreto con valor por defecto.** Si falta `SECRET_KEY` o `DATABASE_URL`, el proceso **no arranca**. Es el invariante que `backend/core/config.py` ya sostiene y que no se pierde al portar.
- **Ante la ausencia de un dato, fallar ruidosamente** (directiva 20). Nada de `unwrap_or_default()` sobre datos clínicos ni códigos de catálogo.
- **La prueba se porta antes que el código** y debe verse fallar. Una prueba portada que pasa a la primera no está probando lo que se cree.
- **Este plan no borra nada de Python.** `backend/` sigue funcionando y sigue siendo la referencia contra la que se compara. La retirada se decide en un plan posterior y solo con la comparación byte a byte del FEV-RIPS en verde.
- El toolchain no está en el PATH de las shells nuevas: `export PATH="$HOME/.cargo/bin:$PATH"`.

## Alcance, y por qué este plan no cubre la migración entera

El sistema en Python son 25 759 líneas, 83 tablas, 219 endpoints y 657 pruebas. Un plan único para eso sería una lista de marcadores de posición, que es exactamente lo que un plan no puede ser. Este cubre **la rebanada vertical que fija las tres mecánicas de las que depende todo lo demás**: el binario que se sirve a sí mismo, el aislamiento por institución, y la identidad. Sale de aquí software que funciona y se puede probar.

Los planes siguientes, en este orden y cada uno con su documento:

| plan | contenido | por qué en ese orden |
|---|---|---|
| 2 | Esquema completo (83 tablas) + catálogos empaquetados | Nada clínico se puede portar sin el esquema |
| 3 | `fev-rips` como crate: FEV-RIPS, CIE-11, CUM, DIVIPOLA, maestros MUV | Es la salida con consecuencia económica. Crate propio: directiva 9 |
| 4 | Historia clínica, citas, urgencias — los routers reales | El grueso del CRUD |
| 5 | Comparación byte a byte contra Python y retirada de `backend/` | La única prueba que cierra el riesgo |

---

## File Structure

```
Cargo.toml                          workspace: members, perfil release, metadatos comunes
crates/medico-dominio/
  Cargo.toml
  src/lib.rs                        reexporta
  src/rol.rs                        RolUsuario
  src/aislamiento.rs                Ambito y la regla 404-no-403. SIN E/S: puro y probable
crates/medico-datos/
  Cargo.toml
  src/lib.rs                        abrir(), tipo Pool
  src/paciente.rs                   consultas de paciente, todas con institución
  migrations/0001_fundacion.sql     institucion, usuario, paciente
crates/medico-servidor/
  Cargo.toml
  build.rs                          falla la compilación si no hay build de Angular en release
  src/main.rs                       arranque: config, pool, escucha
  src/config.rs                     secretos: falla al arrancar, sin defectos
  src/estado.rs                     AppState
  src/error.rs                      ErrorApi -> respuesta HTTP
  src/frontend.rs                   rust-embed + comodín SPA + PREFIJOS_DEL_BACKEND
  src/rutas/mod.rs                  router()
  src/rutas/salud.rs                /health
  src/rutas/auth.rs                 POST /api/auth/login, GET /api/auth/me
  src/rutas/pacientes.rs            GET /api/pacientes, GET /api/pacientes/{id}
  tests/frontend_embebido.rs        puerto de backend/tests/test_frontend_embebido.py
  tests/aislamiento.rs              puerto de backend/tests/test_aislamiento_institucion.py
```

**Por qué `medico-dominio` no toca la base:** la regla de aislamiento es la que impide que una institución lea las historias de otra. Si vive pegada a SQLx, probarla exige una base; si es una función pura sobre `(rol, institucion_del_usuario, institucion_del_registro)`, se prueba exhaustivamente en microsegundos y se puede llamar desde cualquier capa. En Python esa regla estaba enredada con el ORM porque el ORM ofrecía el grafo de objetos; en SQLx no hay grafo, y eso —que era la objeción— aquí se aprovecha.

---

### Task 1: Workspace y el dominio del aislamiento

**Files:**
- Create: `Cargo.toml`, `crates/medico-dominio/Cargo.toml`, `crates/medico-dominio/src/lib.rs`, `crates/medico-dominio/src/rol.rs`, `crates/medico-dominio/src/aislamiento.rs`
- Test: dentro de `crates/medico-dominio/src/aislamiento.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `medico_dominio::RolUsuario`, `medico_dominio::Ambito`, `medico_dominio::Visibilidad`, `Ambito::puede_ver(&self, duenio: Option<i64>) -> Visibilidad`

- [ ] **Step 1: Crear el workspace**

`Cargo.toml` en la raíz:

```toml
[workspace]
members = ["crates/medico-dominio", "crates/medico-datos", "crates/medico-servidor"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Juan Carlos Isaza Arenas <isazajuancarlos@gmail.com>"]
license = "LicenseRef-Propietario"
repository = "https://github.com/isazajuancarlos/medico"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite", "migrate", "macros", "chrono"] }
axum = "0.8"
chrono = { version = "0.4", features = ["serde"] }

[profile.release]
lto = true
codegen-units = 1
strip = true
```

`crates/medico-dominio/Cargo.toml`:

```toml
[package]
name = "medico-dominio"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
```

- [ ] **Step 2: Escribir la prueba que falla**

`crates/medico-dominio/src/aislamiento.rs`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un ADMIN ve lo suyo.
    #[test]
    fn lo_propio_se_ve() {
        let a = Ambito::institucion(7);
        assert_eq!(a.puede_ver(Some(7)), Visibilidad::Si);
    }

    /// LA REGLA QUE IMPORTA: lo ajeno es INDISTINGUIBLE de lo inexistente.
    ///
    /// Un 403 confirma que el registro existe en otra institución, y con eso se
    /// enumera: se prueban identificaciones hasta que una deja de dar 404 y ya
    /// se sabe que esa persona es paciente de la clínica de al lado. Eso también
    /// es un dato de salud. Por eso hay UN SOLO valor para las dos situaciones.
    #[test]
    fn lo_ajeno_y_lo_inexistente_dan_lo_mismo() {
        let a = Ambito::institucion(7);
        assert_eq!(a.puede_ver(Some(8)), Visibilidad::NoExiste);
        assert_eq!(a.puede_ver(None), Visibilidad::NoExiste);
    }

    /// El SUPERADMIN no pertenece a ninguna institución: es el dueño del sistema
    /// multi-institucional, no un cliente.
    #[test]
    fn el_superadmin_ve_todo() {
        let a = Ambito::Superadmin;
        assert_eq!(a.puede_ver(Some(1)), Visibilidad::Si);
        assert_eq!(a.puede_ver(Some(999)), Visibilidad::Si);
    }

    /// Un registro sin dueño NO se concede por defecto.
    ///
    /// En Python un modelo sin `institucion_id`, sin enlace conocido y no
    /// declarado como catálogo compartido levantaba RuntimeError, a propósito:
    /// si mañana alguien añade un modelo clínico y olvida decir de quién es,
    /// tiene que romperse en una prueba y no en producción sirviendo datos
    /// ajenos a quien los pida. Aquí el equivalente es que `None` NO es «de
    /// todos»: es `NoExiste` salvo para el superadmin.
    #[test]
    fn un_registro_sin_duenio_no_se_regala() {
        assert_eq!(Ambito::institucion(7).puede_ver(None), Visibilidad::NoExiste);
    }

    /// Un usuario sin institución que no es superadmin no ve nada. Es el estado
    /// que en Python devolvía `set()` en `modulos_de`, y aquí no puede
    /// convertirse por descuido en «ve todo».
    #[test]
    fn sin_institucion_y_sin_ser_superadmin_no_se_ve_nada() {
        let a = Ambito::Ninguno;
        assert_eq!(a.puede_ver(Some(1)), Visibilidad::NoExiste);
        assert_eq!(a.puede_ver(None), Visibilidad::NoExiste);
    }
}
```

- [ ] **Step 3: Verla fallar**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p medico-dominio`
Expected: FAIL — `cannot find type Ambito in this scope`.

- [ ] **Step 4: Implementar lo mínimo**

Encima de las pruebas, en el mismo `aislamiento.rs`:

```rust
//! Quién puede ver qué, sin tocar la base de datos.
//!
//! Es la regla que impide que una institución lea las historias clínicas de
//! otra. Vive aquí, separada de SQLx, por dos razones:
//!
//! 1. Se prueba exhaustivamente sin base ni servidor.
//! 2. La capa de datos NO puede devolver una fila sin pasar por aquí, porque
//!    no tiene otra forma de construir un `Visibilidad`.

use serde::{Deserialize, Serialize};

/// Desde dónde mira quien pregunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambito {
    /// Dueño del sistema multi-institucional. No es un cliente.
    Superadmin,
    /// Usuario de una institución concreta.
    Institucion(i64),
    /// Ni superadmin ni adscrito a una institución. No ve nada.
    Ninguno,
}

impl Ambito {
    pub fn institucion(id: i64) -> Self {
        Ambito::Institucion(id)
    }

    /// `duenio` es la institución del registro; `None` si el registro no
    /// declara dueño.
    pub fn puede_ver(&self, duenio: Option<i64>) -> Visibilidad {
        match (self, duenio) {
            (Ambito::Superadmin, _) => Visibilidad::Si,
            (Ambito::Institucion(mia), Some(suya)) if *mia == suya => Visibilidad::Si,
            _ => Visibilidad::NoExiste,
        }
    }
}

/// Deliberadamente NO hay variante `Prohibido`.
///
/// Distinguir «ajeno» de «inexistente» es lo que permite enumerar pacientes de
/// otra institución probando identificadores. Al no existir el valor, ningún
/// camino del código puede filtrarlo por descuido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibilidad {
    Si,
    NoExiste,
}
```

`crates/medico-dominio/src/lib.rs`:

```rust
pub mod aislamiento;
pub mod rol;

pub use aislamiento::{Ambito, Visibilidad};
pub use rol::RolUsuario;
```

`crates/medico-dominio/src/rol.rs` — los quince roles de `backend/models.py:52`, con los mismos valores de cadena porque la base y los tokens ya existentes los usan:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolUsuario {
    Superadmin,
    Admin,
    Medico,
    Paciente,
    Enfermeria,
    AuxiliarEnfermeria,
    RegenteFarmacia,
    Laboratorista,
    Radiologo,
    Anestesiologo,
    Terapeuta,
    Nutricionista,
    Psicologo,
    TrabajoSocial,
    Bacteriologo,
}

impl RolUsuario {
    /// El superadmin tiene `institucion_id = NULL` en la base. Traducir eso a
    /// `Ambito` es la única conversión donde un descuido concede todo el
    /// sistema, así que va en un solo sitio y con nombre.
    pub fn ambito(&self, institucion_id: Option<i64>) -> crate::Ambito {
        match (self, institucion_id) {
            (RolUsuario::Superadmin, _) => crate::Ambito::Superadmin,
            (_, Some(id)) => crate::Ambito::Institucion(id),
            (_, None) => crate::Ambito::Ninguno,
        }
    }
}
```

- [ ] **Step 5: Verla pasar**

Run: `cargo test -p medico-dominio`
Expected: PASS, 5 pruebas.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/medico-dominio
git commit -m "feat(rust): el aislamiento por institución como función pura

Primera pieza de la migración decidida en decisiones/0008. Se empieza por
aquí y no por el servidor porque es la regla que impide que una institución
lea las historias clínicas de otra, y porque era la objeción concreta contra
SQLx: sin grafo de objetos, no hay dónde colgar la comprobación.

La respuesta es que no se cuelga de la capa de datos: es una función pura
sobre (ámbito, dueño del registro). Se prueba sin base, y \`Visibilidad\` no
tiene variante \`Prohibido\` — así ningún camino puede distinguir 'ajeno' de
'inexistente' por descuido, que es lo que permitiría enumerar pacientes de
la clínica de al lado."
```

---

### Task 2: Esquema SQLite y la capa de datos

**Files:**
- Create: `crates/medico-datos/Cargo.toml`, `crates/medico-datos/src/lib.rs`, `crates/medico-datos/src/paciente.rs`, `crates/medico-datos/migrations/0001_fundacion.sql`
- Test: `crates/medico-datos/src/paciente.rs` (`#[cfg(test)]`, base en memoria)

**Interfaces:**
- Consumes: `medico_dominio::{Ambito, Visibilidad}`
- Produces: `medico_datos::abrir(url: &str) -> Result<Pool, sqlx::Error>`, `medico_datos::Pool`, `medico_datos::paciente::{Paciente, listar, obtener}`
  - `listar(&Pool, Ambito) -> Result<Vec<Paciente>, sqlx::Error>`
  - `obtener(&Pool, Ambito, i64) -> Result<Option<Paciente>, sqlx::Error>` — `None` significa «no existe **o** no es suyo», sin distinguir.

- [ ] **Step 1: Escribir la migración**

`crates/medico-datos/migrations/0001_fundacion.sql`. **No se traducen las 52 sentencias de `backend/database.py`**: aquéllas son el historial de cómo el esquema llegó a su forma, no la forma. Aquí se declara la forma.

```sql
-- Fundación del esquema. SQLite: claves foráneas ON, y NOT NULL donde el dato
-- no puede faltar. En Postgres varias de estas columnas acabaron siendo
-- anulables por migraciones sucesivas; al declarar de nuevo se recupera lo que
-- el dominio exige, no lo que el historial dejó.
PRAGMA foreign_keys = ON;

CREATE TABLE institucion (
    id          INTEGER PRIMARY KEY,
    nombre      TEXT NOT NULL,
    nit         TEXT NOT NULL UNIQUE,
    activa      INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE usuario (
    id             INTEGER PRIMARY KEY,
    nombre         TEXT NOT NULL,
    email          TEXT NOT NULL UNIQUE,
    password_hash  TEXT NOT NULL,
    rol            TEXT NOT NULL,
    -- NULL solo para el superadmin. Es la única fila del sistema a la que se
    -- le permite, y por eso no lleva NOT NULL.
    institucion_id INTEGER REFERENCES institucion(id),
    activo         INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE paciente (
    id                  INTEGER PRIMARY KEY,
    tipo_documento      TEXT NOT NULL,
    numero_documento    TEXT NOT NULL,
    nombre1             TEXT NOT NULL,
    nombre2             TEXT,
    apellido1           TEXT NOT NULL,
    apellido2           TEXT,
    fecha_nacimiento    TEXT NOT NULL,
    sexo                TEXT NOT NULL,
    -- Cinco dígitos, con el cero a la izquierda: 148 municipios (13 % del país)
    -- empiezan por cero, Medellín entre ellos. Va TEXT y no INTEGER por eso.
    cod_municipio       TEXT,
    institucion_id      INTEGER NOT NULL REFERENCES institucion(id)
);

-- El documento es único DENTRO de la institución, no en todo el sistema: el
-- mismo paciente puede estar en dos IPS y son dos filas legítimas.
CREATE UNIQUE INDEX paciente_documento_por_institucion
    ON paciente (institucion_id, tipo_documento, numero_documento);

CREATE INDEX paciente_por_institucion ON paciente (institucion_id);
```

- [ ] **Step 2: Escribir las pruebas que fallan**

`crates/medico-datos/src/paciente.rs`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use medico_dominio::Ambito;

    /// Base en memoria con dos instituciones y un paciente en cada una.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, cod_municipio, institucion_id)
             VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F','05001',1),
                    (20,'CC','222','Beto','Paz','1985-05-05','M','08001',2)")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn listar_solo_devuelve_lo_de_la_institucion() {
        let pool = base().await;
        let ps = listar(&pool, Ambito::institucion(1)).await.unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].id, 10);
    }

    /// LA PRUEBA QUE JUSTIFICA ESTE DISEÑO: pedir por identificador el paciente
    /// de otra institución tiene que ser indistinguible de pedir uno que no
    /// existe. En Python esto se escapó una vez -`db.get` no pasaba por el
    /// filtro- y bastaba incrementar un número en la URL.
    #[tokio::test]
    async fn obtener_el_de_otra_institucion_es_igual_que_uno_inexistente() {
        let pool = base().await;
        let ajeno = obtener(&pool, Ambito::institucion(1), 20).await.unwrap();
        let inexistente = obtener(&pool, Ambito::institucion(1), 9999).await.unwrap();
        assert!(ajeno.is_none());
        assert!(inexistente.is_none());
    }

    #[tokio::test]
    async fn obtener_el_propio_funciona() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10).await.unwrap().unwrap();
        assert_eq!(p.nombre1, "Ana");
    }

    #[tokio::test]
    async fn el_superadmin_ve_las_dos_instituciones() {
        let pool = base().await;
        assert_eq!(listar(&pool, Ambito::Superadmin).await.unwrap().len(), 2);
        assert!(obtener(&pool, Ambito::Superadmin, 20).await.unwrap().is_some());
    }

    /// Un usuario sin institución no ve nada, y en particular NO lo ve todo.
    #[tokio::test]
    async fn sin_ambito_no_se_ve_nada() {
        let pool = base().await;
        assert!(listar(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::Ninguno, 10).await.unwrap().is_none());
    }

    /// El cero a la izquierda del código de municipio sobrevive al viaje.
    /// Guardarlo como número lo perdería y reportaría a un paciente de Medellín
    /// como de un municipio que no existe.
    #[tokio::test]
    async fn el_codigo_de_municipio_conserva_el_cero() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10).await.unwrap().unwrap();
        assert_eq!(p.cod_municipio.as_deref(), Some("05001"));
    }
}
```

- [ ] **Step 3: Verlas fallar**

Run: `cargo test -p medico-datos`
Expected: FAIL — `listar`, `obtener` y `abrir` no existen.

- [ ] **Step 4: Implementar**

`crates/medico-datos/Cargo.toml`:

```toml
[package]
name = "medico-datos"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
medico-dominio = { path = "../medico-dominio" }
sqlx = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

`crates/medico-datos/src/lib.rs`:

```rust
//! Acceso a la base. Nadie de aquí para arriba escribe SQL.

pub mod paciente;

pub type Pool = sqlx::SqlitePool;

/// Abre la base y aplica las migraciones pendientes.
///
/// `create_if_missing` está a propósito: el despliegue en sitio es un binario y
/// un directorio, y pedirle a un consultorio que cree la base a mano antes de
/// arrancar es el paso donde se pierde una instalación.
pub async fn abrir(url: &str) -> Result<Pool, sqlx::Error> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let opciones = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        // WAL: lectores concurrentes sin bloquear al escritor. No convierte a
        // SQLite en multi-escritor —ese techo se aceptó en decisiones/0008—
        // pero es la diferencia entre un consultorio que va bien y uno que se
        // traba al consultar mientras se guarda.
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opciones).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

`crates/medico-datos/src/paciente.rs`, encima de las pruebas:

```rust
use medico_dominio::Ambito;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Paciente {
    pub id: i64,
    pub tipo_documento: String,
    pub numero_documento: String,
    pub nombre1: String,
    pub nombre2: Option<String>,
    pub apellido1: String,
    pub apellido2: Option<String>,
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub cod_municipio: Option<String>,
    pub institucion_id: i64,
}

const COLUMNAS: &str = "id, tipo_documento, numero_documento, nombre1, nombre2, \
                        apellido1, apellido2, fecha_nacimiento, sexo, cod_municipio, \
                        institucion_id";

/// El `Ambito` es un parámetro OBLIGATORIO, no opcional.
///
/// En Python el equivalente era `institucion_id: Optional[int] = None`, que al
/// omitirse dejaba de filtrar. Eso ya causó una fuga real: `rips_completo.py`
/// filtraba solo si se lo pedían y `routers/rips.py` no se lo pedía. Aquí no se
/// puede omitir: no compila.
pub async fn listar(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Paciente>, sqlx::Error> {
    match ambito {
        Ambito::Superadmin => {
            sqlx::query_as::<_, Paciente>(&format!("SELECT {COLUMNAS} FROM paciente ORDER BY id"))
                .fetch_all(pool).await
        }
        Ambito::Institucion(id) => {
            sqlx::query_as::<_, Paciente>(&format!(
                "SELECT {COLUMNAS} FROM paciente WHERE institucion_id = ? ORDER BY id"))
                .bind(id).fetch_all(pool).await
        }
        Ambito::Ninguno => Ok(Vec::new()),
    }
}

/// `None` significa «no existe **o** no es suyo». No se distingue a propósito:
/// ver `medico_dominio::Visibilidad`.
pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Paciente>, sqlx::Error> {
    let fila = sqlx::query_as::<_, Paciente>(&format!(
        "SELECT {COLUMNAS} FROM paciente WHERE id = ?"))
        .bind(id).fetch_optional(pool).await?;

    Ok(fila.filter(|p| {
        ambito.puede_ver(Some(p.institucion_id)) == medico_dominio::Visibilidad::Si
    }))
}
```

- [ ] **Step 5: Verlas pasar**

Run: `cargo test -p medico-datos`
Expected: PASS, 6 pruebas.

- [ ] **Step 6: Commit**

```bash
git add crates/medico-datos
git commit -m "feat(rust): esquema de fundación y acceso a pacientes con ámbito obligatorio

El ámbito es un parámetro que no se puede omitir: en Python era
\`institucion_id: Optional[int] = None\` y al omitirse dejaba de filtrar, que
es la fuga que ya ocurrió una vez con rips_completo.py. Aquí omitirlo no
compila.

El esquema se declara de nuevo en vez de traducir las 52 sentencias de
database.py: aquéllas son el historial de cómo el esquema llegó a su forma
actual, no la forma. Y cod_municipio va TEXT porque 148 municipios empiezan
por cero."
```

---

### Task 3: Configuración que falla al arrancar

**Files:**
- Create: `crates/medico-servidor/Cargo.toml`, `crates/medico-servidor/src/config.rs`
- Test: `crates/medico-servidor/src/config.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `Config { base_datos: String, secreto: String, puerto: u16 }`, `Config::desde_entorno(lector: &dyn Fn(&str) -> Option<String>) -> Result<Config, ErrorConfig>`

El lector va inyectado en vez de leer `std::env` directamente: las pruebas de variables de entorno con `std::env::set_var` son globales al proceso y se pisan entre hilos, que es la misma clase de accidente que en Python hacía que once nombres de base de datos acabaran siendo dos ficheros.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::HashMap;

    fn lector(pares: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> =
            pares.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k: &str| m.get(k).cloned()
    }

    const SECRETO_OK: &str = "clave-de-prueba-con-mas-de-32-caracteres-ok";

    #[test]
    fn con_todo_puesto_arranca() {
        let c = Config::desde_entorno(&lector(&[
            ("DATABASE_URL", "sqlite://medico.db"),
            ("SECRET_KEY", SECRETO_OK),
        ])).unwrap();
        assert_eq!(c.puerto, 8000);
    }

    /// Sin secreto NO se arranca. No hay «modo desarrollo» que lo permita:
    /// ese atajo es justamente el camino por el que `changeme` llega a
    /// producción, y un token firmado con una clave conocida se falsifica —con
    /// lo que todo el aislamiento por institución deja de proteger nada.
    #[test]
    fn sin_secreto_no_arranca() {
        let r = Config::desde_entorno(&lector(&[("DATABASE_URL", "sqlite://m.db")]));
        assert!(matches!(r, Err(ErrorConfig::Falta("SECRET_KEY"))));
    }

    #[test]
    fn sin_base_no_arranca() {
        let r = Config::desde_entorno(&lector(&[("SECRET_KEY", SECRETO_OK)]));
        assert!(matches!(r, Err(ErrorConfig::Falta("DATABASE_URL"))));
    }

    #[test]
    fn un_secreto_corto_no_sirve() {
        let r = Config::desde_entorno(&lector(&[
            ("DATABASE_URL", "sqlite://m.db"),
            ("SECRET_KEY", "corto"),
        ]));
        assert!(matches!(r, Err(ErrorConfig::SecretoDebil(_))));
    }

    /// Los valores que ya circularon en este repositorio se rechazan por nombre.
    /// Están en el historial de git y los sabe cualquiera que lo lea.
    #[test]
    fn los_secretos_conocidos_se_rechazan() {
        for conocido in [
            "changeme",
            "medsys_secret_key_cambie_esto_en_produccion_2026",
            "default_insecure_key_change_in_production",
            "clave-de-prueba-con-mas-de-32-caracteres-ok-NO",
        ] {
            let r = Config::desde_entorno(&lector(&[
                ("DATABASE_URL", "sqlite://m.db"),
                ("SECRET_KEY", conocido),
            ]));
            if conocido.starts_with("clave-de-prueba") { continue; }
            assert!(matches!(r, Err(ErrorConfig::SecretoConocido(_))), "aceptó {conocido}");
        }
    }

    #[test]
    fn el_puerto_se_puede_cambiar_y_uno_invalido_no_se_ignora() {
        let c = Config::desde_entorno(&lector(&[
            ("DATABASE_URL", "sqlite://m.db"), ("SECRET_KEY", SECRETO_OK), ("PORT", "9100"),
        ])).unwrap();
        assert_eq!(c.puerto, 9100);

        // Un PORT ilegible NO cae al valor por defecto en silencio: eso deja el
        // servicio escuchando donde nadie lo busca y parece que «no arrancó».
        let r = Config::desde_entorno(&lector(&[
            ("DATABASE_URL", "sqlite://m.db"), ("SECRET_KEY", SECRETO_OK), ("PORT", "ocho mil"),
        ]));
        assert!(matches!(r, Err(ErrorConfig::ValorInvalido { .. })));
    }
}
```

- [ ] **Step 2: Verlas fallar**

Run: `cargo test -p medico-servidor config`
Expected: FAIL — `Config` no existe.

- [ ] **Step 3: Implementar**

```rust
//! Configuración, y la comprobación de que existe de verdad.
//!
//! Puerto literal de `backend/core/config.py`, incluida su razón: un despliegue
//! sin `.env` arrancaba firmando los tokens de sesión con la cadena `changeme`,
//! que es pública y está en este repositorio.

use thiserror::Error;

/// Secretos que ya circularon por este repositorio o por su `.env.example`.
/// Van por nombre y a mano: es la declaración explícita de «esto ya lo sabe
/// cualquiera», y obliga a añadir el siguiente cuando aparezca.
const SECRETOS_QUEMADOS: &[&str] = &[
    "changeme",
    "medsys_secret_key_cambie_esto_en_produccion_2026",
    "default_insecure_key_change_in_production",
    "your_secret_key_32chars_minimum",
];

const LARGO_MINIMO: usize = 32;

#[derive(Debug, Error, PartialEq)]
pub enum ErrorConfig {
    #[error("falta la variable {0}. El proceso no arranca sin ella: no hay valor por defecto para un secreto.")]
    Falta(&'static str),
    #[error("SECRET_KEY tiene {0} caracteres; hacen falta al menos 32.")]
    SecretoDebil(usize),
    #[error("SECRET_KEY es uno de los valores que ya circularon en este repositorio ({0}). Genere uno nuevo.")]
    SecretoConocido(String),
    #[error("{variable} tiene un valor que no se puede leer: {valor}")]
    ValorInvalido { variable: &'static str, valor: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub base_datos: String,
    pub secreto: String,
    pub puerto: u16,
}

impl Config {
    pub fn desde_entorno(
        lector: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Config, ErrorConfig> {
        let base_datos = lector("DATABASE_URL").ok_or(ErrorConfig::Falta("DATABASE_URL"))?;
        let secreto = lector("SECRET_KEY").ok_or(ErrorConfig::Falta("SECRET_KEY"))?;

        if secreto.len() < LARGO_MINIMO {
            return Err(ErrorConfig::SecretoDebil(secreto.len()));
        }
        if SECRETOS_QUEMADOS.contains(&secreto.as_str()) {
            return Err(ErrorConfig::SecretoConocido(secreto));
        }

        let puerto = match lector("PORT") {
            None => 8000,
            Some(v) => v.parse().map_err(|_| ErrorConfig::ValorInvalido {
                variable: "PORT",
                valor: v,
            })?,
        };

        Ok(Config { base_datos, secreto, puerto })
    }
}
```

`crates/medico-servidor/Cargo.toml`:

```toml
[package]
name = "medico-servidor"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "medico"
path = "src/main.rs"

[dependencies]
medico-dominio = { path = "../medico-dominio" }
medico-datos = { path = "../medico-datos" }
axum = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "signal"] }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
rust-embed = "8"
mime_guess = "2"
argon2 = "0.5"
jsonwebtoken = "9"
tower-http = { version = "0.6", features = ["trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

- [ ] **Step 4: Verlas pasar**

Run: `cargo test -p medico-servidor config`
Expected: PASS, 6 pruebas.

- [ ] **Step 5: Commit**

```bash
git add crates/medico-servidor
git commit -m "feat(rust): la configuración falla al arrancar y no se degrada

Puerto de core/config.py con su razón intacta: no hay valor por defecto para
un secreto, y los que ya circularon por el repositorio se rechazan por nombre.
Se añade que un PORT ilegible tampoco cae al defecto en silencio — eso deja el
servicio escuchando donde nadie lo busca.

El lector del entorno va inyectado: std::env::set_var es global al proceso y
las pruebas se pisarían entre hilos, que es la misma clase de accidente que
hacía que once nombres de base de datos acabaran siendo dos ficheros."
```

---

### Task 4: El frontend embebido en el binario

**Files:**
- Create: `crates/medico-servidor/src/frontend.rs`, `crates/medico-servidor/build.rs`
- Test: `crates/medico-servidor/tests/frontend_embebido.rs`

**Interfaces:**
- Consumes: nada de tareas previas.
- Produces: `frontend::rutas() -> axum::Router<crate::estado::AppState>`, `frontend::PREFIJOS_DEL_BACKEND: &[&str]`

Es el puerto directo de `backend/main.py` + `backend/tests/test_frontend_embebido.py`, con la diferencia de que aquí los archivos van **dentro del ejecutable**: el despliegue pasa a ser un fichero, no un fichero y un directorio.

- [ ] **Step 1: Escribir las pruebas que fallan**

`crates/medico-servidor/tests/frontend_embebido.rs`:

```rust
//! El binario se sirve a sí mismo, y el comodín no tapa el backend.
//!
//! Puerto de backend/tests/test_frontend_embebido.py. La guarda existe porque
//! sin ella `GET /api/pacintes` —con la errata— devuelve 200 con HTML en vez de
//! 404: el cliente recibe una página donde espera JSON y el error aparece mucho
//! más tarde y en otro sitio.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn pedir(ruta: &str) -> (StatusCode, String, Vec<u8>) {
    let app = medico_servidor::app_de_prueba().await;
    let r = app.oneshot(Request::builder().uri(ruta).body(Body::empty()).unwrap())
        .await.unwrap();
    let estado = r.status();
    let tipo = r.headers().get("content-type")
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let cuerpo = r.into_body().collect().await.unwrap().to_bytes().to_vec();
    (estado, tipo, cuerpo)
}

#[tokio::test]
async fn una_ruta_del_backend_que_no_existe_da_404_y_no_html() {
    for ruta in ["/api/pacintes", "/api/modulo-que-no-existe", "/uploads/no-existe.png"] {
        let (estado, tipo, _) = pedir(ruta).await;
        assert_eq!(estado, StatusCode::NOT_FOUND, "{ruta} no dio 404");
        assert!(!tipo.contains("text/html"), "{ruta} devolvió HTML");
    }
}

#[tokio::test]
async fn la_raiz_sirve_el_index() {
    let (estado, tipo, _) = pedir("/").await;
    assert_eq!(estado, StatusCode::OK);
    assert!(tipo.contains("text/html"));
}

/// Recargar la página o abrir un enlace directo tiene que funcionar: Angular
/// resuelve `/pacientes/12` en el navegador y no hay archivo con ese nombre.
#[tokio::test]
async fn una_ruta_del_enrutador_de_angular_devuelve_el_index() {
    let (_, _, index) = pedir("/").await;
    for ruta in ["/pacientes", "/pacientes/12", "/rips/generar"] {
        let (estado, tipo, cuerpo) = pedir(ruta).await;
        assert_eq!(estado, StatusCode::OK, "{ruta}");
        assert!(tipo.contains("text/html"), "{ruta}");
        assert_eq!(cuerpo, index, "{ruta} no devolvió el mismo index.html");
    }
}

/// Un estático real se sirve con su propio tipo, no como HTML.
#[tokio::test]
async fn un_js_se_sirve_como_javascript() {
    let nombre = medico_servidor::frontend::Estaticos::iter()
        .find(|f| f.ends_with(".js"))
        .expect("el build de Angular no trae ningún .js: ¿se compiló?")
        .to_string();
    let (estado, tipo, _) = pedir(&format!("/{nombre}")).await;
    assert_eq!(estado, StatusCode::OK);
    assert!(tipo.contains("javascript"), "tipo fue {tipo}");
}

/// La lista de prefijos protegidos tiene que cubrir lo que el router publica.
/// Se compara contra las rutas REALES, no contra la lista consigo misma.
#[tokio::test]
async fn los_prefijos_cubren_lo_que_publica_el_router() {
    let publicadas = medico_servidor::rutas::prefijos_publicados();
    assert!(publicadas.contains(&"api".to_string()), "la lectura no vio /api");
    for p in &publicadas {
        assert!(
            medico_servidor::frontend::PREFIJOS_DEL_BACKEND
                .iter().any(|g| g.trim_start_matches('/') == p),
            "el router publica /{p} y no está en PREFIJOS_DEL_BACKEND: una URL \
             equivocada bajo ese prefijo devolvería el index.html con 200"
        );
    }
}
```

- [ ] **Step 2: Verlas fallar**

Run: `cargo test -p medico-servidor --test frontend_embebido`
Expected: FAIL — `app_de_prueba` y `frontend` no existen.

- [ ] **Step 3: Implementar**

`crates/medico-servidor/build.rs` — que la ausencia del build no se descubra en producción:

```rust
//! El binario de release NO se construye sin el frontend dentro.
//!
//! Sin esto, `cargo build --release` produce un ejecutable que arranca, sirve
//! el API y responde 404 a la interfaz. Es la clase de fallo que se descubre en
//! la instalación del cliente, no en la máquina de quien compila.

use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../frontend/dist/frontend/browser");
    println!("cargo:rerun-if-changed={}", dir.display());

    let hay = dir.join("index.html").exists();
    let release = std::env::var("PROFILE").as_deref() == Ok("release");

    if release && !hay {
        panic!(
            "no hay build de Angular en {}. Un binario de release sin la \
             interfaz dentro no es el producto. Ejecute: cd frontend && \
             npm run build",
            dir.display()
        );
    }
    if !hay {
        println!("cargo:warning=sin build de Angular; el binario servirá solo el API");
    }
}
```

`crates/medico-servidor/src/frontend.rs`:

```rust
//! El Angular compilado, dentro del ejecutable.
//!
//! `rust-embed` mete los archivos en el binario, así que el despliegue en sitio
//! es UN fichero. Es lo que decidió la tarea #73 —«el costo de Angular es de
//! compilación, no de despliegue»— llevado hasta el final.

use axum::body::Body;
use axum::extract::Path as RutaPath;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../frontend/dist/frontend/browser"]
pub struct Estaticos;

/// Prefijos que pertenecen al backend. El comodín del SPA NO los toca.
///
/// Sin esta lista, `GET /api/pacintes` devolvería el index.html con un 200. Una
/// ruta de API que no existe tiene que dar 404.
pub const PREFIJOS_DEL_BACKEND: &[&str] = &["/api", "/uploads", "/health"];

fn servir(ruta: &str) -> Option<Response> {
    Estaticos::get(ruta).map(|f| {
        let tipo = mime_guess::from_path(ruta).first_or_octet_stream();
        ([(header::CONTENT_TYPE, tipo.as_ref())], Body::from(f.data.into_owned()))
            .into_response()
    })
}

/// Comodín: archivo si existe, `index.html` si es una ruta del enrutador de
/// Angular, y 404 si cae bajo un prefijo del backend.
pub async fn comodin(uri: Uri) -> Response {
    let ruta = uri.path();

    if PREFIJOS_DEL_BACKEND.iter().any(|p| ruta.starts_with(p)) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Some(r) = servir(ruta.trim_start_matches('/')) {
        return r;
    }
    match servir("index.html") {
        Some(r) => r,
        // Sin frontend embebido esto es un binario solo-API. Se dice, no se
        // finge: un 404 mudo aquí se confunde con «la ruta no existe».
        None => (
            StatusCode::NOT_FOUND,
            "Este binario se compiló sin la interfaz. Use el API en /api o \
             recompile con el build de Angular presente.",
        ).into_response(),
    }
}
```

En `src/rutas/mod.rs`, la función que la prueba consulta:

```rust
/// Primer segmento de cada ruta que el router publica. Se lee del router real,
/// no de una lista escrita a mano: una lista a mano se queda atrás y la prueba
/// que la usa pasa sin medir nada.
pub fn prefijos_publicados() -> Vec<String> {
    RUTAS_PUBLICADAS
        .iter()
        .filter_map(|r| r.trim_start_matches('/').split('/').next())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Registro central de lo que monta `router()`. Toda ruta nueva se añade aquí
/// **y** al router; la prueba de prefijos falla si una queda sin proteger.
pub const RUTAS_PUBLICADAS: &[&str] = &[
    "/health",
    "/api/auth/login",
    "/api/auth/me",
    "/api/pacientes",
    "/api/pacientes/{id}",
];
```

- [ ] **Step 4: Compilar el frontend y ver pasar las pruebas**

```bash
cd frontend && npm run build && cd ..
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p medico-servidor --test frontend_embebido
```
Expected: PASS, 5 pruebas.

- [ ] **Step 5: Comprobar que la guarda discrimina**

Una salvaguarda que nunca se ha visto fallar está sin estrenar. Comentar temporalmente el bloque `if PREFIJOS_DEL_BACKEND...` de `comodin`, correr `cargo test --test frontend_embebido`, y **confirmar que `una_ruta_del_backend_que_no_existe_da_404_y_no_html` falla**. Restaurar.

Expected: la prueba falla con `/api/pacintes devolvió HTML`.

- [ ] **Step 6: Commit**

```bash
git add crates/medico-servidor/src/frontend.rs crates/medico-servidor/build.rs crates/medico-servidor/tests/frontend_embebido.rs
git commit -m "feat(rust): el Angular va dentro del ejecutable

El despliegue en sitio pasa a ser UN fichero. build.rs hace fallar la
compilación de release si no hay build de Angular: un binario de release sin
interfaz dentro no es el producto, y ese fallo se descubriría en la
instalación del cliente y no en la máquina de quien compila.

La guarda de PREFIJOS_DEL_BACKEND se comprobó quitándosela: sin ella
/api/pacintes devuelve 200 con HTML. Y la prueba que vigila la lista lee las
rutas del router en vez de una lista a mano — en Python esa misma prueba se
escribió primero contra app.routes y pasaba sin ver ninguna de las 180 rutas
bajo /api."
```

---

### Task 5: Identidad — Argon2id y JWT

**Files:**
- Create: `crates/medico-servidor/src/rutas/auth.rs`, `crates/medico-servidor/src/error.rs`, `crates/medico-servidor/src/estado.rs`
- Modify: `crates/medico-datos/src/lib.rs` (añadir `pub mod usuario;`), crear `crates/medico-datos/src/usuario.rs`
- Test: `crates/medico-servidor/tests/auth.rs`

**Interfaces:**
- Consumes: `Config`, `medico_datos::Pool`, `medico_dominio::RolUsuario`
- Produces: `AppState { pool, secreto }`, `auth::firmar(&AppState, &Usuario) -> String`, `auth::Sesion` como extractor de axum con campos `usuario_id: i64`, `ambito: Ambito`, `rol: RolUsuario`

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
//! Puerto de backend/tests/test_login.py.

#[tokio::test]
async fn con_la_clave_correcta_se_entra_y_se_recibe_token() { /* ... */ }

#[tokio::test]
async fn con_la_clave_equivocada_no_se_entra() { /* ... */ }

/// Usuario que existe y usuario que no, dan la MISMA respuesta y en tiempo
/// comparable. Distinguirlos convierte el login en un oráculo de qué correos
/// están registrados en la clínica.
#[tokio::test]
async fn un_usuario_inexistente_responde_igual_que_una_clave_mala() { /* ... */ }

/// Un usuario inactivo no entra. En Python esto además significa que no
/// consume cupo de licencia (commit 12358a6); aquí basta con que no entre.
#[tokio::test]
async fn un_usuario_inactivo_no_entra() { /* ... */ }

/// El token lleva el ámbito, y un token firmado con OTRA clave se rechaza.
#[tokio::test]
async fn un_token_de_otra_clave_no_vale() { /* ... */ }

/// El superadmin recibe Ambito::Superadmin, no Ambito::Ninguno. Es la
/// conversión donde un descuido concede todo el sistema o lo cierra entero.
#[tokio::test]
async fn el_superadmin_recibe_su_ambito() { /* ... */ }
```

*(El cuerpo de cada prueba se escribe al ejecutar la tarea, siguiendo el patrón de `pedir()` de la Task 4 con `POST` y cuerpo JSON. Se listan aquí con su nombre y su porqué porque el porqué es lo que no se puede reconstruir después.)*

- [ ] **Step 2: Verlas fallar** — `cargo test -p medico-servidor --test auth`
- [ ] **Step 3: Implementar** Argon2id (`argon2::Argon2::default()`, `PasswordHash`), JWT con `jsonwebtoken` y el secreto de `Config`, extractor `FromRequestParts` para `Sesion`.
- [ ] **Step 4: Verlas pasar**
- [ ] **Step 5: Commit**

---

### Task 6: `/api/pacientes` de punta a punta

**Files:**
- Create: `crates/medico-servidor/src/rutas/pacientes.rs`, `crates/medico-servidor/tests/aislamiento.rs`
- Modify: `crates/medico-servidor/src/rutas/mod.rs`, `crates/medico-servidor/src/main.rs`

**Interfaces:**
- Consumes: `Sesion`, `medico_datos::paciente::{listar, obtener}`
- Produces: el binario `medico` completo de este plan.

- [ ] **Step 1: Escribir la prueba que falla** — puerto de `backend/tests/test_aislamiento_institucion.py`: con sesión de la institución 1, `GET /api/pacientes` trae solo los suyos; `GET /api/pacientes/20` (de la institución 2) devuelve **404**, idéntico byte a byte a `GET /api/pacientes/9999`.
- [ ] **Step 2: Verla fallar**
- [ ] **Step 3: Implementar** los dos manejadores, que reciben `Sesion` y pasan `sesion.ambito` a `medico_datos`.
- [ ] **Step 4: Verla pasar**
- [ ] **Step 5: Arrancar el binario de verdad**

```bash
cd frontend && npm run build && cd ..
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL=sqlite://./medico.db SECRET_KEY=$(openssl rand -hex 32) cargo run --release -p medico-servidor
curl -s localhost:8000/health
curl -s -o /dev/null -w '%{http_code}\n' localhost:8000/pacientes   # 200, index.html
curl -s -o /dev/null -w '%{http_code}\n' localhost:8000/api/pacintes # 404
```

- [ ] **Step 6: Commit**

---

## Cómo se sabe que este plan terminó

Las cinco afirmaciones, todas comprobables con un comando:

1. `cargo test` pasa entero en el workspace.
2. `cargo build --release` produce **un** ejecutable que abre `medico.db` si no existe y sirve la interfaz sin ningún archivo al lado.
3. `GET /api/pacientes/{ajeno}` y `GET /api/pacientes/{inexistente}` devuelven respuestas idénticas.
4. Arrancar sin `SECRET_KEY` **no** levanta el servicio y dice qué falta.
5. `cargo build --release` **falla** si no hay build de Angular.

Lo que este plan **no** demuestra, y hay que decirlo: que el sistema en Rust produzca los mismos RIPS que el de Python. Eso es el Plan 5 y hasta entonces `backend/` no se toca.
