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
use anyhow::Result;
use rolldown_error::BuildError;
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{BoxPlugin, HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver};
use sugar_path::SugarPath;
use tracing_chrome::FlushGuard;

pub struct Bundler {
  pub(crate) options: SharedOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) fs: OsFileSystem,
  pub(crate) resolver: SharedResolver,
  pub(crate) _log_guard: Option<FlushGuard>,
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
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn write(&mut self) -> Result<BundleOutput> {
    let dir = self.options.cwd.as_path().join(&self.options.dir).to_string_lossy().to_string();

    let output = self.bundle_up(true).await?;

    self.plugin_driver.write_bundle(&output.assets).await?;

    self.fs.create_dir_all(dir.as_path()).map_err(|err| {
      anyhow::anyhow!(
        "Could not create directory for output chunks: {:?} \ncwd: {}",
        dir.as_path(),
        self.options.cwd.display()
      )
      .context(err)
    })?;
    for chunk in &output.assets {
      let dest = dir.as_path().join(chunk.file_name());
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).unwrap();
        }
      };
      self.fs.write(dest.as_path(), chunk.content().as_bytes()).map_err(|err| {
        anyhow::anyhow!("Failed to write file in {:?}", dir.as_path().join(chunk.file_name()))
          .context(err)
      })?;
    }

    Ok(output)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&mut self) -> Result<BundleOutput> {
    self.bundle_up(false).await
  }

  pub async fn scan(&mut self) -> Result<ScanStageOutput> {
    self.plugin_driver.build_start().await?;

    let ret = ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs.clone(),
      Arc::clone(&self.resolver),
    )
    .scan()
    .await;

    {
      let args =
        Self::normalize_error(&ret, |ret| &ret.errors).map(|error| HookBuildEndArgs { error });

      self.plugin_driver.build_end(args.as_ref()).await?;
    }

    ret
  }

  async fn try_build(&mut self) -> Result<LinkStageOutput> {
    let build_info = self.scan().await?;

    let link_stage = LinkStage::new(build_info, &self.options);
    Ok(link_stage.link())
  }

  async fn bundle_up(&mut self, is_write: bool) -> Result<BundleOutput> {
    let mut link_stage_output = self.try_build().await?;

    self.plugin_driver.render_start().await?;

    let mut generate_stage =
      GenerateStage::new(&mut link_stage_output, &self.options, &self.plugin_driver);

    let output = {
      let ret = generate_stage.generate().await;

      if let Some(error) = Self::normalize_error(&ret, |ret| &ret.errors) {
        self.plugin_driver.render_error(&HookRenderErrorArgs { error }).await?;
      }

      ret?
    };

    self.plugin_driver.generate_bundle(&output.assets, is_write).await?;

    Ok(output)
  }

  fn normalize_error<T>(
    ret: &Result<T>,
    errors_fn: impl Fn(&T) -> &[BuildError],
  ) -> Option<String> {
    ret.as_ref().map_or_else(
      |error| Some(error.to_string()),
      |ret| errors_fn(ret).first().map(ToString::to_string),
    )
  }
}

fn _test_bundler() {
  #[allow(clippy::needless_pass_by_value)]
  fn _assert_send(_foo: impl Send) {}
  let mut bundler = Bundler::new(BundlerOptions::default());
  let write_fut = bundler.write();
  _assert_send(write_fut);
  let mut bundler = Bundler::new(BundlerOptions::default());
  let generate_fut = bundler.generate();
  _assert_send(generate_fut);
}
