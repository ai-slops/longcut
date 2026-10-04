//! Producer-neutral completed-session metadata, not a project backup or media decoder.

mod loader;
pub mod model;
mod report;
pub mod schema;
mod validation;

pub use loader::{LoadedPackage, inspect, load};
pub use report::{Capabilities, Conformance, Diagnostic, Report, Severity};
pub use validation::validate;
