// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Qué día es hoy, y en qué huso. **Nunca el reloj de la máquina.**
//!
//! # El fallo que este módulo existe para no repetir
//!
//! Hasta el 2026-08-03 cuatro sitios preguntaban la fecha con `chrono::Local`,
//! que es la zona del SISTEMA OPERATIVO donde corre el binario:
//!
//! - la hora que el RIPS reporta para cada atención,
//! - los límites del periodo con que se filtra el cierre del mes,
//! - la edad del paciente,
//! - y el «hoy» con que se juzga si una licencia venció.
//!
//! En el portátil de un consultorio de Medellín eso da lo correcto por
//! casualidad, y por eso nadie lo vio. Medido el 2026-08-03: el VPS donde corre
//! la demostración tiene `Etc/UTC`, así que allí toda atención se reporta cinco
//! horas más tarde de la que ocurrió. **El archivo se acepta** —la hora es una
//! hora válida— y no hay nada en él que insinúe que está corrida. Es la misma
//! familia que `cod_consulta_cups='890201'`: un dato con la forma correcta que
//! nadie afirmó.
//!
//! Lo destapó una prueba que fallaba solo en el CI, que corre en UTC. O sea que
//! el aviso llegó por el sitio equivocado y por casualidad otra vez.
//!
//! # Por qué una zona de PROCESO y no una columna por institución
//!
//! Fue la primera idea, y se descartó al medir. De los cuatro sitios, tres —la
//! edad, la licencia y el rango por defecto de los indicadores— **no tienen una
//! institución a la que preguntarle**: la edad se calcula al listar pacientes,
//! sin más contexto que la fila. Meterle el parámetro a todo para que solo el
//! RIPS lo use sería complicar cuatro caminos para resolver uno.
//!
//! Y una columna `institucion.zona_horaria` hoy tendría el MISMO valor en todas
//! las filas, sin pantalla desde donde cambiarlo —`admin` es uno de los cinco
//! módulos que aún no están portados—: una característica que no se puede
//! operar es una característica falsa, y las falsas se borran.
//!
//! El día que un cliente esté en otro huso, la columna es una migración
//! pequeña y esta zona pasa a ser su defecto. Hoy la variable de entorno cubre
//! ese caso sin inventar una configuración que nadie usa.
//!
//! # Por qué el defecto NO es «rellenar un dato ausente»
//!
//! La directiva 20 dice que ante la ausencia de un dato se falla, y no se
//! sustituye por un defecto. Aquí no hay dato ausente: el producto emite RIPS
//! al Ministerio de Salud de Colombia, que es una obligación legal colombiana,
//! y **Colombia tiene una sola zona y no cambia la hora desde 1993**. El
//! defecto no es una suposición sobre ESTE prestador, es el dominio del
//! producto.
//!
//! Lo que sí se hace como manda la directiva: si el nombre configurado no
//! existe en la base IANA, **el binario no arranca**. No se cae a `Local`, ni a
//! UTC, ni al defecto — un huso equivocado corre todas las fechas en silencio,
//! que es exactamente lo que estamos arreglando.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use std::sync::OnceLock;

/// La zona del producto mientras nadie diga otra cosa.
///
/// Colombia, `-05` fijo. No hay horario de verano que la mueva.
pub const POR_DEFECTO: Tz = chrono_tz::America::Bogota;

static ZONA: OnceLock<Tz> = OnceLock::new();

/// Fija la zona del proceso. La llama el arranque, una sola vez.
///
/// Devuelve la zona que quedó activa. Si ya estaba fijada devuelve la anterior
/// **sin cambiarla**: la zona se decide al arrancar, no a mitad de una jornada
/// —cambiarla con historias ya escritas movería la interpretación de lo que ya
/// está guardado.
pub fn fijar(zona: Tz) -> Tz {
    *ZONA.get_or_init(|| zona)
}

/// Interpreta un nombre IANA (`America/Bogota`) y lo fija.
///
/// El error lleva el nombre recibido porque es lo único que el operador puede
/// corregir, y una lista de ejemplos porque «no es válida» a secas no dice si
/// el problema es el guion, el idioma o la mayúscula.
pub fn fijar_por_nombre(nombre: &str) -> Result<Tz, String> {
    let n = nombre.trim();
    let zona: Tz = n.parse().map_err(|_| {
        format!(
            "«{n}» no es una zona horaria IANA. Se escribe como «America/Bogota», \
             «America/Lima» o «Europe/Madrid» — región/ciudad, en inglés y sin tildes."
        )
    })?;
    Ok(fijar(zona))
}

/// La zona activa del proceso.
pub fn zona() -> Tz {
    *ZONA.get().unwrap_or(&POR_DEFECTO)
}

/// El instante actual, en la zona del prestador.
pub fn ahora() -> DateTime<Tz> {
    chrono::Utc::now().with_timezone(&zona())
}

/// Qué día es HOY para el prestador.
///
/// Es la función que sustituye a `Local::now().date_naive()`. La diferencia se
/// ve solo unas horas al día —las que van de la medianoche local a la del
/// servidor— y ahí decide si un paciente ya cumplió años y si una licencia ya
/// venció.
pub fn hoy() -> NaiveDate {
    ahora().date_naive()
}

