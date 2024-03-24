use std::sync::Arc;

use rolldown_fs::OsFileSystem;
use rolldown_plugin::{BoxPlugin, PluginDriver};
use rolldown_resolver::Resolver;

use crate::{
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
  Bundler, InputOptions, OutputOptions,
};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  input_options: InputOptions,
  output_options: OutputOptions,
  plugins: Vec<BoxPlugin>,
}

impl BundlerBuilder {
  pub fn build(self) -> Bundler {
    rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { input_options, output_options, resolve_options } =
      normalize_options(self.input_options, self.output_options);

    Bundler {
      resolver: Resolver::new(resolve_options, input_options.cwd.clone(), OsFileSystem).into(),
      plugin_driver: PluginDriver::new_shared(self.plugins),
      input_options: Arc::new(input_options),
      output_options,
      fs: OsFileSystem,
    }
  }

  #[must_use]
  pub fn with_input_options(mut self, input_options: InputOptions) -> Self {
    self.input_options = input_options;
    self
  }

  #[must_use]
  pub fn with_plugins(mut self, plugins: Vec<BoxPlugin>) -> Self {
    self.plugins = plugins;
    self
  }

  #[must_use]
  pub fn with_output_options(mut self, output_options: OutputOptions) -> Self {
    self.output_options = output_options;
    self
  }
}
