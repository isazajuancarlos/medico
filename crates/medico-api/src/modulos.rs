// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Qué módulos ve esta institución (#65, punto 6).
//!
//! Es la pieza que permite vender el MISMO binario a un consultorio, una IPS,
//! una clínica y un hospital: lo que cambia no es el código, es la licencia. El
//! frontend arma su menú desde aquí en vez de traer una lista fija.
//!
//! **Sin esto no se puede dar de alta un cliente nuevo sin tocar la base a
//! mano**, que es lo que la ficha de #65 señala como bloqueo.
//!
//! # Quién puede qué
//!
//! - **Leer** el catálogo: cualquiera autenticado. Es su propio menú.
//! - **Cambiar** la licencia: `GestionarInstituciones`, o sea **solo el
//!   superadmin**. Qué módulos compró un cliente es una decisión del vendedor,
//!   no del administrador del cliente — dejársela a éste sería regalarle el
//!   producto entero. Es la misma razón por la que esa capacidad no se puede
//!   configurar desde la matriz de una institución.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use medico_datos::institucion;
use medico_dominio::modulo::{self, EstadoModulo};
use serde::{Deserialize, Serialize};

use crate::auth::{Autenticado, Autorizado, Estado, GestionarInstituciones};

#[derive(Debug, Serialize)]
pub struct ModuloVista {
    pub modulo: &'static str,
    pub estado: EstadoModulo,
}

#[derive(Debug, Serialize)]
pub struct GrupoVista {
    pub grupo: &'static str,
    pub modulos: Vec<ModuloVista>,
}

#[derive(Debug, Serialize)]
pub struct Catalogo {
    pub institucion_id: i64,
    /// Lo licenciado, ordenado. Se manda aparte de los grupos porque el menú lo
    /// usa para decidir de un vistazo, sin recorrer el catálogo entero.
    pub habilitados: Vec<String>,
    pub grupos: Vec<GrupoVista>,
}

/// Las rutas que este binario sirve de verdad.
///
/// Se extrae del router en tiempo de compilación leyendo el propio fuente, y no
/// se escribe a mano, porque una lista de «lo que ya está hecho» mantenida
/// aparte se desincroniza al primer módulo nuevo — y entonces el producto
/// promete al cliente algo que no existe. La prueba
/// `el_catalogo_no_miente_sobre_lo_que_el_binario_sirve` lo comprueba.
///
/// # Se lee SOLO el bloque del router, y esa acotación es la pieza
///
/// La primera versión buscaba en `rutas.rs` **entero**, que son ~9 000 líneas
/// con sus pruebas dentro. El día que una prueba escriba `"/api/quirofano/salas"`
/// para comprobar que devuelve 404, esta función diría que sí y el catálogo
/// anunciaría como habilitado un módulo que no existe — justo lo que la pieza se
/// escribió para impedir, y sin un solo error. Lo señaló la revisión de la rama
/// de estadísticas antes de que ocurriera; `solo_cuenta_lo_que_esta_en_el_router`
/// lo sujeta.
fn sirve(prefijo: &str) -> bool {
    let rutas = bloque_del_router();
    rutas.contains(&format!("\"{prefijo}\"")) || rutas.contains(&format!("\"{prefijo}/"))
}

/// Del `Router::new()` al `.fallback(`, que es donde acaban las rutas.
fn bloque_del_router() -> &'static str {
    const RUTAS: &str = include_str!("rutas.rs");
    let inicio = RUTAS
        .find("Router::new()")
        .expect("rutas.rs sin `Router::new()`: el catálogo no puede saber qué se sirve");
    let fin = RUTAS[inicio..]
        .find(".fallback(")
        .map(|x| inicio + x)
        .expect("el router no cierra con `.fallback(`");
    &RUTAS[inicio..fin]
}

fn construir(institucion_id: i64, habilitados: &[String]) -> Catalogo {
    let grupos = modulo::grupos()
        .into_iter()
        .map(|g| GrupoVista {
            grupo: g,
            modulos: modulo::CATALOGO
                .iter()
                .filter(|m| m.grupo == g)
                .map(|m| ModuloVista {
                    modulo: m.nombre,
                    estado: m.estado(habilitados, sirve),
                })
                .collect(),
        })
        .collect();
    let mut hab = habilitados.to_vec();
    hab.sort();
    Catalogo { institucion_id, habilitados: hab, grupos }
}

