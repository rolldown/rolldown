use crate::bundle::bundle_handle::BundleHandle;

use super::super::{
  SharedOptions, SharedResolver,
  bundler::CacheGuard,
  module_loader::deferred_scan_data::defer_sync_scan_data,
  stages::{
    generate_stage::GenerateStage,
    link_stage::LinkStage,
    scan_stage::{NormalizedScanStageOutput, ScanStage, ScanStageOutput},
  },
  types::{bundle_output::BundleOutput, scan_stage_cache::ScanStageCache},
  utils::fs_utils::clean_dir,
};
use anyhow::Context;
use arcstr::ArcStr;
use rolldown_common::{GetLocalDbMut, Module, ScanMode, SharedFileEmitter, SymbolRefDb};
use rolldown_debug::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{HookBuildEndArgs, HookRenderErrorArgs, SharedPluginDriver};
use rolldown_utils::dashmap::FxDashSet;
use std::sync::Arc;

#[expect(
  clippy::struct_field_names,
  reason = "`bundle_span` emphasizes this's a span for this bundle, not a session level span"
)]
pub struct Bundle {
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) warnings: Vec<BuildDiagnostic>,
  pub(crate) cache: ScanStageCache,
  pub(crate) bundle_span: Arc<tracing::Span>,
}

impl Bundle {
  #[tracing::instrument(level = "debug", skip_all, parent = &*self.bundle_span)]
  /// This method intentionally get the ownership of `self` to show that the method cannot be called multiple times.
  pub async fn write(mut self) -> BuildResult<BundleOutput> {
    async {
      self.trace_action_session_meta();
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan_modules(ScanMode::Full).await?;

      let ret = self.bundle_write(scan_stage_output).await;
      trace_action!(action::BuildEnd { action: "BuildEnd" });
      ret
    }
    .await
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &*self.bundle_span)]
  /// This method intentionally get the ownership of `self` to show that the method cannot be called multiple times.
  pub async fn generate(mut self) -> BuildResult<BundleOutput> {
    async {
      self.trace_action_session_meta();
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan_modules(ScanMode::Full).await?;

      let ret = self.bundle_generate(scan_stage_output).await.map(|mut output| {
        output.warnings.append(&mut self.warnings);
        output
      });
      trace_action!(action::BuildEnd { action: "BuildEnd" });
      ret
    }
    .await
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &*self.bundle_span)]
  /// This method intentionally get the ownership of `self` to show that the method cannot be called multiple times.
  pub async fn scan(mut self) -> BuildResult<()> {
    self.scan_modules(ScanMode::Full).await?;

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &*self.bundle_span)]
  pub(crate) async fn scan_modules(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<NormalizedScanStageOutput> {
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

    let scan_stage_output = self
      .normalize_scan_stage_output_and_update_cache(scan_stage_output, is_full_scan_mode)
      .await?;

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

  pub fn context(&self) -> BundleHandle {
    BundleHandle {
      options: Arc::clone(&self.options),
      plugin_driver: Arc::clone(&self.plugin_driver),
    }
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &*self.bundle_span)]
  pub async fn bundle_write(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let dist_dir = self.options.cwd.join(&self.options.out_dir);

    if self.options.clean_dir && self.options.dir.is_some() {
      clean_dir(&self.fs, &dist_dir).with_context(|| {
        format!("Could not clean directory for output chunks: {}", dist_dir.display())
      })?;
    }

    let mut output = self.bundle_up(scan_stage_output, /* is_write */ true).await?;

    self.fs.create_dir_all(&dist_dir).with_context(|| {
      format!("Could not create directory for output chunks: {}", dist_dir.display())
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
        .with_context(|| format!("Failed to write file in {}", dest.display()))?;
    }

    self
      .plugin_driver
      .write_bundle(&mut output.assets, &self.options, &mut output.warnings)
      .await?;

    output.warnings.append(&mut self.warnings);

    Ok(output)
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &*self.bundle_span)]
  pub(crate) async fn bundle_generate(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    self.bundle_up(scan_stage_output, false).await
  }

  #[tracing::instrument(level = "debug", skip(self, output))]
  async fn normalize_scan_stage_output_and_update_cache(
    &mut self,
    output: ScanStageOutput,
    is_full_scan_mode: bool,
  ) -> BuildResult<NormalizedScanStageOutput> {
    if !self.options.experimental.is_incremental_build_enabled() {
      return Ok(
        output.try_into().expect("Should be able to convert to NormalizedScanStageOutput"),
      );
    }

    if is_full_scan_mode {
      let mut output: NormalizedScanStageOutput =
        output.try_into().expect("Should be able to convert to NormalizedScanStageOutput");
      self.defer_sync_scan_data(&mut output).await?;
      self.cache.set_snapshot(output.make_copy());
      Ok(output)
    } else {
      self.cache.merge(output)?;
      self.cache.update_defer_sync_data(&self.options, &self.resolver).await?;
      Ok(self.cache.create_output())
    }
  }

  #[expect(clippy::needless_pass_by_ref_mut, reason = "Required for Send bound across await")]
  async fn defer_sync_scan_data(
    &mut self,
    scan_stage_output: &mut NormalizedScanStageOutput,
  ) -> BuildResult<()> {
    defer_sync_scan_data(
      &self.options,
      &self.cache.module_id_to_idx,
      &self.resolver,
      scan_stage_output,
    )
    .await
  }

  async fn bundle_up(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
    is_write: bool,
  ) -> BuildResult<BundleOutput> {
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
