mod ast_scanner;
mod bundler;
mod bundler_builder;
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
pub(crate) type SharedOptions = Arc<NormalizedBundlerOptions>;

pub use crate::{
  bundler::Bundler, bundler_builder::BundlerBuilder, chunk::render_chunk::PreRenderedChunk,
  types::rolldown_output::RolldownOutput,
};

pub use rolldown_common::bundler_options::*;

pub use rolldown_resolver::ResolveOptions;
