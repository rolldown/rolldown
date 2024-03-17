use std::sync::Arc;

use rolldown_plugin::{BoxPlugin, PluginDriver};
use rolldown_resolver::Resolver;

use crate::{types::bundler_fs::BundlerFileSystem, Bundler, InputOptions, OutputOptions};

pub struct BundlerBuilder<Fs: BundlerFileSystem> {
  input_options: InputOptions,
  output_options: OutputOptions,
  fs: Fs,
  plugins: Vec<BoxPlugin>,
}

impl<Fs: BundlerFileSystem> BundlerBuilder<Fs> {
  pub fn build(mut self) -> Bundler<Fs> {
    rolldown_tracing::try_init_tracing();
    Bundler {
      resolver: Resolver::with_cwd_and_fs(
        self.input_options.cwd.clone(),
        std::mem::take(&mut self.input_options.resolve),
        self.fs.share(),
      )
      .into(),
      plugin_driver: Arc::new(PluginDriver::new(self.plugins)),
      input_options: Arc::new(self.input_options),
      output_options: self.output_options,
      fs: self.fs,
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

  pub fn with_file_system<NewFs: BundlerFileSystem>(self, fs: NewFs) -> BundlerBuilder<NewFs> {
    BundlerBuilder {
      input_options: self.input_options,
      fs,
      plugins: self.plugins,
      output_options: self.output_options,
    }
  }
}

impl<Fs: BundlerFileSystem> Default for BundlerBuilder<Fs> {
  fn default() -> Self {
    Self {
      input_options: InputOptions::default(),
      fs: Fs::default(),
      plugins: vec![],
      output_options: OutputOptions::default(),
    }
  }
}
