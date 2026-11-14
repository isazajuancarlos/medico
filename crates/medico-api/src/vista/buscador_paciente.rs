// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **Elegir un paciente sin cargar el padrón**, en un sitio y para todos los
//! formularios que lo piden.
//!
//! Hasta el 2026-08-08 cuatro pantallas —despacho de ambulancias, programación
//! de quirófano y las dos del alta de nutrición— pintaban un `<select>` con
//! **todas** las fichas de la institución. En un consultorio da igual; en un
//! hospital con decenas de miles es una página que no se abre. Y la versión de
//! Angular tenía la otra mitad del problema y era peor: pedía las primeras 500 y
//! a partir de ahí un paciente **no se podía elegir y nada lo decía**.
//!
//! # La pieza es una sola porque la regla es una sola
//!
//! Cuatro buscadores copiados divergen en el primer arreglo (directiva 9): uno
//! plegaría las tildes y otro no, uno diría cuántos quedan fuera y otro
//! truncaría en silencio. Aquí el fragmento es el mismo, y por tanto también lo
//! es la frase que se lee cuando hay más de los que caben.
//!
//! # El contrato con el formulario que la usa NO cambia
//!
//! Lo que se envía sigue siendo `paciente_id`, con el mismo nombre y el mismo
//! `<select required>`, así que ningún manejador de POST se entera de este
//! cambio. Lo único que se añade es el campo de texto `q_paciente`, que viaja
//! con el formulario para que la búsqueda **sobreviva a un error de validación**
//! —si no, quien se equivoca en una fecha vuelve a una lista vacía y tiene que
//! encontrar a su paciente otra vez—.
//!
//! # Y deja eslabón en la bitácora
//!
//! Devuelve nombre y documento de varios titulares, así que es exactamente la
//! forma de `/api/pacientes`: `Coleccion` sin ids, con el texto buscado como
//! filtro (ver la sección de la bitácora en `CLAUDE.md`). Un buscador que no
//! anotara sería la puerta de atrás del rastro que las cinco pantallas de al
//! lado sí dejan.

use axum::extract::{Query, State};
use maud::{html, Markup};
use medico_datos::paciente::{Criterio, Paciente};
use serde::Deserialize;

use crate::auth::Estado;
use crate::vista::EnSesion;

/// La ruta del fragmento.
///
/// **Cuelga de `/panel/buscar/` y no de `/panel/pacientes/`** por dos razones, y
/// las dos importan: bajo `pacientes` chocaría con `/panel/pacientes/{id}` —una
/// ficha llamada «elegir»—, y quedaría además dentro del módulo PACIENTES de la
/// licencia, así que un cliente que compró AMBULANCIAS y no PACIENTES se
/// quedaría sin poder despachar. Es el sesgo a cerrar (directiva 21) escondido
/// en una decisión de rutas.
pub const RUTA: &str = "/panel/buscar/paciente";

/// Cuántos se enseñan. Los que no caben **se cuentan y se dicen**, nunca se
/// tiran en silencio (directiva 22).
///
/// Veinte es lo que cabe en una lista sin scroll propio en un portátil de
/// consultorio. No es un límite del sistema: es lo que se enseña de una vez.
pub const TECHO: i64 = 20;

/// El identificador del hueco donde entra la lista. Lo comparten el campo y el
/// fragmento, así que no puede haber dos nombres para lo mismo.
const HUECO: &str = "opciones-de-paciente";

#[derive(Deserialize, Default)]
pub struct Busqueda {
    #[serde(default)]
    pub q_paciente: String,
    /// El que ya estuviera elegido, para no perderlo al escribir.
    #[serde(default)]
    pub paciente_id: String,
}

