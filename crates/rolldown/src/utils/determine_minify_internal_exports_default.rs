use rolldown_common::{OutputFormat, RawMinifyOptions};

/// Determines the default value for `minify_internal_exports` based on format and minify settings.
///
/// Returns `true` if:
/// - format is `Esm`, OR
/// - minify is `Bool(true)` or `Object(_)`
///
/// Otherwise returns `false`.
pub fn determine_minify_internal_exports_default(
  format: Option<OutputFormat>,
  minify: &RawMinifyOptions,
) -> bool {
  matches!(format, Some(OutputFormat::Esm))
    || matches!(minify, RawMinifyOptions::Bool(true) | RawMinifyOptions::Object(_))
}
