use rolldown_common::{ClientHmrInput, ImportKind, IndexModules, Module, ModuleIdx};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashMap;

use super::hmr_ast_finalizer::ModuleInitializerMode;

#[derive(Debug)]
pub(super) struct HmrRenderPlan {
  pub(super) modules_to_render: FxIndexSet<ModuleIdx>,
  modules_to_reexecute: FxIndexSet<ModuleIdx>,
  modules_to_invoke_if_loaded: FxIndexSet<ModuleIdx>,
}

impl HmrRenderPlan {
  pub(super) fn new(
    mut modules_to_reexecute: FxIndexSet<ModuleIdx>,
    modules_to_invoke_if_loaded: FxIndexSet<ModuleIdx>,
  ) -> Self {
    modules_to_reexecute.extend(modules_to_invoke_if_loaded.iter().copied());
    let modules_to_render = modules_to_reexecute.clone();
    Self { modules_to_render, modules_to_reexecute, modules_to_invoke_if_loaded }
  }

  pub(super) fn complete_for_client(
    &mut self,
    modules: &IndexModules,
    actual_updates: &FxIndexSet<ModuleIdx>,
    client: &ClientHmrInput<'_>,
  ) {
    let mut esm_importers_by_dependency = FxHashMap::<ModuleIdx, Vec<ModuleIdx>>::default();
    let mut stack = self.modules_to_render.iter().copied().collect::<Vec<_>>();

    for module_idx in actual_updates {
      if self.modules_to_render.contains(module_idx) {
        self.invoke_if_loaded(*module_idx);
      }
    }

    // Complete the definition closure first. Static ESM imports, require calls, and dynamic
    // imports need an initializer in the patch, but only a static ESM edge proves that the
    // dependency executes with its importer.
    while let Some(importer_idx) = stack.pop() {
      let Module::Normal(importer) = &modules[importer_idx] else {
        continue;
      };

      for rec in &importer.import_records {
        if !matches!(rec.kind, ImportKind::Import | ImportKind::Require | ImportKind::DynamicImport)
        {
          continue;
        }
        let Some(dependency_idx) = rec.resolved_module else {
          continue;
        };
        let Module::Normal(dependency) = &modules[dependency_idx] else {
          continue;
        };

        if rec.kind == ImportKind::Import {
          esm_importers_by_dependency.entry(dependency_idx).or_default().push(importer_idx);
        }

        let added = if actual_updates.contains(&dependency_idx) {
          self.reexecute_and_invoke_if_loaded(dependency_idx)
        } else if !client.is_module_executed(&dependency.stable_id) {
          self.backfill(dependency_idx)
        } else {
          false
        };

        if added {
          stack.push(dependency_idx);
        }
      }
    }

    // A deduplicated static ESM importer would skip its factory and never invoke a real update
    // below it, so upgrade the rendered ESM path between each actual update and its HMR root.
    // Do not upgrade require or dynamic-import paths: either call may be conditional.
    let mut stack = actual_updates
      .iter()
      .copied()
      .filter(|module_idx| self.modules_to_render.contains(module_idx))
      .collect::<Vec<_>>();

    while let Some(dependency_idx) = stack.pop() {
      if let Some(importers) = esm_importers_by_dependency.get(&dependency_idx) {
        for importer_idx in importers.iter().copied() {
          if self.reexecute_and_invoke_if_loaded(importer_idx) {
            stack.push(importer_idx);
          }
        }
      }
    }
  }

  pub(super) fn initializer_mode(&self, module_idx: ModuleIdx) -> ModuleInitializerMode {
    if self.modules_to_reexecute.contains(&module_idx) {
      ModuleInitializerMode::Always
    } else {
      ModuleInitializerMode::Deduplicate
    }
  }

  pub(super) fn modules_to_invoke_if_loaded(&self) -> &FxIndexSet<ModuleIdx> {
    &self.modules_to_invoke_if_loaded
  }

  fn backfill(&mut self, module_idx: ModuleIdx) -> bool {
    self.modules_to_render.insert(module_idx)
  }

  fn reexecute(&mut self, module_idx: ModuleIdx) -> bool {
    let upgraded = self.modules_to_reexecute.insert(module_idx);
    self.modules_to_render.insert(module_idx);
    upgraded
  }

  fn invoke_if_loaded(&mut self, module_idx: ModuleIdx) -> bool {
    self.modules_to_invoke_if_loaded.insert(module_idx)
  }

  fn reexecute_and_invoke_if_loaded(&mut self, module_idx: ModuleIdx) -> bool {
    self.invoke_if_loaded(module_idx);
    self.reexecute(module_idx)
  }
}
