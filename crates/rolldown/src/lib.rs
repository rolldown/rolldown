mod ast_scanner;
mod bundler;
mod bundler_builder;
mod chunk;
mod chunk_graph;
mod error;
mod finalizer;
mod module_loader;
mod options;
mod runtime;
mod stages;
mod types;
mod utils;

use std::sync::Arc;

use rolldown_resolver::Resolver;

pub(crate) type SharedResolver<T> = Arc<Resolver<T>>;

pub use crate::{
  bundler::Bundler,
  bundler_builder::BundlerBuilder,
  chunk::render_chunk::PreRenderedChunk,
  options::{
    file_name_template::FileNameTemplate,
    input_options::{External, InputOptions},
    output_options::{OutputFormat, OutputOptions},
    types::input_item::InputItem,
  },
  types::rolldown_output::RolldownOutput,
};
