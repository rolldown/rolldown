use std::sync::Arc;

use rolldown_error::BuildError;
use rolldown_fs::FileSystemExt;
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{
  bundle::output::Output,
  graph::graph::Graph,
  plugin_driver::{PluginDriver, SharedPluginDriver},
};
use crate::{
  bundler::{bundle::bundle::Bundle, stages::build_stage::BuildStage},
  plugin::plugin::BoxPlugin,
  InputOptions, OutputOptions,
};

type BuildResult<T> = Result<T, Vec<BuildError>>;

pub struct Bundler<T: FileSystemExt> {
  input_options: InputOptions,
  plugin_driver: SharedPluginDriver,
  fs: Arc<T>,
}

impl<T: FileSystemExt + Default + 'static> Bundler<T> {
  pub fn new(input_options: InputOptions, fs: T) -> Self {
    // rolldown_tracing::enable_tracing_on_demand();
    Self::with_plugins(input_options, vec![], fs)
  }

  pub fn with_plugins(input_options: InputOptions, plugins: Vec<BoxPlugin>, fs: T) -> Self {
    // rolldown_tracing::enable_tracing_on_demand();
    let fs = Arc::new(fs);
    Self { input_options, plugin_driver: Arc::new(PluginDriver::new(plugins)), fs }
  }

  pub async fn write(&mut self, output_options: OutputOptions) -> BuildResult<Vec<Output>> {
    let dir =
      self.input_options.cwd.as_path().join(&output_options.dir).to_string_lossy().to_string();

    let assets = self.bundle_up(output_options).await?;

    self.fs.create_dir_all(dir.as_path()).unwrap_or_else(|_| {
      panic!(
        "Could not create directory for output chunks: {:?} \ncwd: {}",
        dir.as_path(),
        self.input_options.cwd.display()
      )
    });
    for chunk in &assets {
      let dest = dir.as_path().join(chunk.file_name());
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).unwrap();
        }
      };
      self.fs.write(dest.as_path(), chunk.content().as_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write file in {:?}", dir.as_path().join(chunk.file_name()))
      });
    }

    Ok(assets)
  }

  pub async fn generate(&mut self, output_options: OutputOptions) -> BuildResult<Vec<Output>> {
    self.bundle_up(output_options).await
  }

  async fn bundle_up(&mut self, output_options: OutputOptions) -> BuildResult<Vec<Output>> {
    tracing::trace!("InputOptions {:#?}", self.input_options);
    tracing::trace!("OutputOptions: {output_options:#?}",);

    let resolver = Arc::new(Resolver::with_cwd_and_fs(
      self.input_options.cwd.clone(),
      false,
      Arc::clone(&self.fs),
    ));

    let build_info =
      BuildStage::new(&self.input_options, Arc::clone(&self.plugin_driver), Arc::clone(&resolver))
        .build(Arc::clone(&self.fs))
        .await?;

    let mut graph = Graph::new(build_info);
    graph.link()?;

    let mut bundle = Bundle::new(&mut graph, &output_options);
    let assets = bundle.generate(&self.input_options);

    Ok(assets)
  }
}
