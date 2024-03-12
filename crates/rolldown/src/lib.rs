mod ast_scanner;
mod bundler;
mod chunk;
mod chunk_graph;
mod error;
mod finalizer;
mod module_loader;
mod options;
mod plugin_driver;
mod runtime;
mod stages;
mod types;
mod utils;

use std::sync::Arc;

use rolldown_resolver::Resolver;

pub(crate) type SharedResolver<T> = Arc<Resolver<T>>;

pub use crate::{
  bundler::{Bundler, RolldownOutput},
  chunk::render_chunk::PreRenderedChunk,
  options::{
    file_name_template::FileNameTemplate,
    input_options::{External, InputItem, InputOptions},
    output_options::{OutputFormat, OutputOptions, SourceMapType},
  },
};
