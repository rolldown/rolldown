use rolldown_common::OutputFormat;

#[derive(Debug)]
pub struct HookResolveFileUrlArgs<'a> {
  /// Preliminary filename of the chunk containing the reference.
  pub chunk_id: &'a str,
  /// Filename of the emitted file, relative to the output directory.
  pub file_name: &'a str,
  pub format: OutputFormat,
  /// Id of the module containing `import.meta.ROLLDOWN_FILE_URL_<reference_id>`.
  pub module_id: &'a str,
  pub reference_id: &'a str,
  /// Path from the chunk to the emitted file.
  pub relative_path: &'a str,
  /// The `<urlId>` of `import.meta.ROLLDOWN_FILE_URL_<referenceId>_<urlId>`, if present.
  /// Only the rolldown-specific form carries it; the `ROLLUP_FILE_URL_` alias never does.
  ///
  /// This API is experimental and may change in minor versions.
  pub url_id: Option<&'a str>,
}
