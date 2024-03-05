use std::sync::Arc;

use rolldown_common::Output;
use rolldown_error::BuildError;
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{BoxPlugin, HookBuildEndArgs};
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{
  options::input_options::SharedInputOptions,
  plugin_driver::{PluginDriver, SharedPluginDriver},
  stages::{
    link_stage::{LinkStage, LinkStageOutput},
    scan_stage::ScanStageOutput,
  },
};
use crate::{
  error::{BatchedErrors, BatchedResult},
  stages::{bundle_stage::BundleStage, scan_stage::ScanStage},
  InputOptions, OutputOptions, SharedResolver,
};

pub struct RolldownOutput {
  pub warnings: Vec<BuildError>,
  pub assets: Vec<Output>,
}

pub struct Bundler<T: FileSystem + Default> {
  input_options: SharedInputOptions,
  plugin_driver: SharedPluginDriver,
  fs: T,
  resolver: SharedResolver<T>,
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
    }
  }

  pub async fn write(&mut self, output_options: OutputOptions) -> BatchedResult<RolldownOutput> {
    let dir =
      self.input_options.cwd.as_path().join(&output_options.dir).to_string_lossy().to_string();

    let output = self.bundle_up(output_options, true).await?;

    self.plugin_driver.write_bundle(&output.assets).await?;

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

  pub async fn scan(&mut self) -> BatchedResult<()> {
    self.plugin_driver.build_start().await?;

    let ret = self.scan_inner().await;

    self.call_build_end_hook(&ret).await?;

    ret?;

    Ok(())
  }

  async fn call_build_end_hook(
    &mut self,
    ret: &Result<ScanStageOutput, BatchedErrors>,
  ) -> BatchedResult<()> {
    if let Err(e) = ret {
      let error = e.get().expect("should have a error");
      self
        .plugin_driver
        .build_end(Some(&HookBuildEndArgs {
          // TODO(hyf0): 1.Need a better way to expose the error
          error: error.to_string(),
        }))
        .await?;
      Ok(())
    } else {
      self.plugin_driver.build_end(None).await?;

      Ok(())
    }
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
    self.plugin_driver.build_start().await?;

    let scan_ret = self.scan_inner().await;

    self.call_build_end_hook(&scan_ret).await?;

    let build_info = scan_ret?;

    let link_stage = LinkStage::new(build_info, &self.input_options);
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
    let mut link_stage_output = self.try_build().await?;

    let mut bundle_stage = BundleStage::new(
      &mut link_stage_output,
      &self.input_options,
      &output_options,
      &self.plugin_driver,
    );

    let assets = bundle_stage.bundle().await?;

    self.plugin_driver.generate_bundle(&assets, is_write).await?;

    Ok(RolldownOutput { warnings: std::mem::take(&mut link_stage_output.warnings), assets })
  }
}
