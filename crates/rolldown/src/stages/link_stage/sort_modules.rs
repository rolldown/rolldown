use oxc_module_graph::LinkConfig;
use rolldown_common::Module;
use rolldown_error::{BuildDiagnostic, EventKindSwitcher};

use super::LinkStage;

impl LinkStage<'_> {
  /// Some notes about the module execution order:
  /// - We assume user-defined entries are always executed orderly.
  /// - Async entries is sorted by `Module#debug_id` of entry module to ensure deterministic output.
  /// - `require(...)` is treated as implicit static `import`, which required modules are executed before the module that requires them.
  /// - Since import statements are hoisted, `require(...)` is always placed after static `import` statements.
  /// - Order of `require(...)` is determined by who shows up first while scanning ast. For such code
  ///
  /// ```js
  /// () => require('b')
  /// require('c')
  /// import 'a';
  /// ```
  ///
  /// The execution order is `a -> b -> c`.
  /// - We only ensure execution order is relative correct, which means imported/required modules are executed before the module that imports/require them.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn sort_modules(&mut self) {
    let config = LinkConfig {
      include_dynamic_imports: self.options.code_splitting.is_disabled(),
      ..Default::default()
    };

    let result = oxc_module_graph::compute_exec_order(&self.link_kernel.graph, &config);

    // Sync exec_order to Rolldown's module_table before apply() consumes the result.
    for (next_exec_order, &oxc_idx) in (0_u32..).zip(result.sorted.iter()) {
      let idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      match &mut self.module_table[idx] {
        Module::Normal(module) => {
          debug_assert!(module.exec_order == u32::MAX);
          module.exec_order = next_exec_order;
        }
        Module::External(module) => {
          debug_assert!(module.exec_order == u32::MAX);
          module.exec_order = next_exec_order;
        }
      }
    }

    // Build sorted_modules (Normal only, matching previous behavior).
    self.sorted_modules = result
      .sorted
      .iter()
      .map(|&oxc_idx| rolldown_common::ModuleIdx::from_usize(oxc_idx.index()))
      .filter(|idx| self.module_table[*idx].as_normal().is_some())
      .collect();

    // Emit circular dependency warnings before apply() consumes result.cycles.
    if self.options.checks.contains(EventKindSwitcher::CircularDependency)
      && !result.cycles.is_empty()
    {
      for cycle in &result.cycles {
        let paths = cycle
          .iter()
          .filter_map(|oxc_idx| {
            let idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
            self.module_table[idx].as_normal().map(|module| module.id.to_string())
          })
          .collect::<Vec<_>>();
        self.warnings.push(BuildDiagnostic::circular_dependency(paths).with_severity_warning());
      }
    }

    // Apply to graph (writes exec_order on graph modules, stores sorted/cycles internally).
    result.apply(&mut self.link_kernel.graph);

    debug_assert_eq!(
      self.sorted_modules.first().copied(),
      Some(self.runtime.id()),
      "runtime module should always be the first module in the sorted modules"
    );
  }
}
