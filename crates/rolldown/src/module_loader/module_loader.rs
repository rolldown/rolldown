use oxc::index::IndexVec;
use rolldown_common::side_effects::DeterminedSideEffects;
use rolldown_common::{
  EcmaModule, EcmaModuleIdx, EntryPoint, EntryPointKind, ExternalModule, ImportKind,
  ImportRecordIdx, ImporterRecord, IndexExternalModules, ModuleIdx, ModuleTable,
  ResolvedRequestInfo,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildError;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

use super::ecma_module_task::EcmaModuleTask;
use super::runtime_ecma_module_task::RuntimeEcmaModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::module_loader::runtime_ecma_module_task::RuntimeEcmaModuleTaskResult;
use crate::module_loader::task_context::TaskContext;
use crate::runtime::{RuntimeModuleBrief, ROLLDOWN_RUNTIME_RESOURCE_ID};
use crate::types::symbols::Symbols;

use crate::{SharedOptions, SharedResolver};

pub struct IntermediateNormalModules {
  pub modules: IndexVec<EcmaModuleIdx, Option<EcmaModule>>,
  pub index_ecma_ast: IndexVec<EcmaModuleIdx, Option<EcmaAst>>,
  pub importers: IndexVec<EcmaModuleIdx, Vec<ImporterRecord>>,
}

impl IntermediateNormalModules {
  pub fn new() -> Self {
    Self { modules: IndexVec::new(), index_ecma_ast: IndexVec::new(), importers: IndexVec::new() }
  }

  pub fn alloc_ecma_module_idx(&mut self, symbols: &mut Symbols) -> EcmaModuleIdx {
    let id = self.modules.push(None);
    self.index_ecma_ast.push(None);
    self.importers.push(Vec::new());
    symbols.alloc_one();
    id
  }
}

pub struct ModuleLoader {
  input_options: SharedOptions,
  shared_context: Arc<TaskContext>,
  rx: tokio::sync::mpsc::Receiver<Msg>,
  visited: FxHashMap<Arc<str>, ModuleIdx>,
  runtime_id: EcmaModuleIdx,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  external_modules: IndexExternalModules,
  symbols: Symbols,
}

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexVec<EcmaModuleIdx, EcmaAst>,
  pub symbols: Symbols,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
  pub errors: Vec<BuildError>,
}

