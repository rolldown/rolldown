use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId, RawPath, ResourceId};
use rolldown_resolver::Resolver;
use rolldown_utils::block_on_spawn_all;
use rustc_hash::{FxHashMap, FxHashSet};

use super::module_task::ModuleTask;
use super::task_result::TaskResult;
use super::Msg;
use crate::bundler::graph::symbols::{SymbolMap, Symbols};
use crate::bundler::module::external_module::ExternalModule;
use crate::bundler::module::module::Module;
use crate::bundler::module::module_id::ModuleVec;
use crate::bundler::options::normalized_input_options::NormalizedInputOptions;
use crate::bundler::resolve_id::{resolve_id, ResolvedRequestInfo};
use crate::bundler::runtime::{Runtime, RUNTIME_PATH};
use crate::BuildError;
use crate::SharedResolver;

pub struct ModuleLoader<'a> {
  input_options: &'a NormalizedInputOptions,
  resolver: SharedResolver,
  visited: FxHashMap<RawPath, ModuleId>,
  remaining: u32,
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
}

impl<'a> ModuleLoader<'a> {
  pub fn new(input_options: &'a NormalizedInputOptions) -> Self {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    Self {
      tx,
      rx,
      input_options,
      resolver: Resolver::with_cwd(input_options.cwd.clone(), false).into(),
      visited: Default::default(),
      remaining: Default::default(),
    }
  }

  pub async fn fetch_all_modules(
    &mut self,
  ) -> anyhow::Result<(Vec<(Option<String>, ModuleId)>, ModuleVec, Symbols, Runtime)> {
    if self.input_options.input.is_empty() {
      return Err(anyhow::format_err!(
        "You must supply options.input to rolldown"
      ));
    }

    let resolved_entries = self.resolve_entries().await?;

    let mut intermediate_modules: IndexVec<ModuleId, Option<Module>> =
      IndexVec::with_capacity(resolved_entries.len() + 1 /* runtime */);
    let mut runtime = Runtime::new(self.try_spawn_new_task(
      &ResolvedRequestInfo {
        path: RUNTIME_PATH.to_string().into(),
        is_external: false,
      },
      &mut intermediate_modules,
    ));
    let mut entries = resolved_entries
      .into_iter()
      .map(|(name, info)| {
        (
          name,
          self.try_spawn_new_task(&info, &mut intermediate_modules),
        )
      })
      .collect::<Vec<_>>();

    let mut dynamic_entries = FxHashSet::default();

    let mut tables: IndexVec<ModuleId, SymbolMap> = Default::default();
    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else { break };
      match msg {
        Msg::Done(task_result) => {
          let TaskResult {
            module_id,
            symbol_map: symbol_table,
            resolved_deps,
            mut builder,
            ..
          } = task_result;

          let import_records = builder.import_records.as_mut().unwrap();

          resolved_deps
            .into_iter()
            .for_each(|(import_record_idx, info)| {
              let id = self.try_spawn_new_task(&info, &mut intermediate_modules);
              let import_record = &mut import_records[import_record_idx];
              import_record.resolved_module = id;
              while tables.len() <= id.raw() as usize {
                tables.push(Default::default());
              }
              // dynamic import as extra entries if enable code splitting
              if import_record.kind == ImportKind::DynamicImport {
                dynamic_entries.insert((Some(info.path.unique(&self.input_options.cwd)), id));
              }
            });

          while tables.len() <= task_result.module_id.raw() as usize {
            tables.push(Default::default());
          }
          intermediate_modules[module_id] = Some(Module::Normal(builder.build()));

          tables[task_result.module_id] = symbol_table
        }
      }
      self.remaining -= 1;
    }
    let symbols = Symbols::new(tables);

    runtime.init_symbols(&symbols.tables[runtime.id]);

    let modules = intermediate_modules
      .into_iter()
      .map(|m| m.unwrap())
      .collect();

    let mut dynamic_entries = Vec::from_iter(dynamic_entries);
    dynamic_entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.extend(dynamic_entries);
    Ok((entries, modules, symbols, runtime))
  }

  pub async fn resolve_manual_chunk_modules(
    &self,
    modules: &[String],
  ) -> anyhow::Result<Vec<ModuleId>> {
    let resolve_results = self
      .resolve_dependencies(&modules.iter().collect::<Vec<_>>())
      .await;

    resolve_results.map(|results| {
      results
        .into_iter()
        .map(|(i, r)| {
          *self
            .visited
            .get(&r.path)
            .unwrap_or_else(|| panic!("The manual chunk module {} isn't imported", modules[i]))
        })
        .collect::<Vec<_>>()
    })
  }

  async fn resolve_dependencies(
    &self,
    deps: &[&String],
  ) -> anyhow::Result<Vec<(usize, ResolvedRequestInfo)>> {
    let resolver = &self.resolver;

    let resolved_ids = block_on_spawn_all(deps.iter().enumerate().map(
      |(index, specifier)| async move {
        let resolve_id = resolve_id(resolver, specifier, None, false).await.unwrap();

        let Some(info) = resolve_id else {
          return Err(BuildError::unresolved_entry(specifier))
        };

        if info.is_external {
          return Err(BuildError::entry_cannot_be_external(info.path.as_str()));
        }

        Ok((index, info))
      },
    ));

    let mut errors = vec![];

    let ret = resolved_ids
      .into_iter()
      .filter_map(|handle| match handle {
        Ok(id) => Some(id),
        Err(e) => {
          errors.push(e);
          None
        }
      })
      .collect();

    Ok(ret)
  }

  async fn resolve_entries(
    &mut self,
  ) -> anyhow::Result<Vec<(Option<String>, ResolvedRequestInfo)>> {
    let entry_modules = self
      .input_options
      .input
      .iter()
      .map(|i| &i.import)
      .collect::<Vec<_>>();

    let resolve_results = self.resolve_dependencies(&entry_modules).await;

    resolve_results.map(|results| {
      results
        .into_iter()
        .map(|(i, info)| (self.input_options.input[i].name.clone(), info))
        .collect::<Vec<_>>()
    })
  }

  fn try_spawn_new_task(
    &mut self,
    info: &ResolvedRequestInfo,
    intermediate_modules: &mut IndexVec<ModuleId, Option<Module>>,
  ) -> ModuleId {
    match self.visited.entry(info.path.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        let id = intermediate_modules.push(None);
        if info.is_external {
          let ext = ExternalModule::new(
            id,
            ResourceId::new(info.path.clone(), &self.input_options.cwd),
          );
          intermediate_modules[id] = Some(Module::External(ext));
        } else {
          not_visited.insert(id);

          self.remaining += 1;

          let module_path = ResourceId::new(info.path.clone(), &self.input_options.cwd);

          let task = ModuleTask::new(id, self.resolver.clone(), module_path, self.tx.clone());
          tokio::spawn(async move { task.run().await });
        }
        id
      }
    }
  }
}
