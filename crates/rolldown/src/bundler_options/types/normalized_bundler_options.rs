//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::{path::PathBuf, sync::Arc};

use derivative::Derivative;
use rolldown_common::Platform;

use crate::{AddonOutputOption, External, InputItem};

use super::{
  file_name_template::FileNameTemplate, output_format::OutputFormat, source_map_type::SourceMapType,
};

pub(crate) type SharedOptions = Arc<NormalizedBundlerOptions>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct NormalizedBundlerOptions {
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: External,
  pub treeshake: bool,
  pub platform: Platform,
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
  pub banner: AddonOutputOption,
  pub footer: AddonOutputOption,
}
