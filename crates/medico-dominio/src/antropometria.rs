// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Antropometría: lo que se DERIVA de un peso y una talla.
//!
//! # Por qué el IMC vive aquí y no en la pantalla
//!
//! Hasta el 2026-08-08 lo calculaba el NAVEGADOR —`calcularIMC()` en
//! `nutricion.component.ts`— y lo mandaba en el cuerpo del POST, y
//! `crear_valoracion` lo guardaba tal cual. O sea que
//! `{"peso": 70, "talla": 1.70, "imc": 18}` entraba en la historia clínica con
//! un índice que **contradice su propio peso y su propia talla**, y nada lo
//! decía. No era explotable ni una fuga: era un valor DERIVADO guardado como si
//! fuera medido, que es la directiva 32 en su forma más cara — el nominal y el
//! derivado son dos cosas, y aquí el derivado lo firmaba el cliente.
//!
//! El peso y la talla se MIDEN; el índice se calcula. Con la llegada del segundo
//! frontal la fórmula habría tenido tres autores —Angular, el panel y cualquier
//! cliente de la API—, así que sube al dominio y la aplica el servidor.
//!
//! # La fórmula es la del sistema vivo, no una nueva
//!
//! `backend/antropometria.py:78` hace `round(peso / talla ** 2, 1)` y el
//! componente de Angular, `Math.round((p / (t * t)) * 10) / 10`. Las dos son la
//! misma y esta es la tercera: un decimal, redondeando. Si aquí se hubiera
//! elegido otro redondeo, el informe nuevo daría números distintos de los que el
//! prestador viene reportando, que es la condición de fidelidad de todo el port.

/// Índice de masa corporal, con un decimal.
///
/// `None` cuando no se puede calcular, y **no cero**: un IMC de 0 tiene forma de
/// dato y se lee como una medición (directiva 20). Sin peso, sin talla o con una
/// talla imposible, la respuesta es que no se sabe.
///
/// La talla va en METROS, que es como la guarda este esquema y como la escribe
/// la pantalla. Una talla en centímetros da un índice ridículamente pequeño y no
/// se corrige adivinando: `1.70` y `170` son los dos números positivos, y elegir
/// por magnitud sería inventar la unidad de una medida clínica.
pub fn imc(peso_kg: f64, talla_m: f64) -> Option<f64> {
    if !peso_kg.is_finite() || !talla_m.is_finite() || peso_kg <= 0.0 || talla_m <= 0.0 {
        return None;
    }
    Some((peso_kg / (talla_m * talla_m) * 10.0).round() / 10.0)
}

/// Los factores del original: IMC objetivo por sexo. `backend/antropometria.py`
/// los migró literalmente del VFP y aquí se conservan iguales, que es la
/// condición de fidelidad del port entero.
const FACTOR_HOMBRE: f64 = 0.23;
const FACTOR_MUJER: f64 = 0.22;

/// Por debajo de esta edad el original NO calcula peso ideal.
const EDAD_MINIMA_PESO_IDEAL: i64 = 12;

/// Peso ideal en kilogramos, o `None` cuando el original no lo calcularía.
///
/// Es `talla² × factor × 100`, con el factor por sexo del sistema vivo. La talla
/// va en METROS, igual que en [`imc`].
///
/// # Las dos desviaciones deliberadas, y son las del Python
///
/// **Devuelve `None`, no cero.** El original devolvía 0, y un 0 en un campo de
/// peso se lee como un dato real y equivocado (directiva 20). Quien llame decide
/// qué enseñar.
///
/// **Un sexo que no empieza por «M» se trata como femenino**, que es lo que hace
/// el original con su `IF Lsexo='M' … ELSE`. No se corrige: cambiarlo daría un
/// peso ideal distinto del que el prestador viene viendo, y eso es criterio
/// clínico nuevo, no migración.
///
/// # Lo que NO cubre, dicho aquí porque se nota en el consultorio
///
/// Nada por debajo de los 12 años, y ahí es donde más se usa una referencia
/// antropométrica. Es del original y se conserva; es candidato de la etapa de
/// mejoras, no de ésta.
pub fn peso_ideal(talla_m: f64, sexo: &str, edad_anios: i64) -> Option<f64> {
    if edad_anios <= EDAD_MINIMA_PESO_IDEAL {
        return None;
    }
    if !talla_m.is_finite() || talla_m <= 0.0 {
        return None;
    }
    let factor = if sexo.trim().to_uppercase().starts_with('M') {
        FACTOR_HOMBRE
    } else {
        FACTOR_MUJER
    };
    Some((talla_m * talla_m * factor * 100.0 * 10.0).round() / 10.0)
}

/// La tabla `PESOTALLA1` del sistema original, transcrita por
/// `herramientas/generar_talla_ideal.py`. 62 filas, y no cambia.
const TALLA_IDEAL_JSON: &str = include_str!("../datos/talla_ideal.json");

