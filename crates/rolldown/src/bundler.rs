use std::sync::Arc;

use super::stages::{
  link_stage::{LinkStage, LinkStageOutput},
  scan_stage::ScanStageOutput,
};
use crate::{
  bundler_builder::BundlerBuilder,
  stages::{generate_stage::GenerateStage, scan_stage::ScanStage},
  types::bundle_output::BundleOutput,
  BundlerOptions, SharedOptions, SharedResolver,
};
use rolldown_error::Result;
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{BoxPlugin, HookBuildEndArgs, SharedPluginDriver};
use sugar_path::SugarPath;

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
  pub async fn write(&mut self) -> Result<BundleOutput> {
    let dir = self.options.cwd.as_path().join(&self.options.dir).to_string_lossy().to_string();

    let output = self.bundle_up(true).await?;

    self.plugin_driver.write_bundle(&output.assets).await?;

    self.fs.create_dir_all(dir.as_path()).unwrap_or_else(|_| {
      panic!(
        "Could not create directory for output chunks: {:?} \ncwd: {}",
        dir.as_path(),
        self.options.cwd.display()
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

  pub async fn generate(&mut self) -> Result<BundleOutput> {
    self.bundle_up(false).await
  }

  pub async fn scan(&mut self) -> Result<ScanStageOutput> {
    self.plugin_driver.build_start().await?;

    let ret = self.scan_inner().await;

    self.call_build_end_hook(&ret).await?;

    ret
  }

  async fn call_build_end_hook(&mut self, ret: &Result<ScanStageOutput>) -> Result<()> {
    if let Ok(ret) = ret {
      if let Some(error) = ret.errors.first() {
        self
          .plugin_driver
          .build_end(Some(&HookBuildEndArgs {
            // TODO(hyf0): 1.Need a better way to expose the error
            error: error.to_string(),
          }))
          .await?;
        return Ok(());
      }
    }

    self.plugin_driver.build_end(None).await?;

    Ok(())
  }

  async fn scan_inner(&mut self) -> Result<ScanStageOutput> {
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
  async fn try_build(&mut self) -> Result<LinkStageOutput> {
    self.plugin_driver.build_start().await?;

    let scan_ret = self.scan_inner().await;

    self.call_build_end_hook(&scan_ret).await?;

    let build_info = scan_ret?;

    let link_stage = LinkStage::new(build_info, &self.options);
    Ok(link_stage.link())
  }

  #[tracing::instrument(skip_all)]
  async fn bundle_up(&mut self, is_write: bool) -> Result<BundleOutput> {
    tracing::trace!("Options {:#?}", self.options);
    let mut link_stage_output = self.try_build().await?;

    self.plugin_driver.render_start().await?;

    let mut generate_stage =
      GenerateStage::new(&mut link_stage_output, &self.options, &self.plugin_driver);

    let assets = generate_stage.generate().await?;

    self.plugin_driver.generate_bundle(&assets, is_write).await?;

    Ok(BundleOutput {
      warnings: std::mem::take(&mut link_stage_output.warnings),
      assets,
      errors: std::mem::take(&mut link_stage_output.errors),
    })
  }
}
