use std::collections::HashMap;
use super::traits::{Plugin, PluginInfo, PluginError, PluginCategory};

/// Factory function type for creating plugins.
pub type PluginFactory = Box<dyn Fn() -> Box<dyn Plugin> + Send + Sync>;

/// Manages plugin registration and instantiation.
pub struct PluginManager {
    factories: HashMap<String, PluginFactory>,
    info_cache: HashMap<String, PluginInfo>,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            info_cache: HashMap::new(),
        }
    }

    /// Register a plugin factory.
    pub fn register<F>(&mut self, id: &str, info: PluginInfo, factory: F)
    where
        F: Fn() -> Box<dyn Plugin> + Send + Sync + 'static,
    {
        self.factories.insert(id.to_string(), Box::new(factory));
        self.info_cache.insert(id.to_string(), info);
    }

    /// Get list of available plugins.
    pub fn available_plugins(&self) -> Vec<&PluginInfo> {
        self.info_cache.values().collect()
    }

    /// Get plugins by category.
    pub fn plugins_by_category(&self, category: PluginCategory) -> Vec<&PluginInfo> {
        self.info_cache.values()
            .filter(|info| info.category == category)
            .collect()
    }

    /// Create an instance of a plugin.
    pub fn create(&self, id: &str) -> Result<Box<dyn Plugin>, PluginError> {
        let factory = self.factories.get(id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        Ok(factory())
    }

    /// Get plugin info by ID.
    pub fn get_info(&self, id: &str) -> Option<&PluginInfo> {
        self.info_cache.get(id)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
