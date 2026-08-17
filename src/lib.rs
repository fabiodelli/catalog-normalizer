//! Normalizza righe prodotto disordinate in un catalogo strutturato.
//!
//! Il principio, in una riga: **le regole risolvono, il modello vede solo il
//! residuo, e le regole validano anche quello.**
//!
//! - [`rules`] contiene la logica deterministica. Ogni campo produce un
//!   [`model::Outcome`]: risolto, assente, o ambiguo con la motivazione.
//! - [`resolver`] gestisce cio' che resta ambiguo. L'implementazione
//!   predefinita non risolve niente; quella opzionale interroga un modello in
//!   lotti, contabilizza i token e fa ri-validare ogni risposta dalle regole.
//! - [`pipeline`] mette in fila le due cose e produce catalogo e log degli scarti.

pub mod error;
pub mod model;
pub mod pipeline;
pub mod resolver;
pub mod rules;

pub use error::{Error, Result};
