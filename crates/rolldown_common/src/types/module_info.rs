use arcstr::ArcStr;
use oxc_str::CompactStr;
use rolldown_utils::indexmap::FxIndexSet;

use crate::{ExportsKind, ModuleId, side_effects::DeterminedSideEffects};

#[derive(Debug, Clone)]
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
  pub side_effects: Option<ModuleInfoSideEffects>,
}

/// The values behind `getModuleInfo().moduleSideEffects`. Both are declared values, not results of
/// the statement analysis. A value that a plugin wrote is kept apart, in
/// [`ModuleSideEffectsOverrides`](crate::ModuleSideEffectsOverrides), and replaces `decided` when
/// the info is read. See internal-docs/cache/implementation.md.
#[derive(Debug, Clone, Copy)]
pub struct ModuleInfoSideEffects {
  /// The last hook value, or the written value once it is applied. `None`: nothing decided, or a
  /// write of `null`, so the info shows `without_hook`.
  pub decided: Option<DeterminedSideEffects>,
  pub without_hook: SideEffectsWithoutHook,
}

/// The value `getModuleInfo().moduleSideEffects` shows when no hook decided.
#[derive(Debug, Clone, Copy)]
pub enum SideEffectsWithoutHook {
  Known(bool),
  /// `treeshake.moduleSideEffects` is a function. It runs for a module only when this value or
  /// the tree-shaking value without hooks is needed, and
  /// [`ModuleSideEffectsOverrides`](crate::ModuleSideEffectsOverrides) keeps its result. When it
  /// returns no boolean, the value is `fallback`.
  OptionFn {
    fallback: bool,
    external: bool,
  },
}

impl ModuleInfoSideEffects {
  /// The shown value. `None` only while it needs a `treeshake.moduleSideEffects` call that has not
  /// run yet.
  pub fn shown(&self) -> Option<DeterminedSideEffects> {
    self.decided.or(match self.without_hook {
      SideEffectsWithoutHook::Known(value) => Some(DeterminedSideEffects::UserDefined(value)),
      SideEffectsWithoutHook::OptionFn { .. } => None,
    })
  }
}

impl ModuleInfo {
  /// The info of a module whose task has not parsed it yet.
  pub fn placeholder(
    id: ModuleId,
    is_entry: bool,
    side_effects: Option<ModuleInfoSideEffects>,
  ) -> Self {
    Self {
      code: None,
      id,
      is_entry,
      importers: FxIndexSet::default(),
      dynamic_importers: FxIndexSet::default(),
      imported_ids: FxIndexSet::default(),
      dynamically_imported_ids: FxIndexSet::default(),
      exports: vec![],
      input_format: ExportsKind::None,
      side_effects,
    }
  }
}
