pub mod config_meta;
pub mod config_option_id;
pub mod content;
pub mod notifications;
pub mod settings;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

// Re-export rmcp elicitation schema types so downstream crates (e.g. wisp)
// don't need a direct rmcp dependency.
pub use rmcp::model::{
    ConstTitle, ElicitationSchema, EnumSchema, MultiSelectEnumSchema, PrimitiveSchema,
    SingleSelectEnumSchema,
};
