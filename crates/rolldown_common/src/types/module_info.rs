use arcstr::ArcStr;
use oxc_str::CompactStr;
use rolldown_utils::indexmap::FxIndexSet;

use crate::{ExportsKind, ModuleId, side_effects::HookSideEffects};

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<ArcStr>,
  pub id: ModuleId,
  pub is_entry: bool,
  pub importers: FxIndexSet<ModuleId>,
  pub dynamic_importers: FxIndexSet<ModuleId>,
  pub imported_ids: FxIndexSet<ModuleId>,
  pub dynamically_imported_ids: FxIndexSet<ModuleId>,
  pub exports: Vec<CompactStr>,
  pub input_format: ExportsKind,
  /// Effective `moduleSideEffects` (`boolean | 'no-treeshake'`). `None` only for the
  /// placeholder published before the module has been parsed.
  pub module_side_effects: Option<HookSideEffects>,
}
