use std::sync::Arc;

use rolldown_plugin::{CustomField, PluginContextResolveOptions};

use crate::options::plugin::JsPluginContextResolveCustomArgId;

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
}

impl TryFrom<BindingPluginContextResolveOptions> for PluginContextResolveOptions {
  type Error = String;

  fn try_from(value: BindingPluginContextResolveOptions) -> Result<Self, Self::Error> {
    let custom = CustomField::new();
    if let Some(js_custom_id) = value.custom {
      custom.insert(JsPluginContextResolveCustomArgId, js_custom_id);
    }
    Ok(Self {
      import_kind: value.import_kind.as_deref().unwrap_or("import-statement").try_into()?,
      skip_self: value.skip_self.unwrap_or(true),
      custom: Arc::new(custom),
    })
  }
}
