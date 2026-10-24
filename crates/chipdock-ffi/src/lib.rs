//! Swift bindings over the shared ChipDock application service.

mod api;
mod records;
mod serial;

pub use api::*;
pub use records::*;
pub use serial::*;

uniffi::setup_scaffolding!();
