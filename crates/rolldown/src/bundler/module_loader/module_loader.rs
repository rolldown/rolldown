use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId, RawPath, ResourceId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystemExt;
use rolldown_resolver::Resolver;
use rustc_hash::{FxHashMap, FxHashSet};

use super::normal_module_task::NormalModuleTask;
use super::runtime_normal_module_task::RuntimeNormalModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::bundler::graph::graph::Graph;
use crate::bundler::module::external_module::ExternalModule;
use crate::bundler::module::Module;
use crate::bundler::module_loader::module_task_context::ModuleTaskContext;
use crate::bundler::options::normalized_input_options::NormalizedInputOptions;
use crate::bundler::plugin_driver::SharedPluginDriver;
use crate::bundler::runtime::RUNTIME_PATH;
use crate::bundler::utils::ast_symbol::AstSymbol;
use crate::bundler::utils::resolve_id::{resolve_id, ResolvedRequestInfo};
use crate::error::{BatchedErrors, BatchedResult};
use crate::SharedResolver;

pub struct ModuleLoader<'a, T: FileSystemExt + Default> {
  ctx: ModuleLoaderContext,
  input_options: &'a NormalizedInputOptions,
  graph: &'a mut Graph,
  resolver: SharedResolver<T>,
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
  fs: Arc<T>,
  plugin_driver: SharedPluginDriver,
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
    graph: &mut Graph,
  ) -> ModuleId {
    match self.visited.entry(info.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = self.intermediate_modules.push(None);
        graph.symbols.add_ast_symbol(id, AstSymbol::default());
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
    input_options: &'a NormalizedInputOptions,
    plugin_driver: SharedPluginDriver,
    graph: &'a mut Graph,
    fs: Arc<T>,
  ) -> Self {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    Self {
      tx,
      rx,
      input_options,
      resolver: Resolver::with_cwd_and_fs(input_options.cwd.clone(), false, Arc::clone(&fs)).into(),
      graph,
      fs,
      ctx: ModuleLoaderContext::default(),
      plugin_driver,
    }
  }

  pub async fn fetch_all_modules(mut self) -> BatchedResult<()> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let resolved_entries = self.resolve_entries().await?;

    self.ctx.intermediate_modules.reserve(resolved_entries.len() + 1 /* runtime */);

    let shared_task_context = ModuleTaskContext {
      input_options: self.input_options,
      tx: &self.tx,
      resolver: &self.resolver,
      fs: &*self.fs,
      plugin_driver: &self.plugin_driver,
    };

    self.graph.runtime.id = self.ctx.try_spawn_runtime_normal_module_task(&shared_task_context);

    let mut entries = resolved_entries
      .into_iter()
      .map(|(name, info)| {
        (name, self.ctx.try_spawn_new_task(&shared_task_context, &info, true, self.graph))
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
            let id = self.ctx.try_spawn_new_task(&shared_task_context, &info, false, self.graph);
            let import_record = &mut import_records[import_record_idx];
            import_record.resolved_module = id;

            // dynamic import as extra entries if enable code splitting
            if import_record.kind == ImportKind::DynamicImport {
              dynamic_entries.insert((Some(info.path.unique(&self.input_options.cwd)), id));
            }
          });

          self.ctx.intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          self.graph.symbols.add_ast_symbol(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let NormalModuleTaskResult { module_id, ast_symbol, builder, .. } = task_result;

          let runtime_normal_module = builder.build();
          self.graph.runtime.init_symbols(&runtime_normal_module);
          self.ctx.intermediate_modules[module_id] = Some(Module::Normal(runtime_normal_module));

          self.graph.symbols.add_ast_symbol(module_id, ast_symbol);
        }
      }
      self.ctx.remaining -= 1;
    }

    self.graph.modules = self.ctx.intermediate_modules.into_iter().map(Option::unwrap).collect();

    let mut dynamic_entries = Vec::from_iter(dynamic_entries);
    dynamic_entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.extend(dynamic_entries);
    self.graph.entries = entries;
    Ok(())
  }

  #[allow(clippy::collection_is_never_read)]
  async fn resolve_entries(&mut self) -> BatchedResult<Vec<(Option<String>, ResolvedRequestInfo)>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids =
      futures::future::join_all(self.input_options.input.iter().map(|input_item| async move {
        let specifier = &input_item.import;
        match resolve_id(resolver, plugin_driver, specifier, None, false).await {
          Ok(r) => {
            let Some(info) = r else {
              return Err(BuildError::unresolved_entry(specifier));
            };

            if info.is_external {
              return Err(BuildError::entry_cannot_be_external(info.path.as_str()));
            }

            Ok((input_item.name.clone(), info))
          }
          Err(e) => Err(e),
        }
      }))
      .await;

    let mut errors = BatchedErrors::default();

    let collected =
      resolved_ids.into_iter().filter_map(|item| errors.take_err_from(item)).collect();

    if errors.is_empty() {
      Ok(collected)
    } else {
      Err(errors)
    }
  }
}
