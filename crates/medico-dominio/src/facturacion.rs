//! La máquina de estados de una factura, sin base de datos.
//!
//! Vive aquí por lo mismo que [`crate::aislamiento`]: es una regla, se prueba en
//! microsegundos y la capa de datos no puede saltársela porque no tiene otra
//! forma de construir el estado siguiente.
//!
//! # Por qué es un tipo y no una cadena
//!
//! En el Python es un `dict` de conjuntos de cadenas
//! (`routers/facturacion.py:_TRANSICIONES`) contra un campo `str`. Eso deja tres
//! formas de equivocarse que aquí no compilan: escribir `"RADICADA "` con un
//! espacio, comparar contra un estado que no existe, y —la que de verdad pasó en
//! este proyecto con `orden.estado`— tener tres escrituras distintas del mismo
//! valor conviviendo hasta que una pantalla deja de encontrar sus filas.

use serde::{Deserialize, Serialize};

/// Los SEIS estados de una factura.
///
/// El sexto, [`EstadoFactura::Anulada`], faltaba en el CHECK de la migración
/// 0044 y lo añade la 0045. No es teórico: la pantalla pinta el botón «Anular»
/// para cualquier factura que no esté pagada ni anulada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EstadoFactura {
    Borrador,
    Radicada,
    Aceptada,
    Glosada,
    Pagada,
    Anulada,
}

impl EstadoFactura {
    /// Cómo se escribe en la base. Es el mismo vocabulario del Python y del
    /// navegador, a propósito: el port tiene que poder compararse con el sistema
    /// vivo fila a fila.
    pub fn como_texto(self) -> &'static str {
        match self {
            EstadoFactura::Borrador => "BORRADOR",
            EstadoFactura::Radicada => "RADICADA",
            EstadoFactura::Aceptada => "ACEPTADA",
            EstadoFactura::Glosada => "GLOSADA",
            EstadoFactura::Pagada => "PAGADA",
            EstadoFactura::Anulada => "ANULADA",
        }
    }

    /// Lee el estado de la base.
    ///
    /// Devuelve `None` ante cualquier otra cosa, y **el llamante tiene que
    /// tratarlo**: no hay caída a `Borrador`. Un valor que no reconocemos
    /// significa que la fila la escribió algo que no es este código, y decidir
    /// por él que «será un borrador» convertiría una factura pagada en editable
    /// (directiva 20).
    pub fn desde_texto(s: &str) -> Option<Self> {
        match s {
            "BORRADOR" => Some(EstadoFactura::Borrador),
            "RADICADA" => Some(EstadoFactura::Radicada),
            "ACEPTADA" => Some(EstadoFactura::Aceptada),
            "GLOSADA" => Some(EstadoFactura::Glosada),
            "PAGADA" => Some(EstadoFactura::Pagada),
            "ANULADA" => Some(EstadoFactura::Anulada),
            _ => None,
        }
    }

    /// A dónde puede ir desde aquí. Copiado de `_TRANSICIONES` del Python.
    ///
    /// `Pagada` y `Anulada` son terminales: una factura pagada no vuelve atrás, y
    /// una anulada tampoco resucita — se emite otra. Ese es el motivo de que
    /// anular no sea borrar: una factura radicada ante una EPS ya existe fuera
    /// de este sistema, y borrar la fila no la borra de allí.
    pub fn siguientes(self) -> &'static [EstadoFactura] {
        use EstadoFactura::*;
        match self {
            Borrador => &[Radicada],
            Radicada => &[Aceptada, Glosada, Anulada],
            Aceptada => &[Pagada, Anulada],
            Glosada => &[Radicada, Anulada],
            Pagada => &[],
            Anulada => &[],
        }
    }

    /// La única pregunta que hace la ruta de cambio de estado.
    pub fn puede_pasar_a(self, destino: EstadoFactura) -> bool {
        self.siguientes().contains(&destino)
    }

    /// **¿Cuenta esta factura como FACTURADO?** (#535, decisión de Juan del
    /// 2026-08-09.) «Facturado» es la factura VÁLIDAMENTE EMITIDA y no revertida:
    /// tiene número de la DIAN y salió. Un `Borrador` todavía no es factura —no
    /// tiene número ni ha salido— y una `Anulada` se revirtió; ninguna de las dos
    /// cuenta. Radicada, Aceptada, Glosada y Pagada sí: todas fueron facturadas,
    /// aunque estén en distinto punto del cobro. Contar el borrador en «total
    /// facturado» infla la cifra que el gerente reporta, que era el defecto de
    /// `estadisticas::financiero` (sumaba los seis estados).
    ///
    /// Es cosa distinta de RADICADO —lo entregado a la EPS, sin `Pagada`— y de
    /// PENDIENTE —`Borrador`+`Radicada`, lo que aún no se acepta—: son tres
    /// magnitudes y cada panel dice cuál enseña.
    pub fn es_facturado(self) -> bool {
        use EstadoFactura::*;
        matches!(self, Radicada | Aceptada | Glosada | Pagada)
    }

    /// ¿Admite que la entidad le ponga una glosa?
    ///
    /// **Un BORRADOR no**: la EPS glosa lo que RECIBIÓ, y un borrador no ha
    /// salido de aquí. Una pagada o una anulada tampoco: ya no hay discusión.
    ///
    /// Vive aquí desde el 2026-08-08 porque la regla estaba dentro de
    /// `agregar_glosa` y la pantalla del panel la necesitaba para no ofrecer un
    /// formulario que el servidor iba a rechazar. Con la regla en dos sitios,
    /// una de las dos copias se queda atrás; preguntándola, no.
    pub fn admite_glosa(self) -> bool {
        matches!(
            self,
            EstadoFactura::Radicada | EstadoFactura::Aceptada | EstadoFactura::Glosada
        )
    }

    /// Solo un BORRADOR se edita, se borra o cambia de líneas.
    ///
    /// Una vez radicada, la factura salió: modificarla cambiaría lo que la EPS ya
    /// tiene. Es la misma inmutabilidad que aplican la historia y el documento
    /// clínico firmados.
    pub fn admite_edicion(self) -> bool {
        self == EstadoFactura::Borrador
    }
}

