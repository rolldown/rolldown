use std::{rc::Rc, sync::Arc};

use arcstr::ArcStr;
use futures::{future::join_all, stream::Scan};
use oxc_index::IndexVec;
use rolldown_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, HybridIndexVec, Module, ModuleIdx,
  ModuleTable, ResolvedId, RuntimeModuleBrief, ScanMode, SymbolRefDb,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::rayon::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use crate::{
  module_loader::{module_loader::ModuleLoaderOutput, ModuleLoader},
  type_alias::{IndexAstScope, IndexEcmaAst},
  types::scan_stage_cache::ScanStageCache,
  utils::load_entry_module::load_entry_module,
  SharedOptions, SharedResolver,
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
  pub index_ast_scope: IndexAstScope,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}
impl NormalizedScanStageOutput {
  /// Make a copy of the current ScanStage, skipping clone some fields that is immutable in
  /// following stage
  pub fn make_copy(&self) -> Self {
    Self {
      module_table: self.module_table.clone(),
      index_ecma_ast: {
        let item = self
          .index_ecma_ast
          .raw
          .par_iter()
          .map(|(ast, module_idx)| (ast.clone_with_another_arena(), *module_idx))
          .collect::<Vec<_>>();
        IndexVec::from_vec(item)
      },
      entry_points: self.entry_points.clone(),
      symbol_ref_db: self.symbol_ref_db.clone(),
      runtime: self.runtime.clone(),
      warnings: vec![],
      index_ast_scope: IndexAstScope::default(),
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map.clone(),
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
      index_ast_scope: value.index_ast_scope,
      entry_points: value.entry_points,
      symbol_ref_db: value.symbol_ref_db,
      runtime: value.runtime,
      warnings: value.warnings,
      dynamic_import_exports_usage_map: value.dynamic_import_exports_usage_map,
    }
  }
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: HybridIndexVec<ModuleIdx, Module>,
  pub index_ecma_ast: IndexEcmaAst,
  pub index_ast_scope: IndexAstScope,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl ScanStageOutput {}

impl ScanStage {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(
    &mut self,
    mode: ScanMode,
    module_id_to_idx: FxHashMap<ArcStr, ModuleIdx>,
  ) -> BuildResult<(ScanStageOutput, FxHashMap<ArcStr, ModuleIdx>)> {
    if self.options.input.is_empty() {
      Err(anyhow::anyhow!("You must supply options.input to rolldown"))?;
    }

    let module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      module_id_to_idx,
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

    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      dynamic_import_exports_usage_map,
      index_ast_scope,
      visited,
    } = module_loader.fetch_modules(user_entries, changed_resolved_ids).await?;

    self.plugin_driver.file_emitter.set_context_load_modules_tx(None).await;

    self.plugin_driver.set_context_load_modules_tx(None).await;
    let ret = (
      ScanStageOutput {
        index_ast_scope,
        module_table,
        entry_points,
        symbol_ref_db,
        runtime,
        warnings,
        index_ecma_ast,
        dynamic_import_exports_usage_map,
      },
      visited,
    );
    Ok(ret)
  }

  /// Resolve `InputOptions.input`
  #[tracing::instrument(level = "debug", skip_all)]
  async fn resolve_user_defined_entries(
    &mut self,
  ) -> BuildResult<Vec<(Option<ArcStr>, ResolvedId)>> {
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
  async fn resolve_absolute_path(&mut self, ids: &Vec<ArcStr>) -> BuildResult<Vec<ResolvedId>> {
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
