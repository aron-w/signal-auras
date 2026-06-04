pub mod adapter;
pub mod capability;
pub mod diagnostics;
pub mod evdev;
pub mod event_loop;
pub mod input;
pub mod kde;
pub mod kde_bridge;
pub mod overlay;
pub mod portal;
pub mod process;
pub mod shortcut;
pub mod uinput;

pub use adapter::{MockableWaylandAdapter, RealWaylandAdapter};
pub use kde::KdePlasmaAdapter;
