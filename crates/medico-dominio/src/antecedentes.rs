// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! La forma de los cuatro bloques de antecedentes del paciente.
//!
//! # Por qué existe este archivo
//!
//! `paciente.ant_familiares`, `ant_personales`, `ant_ginecologicos` y
//! `ant_perinatales` son cuatro columnas JSON sin esquema: la base acepta
//! cualquier objeto y el servidor los guarda tal cual. Hasta hoy **el único
//! sitio del producto que sabía qué campos lleva cada bloque era el componente
//! de Angular** —cuatro `interface` y sus cuatro `default*()`, 65 campos en
//! total, en `historias.component.ts`—.
//!
//! Con un solo frontal eso funcionaba. Con dos, el formato pasa a tener dos
//! autores, y dos autores de un JSON son un JSON que se separa: basta que una
//! pantalla escriba `ant_perinatales` sin `hosp_ira` para que el campo
//! desaparezca del expediente sin que nada falle. Es exactamente lo que ya se
//! hizo con `resultados_json` en laboratorio, y por la misma razón.
//!
//! **Y aquí el precio es mayor.** IRA, EDA, esquema de vacunación completo,
//! desarrollo psicomotor y muertes de hermanos menores de cinco años son los
//! indicadores de AIEPI y de crecimiento y desarrollo, o sea la materia prima
//! de `epidemiologia.py`. Un campo que se cae en silencio no se nota hasta el
//! informe del mes, cuando ya no se puede reconstruir.
//!
//! # Qué NO hace esta tabla
//!
//! **No valida.** `actualizar_antecedentes` sigue aceptando el `Value` que le
//! llegue por `/api`, igual que antes: rechazar un bloque con un campo de más
//! rompería a cualquier cliente que hoy funciona, y estos datos son historia
//! clínica ya escrita. La tabla dice qué campos CONOCE el producto, para poder
//! dibujarlos y volver a leerlos; lo que venga de más se conserva sin tocarlo.
//!
//! # Los dos campos que siguen sin portarse, a propósito
//!
//! `EM` y `M` de la pestaña AGO del sistema vivo. No están en la fórmula
//! obstétrica G-P-A-C-V y no sabemos qué significan —¿«embarazo múltiple» y
//! «muertos»?, ¿«ectópico/molar»?—. Inventar el sentido de un campo es lo mismo
//! que inventar su valor. Preguntado al prestador.

/// Cómo se captura y cómo se guarda un campo.
///
/// Es el tipo del VALOR en el JSON, no un adorno de pantalla: de él salen tanto
/// el control que se dibuja como la forma de leer lo que llega del formulario.
/// Sin esto, un `0` y un `"0"` conviven en la misma columna según quién la
/// escribiera, y quien la lea después tiene que adivinar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// `true`/`false`. Una casilla que no se marca **no se envía**, así que
    /// siempre se escribe explícitamente al guardar.
    Casilla,
    /// Texto libre.
    Texto,
    /// Número entero o `null`. Nunca cero por defecto: en «hermanos muertos
    /// menores de 5 años», el cero es un dato clínico y la ausencia es que
    /// nadie preguntó.
    Entero,
    /// Número con decimales o `null` (peso y talla al nacer).
    Decimal,
    /// `YYYY-MM-DD` o cadena vacía.
    Fecha,
}

#[derive(Debug, Clone, Copy)]
pub struct Campo {
    pub clave: &'static str,
    pub etiqueta: &'static str,
    pub tipo: Tipo,
}

const fn c(clave: &'static str, etiqueta: &'static str) -> Campo {
    Campo { clave, etiqueta, tipo: Tipo::Casilla }
}
const fn t(clave: &'static str, etiqueta: &'static str) -> Campo {
    Campo { clave, etiqueta, tipo: Tipo::Texto }
}
const fn n(clave: &'static str, etiqueta: &'static str) -> Campo {
    Campo { clave, etiqueta, tipo: Tipo::Entero }
}
const fn d(clave: &'static str, etiqueta: &'static str) -> Campo {
    Campo { clave, etiqueta, tipo: Tipo::Decimal }
}
const fn f(clave: &'static str, etiqueta: &'static str) -> Campo {
    Campo { clave, etiqueta, tipo: Tipo::Fecha }
}

