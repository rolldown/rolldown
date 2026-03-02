use super::LinkStage;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn compute_tla(&mut self) {
    if self.tla_module_count == 0 {
      return;
    }

    let tla_modules = oxc_module_graph::compute_tla(&self.link_kernel.graph);
    for oxc_idx in &tla_modules {
      let idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      self.metas[idx].is_tla_or_contains_tla_dependency = true;
    }
  }
}
