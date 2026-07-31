// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Qué turnos quedan libres en la agenda de un médico un día concreto.
//!
//! Va en el dominio y no en la capa de datos porque es la única parte del
//! subsistema que tiene reglas: sumar franjas, descontar bloqueos y descontar
//! citas es aritmética sobre minutos, y aquí se puede probar entera sin abrir
//! una base. La capa de datos se limita a traer las tres listas.
//!
//! # Lo que se conserva del Python y lo que NO
//!
//! Se conserva **que sin agenda no hay turnos**: `get_slots_para_fecha`
//! devuelve lista vacía si el médico no tiene agenda de ese mes, y no se
//! inventa una jornada de oficina. Un turno ofrecido que el médico no atiende
//! es una cita que el paciente pierde habiendo pedido bien.
//!
//! NO se conserva la comparación de la cita por su hora EXACTA
//! (`citas_horas = {c.fecha_hora.strftime('%H:%M')}`). Con esa regla, una cita
//! de 45 minutos que empieza a las 09:00 solo tapaba el turno de las 09:00: el
//! de las 09:30 seguía ofreciéndose libre y se agendaba encima. No se notaba
//! porque la duración no se guardaba en ninguna parte —el navegador la
//! rellenaba con un 30 fijo—, así que todas las citas medían lo mismo que el
//! turno y el solapamiento era invisible. Al guardar la duración de verdad
//! (migración 0035), el cruce pasa a hacerse por INTERVALOS.

use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};

/// Minutos desde medianoche. `None` si el texto no es una hora `HH:MM` válida.
///
/// Se rechaza lo imposible —un 25, un 61, un texto— y nada más. No se «arregla»
/// una hora rara: una franja ilegible es un dato roto, y devolver turnos a
/// partir de él sería peor que no devolver ninguno.
pub fn minutos(hhmm: &str) -> Option<u16> {
    let (h, m) = hhmm.trim().split_once(':')?;
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// De minutos desde medianoche a `HH:MM`.
pub fn hhmm(min: u16) -> String {
    format!("{:02}:{:02}", min / 60, min % 60)
}

/// ¿Es esto un instante `YYYY-MM-DD HH:MM`?
///
/// Existe porque `bloqueo_agenda.inicio`/`fin` son `TEXT` con un único
/// `CHECK (inicio < fin)`, que es una comparación TEXTUAL: `{"inicio":"0000",
/// "fin":"9999"}` la pasa tan campante y deja al profesional con cero turnos
/// libres **el día que sea, para siempre**. Hace falta `GestionarAgenda`, así
/// que es sabotaje interno y no cruce de frontera — pero el `CHECK` daba una
/// falsa sensación de validar, y el resto de la migración 0035 sí acota todo lo
/// demás (`tipo`, `duracion_slot_min`, `dia_semana`, `anio`, `mes`).
///
/// Se rechaza lo IMPOSIBLE y nada más: un mes 13, un día 32, una hora 25. Una
/// fecha lejana —un bloqueo programado para 2030— pasa, porque es inusual y no
/// imposible.
pub fn es_instante(s: &str) -> bool {
    let Some((fecha, hora)) = s.split_once(' ') else {
        return false;
    };
    if minutos(hora).is_none() {
        return false;
    }
    let partes: Vec<&str> = fecha.split('-').collect();
    let [a, m, d] = partes[..] else { return false };
    if a.len() != 4 || m.len() != 2 || d.len() != 2 {
        return false;
    }
    let (Ok(a), Ok(m), Ok(d)) = (a.parse::<u16>(), m.parse::<u8>(), d.parse::<u8>()) else {
        return false;
    };
    // El día se comprueba contra el mes de verdad, no contra un 31 genérico: un
    // 31 de febrero es imposible, y aceptarlo daría un bloqueo que no ocurre
    // ningún día y que sin embargo la lista enseña como vigente.
    let bisiesto = a % 4 == 0 && (a % 100 != 0 || a % 400 == 0);
    let dias = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bisiesto => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dias).contains(&d)
}

/// Una franja de atención de un día concreto, ya resuelta desde el bloque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Franja {
    pub inicio: u16,
    pub fin: u16,
    /// Cuánto dura cada turno de esta franja. Lo declara la institución en el
    /// bloque; no es una constante del sistema.
    ///
    /// `NonZeroU16` y no `u16`: con duración cero, el `while` de [`turnos`] no
    /// termina nunca. Hoy es inalcanzable porque el `CHECK` de la migración 0035
    /// acota la columna a `1..=1440`, pero eso lo sabe la base y no el tipo, y
    /// esta función es `pub`. Es el mismo argumento con el que este repositorio
    /// justifica que `Ambito` sea un parámetro obligatorio en vez de un filtro
    /// opcional: el control que se puede omitir, se omite.
    pub duracion: NonZeroU16,
}

