use super::stages::{link_stage::LinkStage, scan_stage::NormalizedScanStageOutput};
use crate::{
  BundlerOptions, SharedOptions, SharedResolver,
  bundler_builder::BundlerBuilder,
  hmr::hmr_stage::{HmrStage, HmrStageInput},
  stages::{
    generate_stage::GenerateStage,
    scan_stage::{ScanStage, ScanStageOutput},
  },
  types::{bundle_output::BundleOutput, scan_stage_cache::ScanStageCache},
};
use anyhow::Result;
use arcstr::ArcStr;
use rolldown_common::{
  ClientHmrInput, ClientHmrUpdate, GetLocalDbMut, HmrUpdate, Module, ScanMode, SharedFileEmitter,
  SymbolRefDb,
};
use rolldown_debug::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};
use rolldown_fs::{FileSystem, FileSystemUtils, OsFileSystem};
use rolldown_plugin::{
  __inner::SharedPluginable, HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver,
};
use rolldown_utils::dashmap::FxDashSet;
use rustc_hash::FxHashSet;
use std::{
  any::Any,
  sync::{Arc, atomic::AtomicU32},
};
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
  pub(crate) cache: ScanStageCache,
  pub(crate) session: rolldown_debug::Session,
  pub(crate) build_count: u32,
}

impl Bundler {
  // --- Public API ---

  pub fn new(options: BundlerOptions) -> BuildResult<Self> {
    BundlerBuilder::default().with_options(options).build()
  }

