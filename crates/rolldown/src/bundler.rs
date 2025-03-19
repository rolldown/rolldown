use super::stages::{link_stage::LinkStage, scan_stage::ScanStageOutput};
use crate::{
  BundlerOptions, SharedOptions, SharedResolver,
  bundler_builder::BundlerBuilder,
  hmr::hmr_manager::{HmrManager, HmrManagerInput},
  stages::{generate_stage::GenerateStage, scan_stage::ScanStage},
  types::bundle_output::BundleOutput,
};
use anyhow::Result;

use rolldown_common::{Cache, NormalizedBundlerOptions, SharedFileEmitter};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{
  __inner::SharedPluginable, HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver,
};
use std::sync::Arc;
use tracing_chrome::FlushGuard;

pub struct Bundler {
  pub closed: bool,
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) warnings: Vec<BuildDiagnostic>,
  pub(crate) _log_guard: Option<FlushGuard>,
  #[allow(unused)]
  pub(crate) cache: Arc<Cache>,
  pub(crate) hmr_manager: Option<HmrManager>,
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
  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    let scan_stage_output = self.scan().await?;

    self.bundle_write(scan_stage_output).await
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    let scan_stage_output = self.scan().await?;

    self.bundle_up(scan_stage_output, /* is_write */ false).await.map(|mut output| {
      output.warnings.append(&mut self.warnings);
      output
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    self.closed = true;
    self.plugin_driver.close_bundle().await?;

    Ok(())
  }

  pub async fn scan(&mut self) -> BuildResult<ScanStageOutput> {
    let scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
      Arc::clone(&self.cache),
    )
    .scan()
    .await
    {
      Ok(v) => v,
      Err(errs) => {
        self
          .plugin_driver
          .build_end(Some(&HookBuildEndArgs { errors: &errs, cwd: &self.options.cwd }))
          .await?;
        self.plugin_driver.close_bundle().await?;
        return Err(errs);
      }
    };

    self.plugin_driver.build_end(None).await?;

    Ok(scan_stage_output)
  }

  pub async fn bundle_write(
    &mut self,
    scan_stage_output: ScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let mut output = self.bundle_up(scan_stage_output, /* is_write */ true).await?;

    let dist_dir = self.options.cwd.join(&self.options.out_dir);

    self.fs.create_dir_all(&dist_dir).map_err(|err| {
      anyhow::anyhow!("Could not create directory for output chunks: {:?}", dist_dir).context(err)
    })?;

    for chunk in &output.assets {
      let dest = dist_dir.join(chunk.filename());
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

    self
      .plugin_driver
      .write_bundle(&mut output.assets, &self.options, &mut output.warnings)
      .await?;

    output.warnings.append(&mut self.warnings);

    Ok(output)
  }

  #[allow(clippy::missing_transmute_annotations, clippy::needless_pass_by_ref_mut)]
  async fn bundle_up(
    &mut self,
    scan_stage_output: ScanStageOutput,
    is_write: bool,
  ) -> BuildResult<BundleOutput> {
    if self.closed {
      return Err(
        anyhow::anyhow!(
          "Bundle is already closed, no more calls to 'generate' or 'write' are allowed."
        )
        .into(),
      );
    }

    let mut link_stage_output = LinkStage::new(scan_stage_output, &self.options).link();

    let bundle_output =
      GenerateStage::new(&mut link_stage_output, &self.options, &self.plugin_driver)
        .generate()
        .await; // Notice we don't use `?` to break the control flow here.

    if let Err(errors) = &bundle_output {
      self
        .plugin_driver
        .render_error(&HookRenderErrorArgs { errors, cwd: &self.options.cwd })
        .await?;
    }

    let mut output = bundle_output?;

    // Add additional files from build plugins.
    self.file_emitter.add_additional_files(&mut output.assets, &mut output.warnings);

    self
      .plugin_driver
      .generate_bundle(&mut output.assets, is_write, &self.options, &mut output.warnings)
      .await?;

    output.watch_files = self.plugin_driver.watch_files.iter().map(|f| f.clone()).collect();

    if self.options.is_hmr_enabled() {
      self.hmr_manager = Some(HmrManager::new(HmrManagerInput {
        module_db: link_stage_output.module_table,
        fs: self.fs,
        options: Arc::clone(&self.options),
        resolver: Arc::clone(&self.resolver),
        plugin_driver: Arc::clone(&self.plugin_driver),
        cache: Arc::clone(&self.cache),
      }));
    }
    Ok(output)
  }

  #[inline]
  pub fn options(&self) -> &NormalizedBundlerOptions {
    &self.options
  }

  pub async fn generate_hmr_patch(&mut self, changed_files: Vec<String>) -> BuildResult<String> {
    self
      .hmr_manager
      .as_ref()
      .expect("HMR manager is not initialized")
      .generate_hmr_patch(changed_files)
      .await
  }
}

fn _test_bundler() {
  #[allow(clippy::needless_pass_by_value)]
  fn assert_send(_foo: impl Send) {}
  let mut bundler = Bundler::new(BundlerOptions::default());
  let write_fut = bundler.write();
  assert_send(write_fut);
  let mut bundler = Bundler::new(BundlerOptions::default());
  let generate_fut = bundler.generate();
  assert_send(generate_fut);
}
