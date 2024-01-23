use std::sync::Arc;

use rolldown_error::BuildError;
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{
  bundle::output::Output,
  options::input_options::SharedInputOptions,
  plugin_driver::{PluginDriver, SharedPluginDriver},
  stages::{
    link_stage::{LinkStage, LinkStageOutput},
    scan_stage::ScanStageOutput,
  },
};
use crate::{
  bundler::stages::{bundle_stage::BundleStage, scan_stage::ScanStage},
  error::{BatchedErrors, BatchedResult},
  plugin::plugin::BoxPlugin,
  HookBuildEndArgs, InputOptions, OutputOptions, SharedResolver,
};

// Rolldown use this alias for outside users.
type BuildResult<T> = Result<T, Vec<BuildError>>;

pub struct RolldownOutput {
  pub warnings: Vec<BuildError>,
  pub assets: Vec<Output>,
}

pub struct Bundler<T: FileSystem + Default> {
  input_options: SharedInputOptions,
  plugin_driver: SharedPluginDriver,
  fs: T,
  resolver: SharedResolver<T>,
  // Store the build result, using for generate/write.
  build_result: Option<LinkStageOutput>,
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
  pub fn with_plugins_and_fs(
    mut input_options: InputOptions,
    plugins: Vec<BoxPlugin>,
    fs: T,
  ) -> Self {
    rolldown_tracing::try_init_tracing();
    Self {
      resolver: Resolver::with_cwd_and_fs(
        input_options.cwd.clone(),
        std::mem::take(&mut input_options.resolve),
        fs.share(),
      )
      .into(),
      plugin_driver: Arc::new(PluginDriver::new(plugins)),
      input_options: Arc::new(input_options),
      fs,
      build_result: None,
    }
  }

  pub async fn write(&mut self, output_options: OutputOptions) -> BatchedResult<RolldownOutput> {
    let dir =
      self.input_options.cwd.as_path().join(&output_options.dir).to_string_lossy().to_string();

    let output = self.bundle_up(output_options, true).await?;

    self.fs.create_dir_all(dir.as_path()).unwrap_or_else(|_| {
      panic!(
        "Could not create directory for output chunks: {:?} \ncwd: {}",
        dir.as_path(),
        self.input_options.cwd.display()
      )
    });
    for chunk in &output.assets {
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

    Ok(output)
  }

  pub async fn generate(&mut self, output_options: OutputOptions) -> BatchedResult<RolldownOutput> {
    self.bundle_up(output_options, false).await
  }

  pub async fn build(&mut self) -> BuildResult<()> {
    self.build_inner().await?;
    Ok(())
  }

  pub async fn scan(&mut self) -> BatchedResult<()> {
    self.plugin_driver.build_start().await?;

    let ret = self.scan_inner().await;

    self.call_build_end_hook(ret.err()).await?;

    Ok(())
  }

  async fn build_inner(&mut self) -> BatchedResult<()> {
    self.plugin_driver.build_start().await?;

    let ret = self.try_build().await;

    let (err, value) = match ret {
      Err(e) => (Some(e), None),
      Ok(value) => (None, Some(value)),
    };

    self.call_build_end_hook(err).await?;

    self.build_result = value;

    Ok(())
  }

  async fn call_build_end_hook(&mut self, ret: Option<BatchedErrors>) -> BatchedResult<()> {
    if let Some(e) = ret {
      let error = e.get().expect("should have a error");
      self
        .plugin_driver
        .build_end(Some(&HookBuildEndArgs {
          // TODO(hyf0): 1.Need a better way to expose the error
          error: error.to_string(),
        }))
        .await?;
      return Err(e);
    }

    self.plugin_driver.build_end(None).await?;

    Ok(())
  }

  async fn scan_inner(&mut self) -> BatchedResult<ScanStageOutput> {
    ScanStage::new(
      Arc::clone(&self.input_options),
      Arc::clone(&self.plugin_driver),
      self.fs.share(),
      Arc::clone(&self.resolver),
    )
    .scan()
    .await
  }

  #[tracing::instrument(skip_all)]
  async fn try_build(&mut self) -> BatchedResult<LinkStageOutput> {
    let build_info = self.scan_inner().await?;

    let link_stage = LinkStage::new(build_info);

    Ok(link_stage.link())
  }

  #[tracing::instrument(skip_all)]
  async fn bundle_up(
    &mut self,
    output_options: OutputOptions,
    is_write: bool,
  ) -> BatchedResult<RolldownOutput> {
    tracing::trace!("InputOptions {:#?}", self.input_options);
    tracing::trace!("OutputOptions: {output_options:#?}",);
    let graph = self.build_result.as_mut().expect("Build should success");
    let mut bundle_stage =
      BundleStage::new(graph, &self.input_options, &output_options, &self.plugin_driver);
    let assets = bundle_stage.bundle().await?;

    self.plugin_driver.generate_bundle(&assets, is_write).await?;

    Ok(RolldownOutput { warnings: std::mem::take(&mut graph.warnings), assets })
  }
}
