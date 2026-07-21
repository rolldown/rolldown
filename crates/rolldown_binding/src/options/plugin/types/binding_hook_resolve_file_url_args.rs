// Passed to JS plugin `resolveFileUrl` hooks.
#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingHookResolveFileUrlArgs {
  /// Preliminary filename of the chunk containing the reference.
  pub chunk_id: String,
  /// Filename of the emitted file, relative to the output directory.
  pub file_name: String,
  #[napi(ts_type = "'es' | 'cjs' | 'iife' | 'umd'")]
  pub format: String,
  /// Id of the module containing the `import.meta.ROLLDOWN_FILE_URL_*` reference.
  pub module_id: String,
  pub reference_id: String,
  /// Path from the chunk to the emitted file.
  pub relative_path: String,
}