/// De qué institución habla. Quien pertenece a una usa la suya y el parámetro se
/// ignora; el superadmin dice cuál; quien no tiene ninguna no administra nada.
///
/// Decide sobre el ÁMBITO y no sobre la columna `institucion_id`, por la misma
/// razón que `capacidades::institucion_de` y `rutas::institucion_objetivo`:
/// tratar «sin institución» como «superadmin» deja a un `admin` con
/// `institucion_id = NULL` elegir la ajena. Hoy `listar` solo llama con `None`
/// —así que el bug era inerte aquí, no explotable—, pero el patrón se unifica
/// para que no reviva el día que alguien le pase un id del cliente (directiva 1).
fn institucion_de(
    u: &medico_datos::usuario::Usuario,
    pedida: Option<i64>,
) -> Result<i64, (StatusCode, &'static str)> {
    match u.ambito() {
        medico_dominio::Ambito::Institucion(propia) => Ok(propia),
        medico_dominio::Ambito::Superadmin => pedida.ok_or((
            StatusCode::BAD_REQUEST,
            "sin institución propia: indique cuál",
        )),
        medico_dominio::Ambito::Ninguno => Err((
            StatusCode::FORBIDDEN,
            "su cuenta no pertenece a ninguna institución",
        )),
    }
}

