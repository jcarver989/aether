use rmcp::{RoleServer, service::DynService};
use std::collections::HashMap;

/// Factory function that creates an MCP server instance
pub type ServerFactory = Box<dyn Fn() -> Box<dyn DynService<RoleServer>> + Send + Sync>;

/// Registry for InMemory MCP server factories
pub struct McpServerRegistry {
    factories: HashMap<String, ServerFactory>,
}

impl McpServerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a server factory with a given name
    pub fn register(&mut self, name: impl Into<String>, factory: ServerFactory) -> &mut Self {
        self.factories.insert(name.into(), factory);
        self
    }

    /// Create a server instance from a registered factory
    pub fn create(&self, name: &str) -> Result<Box<dyn DynService<RoleServer>>, RegistryError> {
        self.factories
            .get(name)
            .ok_or_else(|| RegistryError::FactoryNotFound(name.to_string()))
            .map(|f| f())
    }
}

impl Default for McpServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum RegistryError {
    FactoryNotFound(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::FactoryNotFound(name) => {
                write!(f, "Server factory '{}' not found in registry", name)
            }
        }
    }
}

impl std::error::Error for RegistryError {}
