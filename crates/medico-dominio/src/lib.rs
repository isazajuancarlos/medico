//! Reglas del dominio de `medico` que no tocan la base de datos.
//!
//! Todo lo que hay aquí se prueba sin servidor, sin base y en microsegundos.
//! Lo que decide quién ve qué vive en [`aislamiento`]; los roles, en [`rol`].

pub mod agenda;
pub mod aislamiento;
pub mod antecedentes;
pub mod antropometria;
pub mod ajustes;
pub mod cie11;
pub mod consentimiento;
pub mod cum;
pub mod derivaciones;
pub mod facturacion;
pub mod ihce;
pub mod modulo;
pub mod muv;
pub mod procedencia;
pub mod quirofano;
pub mod rips;
pub mod rol;
pub mod sangre;
pub mod zona;

pub use aislamiento::{Ambito, Visibilidad};
pub use rol::RolUsuario;