/// Una fila de la referencia antropométrica del original.
#[derive(serde::Deserialize)]
struct FilaTalla {
    edad: i64,
    /// `"meses"` o `"anios"`, que es la columna TIPO del DBF.
    unidad: String,
    sexo: String,
    talla_ideal_m: f64,
    #[allow(dead_code)]
    peso_ideal_kg: f64,
}

#[derive(serde::Deserialize)]
struct TablaTalla {
    filas: Vec<FilaTalla>,
}

fn tabla() -> &'static Vec<FilaTalla> {
    use std::sync::OnceLock;
    static TABLA: OnceLock<Vec<FilaTalla>> = OnceLock::new();
    TABLA.get_or_init(|| {
        serde_json::from_str::<TablaTalla>(TALLA_IDEAL_JSON)
            .expect("talla_ideal.json empotrado y no se deja leer")
            .filas
    })
}

/// Hasta esta edad la tabla se consulta por MESES; a partir de ella, por años.
const MESES_DE_UN_ANIO: i64 = 12;
/// La última edad que la tabla cubre. Por encima no hay fila, y no se extrapola.
const EDAD_MAXIMA_TABLA_ANIOS: i64 = 19;

/// Talla ideal en metros, buscada en la tabla del sistema original.
///
/// La edad entra en MESES y siempre en meses, que es lo que permite que la
/// función tenga una sola puerta: la tabla trae dos tramos —de 0 a 11 meses y de
/// 1 a 19 años— y quien llama no tiene por qué saber en cuál cae su paciente.
/// Pedir «edad y unidad» habría dejado al llamante eligiendo el tramo, que es
/// justo la decisión que aquí se resuelve.
///
/// # No es una fórmula: es una BÚSQUEDA, y por eso no se interpola
///
/// [`peso_ideal`] calcula; ésta consulta. Un paciente de 42 meses cae en la fila
/// de 3 años, no entre la de 3 y la de 4: la tabla del original es escalonada y
/// promediar entre dos filas produciría un número que ese sistema nunca dio.
///
/// # La advertencia que hay que leer antes de cuadrar cifras
///
/// **En el sistema vivo esto NUNCA funcionó.** El formulario hace `SEEK` sobre
/// `PESOTALLA`, que no tiene columna EDAD ni ese índice, así que la búsqueda no
/// puede acertar y la talla ideal sale siempre en 0. Ésta es, por tanto, la
/// única pieza del port sin número del prestador contra el que contrastar: no es
/// migración fiel de un comportamiento, es la tabla que el original traía y no
/// llegó a usar.
///
/// # Las dos reglas que sí vienen de [`peso_ideal`]
///
/// Devuelve `None` y **no cero** cuando no hay fila —el 0 tiene forma de dato y
/// se lee como una medición—, y un sexo que no empieza por «M» cae al lado
/// femenino, que es el `IF Lsexo='M' … ELSE` del original.
pub fn talla_ideal(edad_meses: i64, sexo: &str) -> Option<f64> {
    if edad_meses < 0 {
        return None;
    }
    let (unidad, edad) = if edad_meses < MESES_DE_UN_ANIO {
        ("meses", edad_meses)
    } else {
        let anios = edad_meses / MESES_DE_UN_ANIO;
        if anios > EDAD_MAXIMA_TABLA_ANIOS {
            return None;
        }
        ("anios", anios)
    };
    let sexo = if sexo.trim().to_uppercase().starts_with('M') {
        "M"
    } else {
        "F"
    };
    tabla()
        .iter()
        .find(|f| f.unidad == unidad && f.edad == edad && f.sexo == sexo)
        .map(|f| f.talla_ideal_m)
}