/// `GET /api/modulos` — el catálogo con el estado de cada módulo.
pub async fn listar(
    State(e): State<Estado>,
    Autenticado(u): Autenticado,
) -> Result<Json<Catalogo>, Response> {
    let id = institucion_de(&u, None).map_err(IntoResponse::into_response)?;
    let habilitados = institucion::modulos_habilitados(&e.pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    Ok(Json(construir(id, &habilitados)))
}

#[derive(Debug, Deserialize)]
pub struct FijarModulos {
    pub modulos: Vec<String>,
}

/// `PUT /api/instituciones/{id}/modulos` — cambia la licencia. **Superadmin.**
///
/// Un nombre que no está en el catálogo se **rechaza**, no se filtra en
/// silencio: guardar una licencia con `TELETRANSPORTE` dentro la dejaría inerte
/// y nadie sabría por qué ese cliente no ve el módulo que cree haber comprado.
pub async fn fijar(
    State(e): State<Estado>,
    _autorizado: Autorizado<GestionarInstituciones>,
    Path(id): Path<i64>,
    Json(p): Json<FijarModulos>,
) -> Result<Json<Catalogo>, Response> {
    let desconocidos: Vec<&str> = p
        .modulos
        .iter()
        .map(|s| s.as_str())
        .filter(|n| !modulo::existe(n))
        .collect();
    if !desconocidos.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("estos módulos no existen en el catálogo: {desconocidos:?}"),
        )
            .into_response());
    }

    let existia = institucion::fijar_modulos(&e.pool, id, &p.modulos)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    if !existia {
        return Err(StatusCode::NOT_FOUND.into_response());
    }

    let ahora = institucion::modulos_habilitados(&e.pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .unwrap_or_default();
    Ok(Json(construir(id, &ahora)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **El catálogo no puede mentir sobre lo que el binario sirve.**
    ///
    /// Cada módulo con `prefijo` declarado afirma «si el router sirve esto,
    /// estoy construido». Si alguien escribe un prefijo que no existe —una
    /// errata, o una ruta que se renombró—, el módulo quedaría PENDIENTE para
    /// siempre y el cliente vería «aún no está» sobre algo que sí está. Al
    /// revés no puede pasar: sin prefijo es pendiente por definición.
    ///
    /// Es la misma forma que `ningun_borrado_se_conforma_con_estar_autenticado`:
    /// se comprueba contra el fuente, no caso por caso, para que un módulo nuevo
    /// no dependa de que alguien se acuerde.
    #[test]
    fn el_catalogo_no_miente_sobre_lo_que_el_binario_sirve() {
        let rotos: Vec<&str> = modulo::CATALOGO
            .iter()
            .filter_map(|m| m.prefijo.filter(|p| !sirve(p)).map(|_| m.nombre))
            .collect();
        assert!(
            rotos.is_empty(),
            "estos módulos declaran una ruta que el router NO sirve, así que se \
             mostrarían como «pendientes» para siempre: {rotos:?}",
        );
    }

    /// **La otra dirección: construido y declarado PENDIENTE.**
    ///
    /// La prueba de arriba solo caza «declara una ruta que no existe». Su pareja
    /// —un módulo sin prefijo cuya ruta el router SÍ sirve— no se queja nunca, y
    /// llevaba tiempo pasando: `RIPS` estaba servido y anunciado como pendiente,
    /// así que un cliente que lo licenció leía «aún no está» sobre un módulo que
    /// el binario le estaba sirviendo. Lo mismo iba a repetirse con
    /// `ESTADISTICAS` al portarlo, que es cuando salió (directiva 23: al arreglar
    /// el lado ruidoso, busca el falso negativo que comparte causa).
    ///
    /// La regla mira la CONVENCIÓN del proyecto —`NUTRICION` → `/api/nutricion`,
    /// con guiones o sin ellos— y no una lista a mano. Lo que promete es que
    /// **nadie que siga la convención se quede sin declarar**; los que no la
    /// siguen quedan fuera de su alcance, y son SEIS, no cuatro:
    ///
    /// | módulo | ruta real | por qué se escapa |
    /// |---|---|---|
    /// | `EPICRISIS`, `INTERCONSULTAS`, `EVOLUCIONES`, `NOTA_OPERATORIA` | `/api/documentos` | cuatro módulos, una espina genérica |
    /// | `HISTORIA` | `/api/historias` | el nombre es singular y la ruta plural |
    /// | `AUDITORIA` | `/api/acceso` | la ruta no se llama como el módulo |
    ///
    /// La primera versión de esta nota decía «los cuatro documentos clínicos» y
    /// se dejaba dos: enumerar de menos en la documentación de un guardián es
    /// prometer cobertura que no hay. Lo contó la revisión, simulando el mutante
    /// módulo por módulo — de 23, caza 17.
    #[test]
    fn ningun_modulo_construido_se_anuncia_como_pendiente() {
        let mudos: Vec<String> = modulo::CATALOGO
            .iter()
            .filter(|m| m.prefijo.is_none())
            .filter_map(|m| {
                let bajo = m.nombre.to_lowercase();
                [format!("/api/{}", bajo.replace('_', "-")), format!("/api/{bajo}")]
                    .into_iter()
                    .find(|r| sirve(r))
                    .map(|r| format!("{} sirve {r}", m.nombre))
            })
            .collect();
        assert!(
            mudos.is_empty(),
            "estos módulos están CONSTRUIDOS y el catálogo los anuncia como pendientes, \
             así que el cliente que los compró lee «aún no está»: {mudos:?}. \
             Declárales su `prefijo` en `medico_dominio::modulo::CATALOGO`."
        );
    }

    /// **Solo cuenta lo que está en el `Router::new()`.**
    ///
    /// Es la pareja de `sirve`: el archivo entero son ~9 000 líneas y las
    /// pruebas nombran rutas a montones. El caso rojo usa una ruta que aparece
    /// LITERALMENTE en este archivo y **no** está registrada — si `sirve` leyera
    /// el fuente completo diría que sí.
    #[test]
    fn solo_cuenta_lo_que_esta_en_el_router() {
        const SENUELO: &str = "/api/modulo-que-solo-existe-en-esta-prueba";
        let fuente = include_str!("rutas.rs");
        assert!(
            bloque_del_router().len() < fuente.len(),
            "el bloque del router es el archivo entero: la acotación no está haciendo nada"
        );
        assert!(
            bloque_del_router().contains("/api/pacientes"),
            "CONTROL EN ROJO: el bloque no contiene ni las rutas reales"
        );
        assert!(!sirve(SENUELO), "«{SENUELO}» no está registrada y sirve() dice que sí");
    }

    /// Y que `sirve` de verdad discrimine: si devolviera `true` siempre, la
    /// prueba de arriba pasaría con el catálogo entero inventado.
    #[test]
    fn sirve_distingue_una_ruta_real_de_una_inventada() {
        assert!(sirve("/api/pacientes"));
        assert!(sirve("/api/nutricion"));
        assert!(!sirve("/api/teletransporte"));
        assert!(!sirve("/api/kardex"), "KARDEX no está construido");
    }

    #[test]
    fn el_catalogo_se_pinta_por_grupos_y_en_orden() {
        let c = construir(1, &["CITAS".to_string()]);
        assert_eq!(c.institucion_id, 1);
        assert_eq!(c.grupos.len(), 10);
        assert_eq!(c.grupos[0].grupo, "Core");
        // CITAS licenciado y servido; PACIENTES no licenciado.
        let core = &c.grupos[0].modulos;
        let citas = core.iter().find(|m| m.modulo == "CITAS").unwrap();
        let pacientes = core.iter().find(|m| m.modulo == "PACIENTES").unwrap();
        assert_eq!(citas.estado, EstadoModulo::Habilitado);
        assert_eq!(pacientes.estado, EstadoModulo::NoIncluido);
        // Y uno licenciado pero sin construir. **No se escribe a mano**: se pide
        // al catálogo, porque un nombre fijo aquí se pone rojo —o peor, se queda
        // verde sin medir— el día que ese módulo se construya.
        let pendiente = medico_dominio::modulo::alguno_pendiente()
            .expect("no queda ningún módulo pendiente: este caso ya no mide nada y hay que retirarlo");
        let c2 = construir(1, &[pendiente.nombre.to_string()]);
        let visto = c2
            .grupos
            .iter()
            .flat_map(|g| &g.modulos)
            .find(|m| m.modulo == pendiente.nombre)
            .unwrap();
        assert_eq!(visto.estado, EstadoModulo::Pendiente);
    }

    /// Todos los módulos del catálogo aparecen en algún grupo: ninguno se pierde
    /// por un grupo mal escrito, que dejaría un módulo comprado invisible.
    #[test]
    fn ningun_modulo_se_queda_fuera_del_menu() {
        let c = construir(1, &[]);
        let pintados: usize = c.grupos.iter().map(|g| g.modulos.len()).sum();
        assert_eq!(pintados, modulo::CATALOGO.len());
    }
}
