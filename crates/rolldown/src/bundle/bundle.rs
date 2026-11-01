use crate::bundle::bundle_handle::BundleHandle;

use super::super::{
  SharedOptions, SharedResolver,
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
use rolldown_common::{GetLocalDb, GetLocalDbMut, Module, ScanMode, SharedFileEmitter, SymbolRefDb};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_plugin::{
  HookBuildEndArgs, HookCloseBundleArgs, HookRenderErrorArgs, SharedPluginDriver,
};
use rolldown_utils::dashmap::FxDashSet;
use std::{path::Path, sync::Arc};
use sugar_path::SugarPath;

/// How many modules fast native relinks can append before the cached link tables grow.
const APPENDED_MODULES_HEADROOM: usize = 64;

#[expect(
  clippy::struct_field_names,
  reason = "`bundle_span` emphasizes this's a span for this bundle, not a session level span"
)]
pub struct Bundle<Fs: FileSystem + Clone + 'static = OsFileSystem> {
  pub(crate) fs: Fs,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver<Fs>,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) warnings: Vec<BuildDiagnostic>,
  pub(crate) cache: ScanStageCache,
  pub(crate) bundle_span: tracing::Span,
}

impl<Fs: FileSystem + Clone + 'static> Bundle<Fs> {
  /// Consume a successfully generated and closed bundle, retaining only its scan.
  /// Never retain this result after a failed, cancelled or rejected transaction.
  pub fn into_revision_scan_cache(mut self) -> crate::RevisionScanCache {
    self.cache.invalidate_native_relink();
    crate::RevisionScanCache(self.cache)
  }

  pub fn contains_leaf(&self, id: &str) -> bool {
    self.cache.contains_leaf(id)
  }

  pub fn can_partial_rescan(&self, id: &str) -> bool {
    self.cache.can_partial_rescan(id)
  }

  pub fn module_import_signature(
    &self,
    id: &str,
  ) -> Option<crate::types::scan_stage_cache::ModuleImportSignature> {
    self.cache.module_import_signature(id)
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  /// This method intentionally get the ownership of `self` to show that the method cannot be called multiple times.
  pub async fn write(mut self) -> BuildResult<BundleOutput> {
    let start = self.plugin_driver.build_timings.start();
    let result = async {
      self.trace_action_session_meta();
      trace_action!(action::BuildStart { action: "BuildStart" });
      let scan_stage_output = self.scan_modules(ScanMode::Full).await?;

      let ret = self.bundle_write(scan_stage_output).await;
      trace_action!(action::BuildEnd { action: "BuildEnd" });
      ret
    }
    .await;
    self.plugin_driver.build_timings.record_total(start);
    result
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  /// This method intentionally get the ownership of `self` to show that the method cannot be called multiple times.
  pub async fn generate(mut self) -> BuildResult<BundleOutput> {
    let start = self.plugin_driver.build_timings.start();
    let result = async {
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
    .await;
    self.plugin_driver.build_timings.record_total(start);
    result
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  /// This method intentionally get the ownership of `self` to show that the method cannot be called multiple times.
  pub async fn scan(mut self) -> BuildResult<()> {
    let mut scan_stage_output = self.scan_modules(ScanMode::Full).await?;
    self.merge_immutable_fields_for_cache(
      scan_stage_output.symbol_ref_db,
      scan_stage_output.index_ecma_ast,
      &mut scan_stage_output.module_table,
    );

    Ok(())
  }

  pub fn cache_snapshot(&self) -> Option<&NormalizedScanStageOutput> {
    self.cache.snapshot()
  }

  pub fn set_sourcemap_content_filter(
    &mut self,
    filter: Option<crate::types::scan_stage_cache::SourcemapContentFilter>,
  ) {
    self.cache.sourcemap_content_filter =
      filter.map(crate::types::scan_stage_cache::SourcemapContentFilterHolder);
    self.cache.pre_redacted_maps.clear();
  }

  pub fn take_pre_redacted_maps(&mut self) -> rustc_hash::FxHashSet<ArcStr> {
    std::mem::take(&mut self.cache.pre_redacted_maps)
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  pub async fn scan_modules_for_cache(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<Vec<BuildDiagnostic>> {
    self.plugin_driver.file_emitter.enter_build_phase();
    let result = self.scan_modules_for_cache_impl(scan_mode).await;
    self.plugin_driver.file_emitter.enter_output_phase();
    result
  }

  async fn scan_modules_for_cache_impl(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<Vec<BuildDiagnostic>> {
    trace_action!(action::BuildStart { action: "BuildStart" });
    let is_full_scan_mode = scan_mode.is_full();

    let mut scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs.clone(),
      Arc::clone(&self.resolver),
    )
    .scan(scan_mode, &mut self.cache)
    .await
    {
      Ok(v) => v,
      Err(errs) => {
        debug_assert!(errs.iter().all(|e| e.severity() == Severity::Error));
        self
          .plugin_driver
          .build_end(Some(&HookBuildEndArgs { errors: &errs, cwd: &self.options.cwd }))
          .await?;
        self
          .plugin_driver
          .close_bundle(Some(&HookCloseBundleArgs { errors: &errs, cwd: &self.options.cwd }))
          .await?;
        return Err(errs);
      }
    };

    let warnings = if is_full_scan_mode {
      let mut output: NormalizedScanStageOutput = scan_stage_output
        .try_into()
        .expect("Should be able to convert to NormalizedScanStageOutput");
      let warnings = std::mem::take(&mut output.warnings);
      let result =
        defer_sync_scan_data(&self.options, &self.cache.module_id_to_idx, &mut output).await;
      if self.options.experimental.is_incremental_build_enabled() {
        self.cache.set_snapshot(output);
      }
      result?;
      warnings
    } else {
      let warnings = std::mem::take(&mut scan_stage_output.warnings);
      self.cache.merge(scan_stage_output, &self.plugin_driver)?;
      self.cache.update_defer_sync_data(&self.options).await?;
      warnings
    };

    if let Some(snapshot) = self.cache.snapshot() {
      Self::trace_action_module_graph_ready(snapshot);
    }
    if let Err(errs) = self.plugin_driver.build_end(None).await {
      return Err(errs.into());
    }
    trace_action!(action::BuildEnd { action: "BuildEnd" });
    Ok(warnings)
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  pub async fn scan_modules(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<NormalizedScanStageOutput> {
    // The whole scan (buildStart through buildEnd) is the build phase. Resetting on every exit
    // means a failed scan cannot leave the emitter stuck in the build phase, where a later emit
    // (e.g. an HMR patch after a failed rebuild) could still change an already-emitted filename.
    self.plugin_driver.file_emitter.enter_build_phase();
    let result = self.scan_modules_impl(scan_mode).await;
    self.plugin_driver.file_emitter.enter_output_phase();
    result
  }

  async fn scan_modules_impl(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<NormalizedScanStageOutput> {
    trace_action!(action::BuildStart { action: "BuildStart" });
    let is_full_scan_mode = scan_mode.is_full();

    let scan_stage_output = match ScanStage::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs.clone(),
      Arc::clone(&self.resolver),
    )
    .scan(scan_mode, &mut self.cache)
    .await
    {
      Ok(v) => v,
      Err(errs) => {
        debug_assert!(errs.iter().all(|e| e.severity() == Severity::Error));
        self
          .plugin_driver
          .build_end(Some(&HookBuildEndArgs { errors: &errs, cwd: &self.options.cwd }))
          .await?;
        self
          .plugin_driver
          .close_bundle(Some(&HookCloseBundleArgs { errors: &errs, cwd: &self.options.cwd }))
          .await?;
        return Err(errs);
      }
    };

    let mut scan_stage_output = self
      .normalize_scan_stage_output_and_update_cache(scan_stage_output, is_full_scan_mode)
      .await?;

    // Make sure the cache is reset if incremental build is not enabled.
    if !self.options.experimental.is_incremental_build_enabled() && is_full_scan_mode {
      std::mem::take(&mut self.cache);
    }

    Self::trace_action_module_graph_ready(&scan_stage_output);
    if let Err(errs) = self.plugin_driver.build_end(None).await {
      self.merge_immutable_fields_for_cache(
        scan_stage_output.symbol_ref_db,
        scan_stage_output.index_ecma_ast,
        &mut scan_stage_output.module_table,
      );
      return Err(errs.into());
    }
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
      closed: Arc::default(),
    }
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  pub async fn bundle_write(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let result = self.bundle_write_impl(scan_stage_output).await;
    self.invalidate_js_side_cache(result).await
  }

  async fn bundle_write_impl(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let dist_dir = self.options.cwd.join(&self.options.out_dir);

    if self.options.clean_dir && self.options.file.is_none() {
      if let Err(err) = clean_dir(&self.fs, &dist_dir) {
        self.warnings.push(
          BuildDiagnostic::could_not_clean_directory(
            dist_dir.display().to_string(),
            err.to_string(),
          )
          .with_severity_warning(),
        );
      }
    }

    let mut output = self.bundle_up(scan_stage_output, /* is_write */ true).await?;

    self.fs.create_dir_all(&dist_dir).with_context(|| {
      format!("Could not create directory for output chunks: {}", dist_dir.display())
    })?;

    for chunk in &output.assets {
      let filename = chunk.filename();
      if filename.contains('\0') {
        let pattern_name = match chunk {
          rolldown_common::Output::Chunk(c) => {
            if c.is_entry {
              "output.entryFileNames"
            } else {
              "output.chunkFileNames"
            }
          }
          rolldown_common::Output::Asset(_) => "output.assetFileNames",
        };
        return Err(
          BuildDiagnostic::invalid_option(rolldown_error::InvalidOptionType::NulByteInFilename {
            pattern_name: pattern_name.to_string(),
          })
          .into(),
        );
      }
      let dest = dist_dir.join(filename);
      if let Some(p) = dest.parent() {
        if !self.fs.exists(p) {
          self.fs.create_dir_all(p).with_context(|| {
            format!("Could not create directory for output chunks: {}", p.display())
          })?;
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

  #[tracing::instrument(level = "debug", skip_all, parent = &self.bundle_span)]
  pub async fn bundle_generate(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let result = self.bundle_up(scan_stage_output, false).await;
    self.invalidate_js_side_cache(result).await
  }

  /// A plugin may read `chunk.code` in `generateBundle` or `writeBundle` before a later hook fails, so the
  /// JS side must drop its per-build caches on every exit of the output stage, not only on success.
  async fn invalidate_js_side_cache(
    &mut self,
    result: BuildResult<BundleOutput>,
  ) -> BuildResult<BundleOutput> {
    let Some(invalidate_js_side_cache) = &self.options.invalidate_js_side_cache else {
      return result;
    };
    match (result, invalidate_js_side_cache.call().await) {
      (Ok(output), Ok(())) => Ok(output),
      // The build error is the one to report.
      (Err(errors), _) => Err(errors),
      (Ok(_), Err(err)) => {
        self.cache.invalidate_native_relink();
        Err(err.into())
      }
    }
  }

  #[tracing::instrument(level = "debug", skip(self, output))]
  async fn normalize_scan_stage_output_and_update_cache(
    &mut self,
    mut output: ScanStageOutput,
    is_full_scan_mode: bool,
  ) -> BuildResult<NormalizedScanStageOutput> {
    let is_incremental = self.options.experimental.is_incremental_build_enabled();

    if is_full_scan_mode {
      let mut output: NormalizedScanStageOutput =
        output.try_into().expect("Should be able to convert to NormalizedScanStageOutput");
      // The scan produced a complete `output`; `defer_sync_scan_data` only
      // mutates per-module `side_effects` best-effort, so its error doesn't
      // invalidate `output`. Commit it rather than fall back to the stale prior
      // snapshot.
      // See internal-docs/bundler-data-lifecycle/implementation.md ("Cache integrity on a failed build").
      let result =
        defer_sync_scan_data(&self.options, &self.cache.module_id_to_idx, &mut output).await;
      if is_incremental {
        self.cache.set_snapshot(output.make_copy());
      }
      if let Err(errs) = result {
        self.merge_immutable_fields_for_cache(
          output.symbol_ref_db,
          output.index_ecma_ast,
          &mut output.module_table,
        );
        return Err(errs);
      }
      return Ok(output);
    }

    let warnings = std::mem::take(&mut output.warnings);
    self.cache.merge(output, &self.plugin_driver)?;
    self.cache.update_defer_sync_data(&self.options).await?;
    if self.cache.has_valid_native_relink() && !trace_action_enabled!() {
      let mut out = self.cache.create_fast_native_relink_placeholder_output();
      out.warnings = warnings;
      return Ok(out);
    }
    let mut output = self.cache.create_output();
    output.warnings = warnings;
    Ok(output)
  }

  async fn bundle_up(
    &mut self,
    mut scan_stage_output: NormalizedScanStageOutput,
    is_write: bool,
  ) -> BuildResult<BundleOutput> {
    self.cache.last_fast_native_relink = false;
    self.cache.last_native_relink_fallback = None;
    self.cache.last_native_relink_fallback_detail = None;
    self.cache.last_rebuilt_chunk_filenames.clear();
    self.cache.last_rebuilt_chunk_outputs.clear();
    if self.cache.has_valid_native_relink() && !trace_action_enabled!() {
      if let Some(fast_result) =
        Box::pin(self.try_bundle_up_fast_native_relink(is_write, &mut scan_stage_output.warnings))
          .await
      {
        return fast_result;
      }
      self.cache.invalidate_native_relink();
      if scan_stage_output.module_table.modules.is_empty() {
        let warnings = std::mem::take(&mut scan_stage_output.warnings);
        scan_stage_output = self.cache.create_output();
        scan_stage_output.warnings = warnings;
      }
    } else {
      self.cache.last_native_relink_fallback = Some(if !self.cache.has_snapshot() {
        "no_snapshot"
      } else if self.cache.cached_native_relink.is_none() {
        "no_cached_link"
      } else {
        "trace_action"
      });
    }
    if scan_stage_output.index_ecma_ast.is_empty()
      && self.cache.has_snapshot()
    {
      scan_stage_output.index_ecma_ast =
        std::mem::take(&mut self.cache.get_snapshot_mut().index_ecma_ast);
    }
    // The one stretch of a build with no plugin in it, which is what makes it a usable
    // baseline for "was this build plugin-bound?" — see `BuildTimings`.
    let link_start = self.plugin_driver.build_timings.start();
    struct LinkUnwindGuard<'a>(&'a mut ScanStageCache, bool);
    impl Drop for LinkUnwindGuard<'_> {
      fn drop(&mut self) {
        if !self.1 {
          std::mem::take(self.0);
        }
      }
    }
    let mut link_guard = LinkUnwindGuard(&mut self.cache, false);
    let (mut link_stage_output, ast_table, used_symbol_refs_builder) =
      LinkStage::new(scan_stage_output, &self.options).link();
    link_guard.1 = true;
    drop(link_guard);
    self.plugin_driver.build_timings.record_link_stage(link_start);

    let is_incremental =
      self.options.experimental.is_incremental_build_enabled() && self.cache.has_snapshot();
    let mut generate_stage = GenerateStage::new_with_cache(
      &mut link_stage_output,
      ast_table,
      &self.options,
      &self.plugin_driver,
      is_incremental.then_some(&mut self.cache),
    );
    let bundle_output = Box::pin(generate_stage.generate(used_symbol_refs_builder)).await; // Notice we don't use `?` to break the control flow here.
    let ast_table = generate_stage.take_restored_ast_table();
    let pending_native_relink = generate_stage.pending_native_relink.take();
    drop(generate_stage);
    if bundle_output.is_ok() {
      self.cache.inlined_commonjs_constants =
        Some(crate::stages::link_stage::inlined_commonjs_constants(&link_stage_output));
      self.cache.split_json_defaults =
        Some(crate::stages::link_stage::split_json_defaults(&link_stage_output));
      self.cache.commonjs_evaluations =
        Some(crate::utils::chunk::render_chunk_exports::commonjs_evaluations(
          &link_stage_output,
          &self.options,
        ));
      self.cache.commonjs_interop =
        Some(crate::stages::link_stage::commonjs_interop(&link_stage_output));
    }

    if bundle_output.is_ok()
      && is_incremental
      && let Some(pending) = pending_native_relink
    {
      let snapshot = self.cache.get_snapshot_mut();
      if !ast_table.is_empty() {
        snapshot.index_ecma_ast = ast_table;
      }
      for (snap_mod, build_mod) in snapshot
        .module_table
        .modules
        .iter_mut()
        .zip(link_stage_output.module_table.modules.iter_mut())
      {
        if let (Some(snap_normal), Some(build_normal)) =
          (snap_mod.as_normal_mut(), build_mod.as_normal_mut())
          && !build_normal.sourcemap_chain.is_empty()
        {
          snap_normal.sourcemap_chain = std::mem::take(&mut build_normal.sourcemap_chain);
        }
      }
      let scanned_side_effects =
        snapshot.module_table.modules.iter().map(|module| *module.side_effects()).collect();
      self.cache.dirty_native_modules.clear();
      // Fast relinks reuse the link output for the rest of the session.
      if !self.cache.keep_unused_capacity {
        crate::utils::retained_memory::shrink_link_tables(&mut link_stage_output);
      }
      // Room for the modules that fast relinks append, which then move no table.
      link_stage_output.module_table.modules.reserve(APPENDED_MODULES_HEADROOM);
      link_stage_output.metas.reserve(APPENDED_MODULES_HEADROOM);
      link_stage_output.stmt_infos.reserve(APPENDED_MODULES_HEADROOM);
      self.cache.cached_native_relink =
        Some(Box::new(crate::types::scan_stage_cache::CachedNativeRelinkState {
          link_output: link_stage_output,
          generate_state: *pending,
          scanned_side_effects,
        }));
    } else if is_incremental {
      for (idx, orig_db) in std::mem::take(&mut link_stage_output.pre_lazy_export_symbols) {
        *link_stage_output.symbol_db.local_db_mut(idx) = orig_db;
      }
      // `create_output`/`make_copy` strip symbol-table scoping from the cache for
      // performance; reinstate it here, before the fallible steps below, so the
      // cache stays whole on their `Err` paths.
      // See internal-docs/bundler-data-lifecycle/implementation.md ("Cache integrity on a failed build").
      self.merge_immutable_fields_for_cache(
        std::mem::take(&mut link_stage_output.symbol_db),
        ast_table,
        &mut link_stage_output.module_table,
      );

      // `link_stage_output` is dead from here on (its `symbol_db` was just taken
      // for the cache merge); ship the remaining heavy fields (module_table,
      // metas, stmt_infos, ...) to a rayon worker so their free() happens off
      // the critical path.
      crate::utils::defer_drop::spawn_drop(link_stage_output);
    } else {
      std::mem::take(&mut self.cache);
      crate::utils::defer_drop::spawn_drop((link_stage_output, ast_table));
    }

    if let Err(errors) = &bundle_output {
      self.cache.invalidate_native_relink();
      debug_assert!(errors.iter().all(|e| e.severity() == Severity::Error));
      self
        .plugin_driver
        .render_error(&HookRenderErrorArgs { errors, cwd: &self.options.cwd })
        .await?;
    }

    let mut output = bundle_output?;
    let pre_emit_assets_len = output.assets.len();

    // Add additional files from build plugins.
    self.file_emitter.add_additional_files(&mut output.assets, &mut output.warnings);

    if let Err(err) = self
      .plugin_driver
      .generate_bundle(&mut output.assets, is_write, &self.options, &mut output.warnings)
      .await
    {
      self.cache.invalidate_native_relink();
      return Err(err.into());
    }

    if output.assets.len() != pre_emit_assets_len {
      self.cache.invalidate_native_relink();
    }

    if let Err(errors) =
      GenerateStage::validate_mangle_properties_output(&self.options, &output.assets)
    {
      self.cache.invalidate_native_relink();
      self
        .plugin_driver
        .render_error(&HookRenderErrorArgs { errors: &errors, cwd: &self.options.cwd })
        .await?;
      return Err(errors);
    }

    for asset in &output.assets {
      if self.cache.last_reused_chunk_filenames.contains(asset.filename()) {
        continue;
      }
      if is_filename_outside_output_dir(asset.filename()) {
        self.cache.invalidate_native_relink();
        return Err(
          vec![BuildDiagnostic::filename_outside_output_directory(asset.filename().to_string())]
            .into(),
        );
      }
    }

    Ok(output)
  }

  #[expect(clippy::too_many_lines)]
  async fn try_bundle_up_fast_native_relink(
    &mut self,
    is_write: bool,
    scan_warnings: &mut Vec<BuildDiagnostic>,
  ) -> Option<BuildResult<BundleOutput>> {
    if !self.options.preserve_modules
      || !crate::utils::native_records::enabled(&self.options)
      || !self.cache.omit_reused_chunk_bytes
      || !self.options.experimental.is_incremental_build_enabled()
      || !self.cache.has_snapshot()
      || self.options.banner.is_some()
      || self.options.footer.is_some()
      || self.options.intro.is_some()
      || self.options.outro.is_some()
      || self.options.post_banner.is_some()
      || self.options.post_footer.is_some()
      || self.options.experimental.is_attach_debug_info_full()
      || self.options.sourcemap_debug_ids
      || self.options.paths.is_some()
      || self.options.minify.is_enabled()
      || self.options.optimization.inline_const_pass() > 1
    {
      self.cache.last_native_relink_fallback = Some("options");
      return None;
    }

    let Some(mut cached_relink) = self.cache.cached_native_relink.take() else {
      self.cache.last_native_relink_fallback = Some("no_cached_link");
      return None;
    };
    super::native_relink::take_rejection();
    super::native_relink::take_rejection_detail();
    let prepared = self.try_prepare_fast_native_relink(&mut cached_relink);
    let Some(super::native_relink::PreparedFastNativeRelink {
      modules: prepared,
      graph,
      unreachable,
      rerendered,
    }) = prepared
    else {
      self.cache.last_native_relink_fallback =
        Some(super::native_relink::take_rejection().unwrap_or("unlabeled"));
      self.cache.last_native_relink_fallback_detail = super::native_relink::take_rejection_detail();
      self.cache.cached_native_relink = Some(cached_relink);
      return None;
    };

    let link_start = self.plugin_driver.build_timings.start();
    let mut dirty_modules_with_used = Vec::with_capacity(prepared.len());
    let mut renames = Vec::new();
    let mut cjs_interops = Vec::new();
    let mut edges_changed = Vec::new();
    {
      let snapshot = self.cache.get_snapshot_mut();
      // The relink links the scans of the changed and appended modules from now on.
      cached_relink.scanned_side_effects.resize(
        snapshot.module_table.modules.len(),
        rolldown_common::side_effects::DeterminedSideEffects::NoTreeshake,
      );
      for item in prepared {
        let idx = item.idx;
        let chunk_idx = item.chunk_idx;
        cached_relink.scanned_side_effects[idx] = *snapshot.module_table[idx].side_effects();
        let mut local_db_without_scoping = rolldown_common::SymbolRefDbForModule::new(
          oxc::semantic::Scoping::default(),
          idx,
          oxc::semantic::ScopeId::new(0),
        );
        local_db_without_scoping.classic_data =
          snapshot.symbol_ref_db.local_db(idx).classic_data.clone();
        local_db_without_scoping.flags = snapshot.symbol_ref_db.local_db(idx).flags.clone();
        let mut full_local_db = std::mem::replace(
          snapshot.symbol_ref_db.local_db_mut(idx),
          local_db_without_scoping,
        );
        full_local_db.classic_data = item.new_local_db.classic_data;
        full_local_db.flags = item.new_local_db.flags;
        if let Some((wrapper, name)) = &item.wrapper {
          let created = full_local_db.ast_scopes.create_facade_root_symbol_ref(name);
          debug_assert_eq!(created, *wrapper, "the wrapper follows the scanned symbols");
        }
        for (sym_id, name) in item.cjs_namespace_renames {
          full_local_db.ast_scopes.set_symbol_name(sym_id, &name);
        }
        for (stmt_idx, stmt_info) in item.new_stmt_infos.iter_enumerated() {
          if item.stmt_info_included.has_bit(stmt_idx) {
            for declared in &stmt_info.declared_symbols {
              if declared.inner().owner == idx {
                full_local_db.classic_data[declared.inner().symbol].chunk_idx = Some(chunk_idx);
              }
            }
          }
        }
        cached_relink.link_output.symbol_db[idx] = Some(full_local_db);

        let mut new_mod = snapshot.module_table[idx].clone();
        if let Some(new_n) = new_mod.as_normal_mut() {
          new_n.exec_order = item.exec_order;
          new_n.exports_kind = item.post_link_exports_kind;
          new_n.side_effects = item.post_link_side_effects;
          new_n.constant_export_map.clear();
        }
        cached_relink.link_output.module_table[idx] = new_mod;
        cached_relink.link_output.stmt_infos[idx] = item.new_stmt_infos;
        let meta = &mut cached_relink.link_output.metas[idx];
        meta.stmt_info_included = item.stmt_info_included;
        meta.resolved_member_expr_refs = item.new_resolved_member_expr_refs;
        meta.folded_namespace_in_checks = item.new_folded_namespace_in_checks;
        meta.depended_runtime_helper = item.depended_runtime_helper;
        meta.native_default_expression = item.native_default_expression;
        if item.added {
          let retained = meta
            .resolved_exports
            .values()
            .map(|export| export.symbol_ref)
            .filter(|symbol| item.local_used_symbols.contains(symbol))
            .collect::<Vec<_>>();
          for symbol in retained {
            cached_relink.link_output.retained_export_symbols.insert(symbol);
          }
        } else if item.edges_changed {
          edges_changed.push((idx, item.edge_targets));
        }
        if !item.renamed.is_empty() {
          renames.push((idx, item.renamed));
        }
        if let Some(cjs_interop) = item.cjs_interop {
          cjs_interops.push((idx, cjs_interop));
        }
        cached_relink.generate_state.used_overrides.insert(idx, item.local_used_symbols.clone());
        dirty_modules_with_used.push((idx, chunk_idx, item.local_used_symbols));
      }
      // A changed module that the updated graph does not reach renders nothing. Its scan,
      // scope tree included, replaces the cached link data, so that invalidating the cached
      // output merges the scan back into the snapshot.
      for idx in unreachable {
        let mut scanned_without_scoping = rolldown_common::SymbolRefDbForModule::new(
          oxc::semantic::Scoping::default(),
          idx,
          oxc::semantic::ScopeId::new(0),
        );
        scanned_without_scoping.classic_data =
          snapshot.symbol_ref_db.local_db(idx).classic_data.clone();
        scanned_without_scoping.flags = snapshot.symbol_ref_db.local_db(idx).flags.clone();
        let scanned =
          std::mem::replace(snapshot.symbol_ref_db.local_db_mut(idx), scanned_without_scoping);
        cached_relink.link_output.symbol_db[idx] = Some(scanned);
        cached_relink.scanned_side_effects[idx] = *snapshot.module_table[idx].side_effects();
        let mut module = snapshot.module_table[idx].clone();
        if let Some(module) = module.as_normal_mut() {
          module.exec_order = u32::MAX;
        }
        cached_relink.link_output.module_table[idx] = module;
        cached_relink.link_output.stmt_infos[idx] = snapshot.stmt_infos[idx].clone();
        cached_relink.link_output.metas[idx] =
          crate::types::linking_metadata::LinkingMetadata::default();
        cached_relink.generate_state.used_overrides.remove(&idx);
      }
      // A record that the update only makes start or stop calling its module's wrapper as it
      // evaluates renders again from its cached link data, with the symbols that the cached link
      // uses (#684), and with the maps of its module's load and transform hooks, which caching the
      // link moved to the snapshot, as a changed module's record renders with its scan's.
      for (idx, chunk_idx) in rerendered {
        if let (Some(cached_module), Some(scanned)) = (
          cached_relink.link_output.module_table[idx].as_normal_mut(),
          snapshot.module_table[idx].as_normal(),
        ) {
          cached_module.sourcemap_chain.clone_from(&scanned.sourcemap_chain);
        }
        let used = cached_relink.generate_state.used_overrides.get(&idx).cloned().unwrap_or_else(
          || {
            let count = cached_relink.link_output.symbol_db[idx]
              .as_ref()
              .map_or(0, |db| db.classic_data.len());
            (0..count)
              .map(|symbol| {
                rolldown_common::SymbolRef::from((idx, oxc::semantic::SymbolId::from_usize(symbol)))
              })
              .filter(|symbol| cached_relink.generate_state.used_symbol_refs.contains(symbol))
              .collect()
          },
        );
        dirty_modules_with_used.push((idx, chunk_idx, used));
      }
      self.cache.dirty_native_modules.clear();
      self.cache.added_native_modules.clear();
      self.cache.native_importer_changes.clear();
    }
    let Some(snapshot) = self.cache.snapshot() else {
      self.cache.last_native_relink_fallback = Some("no_snapshot");
      return None;
    };
    let mut graph_change = None;
    let retired: rustc_hash::FxHashSet<rolldown_common::ModuleIdx> =
      graph.as_ref().map(|plan| plan.retired.iter().copied().collect()).unwrap_or_default();
    let added: rustc_hash::FxHashSet<rolldown_common::ModuleIdx> =
      graph.as_ref().map(|plan| plan.added.iter().copied().collect()).unwrap_or_default();
    if let Some(plan) = &graph {
      let link_output = &mut cached_relink.link_output;
      for (idx, module) in link_output.module_table.modules.iter_mut_enumerated() {
        match module {
          Module::Normal(module) => module.exec_order = plan.exec_orders[idx],
          Module::External(module) => module.exec_order = plan.exec_orders[idx],
        }
      }
      link_output.sorted_modules.clone_from(&plan.sorted_modules);
      link_output.unreachable_modules.clone_from(&plan.unreachable);
      for &idx in &plan.retired {
        link_output.metas[idx].is_included = false;
      }
      for &idx in plan.importer_changes.iter().chain(&plan.retired) {
        if let (Some(scanned), Some(module)) = (
          snapshot.module_table.modules[idx].as_normal(),
          link_output.module_table.modules[idx].as_normal_mut(),
        ) {
          module.ecma_view.importers.clone_from(&scanned.ecma_view.importers);
          module.ecma_view.importers_idx.clone_from(&scanned.ecma_view.importers_idx);
          module.ecma_view.dynamic_importers.clone_from(&scanned.ecma_view.dynamic_importers);
          module
            .ecma_view
            .dynamic_importers_idx
            .clone_from(&scanned.ecma_view.dynamic_importers_idx);
        }
      }
      graph_change = Some(crate::stages::generate_stage::NativeChunkChange {
        added: plan.added.iter().map(|idx| (*idx, plan.added_chunks[idx])).collect(),
        retired: plan.retired.clone(),
        edges_changed,
        sorted_modules: plan.sorted_modules.clone(),
        reordered: plan.reordered,
      });
    }
    for (idx, renamed) in &renames {
      super::native_relink::apply_native_renames(
        &mut cached_relink,
        snapshot,
        *idx,
        renamed,
        &retired,
        &added,
      );
    }
    for (idx, (named_imports, record_namespaces)) in cjs_interops {
      let meta = &mut cached_relink.link_output.metas[idx];
      meta.named_import_to_cjs_module = named_imports;
      meta.import_record_ns_to_cjs_module = record_namespaces;
    }
    self.plugin_driver.build_timings.record_link_stage(link_start);

    let ast_table = std::mem::take(&mut self.cache.get_snapshot_mut().index_ecma_ast);
    let mut generate_stage = GenerateStage::new_with_cache(
      &mut cached_relink.link_output,
      ast_table,
      &self.options,
      &self.plugin_driver,
      Some(&mut self.cache),
    );
    let bundle_output = Box::pin(generate_stage.generate_fast_native_relink(
      &mut cached_relink.generate_state,
      &dirty_modules_with_used,
      graph_change.as_ref(),
    ))
    .await;
    let ast_table = generate_stage.take_restored_ast_table();
    drop(generate_stage);
    if !ast_table.is_empty() && self.cache.has_snapshot() {
      self.cache.get_snapshot_mut().index_ecma_ast = ast_table;
    }
    self.cache.cached_native_relink = Some(cached_relink);
    let (bundle_output, retired_filenames) = match bundle_output {
      Ok(Ok((output, retired_filenames))) => (Ok(output), retired_filenames),
      // The graph change cannot be rendered alone; link the whole graph.
      Ok(Err(reason)) => {
        self.cache.last_native_relink_fallback = Some(reason);
        return None;
      }
      Err(errors) => (Err(errors), Vec::new()),
    };
    let native_graph_change = graph.map(|plan| {
      let snapshot = self.cache.get_snapshot();
      let id_of = |idx: &rolldown_common::ModuleIdx| {
        snapshot.module_table.modules[*idx].id().as_arc_str().clone()
      };
      crate::types::scan_stage_cache::NativeGraphChange {
        added: plan.added.iter().map(id_of).collect(),
        retired: plan.retired.iter().map(id_of).collect(),
        retired_filenames,
      }
    });

    if let Err(errors) = &bundle_output {
      self.cache.invalidate_native_relink();
      debug_assert!(errors.iter().all(|e| e.severity() == Severity::Error));
      if let Err(hook_err) = self
        .plugin_driver
        .render_error(&HookRenderErrorArgs { errors, cwd: &self.options.cwd })
        .await
      {
        return Some(Err(hook_err.into()));
      }
      return Some(bundle_output);
    }

    let mut output = match bundle_output {
      Ok(v) => v,
      Err(errs) => return Some(Err(errs)),
    };
    self.cache.inlined_commonjs_constants = self
      .cache
      .cached_native_relink
      .as_ref()
      .map(|cached| crate::stages::link_stage::inlined_commonjs_constants(&cached.link_output));
    self.cache.split_json_defaults = self
      .cache
      .cached_native_relink
      .as_ref()
      .map(|cached| crate::stages::link_stage::split_json_defaults(&cached.link_output));
    self.cache.commonjs_evaluations = self.cache.cached_native_relink.as_ref().map(|cached| {
      crate::utils::chunk::render_chunk_exports::commonjs_evaluations(
        &cached.link_output,
        &self.options,
      )
    });
    self.cache.commonjs_interop = self
      .cache
      .cached_native_relink
      .as_ref()
      .map(|cached| crate::stages::link_stage::commonjs_interop(&cached.link_output));
    if !scan_warnings.is_empty() {
      let mut combined = std::mem::take(scan_warnings);
      combined.append(&mut output.warnings);
      output.warnings = combined;
    }
    let pre_emit_assets_len = output.assets.len();

    self.file_emitter.add_additional_files(&mut output.assets, &mut output.warnings);

    if let Err(err) = self
      .plugin_driver
      .generate_bundle(&mut output.assets, is_write, &self.options, &mut output.warnings)
      .await
    {
      self.cache.invalidate_native_relink();
      return Some(Err(err.into()));
    }

    if output.assets.len() != pre_emit_assets_len {
      self.cache.invalidate_native_relink();
    }

    if let Err(errors) =
      GenerateStage::validate_mangle_properties_output(&self.options, &output.assets)
    {
      self.cache.invalidate_native_relink();
      if let Err(hook_err) = self
        .plugin_driver
        .render_error(&HookRenderErrorArgs { errors: &errors, cwd: &self.options.cwd })
        .await
      {
        return Some(Err(hook_err.into()));
      }
      return Some(Err(errors));
    }

    for asset in &output.assets {
      if self.cache.last_reused_chunk_filenames.contains(asset.filename()) {
        continue;
      }
      if is_filename_outside_output_dir(asset.filename()) {
        self.cache.invalidate_native_relink();
        return Some(Err(
          vec![BuildDiagnostic::filename_outside_output_directory(asset.filename().to_string())]
            .into(),
        ));
      }
    }

    self.cache.last_fast_native_relink = output.assets.len() == pre_emit_assets_len;
    self.cache.last_native_graph_change = native_graph_change;
    Some(Ok(output))
  }

  pub fn restore_cache_from_scan(&mut self, mut scan_stage_output: NormalizedScanStageOutput) {
    self.merge_immutable_fields_for_cache(
      scan_stage_output.symbol_ref_db,
      scan_stage_output.index_ecma_ast,
      &mut scan_stage_output.module_table,
    );
  }

  fn merge_immutable_fields_for_cache(
    &mut self,
    symbol_db: SymbolRefDb,
    ast_table: crate::type_alias::IndexEcmaAst,
    module_table: &mut rolldown_common::ModuleTable,
  ) {
    if !self.options.experimental.is_incremental_build_enabled() || !self.cache.has_snapshot() {
      return;
    }
    let snapshot = self.cache.get_snapshot_mut();
    if !ast_table.is_empty() {
      snapshot.index_ecma_ast = ast_table;
    }
    for (snap_mod, build_mod) in snapshot
      .module_table
      .modules
      .iter_mut()
      .zip(module_table.modules.iter_mut())
    {
      if let (Some(snap_normal), Some(build_normal)) =
        (snap_mod.as_normal_mut(), build_mod.as_normal_mut())
        && !build_normal.sourcemap_chain.is_empty()
      {
        snap_normal.sourcemap_chain = std::mem::take(&mut build_normal.sourcemap_chain);
      }
    }
    for (idx, symbol_ref_db) in symbol_db.into_inner().into_iter_enumerated() {
      let Some(db_for_module) = symbol_ref_db else {
        continue;
      };
      let cache_db = snapshot.symbol_ref_db.local_db_mut(idx);
      cache_db.merge_from_build(db_for_module);
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
                .filter_map(|r| {
                  r.resolved_module.map(|module_idx| action::ModuleImport {
                    module_id: scan_stage_output.module_table[module_idx].id().to_string(),
                    kind: r.kind.to_string(),
                    module_request: r.module_request.to_string(),
                  })
                })
                .collect(),
            ),
            importers: Some(module.importers.iter().map(ToString::to_string).collect()),
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

/// Check if a filename would escape the output directory.
///
/// Rejects paths rooted in POSIX or Windows syntax and relative paths that
/// normalize outside the output directory or to the directory itself.
fn is_filename_outside_output_dir(filename: &str) -> bool {
  let bytes = filename.as_bytes();
  let starts_with_path_root = bytes.first().is_some_and(|byte| matches!(*byte, b'/' | b'\\'));
  let has_windows_drive_prefix =
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
  // Drive-relative paths can escape only on Windows; absolute drive paths are
  // rejected on every host for cross-platform consistency.
  let has_disallowed_windows_drive_prefix = has_windows_drive_prefix
    && (cfg!(windows) || bytes.get(2).is_some_and(|byte| matches!(*byte, b'/' | b'\\')));
  if starts_with_path_root || has_disallowed_windows_drive_prefix {
    return true;
  }

  let normalized = filename.normalize();

  normalized == Path::new(".")
    || normalized.starts_with(Path::new(".."))
    // Preserve the existing cross-platform guard for a leading Windows parent.
    || normalized.to_string_lossy().starts_with("..\\")
}

#[cfg(test)]
mod tests {
  use super::is_filename_outside_output_dir;

  #[test]
  fn output_filename_cannot_resolve_to_or_escape_output_directory() {
    for filename in [
      "",
      ".",
      "./",
      "foo/..",
      "foo/../",
      "foo/bar/../..",
      "..",
      "../file",
      "..\\file",
      "a/../../file",
      "/file",
      "\\file",
      "C:/file",
      "C:\\file",
      "c:\\file",
      "\\\\server\\share\\file",
      "\\\\?\\C:\\file",
    ] {
      assert!(is_filename_outside_output_dir(filename), "expected {filename:?} to be rejected");
    }

    for filename in [
      "file.js",
      "dir/file.js",
      "dir\\file.js",
      "./asset.txt",
      ".\\asset.txt",
      "foo/",
      "foo\\",
      "..file.js",
      ".hidden",
      "foo/bar/../baz.js",
    ] {
      assert!(!is_filename_outside_output_dir(filename), "expected {filename:?} to be accepted");
    }
  }

  #[cfg(windows)]
  #[test]
  fn windows_output_filename_cannot_use_drive_relative_or_backslash_traversal() {
    for filename in [".\\", "C:", "C:file", "c:file", "foo\\..", "a\\..\\..\\file"] {
      assert!(is_filename_outside_output_dir(filename), "expected {filename:?} to be rejected");
    }
  }

  #[cfg(not(windows))]
  #[test]
  fn unix_output_filename_keeps_host_native_backslash_and_colon_semantics() {
    for filename in [".\\", "C:", "C:file", "c:file", "foo\\..", "a\\..\\..\\file"] {
      assert!(!is_filename_outside_output_dir(filename), "expected {filename:?} to be accepted");
    }
  }
}
