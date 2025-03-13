use arcstr::ArcStr;
use ariadne::{Label, Span};
use rustc_hash::FxHashMap;

use crate::{
  BuildDiagnostic, EventKindSwitcher,
  diagnostic::{DiagnosticFileId, RolldownLabelSpan},
};

pub fn is_context_too_long(
  label: &Label<RolldownLabelSpan>,
  files: &FxHashMap<DiagnosticFileId, ArcStr>,
) -> bool {
  let span = label.span();
  let source_id = span.source();
  let source = files.get(source_id).expect("should have file");
  if source.len() < 600 {
    return false;
  }
  let rope = ropey::Rope::from_str(source);
  let has_line_feed_after = rope
    .try_byte_to_char(span.end())
    .ok()
    .and_then(|end| {
      let postfix = rope.slice(end..);
      postfix.chars().position(|char| char == '\n').map(|offset| offset < 300)
    })
    // We treat EOF as a line feed
    .unwrap_or(true);

  let has_line_feed_before = rope
    .try_byte_to_char(span.start())
    .ok()
    .and_then(|start| {
      // We treat Start of the file as a line feed
      if start < 300 {
        return Some(true);
      }
      let postfix = rope.slice(..start);
      postfix.chars().position(|char| char == '\n').map(|offset| offset < 300)
    })
    .unwrap_or(false);
  !has_line_feed_after || !has_line_feed_before
}

pub fn filter_out_disabled_diagnostics(
  diagnostics: Vec<BuildDiagnostic>,
  switcher: &EventKindSwitcher,
) -> Vec<BuildDiagnostic> {
  diagnostics
    .into_iter()
    .filter(|d| switcher.contains(EventKindSwitcher::from_bits_truncate(1 << d.kind() as u32)))
    .collect()
}
