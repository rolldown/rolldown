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

  if span.start() > rope.len_bytes() || span.end() > rope.len_bytes() {
    unreachable!(
      "Internal error: Diagnostic span is out of range. This should never happen! \
       Span: {}..{}, Rope length: {} bytes, Source ID: {:?}, Label message: {:?}. \
       Please report this bug with the source file that triggered this error.",
      span.start(),
      span.end(),
      rope.len_bytes(),
      source_id,
      label.display_info().msg()
    );
  }

  // 1. If start to beginning of the file is less than 300 characters, treated as it has line feed before.
  // 2. If end to end of the file is less than 300 characters, treated as it has line feed after.
  let postfix = rope.byte_slice(span.end()..);
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

  let prefix = rope.byte_slice(..span.start());

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
