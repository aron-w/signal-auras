pub mod adapter;
pub mod capability;
pub mod diagnostics;
pub mod input;
pub mod kde;
pub mod kde_bridge;
pub mod portal;
pub mod process;
pub mod shortcut;

pub use adapter::{MockableWaylandAdapter, RealWaylandAdapter};
pub use kde::KdePlasmaAdapter;
