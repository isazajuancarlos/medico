// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza
//! **De dónde salió cada maestro, y de cuándo** — dicho en cada arranque.
//!
//! # El modo de fallo que esto ataca
//!
//! Un maestro caducado o no oficial **NO FALLA**. Deja pasar códigos que ya no
//! existen, rechaza los nuevos, y la primera señal llega como **glosa semanas
//! después**, cuando la EPS devuelve la factura. No hay excepción, no hay rojo,
//! no hay nada.
//!
//! Y hasta el 2026-08-14 tampoco se podía **saber** de cuándo eran: los cinco
//! declaraban metadatos distintos e incompletos —CIE-10 fuente sin fecha, CUM
//! fuente con fecha, DIVIPOLA fecha sin fuente— y el binario no decía nada.
//!
//! # Lo que hace, y lo que NO
//!
//! Lee lo que cada maestro DECLARA y lo dice al arrancar, siempre. Es el mismo
//! patrón que este repositorio ya usa dos veces y por el mismo motivo:
//!
//! - `MEDICO_ZONA` se anuncia esté puesta o no;
//! - `alertas_pasada` deja su línea también cuando va bien, para que «no había a
//!   quién avisar» no se vea igual que «el reloj lleva tres semanas muerto».
//!
//! **NO BLOQUEA, y eso es invariante del producto** (modo degradado, #71):
//! dejar un consultorio sin sistema por un catálogo viejo es peor que el
//! catálogo viejo. Avisa y sigue.
//!
//! **NO INVENTA lo que el maestro no declara.** Un maestro sin fecha sale como
//! «sin fecha declarada», que es en sí mismo el aviso — no se rellena con la
//! fecha del archivo ni con la de hoy (directiva 20).
//!
//! # Lo que esto DESTAPÓ al escribirse, y es lo más importante
//!
//! El maestro de **CIE-10 declara en su propio JSON que NO es oficial**:
//!
//! > «La hoja declara en su segunda fila "Subido por Niccpaganini ·
//! > medicblog.wordpress.com". NO es una publicación oficial de MinSalud ni de
//! > la OMS.»
//!
//! Doce mil cuatrocientos veintitrés diagnósticos que se facturan al sistema de
//! salud, salidos de un blog. La advertencia llevaba ahí desde el 2026-07-31,
//! dentro de un archivo de 2,3 MB que nadie abre. Ahora sale en cada arranque.

/// De dónde salió un maestro, tal como él mismo lo declara.
///
/// Los campos son `Option` a propósito: **lo que el maestro no declara se dice
/// que no lo declara**, no se rellena.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Procedencia {
    /// Cómo se llama, para el renglón del arranque.
    pub maestro: &'static str,
    /// El documento del que salió, si lo dice.
    pub fuente: Option<String>,
    /// Cuándo, si lo dice. Formato libre: es lo que declara el archivo.
    pub fecha: Option<String>,
    /// Lo que el propio maestro avisa de sí mismo. Cuando existe, MANDA: es
    /// quien lo generó diciendo qué no se puede dar por bueno.
    pub advertencia: Option<String>,
}

impl Procedencia {
    /// ¿Hay algo que decir en voz alta sobre este maestro?
    ///
    /// Dos motivos, y los dos son del maestro, no de un umbral que inventemos:
    /// que **no declare fecha** —y entonces no se puede saber si caducó— o que
    /// **traiga advertencia propia**.
    ///
    /// NO se mira la antigüedad contra un umbral en meses. Se pensó y se
    /// descartó: un umbral es una cifra inventada, y una alarma que salta por
    /// una cifra inventada es la que a la tercera nadie mira (directiva 23). Lo
    /// que sí es un hecho verificable es que el maestro no diga de cuándo es.
    pub fn hay_que_decirlo(&self) -> bool {
        self.fecha.is_none() || self.advertencia.is_some()
    }

    /// El renglón del arranque. Sale SIEMPRE, avise o no: si solo saliera cuando
    /// hay problema, un arranque silencioso significaría a la vez «todo bien» y
    /// «esto no llegó a correr».
    pub fn renglon(&self, filas: usize) -> String {
        let fuente = self.fuente.as_deref().unwrap_or("sin fuente declarada");
        let fecha = self.fecha.as_deref().unwrap_or("SIN FECHA DECLARADA");
        format!("  {:<12} {:>7} filas · {} · {}", self.maestro, filas, fecha, fuente)
    }
}

