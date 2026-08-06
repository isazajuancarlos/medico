// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! La máquina de estados de una cirugía, y qué le pasa a la sala.
//!
//! # Por qué vive en el dominio y no en el manejador HTTP
//!
//! Porque son reglas del negocio, no de la web: una cirugía COMPLETADA no puede
//! volver a empezar, y una CANCELADA no se completa. En el lado Python esas
//! reglas están escritas tres veces, una en cada endpoint, con su `if` y su
//! mensaje —y la tercera no dice lo mismo que las otras dos: `cancelar` prohíbe
//! `COMPLETADA` y `CANCELADA`, pero deja pasar `SUSPENDIDA`, mientras que
//! `iniciar` exige exactamente `PROGRAMADA`. Una cirugía suspendida no se puede
//! reanudar ni cancelar: queda en un estado del que no sale.
//!
//! Aquí la tabla de transiciones está en UN sitio, y se puede probar sin base de
//! datos ni servidor.
//!
//! # Lo que la sala hace, que es la otra mitad
//!
//! Ocupar y liberar el quirófano no es un efecto secundario que se acuerde el
//! programador: es parte de la transición. Empezar ocupa la sala; terminarla o
//! cancelarla ya empezada la libera. Cancelar una que **no** empezó no toca la
//! sala — nunca la ocupó.

use serde::{Deserialize, Serialize};

/// En qué punto está una cirugía.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EstadoCirugia {
    Programada,
    EnCurso,
    Completada,
    Cancelada,
    Suspendida,
}

impl EstadoCirugia {
    /// El texto con el que se guarda y viaja. Es el mismo del `CHECK` de la
    /// tabla, y `los_nombres_son_los_del_esquema` lo ata.
    pub fn como_texto(self) -> &'static str {
        match self {
            EstadoCirugia::Programada => "PROGRAMADA",
            EstadoCirugia::EnCurso => "EN_CURSO",
            EstadoCirugia::Completada => "COMPLETADA",
            EstadoCirugia::Cancelada => "CANCELADA",
            EstadoCirugia::Suspendida => "SUSPENDIDA",
        }
    }

    /// Lee el texto de la base. **Un valor desconocido devuelve `None`**, no un
    /// estado por defecto: una fila con el estado corrupto no es una cirugía
    /// programada, y tratarla como tal la dejaría empezar.
    pub fn desde_texto(t: &str) -> Option<Self> {
        Some(match t {
            "PROGRAMADA" => EstadoCirugia::Programada,
            "EN_CURSO" => EstadoCirugia::EnCurso,
            "COMPLETADA" => EstadoCirugia::Completada,
            "CANCELADA" => EstadoCirugia::Cancelada,
            "SUSPENDIDA" => EstadoCirugia::Suspendida,
            _ => return None,
        })
    }

    /// Un estado del que ya no se sale. La cirugía ocurrió o no va a ocurrir.
    pub fn es_final(self) -> bool {
        matches!(self, EstadoCirugia::Completada | EstadoCirugia::Cancelada)
    }
}

/// Lo que se le pide hacer a una cirugía.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Iniciar,
    Completar,
    Cancelar,
    Suspender,
}

impl Accion {
    pub fn verbo(self) -> &'static str {
        match self {
            Accion::Iniciar => "iniciar",
            Accion::Completar => "completar",
            Accion::Cancelar => "cancelar",
            Accion::Suspender => "suspender",
        }
    }
}

/// Qué le pasa a la SALA cuando la cirugía cambia de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfectoEnSala {
    /// Nada: la sala no cambia.
    Ninguno,
    /// Pasa a `EN_USO`.
    Ocupar,
    /// Vuelve a `DISPONIBLE`, **solo si estaba `EN_USO`**. Nunca se saca una
    /// sala de `MANTENIMIENTO` ni de `BLOQUEADO` por terminar una cirugía: esos
    /// dos los pone una persona por un motivo, y el flujo no los pisa.
    Liberar,
}

/// La transición aceptada: a qué estado va y qué le pasa a la sala.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transicion {
    pub destino: EstadoCirugia,
    pub sala: EfectoEnSala,
}

/// Por qué NO se puede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rechazo {
    pub desde: EstadoCirugia,
    pub accion: Accion,
}

