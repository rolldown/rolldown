use super::module_task::{ModuleTask, ModuleTaskOwner};
use super::runtime_module_task::RuntimeModuleTask;
use super::task_context::TaskContextMeta;
use crate::module_loader::task_context::TaskContext;
use crate::type_alias::IndexEcmaAst;
use arcstr::ArcStr;
use oxc::index::IndexVec;
use oxc::transformer::ReplaceGlobalDefinesConfig;
use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_common::{
  EntryPoint, EntryPointKind, ExternalModule, ImportKind, ImportRecordIdx, ImporterRecord, Module,
  ModuleId, ModuleIdx, ModuleLoaderMsg, ModuleTable, NormalModuleTaskResult, ResolvedId,
  RuntimeModuleBrief, RuntimeModuleTaskResult, SymbolNameRefToken, SymbolRefDb, RUNTIME_MODULE_ID,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::ecmascript::legitimize_identifier_name;
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
}

impl ModuleLoader {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> anyhow::Result<Self> {
    let tx_to_runtime_module = plugin_driver.tx.clone();

    let meta = TaskContextMeta {
      replace_global_define_config: if options.define.is_empty() {
        None
      } else {
        Some(ReplaceGlobalDefinesConfig::new(&options.define).map_err(|errs| {
          // TODO: maybe we should give better diagnostics here. since oxc return
          // `Vec<OxcDiagnostic>`
          anyhow::format_err!(
            "Failed to generate defines config from {:?}. Got {:#?}",
            options.define,
            errs
          )
        })?)
      },
    };
    let common_data =
      Arc::new(TaskContext { options: Arc::clone(&options), resolver, fs, plugin_driver, meta });

    let mut intermediate_normal_modules = IntermediateNormalModules::new();
    let symbols = SymbolRefDb::default();
    let runtime_id = intermediate_normal_modules.alloc_ecma_module_idx();

    let task = RuntimeModuleTask::new(runtime_id, tx_to_runtime_module);

    #[cfg(target_family = "wasm")]
    {
      task.run().unwrap();
    }
    // task is sync, but execution time is too short at the moment
    // so we are using spawn instead of spawn_blocking here to avoid an additional blocking thread creation within tokio
    #[cfg(not(target_family = "wasm"))]
    {
      let handle = tokio::runtime::Handle::current();
      handle.spawn(async { task.run() });
    }

    Ok(Self {
      shared_context: common_data,
      options,
      visited: FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_id)]),
      runtime_id,
      // runtime module is always there
      remaining: 1,
      intermediate_normal_modules,
      symbol_ref_db: symbols,
    })
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
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
          let ext = ExternalModule::new(
            idx,
            ArcStr::clone(&resolved_id.id),
            external_module_side_effects,
            SymbolNameRefToken::new(idx, legitimize_identifier_name(&resolved_id.id).into()),
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
  ) -> anyhow::Result<BuildResult<ModuleLoaderOutput>> {
    if self.options.input.is_empty() {
      return Err(anyhow::format_err!("You must supply options.input to rolldown"));
    }

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
        id: self.try_spawn_new_task(info, None, true),
        kind: EntryPointKind::UserDefined,
      })
      .inspect(|e| {
        user_defined_entry_ids.insert(e.id);
      })
      .collect::<Vec<_>>();

    let mut dynamic_import_entry_ids = FxHashSet::default();

    let mut runtime_brief: Option<RuntimeModuleBrief> = None;

    let rx = self.shared_context.plugin_driver.rx.clone();
    let mut rx = rx.lock().await;

    while self.remaining > 0 {
      let Some(msg) = rx.recv().await else {
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
            ecma_related,
          } = task_result;
          all_warnings.extend(warnings);

          let import_records: IndexVec<ImportRecordIdx, rolldown_common::ResolvedImportRecord> =
            raw_import_records
              .into_iter()
              .zip(resolved_deps)
              .map(|(raw_rec, info)| {
                let normal_module = module.as_normal().unwrap();
                let owner = ModuleTaskOwner::new(
                  normal_module.source.clone(),
                  normal_module.stable_id.as_str().into(),
                  raw_rec.span,
                );
                let id = self.try_spawn_new_task(info, Some(owner), false);
                // Dynamic imported module will be considered as an entry
                self.intermediate_normal_modules.importers[id].push(ImporterRecord {
                  kind: raw_rec.kind,
                  importer_path: ModuleId::new(module.id()),
                });
                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  dynamic_import_entry_ids.insert(id);
                }
                raw_rec.into_resolved(id)
              })
              .collect::<IndexVec<ImportRecordIdx, _>>();

          module.set_import_records(import_records);
          if let Some((ast, ast_symbol)) = ecma_related {
            let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module.idx()));
            module.set_ecma_ast_idx(ast_idx);
            self.symbol_ref_db.store_local_db(module_idx, ast_symbol);
          }
          self.intermediate_normal_modules.modules[module_idx] = Some(module);
        }
        ModuleLoaderMsg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeModuleTaskResult { local_symbol_ref_db, mut module, runtime, ast } =
            task_result;
          let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module.idx));
          module.ecma_ast_idx = Some(ast_idx);
          self.intermediate_normal_modules.modules[self.runtime_id] = Some(module.into());

          self.symbol_ref_db.store_local_db(self.runtime_id, local_symbol_ref_db);
          runtime_brief = Some(runtime);
        }
        ModuleLoaderMsg::FetchModule(resolve_id) => {
          self.try_spawn_new_task(resolve_id, None, false);
        }
        ModuleLoaderMsg::BuildErrors(e) => {
          errors.extend(e);
        }
      }
      self.remaining -= 1;
    }

    if !errors.is_empty() {
      return Ok(Err(errors.into()));
    }

    let modules: IndexVec<ModuleIdx, Module> = self
      .intermediate_normal_modules
      .modules
      .into_iter()
      .flatten()
      .enumerate()
      .map(|(id, mut module)| {
        let id = ModuleIdx::from(id);
        if let Some(module) = module.as_normal_mut() {
          // Note: (Compat to rollup)
          // The `dynamic_importers/importers` should be added after `module_parsed` hook.
          let importers = std::mem::take(&mut self.intermediate_normal_modules.importers[id]);
          for importer in &importers {
            if importer.kind.is_static() {
              module.importers.push(importer.importer_path.clone());
            } else {
              module.dynamic_importers.push(importer.importer_path.clone());
            }
          }
          if !importers.is_empty() {
            self
              .shared_context
              .plugin_driver
              .set_module_info(&module.id, Arc::new(module.to_module_info()));
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

    Ok(Ok(ModuleLoaderOutput {
      module_table: ModuleTable { modules },
      symbol_ref_db: self.symbol_ref_db,
      index_ecma_ast: self.intermediate_normal_modules.index_ecma_ast,
      entry_points,
      runtime: runtime_brief.expect("Failed to find runtime module. This should not happen"),
      warnings: all_warnings,
    }))
  }
}
