use std::{sync::Arc, thread};

use arcstr::ArcStr;
use futures::future::join_all;
use oxc::span::Span;
use oxc_index::IndexVec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_common::SourceMapGenMsg;
use rolldown_common::{
  EntryPoint, FlatOptions, HybridIndexVec, Module, ModuleId, ModuleIdx, ModuleTable,
  PreserveEntrySignatures, ResolvedId, RuntimeModuleBrief, ScanMode, StmtInfos, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage, side_effects::HookSideEffects,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, SharedResolver,
  module_loader::{ModuleLoader, module_loader::ModuleLoaderOutput},
  persistent_cache::SharedPersistentScanCache,
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::scan_stage_cache::ScanStageCache,
  utils::load_entry_module::load_entry_module,
};

type SourcemapChannel =
  (Option<std::sync::mpsc::Sender<SourceMapGenMsg>>, Option<thread::JoinHandle<()>>);

pub struct ScanStage<Fs: FileSystem + Clone + 'static> {
  options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: Fs,
  resolver: SharedResolver<Fs>,
  persistent_cache: Option<SharedPersistentScanCache>,
}

#[derive(Debug)]
pub struct NormalizedScanStageOutput {
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  /// Per-module `StmtInfos` side table, parallel to `module_table.modules`.
  /// External modules get an empty `StmtInfos::new()` placeholder. Routed
  /// directly into `LinkStage.stmt_infos` instead of living on `EcmaView`.
  pub stmt_infos: IndexStmtInfos,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  // TODO: merge the preserve_entry_signatures_map in incremental build
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
}

impl NormalizedScanStageOutput {
  /// Make a snapshot of the current ScanStage, skipping clone some fields that is immutable in
  /// following stage
  pub fn make_copy(&self) -> Self {
    Self {
      module_table: self.module_table.clone(),
      index_ecma_ast: {
        let iter = self.index_ecma_ast.raw.par_iter();

        let index_ecma_ast = iter
          .map(|ast| ast.as_ref().map(rolldown_ecmascript::EcmaAst::clone_with_another_arena))
          .collect::<Vec<_>>();
        IndexVec::from_vec(index_ecma_ast)
      },
      stmt_infos: self.stmt_infos.clone(),
      entry_points: self.entry_points.clone(),
      symbol_ref_db: self.symbol_ref_db.clone_without_scoping(),
      runtime: self.runtime.clone(),
      warnings: vec![],
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map.clone(),
      overrode_preserve_entry_signature_map: self.overrode_preserve_entry_signature_map.clone(),
      entry_point_to_reference_ids: self.entry_point_to_reference_ids.clone(),
      flat_options: self.flat_options,
      user_defined_entry_modules: self.user_defined_entry_modules.clone(),
      tla_module_count: self.tla_module_count,
      tla_keyword_span_map: self.tla_keyword_span_map.clone(),
    }
  }
}

impl TryFrom<ScanStageOutput> for NormalizedScanStageOutput {
  type Error = &'static str;

  fn try_from(value: ScanStageOutput) -> Result<Self, Self::Error> {
    let module_table = match value.module_table {
      HybridIndexVec::IndexVec(modules) => ModuleTable { modules },
      HybridIndexVec::Map(_) => return Err("module_table must be normalized to IndexVec first"),
    };

    let index_ecma_ast = match value.index_ecma_ast {
      HybridIndexVec::IndexVec(ast) => ast,
      HybridIndexVec::Map(_) => return Err("index_ecma_ast must be normalized to IndexVec first"),
    };

    let stmt_infos = match value.stmt_infos {
      HybridIndexVec::IndexVec(stmt_infos) => stmt_infos,
      HybridIndexVec::Map(_) => return Err("stmt_infos must be normalized to IndexVec first"),
    };

    Ok(Self {
      module_table,
      index_ecma_ast,
      stmt_infos,
      entry_points: value.entry_points,
      symbol_ref_db: value.symbol_ref_db,
      runtime: value.runtime,
      warnings: value.warnings,
      dynamic_import_exports_usage_map: value.dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map: value.overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: value.entry_point_to_reference_ids,
      flat_options: value.flat_options,
      user_defined_entry_modules: value.user_defined_entry_modules,
      tla_module_count: value.tla_module_count,
      tla_keyword_span_map: value.tla_keyword_span_map,
    })
  }
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: HybridIndexVec<ModuleIdx, Module>,
  pub index_ecma_ast: HybridIndexVec<ModuleIdx, Option<EcmaAst>>,
  pub stmt_infos: HybridIndexVec<ModuleIdx, StmtInfos>,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
}

