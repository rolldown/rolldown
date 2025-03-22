use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::join_all;
use rolldown_common::{
  Cache, EntryPoint, ModuleIdx, ModuleTable, ResolvedId, RuntimeModuleBrief, SymbolRefDb,
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
  utils::load_entry_module::load_entry_module,
};

pub struct ScanStage {
  options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: OsFileSystem,
  resolver: SharedResolver,
  cache: Arc<Cache>,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl ScanStageOutput {
  /// Make a copy of the current ScanStage, skipping clone some fields that is immutable in
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
      symbol_ref_db: self.symbol_ref_db.clone(),
      runtime: self.runtime.clone(),
      warnings: vec![],
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
    cache: Arc<Cache>,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver, cache }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(&mut self) -> BuildResult<ScanStageOutput> {
    if self.options.input.is_empty() {
      Err(anyhow::anyhow!("You must supply options.input to rolldown"))?;
    }

    let module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      Arc::clone(&self.cache),
    )?;

    // For `pluginContext.emitFile` with `type: chunk`, support it at buildStart hook.
    self
      .plugin_driver
      .file_emitter
      .set_context_load_modules_tx(Some(module_loader.tx.clone()))
      .await;

    self.plugin_driver.build_start(&self.options).await?;

    let user_entries = self.resolve_user_defined_entries().await?;

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
    } = module_loader.fetch_all_modules(user_entries).await?;

    self.plugin_driver.file_emitter.set_context_load_modules_tx(None).await;

    self.plugin_driver.set_context_load_modules_tx(None).await;

    Ok(ScanStageOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      dynamic_import_exports_usage_map,
    })
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
}
