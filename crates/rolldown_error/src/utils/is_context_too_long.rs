use arcstr::ArcStr;
use ariadne::{Label, Span};
use rustc_hash::FxHashMap;

use crate::build_diagnostic::diagnostic::{DiagnosticFileId, RolldownLabelSpan};

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
  // 1. If start to beginning of the file is less than 300 characters, treated as it has line feed before.
  // 2. If end to end of the file is less than 300 characters, treated as it has line feed after.
  let end = span.end();
  let start = span.start();
  let postfix = rope.slice(end..);
  let mut has_line_feed_after = false;
  let mut cnt = 0;
  for ch in postfix.chars() {
    if ch == '\n' {
      has_line_feed_after = true;
      break;
    }
    cnt += 1;
    if cnt > 300 {
      break;
    }
  }

  if cnt < 300 {
    has_line_feed_after = true;
  }

  let prefix = rope.slice(..start);

  let mut has_line_feed_before = false;
  let mut cnt = 0;
  for ch in prefix.chars().reversed() {
    if ch == '\n' {
      has_line_feed_before = true;
      break;
    }
    cnt += 1;
    if cnt > 300 {
      break;
    }
  }
  if cnt < 300 {
    has_line_feed_before = true;
  }

  !has_line_feed_after || !has_line_feed_before
}
