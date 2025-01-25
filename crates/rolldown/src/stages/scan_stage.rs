use std::{collections::hash_map::Entry, sync::Arc};

use arcstr::ArcStr;
use futures::future::join_all;
use oxc_index::IndexVec;
use rolldown_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EcmaAstIdx, EntryPoint, HybridIndexVec,
  ModuleIdx, ModuleTable, ResolvedId, RuntimeModuleBrief, ScanMode, SymbolRefDb,
  SymbolRefDbForModule,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{
  index_vec_ext::IndexVecExt,
  rayon::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
};
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
  cache: Arc<ScanStageCache>,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub index_ast_scope: IndexAstScope,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: HybridIndexVec<ModuleIdx, Option<SymbolRefDbForModule>>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
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

/// # Panic
/// 1. Use with caution, the function will panic if the `temp_symbol_ref_db` is `[HybridIndexVec::Map]`
impl From<ScanStageOutput> for NormalizedScanStageOutput {
  fn from(value: ScanStageOutput) -> Self {
    Self {
      module_table: value.module_table,
      index_ecma_ast: value.index_ecma_ast,
      index_ast_scope: value.index_ast_scope,
      entry_points: value.entry_points,
      symbol_ref_db: match value.symbol_ref_db {
        HybridIndexVec::IndexVec(v) => SymbolRefDb::from_inner(v),
        HybridIndexVec::Map(_) => {
          unreachable!()
        }
      },
      runtime: value.runtime,
      warnings: value.warnings,
      dynamic_import_exports_usage_map: value.dynamic_import_exports_usage_map,
    }
  }
}

impl NormalizedScanStageOutput {
  /// Make a copy of the current ScanStage, skipping clone some fields that is immutable in
  /// following stage
  pub fn make_copy(&self) -> Self {
    // let (module_table, index_ecma_ast, symbol_ref_db) = std::thread::scope(|s| {
    //   let module_table_handle = s.spawn(|| self.module_table.clone());
    //   let index_ecma_ast_handle = s.spawn(|| {
    //     let vec: IndexVec<_, _> = self
    //       .index_ecma_ast
    //       .iter()
    //       .map(|item| (item.0.clone_with_another_arena(), item.1))
    //       .collect::<_>();
    //     vec
    //   });
    //   let symbol_ref_db_handle = s.spawn(|| self.symbol_ref_db.clone());
    //   (
    //     module_table_handle.join().unwrap(),
    //     index_ecma_ast_handle.join().unwrap(),
    //     symbol_ref_db_handle.join().unwrap(),
    //   )
    // });
    Self {
      module_table: {
        let vec = self.module_table.modules.raw.par_iter().map(|item| item.clone()).collect();
        let modules = IndexVec::from_vec(vec);
        ModuleTable { modules }
      },
      index_ecma_ast: {
        let vec = self
          .index_ecma_ast
          .raw
          .par_iter()
          .map(|item| (item.0.clone_with_another_arena(), item.1))
          .collect::<Vec<_>>();
        IndexVec::from_vec(vec)
      },
      entry_points: self.entry_points.clone(),
      symbol_ref_db: self.symbol_ref_db.parallel_clone(),
      runtime: self.runtime.clone(),
      warnings: vec![],
      index_ast_scope: IndexAstScope::default(),
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map.clone(),
    }
  }
}

impl ScanStage {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
    scan_stage_cache: Arc<ScanStageCache>,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver, cache: scan_stage_cache }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(
    &mut self,
    mode: ScanMode,
  ) -> BuildResult<(ScanStageOutput, FxHashMap<ArcStr, ModuleIdx>, FxHashMap<ModuleIdx, ModuleIdx>)>
  {
    if self.options.input.is_empty() {
      Err(anyhow::anyhow!("You must supply options.input to rolldown"))?;
    }

    let (normalized_visited, changed_ids_visited_snapshot) = match mode {
      ScanMode::Full => (FxHashMap::default(), FxHashMap::default()),
      ScanMode::Partial(ref changed) => {
        let mut visited = self.cache.module_id_to_idx().clone();
        let mut changed_ids_visited_snapshot = FxHashMap::default();
        // when rebuild mode,
        // the newly allocated ModuleIdx starts from `0`, which may conflict with the existing ModuleIdx
        for resource_id in changed {
          let item = visited.remove(resource_id);
          if let Some(idx) = item {
            changed_ids_visited_snapshot.insert(resource_id.clone(), idx);
          }
        }
        (visited, changed_ids_visited_snapshot)
      }
    };

    let module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      normalized_visited,
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
      mut visited,
    } = module_loader.fetch_modules(user_entries, changed_resolved_ids).await?;

    let mut changed_id_remapping = FxHashMap::default();
    // revert the original `ModuleIdx` for changed modules
    // 1. for foll build mode, the `ModuleIdx` is already correct, and the `changed_ids_visited_snapshot` is empty
    // 2. for rebuild mode, the `ResourceId` -> `ModuleIdx` will be revert to previous bundle state

    for (k, original_module_idx) in changed_ids_visited_snapshot {
      match visited.entry(k) {
        Entry::Occupied(mut occ) => {
          let new_module_idx = *occ.get();
          occ.insert(original_module_idx);
          changed_id_remapping.insert(original_module_idx, new_module_idx);
        }
        Entry::Vacant(_) => {
          unreachable!("Each changed module should be visited");
        }
      }
    }

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
      changed_id_remapping,
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
