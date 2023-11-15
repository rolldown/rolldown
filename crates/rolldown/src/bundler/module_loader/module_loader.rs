use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{FilePath, ImportKind, ImportRecordId, ModuleId, ResourceId};
use rolldown_fs::FileSystem;
use rustc_hash::{FxHashMap, FxHashSet};

use super::normal_module_task::NormalModuleTask;
use super::runtime_normal_module_task::RuntimeNormalModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::bundler::module::external_module::ExternalModule;
use crate::bundler::module::{Module, ModuleVec};
use crate::bundler::module_loader::module_task_context::ModuleTaskCommonData;
use crate::bundler::options::input_options::SharedInputOptions;
use crate::bundler::plugin_driver::SharedPluginDriver;
use crate::bundler::runtime::Runtime;
use crate::bundler::utils::resolve_id::ResolvedRequestInfo;
use crate::bundler::utils::symbols::Symbols;
use crate::error::BatchedResult;
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

  fn try_spawn_new_task(&mut self, info: &ResolvedRequestInfo, is_entry: bool) -> ModuleId {
    match self.visited.entry(info.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = Self::alloc_module_id(&mut self.intermediate_modules, &mut self.symbols);
        not_visited.insert(id);
        if info.is_external {
          let ext = ExternalModule::new(id, ResourceId::new(info.path.clone()));
          self.intermediate_modules[id] = Some(Module::External(ext));
        } else {
          self.remaining += 1;

          let module_path = info.path.clone();

          let task = NormalModuleTask::new(
            // safety: Data in `ModuleTaskContext` are alive as long as the `NormalModuleTask`, but rustc doesn't know that.
            unsafe { self.common_data.assume_static() },
            id,
            is_entry,
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
    user_defined_entries: &[(Option<String>, ResolvedRequestInfo)],
    runtime: &mut Runtime,
  ) -> BatchedResult<(ModuleVec, Symbols, Vec<(Option<String>, ModuleId)>)> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    self.intermediate_modules.reserve(user_defined_entries.len() + 1 /* runtime */);

    let mut entries: Vec<(Option<String>, ModuleId)> = user_defined_entries
      .iter()
      .map(|(name, info)| (name.clone(), self.try_spawn_new_task(info, true)))
      .collect::<Vec<_>>();

    let mut dynamic_entries = FxHashSet::default();

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
            ..
          } = task_result;

          let import_records = raw_import_records
            .into_iter()
            .zip(resolved_deps)
            .map(|(raw_rec, info)| {
              let id = self.try_spawn_new_task(&info, false);
              // dynamic import as extra entries if enable code splitting
              if raw_rec.kind == ImportKind::DynamicImport {
                dynamic_entries.insert((Some(info.path.unique(&self.input_options.cwd)), id));
              }
              raw_rec.into_import_record(id)
            })
            .collect::<IndexVec<ImportRecordId, _>>();
          builder.import_records = Some(import_records);
          builder.pretty_path =
            Some(builder.path.as_ref().unwrap().prettify(&self.input_options.cwd));
          self.intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          self.symbols.add_ast_symbol(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let NormalModuleTaskResult { module_id, ast_symbol, builder, .. } = task_result;

          let runtime_normal_module = builder.build();
          runtime.init_symbols(&runtime_normal_module);
          self.intermediate_modules[module_id] = Some(Module::Normal(runtime_normal_module));

          self.symbols.add_ast_symbol(module_id, ast_symbol);
        }
      }
      self.remaining -= 1;
    }

    let modules = self.intermediate_modules.into_iter().map(Option::unwrap).collect();

    let mut dynamic_entries = Vec::from_iter(dynamic_entries);
    dynamic_entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.extend(dynamic_entries);
    Ok((modules, self.symbols, entries))
  }
}
