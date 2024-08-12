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
use rolldown_common::{NormalizedBundlerOptions, SharedFileEmitter};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{
  HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver, __inner::SharedPluginable,
};
use tracing_chrome::FlushGuard;

pub struct Bundler {
  pub(crate) options: SharedOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) fs: OsFileSystem,
  pub(crate) resolver: SharedResolver,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) _log_guard: Option<FlushGuard>,
}

impl Bundler {
  pub fn new(options: BundlerOptions) -> Self {
    BundlerBuilder::default().with_options(options).build()
  }

  pub fn with_plugins(options: BundlerOptions, plugins: Vec<SharedPluginable>) -> Self {
    BundlerBuilder::default().with_options(options).with_plugins(plugins).build()
  }
}

impl Bundler {
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn write(&mut self) -> Result<BundleOutput> {
    let dir = self.options.cwd.join(&self.options.dir);

    let mut output = self.bundle_up(/* is_write */ true).await?;

    self.fs.create_dir_all(&dir).map_err(|err| {
      anyhow::anyhow!("Could not create directory for output chunks: {:?}", dir).context(err)
    })?;

    for chunk in &output.assets {
      let dest = dir.join(chunk.filename());
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).unwrap();
        }
      };
      self
        .fs
        .write(&dest, chunk.content_as_bytes())
        .map_err(|err| anyhow::anyhow!("Failed to write file in {:?}", dest).context(err))?;
    }

    self.plugin_driver.write_bundle(&mut output.assets).await?;

    Ok(output)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&mut self) -> Result<BundleOutput> {
    self.bundle_up(/* is_write */ false).await
  }

  pub async fn scan(&mut self) -> Result<DiagnosableResult<ScanStageOutput>> {
    self.plugin_driver.build_start().await?;

    let mut error_for_build_end_hook = None;

    let scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
    )
    .scan()
    .await
    {
      Ok(v) => v,
      Err(err) => {
        // TODO: So far we even call build end hooks on unhandleable errors . But should we call build end hook even for unhandleable errors?
        error_for_build_end_hook = Some(err.to_string());
        self
          .plugin_driver
          .build_end(error_for_build_end_hook.map(|error| HookBuildEndArgs { error }).as_ref())
          .await?;
        return Err(err);
      }
    };

    let scan_stage_output = match scan_stage_output {
      Ok(v) => v,
      Err(errs) => {
        if let Some(err_msg) = errs.first().map(ToString::to_string) {
          error_for_build_end_hook = Some(err_msg.clone());
        }
        self
          .plugin_driver
          .build_end(error_for_build_end_hook.map(|error| HookBuildEndArgs { error }).as_ref())
          .await?;
        return Ok(Err(errs));
      }
    };

    self
      .plugin_driver
      .build_end(error_for_build_end_hook.map(|error| HookBuildEndArgs { error }).as_ref())
      .await?;

    Ok(Ok(scan_stage_output))
  }

  async fn try_build(&mut self) -> Result<DiagnosableResult<LinkStageOutput>> {
    let build_info = match self.scan().await? {
      Ok(scan_stage_output) => scan_stage_output,
      Err(errors) => return Ok(Err(errors)),
    };
    Ok(Ok(LinkStage::new(build_info, &self.options).link()))
  }

  #[allow(clippy::missing_transmute_annotations)]
  async fn bundle_up(&mut self, is_write: bool) -> Result<BundleOutput> {
    let mut link_stage_output = match self.try_build().await? {
      Ok(v) => v,
      Err(errors) => return Ok(BundleOutput { assets: vec![], warnings: vec![], errors }),
    };

    self.plugin_driver.set_module_table(unsafe {
      // Can't ensure the safety here. It's only a temporary solution.
      // - We won't mutate the `module_table` in the generate stage.
      // - We transmute the stacked reference to a static lifetime and it haven't met errors due to we happen
      // to only need to access the `module_table` during this function call.
      std::mem::transmute(&link_stage_output.module_table)
    });

    self.plugin_driver.render_start().await?;

    let mut output = {
      let bundle_output =
        GenerateStage::new(&mut link_stage_output, &self.options, &self.plugin_driver)
          .generate()
          .await;

      if let Some(error) = Self::normalize_error(&bundle_output, |ret| &ret.errors) {
        self.plugin_driver.render_error(&HookRenderErrorArgs { error }).await?;
      }

      bundle_output?
    };

    // Add additional files from build plugins.
    self.file_emitter.add_additional_files(&mut output.assets);

    self.plugin_driver.generate_bundle(&mut output.assets, is_write).await?;

    Ok(output)
  }

  fn normalize_error<T>(
    ret: &Result<T>,
    errors_fn: impl Fn(&T) -> &[BuildDiagnostic],
  ) -> Option<String> {
    ret.as_ref().map_or_else(
      |error| Some(error.to_string()),
      |ret| errors_fn(ret).first().map(ToString::to_string),
    )
  }

  pub fn options(&self) -> &NormalizedBundlerOptions {
    &self.options
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
