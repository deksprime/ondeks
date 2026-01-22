//! Plugin architecture for extensible audio processing.
//!
//! This module provides the foundation for a plugin system that allows
//! custom instruments and effects to be registered and instantiated.

mod traits;
mod manager;

pub use traits::{Plugin, PluginInfo, PluginCategory, PluginError};
pub use manager::PluginManager;
