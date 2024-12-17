use super::module_task::{ModuleTask, ModuleTaskOwner};
use super::runtime_module_task::RuntimeModuleTask;
use super::task_context::TaskContextMeta;
use crate::module_loader::task_context::TaskContext;
use crate::type_alias::IndexEcmaAst;
use arcstr::ArcStr;
use oxc::semantic::{ScopeId, SymbolTable};
use oxc::transformer::ReplaceGlobalDefinesConfig;
use oxc_index::IndexVec;
use rolldown_common::dynamic_import_usage::DynamicImportExportsUsage;
use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_common::{
  EcmaRelated, EntryPoint, EntryPointKind, ExternalModule, ImportKind, ImportRecordIdx,
  ImporterRecord, Module, ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, ModuleTable,
  ModuleType, NormalModuleTaskResult, ResolvedId, RuntimeModuleBrief, RuntimeModuleTaskResult,
  SymbolRefDb, SymbolRefDbForModule, RUNTIME_MODULE_ID,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::ecmascript::legitimize_identifier_name;
use rolldown_utils::indexmap::FxIndexSet;
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

use crate::{SharedOptions, SharedResolver};

pub struct IntermediateNormalModules {
  pub modules: IndexVec<ModuleIdx, Option<Module>>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub index_ecma_ast: IndexEcmaAst,
}

impl IntermediateNormalModules {
  pub fn new() -> Self {
    Self {
      modules: IndexVec::new(),
      importers: IndexVec::new(),
      index_ecma_ast: IndexVec::default(),
    }
  }

  pub fn alloc_ecma_module_idx(&mut self) -> ModuleIdx {
    let id = self.modules.push(None);
    self.importers.push(Vec::new());
    id
  }
}

pub struct ModuleLoader {
  options: SharedOptions,
  shared_context: Arc<TaskContext>,
  tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
  rx: tokio::sync::mpsc::Receiver<ModuleLoaderMsg>,
  visited: FxHashMap<ArcStr, ModuleIdx>,
  runtime_id: ModuleIdx,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  symbol_ref_db: SymbolRefDb,
}

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub symbol_ref_db: SymbolRefDb,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl ModuleLoader {
  pub fn new(
    fs: OsFileSystem,
    options: SharedOptions,
    resolver: SharedResolver,
    plugin_driver: SharedPluginDriver,
  ) -> BuildResult<Self> {
    // 1024 should be enough for most cases
    // over 1024 pending tasks are insane
    let (tx, rx) = tokio::sync::mpsc::channel(1024);

    let meta = TaskContextMeta {
      replace_global_define_config: if options.define.is_empty() {
        None
      } else {
        ReplaceGlobalDefinesConfig::new(&options.define).map(Some).map_err(|errs| {
          errs
            .into_iter()
            .map(|err| BuildDiagnostic::invalid_define_config(err.message.to_string()))
            .collect::<Vec<BuildDiagnostic>>()
        })?
      },
    };

    let shared_context = Arc::new(TaskContext {
      options: Arc::clone(&options),
      tx: tx.clone(),
      resolver,
      fs,
      plugin_driver,
      meta,
    });

    let mut intermediate_normal_modules = IntermediateNormalModules::new();
    let runtime_id = intermediate_normal_modules.alloc_ecma_module_idx();

    let task = RuntimeModuleTask::new(runtime_id, tx.clone(), Arc::clone(&options));

    #[cfg(target_family = "wasm")]
    {
      task.run();
    }
    // task is sync, but execution time is too short at the moment
    // so we are using spawn instead of spawn_blocking here to avoid an additional blocking thread creation within tokio
    #[cfg(not(target_family = "wasm"))]
    {
      let handle = tokio::runtime::Handle::current();
      handle.spawn(async { task.run() });
    }

    Ok(Self {
      tx,
      rx,
      options,
      runtime_id,
      // runtime module is always there
      remaining: 1,
      shared_context,
      intermediate_normal_modules,
      symbol_ref_db: SymbolRefDb::default(),
      visited: FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_id)]),
    })
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> ModuleIdx {
    match self.visited.entry(resolved_id.id.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        if resolved_id.is_external {
          let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();
          not_visited.insert(idx);
          let external_module_side_effects = if let Some(hook_side_effects) =
            resolved_id.side_effects
          {
            match hook_side_effects {
              HookSideEffects::True => DeterminedSideEffects::UserDefined(true),
              HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
              HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
            }
          } else {
            match self.options.treeshake {
              rolldown_common::TreeshakeOptions::Boolean(false) => {
                DeterminedSideEffects::NoTreeshake
              }
              rolldown_common::TreeshakeOptions::Boolean(true) => unreachable!(),
              rolldown_common::TreeshakeOptions::Option(ref opt) => match opt.module_side_effects {
                rolldown_common::ModuleSideEffects::Boolean(false) => {
                  DeterminedSideEffects::UserDefined(false)
                }
                _ => {
                  if resolved_id.is_external_without_side_effects {
                    DeterminedSideEffects::UserDefined(false)
                  } else {
                    DeterminedSideEffects::NoTreeshake
                  }
                }
              },
            }
          };

          let id = ModuleId::new(&resolved_id.id);
          self.shared_context.plugin_driver.set_module_info(
            &id.clone(),
            Arc::new(ModuleInfo {
              code: None,
              id,
              is_entry: false,
              importers: FxIndexSet::default(),
              dynamic_importers: FxIndexSet::default(),
              imported_ids: FxIndexSet::default(),
              dynamically_imported_ids: FxIndexSet::default(),
              exports: vec![],
            }),
          );

          self.symbol_ref_db.store_local_db(
            idx,
            SymbolRefDbForModule::new(SymbolTable::default(), idx, ScopeId::new(0)),
          );
          let symbol_ref = self.symbol_ref_db.create_facade_root_symbol_ref(
            idx,
            legitimize_identifier_name(resolved_id.id.as_str()).into(),
          );
          let ext = ExternalModule::new(
            idx,
            ArcStr::clone(&resolved_id.id),
            external_module_side_effects,
            symbol_ref,
          );
          self.intermediate_normal_modules.modules[idx] = Some(ext.into());
          idx
        } else {
          let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();
          not_visited.insert(idx);
          self.remaining += 1;

          let task = ModuleTask::new(
            Arc::clone(&self.shared_context),
            idx,
            resolved_id,
            owner,
            is_user_defined_entry,
            assert_module_type,
          );
          #[cfg(target_family = "wasm")]
          {
            let handle = tokio::runtime::Handle::current();
            // could not block_on/spawn the main thread in WASI
            std::thread::spawn(move || {
              handle.spawn(task.run());
            });
          }
          #[cfg(not(target_family = "wasm"))]
          tokio::spawn(task.run());
          idx
        }
      }
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<ArcStr>, ResolvedId)>,
  ) -> BuildResult<ModuleLoaderOutput> {
    if self.options.input.is_empty() {
      Err(anyhow::anyhow!("You must supply options.input to rolldown"))?;
    }

    self.shared_context.plugin_driver.set_context_load_modules_tx(Some(self.tx.clone())).await;

    let mut errors = vec![];
    let mut all_warnings: Vec<BuildDiagnostic> = vec![];

    let entries_count = user_defined_entries.len() + /* runtime */ 1;
    self.intermediate_normal_modules.modules.reserve(entries_count);
    self.intermediate_normal_modules.index_ecma_ast.reserve(entries_count);

    // Store the already consider as entry module
    let mut user_defined_entry_ids = FxHashSet::with_capacity(user_defined_entries.len());

    let mut entry_points = user_defined_entries
      .into_iter()
      .map(|(name, info)| EntryPoint {
        name,
        id: self.try_spawn_new_task(info, None, true, None),
        kind: EntryPointKind::UserDefined,
      })
      .inspect(|e| {
        user_defined_entry_ids.insert(e.id);
      })
      .collect::<Vec<_>>();

    let mut dynamic_import_entry_ids = FxHashSet::default();
    let mut dynamic_import_exports_usage_pairs = vec![];

    let mut runtime_brief: Option<RuntimeModuleBrief> = None;
    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };
      match msg {
        ModuleLoaderMsg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            module_idx,
            resolved_deps,
            mut module,
            raw_import_records,
            warnings,
            mut ecma_related,
          } = task_result;
          all_warnings.extend(warnings);
          let mut dynamic_import_rec_exports_usage = ecma_related
            .as_mut()
            .map(|item| std::mem::take(&mut item.dynamic_import_rec_exports_usage))
            .unwrap_or_default();
          let import_records: IndexVec<ImportRecordIdx, rolldown_common::ResolvedImportRecord> =
            raw_import_records
              .into_iter_enumerated()
              .zip(resolved_deps)
              .map(|((rec_idx, raw_rec), info)| {
                let normal_module = module.as_normal().unwrap();
                let owner = ModuleTaskOwner::new(
                  normal_module.source.clone(),
                  normal_module.stable_id.as_str().into(),
                  raw_rec.span,
                );
                let id = self.try_spawn_new_task(
                  info,
                  Some(owner),
                  false,
                  raw_rec.asserted_module_type.clone(),
                );
                // Dynamic imported module will be considered as an entry
                self.intermediate_normal_modules.importers[id].push(ImporterRecord {
                  kind: raw_rec.kind,
                  importer_path: ModuleId::new(module.id()),
                });
                // defer usage merging, since we only have one consumer, we should keep action during fetching as simple
                // as possible
                if let Some(usage) = dynamic_import_rec_exports_usage.remove(&rec_idx) {
                  dynamic_import_exports_usage_pairs.push((id, usage));
                }
                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  dynamic_import_entry_ids.insert(id);
                }
                raw_rec.into_resolved(id)
              })
              .collect::<IndexVec<ImportRecordIdx, _>>();

          module.set_import_records(import_records);
          if let Some(EcmaRelated { ast, symbols, .. }) = ecma_related {
            let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module.idx()));
            module.set_ecma_ast_idx(ast_idx);
            self.symbol_ref_db.store_local_db(module_idx, symbols);
          }
          self.intermediate_normal_modules.modules[module_idx] = Some(module);
          self.remaining -= 1;
        }
        ModuleLoaderMsg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeModuleTaskResult {
            local_symbol_ref_db,
            mut module,
            runtime,
            ast,
            raw_import_records,
            resolved_deps,
          } = task_result;
          let import_records: IndexVec<ImportRecordIdx, rolldown_common::ResolvedImportRecord> =
            raw_import_records
              .into_iter_enumerated()
              .zip(resolved_deps)
              .map(|((_rec_idx, raw_rec), info)| {
                let id =
                  self.try_spawn_new_task(info, None, false, raw_rec.asserted_module_type.clone());
                // Dynamic imported module will be considered as an entry
                self.intermediate_normal_modules.importers[id]
                  .push(ImporterRecord { kind: raw_rec.kind, importer_path: module.id.clone() });

                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  dynamic_import_entry_ids.insert(id);
                }
                raw_rec.into_resolved(id)
              })
              .collect::<IndexVec<ImportRecordIdx, _>>();
          let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module.idx));
          module.ecma_ast_idx = Some(ast_idx);
          module.import_records = import_records;
          self.intermediate_normal_modules.modules[self.runtime_id] = Some(module.into());

          self.symbol_ref_db.store_local_db(self.runtime_id, local_symbol_ref_db);
          runtime_brief = Some(runtime);
          self.remaining -= 1;
        }
        ModuleLoaderMsg::FetchModule(resolve_id) => {
          self.try_spawn_new_task(resolve_id, None, false, None);
        }
        ModuleLoaderMsg::BuildErrors(e) => {
          errors.extend(e);
          self.remaining -= 1;
        }
      }
    }

    if !errors.is_empty() {
      return Err(errors.into());
    }

    let dynamic_import_exports_usage_map = dynamic_import_exports_usage_pairs.into_iter().fold(
      FxHashMap::default(),
      |mut acc, (idx, usage)| {
        match acc.entry(idx) {
          std::collections::hash_map::Entry::Vacant(vac) => {
            vac.insert(usage);
          }
          std::collections::hash_map::Entry::Occupied(mut occ) => {
            occ.get_mut().merge(usage);
          }
        };
        acc
      },
    );

    self.shared_context.plugin_driver.set_context_load_modules_tx(None).await;

    let modules: IndexVec<ModuleIdx, Module> = self
      .intermediate_normal_modules
      .modules
      .into_iter()
      .enumerate()
      .map(|(id, module)| {
        let mut module = module.expect("Module tasks did't complete as expected");

        if let Some(module) = module.as_normal_mut() {
          let id = ModuleIdx::from(id);
          // Note: (Compat to rollup)
          // The `dynamic_importers/importers` should be added after `module_parsed` hook.
          let importers = std::mem::take(&mut self.intermediate_normal_modules.importers[id]);
          for importer in &importers {
            if importer.kind.is_static() {
              module.importers.insert(importer.importer_path.clone());
            } else {
              module.dynamic_importers.insert(importer.importer_path.clone());
            }
          }
          if !importers.is_empty() {
            self
              .shared_context
              .plugin_driver
              .set_module_info(&module.id, Arc::new(module.to_module_info(None)));
          }
        }

        module
      })
      .collect();

    // if `inline_dynamic_imports` is set to be true, here we should not put dynamic imports to entries
    if !self.options.inline_dynamic_imports {
      let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
      dynamic_import_entry_ids.sort_unstable_by_key(|id| modules[*id].stable_id());

      entry_points.extend(dynamic_import_entry_ids.into_iter().map(|id| EntryPoint {
        name: None,
        id,
        kind: EntryPointKind::DynamicImport,
      }));
    }

    Ok(ModuleLoaderOutput {
      module_table: ModuleTable { modules },
      symbol_ref_db: self.symbol_ref_db,
      index_ecma_ast: self.intermediate_normal_modules.index_ecma_ast,
      entry_points,
      runtime: runtime_brief.expect("Failed to find runtime module. This should not happen"),
      warnings: all_warnings,
      dynamic_import_exports_usage_map,
    })
  }
}
