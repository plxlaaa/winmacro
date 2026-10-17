//! winmacro-logic: pure domain logic shared by the Tauri backend.
//! No Tauri / GUI dependencies — fully unit-testable on any host.

pub mod filter;
pub mod model;
pub mod store;
pub mod transform;

pub use model::{InputEvent, Recording, Settings};