/// Uno de los cuatro bloques. `columna` es el nombre EXACTO de la columna de
/// `paciente`, que es también la clave que espera `PUT /api/pacientes/{id}/antecedentes`.
#[derive(Debug, Clone, Copy)]
pub struct Bloque {
    pub columna: &'static str,
    pub titulo: &'static str,
    /// Cuándo tiene sentido preguntarlo. No cierra nada —el clínico decide—,
    /// pero permite que la pantalla no abra por defecto el bloque perinatal de
    /// un paciente de sesenta años.
    pub nota: &'static str,
    pub campos: &'static [Campo],
}

pub const FAMILIARES: &[Campo] = &[
    c("hta", "Hipertensión arterial"),
    c("diabetes", "Diabetes"),
    c("coronarias", "Enfermedad coronaria"),
    c("cancer", "Cáncer"),
    // Qué cáncer y en quién: «cáncer en la familia» sin decir cuál ni de quién
    // no orienta ningún tamizaje. El original lo tiene atado a un control
    // (`enca_historia.CANCER`, C150), o sea que se rellena.
    t("cancer_desc", "¿Cuál y en quién?"),
    c("acv", "Accidente cerebrovascular"),
    c("tbc", "Tuberculosis"),
    c("asma", "Asma"),
    t("otros", "Otros"),
];

pub const PERSONALES: &[Campo] = &[
    c("hta", "Hipertensión arterial"),
    c("diabetes", "Diabetes"),
    c("coronarias", "Enfermedad coronaria"),
    c("acv", "Accidente cerebrovascular"),
    c("tbc", "Tuberculosis"),
    c("asma", "Asma"),
    c("cancer_activo", "Cáncer"),
    t("cancer_desc", "¿Cuál?"),
    c("quirurgicos_activo", "Cirugías"),
    t("quirurgicos_desc", "¿Cuáles?"),
    c("its_activo", "Infecciones de transmisión sexual"),
    t("its_desc", "¿Cuáles?"),
    t("otros", "Otros"),
    t("habitos", "Hábitos"),
];

pub const GINECOLOGICOS: &[Campo] = &[
    n("menarquia", "Edad de la menarquia"),
    // Edad de inicio de relaciones sexuales: factor de riesgo que el clínico
    // pregunta y que el sistema vivo registra (capturas del 2026-08-03, AGO).
    n("irs", "Edad de inicio de relaciones sexuales"),
    c("ciclos_regulares", "Ciclos regulares"),
    c("dismenorrea", "Dismenorrea"),
    c("metrorragia", "Metrorragia"),
    c("planificacion", "Planifica"),
    t("metodo_planificacion", "Método"),
    c("citologia_vigente", "Citología vigente"),
    f("fecha_citologia", "Fecha de la citología"),
    c("lactancia_actual", "Lactando actualmente"),
    c("embarazo_actual", "Embarazo actual"),
    n("semanas_gestacion", "Semanas de gestación"),
    c("preclampsia", "Preclampsia"),
    n("gestas", "Gestaciones"),
    n("partos", "Partos"),
    n("cesareas", "Cesáreas"),
    n("abortos", "Abortos"),
    n("hijos_vivos", "Hijos vivos"),
    f("fum", "Fecha de la última menstruación"),
    // La probable de parto ordena el control prenatal; la del último parto se
    // usa a diario. Las dos estaban en el sistema vivo y no aquí.
    f("fpp", "Fecha probable de parto"),
    f("fup", "Fecha del último parto"),
    t("obs_adicionales", "Observaciones"),
];

