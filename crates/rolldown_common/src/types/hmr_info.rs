use arcstr::ArcStr;
use oxc_str::CompactStr;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ImportRecordIdx, ModuleId};

#[derive(Debug, Default, Clone)]
pub struct HmrInfo {
  pub deps: FxHashSet<ModuleId>,
  pub module_request_to_import_record_idx: FxHashMap<ArcStr, ImportRecordIdx>,
  /// Export names passed to `import.meta.hot.acceptExports` as string literals. `None` when
  /// the module does not call it, or passes a value the scanner cannot read.
  pub accepted_exports: Option<FxHashSet<CompactStr>>,
}
