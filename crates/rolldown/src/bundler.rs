use std::sync::Arc;

use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{BoxPlugin, HookBuildEndArgs, SharedPluginDriver};
use sugar_path::SugarPath;

use super::stages::{
  link_stage::{LinkStage, LinkStageOutput},
  scan_stage::ScanStageOutput,
};
use crate::{
  bundler_builder::BundlerBuilder,
  error::{BatchedErrors, BatchedResult},
  stages::{bundle_stage::BundleStage, scan_stage::ScanStage},
  types::bundle_output::BundleOutput,
  BundlerOptions, SharedOptions, SharedResolver,
};

pub struct Bundler {
  pub(crate) options: SharedOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) fs: OsFileSystem,
  pub(crate) resolver: SharedResolver,
}

impl Bundler {
  pub fn new(input_options: BundlerOptions) -> Self {
    BundlerBuilder::default().with_options(input_options).build()
  }

  pub fn with_plugins(input_options: BundlerOptions, plugins: Vec<BoxPlugin>) -> Self {
    BundlerBuilder::default().with_options(input_options).with_plugins(plugins).build()
  }
}

impl Bundler {
  pub async fn write(&mut self) -> BatchedResult<BundleOutput> {
    let dir = self.options.cwd.as_path().join(&self.options.dir).to_string_lossy().to_string();

    let output = self.bundle_up(true).await?;

    self.plugin_driver.write_bundle(&output.assets).await?;
    tracing::info!("output {output:#?}");

    // self.fs.create_dir_all(dir.as_path()).unwrap_or_else(|_| {
    //   panic!(
    //     "Could not create directory for output chunks: {:?} \ncwd: {}",
    //     dir.as_path(),
    //     self.options.cwd.display()
    //   )
    // });
    tracing::info!("Write bundle to {:?}", dir);
    for chunk in &output.assets {
      let dest = dir.as_path().join(chunk.file_name());
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).unwrap();
        }
      };
      std::fs::write(dest.as_path(), chunk.content().as_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write file in {:?}", dir.as_path().join(chunk.file_name()))
      });
    }
    tracing::info!("Write bundle to {:?}", dir);

    Ok(output)
  }

  pub async fn generate(&mut self) -> BatchedResult<BundleOutput> {
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
      Arc::clone(&self.options),
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

    let link_stage = LinkStage::new(build_info, &self.options);
    Ok(link_stage.link())
  }

  #[tracing::instrument(skip_all)]
  async fn bundle_up(&mut self, is_write: bool) -> BatchedResult<BundleOutput> {
    tracing::trace!("Options {:#?}", self.options);
    let mut link_stage_output = self.try_build().await?;
    tracing::trace!("create bundle stage");
    let mut bundle_stage =
      BundleStage::new(&mut link_stage_output, &self.options, &self.plugin_driver);

    let assets = bundle_stage.bundle().await?;
    tracing::trace!("generate bundle");

    self.plugin_driver.generate_bundle(&assets, is_write).await?;
    tracing::trace!("plugin_driver::generate bundle");

    Ok(BundleOutput { warnings: std::mem::take(&mut link_stage_output.warnings), assets })
  }
}
