use std::{sync::Arc, thread};

use arcstr::ArcStr;
use futures::future::join_all;
use oxc::span::Span;
use oxc_index::IndexVec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_common::SourceMapGenMsg;
use rolldown_common::{
  EntryPoint, FlatOptions, HybridIndexVec, Module, ModuleIdx, ModuleTable, PreserveEntrySignatures,
  ResolvedId, RuntimeModuleBrief, ScanMode, SourcemapChainElement, StmtInfos, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, SharedResolver,
  module_loader::{ModuleLoader, module_loader::ModuleLoaderOutput},
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::scan_stage_cache::ScanStageCache,
  utils::load_entry_module::load_entry_module,
};

type SourcemapChannel = (
  Option<Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
  Option<thread::JoinHandle<FxHashMap<ModuleIdx, Vec<SourcemapChainElement>>>>,
);

pub struct ScanStage<Fs: FileSystem + Clone + 'static> {
  options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: Fs,
  resolver: SharedResolver<Fs>,
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
  ) -> Self {
    Self { options, plugin_driver, fs, resolver }
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
        ScanMode::Partial(self.resolve_absolute_path(&changed_ids).await?)
      }
    };
    let (tx_clone, handler) = self.create_sourcemap_channel();

    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      cache,
      fetch_mode.is_full(),
      tx_clone,
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

    let mut module_loader_output = module_loader.fetch_modules(fetch_mode).await?;

    if let Some(handler) = handler {
      self.process_sourcemap_handler(handler, &mut module_loader_output);
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
        let mut map: FxHashMap<ModuleIdx, Vec<_>> = FxHashMap::default();
        while let Ok(msg) = rx.recv() {
          match msg {
            SourceMapGenMsg::MagicString(v) => {
              let (module_idx, plugin_idx, magic_string) = *v;
              let generated_sourcemap =
                magic_string.source_map(string_wizard::SourceMapOptions::default());
              map
                .entry(module_idx)
                .or_default()
                .push(SourcemapChainElement::Transform((plugin_idx, generated_sourcemap)));
            }
            SourceMapGenMsg::Terminate => {
              break;
            }
          }
        }
        map
      });
      (Some(Arc::new(tx)), Some(handler))
    } else {
      (None, None)
    }
  }

  fn process_sourcemap_handler(
    &self,
    handler: thread::JoinHandle<FxHashMap<ModuleIdx, Vec<SourcemapChainElement>>>,
    module_loader_output: &mut ModuleLoaderOutput,
  ) {
    let map: FxHashMap<ModuleIdx, Vec<_>> = handler.join().unwrap();
    if !map.is_empty() {
      let transform_plugin_order_map = self
        .plugin_driver
        .order_by_transform_meta
        .iter()
        .enumerate()
        .map(|(i, plugin_idx)| (*plugin_idx, i))
        .collect::<FxHashMap<_, _>>();
      for (module_idx, sourcemaps) in map {
        let Some(module) = module_loader_output.module_table.get_mut(module_idx).as_normal_mut()
        else {
          continue;
        };
        module.sourcemap_chain.extend(sourcemaps);
        // Partition sourcemap chain into Load and Transform elements (O(n) instead of O(n log n) sort)
        // Load elements come first, then Transform elements sorted by plugin order
        let chain_len = module.sourcemap_chain.len();
        let mut load_elements = Vec::with_capacity(chain_len);
        let mut transform_elements = Vec::with_capacity(chain_len);

        for element in module.sourcemap_chain.drain(..) {
          match element {
            SourcemapChainElement::Load(_) => load_elements.push(element),
            SourcemapChainElement::Transform(_) => transform_elements.push(element),
          }
        }

        // Sort only Transform elements by plugin order
        transform_elements.sort_by(|a, b| {
          if let (
            SourcemapChainElement::Transform((a_plugin_idx, _)),
            SourcemapChainElement::Transform((b_plugin_idx, _)),
          ) = (a, b)
          {
            let a_order =
              transform_plugin_order_map.get(a_plugin_idx).copied().unwrap_or(usize::MAX);
            let b_order =
              transform_plugin_order_map.get(b_plugin_idx).copied().unwrap_or(usize::MAX);
            a_order.cmp(&b_order)
          } else {
            std::cmp::Ordering::Equal
          }
        });

        // Reconstruct: Load elements first, then sorted Transform elements
        module.sourcemap_chain = load_elements;
        module.sourcemap_chain.extend(transform_elements);
      }
    }
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
