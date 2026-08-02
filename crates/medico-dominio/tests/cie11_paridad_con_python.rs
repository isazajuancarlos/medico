// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! El binario y el Python tienen que derivar el MISMO CIE-11 (#250).
//!
//! El ADR no autoriza apagar el Python hasta que el RIPS de Rust sea byte a
//! byte idéntico, así que cualquier diferencia entre los dos es un fallo, no una
//! variante. Y la diferencia peligrosa no rompe nada: dos códigos CIE-11
//! distintos para el mismo diagnóstico entran los dos como datos válidos.
//!
//! Hay dos formas de divergir y esta prueba cubre las dos:
//!
//! 1. **Los datos.** `crates/medico-dominio/datos/mapeo_cie10_cie11.json` es la
//!    reducción del de `backend/datos/`, y la escribe la misma corrida de
//!    `herramientas/generar_mapeo_cie11.py`. Si alguien regenera uno solo, o
//!    edita uno a mano, aquí se ve.
//! 2. **La lógica.** `derivar` exige equivalencia exacta Y destino único. La
//!    prueba la recalcula desde el archivo del Python —sin usar el código que
//!    está probando— y compara los 12 406 códigos, uno a uno.
//!
//! Se lee el archivo del Python por ruta relativa al crate. Si no está, la
//! prueba FALLA en vez de saltarse: una comprobación que se desactiva sola
//! cuando falta su insumo es la que nunca vuelve a decir nada (directiva 20).

use medico_dominio::cie11;
use serde_json::Value;

fn mapeo_del_python() -> Value {
    let ruta = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../backend/datos/mapeo_cie10_cie11.json"
    );
    let crudo = std::fs::read_to_string(ruta).unwrap_or_else(|e| {
        panic!(
            "no se pudo leer el mapeo del Python en {ruta}: {e}.\n\
             Es la fuente contra la que se compara el empotrado; sin él esta \
             prueba no comprueba nada y por eso falla en vez de saltarse."
        )
    });
    serde_json::from_str(&crudo).expect("el mapeo del Python es JSON válido")
}

#[test]
fn el_empotrado_es_la_reduccion_exacta_del_mapeo_del_python() {
    let python = mapeo_del_python();
    let mapa = python["mapa"].as_object().expect("el Python trae `mapa`");

    assert_eq!(
        cie11::total_mapeados(),
        mapa.len(),
        "el empotrado tiene {} códigos y el del Python {}: alguno se regeneró solo",
        cie11::total_mapeados(),
        mapa.len()
    );
    assert_eq!(cie11::release(), python["release"].as_str().unwrap());
    assert_eq!(
        cie11::version_tabla(),
        python["version_tabla"].as_str().unwrap()
    );

    for (codigo, entrada) in mapa {
        let eq = cie11::buscar(Some(codigo))
            .unwrap_or_else(|| panic!("{codigo} está en el Python y no en el empotrado"));

        let nivel_python = entrada["nivel"].as_str().unwrap();
        let nivel_rust = match eq.nivel {
            cie11::Nivel::Exacto => "exacto",
            cie11::Nivel::Categoria => "categoria",
        };
        assert_eq!(nivel_rust, nivel_python, "nivel distinto en {codigo}");
        assert_eq!(
            eq.via,
            entrada.get("via").and_then(Value::as_str),
            "vía distinta en {codigo}"
        );

        let destinos = entrada["destinos"].as_array().unwrap();
        assert_eq!(
            eq.destinos.len(),
            destinos.len(),
            "número de destinos distinto en {codigo}"
        );
        // El ORDEN importa: `derivar` toma el primero cuando hay uno solo, y la
        // pantalla de captura los ofrecerá en este orden.
        for (i, (rust, py)) in eq.destinos.iter().zip(destinos).enumerate() {
            assert_eq!(rust.codigo, py["codigo"].as_str().unwrap(),
                       "destino {i} de {codigo}");
            assert_eq!(rust.titulo, py["titulo"].as_str().unwrap(),
                       "título del destino {i} de {codigo}");
        }
    }
}

#[test]
fn los_12406_codigos_derivan_igual_que_en_el_python() {
    let python = mapeo_del_python();
    let mapa = python["mapa"].as_object().unwrap();

    let mut derivados = 0usize;
    let mut vacios = 0usize;
    for (codigo, entrada) in mapa {
        // La regla del Python, recalculada aquí desde sus propios datos: exacta
        // Y única. No se llama a nada de `cie11` para obtenerla, o la prueba
        // estaría comparando el código consigo mismo.
        let destinos = entrada["destinos"].as_array().unwrap();
        let esperado = if entrada["nivel"] == "exacto" && destinos.len() == 1 {
            Some((
                destinos[0]["codigo"].as_str().unwrap(),
                destinos[0]["titulo"].as_str().unwrap(),
            ))
        } else {
            None
        };

        let obtenido = cie11::derivar(Some(codigo)).map(|d| (d.codigo.as_str(), d.titulo.as_str()));
        assert_eq!(obtenido, esperado, "derivación distinta en {codigo}");

        match esperado {
            Some(_) => derivados += 1,
            None => vacios += 1,
        }
        // Y el par que se emite al RIPS dice lo mismo que la derivación.
        let par = cie11::par(Some(codigo));
        match esperado {
            Some((c, t)) => assert_eq!(par, (c, t), "par distinto en {codigo}"),
            None => assert_eq!(par, ("", ""), "{codigo} no deriva y el par no va vacío"),
        }
    }

    // Los números medidos el 2026-08-01. Van aquí porque una prueba que recorre
    // todo y no afirma cuánto recorrió pasa igual con el mapa vacío.
    assert_eq!(derivados, 8_070, "cambió cuántos códigos se derivan solos");
    assert_eq!(vacios, 4_336, "cambió cuántos quedan sin derivar");
}
