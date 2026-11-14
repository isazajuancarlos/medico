// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El RIPS de la Resolución 948 de 2026, sin tocar la base (#65/#67/#250).
//!
//! Port de `backend/rips_completo.py`. Se divide en dos mitades y la separación
//! es la que hace que esto se pueda probar en microsegundos:
//!
//! - [`recolector`] — qué falta y dónde buscarlo. Cada campo de cada documento
//!   pasa por ahí.
//! - [`documentos`] — las piezas que los cinco archivos comparten: el bloque de
//!   identificación del usuario, el documento del profesional que atendió, los
//!   campos de plata y las dos formas de fecha.
//! - [`ac`] — Archivo de Consultas.
//! - [`ap`] — Archivo de Procedimientos.
//! - [`am`] — Archivo de Medicamentos.
//! - [`au`] — Archivo de Urgencias.
//! - [`ah`] — Archivo de Hospitalización.
//! - [`an`] — el arreglo `recienNacidos`, que no es archivo aparte sino un
//!   servicio más del usuario de la MADRE.
//! - [`fev`] — **FEV-RIPS**: el documento ÚNICO que funde los cinco y su sobre,
//!   que es lo que se envía al MUV y la condición del ADR para apagar el Python.
//!
//! # La forma del JSON vive en los tipos
//!
//! Los documentos son `struct` con `#[serde(rename)]`, no mapas: los nombres de
//! la 948 (`codDiagnosticoPrincipal`, `vrServicio`) quedan escritos una vez y el
//! compilador impide que uno se escriba mal. Y el ORDEN de declaración es el
//! orden en que serde los emite, que es el que trae el Python — hace falta para
//! poder comparar los dos JSON byte a byte, que es la condición que el ADR pone
//! para apagar el Python.
//!
//! # Nada de esto consulta la base
//!
//! Los generadores reciben las filas ya traídas (`Fuente*`), así que las reglas
//! se prueban con datos escritos a mano y `medico-datos` solo pone el SQL.

pub mod ac;
pub mod ah;
pub mod am;
pub mod an;
pub mod aos;
pub mod ap;
pub mod au;
pub mod documentos;
pub mod fev;
pub mod recolector;

pub use ac::{documento_ac, Consulta, FuenteConsulta};
pub use ah::{documento_ah, FuenteHospitalizacion, Hospitalizacion};
pub use am::{documento_am, Cantidad, FuenteMedicamento, Medicamento};
pub use an::{anexar_recien_nacidos, FuenteRecienNacido, RecienNacido};
pub use aos::{documento_aos, FuenteOtroServicio, OtroServicio};
pub use ap::{documento_ap, FuenteProcedimiento, Procedimiento};
pub use au::{documento_au, FuenteUrgencia, Urgencia};
pub use documentos::{FuenteInstitucion, FuentePaciente, FuenteProfesional, Servicios, Usuario};
pub use recolector::{Dato, DatoNumero, DatosIncompletos, Donde, Falta, Recolector};