/// Lee la procedencia de un maestro empotrado, sin fallar si no la declara.
///
/// Los cinco maestros usan claves distintas para lo mismo —`generado`,
/// `fecha_fuente`— porque los escribieron guiones distintos en momentos
/// distintos. Se aceptan las dos en vez de normalizar los archivos: cambiar un
/// maestro de 2,6 MB para renombrar una clave es tocar el dato por la forma.
pub fn leer(maestro: &'static str, json: &str) -> Procedencia {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Procedencia {
        maestro,
        fuente: s("fuente"),
        fecha: s("generado").or_else(|| s("fecha_fuente")),
        advertencia: s("advertencia"),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_las_dos_formas_de_declarar_la_fecha() {
        // `divipola` usa `generado`; `cum` usa `fecha_fuente`. Las dos valen.
        assert_eq!(leer("d", r#"{"generado":"2026-07-19"}"#).fecha.as_deref(), Some("2026-07-19"));
        assert_eq!(leer("c", r#"{"fecha_fuente":"2026-07-20"}"#).fecha.as_deref(), Some("2026-07-20"));
    }

    /// **LA PAREJA, y es la que importa**: un maestro sin fecha tiene que
    /// avisar, y uno con fecha y sin advertencia tiene que callar. Sin la
    /// segunda mitad pasaría igual un aviso cableado a `true`.
    #[test]
    fn avisa_sin_fecha_y_calla_con_ella() {
        assert!(leer("x", r#"{"fuente":"a.xlsx"}"#).hay_que_decirlo(), "sin fecha hay que decirlo");
        assert!(
            !leer("x", r#"{"fuente":"a.xlsx","generado":"2026-08-14"}"#).hay_que_decirlo(),
            "con fecha y sin advertencia NO hay nada que decir"
        );
    }

    /// La advertencia del propio maestro manda aunque tenga fecha. Es el caso
    /// del CIE-10: fecha no tiene, pero aunque la tuviera seguiría sin ser una
    /// publicación oficial.
    #[test]
    fn una_advertencia_propia_avisa_aunque_haya_fecha() {
        let p = leer("x", r#"{"generado":"2026-08-14","advertencia":"no es oficial"}"#);
        assert!(p.hay_que_decirlo());
        assert_eq!(p.advertencia.as_deref(), Some("no es oficial"));
    }

    /// Un JSON que no sea un objeto —o que no parsee— no puede tumbar el
    /// arranque: el maestro se siembra igual, solo que sin procedencia que
    /// contar. Es el modo degradado del producto (#71).
    #[test]
    fn un_json_raro_no_revienta() {
        for j in ["[1,2,3]", "no soy json", "", "null"] {
            let p = leer("x", j);
            assert!(p.fuente.is_none() && p.fecha.is_none());
            assert!(p.hay_que_decirlo(), "sin nada que declarar, hay que decirlo");
        }
    }

    #[test]
    fn el_renglon_dice_lo_que_falta_en_vez_de_callarlo() {
        let r = leer("CIE-10", r#"{"fuente":"CIE10.xlsx"}"#).renglon(12423);
        assert!(r.contains("SIN FECHA DECLARADA"), "{r}");
        assert!(r.contains("12423"), "{r}");
        assert!(r.contains("CIE10.xlsx"), "{r}");
    }

    /// **CONTRA LOS MAESTROS DE VERDAD**, no contra JSON sintético — que es lo
    /// que hace que esta prueba mida el producto y no el arnés.
    ///
    /// Fija el estado de HOY, que es malo y conviene que se vea: de los cinco,
    /// solo DOS declaran fecha. El día que se arreglen, esta prueba se pone roja
    /// y hay que subir el número. Que se ponga roja es el objetivo.
    #[test]
    fn hoy_solo_dos_de_los_cinco_maestros_declaran_de_cuando_son() {
        let ps = crate::clinico::procedencias();
        assert_eq!(ps.len(), 5, "son cinco maestros");
        let con_fecha = ps.iter().filter(|p| p.fecha.is_some()).count();
        assert_eq!(
            con_fecha, 2,
            "hoy solo CUM y DIVIPOLA dicen de cuándo son; si esto cambia, actualiza el número: {:?}",
            ps.iter().map(|p| (p.maestro, p.fecha.clone())).collect::<Vec<_>>()
        );
    }

    /// Y el que más importa, con nombre propio: **el CIE-10 declara que NO es
    /// oficial**. 12 423 diagnósticos que se facturan al sistema de salud,
    /// salidos de un blog de WordPress.
    ///
    /// Si algún día se sustituye por la tabla oficial, esta prueba se pone roja
    /// — y ése es justo el día en que hay que quitarla.
    #[test]
    fn el_cie10_sigue_sin_ser_una_fuente_oficial() {
        let ps = crate::clinico::procedencias();
        let cie = ps.iter().find(|p| p.maestro == "CIE-10").expect("el CIE-10 está");
        let a = cie.advertencia.as_deref().unwrap_or("");
        assert!(
            a.contains("NO es una publicación oficial"),
            "el maestro tiene que seguir declarando su origen: {a}"
        );
    }
}
