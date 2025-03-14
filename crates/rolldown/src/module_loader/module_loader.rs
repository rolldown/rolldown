use super::module_task::{ModuleTask, ModuleTaskOwner};
use super::runtime_module_task::RuntimeModuleTask;
use super::task_context::TaskContextMeta;
use crate::ecmascript::ecma_module_view_factory::normalize_side_effects;
use crate::module_loader::task_context::TaskContext;
use crate::type_alias::IndexEcmaAst;
use crate::utils::load_entry_module::load_entry_module;
use arcstr::ArcStr;
use oxc::semantic::{ScopeId, Scoping};
use oxc::transformer::ReplaceGlobalDefinesConfig;
use oxc_index::IndexVec;
use rolldown_common::dynamic_import_usage::DynamicImportExportsUsage;
use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_common::{
  Cache, DUMMY_MODULE_IDX, EcmaRelated, EntryPoint, EntryPointKind, ExternalModule, ImportKind,
  ImportRecordIdx, ImportRecordMeta, ImporterRecord, Module, ModuleId, ModuleIdx, ModuleInfo,
  ModuleLoaderMsg, ModuleSideEffects, ModuleTable, ModuleType, NormalModuleTaskResult,
  RUNTIME_MODULE_ID, ResolvedExternal, ResolvedId, RuntimeModuleBrief, RuntimeModuleTaskResult,
  StmtInfoIdx, SymbolRefDb, SymbolRefDbForModule,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::ecmascript::legitimize_identifier_name;
use rolldown_utils::indexmap::FxIndexSet;
use rolldown_utils::rayon::{IntoParallelIterator, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;
use std::path::Path;
use std::sync::Arc;
use sugar_path::SugarPath;

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
  pub tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
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
    cache: Arc<Cache>,
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
      cache,
    });

    let mut intermediate_normal_modules = IntermediateNormalModules::new();
    let runtime_id = intermediate_normal_modules.alloc_ecma_module_idx();

    let task = RuntimeModuleTask::new(runtime_id, tx.clone(), Arc::clone(&options));

    tokio::spawn(async { task.run() });

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
    user_defined_entries: &[(Option<ArcStr>, ResolvedId)],
  ) -> ModuleIdx {
    match self.visited.entry(resolved_id.id.clone()) {
      Entry::Occupied(visited) => *visited.get(),
      Entry::Vacant(not_visited) => {
        let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();

        if resolved_id.external.is_external() {
          let external_module_side_effects = match resolved_id.side_effects {
            Some(hook_side_effects) => match hook_side_effects {
              HookSideEffects::True => DeterminedSideEffects::UserDefined(true),
              HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
              HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
            },
            _ => match self.options.treeshake.as_ref() {
              None => DeterminedSideEffects::NoTreeshake,
              Some(opt) => match opt.module_side_effects {
                ModuleSideEffects::Boolean(false) => DeterminedSideEffects::UserDefined(false),
                _ => {
                  if resolved_id.is_external_without_side_effects {
                    DeterminedSideEffects::UserDefined(false)
                  } else {
                    DeterminedSideEffects::NoTreeshake
                  }
                }
              },
            },
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
            SymbolRefDbForModule::new(Scoping::default(), idx, ScopeId::new(0)),
          );

          let need_renormalize_render_path =
            !matches!(resolved_id.external, ResolvedExternal::Absolute)
              && Path::new(resolved_id.id.as_str()).is_absolute();

          let name = if need_renormalize_render_path {
            let entries_common_dir = commondir::CommonDir::try_new(
              user_defined_entries.iter().map(|(_, resolved_id)| resolved_id.id.as_str()),
            )
            .expect("should have common dir for entries");
            let relative_path =
              Path::new(resolved_id.id.as_str()).relative(entries_common_dir.common_root());
            relative_path.to_slash_lossy().into()
          } else {
            resolved_id.id.clone()
          };

          let identifier_name = if need_renormalize_render_path {
            Path::new(resolved_id.id.as_str())
              .relative(&self.options.cwd)
              .normalize()
              .to_slash_lossy()
              .into()
          } else {
            resolved_id.id.clone()
          };
          let legitimized_identifier_name = legitimize_identifier_name(&identifier_name);

          let symbol_ref =
            self.symbol_ref_db.create_facade_root_symbol_ref(idx, &legitimized_identifier_name);

          let ext = ExternalModule::new(
            idx,
            resolved_id.id,
            name,
            legitimized_identifier_name.into(),
            external_module_side_effects,
            symbol_ref,
            need_renormalize_render_path,
          );
          self.intermediate_normal_modules.modules[idx] = Some(ext.into());
        } else {
          self.remaining += 1;

          let task = ModuleTask::new(
            Arc::clone(&self.shared_context),
            idx,
            resolved_id,
            owner,
            is_user_defined_entry,
            assert_module_type,
          );

          tokio::spawn(task.run());
        }

        *not_visited.insert(idx)
      }
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<ArcStr>, ResolvedId)>,
  ) -> BuildResult<ModuleLoaderOutput> {
    let mut errors = vec![];
    let mut all_warnings: Vec<BuildDiagnostic> = vec![];

    let entries_count = user_defined_entries.len() + /* runtime */ 1;
    self.intermediate_normal_modules.modules.reserve(entries_count);
    self.intermediate_normal_modules.index_ecma_ast.reserve(entries_count);

    // Store the already consider as entry module
    let mut user_defined_entry_ids = FxHashSet::with_capacity(user_defined_entries.len());

    let mut entry_points = user_defined_entries
      .iter()
      .map(|(name, info)| EntryPoint {
        name: name.clone(),
        id: self.try_spawn_new_task(info.clone(), None, true, None, &user_defined_entries),
        kind: EntryPointKind::UserDefined,
        file_name: None,
        reference_id: None,
        related_stmt_infos: vec![],
      })
      .inspect(|e| {
        user_defined_entry_ids.insert(e.id);
      })
      .collect::<Vec<_>>();

    let mut dynamic_import_entry_ids: FxHashMap<ModuleIdx, Vec<(ModuleIdx, StmtInfoIdx)>> =
      FxHashMap::default();
    let mut dynamic_import_exports_usage_pairs = vec![];
    let mut extra_entry_points = vec![];

    let mut runtime_brief: Option<RuntimeModuleBrief> = None;
    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };
      match msg {
        ModuleLoaderMsg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            mut module,
            mut ecma_related,
            resolved_deps,
            raw_import_records,
            warnings,
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
                if raw_rec.meta.contains(ImportRecordMeta::IS_DUMMY) {
                  return raw_rec.into_resolved(DUMMY_MODULE_IDX);
                }
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
                  &user_defined_entries,
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
                  match dynamic_import_entry_ids.entry(id) {
                    Entry::Vacant(vac) => match raw_rec.related_stmt_info_idx {
                      Some(stmt_info_idx) => {
                        vac.insert(vec![(module.idx(), stmt_info_idx)]);
                      }
                      None => {
                        vac.insert(vec![]);
                      }
                    },
                    Entry::Occupied(mut occ) => {
                      if let Some(stmt_info_idx) = raw_rec.related_stmt_info_idx {
                        occ.get_mut().push((module.idx(), stmt_info_idx));
                      }
                    }
                  }
                }
                raw_rec.into_resolved(id)
              })
              .collect::<IndexVec<ImportRecordIdx, _>>();

          module.set_import_records(import_records);

          let module_idx = module.idx();
          if let Some(EcmaRelated { ast, symbols, .. }) = ecma_related {
            let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module_idx));
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
                let id = self.try_spawn_new_task(
                  info,
                  None,
                  false,
                  raw_rec.asserted_module_type.clone(),
                  &user_defined_entries,
                );
                // Dynamic imported module will be considered as an entry
                self.intermediate_normal_modules.importers[id]
                  .push(ImporterRecord { kind: raw_rec.kind, importer_path: module.id.clone() });

                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  match dynamic_import_entry_ids.entry(id) {
                    Entry::Vacant(vac) => match raw_rec.related_stmt_info_idx {
                      Some(stmt_info_idx) => {
                        vac.insert(vec![(module.idx, stmt_info_idx)]);
                      }
                      None => {
                        vac.insert(vec![]);
                      }
                    },
                    Entry::Occupied(mut occ) => {
                      if let Some(stmt_info_idx) = raw_rec.related_stmt_info_idx {
                        occ.get_mut().push((module.idx, stmt_info_idx));
                      }
                    }
                  }
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
          self.try_spawn_new_task(resolve_id, None, false, None, &user_defined_entries);
        }
        ModuleLoaderMsg::AddEntryModule(msg) => {
          let data = msg.chunk;
          let result = load_entry_module(
            &self.shared_context.resolver,
            &self.shared_context.plugin_driver,
            &data.id,
            data.importer.as_deref(),
          )
          .await;
          let resolved_id = match result {
            Ok(result) => result,
            Err(e) => {
              errors.push(e);
              continue;
            }
          };
          extra_entry_points.push(EntryPoint {
            name: data.name.clone(),
            id: self.try_spawn_new_task(resolved_id, None, true, None, &user_defined_entries),
            kind: EntryPointKind::UserDefined,
            file_name: data.file_name.clone(),
            reference_id: Some(msg.reference_id),
            related_stmt_infos: vec![],
          });
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

    // defer sync user modified data in js side
    if let Some(ref func) = self.options.defer_sync_scan_data {
      let data = func.exec().await?;
      for d in data {
        let source_id = ArcStr::from(d.id);
        let Some(idx) = self.visited.get(&source_id) else {
          continue;
        };
        let Some(normal) = self.intermediate_normal_modules.modules[*idx]
          .as_mut()
          .and_then(|item| item.as_normal_mut())
        else {
          continue;
        };
        // TODO: Document this and recommend user to return `moduleSideEffects` in hook return
        // value rather than mutate the `ModuleInfo`
        normal.ecma_view.side_effects = match d.side_effects {
          Some(HookSideEffects::False) => DeterminedSideEffects::UserDefined(false),
          Some(HookSideEffects::NoTreeshake) => DeterminedSideEffects::NoTreeshake,
          _ => {
            // for Some(HookSideEffects::True) and None, we need to re resolve module source_id,
            // get package_json and re analyze the side effects
            let resolved_id: ResolvedId = self
              .shared_context
              .resolver
              // other params except `source_id` is not important, since we need `package_json`
              // from `resolved_id` to re analyze the side effects
              .resolve(None, source_id.as_str(), ImportKind::Import, normal.is_user_defined_entry)
              .expect("Should have resolved id")
              .into();
            normalize_side_effects(
              d.side_effects,
              &self.options,
              &normal.module_type,
              &resolved_id,
              &normal.stable_id,
              &normal.stmt_infos,
            )
            .await?
          }
        };
      }
    }

    let dynamic_import_exports_usage_map = dynamic_import_exports_usage_pairs.into_iter().fold(
      FxHashMap::default(),
      |mut acc, (idx, usage)| {
        match acc.entry(idx) {
          Entry::Vacant(vac) => {
            vac.insert(usage);
          }
          Entry::Occupied(mut occ) => {
            occ.get_mut().merge(usage);
          }
        };
        acc
      },
    );

    let mut none_empty_importer_module = vec![];
    let modules: IndexVec<ModuleIdx, Module> = self
      .intermediate_normal_modules
      .modules
      .into_iter_enumerated()
      .map(|(idx, module)| {
        let mut module = module.expect("Module tasks did't complete as expected");

        if let Some(module) = module.as_normal_mut() {
          // Note: (Compat to rollup)
          // The `dynamic_importers/importers` should be added after `module_parsed` hook.
          let importers = std::mem::take(&mut self.intermediate_normal_modules.importers[idx]);
          for importer in &importers {
            if importer.kind.is_static() {
              module.importers.insert(importer.importer_path.clone());
            } else {
              module.dynamic_importers.insert(importer.importer_path.clone());
            }
          }
          if !importers.is_empty() {
            none_empty_importer_module.push(idx);
          }
        }

        module
      })
      .collect();

    none_empty_importer_module.into_par_iter().for_each(|idx| {
      let module = &modules[idx];
      let Some(module) = module.as_normal() else {
        return;
      };
      self
        .shared_context
        .plugin_driver
        .set_module_info(&module.id, Arc::new(module.to_module_info(None)));
    });
    // if `inline_dynamic_imports` is set to be true, here we should not put dynamic imports to entries
    if !self.options.inline_dynamic_imports {
      let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
      dynamic_import_entry_ids.sort_unstable_by_key(|(id, _)| modules[*id].stable_id());

      entry_points.extend(dynamic_import_entry_ids.into_iter().map(|(id, related_stmt_infos)| {
        EntryPoint {
          name: None,
          id,
          kind: EntryPointKind::DynamicImport,
          file_name: None,
          reference_id: None,
          related_stmt_infos,
        }
      }));
    }

    extra_entry_points.sort_unstable_by_key(|entry| modules[entry.id].stable_id());
    entry_points.extend(extra_entry_points);

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
