//! GPUI View components for Sona Desktop.

pub mod create;
pub mod dialogs;
pub mod overlays;
pub mod projects;
#[allow(clippy::too_many_arguments)]
pub mod settings;
pub mod shell;
pub mod tools;
pub mod transcript;

pub use shell::{AppMode, MainShell};
