use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::join_all;
use rolldown_common::{
  EntryPoint, HybridIndexVec, Module, ModuleIdx, ModuleTable, PreserveEntrySignatures, ResolvedId,
  RuntimeModuleBrief, ScanMode, SymbolRef, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rustc_hash::FxHashMap;

use crate::{
  SharedOptions, SharedResolver,
  module_loader::{ModuleLoader, module_loader::ModuleLoaderOutput},
  type_alias::IndexEcmaAst,
  types::scan_stage_cache::ScanStageCache,
  utils::load_entry_module::load_entry_module,
};

pub struct ScanStage {
  options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: OsFileSystem,
  resolver: SharedResolver,
}

#[derive(Debug)]
pub struct NormalizedScanStageOutput {
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, Vec<SymbolRef>>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  // TODO: merge the preserve_entry_signatures_map in incremental build
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
}

impl NormalizedScanStageOutput {
  /// Make a snapshot of the current ScanStage, skipping clone some fields that is immutable in
  /// following stage
  pub fn make_copy(&self) -> Self {
    Self {
      module_table: self.module_table.clone(),
      index_ecma_ast: self
        .index_ecma_ast
        .iter()
        .map(|(ast, module_idx)| (ast.clone_with_another_arena(), *module_idx))
        .collect(),
      entry_points: self.entry_points.clone(),
      symbol_ref_db: self.symbol_ref_db.clone_without_scoping(),
      runtime: self.runtime.clone(),
      warnings: vec![],
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map.clone(),
      safely_merge_cjs_ns_map: self.safely_merge_cjs_ns_map.clone(),
      overrode_preserve_entry_signature_map: self.overrode_preserve_entry_signature_map.clone(),
      entry_point_to_reference_ids: self.entry_point_to_reference_ids.clone(),
    }
  }
}

impl From<ScanStageOutput> for NormalizedScanStageOutput {
  fn from(value: ScanStageOutput) -> Self {
    Self {
      module_table: match value.module_table {
        HybridIndexVec::IndexVec(modules) => ModuleTable { modules },
        HybridIndexVec::Map(_) => unreachable!("Please normalized first"),
      },
      index_ecma_ast: value.index_ecma_ast,
      entry_points: value.entry_points,
      symbol_ref_db: value.symbol_ref_db,
      runtime: value.runtime,
      warnings: value.warnings,
      dynamic_import_exports_usage_map: value.dynamic_import_exports_usage_map,
      safely_merge_cjs_ns_map: value.safely_merge_cjs_ns_map,
      overrode_preserve_entry_signature_map: value.overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: value.entry_point_to_reference_ids,
    }
  }
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: HybridIndexVec<ModuleIdx, Module>,
  pub index_ecma_ast: IndexEcmaAst,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, Vec<SymbolRef>>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
}

impl ScanStage {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver }
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn scan(
    &self,
    mode: ScanMode,
    cache: &mut ScanStageCache,
  ) -> BuildResult<ScanStageOutput> {
    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      cache,
      mode.is_full(),
    )?;

    // For `pluginContext.emitFile` with `type: chunk`, support it at buildStart hook.
    self
      .plugin_driver
      .file_emitter
      .set_context_load_modules_tx(Some(module_loader.tx.clone()))
      .await;

    self.plugin_driver.build_start(&self.options).await?;

    let user_entries = match mode {
      ScanMode::Full => self.resolve_user_defined_entries().await?,
      ScanMode::Partial(_) => vec![],
    };

    let changed_resolved_ids = match mode {
      ScanMode::Full => vec![],
      ScanMode::Partial(changed_ids) => self.resolve_absolute_path(&changed_ids).await?,
    };

    // For `await pluginContext.load`, if support it at buildStart hook, it could be caused stuck.
    self.plugin_driver.set_context_load_modules_tx(Some(module_loader.tx.clone())).await;

    let module_loader_output =
      module_loader.fetch_modules(user_entries, &changed_resolved_ids).await?;
    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      dynamic_import_exports_usage_map,
      new_added_modules_from_partial_scan: _,
      safely_merge_cjs_ns_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
    } = module_loader_output;

    self.plugin_driver.file_emitter.set_context_load_modules_tx(None).await;

    self.plugin_driver.set_context_load_modules_tx(None).await;

    Ok(ScanStageOutput {
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      dynamic_import_exports_usage_map,
      module_table,
      safely_merge_cjs_ns_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
    })
  }

  /// Resolve `InputOptions.input`
  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  async fn resolve_user_defined_entries(&self) -> BuildResult<Vec<(Option<ArcStr>, ResolvedId)>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids = join_all(self.options.input.iter().map(|input_item| async move {
      let resolved = load_entry_module(resolver, plugin_driver, &input_item.import, None).await;

      resolved.map(|info| (input_item.name.as_ref().map(Into::into), info))
    }))
    .await;

    let mut ret = Vec::with_capacity(self.options.input.len());

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

  /// Make sure the passed `ids` is all absolute path
  async fn resolve_absolute_path(&self, ids: &[ArcStr]) -> BuildResult<Vec<ResolvedId>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids = join_all(ids.iter().map(|input_item| async move {
      // The importer is useless, since all path is absolute path

      load_entry_module(resolver, plugin_driver, input_item, None).await
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