/// La categoría del IMC según la OMS, que es la que pinta la pantalla.
///
/// Los cortes son los de la OMS y los mismos que usaba el frontal: < 18,5 bajo
/// peso; < 25 normal; < 30 sobrepeso; el resto obesidad. **No se afina más**
/// —obesidad I, II, III— porque el producto no lo hacía y añadir grados aquí
/// haría que la misma valoración se leyera distinto en las dos pantallas
/// mientras convivan.
pub fn categoria_imc(imc: f64) -> &'static str {
    if imc < 18.5 {
        "bajo peso"
    } else if imc < 25.0 {
        "normal"
    } else if imc < 30.0 {
        "sobrepeso"
    } else {
        "obesidad"
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **La fórmula es la del sistema vivo**, con su redondeo. Un redondeo
    /// distinto daría números que no cuadran con los que el prestador viene
    /// reportando.
    #[test]
    fn el_imc_es_el_mismo_que_calcula_el_sistema_vivo() {
        // 70 / 1.70² = 24,2214…  → 24,2
        assert_eq!(imc(70.0, 1.70), Some(24.2));
        // 60 / 1.60² = 23,4375 → 23,4 (y no 23,44: un decimal)
        assert_eq!(imc(60.0, 1.60), Some(23.4));
        // 50 / 1.75² = 16,3265… → 16,3
        assert_eq!(imc(50.0, 1.75), Some(16.3));
    }

    /// **Lo que no se puede calcular es `None`, nunca cero.** Un IMC de 0 tiene
    /// forma de dato y se lee como una medición.
    #[test]
    fn sin_peso_o_sin_talla_no_se_inventa_un_indice() {
        assert_eq!(imc(0.0, 1.70), None);
        assert_eq!(imc(70.0, 0.0), None);
        assert_eq!(imc(-70.0, 1.70), None, "un peso negativo no es un peso");
        assert_eq!(imc(70.0, -1.70), None);
        assert_eq!(imc(f64::NAN, 1.70), None);
        assert_eq!(imc(70.0, f64::INFINITY), None);
        // Y la pareja, que es la que hace útil todo lo anterior: con datos, sale.
        assert!(imc(70.0, 1.70).is_some());
    }

    /// **El peso ideal es el del sistema vivo**, factor por factor. Contrastado
    /// contra `backend/antropometria.py`, que es el que migró la fórmula del VFP
    /// y el que el prestador viene viendo.
    #[test]
    fn el_peso_ideal_es_el_mismo_que_calcula_el_sistema_vivo() {
        // 1,70² × 0,23 × 100 = 66,47 → 66,5 (hombre)
        assert_eq!(peso_ideal(1.70, "M", 30), Some(66.5));
        // 1,60² × 0,22 × 100 = 56,32 → 56,3 (mujer)
        assert_eq!(peso_ideal(1.60, "F", 30), Some(56.3));
        // El mismo cuerpo, distinto sexo, distinto objetivo: no es el mismo número.
        assert_ne!(peso_ideal(1.70, "M", 30), peso_ideal(1.70, "F", 30));
    }

    /// **Paridad con el oráculo**, que es como se prueba todo lo que este port
    /// hereda del sistema vivo. Los ocho valores salen de correr
    /// `backend/antropometria.py:peso_ideal` con los mismos argumentos
    /// (2026-08-11), no de recalcular la fórmula a mano — recalcularla aquí
    /// mediría esta implementación contra sí misma.
    #[test]
    fn coincide_con_el_python_valor_a_valor() {
        let casos: &[(f64, &str, i64, Option<f64>)] = &[
            (1.70, "M", 30, Some(66.5)),
            (1.60, "F", 30, Some(56.3)),
            (1.65, "X", 40, Some(59.9)),
            (1.50, "M", 12, None),
            (1.50, "M", 13, Some(51.8)),
            (1.65, "m", 40, Some(62.6)),
            (1.82, "M", 55, Some(76.2)),
            (1.55, "FEMENINO", 22, Some(52.9)),
        ];
        for &(talla, sexo, edad, esperado) in casos {
            assert_eq!(
                peso_ideal(talla, sexo, edad),
                esperado,
                "talla {talla} sexo {sexo} edad {edad}"
            );
        }
    }

    /// **El corte de los 12 años es `<=`, no `<`.** El original no calcula A los
    /// doce; empieza a los trece. Es el borde donde se equivoca un `<`.
    #[test]
    fn a_los_doce_todavia_no_hay_peso_ideal() {
        assert_eq!(peso_ideal(1.50, "M", 12), None, "a los 12 el original no calcula");
        assert!(peso_ideal(1.50, "M", 13).is_some(), "a los 13 sí");
        assert_eq!(peso_ideal(1.20, "F", 8), None);
    }

    /// **Cualquier sexo que no empiece por «M» es femenino**, igual que el
    /// `IF Lsexo='M' … ELSE` del original. Se prueba para que quede escrito que
    /// es fidelidad y no un descuido.
    #[test]
    fn el_sexo_que_no_empieza_por_eme_se_trata_como_femenino() {
        let mujer = peso_ideal(1.65, "F", 40);
        assert_eq!(peso_ideal(1.65, "", 40), mujer, "vacío cae al lado femenino");
        assert_eq!(peso_ideal(1.65, "X", 40), mujer);
        assert_eq!(peso_ideal(1.65, "  f  ", 40), mujer, "se recorta y no distingue caja");
        // Y la pareja: «m» minúscula SÍ es el lado masculino.
        assert_eq!(peso_ideal(1.65, "m", 40), peso_ideal(1.65, "MASCULINO", 40));
        assert_ne!(peso_ideal(1.65, "m", 40), mujer);
    }

    /// Una talla imposible no produce un peso ideal de cero.
    #[test]
    fn sin_talla_no_se_inventa_un_peso_ideal() {
        assert_eq!(peso_ideal(0.0, "M", 30), None);
        assert_eq!(peso_ideal(-1.70, "M", 30), None);
        assert_eq!(peso_ideal(f64::NAN, "M", 30), None);
        assert!(peso_ideal(1.70, "M", 30).is_some());
    }

    /// Los cuatro tramos de la OMS, **por sus bordes**: es donde se equivoca un
    /// `<` que debía ser `<=`.
    #[test]
    fn la_categoria_cambia_en_los_cortes_de_la_oms() {
        assert_eq!(categoria_imc(18.4), "bajo peso");
        assert_eq!(categoria_imc(18.5), "normal", "18,5 ya es normal");
        assert_eq!(categoria_imc(24.9), "normal");
        assert_eq!(categoria_imc(25.0), "sobrepeso", "25 ya es sobrepeso");
        assert_eq!(categoria_imc(29.9), "sobrepeso");
        assert_eq!(categoria_imc(30.0), "obesidad", "30 ya es obesidad");
        assert_eq!(categoria_imc(45.0), "obesidad");
    }

    // ————————————————————————— la talla ideal —————————————————————————

    /// El positivo de control de todo lo de abajo: si el `include_str!`
    /// apuntara a otro sitio o el JSON llegara vacío, cada `talla_ideal`
    /// devolvería `None` y las pruebas que esperan `None` seguirían VERDES.
    /// Sin esta, «la tabla dice que no hay fila» y «no hay tabla» se ven igual.
    #[test]
    fn la_tabla_empotrada_es_la_de_pesotalla1() {
        let t = tabla();
        assert_eq!(t.len(), 62, "PESOTALLA1 tiene 62 filas");
        assert_eq!(
            t.iter().filter(|f| f.unidad == "meses").count(),
            24,
            "doce meses por cada sexo"
        );
        assert_eq!(
            t.iter().filter(|f| f.unidad == "anios").count(),
            38,
            "diecinueve años por cada sexo"
        );
    }

    /// Valores leídos de la tabla, no calculados aquí: si alguien regenerara el
    /// JSON mal, esto se pone rojo con el número que salió.
    #[test]
    fn la_talla_ideal_sale_de_la_fila_que_le_toca() {
        // Recién nacido: 0 meses. Hombre 0,50 y mujer 0,48 — no es el mismo
        // valor, que es lo que prueba que el sexo se está usando.
        assert_eq!(talla_ideal(0, "M"), Some(0.50));
        assert_eq!(talla_ideal(0, "F"), Some(0.48));
        // Once meses sigue en el tramo de MESES.
        assert_eq!(talla_ideal(11, "M"), Some(0.77));
        // Doce meses cruza al tramo de AÑOS y cambia de fila: 0,80, no 0,77.
        assert_eq!(talla_ideal(12, "M"), Some(0.80));
        // Adulto: 19 años, la última fila que la tabla trae.
        assert_eq!(talla_ideal(19 * 12, "M"), Some(1.65));
        assert_eq!(talla_ideal(19 * 12, "F"), Some(1.70));
    }

    /// La pareja del de arriba: lo que la tabla NO cubre dice que no se sabe.
    #[test]
    fn fuera_de_la_tabla_no_se_inventa_una_talla() {
        // Veinte años: la tabla acaba en 19 y no se extrapola.
        assert_eq!(talla_ideal(20 * 12, "M"), None);
        assert_eq!(talla_ideal(45 * 12, "F"), None);
        // Una edad imposible tampoco cae en la fila 0 por descuido.
        assert_eq!(talla_ideal(-1, "M"), None);
    }

    /// Es una BÚSQUEDA escalonada, no una curva: 42 meses son 3 años, y la
    /// respuesta es la fila de 3 — no un valor entre la de 3 y la de 4.
    #[test]
    fn dentro_del_mismo_anio_la_talla_no_se_interpola() {
        let tres = talla_ideal(3 * 12, "M");
        assert_eq!(talla_ideal(3 * 12 + 6, "M"), tres);
        assert_eq!(talla_ideal(3 * 12 + 11, "M"), tres);
        assert_ne!(talla_ideal(4 * 12, "M"), tres, "al cumplir 4 sí cambia");
    }

    /// FIDELIDAD, no descuido: el `IF Lsexo='M' … ELSE` del original manda
    /// cualquier cosa que no empiece por «M» al lado femenino. Misma regla que
    /// [`peso_ideal`], y por eso se prueba con su nombre.
    #[test]
    fn un_sexo_que_no_empieza_por_eme_cae_al_lado_femenino() {
        let femenino = talla_ideal(0, "F");
        assert_eq!(talla_ideal(0, "otro"), femenino);
        assert_eq!(talla_ideal(0, ""), femenino);
        assert_eq!(talla_ideal(0, "masculino"), talla_ideal(0, "M"));
    }
}