impl<Fs: FileSystem + Clone + 'static> ScanStage<Fs> {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: Fs,
    resolver: SharedResolver<Fs>,
    persistent_cache: Option<SharedPersistentScanCache>,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver, persistent_cache }
  }

  /// Each changed module drops the `moduleSideEffects` that plugins wrote, and `getModuleInfo`
  /// shows its start value again, as for its JS options: the module will be fetched from the
  /// resolution that created it (`ModuleLoader::try_spawn_new_task`).
  fn reset_module_info_side_effects(&self, changed_ids: &[ArcStr], cache: &ScanStageCache) {
    for changed_id in changed_ids {
      self.plugin_driver.module_side_effects_overrides.remove(changed_id);
    }
    let Some(snapshot) = cache.snapshot() else {
      return;
    };
    for changed_id in changed_ids {
      let Some(Module::Normal(module)) = cache
        .module_id_to_idx
        .get(&ModuleId::new(changed_id.as_str()))
        .and_then(|state| snapshot.module_table.modules.get(state.idx()))
      else {
        continue;
      };
      self.plugin_driver.set_module_info_side_effects(
        &module.id,
        module.originative_resolved_id.side_effects.map(HookSideEffects::declared),
      );
    }
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn scan(
    &self,
    mode: ScanMode<ArcStr>,
    cache: &mut ScanStageCache,
  ) -> BuildResult<ScanStageOutput> {
    let fetch_mode = match mode {
      ScanMode::Full => ScanMode::Full,
      ScanMode::Partial(changed_ids) => {
        // Reset the changed modules before they are resolved again below, so `resolveId` hooks
        // do not see the options from their last fetch. `fetch_modules` resets them again after
        // `buildStart`.
        if let Some(reset_js_module_options) = &self.options.reset_js_module_options {
          reset_js_module_options
            .call(Some(changed_ids.iter().map(ToString::to_string).collect()))
            .await?;
        }
        self.reset_module_info_side_effects(&changed_ids, cache);
        ScanMode::Partial(self.resolve_absolute_path(&changed_ids).await?)
      }
    };
    let (tx_clone, handler) = self.create_sourcemap_channel();

    let cache_session = match &self.persistent_cache {
      Some(persistent_cache) => {
        Some(persistent_cache.start_scan(&self.fs, fetch_mode.is_full(), &self.options).await)
      }
      None => None,
    };

    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      cache,
      fetch_mode.is_full(),
      tx_clone,
      cache_session,
    )?;

    // For `pluginContext.emitFile` with `type: chunk`, support it at buildStart hook.
    self
      .plugin_driver
      .file_emitter
      .set_context_load_modules_tx(Some(module_loader.shared_context.tx.clone()))?;

    self.plugin_driver.build_start(&self.options).await?;

    // For `await pluginContext.load`, if support it at buildStart hook, it could be caused stuck.
    self
      .plugin_driver
      .set_context_load_modules_tx(Some(module_loader.shared_context.tx.clone()))?;

    let fetch_result = module_loader.fetch_modules(fetch_mode).await;
    // The results saved by modules that finished are valid also when the scan failed.
    if let Some(persistent_cache) = &self.persistent_cache {
      persistent_cache.start_flush();
    }
    let module_loader_output = fetch_result?;

    if let Some(handler) = handler {
      // The maps already sit in each module's sourcemap chain, in plugin
      // order. Joining only makes sure they are all generated.
      handler.join().expect("SourceMapGen: sourcemap worker thread panicked");
    }

    self.plugin_driver.file_emitter.set_context_load_modules_tx(None)?;

    self.plugin_driver.set_context_load_modules_tx(None)?;

    Ok(module_loader_output.into())
  }

  fn create_sourcemap_channel(&self) -> SourcemapChannel {
    if self.options.experimental.is_native_magic_string_enabled()
      && self.options.is_sourcemap_enabled()
    {
      let (tx, rx) = std::sync::mpsc::channel::<SourceMapGenMsg>();
      let handler = thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
          match msg {
            SourceMapGenMsg::MagicString(pending) => {
              // Generates the map now, off the JS thread, so it's ready at render time.
              pending.get();
            }
            SourceMapGenMsg::Terminate => {
              break;
            }
          }
        }
      });
      (Some(tx), Some(handler))
    } else {
      (None, None)
    }
  }

  /// Make sure the passed `ids` is all absolute path
  async fn resolve_absolute_path(&self, ids: &[ArcStr]) -> BuildResult<Vec<ResolvedId>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;
    let options = &self.options;

    let resolved_ids = join_all(ids.iter().map(|input_item| async move {
      // The importer is useless, since all path is absolute path

      load_entry_module(options, resolver, plugin_driver, input_item, None).await
    }))
    .await;

    let mut ret = Vec::with_capacity(ids.len());

    let mut errors = vec![];

    for resolve_id in resolved_ids {
      match resolve_id {
        Ok(item) => {
          ret.push(item);
        }
        Err(e) => errors.push(e),
      }
    }

    if !errors.is_empty() {
      Err(errors)?;
    }

    Ok(ret)
  }
}

impl From<ModuleLoaderOutput> for ScanStageOutput {
  fn from(module_loader_output: ModuleLoaderOutput) -> Self {
    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      stmt_infos,
      dynamic_import_exports_usage_map,
      new_added_modules_from_partial_scan: _,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      flat_options,
      user_defined_entry_modules,
      tla_module_count,
      tla_keyword_span_map,
    } = module_loader_output;
    ScanStageOutput {
      module_table,
      index_ecma_ast,
      stmt_infos,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      flat_options,
      user_defined_entry_modules,
      tla_module_count,
      tla_keyword_span_map,
    }
  }
}
