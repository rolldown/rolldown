use arcstr::ArcStr;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ImportRecordIdx, ModuleId};

#[derive(Debug, Default, Clone)]
pub struct HmrInfo {
  pub deps: FxHashSet<ModuleId>,
  pub module_request_to_import_record_idx: FxHashMap<ArcStr, ImportRecordIdx>,
}
