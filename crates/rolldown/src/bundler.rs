use super::stages::{link_stage::LinkStage, scan_stage::ScanStageOutput};
use crate::{
  bundler_builder::BundlerBuilder,
  stages::{
    generate_stage::GenerateStage,
    scan_stage::{NormalizedScanStageOutput, ScanStage},
  },
  types::{bundle_output::BundleOutput, scan_stage_cache::ScanStageCache},
  BundlerOptions, SharedOptions, SharedResolver,
};
use anyhow::Result;

use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, NormalizedBundlerOptions, ScanMode, SharedFileEmitter};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{
  HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver, __inner::SharedPluginable,
};
use rustc_hash::FxHashMap;
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
  pub(crate) scan_stage_cache: Arc<ScanStageCache>,
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
    let scan_stage_output = self.scan(vec![]).await?;

    self.bundle_write(scan_stage_output).await
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    let scan_stage_output = self.scan(vec![]).await?;

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

  pub async fn scan(&mut self, changed_ids: Vec<ArcStr>) -> BuildResult<NormalizedScanStageOutput> {
    let mode =
      if !self.options.experimental.is_incremental_build_enabled() || changed_ids.is_empty() {
        ScanMode::Full
      } else {
        ScanMode::Partial(changed_ids)
      };
    let is_full_scan_mode = mode.is_full();
    let cache_mut = Arc::get_mut(&mut self.scan_stage_cache).unwrap();
    let module_id_to_idx = std::mem::take(&mut cache_mut.module_id_to_idx);
    let (scan_stage_output, module_id_to_idx) = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
    )
    .scan(mode, module_id_to_idx)
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

    let scan_stage_output =
      self.update_scan_stage_cache(scan_stage_output, module_id_to_idx, is_full_scan_mode);

    self.plugin_driver.build_end(None).await?;
    Ok(scan_stage_output)
  }

  pub fn update_scan_stage_cache(
    &mut self,
    output: ScanStageOutput,
    module_id_to_idx: FxHashMap<ArcStr, ModuleIdx>,
    is_full_scan_mode: bool,
  ) -> NormalizedScanStageOutput {
    if !self.options.experimental.is_incremental_build_enabled() {
      return output.into();
    }
    let scan_stage_cache = Arc::get_mut(&mut self.scan_stage_cache).unwrap();

    scan_stage_cache.set_module_id_to_idx(module_id_to_idx);

    let output = if is_full_scan_mode {
      let output: NormalizedScanStageOutput = output.into();
      scan_stage_cache.set_cache(output.make_copy());
      // for (id, idx) in scan_stage_cache.module_id_to_idx() {
      //   dbg!(&id, idx);
      // }
      output
    } else {
      scan_stage_cache.merge(output);
      scan_stage_cache.create_output()
    };
    output
  }

  pub async fn bundle_write(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
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

    self.plugin_driver.write_bundle(&mut output.assets, &self.options).await?;

    output.warnings.append(&mut self.warnings);

    Ok(output)
  }

  #[allow(clippy::missing_transmute_annotations)]
  async fn bundle_up(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
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

    if self.options.experimental.is_incremental_build_enabled() {
      let scan_stage_cache = Arc::get_mut(&mut self.scan_stage_cache).unwrap();
      scan_stage_cache.set_ast_scopes(link_stage_output.ast_scope_table);
    }

    if let Err(errs) = &bundle_output {
      self
        .plugin_driver
        .render_error(&HookRenderErrorArgs { errors: errs, cwd: &self.options.cwd })
        .await?;
    }

    let mut output = bundle_output?;

    // Add additional files from build plugins.
    self.file_emitter.add_additional_files(&mut output.assets);

    self.plugin_driver.generate_bundle(&mut output.assets, is_write, &self.options).await?;

    output.watch_files = self.plugin_driver.watch_files.iter().map(|f| f.clone()).collect();

    Ok(output)
  }

  pub fn options(&self) -> &NormalizedBundlerOptions {
    &self.options
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
