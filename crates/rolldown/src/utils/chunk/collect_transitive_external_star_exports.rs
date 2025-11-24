use rolldown_common::{IndexModules, ModuleIdx};
use rustc_hash::FxHashSet;

/// Collects all external module indices that are transitively star-exported through internal modules.
///
/// This function performs a BFS traversal starting from the given entry module, following `export *`
/// statements through internal (normal) modules and collecting all external modules encountered.
///
/// # Example
/// Consider this module graph:
/// - `index.js`: `export * from './server.js'`
/// - `server.js`: `export * from 'external-lib'`
///
/// When called with `index.js` as the entry module, this function will:
/// 1. Start at `index.js`
/// 2. See it star-exports `server.js` (internal module), so add it to the queue
/// 3. Visit `server.js`
/// 4. See it star-exports `external-lib` (external module), so collect it
/// 5. Return `{external-lib}`
///
/// # Arguments
/// * `entry_module_idx` - The starting module for the traversal
/// * `module_table` - The module table to look up modules
///
/// # Returns
/// A set of external module indices that are transitively star-exported from the entry module.
///
/// # Related
/// This utility was extracted to avoid duplication across:
/// - `render_chunk_exports.rs` (issue #7115)
/// - `deconflict_chunk_symbols.rs` (issue #7115)
pub fn collect_transitive_external_star_exports(
  entry_module_idx: ModuleIdx,
  module_table: &IndexModules,
) -> FxHashSet<ModuleIdx> {
  let mut visited = FxHashSet::default();
  let mut queue = vec![entry_module_idx];
  let mut transitive_external_star_exports = FxHashSet::default();

  while let Some(module_idx) = queue.pop() {
    if !visited.insert(module_idx) {
      continue;
    }

    let rolldown_common::Module::Normal(module) = &module_table[module_idx] else {
      continue;
    };

    for star_export_idx in module.star_export_module_ids() {
      match &module_table[star_export_idx] {
        rolldown_common::Module::Normal(_) => {
          // Internal module - traverse it
          queue.push(star_export_idx);
        }
        rolldown_common::Module::External(_) => {
          // External module - collect it
          transitive_external_star_exports.insert(star_export_idx);
        }
      }
    }
  }

  transitive_external_star_exports
}
