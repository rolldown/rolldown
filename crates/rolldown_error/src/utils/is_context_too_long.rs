#![expect(clippy::print_stderr)]

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
  let Some(source) = files.get(source_id) else {
    return true;
  };
  if source.len() < 600 {
    return false;
  }

  // Only the ~300 chars on either side of the span matter, so slice the source
  // string directly instead of building a whole-file rope. Building a rope here
  // is O(file length) per call, which turns rendering N diagnostics in one large
  // file into O(N^2) (see #9748).
  let (Some(prefix), Some(postfix)) = (source.get(..span.start()), source.get(span.end()..)) else {
    eprintln!(
      "Internal error: Diagnostic span is out of range. \
       Span: {}..{}, source length: {} bytes, Source ID: {:?}, Label message: {:?}. \
       Please report this bug with the source file that triggered this error.",
      span.start(),
      span.end(),
      source.len(),
      source_id,
      label.display_info().msg()
    );
    return true;
  };

  // 1. If start to beginning of the file is less than 300 characters, treated as it has line feed before.
  // 2. If end to end of the file is less than 300 characters, treated as it has line feed after.
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

  let mut has_line_feed_before = false;
  let mut cnt = 0;
  for ch in prefix.chars().rev() {
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
