use super::stages::{link_stage::LinkStage, scan_stage::NormalizedScanStageOutput};
use crate::{
  BundlerOptions, SharedOptions, SharedResolver,
  bundler_builder::BundlerBuilder,
  hmr::hmr_manager::{HmrManager, HmrManagerInput},
  stages::{
    generate_stage::GenerateStage,
    scan_stage::{ScanStage, ScanStageOutput},
  },
  types::{bundle_output::BundleOutput, scan_stage_cache::ScanStageCache},
};
use anyhow::Result;

use arcstr::ArcStr;
use rolldown_common::{
  GetLocalDbMut, HmrOutput, Module, NormalizedBundlerOptions, ScanMode, SharedFileEmitter,
  SymbolRefDb,
};
use rolldown_debug::{action, trace_action};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{
  __inner::SharedPluginable, HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver,
};
use rolldown_utils::dashmap::FxDashSet;
use std::{any::Any, sync::Arc};
use tracing::Instrument;

pub struct Bundler {
  pub closed: bool,
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) warnings: Vec<BuildDiagnostic>,
  pub(crate) _log_guard: Option<Box<dyn Any + Send>>,
  #[allow(unused)]
  pub(crate) cache: ScanStageCache,
  pub(crate) hmr_manager: Option<HmrManager>,
  pub(crate) session_span: tracing::Span,
  // Guard for the tracing system. Responsible for cleaning up the allocated resources when the bundler gets dropped.
  pub(crate) _debug_tracer: Option<rolldown_debug::DebugTracer>,
  pub(crate) build_count: u32,
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
  #[tracing::instrument(level = "debug", skip_all, parent = &self.session_span)]
  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    let build_count = self.inc_build_count();
    async {
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan(vec![]).await?;

      let ret = self.bundle_write(scan_stage_output).await;
      trace_action!(action::BuildEnd { action: "BuildEnd" });
      ret
    }
    .instrument(tracing::info_span!(
      "write",
      CONTEXT_build_id = &*rolldown_debug::generate_build_id(build_count)
    ))
    .await
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.session_span)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    let build_count = self.inc_build_count();
    async {
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan(vec![]).await?;

      let ret = self.bundle_up(scan_stage_output, /* is_write */ false).await.map(|mut output| {
        output.warnings.append(&mut self.warnings);
        output
      });
      trace_action!(action::BuildEnd { action: "BuildEnd" });
      ret
    }
    .instrument(tracing::info_span!(
      "generate",
      CONTEXT_build_id = &*rolldown_debug::generate_build_id(build_count)
    ))
    .await
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

  // The rollup always crate a new build at watch mode, it cloud be call multiply times.
  // Here only reset the closed flag to make it possible to call again.
  pub fn reset_closed(&mut self) {
    self.closed = false;
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn scan(&mut self, changed_ids: Vec<ArcStr>) -> BuildResult<NormalizedScanStageOutput> {
    trace_action!(action::BuildStart { action: "BuildStart" });
    let mode =
      if !self.options.experimental.is_incremental_build_enabled() || changed_ids.is_empty() {
        ScanMode::Full
      } else {
        ScanMode::Partial(changed_ids)
      };
    let is_full_scan_mode = mode.is_full();

    // Make sure the cache is reset if incremental build is not enabled.
    let mut scan_stage_cache_guard = CacheGuard {
      is_incremental_build_enabled: self.options.experimental.is_incremental_build_enabled(),
      cache: &mut self.cache,
    };

    let scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
      self.session_span.clone(),
    )
    .scan(mode, scan_stage_cache_guard.inner())
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

    // Manually drop it to avoid holding the mut reference.
    drop(scan_stage_cache_guard);

    let scan_stage_output =
      self.normalize_scan_stage_output_and_update_cache(scan_stage_output, is_full_scan_mode);

    Self::trace_action_module_graph_ready(&scan_stage_output);
    self.plugin_driver.build_end(None).await?;
    trace_action!(action::BuildEnd { action: "BuildEnd" });
    Ok(scan_stage_output)
  }

  pub fn normalize_scan_stage_output_and_update_cache(
    &mut self,
    output: ScanStageOutput,
    is_full_scan_mode: bool,
  ) -> NormalizedScanStageOutput {
    if !self.options.experimental.is_incremental_build_enabled() {
      return output.into();
    }

    if is_full_scan_mode {
      let output: NormalizedScanStageOutput = output.into();
      self.cache.set_snapshot(output.make_copy());
      output
    } else {
      self.cache.merge(output);
      self.cache.create_output()
    }
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
      }
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

    if let Some(invalidate_js_side_cache) = &self.options.invalidate_js_side_cache {
      invalidate_js_side_cache.call().await?;
    }

    self.merge_immutable_fields_for_cache(link_stage_output.symbol_db);

    if self.options.is_hmr_enabled() {
      self.hmr_manager = Some(HmrManager::new(HmrManagerInput {
        module_db: link_stage_output.module_table,
        fs: self.fs,
        options: Arc::clone(&self.options),
        resolver: Arc::clone(&self.resolver),
        plugin_driver: Arc::clone(&self.plugin_driver),
        index_ecma_ast: link_stage_output.ast_table,
        // Don't forget to reset the cache if you want to rebuild the bundle instead hmr.
        cache: std::mem::take(&mut self.cache),
        session_span: self.session_span.clone(),
      }));
    }
    Ok(output)
  }

  #[inline]
  pub fn options(&self) -> &NormalizedBundlerOptions {
    &self.options
  }

  pub fn get_watch_files(&self) -> &Arc<FxDashSet<ArcStr>> {
    &self.plugin_driver.watch_files
  }

  pub async fn generate_hmr_patch(&mut self, changed_files: Vec<String>) -> BuildResult<HmrOutput> {
    self.hmr_manager.as_mut().expect("HMR manager is not initialized").hmr(changed_files).await
  }

  pub async fn hmr_invalidate(
    &mut self,
    file: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<HmrOutput> {
    self
      .hmr_manager
      .as_mut()
      .expect("HMR manager is not initialized")
      .hmr_invalidate(file, first_invalidated_by)
      .await
  }

  fn merge_immutable_fields_for_cache(&mut self, symbol_db: SymbolRefDb) {
    if !self.options.experimental.is_incremental_build_enabled() {
      return;
    }
    let snapshot = self.cache.get_snapshot_mut();
    for (idx, symbol_ref_db) in symbol_db.into_inner().into_iter_enumerated() {
      let Some(db_for_module) = symbol_ref_db else {
        continue;
      };
      let cache_db = snapshot.symbol_ref_db.local_db_mut(idx);
      let (scoping, _) = db_for_module.ast_scopes.into_inner();
      cache_db.ast_scopes.set_scoping(scoping);
    }
  }

  fn trace_action_module_graph_ready(scan_stage_output: &NormalizedScanStageOutput) {
    if tracing::enabled!(tracing::Level::TRACE) {
      let modules = scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|m| match m {
          Module::Normal(module) => action::Module {
            id: module.id.to_string(),
            is_external: false,
            imports: Some(
              module
                .import_records
                .iter()
                .map(|r| action::ModuleImport {
                  id: scan_stage_output.module_table[r.resolved_module].id().to_string(),
                  kind: r.kind.to_string(),
                  module_request: r.module_request.to_string(),
                })
                .collect(),
            ),
            importers: Some(module.importers.iter().map(|i| i.to_string()).collect()),
          },
          Module::External(module) => action::Module {
            id: module.id.to_string(),
            is_external: true,
            imports: None,
            importers: None,
          },
        })
        .collect();
      trace_action!(action::ModuleGraphReady { action: "ModuleGraphReady", modules });
    }
  }

  fn inc_build_count(&mut self) -> u32 {
    let count = self.build_count;
    self.build_count += 1;
    count
  }
}

struct CacheGuard<'a> {
  is_incremental_build_enabled: bool,
  cache: &'a mut ScanStageCache,
}
impl CacheGuard<'_> {
  pub fn inner(&mut self) -> &mut ScanStageCache {
    self.cache
  }
}

impl Drop for CacheGuard<'_> {
  fn drop(&mut self) {
    if !self.is_incremental_build_enabled {
      std::mem::take(self.cache);
    }
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