  pub fn with_plugins(
    options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
  ) -> BuildResult<Self> {
    BundlerBuilder::default().with_options(options).with_plugins(plugins).build()
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    let build_count = self.build_count;
    async {
      self.trace_action_session_meta();
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan(ScanMode::Full).await?;

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

  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    let build_count = self.build_count;
    async {
      self.trace_action_session_meta();
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan(ScanMode::Full).await?;

      let ret = self.bundle_generate(scan_stage_output).await.map(|mut output| {
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
    self.inner_close().await
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn scan(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<NormalizedScanStageOutput> {
    self.create_error_if_closed()?;
    trace_action!(action::BuildStart { action: "BuildStart" });
    let is_full_scan_mode = scan_mode.is_full();

    // Make sure the cache is reset if incremental build is not enabled.
    let mut scan_stage_cache_guard = CacheGuard {
      is_incremental_build_enabled: self.options.experimental.is_incremental_build_enabled(),
      cache: &mut self.cache,
    };

    let scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs.clone(),
      Arc::clone(&self.resolver),
    )
    .scan(scan_mode, scan_stage_cache_guard.inner())
    .await
    {
      Ok(v) => v,
      Err(errs) => {
        debug_assert!(errs.iter().all(|e| e.severity() == Severity::Error));
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

  #[inline]
  pub fn options(&self) -> &SharedOptions {
    &self.options
  }

  pub fn get_watch_files(&self) -> &Arc<FxDashSet<ArcStr>> {
    &self.plugin_driver.watch_files
  }

  // --- Internal API ---

  fn create_error_if_closed(&self) -> BuildResult<()> {
    if self.closed {
      Err(anyhow::anyhow!("Bundler is closed"))?;
    }
    Ok(())
  }

  // The rollup always crate a new build at watch mode, it cloud be call multiply times.
  // Here only reset the closed flag to make it possible to call again.
  pub(crate) fn reset_closed_for_watch_mode(&mut self) {
    self.closed = false;
  }

  pub(crate) async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &[String],
    clients: &[ClientHmrInput],
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.fs.clone(),
      options: Arc::clone(&self.options),
      resolver: Arc::clone(&self.resolver),
      plugin_driver: Arc::clone(&self.plugin_driver),
      cache: &mut self.cache,
      next_hmr_patch_id,
    });
    hmr_stage.compute_hmr_update_for_file_changes(changed_file_paths, clients).await
  }

  pub(crate) async fn compute_update_for_calling_invalidate(
    &mut self,
    invalidate_caller: String,
    first_invalidated_by: Option<String>,
    executed_modules: &FxHashSet<String>,
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> BuildResult<HmrUpdate> {
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.fs.clone(),
      options: Arc::clone(&self.options),
      resolver: Arc::clone(&self.resolver),
      plugin_driver: Arc::clone(&self.plugin_driver),
      cache: &mut self.cache,
      next_hmr_patch_id,
    });
    hmr_stage
      .compute_update_for_calling_invalidate(
        invalidate_caller,
        first_invalidated_by,
        executed_modules,
      )
      .await
  }

  pub(crate) fn take_cache(&mut self) -> ScanStageCache {
    std::mem::take(&mut self.cache)
  }

  pub(crate) fn set_cache(&mut self, cache: ScanStageCache) {
    self.cache = cache;
  }

  pub(crate) async fn bundle_write(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let mut output = self.bundle_up(scan_stage_output, /* is_write */ true).await?;

    let dist_dir = self.options.cwd.join(&self.options.out_dir);

    if self.options.clean_dir && self.options.dir.is_some() {
      self.fs.clean_dir(&dist_dir).expect("cannot clean out dir");
    }

    self.fs.create_dir_all(&dist_dir).map_err(|err| {
      anyhow::anyhow!("Could not create directory for output chunks: {}", dist_dir.display())
        .context(err)
    })?;

    for chunk in &output.assets {
      let dest = dist_dir.join(chunk.filename());
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).unwrap();
        }
      }
      self.fs.write(&dest, chunk.content_as_bytes()).map_err(|err| {
        anyhow::anyhow!("Failed to write file in {}", dest.display()).context(err)
      })?;
    }

    self
      .plugin_driver
      .write_bundle(&mut output.assets, &self.options, &mut output.warnings)
      .await?;

    output.warnings.append(&mut self.warnings);

    Ok(output)
  }

  pub(crate) async fn bundle_generate(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    self.bundle_up(scan_stage_output, false).await
  }

  #[tracing::instrument(level = "debug", skip(self, output))]
  fn normalize_scan_stage_output_and_update_cache(
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

  async fn inner_close(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    self.closed = true;
    self.plugin_driver.close_bundle().await?;

    // Clean up resources
    self.plugin_driver.clear();
    self.cache = ScanStageCache::default();
    self.resolver.clear_cache();
    self.warnings.clear();

    Ok(())
  }

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
      debug_assert!(errors.iter().all(|e| e.severity() == Severity::Error));
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

    Ok(output)
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
    if trace_action_enabled!() {
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
                  module_id: scan_stage_output.module_table[r.resolved_module].id().to_string(),
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

  fn trace_action_session_meta(&self) {
    if trace_action_enabled!() {
      trace_action!(action::SessionMeta {
        action: "SessionMeta",
        inputs: self
          .options
          .input
          .iter()
          .map(|v| action::InputItem { name: v.name.clone(), filename: v.import.clone() })
          .collect(),
        plugins: self
          .plugin_driver
          .plugins()
          .iter()
          .enumerate()
          .map(|(idx, p)| action::PluginItem {
            name: p.call_name().into_owned(),
            plugin_id: idx.try_into().unwrap()
          })
          .collect(),
        cwd: self.options.cwd.to_string_lossy().to_string(),
        platform: self.options.platform.to_string(),
        format: self.options.format.to_string(),
        dir: self.options.dir.clone(),
        file: self.options.file.clone(),
      });
    }
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
  fn assert_send(_foo: impl Send) {}
  let mut bundler = Bundler::new(BundlerOptions::default()).expect("Failed to create bundler");
  let write_fut = bundler.write();
  assert_send(write_fut);
  let mut bundler = Bundler::new(BundlerOptions::default()).expect("Failed to create bundler");
  let generate_fut = bundler.generate();
  assert_send(generate_fut);
}
