use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId, RawPath, ResourceId};
use rolldown_fs::FileSystemExt;
use rolldown_resolver::Resolver;
use rustc_hash::{FxHashMap, FxHashSet};

use super::normal_module_task::NormalModuleTask;
use super::runtime_normal_module_task::RuntimeNormalModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::bundler::graph::symbols::Symbols;
use crate::bundler::module::external_module::ExternalModule;
use crate::bundler::module::{Module, ModuleVec};
use crate::bundler::module_loader::module_task_context::ModuleTaskContext;
use crate::bundler::options::input_options::InputOptions;
use crate::bundler::plugin_driver::SharedPluginDriver;
use crate::bundler::runtime::{Runtime, RUNTIME_PATH};
use crate::bundler::utils::ast_symbol::AstSymbol;
use crate::bundler::utils::resolve_id::ResolvedRequestInfo;
use crate::error::BatchedResult;
use crate::SharedResolver;

pub struct ModuleLoader<'a, T: FileSystemExt + Default> {
  ctx: ModuleLoaderContext,
  input_options: &'a InputOptions,
  resolver: SharedResolver<T>,
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
  fs: Arc<T>,
  plugin_driver: SharedPluginDriver,
  symbols: Symbols,
  runtime: Runtime,
}

#[derive(Debug, Default)]
pub struct ModuleLoaderContext {
  visited: FxHashMap<RawPath, ModuleId>,
  remaining: u32,
  intermediate_modules: IndexVec<ModuleId, Option<Module>>,
}

impl ModuleLoaderContext {
  fn try_spawn_runtime_normal_module_task<T: FileSystemExt + Default + 'static>(
    &mut self,
    task_context: &ModuleTaskContext<T>,
  ) -> ModuleId {
    match self.visited.entry(RUNTIME_PATH.to_string().into()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = self.intermediate_modules.push(None);
        not_visited.insert(id);
        self.remaining += 1;
        let task = RuntimeNormalModuleTask::new(
          // safety: Data in `ModuleTaskContext` are alive as long as the `NormalModuleTask`, but rustc doesn't know that.
          unsafe { task_context.assume_static() },
          id,
        );
        tokio::spawn(async move { task.run() });
        id
      }
    }
  }

  fn try_spawn_new_task<T: FileSystemExt + Default + 'static>(
    &mut self,
    module_task_context: &ModuleTaskContext<T>,
    info: &ResolvedRequestInfo,
    is_entry: bool,
    symbols: &mut Symbols,
  ) -> ModuleId {
    match self.visited.entry(info.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = self.intermediate_modules.push(None);
        symbols.add_ast_symbol(id, AstSymbol::default());
        not_visited.insert(id);
        if info.is_external {
          let ext = ExternalModule::new(
            id,
            ResourceId::new(info.path.clone(), &module_task_context.input_options.cwd),
          );
          self.intermediate_modules[id] = Some(Module::External(ext));
        } else {
          self.remaining += 1;

          let module_path =
            ResourceId::new(info.path.clone(), &module_task_context.input_options.cwd);

          let task = NormalModuleTask::new(
            // safety: Data in `ModuleTaskContext` are alive as long as the `NormalModuleTask`, but rustc doesn't know that.
            unsafe { module_task_context.assume_static() },
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
}

impl<'a, T: FileSystemExt + 'static + Default> ModuleLoader<'a, T> {
  pub fn new(
    input_options: &'a InputOptions,
    plugin_driver: SharedPluginDriver,
    fs: Arc<T>,
  ) -> Self {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    Self {
      tx,
      rx,
      input_options,
      resolver: Resolver::with_cwd_and_fs(input_options.cwd.clone(), false, Arc::clone(&fs)).into(),
      fs,
      ctx: ModuleLoaderContext::default(),
      plugin_driver,
      symbols: Symbols::default(),
      runtime: Runtime::default(),
    }
  }

  pub async fn fetch_all_modules(
    mut self,
    resolved_entries: &[(Option<String>, ResolvedRequestInfo)],
  ) -> BatchedResult<(ModuleVec, Runtime, Symbols, Vec<(Option<String>, ModuleId)>)> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    self.ctx.intermediate_modules.reserve(resolved_entries.len() + 1 /* runtime */);

    let shared_task_context = ModuleTaskContext {
      input_options: self.input_options,
      tx: &self.tx,
      resolver: &self.resolver,
      fs: &*self.fs,
      plugin_driver: &self.plugin_driver,
    };

    self.runtime.id = self.ctx.try_spawn_runtime_normal_module_task(&shared_task_context);

    let mut entries: Vec<(Option<String>, ModuleId)> = resolved_entries
      .iter()
      .map(|(name, info)| {
        (
          name.clone(),
          self.ctx.try_spawn_new_task(&shared_task_context, info, true, &mut self.symbols),
        )
      })
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
            let id =
              self.ctx.try_spawn_new_task(&shared_task_context, &info, false, &mut self.symbols);
            let import_record = &mut import_records[import_record_idx];
            import_record.resolved_module = id;

            // dynamic import as extra entries if enable code splitting
            if import_record.kind == ImportKind::DynamicImport {
              dynamic_entries.insert((Some(info.path.unique(&self.input_options.cwd)), id));
            }
          });

          self.ctx.intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          self.symbols.add_ast_symbol(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let NormalModuleTaskResult { module_id, ast_symbol, builder, .. } = task_result;

          let runtime_normal_module = builder.build();
          self.runtime.init_symbols(&runtime_normal_module);
          self.ctx.intermediate_modules[module_id] = Some(Module::Normal(runtime_normal_module));

          self.symbols.add_ast_symbol(module_id, ast_symbol);
        }
      }
      self.ctx.remaining -= 1;
    }

    let modules = self.ctx.intermediate_modules.into_iter().map(Option::unwrap).collect();

    let mut dynamic_entries = Vec::from_iter(dynamic_entries);
    dynamic_entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.extend(dynamic_entries);
    Ok((modules, self.runtime, self.symbols, entries))
  }
}
