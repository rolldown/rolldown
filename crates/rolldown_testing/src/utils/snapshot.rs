use std::ffi::OsStr;
use std::path::Path;

use rolldown_error::{BuildDiagnostic, DiagnosticOptions};

use crate::types::SnapshotSection;

/// Normalize file extension for syntax highlighting (converts mjs/cjs to js)
pub fn normalize_file_extension(ext: &str) -> &str {
  match ext {
    "mjs" | "cjs" => "js",
    _ => ext,
  }
}

/// Get file extension from path as string, with normalization for js variants
pub fn get_normalized_extension(path: &Path) -> &str {
  path.extension().and_then(OsStr::to_str).map_or("unknown", normalize_file_extension)
}

/// Render diagnostics into snapshot sections with code blocks and sorting
/// It looks like:
///
/// # [code]
///
/// ```text
/// [diagnostic message]
/// ```
pub fn render_diagnostics(
  diagnostics: impl Iterator<Item = (impl ToString, impl ToString)>,
) -> Vec<SnapshotSection> {
  let mut rendered_diagnostics = diagnostics
    .map(|(code, diagnostic)| {
      let mut child = SnapshotSection::with_title(code.to_string());
      child.add_content("```text\n");
      child.add_content(&diagnostic.to_string());
      child.add_content("\n```");
      child
    })
    .collect::<Vec<_>>();

  // FIXME: For compatibility with previous snapshots, we still sort by title first. Will use a performant way later.
  rendered_diagnostics.sort_by_cached_key(SnapshotSection::render);

  rendered_diagnostics
}

/// Create an error section with sorted diagnostics
/// It looks like:
///
/// # Errors
///
/// ## [code]
///
/// ```text
/// [diagnostic message]
/// ```
pub fn create_error_section(errs: Vec<BuildDiagnostic>, cwd: &Path) -> SnapshotSection {
  let mut errors = errs;

  let mut errors_section = SnapshotSection::with_title("Errors");
  errors.sort_by_key(|e| e.kind().to_string());

  let diagnostics = errors
    .into_iter()
    .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

  let rendered_diagnostics = render_diagnostics(diagnostics);

  for diag in rendered_diagnostics {
    errors_section.add_child(diag);
  }
  errors_section
}