pub const PERINATALES: &[Campo] = &[
    c("embarazo_a_termino", "Embarazo a término"),
    c("embarazo_deseado", "Embarazo deseado"),
    n("semanas_gestacion_nacer", "Semanas de gestación al nacer"),
    d("peso_nacer", "Peso al nacer (g)"),
    d("talla_nacer", "Talla al nacer (cm)"),
    n("apgar_1min", "Apgar al minuto"),
    n("apgar_5min", "Apgar a los 5 minutos"),
    c("parto_normal", "Parto normal"),
    c("parto_cesarea", "Cesárea"),
    c("parto_forceps", "Fórceps"),
    c("lactancia_materna", "Lactancia materna"),
    n("lactancia_meses", "Meses de lactancia"),
    // Los indicadores de AIEPI y de crecimiento y desarrollo. Sin ellos el
    // informe nuevo no da los números que el prestador viene entregando.
    c("hosp_ira", "Hospitalización por IRA"),
    c("hosp_eda", "Hospitalización por EDA"),
    t("hosp_otra", "Otra hospitalización"),
    c("inmunizacion_completa", "Esquema de vacunación completo"),
    c("desarrollo_psicomotor_normal", "Desarrollo psicomotor normal"),
    n("hermanos_muertos_menores_5", "Hermanos muertos menores de 5 años"),
    // El número dice cuántos; la causa es lo que lo convierte en antecedente
    // utilizable, y en materia del informe de AIEPI. El original lo tiene al
    // lado del contador (`enca_historia.CAUSA_HERM`, C100).
    t("causa_muerte_hermanos", "¿De qué murieron?"),
    t("complicaciones", "Complicaciones"),
];

pub const BLOQUES: &[Bloque] = &[
    Bloque {
        columna: "ant_familiares",
        titulo: "Antecedentes familiares",
        nota: "Lo que hay en la familia de primer grado.",
        campos: FAMILIARES,
    },
    Bloque {
        columna: "ant_personales",
        titulo: "Antecedentes personales",
        nota: "Lo que ha tenido el propio paciente.",
        campos: PERSONALES,
    },
    Bloque {
        columna: "ant_ginecologicos",
        titulo: "Antecedentes ginecobstétricos",
        nota: "Se pregunta a quien corresponda; ningún campo es obligatorio.",
        campos: GINECOLOGICOS,
    },
    Bloque {
        columna: "ant_perinatales",
        titulo: "Antecedentes perinatales",
        nota: "Del NIÑO, no de la madre: son los indicadores de AIEPI y de \
               crecimiento y desarrollo.",
        campos: PERINATALES,
    },
];

/// El bloque de una columna, o `None` si esa columna no es de antecedentes.
///
/// Devuelve `Option` y no un defecto (directiva 20): quien pregunte por una
/// columna que no existe tiene un error, y darle el bloque familiar escribiría
/// los antecedentes de la familia en cualquier sitio.
pub fn bloque_de(columna: &str) -> Option<&'static Bloque> {
    BLOQUES.iter().find(|b| b.columna == columna)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Ninguna clave se repite dentro de un bloque.** Dos campos con la misma
    /// clave son un campo que pisa al otro al guardar, y el que se pierde es el
    /// que se dibujó primero — sin un solo error.
    #[test]
    fn ninguna_clave_se_repite_dentro_de_un_bloque() {
        for b in BLOQUES {
            let mut vistas: Vec<&str> = Vec::new();
            for c in b.campos {
                assert!(
                    !vistas.contains(&c.clave),
                    "«{}» está dos veces en {}",
                    c.clave,
                    b.columna
                );
                vistas.push(c.clave);
            }
        }
        // CONTROL: la comprobación de arriba pasa igual con la lista vacía.
        assert_eq!(BLOQUES.len(), 4);
        assert!(BLOQUES.iter().all(|b| b.campos.len() >= 9));
    }

    /// La pareja del anterior: **las claves SÍ se repiten entre bloques** —`hta`
    /// está en familiares y en personales— y eso es correcto, porque cada bloque
    /// es una columna distinta. Se escribe para que nadie «arregle» el guardián
    /// de arriba haciéndolo global.
    #[test]
    fn la_misma_clave_en_dos_bloques_es_legitima() {
        let en_fam = FAMILIARES.iter().any(|c| c.clave == "hta");
        let en_per = PERSONALES.iter().any(|c| c.clave == "hta");
        assert!(en_fam && en_per, "el caso que justifica que el guardián sea por bloque");
    }

    #[test]
    fn se_pregunta_por_columna_y_una_ajena_no_devuelve_nada() {
        assert_eq!(bloque_de("ant_perinatales").unwrap().campos.len(), PERINATALES.len());
        // La pareja: una columna que no es de antecedentes no cae en un defecto.
        assert!(bloque_de("nombre_completo").is_none());
        assert!(bloque_de("").is_none());
    }
}
