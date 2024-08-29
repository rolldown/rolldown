use super::module_task::{ModuleTask, ModuleTaskOwner};
use super::runtime_module_task::RuntimeModuleTask;
use super::task_context::TaskContextMeta;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::module_loader::runtime_module_task::RuntimeModuleTaskResult;
use crate::module_loader::task_context::TaskContext;
use crate::runtime::{RuntimeModuleBrief, RUNTIME_MODULE_ID};
use crate::type_alias::IndexEcmaAst;
use crate::types::symbols::Symbols;
use arcstr::ArcStr;
use oxc::index::IndexVec;
use oxc::minifier::ReplaceGlobalDefinesConfig;
use oxc::span::Span;
use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_common::{
  EntryPoint, EntryPointKind, ExternalModule, ImportKind, ImportRecordIdx, ImporterRecord, Module,
  ModuleIdx, ModuleTable, ResolvedId,
};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
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

  pub fn alloc_ecma_module_idx(&mut self, symbols: &mut Symbols) -> ModuleIdx {
    let id = self.modules.push(None);
    self.importers.push(Vec::new());
    symbols.alloc_one();
    id
  }
}

pub struct ModuleLoader {
  options: SharedOptions,
  shared_context: Arc<TaskContext>,
  rx: tokio::sync::mpsc::Receiver<Msg>,
  visited: FxHashMap<ArcStr, ModuleIdx>,
  runtime_id: ModuleIdx,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  symbols: Symbols,
}

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub symbols: Symbols,
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
    // 1024 should be enough for most cases
    // over 1024 pending tasks are insane
    let (tx, rx) = tokio::sync::mpsc::channel::<Msg>(1024);

    let tx_to_runtime_module = tx.clone();

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
    let common_data = Arc::new(TaskContext {
      options: Arc::clone(&options),
      tx,
      resolver,
      fs,
      plugin_driver,
      meta,
    });

    let mut intermediate_normal_modules = IntermediateNormalModules::new();
    let mut symbols = Symbols::default();
    let runtime_id = intermediate_normal_modules.alloc_ecma_module_idx(&mut symbols);

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
      rx,
      options,
      visited: FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_id)]),
      runtime_id,
      // runtime module is always there
      remaining: 1,
      intermediate_normal_modules,
      symbols,
    })
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
  ) -> ModuleIdx {
    match self.visited.entry(resolved_id.id.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        if resolved_id.is_external {
          let idx = self.intermediate_normal_modules.alloc_ecma_module_idx(&mut self.symbols);
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
                _ => DeterminedSideEffects::NoTreeshake,
              },
            }
          };
          let ext =
            ExternalModule::new(idx, ArcStr::clone(&resolved_id.id), external_module_side_effects);
          self.intermediate_normal_modules.modules[idx] = Some(ext.into());
          idx
        } else {
          let idx = self.intermediate_normal_modules.alloc_ecma_module_idx(&mut self.symbols);
          not_visited.insert(idx);
          self.remaining += 1;

          let task = ModuleTask::new(Arc::clone(&self.shared_context), idx, resolved_id, owner);
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
  ) -> anyhow::Result<DiagnosableResult<ModuleLoaderOutput>> {
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
        id: self.try_spawn_new_task(info, /* is_user_defined_entry */ None),
        kind: EntryPointKind::UserDefined,
      })
      .inspect(|e| {
        user_defined_entry_ids.insert(e.id);
      })
      .collect::<Vec<_>>();

    let mut dynamic_import_entry_ids = FxHashSet::default();

    let mut runtime_brief: Option<RuntimeModuleBrief> = None;

    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };
      match msg {
        Msg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            module_idx,
            resolved_deps,
            mut module,
            raw_import_records,
            warnings,
            ecma_related,
          } = task_result;
          all_warnings.extend(warnings);

          let import_records: IndexVec<ImportRecordIdx, rolldown_common::ImportRecord> =
            raw_import_records
              .into_iter()
              .zip(resolved_deps)
              .map(|(raw_rec, info)| {
                let ecma_module = module.as_ecma().unwrap();
                let owner = ModuleTaskOwner::new(
                  ecma_module.source.clone(),
                  ecma_module.stable_id.as_str().into(),
                  Span::new(raw_rec.module_request_start, raw_rec.module_request_end()),
                );
                let id = self.try_spawn_new_task(info, Some(owner));
                // Dynamic imported module will be considered as an entry
                self.intermediate_normal_modules.importers[id].push(ImporterRecord {
                  kind: raw_rec.kind,
                  importer_path: module.id().to_string().into(),
                });
                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  dynamic_import_entry_ids.insert(id);
                }
                raw_rec.into_import_record(id)
              })
              .collect::<IndexVec<ImportRecordIdx, _>>();

          module.set_import_records(import_records);
          if let Some((ast, ast_symbol)) = ecma_related {
            let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module.idx()));
            module.set_ecma_ast_idx(ast_idx);
            self.symbols.add_ast_symbols(module_idx, ast_symbol);
          }
          self.intermediate_normal_modules.modules[module_idx] = Some(module);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeModuleTaskResult { ast_symbols, mut module, runtime, ast } = task_result;
          let ast_idx = self.intermediate_normal_modules.index_ecma_ast.push((ast, module.idx));
          module.ecma_ast_idx = Some(ast_idx);
          self.intermediate_normal_modules.modules[self.runtime_id] = Some(module.into());

          self.symbols.add_ast_symbols(self.runtime_id, ast_symbols);
          runtime_brief = Some(runtime);
        }
        Msg::BuildErrors(e) => {
          errors.extend(e);
        }
        // Expect cast to u32, since we are not going to have more than 2^32 tasks, or the
        // `remaining` will overflow
        #[allow(clippy::cast_possible_truncation)]
        Msg::Panics(err) => {
          // `self.remaining -1` for the panic task it self
          self.remaining -= 1;
          // gracefully shutdown all working thread, only receive and do not spawn
          while self.remaining > 0 {
            let mut task = Vec::with_capacity(self.remaining as usize);
            let received = self.rx.recv_many(&mut task, self.remaining as usize).await;
            self.remaining -= received as u32;
          }
          return Err(err);
        }
      }
      self.remaining -= 1;
    }

    if !errors.is_empty() {
      return Ok(Err(errors));
    }

    let modules: IndexVec<ModuleIdx, Module> = self
      .intermediate_normal_modules
      .modules
      .into_iter()
      .flatten()
      .enumerate()
      .map(|(id, mut module)| {
        if let Some(module) = module.as_ecma_mut() {
          // Note: (Compat to rollup)
          // The `dynamic_importers/importers` should be added after `module_parsed` hook.
          for importer in std::mem::take(&mut self.intermediate_normal_modules.importers[id]) {
            if importer.kind.is_static() {
              module.importers.push(importer.importer_path);
            } else {
              module.dynamic_importers.push(importer.importer_path);
            }
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
      symbols: self.symbols,
      index_ecma_ast: self.intermediate_normal_modules.index_ecma_ast,
      entry_points,
      runtime: runtime_brief.expect("Failed to find runtime module. This should not happen"),
      warnings: all_warnings,
    }))
  }
}
