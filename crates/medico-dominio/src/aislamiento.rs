//! Quién puede ver qué, sin tocar la base de datos.
//!
//! Es la regla que impide que una institución lea las historias clínicas de
//! otra. Vive aquí, separada de SQLx, por dos razones:
//!
//! 1. Se prueba exhaustivamente sin base ni servidor.
//! 2. La capa de datos no puede conceder una fila sin pasar por aquí, porque no
//!    tiene otra forma de construir un [`Visibilidad`].
//!
//! === POR QUÉ ESTO NO ES UNA TRADUCCIÓN DE `core/aislamiento.py` ===
//!
//! En Python la regla estaba enredada con el ORM: `obtener()` recorría una
//! cadena de enlaces (`paciente_id` → `Paciente.institucion_id`) por `getattr`
//! sobre instancias de SQLModel, porque el ORM ofrecía ese grafo. Con SQLx no
//! hay grafo de objetos, y esa fue la objeción concreta contra migrar.
//!
//! La respuesta no es reconstruir el grafo: es que **el dueño lo resuelve la
//! consulta**, que es donde SQL es bueno, y aquí solo queda la decisión. Lo que
//! en Python eran 169 llamadas a `db.get` que había que recordar envolver, aquí
//! es un parámetro que no se puede omitir sin que deje de compilar.

use serde::{Deserialize, Serialize};

/// Desde dónde mira quien pregunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambito {
    /// Dueño del sistema multi-institucional. No es un cliente: en la base
    /// tiene `institucion_id = NULL`.
    Superadmin,
    /// Usuario adscrito a una institución concreta.
    Institucion(i64),
    /// Ni superadmin ni adscrito. No ve nada.
    ///
    /// Existe como variante propia para que «sin institución» no pueda
    /// confundirse con «superadmin»: los dos eran `institucion_id = None` en
    /// Python, y distinguirlos dependía de mirar además el rol en cada sitio.
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

/// Deliberadamente **no** hay variante `Prohibido`.
///
/// Un 403 confirma que el registro existe en otra institución, y con eso se
/// enumera: se prueban identificaciones hasta que una deja de dar 404 y ya se
/// sabe que esa persona es paciente de la clínica de al lado. Eso también es un
/// dato de salud.
///
/// Al no existir el valor, ningún camino del código puede filtrarlo por
/// descuido. Es la diferencia entre una regla que hay que recordar y una que no
/// se puede romper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibilidad {
    Si,
    NoExiste,
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_propio_se_ve() {
        assert_eq!(Ambito::institucion(7).puede_ver(Some(7)), Visibilidad::Si);
    }

    /// LA REGLA QUE IMPORTA: lo ajeno es indistinguible de lo inexistente.
    #[test]
    fn lo_ajeno_y_lo_inexistente_dan_lo_mismo() {
        let a = Ambito::institucion(7);
        assert_eq!(a.puede_ver(Some(8)), Visibilidad::NoExiste);
        assert_eq!(a.puede_ver(None), Visibilidad::NoExiste);
    }

    #[test]
    fn el_superadmin_ve_todo() {
        assert_eq!(Ambito::Superadmin.puede_ver(Some(1)), Visibilidad::Si);
        assert_eq!(Ambito::Superadmin.puede_ver(Some(999)), Visibilidad::Si);
    }

    /// Un registro sin dueño no se regala.
    ///
    /// En Python, un modelo sin `institucion_id`, sin enlace conocido y no
    /// declarado como catálogo compartido levantaba `RuntimeError` a propósito:
    /// si mañana alguien añade un modelo clínico y olvida decir de quién es,
    /// tiene que romperse en una prueba y no en producción sirviendo datos
    /// ajenos a quien los pida. Aquí el equivalente es que `None` no significa
    /// «de todos».
    #[test]
    fn un_registro_sin_duenio_no_se_regala() {
        assert_eq!(
            Ambito::institucion(7).puede_ver(None),
            Visibilidad::NoExiste
        );
    }

    /// Sin institución y sin ser superadmin, no se ve nada — y en particular no
    /// se ve todo, que es el fallo que esta variante existe para hacer
    /// imposible.
    #[test]
    fn sin_institucion_y_sin_ser_superadmin_no_se_ve_nada() {
        assert_eq!(Ambito::Ninguno.puede_ver(Some(1)), Visibilidad::NoExiste);
        assert_eq!(Ambito::Ninguno.puede_ver(None), Visibilidad::NoExiste);
    }

    /// Barrido: para cualquier par de instituciones distintas, en ningún caso
    /// se concede. No es una prueba de ejemplo, es de la regla entera sobre un
    /// rango — 100×100 combinaciones (directiva 8).
    #[test]
    fn ninguna_institucion_ve_a_ninguna_otra() {
        for mia in 1..=100i64 {
            for suya in 1..=100i64 {
                let v = Ambito::institucion(mia).puede_ver(Some(suya));
                if mia == suya {
                    assert_eq!(v, Visibilidad::Si, "{mia} no se ve a sí misma");
                } else {
                    assert_eq!(v, Visibilidad::NoExiste, "{mia} vio a {suya}");
                }
            }
        }
    }
}
