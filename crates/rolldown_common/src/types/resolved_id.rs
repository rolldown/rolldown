use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_utils::{dataurl::is_data_url, stabilize_id::stabilize_id};

use super::module_id::ModuleId;
use crate::{ImportAttributes, ModuleDefFormat, PackageJson, side_effects::HookSideEffects};

#[derive(Debug, Clone, Copy)]
pub enum ResolvedExternal {
  Bool(bool),
  Absolute,
  Relative,
}

impl Default for ResolvedExternal {
  fn default() -> Self {
    ResolvedExternal::Bool(false)
  }
}

impl ResolvedExternal {
  pub fn is_external(&self) -> bool {
    match self {
      ResolvedExternal::Bool(b) => *b,
      _ => true,
    }
  }
}

impl From<bool> for ResolvedExternal {
  fn from(b: bool) -> Self {
    ResolvedExternal::Bool(b)
  }
}

#[derive(Debug, Default, Clone)]
pub struct ResolvedId {
  pub id: ModuleId,
  pub module_def_format: ModuleDefFormat,
  pub external: ResolvedExternal,
  // If the js side is return object, the relative id is finally id, else it will be converted to an absolute id
  pub normalize_external_id: Option<bool>,
  pub package_json: Option<Arc<PackageJson>>,
  pub side_effects: Option<HookSideEffects>,
  pub is_external_without_side_effects: bool,
  /// The import attributes that are part of the module identity. Always empty unless
  /// `experimental.importAttributes` is enabled.
  pub attributes: ImportAttributes,
}

impl ResolvedId {
  /// Finishes a resolution: the attributes the import resolved with become part of the module
  /// id when `experimental.importAttributes` is enabled, and are dropped otherwise.
  #[must_use]
  pub fn finalize_attributes(mut self, import_attributes_enabled: bool) -> Self {
    let attributes = std::mem::take(&mut self.attributes);
    if import_attributes_enabled { self.with_attributes(attributes) } else { self }
  }

  /// Makes `attributes` part of the module id, replacing the ones it had.
  /// See internal-docs/import-attributes/implementation.md
  #[must_use]
  pub fn with_attributes(mut self, attributes: ImportAttributes) -> Self {
    if !attributes.is_empty() || !self.attributes.is_empty() {
      self.id = ModuleId::with_attributes(self.id.raw_id(), &attributes);
    }
    self.attributes = attributes;
    self
  }

  /// Create a dummy ResolvedId, which is not exists in the file system
  /// note: A dummy `ResolvedId` usually used with `DUMMY_MODULE_IDX`
  pub fn make_dummy() -> Self {
    Self {
      id: ModuleId::default(),
      module_def_format: ModuleDefFormat::Unknown,
      external: false.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: false,
      attributes: ImportAttributes::default(),
    }
  }

  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    if is_data_url(&self.id) {
      return format!("<{}>", self.id.as_str());
    }

    if let Some(original) = self.id.strip_empty_prefix() {
      return format!("(ignored) {}", stabilize_id(original, cwd.as_ref()));
    }

    stabilize_id(&self.id, cwd.as_ref())
  }

  pub fn new_external_without_side_effects(id: ArcStr) -> Self {
    Self {
      id: ModuleId::new(id),
      module_def_format: ModuleDefFormat::Unknown,
      external: true.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: true,
      attributes: ImportAttributes::default(),
    }
  }
}
