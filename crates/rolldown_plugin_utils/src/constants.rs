use rolldown_plugin::typedmap::TypedMapKey;
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use rustc_hash::FxHashMap;

#[derive(Hash, PartialEq, Eq)]
pub struct ViteImportGlob;
pub struct ViteImportGlobValue(pub bool);

impl ViteImportGlobValue {
  pub fn is_sub_imports_pattern(&self) -> bool {
    self.0
  }
}

impl TypedMapKey for ViteImportGlob {
  type Value = ViteImportGlobValue;
}

#[derive(Debug, Default)]
pub struct ViteMetadata {
  pub imported_assets: FxDashSet<String>,
}

#[derive(Debug, Default)]
pub struct CSSModuleCache {
  pub inner: FxDashMap<String, FxHashMap<String, String>>,
}

#[derive(Debug, Default)]
pub struct HTMLProxyResult {
  pub inner: FxDashMap<String, String>,
}