/// Un instante guardado en UTC, leído en la zona del prestador.
pub fn desde_utc(utc: NaiveDateTime) -> NaiveDateTime {
    chrono::Utc
        .from_utc_datetime(&utc)
        .with_timezone(&zona())
        .naive_local()
}

/// Una hora local del prestador, escrita como el texto UTC que guarda la base.
///
/// Devuelve `None` cuando ese instante **no existe** en la zona —el salto del
/// horario de verano se come una hora—, en vez de correrlo al más cercano: un
/// filtro de periodo construido sobre una hora inventada mueve atenciones de
/// mes sin que nada falle. Colombia no cambia la hora, así que hoy no ocurre;
/// el día que el producto salga del país, ocurrirá.
pub fn a_utc(local: NaiveDateTime) -> Option<NaiveDateTime> {
    zona()
        .from_local_datetime(&local)
        .earliest()
        .map(|t| t.with_timezone(&chrono::Utc).naive_utc())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveDate;

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).unwrap()
    }

    /// El defecto es Colombia, y son cinco horas menos que UTC.
    ///
    /// Se comprueba sobre la CONVERSIÓN y no sobre el nombre: que la constante
    /// se llame `America::Bogota` no dice qué hace. Esto sí.
    #[test]
    fn el_defecto_es_colombia_y_son_cinco_horas() {
        let medianoche_utc = dia(2026, 3, 15).and_hms_opt(0, 0, 0).unwrap();
        let en_bogota = chrono::Utc
            .from_utc_datetime(&medianoche_utc)
            .with_timezone(&POR_DEFECTO)
            .naive_local();
        assert_eq!(
            en_bogota,
            dia(2026, 3, 14).and_hms_opt(19, 0, 0).unwrap(),
            "la medianoche UTC es todavía el día ANTERIOR en Colombia"
        );
    }

    /// Y no cambia con el verano del norte: agosto da lo mismo que marzo.
    ///
    /// Es lo que justifica que el defecto sea seguro. Si Colombia moviera la
    /// hora, un defecto fijo sería una suposición y no un dato del dominio.
    #[test]
    fn colombia_no_mueve_la_hora_en_todo_el_anio() {
        for (mes, dd) in [(1, 15), (4, 15), (7, 15), (10, 15)] {
            let utc = dia(2026, mes, dd).and_hms_opt(12, 0, 0).unwrap();
            let local = chrono::Utc
                .from_utc_datetime(&utc)
                .with_timezone(&POR_DEFECTO)
                .naive_local();
            assert_eq!(
                local,
                dia(2026, mes, dd).and_hms_opt(7, 0, 0).unwrap(),
                "mes {mes}: Colombia sigue en -05"
            );
        }
    }

    /// Un nombre que no existe se rechaza, y el mensaje dice cómo se escribe.
    #[test]
    fn un_nombre_que_no_existe_se_rechaza() {
        for malo in ["America/Medellin", "Bogota", "GMT-5", "america/bogota", ""] {
            let e = fijar_por_nombre(malo).unwrap_err();
            assert!(e.contains(malo.trim()), "el error nombra lo recibido: {e}");
            assert!(e.contains("America/Bogota"), "y enseña la forma buena: {e}");
        }
    }

    /// Los nombres buenos SÍ pasan — una comprobación que solo rechaza no
    /// se distingue de una que nunca acepta.
    #[test]
    fn los_nombres_buenos_pasan() {
        for bueno in ["America/Bogota", "America/Lima", "Europe/Madrid", "UTC"] {
            assert!(bueno.parse::<Tz>().is_ok(), "{bueno} es una zona IANA válida");
        }
        // Con espacios alrededor también: un `.env` copiado a mano los trae.
        assert!(" America/Bogota ".trim().parse::<Tz>().is_ok());
    }

    /// La ida y la vuelta se cancelan: local → UTC → local devuelve lo mismo.
    ///
    /// Es la invariante que sostiene el cierre del mes. Si no se cumpliera, una
    /// atención se filtraría en un marco y se emitiría en otro, que es el fallo
    /// que el módulo del RIPS ya documenta.
    #[test]
    fn la_ida_y_la_vuelta_se_cancelan() {
        for (h, m) in [(0, 0), (7, 30), (19, 0), (23, 59)] {
            let local = dia(2026, 3, 31).and_hms_opt(h, m, 0).unwrap();
            let utc = a_utc(local).expect("Colombia no tiene horas inexistentes");
            assert_eq!(desde_utc(utc), local, "{h}:{m} no sobrevivió el viaje");
        }
    }

    /// El caso del borde, que es el que paga: las 20:00 del 31 de marzo en
    /// Colombia se guardan como el 1 de abril en UTC.
    #[test]
    fn la_atencion_de_la_noche_cambia_de_dia_al_guardarse() {
        let local = dia(2026, 3, 31).and_hms_opt(20, 0, 0).unwrap();
        let utc = a_utc(local).unwrap();
        assert_eq!(utc, dia(2026, 4, 1).and_hms_opt(1, 0, 0).unwrap());
        assert_eq!(desde_utc(utc).date(), dia(2026, 3, 31), "y se cierra en MARZO");
    }
}
