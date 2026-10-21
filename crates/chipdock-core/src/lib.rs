//! Hardware-independent application core and typed UniFFI API for ChipDock.

pub mod api;
pub mod error;
pub mod models;
pub mod serial;
pub mod services;
pub mod timestamp;

uniffi::setup_scaffolding!();
