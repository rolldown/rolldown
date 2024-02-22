mod ast_scanner;
pub mod bundle;
#[allow(clippy::module_inception)]
pub mod bundler;
pub mod chunk;
mod chunk_graph;
mod finalizer;
mod linker;
mod module;
mod module_loader;
pub mod options;
pub mod plugin_driver;
mod runtime;
pub mod stages;
mod types;
pub mod utils;
