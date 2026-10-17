//! Hardware-independent application core and typed UniFFI API for FlashDesk.

pub mod api;
pub mod error;
pub mod models;
pub mod services;
pub mod timestamp;

uniffi::setup_scaffolding!();
