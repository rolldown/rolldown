use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{
  EntryPoint, EntryPointKind, FilePath, ImportKind, ImportRecordId, ModuleId, ResourceId,
};
use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use rustc_hash::{FxHashMap, FxHashSet};

use super::normal_module_task::NormalModuleTask;
use super::runtime_normal_module_task::RuntimeNormalModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::bundler::module::external_module::ExternalModule;
use crate::bundler::module::{Module, ModuleVec};
use crate::bundler::module_loader::module_task_context::ModuleTaskCommonData;
use crate::bundler::module_loader::runtime_normal_module_task::RuntimeNormalModuleTaskResult;
use crate::bundler::options::input_options::SharedInputOptions;
use crate::bundler::plugin_driver::SharedPluginDriver;
use crate::bundler::runtime::RuntimeModuleBrief;
use crate::bundler::utils::resolve_id::ResolvedRequestInfo;
use crate::bundler::utils::symbols::Symbols;
use crate::error::{BatchedErrors, BatchedResult};
use crate::SharedResolver;

pub struct ModuleLoader<T: FileSystem + Default> {
  input_options: SharedInputOptions,
  common_data: ModuleTaskCommonData<T>,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
  visited: FxHashMap<FilePath, ModuleId>,
  runtime_id: Option<ModuleId>,
  remaining: u32,
  intermediate_modules: IndexVec<ModuleId, Option<Module>>,
  symbols: Symbols,
}

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub modules: ModuleVec,
  pub symbols: Symbols,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
}

impl<T: FileSystem + 'static + Default> ModuleLoader<T> {
  pub fn new(
    input_options: SharedInputOptions,
    plugin_driver: SharedPluginDriver,
    fs: T,
    resolver: SharedResolver<T>,
  ) -> Self {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    let common_data = ModuleTaskCommonData {
      input_options: Arc::clone(&input_options),
      tx,
      resolver,
      fs,
      plugin_driver,
    };

    Self {
      common_data,
      rx,
      input_options,
      visited: FxHashMap::default(),
      runtime_id: None,
      remaining: 0,
      intermediate_modules: IndexVec::new(),
      symbols: Symbols::default(),
    }
  }

  fn alloc_module_id(
    intermediate_modules: &mut IndexVec<ModuleId, Option<Module>>,
    symbols: &mut Symbols,
  ) -> ModuleId {
    let id = intermediate_modules.push(None);
    symbols.alloc_one();
    id
  }

  fn try_spawn_new_task(&mut self, info: ResolvedRequestInfo) -> ModuleId {
    match self.visited.entry(info.path.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = Self::alloc_module_id(&mut self.intermediate_modules, &mut self.symbols);
        not_visited.insert(id);
        if info.is_external {
          let ext = ExternalModule::new(id, ResourceId::new(info.path.path));
          self.intermediate_modules[id] = Some(Module::External(ext));
        } else {
          self.remaining += 1;

          let module_path = info.path.clone();

          let task = NormalModuleTask::new(
            // safety: Data in `ModuleTaskContext` are alive as long as the `NormalModuleTask`, but rustc doesn't know that.
            unsafe { self.common_data.assume_static() },
            id,
            module_path,
            info.module_type,
          );
          tokio::spawn(async move { task.run().await });
        }
        id
      }
    }
  }

  pub fn try_spawn_runtime_module_task(&mut self) -> ModuleId {
    *self.runtime_id.get_or_insert_with(|| {
      let id = Self::alloc_module_id(&mut self.intermediate_modules, &mut self.symbols);
      self.remaining += 1;
      let task = RuntimeNormalModuleTask::new(id, self.common_data.tx.clone());
      tokio::spawn(async move { task.run() });
      id
    })
  }

  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<String>, ResolvedRequestInfo)>,
  ) -> BatchedResult<ModuleLoaderOutput> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let mut errors = BatchedErrors::default();
    let mut all_warnings: Vec<BuildError> = Vec::new();

    self.intermediate_modules.reserve(user_defined_entries.len() + 1 /* runtime */);

    // Store the already consider as entry module
    let mut user_defined_entry_ids = {
      let mut tmp = FxHashSet::default();
      tmp.reserve(user_defined_entries.len());
      tmp
    };

    let mut entry_points = user_defined_entries
      .into_iter()
      .map(|(name, info)| EntryPoint {
        name,
        id: self.try_spawn_new_task(info),
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
            mut builder,
            raw_import_records,
            warnings,
          } = task_result;
          all_warnings.extend(warnings);
          let import_records = raw_import_records
            .into_iter()
            .zip(resolved_deps)
            .map(|(raw_rec, info)| {
              let id = self.try_spawn_new_task(info);
              // Dynamic imported module will be considered as an entry
              if matches!(raw_rec.kind, ImportKind::DynamicImport)
                && !user_defined_entry_ids.contains(&id)
              {
                dynamic_import_entry_ids.insert(id);
              }
              raw_rec.into_import_record(id)
            })
            .collect::<IndexVec<ImportRecordId, _>>();
          builder.import_records = Some(import_records);
          builder.is_user_defined_entry = Some(user_defined_entry_ids.contains(&module_id));
          self.intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          self.symbols.add_ast_symbol(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeNormalModuleTaskResult { ast_symbol, builder, runtime, warnings: _ } =
            task_result;

          self.intermediate_modules[runtime.id()] = Some(Module::Normal(builder.build()));
          self.symbols.add_ast_symbol(runtime.id(), ast_symbol);
          runtime_brief = Some(runtime);
        }
        Msg::Errors(errs) => {
          errors.merge(errs);
        }
      }
      self.remaining -= 1;
    }

    if !errors.is_empty() {
      return Err(errors);
    }

    let modules: IndexVec<ModuleId, Module> =
      self.intermediate_modules.into_iter().map(Option::unwrap).collect();

    let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
    dynamic_import_entry_ids.sort_by_key(|id| modules[*id].resource_id());

    entry_points.extend(dynamic_import_entry_ids.into_iter().map(|id| EntryPoint {
      name: None,
      id,
      kind: EntryPointKind::DynamicImport,
    }));

    Ok(ModuleLoaderOutput {
      modules,
      symbols: self.symbols,
      entry_points,
      runtime: runtime_brief.expect("Failed to find runtime module. This should not happen"),
      warnings: all_warnings,
    })
  }
}
