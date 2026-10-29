use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_utils::{
  concat_string, dataurl::is_data_url, ecmascript::escape_line_terminators,
  stabilize_id::stabilize_id,
};

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
  pub attributes: ImportAttributes,
  pub module_def_format: ModuleDefFormat,
  pub external: ResolvedExternal,
  // If the js side is return object, the relative id is finally id, else it will be converted to an absolute id
  pub normalize_external_id: Option<bool>,
  pub package_json: Option<Arc<PackageJson>>,
  pub side_effects: Option<HookSideEffects>,
  pub is_external_without_side_effects: bool,
}

impl ResolvedId {
  /// Finishes a resolution: the attributes the import resolved with become part of the module
  /// id when `experimental.importAttributes` is enabled, and are dropped otherwise. An external
  /// keeps them either way, since the output renders them.
  #[must_use]
  pub fn finalize_attributes(mut self, import_attributes_enabled: bool) -> Self {
    let attributes = std::mem::take(&mut self.attributes);
    if (import_attributes_enabled || self.external.is_external()) && !attributes.is_empty() {
      self.id = ModuleId::with_attributes(self.id.raw_id(), &attributes);
      self.attributes = attributes;
    }
    self
  }

  /// Create a dummy ResolvedId, which is not exists in the file system
  /// note: A dummy `ResolvedId` usually used with `DUMMY_MODULE_IDX`
  pub fn make_dummy() -> Self {
    Self {
      id: ModuleId::default(),
      attributes: ImportAttributes::default(),
      module_def_format: ModuleDefFormat::Unknown,
      external: false.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: false,
    }
  }

  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  /// 3. has its line terminators escaped, so it can go in a `//` comment
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    let raw_id = self.id.raw_id();
    let debug_id = if is_data_url(raw_id) {
      concat_string!("<", raw_id, ">")
    } else if let Some(original) = self.id.strip_empty_prefix() {
      concat_string!("(ignored) ", stabilize_id(original, cwd.as_ref()))
    } else {
      stabilize_id(raw_id, cwd.as_ref())
    };
    let debug_id = match self.attributes.to_with_clause(false) {
      Some(with_clause) => concat_string!(debug_id, " ", with_clause),
      None => debug_id,
    };
    escape_line_terminators(debug_id)
  }

  pub fn new_external_without_side_effects(id: ArcStr) -> Self {
    Self {
      id: ModuleId::new(id),
      attributes: ImportAttributes::default(),
      module_def_format: ModuleDefFormat::Unknown,
      external: true.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: true,
    }
  }
}

#[test]
fn debug_id_escapes_comments_without_changing_module_identity() {
  use rolldown_std_utils::PathExt as _;

  let cwd = std::env::current_dir().unwrap();
  for suffix in ["plain", "—…😀", "\n\r\u{2028}\u{2029}", r"\n\r\u2028\u2029"] {
    let escaped = suffix
      .replace('\n', "\\n")
      .replace('\r', "\\r")
      .replace('\u{2028}', "\\u2028")
      .replace('\u{2029}', "\\u2029");
    for (id, expected) in [
      (ModuleId::new(format!("bare:{suffix}")), format!("bare:{escaped}")),
      (ModuleId::new(format!("\0virtual:{suffix}")), format!("\\0virtual:{escaped}")),
      (ModuleId::new_empty(&format!("bare:{suffix}")), format!("(ignored) bare:{escaped}")),
      (
        ModuleId::new(format!("data:text/javascript,{suffix}")),
        format!("<data:text/javascript,{escaped}>"),
      ),
    ] {
      let resolved = ResolvedId { id: id.clone(), ..Default::default() };
      let stable_id = id.stabilize(&cwd);
      assert_eq!(resolved.debug_id(&cwd), expected);
      assert_eq!(resolved.id, id);
      assert_eq!(resolved.id.stabilize(&cwd), stable_id);
    }
  }
  let path = cwd.join("module\n\r\u{2028}\u{2029}.js");
  let resolved = ResolvedId { id: ModuleId::new(path.expect_to_str()), ..Default::default() };
  assert_eq!(resolved.debug_id(&cwd), r"module\n\r\u2028\u2029.js");
  assert_eq!(resolved.id.as_str(), path.expect_to_str());
}
