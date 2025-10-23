mod files;
mod server;
mod substitution;

pub use server::{PluginsMcp, PluginsMcpArgs};
pub use substitution::substitute_parameters;
