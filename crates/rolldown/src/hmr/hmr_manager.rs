use arcstr::ArcStr;
use rolldown_common::{EcmaModuleAstUsage, Module, ModuleIdx, ModuleTable};
use rolldown_utils::indexmap::FxIndexMap;

pub struct HmrManager {
  module_db: ModuleTable,
  pub module_idx_by_abs_path: FxIndexMap<ArcStr, ModuleIdx>,
}

impl HmrManager {
  #[allow(clippy::dbg_macro)] // FIXME: Remove dbg! macro once the feature is stable
  pub fn generate_hmr_patch(&self, changed_file_paths: Vec<String>) -> String {
    let mut changed_modules = vec![];
    for changed_file_path in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path);
      match self.module_idx_by_abs_path.get(&changed_file_path) {
        Some(module_idx) => {
          changed_modules.push(*module_idx);
        }
        _ => {
          tracing::debug!("No corresponding module found for changed file path: {:?}", changed_file_path);
        }
      }
    }

    // Only changed modules might introduce new modules, we run a new module loader to fetch possible new modules and updated content of changed modules
    // TODO(hyf0): Run module loader

    let mut affected_modules = vec![];
    while let Some(changed_module_idx) = changed_modules.pop() {
      let Module::Normal(changed_module) = &self.module_db.modules[changed_module_idx] else {
        continue;
      };

      if changed_module.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept) {
        affected_modules.push(changed_module_idx);
        continue;
      }

      // TODO(hyf0): If it's not a self-accept module, we should traverse its dependents recursively
    }

    todo!()
  }
}
