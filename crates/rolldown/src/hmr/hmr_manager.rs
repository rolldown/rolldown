use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, ModuleTable};
use rolldown_utils::indexmap::FxIndexMap;

pub struct HmrManager {
  _modules: ModuleTable,
  pub _module_idx_by_abs_path: FxIndexMap<ArcStr, ModuleIdx>,
}

impl HmrManager {
  pub fn generate_hmr_patch(&self, _changed_file_paths: Vec<String>) -> String {
    todo!()
  }
}
