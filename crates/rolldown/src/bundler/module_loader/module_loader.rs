use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId, RawPath, ResourceId};
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
use crate::bundler::runtime::{Runtime, RUNTIME_PATH};
use crate::bundler::utils::ast_symbol::AstSymbol;
use crate::bundler::utils::resolve_id::ResolvedRequestInfo;
use crate::bundler::utils::symbols::Symbols;
use crate::error::BatchedResult;
use crate::SharedResolver;

pub struct ModuleLoader<T: FileSystem + Default> {
  ctx: ModuleLoaderContext,
  input_options: SharedInputOptions,
  common_data: ModuleTaskCommonData<T>,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
}

#[derive(Debug, Default)]
pub struct ModuleLoaderContext {
  visited: FxHashMap<RawPath, ModuleId>,
  remaining: u32,
  intermediate_modules: IndexVec<ModuleId, Option<Module>>,
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

    Self { common_data, rx, input_options, ctx: ModuleLoaderContext::default() }
  }

  fn try_spawn_new_task(
    &mut self,
    info: &ResolvedRequestInfo,
    is_entry: bool,
    symbols: &mut Symbols,
  ) -> ModuleId {
    match self.ctx.visited.entry(info.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = self.ctx.intermediate_modules.push(None);
        symbols.add_ast_symbol(id, AstSymbol::default());
        not_visited.insert(id);
        if info.is_external {
          let ext = ExternalModule::new(
            id,
            ResourceId::new(info.path.clone(), &self.common_data.input_options.cwd),
          );
          self.ctx.intermediate_modules[id] = Some(Module::External(ext));
        } else {
          self.ctx.remaining += 1;

          let module_path = ResourceId::new(info.path.clone(), &self.common_data.input_options.cwd);

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
    match self.ctx.visited.entry(RUNTIME_PATH.to_string().into()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = self.ctx.intermediate_modules.push(None);
        not_visited.insert(id);
        self.ctx.remaining += 1;
        let task = RuntimeNormalModuleTask::new(
          // safety: Data in `ModuleTaskContext` are alive as long as the `NormalModuleTask`, but rustc doesn't know that.
          unsafe { self.common_data.assume_static() },
          id,
        );
        tokio::spawn(async move { task.run() });
        id
      }
    }
  }

  pub async fn fetch_all_modules(
    mut self,
    resolved_entries: &[(Option<String>, ResolvedRequestInfo)],
    runtime: &mut Runtime,
  ) -> BatchedResult<(ModuleVec, Symbols, Vec<(Option<String>, ModuleId)>)> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    self.ctx.intermediate_modules.reserve(resolved_entries.len() + 1 /* runtime */);

    let mut symbols = Symbols::default();

    let mut entries: Vec<(Option<String>, ModuleId)> = resolved_entries
      .iter()
      .map(|(name, info)| (name.clone(), self.try_spawn_new_task(info, true, &mut symbols)))
      .collect::<Vec<_>>();

    let mut dynamic_entries = FxHashSet::default();

    while self.ctx.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };
      match msg {
        Msg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult { module_id, ast_symbol, resolved_deps, mut builder, .. } =
            task_result;

          let import_records = builder.import_records.as_mut().unwrap();

          resolved_deps.into_iter().for_each(|(import_record_idx, info)| {
            let id = self.try_spawn_new_task(&info, false, &mut symbols);
            let import_record = &mut import_records[import_record_idx];
            import_record.resolved_module = id;

            // dynamic import as extra entries if enable code splitting
            if import_record.kind == ImportKind::DynamicImport {
              dynamic_entries.insert((Some(info.path.unique(&self.input_options.cwd)), id));
            }
          });

          self.ctx.intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          symbols.add_ast_symbol(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let NormalModuleTaskResult { module_id, ast_symbol, builder, .. } = task_result;

          let runtime_normal_module = builder.build();
          runtime.init_symbols(&runtime_normal_module);
          self.ctx.intermediate_modules[module_id] = Some(Module::Normal(runtime_normal_module));

          symbols.add_ast_symbol(module_id, ast_symbol);
        }
      }
      self.ctx.remaining -= 1;
    }

    let modules = self.ctx.intermediate_modules.into_iter().map(Option::unwrap).collect();

    let mut dynamic_entries = Vec::from_iter(dynamic_entries);
    dynamic_entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.extend(dynamic_entries);
    Ok((modules, symbols, entries))
  }
}