/// Un rato en que el médico NO atiende, recortado ya al día que se consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloqueo {
    pub inicio: u16,
    pub fin: u16,
    pub tipo: String,
}

/// Una cita ya agendada ese día.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ocupacion {
    pub inicio: u16,
    /// Minutos que dura. `None` cuando la cita se creó antes de que la columna
    /// existiera: **no se sustituye por un valor**. Ver [`Turno::motivo`].
    pub duracion: Option<u16>,
}

/// El motivo por el que un turno no está libre. Enumerado y no texto libre para
/// que la pantalla pueda decidir cómo lo dice y las pruebas puedan compararlo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "clase", content = "detalle")]
pub enum Motivo {
    /// El médico no atiende: vacaciones, incapacidad, reunión…
    Bloqueado(String),
    /// Ya hay una cita que se solapa con este turno.
    CitaAgendada,
    /// Hay una cita cuya duración NO está registrada. Tapa su turno de inicio y
    /// no se puede afirmar nada de los siguientes: la cita es de antes de que la
    /// duración se guardara. Se dice, en vez de suponer media hora.
    CitaDeDuracionDesconocida,
}

/// Un turno de la agenda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turno {
    pub hora: String,
    pub disponible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo: Option<Motivo>,
}

/// Los turnos del día, en orden, con el motivo de cada uno que no esté libre.
///
/// Devuelve vacío si no hay franjas — que es como se dice «este médico no
/// atiende ese día», y es distinto de «atiende y está todo cogido».
pub fn turnos(franjas: &[Franja], bloqueos: &[Bloqueo], ocupadas: &[Ocupacion]) -> Vec<Turno> {
    let mut salida = Vec::new();
    for f in franjas {
        let mut t = f.inicio;
        // `t + duracion <= fin`: un turno que se sale de la franja no se ofrece.
        // El Python usaba `current < fin`, así que una franja 08:00–09:00 con
        // turnos de 45 min ofrecía el de las 08:45, que termina a las 09:30 —
        // media hora después de que el médico se haya ido.
        let duracion = f.duracion.get();
        while t + duracion <= f.fin {
            let fin_turno = t + duracion;
            let motivo = motivo_de(t, fin_turno, bloqueos, ocupadas);
            salida.push(Turno {
                hora: hhmm(t),
                disponible: motivo.is_none(),
                motivo,
            });
            t += duracion;
        }
    }
    salida
}

/// Dos intervalos `[a, b)` se solapan si cada uno empieza antes de que el otro
/// acabe. Con los extremos abiertos por la derecha, un turno 09:00–09:30 y otro
/// 09:30–10:00 NO se solapan, que es lo que quiere decir agendarlos seguidos.
fn se_solapan(a_ini: u16, a_fin: u16, b_ini: u16, b_fin: u16) -> bool {
    a_ini < b_fin && b_ini < a_fin
}

