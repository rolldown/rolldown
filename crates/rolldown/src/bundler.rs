use std::sync::Arc;

use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{BoxPlugin, HookBuildEndArgs, SharedPluginDriver};

use sugar_path::AsPath;

use super::stages::{
  link_stage::{LinkStage, LinkStageOutput},
  scan_stage::ScanStageOutput,
};
use crate::{
  bundler_builder::BundlerBuilder,
  error::{BatchedErrors, BatchedResult},
  options::{
    normalized_input_options::SharedNormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
  stages::{bundle_stage::BundleStage, scan_stage::ScanStage},
  types::rolldown_output::RolldownOutput,
  InputOptions, OutputOptions, SharedResolver,
};

pub struct Bundler {
  pub(crate) input_options: SharedNormalizedInputOptions,
  pub(crate) output_options: NormalizedOutputOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) fs: OsFileSystem,
  pub(crate) resolver: SharedResolver,
}

impl Bundler {
  pub fn new(input_options: InputOptions, output_options: OutputOptions) -> Self {
    BundlerBuilder::default()
      .with_input_options(input_options)
      .with_output_options(output_options)
      .build()
  }

  pub fn with_plugins(
    input_options: InputOptions,
    output_options: OutputOptions,
    plugins: Vec<BoxPlugin>,
  ) -> Self {
    BundlerBuilder::default()
      .with_input_options(input_options)
      .with_output_options(output_options)
      .with_plugins(plugins)
      .build()
  }
}

impl Bundler {
  pub async fn write(&mut self) -> BatchedResult<RolldownOutput> {
    let dir =
      self.input_options.cwd.as_path().join(&self.output_options.dir).to_string_lossy().to_string();

    let output = self.bundle_up(true).await?;

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

  pub async fn generate(&mut self) -> BatchedResult<RolldownOutput> {
    self.bundle_up(false).await
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
      self.fs.clone(),
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
  async fn bundle_up(&mut self, is_write: bool) -> BatchedResult<RolldownOutput> {
    tracing::trace!("InputOptions {:#?}", self.input_options);
    tracing::trace!("OutputOptions: {:#?}", self.output_options);
    let mut link_stage_output = self.try_build().await?;

    let mut bundle_stage = BundleStage::new(
      &mut link_stage_output,
      &self.input_options,
      &self.output_options,
      &self.plugin_driver,
    );

    let assets = bundle_stage.bundle().await?;

    self.plugin_driver.generate_bundle(&assets, is_write).await?;

    Ok(RolldownOutput { warnings: std::mem::take(&mut link_stage_output.warnings), assets })
  }
}