impl Rechazo {
    /// El mensaje que ve quien lo intentó. Dice el estado ACTUAL porque es el
    /// dato con el que puede decidir qué hacer: «no se puede» a secas manda a
    /// recargar la pantalla a ciegas.
    pub fn mensaje(&self) -> String {
        format!(
            "no se puede {} una cirugía en estado {}",
            self.accion.verbo(),
            self.desde.como_texto()
        )
    }
}

/// **La tabla entera de transiciones, en un sitio.**
///
/// Se lee como la matriz que es: desde cada estado, qué acciones valen. Lo que
/// no está aquí, no se puede — y el `match` exhaustivo obliga a decidirlo el día
/// que entre un estado nuevo, en vez de dejarlo caer en un `else` permisivo.
pub fn transicion(desde: EstadoCirugia, accion: Accion) -> Result<Transicion, Rechazo> {
    use Accion::*;
    use EfectoEnSala::*;
    use EstadoCirugia::*;

    let t = |destino, sala| Ok(Transicion { destino, sala });
    match (desde, accion) {
        // Lo normal.
        (Programada, Iniciar) => t(EnCurso, Ocupar),
        (EnCurso, Completar) => t(Completada, Liberar),

        // Cancelar antes de empezar no toca la sala: nunca la ocupó.
        (Programada, Cancelar) => t(Cancelada, Ninguno),
        // Cancelar una ya empezada sí la libera.
        (EnCurso, Cancelar) => t(Cancelada, Liberar),

        // Suspender es «hoy no, pero sigue pendiente». Desde EN_CURSO libera la
        // sala; desde PROGRAMADA no hay nada que liberar.
        (Programada, Suspender) => t(Suspendida, Ninguno),
        (EnCurso, Suspender) => t(Suspendida, Liberar),

        // **UNA SUSPENDIDA VUELVE A LA VIDA.** En el lado Python no: `iniciar`
        // exige exactamente `PROGRAMADA` y `cancelar` no nombra `SUSPENDIDA`, así
        // que una cirugía suspendida se quedaba sin salida —ni se reanuda ni se
        // cierra— y ese estado no aparecía en ninguna lista de trabajo.
        //
        // AVISO: hoy `SUSPENDIDA` **no se puede alcanzar por HTTP**. No hay ruta
        // `suspender`, ni en este binario ni en el Python, así que estas dos
        // transiciones y `Accion::Suspender` son correctas y no las ejecuta
        // ninguna petición. Se conservan porque el modelo es el bueno y el
        // frontal ya ofrece «Suspendidas» como filtro; lo que falta es el verbo,
        // y va en su propia tarea. Lo contó la revisión de esta rama, que además
        // avisó de que al reanudar se pisaría `hora_inicio_real` sin dejar
        // rastro: eso hay que resolverlo ANTES de añadir la ruta, separando la
        // hora nominal del primer inicio de la del reinicio (directiva 32).
        (Suspendida, Iniciar) => t(EnCurso, Ocupar),
        (Suspendida, Cancelar) => t(Cancelada, Ninguno),

        // Lo demás, no. Los dos finales no se mueven, y no se puede completar lo
        // que no ha empezado.
        _ => Err(Rechazo { desde, accion }),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use Accion::*;
    use EstadoCirugia::*;

    #[test]
    fn el_camino_feliz() {
        let a = transicion(Programada, Iniciar).unwrap();
        assert_eq!(a.destino, EnCurso);
        assert_eq!(a.sala, EfectoEnSala::Ocupar);

        let b = transicion(EnCurso, Completar).unwrap();
        assert_eq!(b.destino, Completada);
        assert_eq!(b.sala, EfectoEnSala::Liberar, "al terminar se libera la sala");
    }

    /// **Los estados finales no se mueven.** Es la mitad que decide: una tabla
    /// que solo probara el camino feliz pasaría igual con `_ => Ok(…)`.
    #[test]
    fn de_un_estado_final_no_sale_nada() {
        for final_ in [Completada, Cancelada] {
            for accion in [Iniciar, Completar, Cancelar, Suspender] {
                let r = transicion(final_, accion);
                assert!(
                    r.is_err(),
                    "se permitió {} una cirugía {}",
                    accion.verbo(),
                    final_.como_texto()
                );
            }
            assert!(final_.es_final());
        }
    }

    #[test]
    fn no_se_completa_lo_que_no_ha_empezado() {
        for desde in [Programada, Suspendida] {
            assert!(
                transicion(desde, Completar).is_err(),
                "se completó una cirugía en {}",
                desde.como_texto()
            );
        }
    }

    /// La suspendida tiene salida, y el Python no se la daba.
    #[test]
    fn una_suspendida_se_reanuda_o_se_cancela() {
        assert_eq!(transicion(Suspendida, Iniciar).unwrap().destino, EnCurso);
        assert_eq!(transicion(Suspendida, Cancelar).unwrap().destino, Cancelada);
        assert!(
            transicion(Suspendida, Suspender).is_err(),
            "suspender dos veces no significa nada"
        );
    }

    /// Cancelar sin haber empezado NO libera la sala. Si lo hiciera, cancelar
    /// una cirugía de mañana sacaría de `EN_USO` la sala donde hay otra
    /// operando ahora mismo.
    #[test]
    fn cancelar_sin_empezar_no_toca_la_sala() {
        assert_eq!(
            transicion(Programada, Cancelar).unwrap().sala,
            EfectoEnSala::Ninguno
        );
        assert_eq!(
            transicion(EnCurso, Cancelar).unwrap().sala,
            EfectoEnSala::Liberar,
            "la que sí empezó tiene que liberarla"
        );
    }

    /// Iniciar dos veces no vale: la segunda pisaría la hora de inicio real.
    #[test]
    fn no_se_inicia_dos_veces() {
        assert!(transicion(EnCurso, Iniciar).is_err());
    }

    /// Los textos son EXACTAMENTE los del `CHECK` de la migración 0047. Si
    /// divergen, la escritura falla en producción y aquí todo estaría verde.
    #[test]
    fn los_nombres_son_los_del_esquema() {
        let sql = include_str!("../../medico-datos/migrations/0047_quirofano.sql");
        for e in [Programada, EnCurso, Completada, Cancelada, Suspendida] {
            let t = e.como_texto();
            assert!(
                sql.contains(&format!("'{t}'")),
                "«{t}» no aparece en el CHECK de la migración"
            );
            assert_eq!(EstadoCirugia::desde_texto(t), Some(e), "ida y vuelta de {t}");
        }
    }

    /// Un estado que no existe se lee como `None`, no como «programada».
    #[test]
    fn un_estado_desconocido_no_se_convierte_en_uno_valido() {
        for basura in ["", "programada", "EN CURSO", "ANULADA", "PROGRAMADA "] {
            assert_eq!(
                EstadoCirugia::desde_texto(basura),
                None,
                "«{basura}» se leyó como un estado válido"
            );
        }
    }

    /// El mensaje del rechazo nombra el estado actual y el verbo intentado: es
    /// lo que el usuario necesita para saber qué hacer.
    #[test]
    fn el_rechazo_dice_el_estado_y_el_verbo() {
        let e = transicion(Completada, Iniciar).unwrap_err().mensaje();
        assert!(e.contains("COMPLETADA"), "{e}");
        assert!(e.contains("iniciar"), "{e}");
    }

    /// El recuento: de 20 pares (5 estados × 4 acciones) exactamente 8 valen.
    /// Es el control de que la tabla ni se abrió ni se cerró sin querer — un
    /// cambio que mueva ese número tiene que ser deliberado.
    ///
    /// **Dos de los 8 son inalcanzables hoy** —las que salen de `SUSPENDIDA`—
    /// porque no hay ruta `suspender`. Se dice aquí para que el número no se lea
    /// como «ocho caminos que el producto ofrece».
    #[test]
    fn de_veinte_pares_valen_ocho() {
        let todos = [Programada, EnCurso, Completada, Cancelada, Suspendida];
        let acciones = [Iniciar, Completar, Cancelar, Suspender];
        let validos = todos
            .iter()
            .flat_map(|e| acciones.iter().map(move |a| transicion(*e, *a)))
            .filter(Result::is_ok)
            .count();
        assert_eq!(validos, 8, "la tabla de transiciones cambió de tamaño");
    }
}
