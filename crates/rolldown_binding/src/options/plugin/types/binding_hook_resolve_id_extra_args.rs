use super::binding_import_attributes::BindingImportAttributes;

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: Created in js_plugin.rs and passed to JS plugin resolve_id hooks
// - Receive from JS: Received in binding_callable_builtin_plugin.rs as parameter (unused currently)
#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookResolveIdExtraArgs {
  pub custom: Option<u32>,
  pub is_entry: bool,
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
  pub kind: String,
  /// The import attributes of the import being resolved. Omitted when empty.
  #[napi(ts_type = "Record<string, string>")]
  pub attributes: Option<BindingImportAttributes>,
  /// The importer without its import attributes. Omitted when equal to the importer.
  pub importer_raw_id: Option<String>,
  /// The import attributes of the importer itself. Omitted when empty.
  #[napi(ts_type = "Record<string, string>")]
  pub importer_attributes: Option<BindingImportAttributes>,
}