impl ModuleLoader {
  pub fn new(
    input_options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> Self {
    // 1024 should be enough for most cases
    // over 1024 pending tasks are insane
    let (tx, rx) = tokio::sync::mpsc::channel::<Msg>(1024);

    let tx_to_runtime_module = tx.clone();

    let common_data = Arc::new(TaskContext {
      input_options: Arc::clone(&input_options),
      tx,
      resolver,
      fs,
      plugin_driver,
    });

    let mut intermediate_normal_modules = IntermediateNormalModules::new();
    let mut symbols = Symbols::default();
    let runtime_id = intermediate_normal_modules.alloc_ecma_module_idx(&mut symbols);

    let task = RuntimeEcmaModuleTask::new(runtime_id, tx_to_runtime_module);

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

    Self {
      shared_context: common_data,
      rx,
      input_options,
      visited: FxHashMap::from_iter([(ROLLDOWN_RUNTIME_RESOURCE_ID.into(), runtime_id.into())]),
      runtime_id,
      // runtime module is always there
      remaining: 1,
      intermediate_normal_modules,
      external_modules: IndexVec::new(),
      symbols,
    }
  }

  fn try_spawn_new_task(
    &mut self,
    info: ResolvedRequestInfo,
    is_user_defined_entry: bool,
  ) -> ModuleIdx {
    match self.visited.entry(Arc::<str>::clone(&info.path.path)) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        if info.is_external {
          let id = self.external_modules.len_idx();
          not_visited.insert(id.into());
          let external_module_side_effects = match self.input_options.treeshake {
            rolldown_common::TreeshakeOptions::False => DeterminedSideEffects::NoTreeshake,
            rolldown_common::TreeshakeOptions::Option(ref opt) => match opt.module_side_effects {
              rolldown_common::ModuleSideEffects::Boolean(false) => {
                DeterminedSideEffects::UserDefined(false)
              }
              _ => DeterminedSideEffects::NoTreeshake,
            },
          };
          let ext =
            ExternalModule::new(id, info.path.path.to_string(), external_module_side_effects);
          self.external_modules.push(ext);
          id.into()
        } else {
          let id = self.intermediate_normal_modules.alloc_ecma_module_idx(&mut self.symbols);
          not_visited.insert(id.into());
          self.remaining += 1;
          let module_path = info.path.clone();

          let task = EcmaModuleTask::new(
            Arc::clone(&self.shared_context),
            id,
            module_path,
            info.module_type,
            is_user_defined_entry,
            info.package_json,
            info.side_effects,
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
          id.into()
        }
      }
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(String, ResolvedRequestInfo)>,
  ) -> anyhow::Result<ModuleLoaderOutput> {
    if self.input_options.input.is_empty() {
      return Err(anyhow::format_err!("You must supply options.input to rolldown"));
    }

    let mut errors = vec![];
    let mut all_warnings: Vec<BuildError> = vec![];

    let entries_count = user_defined_entries.len() + /* runtime */ 1;
    self.intermediate_normal_modules.modules.reserve(entries_count);
    self.intermediate_normal_modules.index_ecma_ast.reserve(entries_count);

    // Store the already consider as entry module
    let mut user_defined_entry_ids = FxHashSet::with_capacity(user_defined_entries.len());

    let mut entry_points = user_defined_entries
      .into_iter()
      .map(|(name, info)| EntryPoint {
        name: Some(name),
        id: self.try_spawn_new_task(info, /* is_user_defined_entry */ true).expect_ecma(),
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
            module_id,
            ast_symbol,
            resolved_deps,
            mut module,
            raw_import_records,
            warnings,
            ast,
          } = task_result;
          all_warnings.extend(warnings);

          let import_records = raw_import_records
            .into_iter()
            .zip(resolved_deps)
            .map(|(raw_rec, info)| {
              let id = self.try_spawn_new_task(info, false);
              // Dynamic imported module will be considered as an entry
              if let ModuleIdx::Ecma(id) = id {
                self.intermediate_normal_modules.importers[id].push(ImporterRecord {
                  kind: raw_rec.kind,
                  importer_path: module.resource_id.clone(),
                });
                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  dynamic_import_entry_ids.insert(id);
                }
              }
              raw_rec.into_import_record(id)
            })
            .collect::<IndexVec<ImportRecordIdx, _>>();
          module.import_records = import_records;

          self.intermediate_normal_modules.modules[module_id] = Some(module);
          self.intermediate_normal_modules.index_ecma_ast[module_id] = Some(ast);

          self.symbols.add_ast_symbols(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeEcmaModuleTaskResult { ast_symbol, module, runtime, ast } = task_result;

          self.intermediate_normal_modules.modules[self.runtime_id] = Some(module);
          self.intermediate_normal_modules.index_ecma_ast[self.runtime_id] = Some(ast);

          self.symbols.add_ast_symbols(self.runtime_id, ast_symbol);
          runtime_brief = Some(runtime);
        }
        Msg::BuildErrors(e) => {
          errors.extend(e);
        }
        Msg::Panics(err) => {
          return Err(err);
        }
      }
      self.remaining -= 1;
    }

    let modules: IndexVec<EcmaModuleIdx, EcmaModule> = self
      .intermediate_normal_modules
      .modules
      .into_iter()
      .flatten()
      .enumerate()
      .map(|(id, mut module)| {
        // Note: (Compat to rollup)
        // The `dynamic_importers/importers` should be added after `module_parsed` hook.
        for importer in std::mem::take(&mut self.intermediate_normal_modules.importers[id]) {
          if importer.kind.is_static() {
            module.importers.push(importer.importer_path);
          } else {
            module.dynamic_importers.push(importer.importer_path);
          }
        }
        module
      })
      .collect();

    let index_ecma_ast: IndexVec<EcmaModuleIdx, EcmaAst> =
      self.intermediate_normal_modules.index_ecma_ast.into_iter().flatten().collect();

    let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
    dynamic_import_entry_ids.sort_unstable_by_key(|id| &modules[*id].stable_resource_id);

    entry_points.extend(dynamic_import_entry_ids.into_iter().map(|id| EntryPoint {
      name: None,
      id,
      kind: EntryPointKind::DynamicImport,
    }));

    Ok(ModuleLoaderOutput {
      module_table: ModuleTable { ecma_modules: modules, external_modules: self.external_modules },
      symbols: self.symbols,
      index_ecma_ast,
      entry_points,
      runtime: runtime_brief.expect("Failed to find runtime module. This should not happen"),
      warnings: all_warnings,
      errors,
    })
  }
}
