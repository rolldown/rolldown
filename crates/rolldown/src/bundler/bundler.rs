use std::sync::Arc;

use rolldown_error::BuildError;
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{
  bundle::output::Output,
  graph::graph::Graph,
  plugin_driver::{PluginDriver, SharedPluginDriver},
};
use crate::{
  bundler::{bundle::bundle::Bundle, stages::scan_stage::ScanStage},
  error::BatchedResult,
  plugin::plugin::BoxPlugin,
  HookBuildEndArgs, InputOptions, OutputOptions,
};

type BuildResult<T> = Result<T, Vec<BuildError>>;

pub struct Bundler<T: FileSystem> {
  input_options: InputOptions,
  plugin_driver: SharedPluginDriver,
  fs: Arc<T>,
}

impl Bundler<OsFileSystem> {
  pub fn new(input_options: InputOptions) -> Self {
    Self::with_plugins(input_options, vec![])
  }

  pub fn with_plugins(input_options: InputOptions, plugins: Vec<BoxPlugin>) -> Self {
    Self::with_plugins_and_fs(input_options, plugins, OsFileSystem)
  }
}

impl<T: FileSystem + Default + 'static> Bundler<T> {
  pub fn with_plugins_and_fs(input_options: InputOptions, plugins: Vec<BoxPlugin>, fs: T) -> Self {
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

  async fn build(&mut self) -> BatchedResult<Graph> {
    self.plugin_driver.build_start().await?;

    let build_ret = self.build_inner().await;

    if let Err(e) = build_ret {
      let error = e.get().expect("should have a error");
      self
        .plugin_driver
        .build_end(Some(&HookBuildEndArgs {
          // TODO(hyf0): 1.Need a better way to expose the error
          error: format!("{:?}\n{:?}", error.code(), error.to_diagnostic().print_to_string()),
        }))
        .await?;
      return Err(e);
    }

    self.plugin_driver.build_end(None).await?;
    build_ret
  }

  async fn build_inner(&mut self) -> BatchedResult<Graph> {
    // TODO: should use a unified resolver
    let resolver = Arc::new(Resolver::with_cwd_and_fs(
      self.input_options.cwd.clone(),
      false,
      Arc::clone(&self.fs),
    ));

    let build_info =
      ScanStage::new(&self.input_options, Arc::clone(&self.plugin_driver), Arc::clone(&resolver))
        .scan(Arc::clone(&self.fs))
        .await?;

    let mut graph = Graph::new(build_info);
    graph.link()?;
    Ok(graph)
  }

  async fn bundle_up(&mut self, output_options: OutputOptions) -> BuildResult<Vec<Output>> {
    tracing::trace!("InputOptions {:#?}", self.input_options);
    tracing::trace!("OutputOptions: {output_options:#?}",);
    let mut graph = self.build().await?;
    let mut bundle = Bundle::new(&mut graph, &output_options);
    let assets = bundle.generate(&self.input_options);

    Ok(assets)
  }
}
