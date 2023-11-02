use super::{linker::Linker, linker_info::LinkingInfoVec, symbols::Symbols};
use crate::bundler::{
  module::ModuleVec, module_loader::ModuleLoader,
  options::normalized_input_options::NormalizedInputOptions, runtime::Runtime,
};
use rolldown_common::ModuleId;
use rustc_hash::FxHashSet;

#[derive(Default, Debug)]
pub struct Graph {
  pub modules: ModuleVec,
  pub linking_infos: LinkingInfoVec,
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub sorted_modules: Vec<ModuleId>,
  pub symbols: Symbols,
  pub runtime: Runtime,
}

impl Graph {
  pub async fn generate_module_graph(
    &mut self,
    input_options: &NormalizedInputOptions,
  ) -> anyhow::Result<()> {
    ModuleLoader::new(input_options, self).fetch_all_modules().await?;

    tracing::trace!("{:#?}", self);

    self.sort_modules();

    self.link();

    Ok(())
  }

  #[allow(clippy::items_after_statements)]
  pub fn sort_modules(&mut self) {
    let mut sorted_modules = Vec::with_capacity(self.modules.len());
    let mut next_exec_order = 0;
    let mut visited_modules = FxHashSet::default();

    // The runtime module should always be the first module to be executed
    enter_module(
      &mut self.modules,
      self.runtime.id,
      &mut visited_modules,
      &mut next_exec_order,
      &mut sorted_modules,
    );

    for (_, id) in &self.entries {
      enter_module(
        &mut self.modules,
        *id,
        &mut visited_modules,
        &mut next_exec_order,
        &mut sorted_modules,
      );
    }

    self.sorted_modules = sorted_modules;
    debug_assert_eq!(
      self.sorted_modules.first().copied(),
      Some(self.runtime.id),
      "runtime module should always be the first module in the sorted modules"
    );

    fn enter_module(
      modules: &mut ModuleVec,
      id: ModuleId,
      visited_modules: &mut FxHashSet<ModuleId>,
      next_exec_order: &mut u32,
      sorted_modules: &mut Vec<ModuleId>,
    ) {
      if !visited_modules.contains(&id) {
        visited_modules.insert(id);
        let child_module_ids = modules[id]
          .import_records()
          .iter()
          .filter_map(|rec| {
            (rec.kind.is_static() && rec.resolved_module.is_valid()).then_some(rec.resolved_module)
          })
          .collect::<Vec<_>>();
        for child_module_id in child_module_ids {
          enter_module(modules, child_module_id, visited_modules, next_exec_order, sorted_modules);
        }
        *modules[id].exec_order_mut() = *next_exec_order;
        sorted_modules.push(id);
        *next_exec_order += 1;
      }
    }
  }

  pub fn link(&mut self) {
    Linker::new(self).link();
  }
}