/// El campo listo para meter en un formulario: la caja de texto y el hueco con
/// la lista.
///
/// `elegido` es lo que el formulario traía puesto (el `paciente_id` de un envío
/// que falló), y `q` el texto que se había escrito.
pub fn campo(q: &str, elegido: &str, encontrados: &[Paciente], total: i64) -> Markup {
    html! {
        label for="q_paciente" { "Paciente" }
        // `input changed delay` y no `keyup`: sin el retardo se dispara una
        // consulta por tecla. `search` cubre la equis de limpiar del navegador,
        // que no emite `input` en todos.
        input #q_paciente type="search" name="q_paciente" value=(q)
              placeholder="Nombre o documento"
              autocomplete="off"
              hx-get=(RUTA)
              hx-trigger="input changed delay:300ms, search"
              hx-target={ "#" (HUECO) }
              hx-swap="innerHTML"
              hx-include="[name='paciente_id']";
        div id=(HUECO) { (lista(q, elegido, encontrados, total)) }
    }
}

/// La lista en sí: el `<select>` con lo que casó y, si hay más, cuántos.
fn lista(q: &str, elegido: &str, encontrados: &[Paciente], total: i64) -> Markup {
    let sobran = total - encontrados.len() as i64;
    html! {
        label class="oculto" for="paciente_id" { "Paciente elegido" }
        select #paciente_id name="paciente_id" required size="6" {
            option value="" { "Elija uno" }
            @for p in encontrados {
                // El MISMO criterio de nombre que el listado de pacientes: dos
                // copias saldrían con otro orden y el mismo paciente se leería
                // distinto en dos pantallas.
                option value=(p.id) selected[elegido == p.id.to_string()] {
                    (crate::vista::pacientes::nombre_completo(p))
                    " — " (p.numero_documento)
                }
            }
        }
        @if sobran > 0 {
            // Se DICE cuántos quedan fuera. Sin esta frase, el paciente 21 no
            // existe para quien mira, que es el defecto exacto que tenía la
            // pantalla de Angular con sus primeros 500.
            p class="sutil" {
                "Coinciden " (total) "; se enseñan " (encontrados.len()) ". "
                "Escriba un poco más para acotar."
            }
        } @else if encontrados.is_empty() && !q.trim().is_empty() {
            p class="sutil" { "Ningún paciente coincide con «" (q) "»." }
        } @else if encontrados.is_empty() {
            p class="sutil" {
                "No hay pacientes registrados: esto se registra a nombre de alguien."
            }
        }
    }
}

/// Lo que hay que consultar para pintar el campo. Se comparte para que la
/// pantalla que dibuja el formulario y el fragmento que responde al tecleo
/// pregunten **lo mismo**.
pub async fn encontrar(
    e: &Estado,
    u: &medico_datos::usuario::Usuario,
    q: &str,
) -> Result<(Vec<Paciente>, i64), sqlx::Error> {
    let h = medico_datos::paciente::buscar(
        &e.pool,
        u.ambito(),
        &Criterio {
            buscar: (!q.trim().is_empty()).then(|| q.trim()),
            limite: Some(TECHO),
        },
    )
    .await?;
    Ok((h.filas, h.total))
}

/// `GET /panel/buscar/paciente?q_paciente=&paciente_id=` — el fragmento.
pub async fn fragmento(
    State(e): State<Estado>,
    EnSesion(u): EnSesion,
    Query(b): Query<Busqueda>,
) -> Markup {
    let (encontrados, total) = match encontrar(&e, &u, &b.q_paciente).await {
        Ok(x) => x,
        // Un fragmento no puede devolver la página de avería: se swapearía
        // dentro del formulario. Dice lo que pasa, en su sitio.
        Err(_) => {
            return html! {
                p class="sutil" { "No se pudo consultar el padrón. Vuelva a intentarlo." }
            }
        }
    };
    crate::rutas::registrar_coleccion(
        &e,
        &u,
        "listar",
        crate::rutas::RECURSO_PACIENTES,
        crate::rutas::filtro_de(&[("buscar", Some(b.q_paciente.as_str()))]),
        encontrados.len(),
        vec![],
    )
    .await;
    lista(&b.q_paciente, &b.paciente_id, &encontrados, total)
}