#[cfg(test)]
mod pruebas {
    use super::EstadoFactura::*;
    use super::*;

    /// **FACTURADO = emitido y no anulado** (#535). Radicada, Aceptada, Glosada y
    /// Pagada cuentan; Borrador (no ha salido) y Anulada (se revirtió) no. La
    /// pareja son los DOS que no cuentan: sin ellos, un `matches!` que devolviera
    /// siempre `true` pasaría igual.
    #[test]
    fn facturado_es_lo_emitido_y_no_anulado() {
        assert!(Radicada.es_facturado());
        assert!(Aceptada.es_facturado());
        assert!(Glosada.es_facturado());
        assert!(Pagada.es_facturado());
        assert!(!Borrador.es_facturado(), "un borrador no es factura: no ha salido");
        assert!(!Anulada.es_facturado(), "una anulada se revirtió");
    }

    /// **Un BORRADOR no se glosa y un estado final tampoco.** La EPS glosa lo
    /// que recibió; un borrador no ha salido de aquí, y sobre una pagada o una
    /// anulada ya no hay discusión.
    #[test]
    fn solo_admite_glosa_lo_que_la_entidad_llego_a_recibir() {
        assert!(!Borrador.admite_glosa(), "se glosó un borrador");
        assert!(Radicada.admite_glosa());
        assert!(Aceptada.admite_glosa());
        assert!(Glosada.admite_glosa(), "una glosada admite otra glosa");
        assert!(!Pagada.admite_glosa());
        assert!(!Anulada.admite_glosa());
    }

    /// Ida y vuelta por los seis. Si alguien añade un estado y olvida una de las
    /// dos mitades, esto se cae.
    #[test]
    fn el_texto_va_y_vuelve() {
        for e in [Borrador, Radicada, Aceptada, Glosada, Pagada, Anulada] {
            assert_eq!(EstadoFactura::desde_texto(e.como_texto()), Some(e));
        }
    }

    /// La pareja del anterior: lo que no es un estado NO se convierte en uno.
    #[test]
    fn lo_que_no_es_un_estado_no_se_rellena() {
        for s in ["", "borrador", "RADICADA ", "ANULADO", "CUALQUIERA", "0"] {
            assert_eq!(EstadoFactura::desde_texto(s), None, "{s:?} coló");
        }
    }

    #[test]
    fn el_camino_normal_de_una_factura() {
        assert!(Borrador.puede_pasar_a(Radicada));
        assert!(Radicada.puede_pasar_a(Aceptada));
        assert!(Aceptada.puede_pasar_a(Pagada));
    }

    /// Una glosa se responde y la factura vuelve a radicarse.
    #[test]
    fn una_glosada_puede_volver_a_radicarse() {
        assert!(Glosada.puede_pasar_a(Radicada));
    }

    /// LAS TERMINALES SON TERMINALES. Es la regla que impide que una factura ya
    /// cobrada vuelva a borrador y se edite.
    #[test]
    fn de_pagada_y_de_anulada_no_se_sale() {
        for destino in [Borrador, Radicada, Aceptada, Glosada, Pagada, Anulada] {
            assert!(!Pagada.puede_pasar_a(destino), "Pagada -> {destino:?}");
            assert!(!Anulada.puede_pasar_a(destino), "Anulada -> {destino:?}");
        }
    }

    /// Anular está disponible desde los tres estados vivos posteriores al
    /// borrador, que es exactamente lo que ofrece el botón de la pantalla.
    #[test]
    fn se_puede_anular_desde_los_tres_estados_vivos() {
        assert!(Radicada.puede_pasar_a(Anulada));
        assert!(Aceptada.puede_pasar_a(Anulada));
        assert!(Glosada.puede_pasar_a(Anulada));
    }

    /// Un BORRADOR no se anula: se borra. Anular existe para lo que ya salió.
    #[test]
    fn un_borrador_no_se_anula() {
        assert!(!Borrador.puede_pasar_a(Anulada));
    }

    /// Barrido de la máquina entera: 36 pares, y solo los ocho declarados pasan.
    /// No es una prueba de ejemplos, es de la relación completa (directiva 8).
    #[test]
    fn solo_pasan_las_ocho_transiciones_declaradas() {
        let todos = [Borrador, Radicada, Aceptada, Glosada, Pagada, Anulada];
        let esperadas = [
            (Borrador, Radicada),
            (Radicada, Aceptada),
            (Radicada, Glosada),
            (Radicada, Anulada),
            (Aceptada, Pagada),
            (Aceptada, Anulada),
            (Glosada, Radicada),
            (Glosada, Anulada),
        ];
        let mut vistas = 0;
        for a in todos {
            for b in todos {
                let permitida = esperadas.contains(&(a, b));
                assert_eq!(a.puede_pasar_a(b), permitida, "{a:?} -> {b:?}");
                if permitida {
                    vistas += 1;
                }
            }
        }
        assert_eq!(vistas, 8, "el barrido no recorrió las ocho");
    }

    /// Nadie más que el borrador se edita.
    #[test]
    fn solo_el_borrador_admite_edicion() {
        assert!(Borrador.admite_edicion());
        for e in [Radicada, Aceptada, Glosada, Pagada, Anulada] {
            assert!(!e.admite_edicion(), "{e:?} se dejó editar");
        }
    }
}
