use std::{fmt::Debug, path::PathBuf};

use self::types::{
  external::External, input_item::InputItem, output_format::OutputFormat,
  output_option::AddonOutputOption, platform::Platform, resolve_options::ResolveOptions,
  source_map_type::SourceMapType,
};

pub mod types;

#[derive(Default, Debug)]
pub struct BundlerOptions {
  // --- options for output
  pub input: Vec<InputItem>,
  pub cwd: Option<PathBuf>,
  pub external: Option<External>,
  pub treeshake: Option<bool>,
  pub platform: Option<Platform>,
  // --- options for output
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,
  pub dir: Option<String>,
  pub format: Option<OutputFormat>,
  pub sourcemap: Option<SourceMapType>,
  pub banner: Option<AddonOutputOption>,
  pub footer: Option<AddonOutputOption>,
  // --- options for resolve
  pub resolve: Option<ResolveOptions>,
}
