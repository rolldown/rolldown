mod bundler;
mod error;

use std::sync::Arc;

use rolldown_resolver::Resolver;

pub(crate) type SharedResolver<T> = Arc<Resolver<T>>;

pub use crate::bundler::{
  bundler::{Bundler, RolldownOutput},
  chunk::render_chunk::PreRenderedChunk,
  options::{
    file_name_template::FileNameTemplate,
    input_options::{External, InputItem, InputOptions},
    output_options::{OutputFormat, OutputOptions},
  },
};
