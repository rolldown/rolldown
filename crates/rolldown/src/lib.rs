mod ast_scanner;
mod bundler;
mod bundler_builder;
mod bundler_options;
mod chunk;
mod chunk_graph;
mod error;
mod finalizer;
mod module_loader;
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
  bundler_options::{
    types::external::External, types::input_item::InputItem, types::output_format::OutputFormat,
    types::output_option::AddonOutputOption, types::platform::Platform,
    types::source_map_type::SourceMapType, BundlerOptions,
  },
  chunk::render_chunk::PreRenderedChunk,
  types::rolldown_output::RolldownOutput,
};

pub use rolldown_resolver::ResolveOptions;
