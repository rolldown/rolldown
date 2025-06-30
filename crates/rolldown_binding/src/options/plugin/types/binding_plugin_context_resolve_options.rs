use std::sync::Arc;

use rolldown_plugin::{CustomField, PluginContextResolveOptions};
use rolldown_plugin_utils::constants::{ViteImportGlob, ViteImportGlobValue};

use crate::options::plugin::{
  JsPluginContextResolveCustomArgId, types::binding_vite_plugin_custom::BindingVitePluginCustom,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingPluginContextResolveOptions {
  // Refer to crates/rolldown_common/src/types/import_kind.rs
  /// - `import-statement`: `import { foo } from './lib.js';`
  /// - `dynamic-import`: `import('./lib.js')`
  /// - `require-call`: `require('./lib.js')`
  /// - `import-rule`: `@import 'bg-color.css'`
  /// - `url-token`: `url('./icon.png')`
  /// - `new-url`: `new URL('./worker.js', import.meta.url)`
  /// - `hot-accept`: `import.meta.hot.accept('./lib.js', () => {})`
  #[napi(
    ts_type = "'import-statement' | 'dynamic-import' | 'require-call' | 'import-rule' | 'url-token' | 'new-url' | 'hot-accept'"
  )]
  pub import_kind: Option<String>,
  pub skip_self: Option<bool>,
  pub custom: Option<u32>,
  pub vite_plugin_custom: Option<BindingVitePluginCustom>,
}

impl TryFrom<BindingPluginContextResolveOptions> for PluginContextResolveOptions {
  type Error = String;

  fn try_from(value: BindingPluginContextResolveOptions) -> Result<Self, Self::Error> {
    let custom = CustomField::new();
    if let Some(js_custom_id) = value.custom {
      custom.insert(JsPluginContextResolveCustomArgId, js_custom_id);
    }
    if let Some(is_sub_imports_pattern) = value
      .vite_plugin_custom
      .and_then(|v| v.vite_import_glob.and_then(|v| v.is_sub_imports_pattern))
    {
      custom.insert(ViteImportGlob, ViteImportGlobValue(is_sub_imports_pattern));
    }
    Ok(Self {
      import_kind: value.import_kind.as_deref().unwrap_or("import-statement").try_into()?,
      skip_self: value.skip_self.unwrap_or(true),
      custom: Arc::new(custom),
    })
  }
}
