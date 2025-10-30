use super::BuildEvent;
use crate::DiagnosticOptions;
use crate::build_diagnostic::diagnostic::Diagnostic;
use arcstr::ArcStr;
use oxc::span::Span;

#[derive(Debug)]
pub struct EmptyImportMeta {
  pub filename: String,
  pub source: ArcStr,
  pub span: Span,
  pub format: ArcStr,
  pub is_import_meta_url: bool,
}

impl BuildEvent for EmptyImportMeta {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::EmptyImportMeta
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("`import.meta` may not be a valid syntax with the `{}` output format.", self.format)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);
    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.title =
      format!("`import.meta` may not be a valid syntax with the `{}` output format.", self.format);

    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      String::from(
        "This `import.meta` will be replaced with an empty object (`{}`) automatically. If this is desired, you can suppress this warning by adding `transform.define: { 'import.meta': {} }`. If `import.meta` needs to be kept as-is, you need to set the output format to `esm`.",
      ),
    );

    if self.is_import_meta_url {
      diagnostic.add_help(String::from("If you want to polyfill `import.meta.url` like Rollup does, check out the Document: https://rolldown.rs/in-depth/non-esm-output-formats#well-known-import-meta-properties"));
    }
  }
}
