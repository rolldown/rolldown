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

use rolldown_fs::OsFileSystem;
use rolldown_resolver::Resolver;

pub(crate) type SharedResolver = Arc<Resolver<OsFileSystem>>;

pub use crate::{
  bundler::Bundler,
  bundler_builder::BundlerBuilder,
  chunk::render_chunk::PreRenderedChunk,
  options::{
    file_name_template::FileNameTemplate,
    input_options::{resolve_options::ResolveOptions, External, InputOptions},
    output_options::{OutputFormat, OutputOptions, SourceMapType},
    types::input_item::InputItem,
    types::output_option::AddonOutputOption,
  },
  types::rolldown_output::RolldownOutput,
};
