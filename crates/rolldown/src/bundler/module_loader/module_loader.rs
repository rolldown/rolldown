use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId, RawPath, ResourceId};
use rolldown_error::BuildError;
use rolldown_resolver::Resolver;
use rolldown_utils::block_on_spawn_all;
use rustc_hash::{FxHashMap, FxHashSet};

use super::normal_module_task::NormalModuleTask;
use super::runtime_normal_module_task::RuntimeNormalModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::bundler::graph::graph::Graph;
use crate::bundler::graph::symbols::{AstSymbol, Symbols};
use crate::bundler::module::external_module::ExternalModule;
use crate::bundler::module::Module;
use crate::bundler::options::normalized_input_options::NormalizedInputOptions;
use crate::bundler::runtime::RUNTIME_PATH;
use crate::bundler::utils::resolve_id::{resolve_id, ResolvedRequestInfo};
use crate::error::{BatchedErrors, BatchedResult};
use crate::SharedResolver;

pub struct ModuleLoader<'a> {
  input_options: &'a NormalizedInputOptions,
  graph: &'a mut Graph,
  resolver: SharedResolver,
  visited: FxHashMap<RawPath, ModuleId>,
  remaining: u32,
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
}

impl<'a> ModuleLoader<'a> {
  pub fn new(input_options: &'a NormalizedInputOptions, graph: &'a mut Graph) -> Self {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    Self {
      tx,
      rx,
      input_options,
      resolver: Resolver::with_cwd(input_options.cwd.clone(), false).into(),
      visited: FxHashMap::default(),
      remaining: u32::default(),
      graph,
    }
  }

  pub async fn fetch_all_modules(&mut self) -> BatchedResult<()> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let resolved_entries = self.resolve_entries()?;

    let mut intermediate_modules: IndexVec<ModuleId, Option<Module>> =
      IndexVec::with_capacity(resolved_entries.len() + 1 /* runtime */);
    self.graph.runtime.id = self.try_spawn_runtime_normal_module_task(&mut intermediate_modules);

    let mut entries = resolved_entries
      .into_iter()
      .map(|(name, info)| (name, self.try_spawn_new_task(&info, &mut intermediate_modules, true)))
      .collect::<Vec<_>>();

    let mut dynamic_entries = FxHashSet::default();

    let mut tables: IndexVec<ModuleId, AstSymbol> = IndexVec::default();
    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };
      match msg {
        Msg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            module_id,
            ast_symbol: symbol_table,
            resolved_deps,
            mut builder,
            ..
          } = task_result;

          let import_records = builder.import_records.as_mut().unwrap();

          resolved_deps.into_iter().for_each(|(import_record_idx, info)| {
            let id = self.try_spawn_new_task(&info, &mut intermediate_modules, false);
            let import_record = &mut import_records[import_record_idx];
            import_record.resolved_module = id;
            while tables.len() <= id.raw() as usize {
              tables.push(AstSymbol::default());
            }
            // dynamic import as extra entries if enable code splitting
            if import_record.kind == ImportKind::DynamicImport {
              dynamic_entries.insert((Some(info.path.unique(&self.input_options.cwd)), id));
            }
          });

          while tables.len() <= task_result.module_id.raw() as usize {
            tables.push(AstSymbol::default());
          }
          intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          tables[task_result.module_id] = symbol_table;
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let NormalModuleTaskResult { module_id, ast_symbol: symbol_table, builder, .. } =
            task_result;
          while tables.len() <= task_result.module_id.raw() as usize {
            tables.push(AstSymbol::default());
          }
          let runtime_normal_module = builder.build();
          self.graph.runtime.init_symbols(&runtime_normal_module);
          intermediate_modules[module_id] = Some(Module::Normal(runtime_normal_module));

          tables[task_result.module_id] = symbol_table;
        }
      }
      self.remaining -= 1;
    }
    self.graph.symbols = Symbols::new(tables);

    self.graph.modules = intermediate_modules.into_iter().map(Option::unwrap).collect();

    let mut dynamic_entries = Vec::from_iter(dynamic_entries);
    dynamic_entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.extend(dynamic_entries);
    self.graph.entries = entries;
    Ok(())
  }

  #[allow(clippy::collection_is_never_read)]
  fn resolve_entries(&mut self) -> BatchedResult<Vec<(Option<String>, ResolvedRequestInfo)>> {
    let resolver = &self.resolver;

    let resolved_ids =
      block_on_spawn_all(self.input_options.input.iter().map(|input_item| async move {
        let specifier = &input_item.import;
        let resolve_id = resolve_id(resolver, specifier, None, false).await?;

        let Some(info) = resolve_id else {
          return Err(BuildError::unresolved_entry(specifier));
        };

        if info.is_external {
          return Err(BuildError::entry_cannot_be_external(info.path.as_str()));
        }

        Ok((input_item.name.clone(), info))
      }));

    let mut errors = BatchedErrors::default();

    let collected =
      resolved_ids.into_iter().filter_map(|item| errors.take_err_from(item)).collect();

    if errors.is_empty() {
      Ok(collected)
    } else {
      Err(errors)
    }
  }

  fn try_spawn_new_task(
    &mut self,
    info: &ResolvedRequestInfo,
    intermediate_modules: &mut IndexVec<ModuleId, Option<Module>>,
    is_entry: bool,
  ) -> ModuleId {
    match self.visited.entry(info.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = intermediate_modules.push(None);
        not_visited.insert(id);
        if info.is_external {
          let ext =
            ExternalModule::new(id, ResourceId::new(info.path.clone(), &self.input_options.cwd));
          intermediate_modules[id] = Some(Module::External(ext));
        } else {
          self.remaining += 1;

          let module_path = ResourceId::new(info.path.clone(), &self.input_options.cwd);

          let task = NormalModuleTask::new(
            id,
            is_entry,
            Arc::<rolldown_resolver::Resolver>::clone(&self.resolver),
            module_path,
            info.module_type,
            self.tx.clone(),
          );
          tokio::spawn(async move { task.run().await });
        }
        id
      }
    }
  }

  fn try_spawn_runtime_normal_module_task(
    &mut self,
    intermediate_modules: &mut IndexVec<ModuleId, Option<Module>>,
  ) -> ModuleId {
    match self.visited.entry(RUNTIME_PATH.to_string().into()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = intermediate_modules.push(None);
        not_visited.insert(id);
        self.remaining += 1;
        let task = RuntimeNormalModuleTask::new(id, Arc::clone(&self.resolver), self.tx.clone());
        tokio::spawn(async move { task.run() });
        id
      }
    }
  }
}