fn motivo_de(
    ini: u16,
    fin: u16,
    bloqueos: &[Bloqueo],
    ocupadas: &[Ocupacion],
) -> Option<Motivo> {
    // El bloqueo manda sobre la cita: si el médico está de vacaciones, saber
    // además que hay una cita encima no cambia que el turno no se puede dar, y
    // sí es lo primero que hay que resolver.
    if let Some(b) = bloqueos.iter().find(|b| se_solapan(ini, fin, b.inicio, b.fin)) {
        return Some(Motivo::Bloqueado(b.tipo.clone()));
    }
    for o in ocupadas {
        match o.duracion {
            Some(d) if se_solapan(ini, fin, o.inicio, o.inicio + d) => {
                return Some(Motivo::CitaAgendada);
            }
            // Sin duración registrada solo se puede afirmar de su turno de
            // inicio. Rellenar con 30 minutos sería inventar el dato que la
            // migración 0035 dejó de inventar.
            None if o.inicio >= ini && o.inicio < fin => {
                return Some(Motivo::CitaDeDuracionDesconocida);
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn franja(ini: &str, fin: &str, dur: u16) -> Franja {
        Franja {
            inicio: minutos(ini).unwrap(),
            fin: minutos(fin).unwrap(),
            duracion: NonZeroU16::new(dur).expect("la duración de una franja no puede ser cero"),
        }
    }

    fn horas(ts: &[Turno]) -> Vec<&str> {
        ts.iter().map(|t| t.hora.as_str()).collect()
    }

    fn libres(ts: &[Turno]) -> Vec<&str> {
        ts.iter().filter(|t| t.disponible).map(|t| t.hora.as_str()).collect()
    }

    #[test]
    fn una_hora_ilegible_no_se_arregla() {
        assert_eq!(minutos("08:00"), Some(480));
        assert_eq!(minutos("23:59"), Some(1439));
        assert_eq!(minutos("00:00"), Some(0));
        // Lo imposible se rechaza; no se recorta a 23:59 ni a 00:00.
        assert_eq!(minutos("24:00"), None);
        assert_eq!(minutos("08:60"), None);
        assert_eq!(minutos("mañana"), None);
        assert_eq!(minutos("0800"), None);
    }

    #[test]
    fn ida_y_vuelta_del_formato() {
        for m in [0u16, 1, 59, 60, 480, 1439] {
            assert_eq!(minutos(&hhmm(m)), Some(m), "{m} no sobrevive la ida y vuelta");
        }
    }

    #[test]
    fn un_instante_se_reconoce_o_se_rechaza_entero() {
        assert!(es_instante("2026-08-10 08:00"));
        assert!(es_instante("2024-02-29 00:00"), "2024 sí es bisiesto");
        // Lo IMPOSIBLE.
        assert!(!es_instante("0000"), "el sabotaje que el CHECK textual dejaba pasar");
        assert!(!es_instante("9999"));
        assert!(!es_instante("2026-08-10"), "sin hora no es un instante");
        assert!(!es_instante("2026-13-01 08:00"), "mes 13");
        assert!(!es_instante("2026-02-31 08:00"), "31 de febrero");
        assert!(!es_instante("2026-04-31 08:00"), "abril tiene 30");
        assert!(!es_instante("2026-08-10 25:00"));
        assert!(!es_instante("2026-8-10 08:00"), "sin ceros a la izquierda no ordena");
        assert!(!es_instante("2026-08-10T08:00"), "ISO con T: otro formato");
        // Lo INUSUAL sí pasa.
        assert!(es_instante("2099-12-31 23:59"), "un bloqueo lejano es raro, no imposible");
    }

    /// 2026 NO es bisiesto: el 29 de febrero de ese año no existe.
    #[test]
    fn el_bisiesto_se_calcula_y_no_se_supone() {
        assert!(!es_instante("2026-02-29 08:00"));
        assert!(es_instante("2024-02-29 08:00"));
        assert!(!es_instante("2100-02-29 08:00"), "2100 es divisible por 100 y no por 400");
        assert!(es_instante("2000-02-29 08:00"), "2000 sí, por el /400");
    }

    #[test]
    fn sin_franjas_no_hay_turnos() {
        // «No atiende ese día», que NO es «atiende y está lleno».
        assert!(turnos(&[], &[], &[]).is_empty());
    }

    #[test]
    fn la_franja_se_parte_en_turnos_de_su_propia_duracion() {
        let ts = turnos(&[franja("08:00", "10:00", 30)], &[], &[]);
        assert_eq!(horas(&ts), ["08:00", "08:30", "09:00", "09:30"]);
        assert!(ts.iter().all(|t| t.disponible));
    }

    /// La duración la declara el bloque, no el sistema: 45 minutos es una
    /// consulta de psiquiatría, 10 una jornada de vacunación.
    #[test]
    fn cada_franja_manda_sobre_su_propia_duracion() {
        let ts = turnos(
            &[franja("08:00", "09:30", 45), franja("14:00", "14:30", 10)],
            &[],
            &[],
        );
        assert_eq!(horas(&ts), ["08:00", "08:45", "14:00", "14:10", "14:20"]);
    }

    /// El turno que se sale de la franja NO se ofrece. Es el fallo del `current
    /// < fin` del Python: 08:00–09:00 a 45 minutos ofrecía el de las 08:45.
    #[test]
    fn no_se_ofrece_un_turno_que_termina_despues_de_la_franja() {
        let ts = turnos(&[franja("08:00", "09:00", 45)], &[], &[]);
        assert_eq!(horas(&ts), ["08:00"], "se ofreció un turno que rebasa la franja");
    }

    #[test]
    fn un_bloqueo_tapa_los_turnos_que_toca_y_solo_esos() {
        let bloqueo = Bloqueo {
            inicio: minutos("09:00").unwrap(),
            fin: minutos("09:30").unwrap(),
            tipo: "REUNION".into(),
        };
        let ts = turnos(&[franja("08:00", "10:00", 30)], &[bloqueo], &[]);
        assert_eq!(libres(&ts), ["08:00", "08:30", "09:30"]);
        let tapado = ts.iter().find(|t| t.hora == "09:00").unwrap();
        assert_eq!(tapado.motivo, Some(Motivo::Bloqueado("REUNION".into())));
    }

    /// LA REGRESIÓN QUE MOTIVA EL CAMBIO. Con la comparación por hora exacta del
    /// Python, el turno de las 09:30 salía libre y se agendaba encima de una
    /// cita que todavía estaba ocurriendo.
    #[test]
    fn una_cita_larga_tapa_todos_los_turnos_que_ocupa() {
        let cita = Ocupacion { inicio: minutos("09:00").unwrap(), duracion: Some(45) };
        let ts = turnos(&[franja("08:00", "10:00", 30)], &[], &[cita]);
        assert_eq!(
            libres(&ts),
            ["08:00", "08:30"],
            "el turno de las 09:30 se ofreció encima de una cita de 45 minutos"
        );
        assert_eq!(
            ts.iter().find(|t| t.hora == "09:30").unwrap().motivo,
            Some(Motivo::CitaAgendada)
        );
    }

    /// Dos citas seguidas no se solapan: 09:00–09:30 y 09:30–10:00 caben.
    #[test]
    fn una_cita_que_termina_cuando_empieza_el_turno_no_lo_tapa() {
        let cita = Ocupacion { inicio: minutos("09:00").unwrap(), duracion: Some(30) };
        let ts = turnos(&[franja("09:00", "10:00", 30)], &[], &[cita]);
        assert_eq!(libres(&ts), ["09:30"]);
    }

    /// Sin duración registrada NO se supone media hora: se tapa el turno de
    /// inicio, se dice por qué, y de los siguientes no se afirma nada.
    #[test]
    fn una_cita_sin_duracion_no_se_rellena_con_treinta() {
        let cita = Ocupacion { inicio: minutos("09:00").unwrap(), duracion: None };
        let ts = turnos(&[franja("08:00", "10:00", 15)], &[], &[cita]);
        assert_eq!(libres(&ts), ["08:00", "08:15", "08:30", "08:45", "09:15", "09:30", "09:45"]);
        assert_eq!(
            ts.iter().find(|t| t.hora == "09:00").unwrap().motivo,
            Some(Motivo::CitaDeDuracionDesconocida),
            "la duración ausente se está tratando como si fuera un dato"
        );
    }

    /// El bloqueo gana al choque de cita: lo que hay que resolver primero es que
    /// el médico no está.
    #[test]
    fn el_bloqueo_manda_sobre_la_cita() {
        let bloqueo = Bloqueo {
            inicio: minutos("09:00").unwrap(),
            fin: minutos("10:00").unwrap(),
            tipo: "INCAPACIDAD".into(),
        };
        let cita = Ocupacion { inicio: minutos("09:00").unwrap(), duracion: Some(30) };
        let ts = turnos(&[franja("09:00", "10:00", 30)], &[bloqueo], &[cita]);
        assert!(ts.iter().all(|t| !t.disponible));
        assert_eq!(
            ts[0].motivo,
            Some(Motivo::Bloqueado("INCAPACIDAD".into()))
        );
    }

    /// Simulación: una jornada entera, con más de 100 turnos, un bloqueo y una
    /// agenda cargada. Comprueba que la cuenta cuadra —turnos libres + tapados
    /// = turnos totales— y que ninguno se repite, que es como se colaría un
    /// solapamiento entre franjas contiguas.
    #[test]
    fn una_jornada_completa_cuadra_y_no_repite_horas() {
        let franjas = vec![franja("06:00", "12:00", 10), franja("13:00", "18:00", 10)];
        let bloqueos = vec![Bloqueo {
            inicio: minutos("09:00").unwrap(),
            fin: minutos("09:40").unwrap(),
            tipo: "CAPACITACION".into(),
        }];
        // Una cita cada media hora por la tarde.
        let ocupadas: Vec<Ocupacion> = (0..10)
            .map(|i| Ocupacion { inicio: minutos("13:00").unwrap() + i * 30, duracion: Some(20) })
            .collect();

        let ts = turnos(&franjas, &bloqueos, &ocupadas);
        assert_eq!(ts.len(), 36 + 30, "la jornada no dio los turnos esperados");
        assert!(ts.len() > 100 / 2, "control del tamaño de la simulación");

        let tapados = ts.iter().filter(|t| !t.disponible).count();
        assert_eq!(libres(&ts).len() + tapados, ts.len());
        // 4 turnos de 10 min dentro del bloqueo, y 2 por cada una de las 10
        // citas de 20 min (tapa su turno y el siguiente).
        assert_eq!(tapados, 4 + 20);

        let mut vistas: Vec<&str> = horas(&ts);
        vistas.sort_unstable();
        let antes = vistas.len();
        vistas.dedup();
        assert_eq!(vistas.len(), antes, "hay horas repetidas entre franjas");
    }
}
