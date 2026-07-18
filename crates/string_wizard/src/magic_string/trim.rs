use std::borrow::Cow;
use std::collections::VecDeque;

use crate::MagicString;

impl<'text> MagicString<'text> {
  /// Trims whitespace from the start and end of the string.
  pub fn trim(&mut self, char_type: Option<&str>) -> &mut Self {
    self.trim_start(char_type);
    self.trim_end(char_type);
    self
  }

  /// Trims whitespace from the start of the string.
  pub fn trim_start(&mut self, char_type: Option<&str>) -> &mut Self {
    self.trim_start_aborted(char_type);
    self
  }

  /// Trims whitespace from the end of the string.
  pub fn trim_end(&mut self, char_type: Option<&str>) -> &mut Self {
    self.trim_end_aborted(char_type);
    self
  }

  /// Trims newlines from the start and end of the string.
  pub fn trim_lines(&mut self) -> &mut Self {
    self.trim(Some("[\r\n]"))
  }

  /// Internal method that trims from the start and returns true if aborted early.
  fn trim_start_aborted(&mut self, char_type: Option<&str>) -> bool {
    let pattern = char_type.unwrap_or("\\s");

    // Trim intro
    if trim_deque_start(&mut self.intro, pattern) {
      return true;
    }

    // Trim chunks from start
    let mut chunk_idx = Some(self.first_chunk_idx);
    while let Some(idx) = chunk_idx {
      let chunk = &self.chunks[idx];
      let next_idx = chunk.next;
      let is_edited = chunk.edited_content.is_some();
      let chunk_end = chunk.end();

      // Get the chunk content
      let content = if let Some(ref edited) = chunk.edited_content {
        edited.as_ref().to_string()
      } else {
        chunk.span.text(&self.source).to_string()
      };

      // Trim intro of chunk
      if trim_deque_start(&mut self.chunks[idx].intro, pattern) {
        return true;
      }

      // Trim the content
      let trimmed_content = trim_start_pattern(&content, pattern);

      if content.is_empty() {
        // Content is empty (e.g. from a remove); the outro is next in output order, so trim
        // it and abort if non-whitespace remains.
        if trim_deque_start(&mut self.chunks[idx].outro, pattern) {
          return true;
        }
        chunk_idx = next_idx;
        continue;
      }

      if trimmed_content.len() == content.len() {
        // No trimming happened and content is not empty, we're done
        return true;
      }

      if !trimmed_content.is_empty() {
        // Partial trim. For an unedited chunk, split at the trim boundary and blank only
        // the whitespace half — the kept half stays *unedited*, so hires sourcemaps keep
        // their per-character fidelity (magic-string's `Chunk.trimStart` splits the same
        // way). An edited chunk has no per-character mapping to preserve; storing the
        // trimmed content is what upstream's re-edit amounts to.
        if is_edited {
          self.chunks[idx].edited_content = Some(trimmed_content.to_string().into());
        } else {
          let boundary = chunk_end - trimmed_content.len() as u32;
          if self.split_at(boundary).is_ok() {
            // `split_at` keeps the first half at `idx`; blank just the leading whitespace.
            self.chunks[idx].edited_content = Some("".into());
          } else {
            // Unreachable: only edited chunks refuse to split. Degrade to the whole-chunk
            // edit rather than panicking across the FFI boundary.
            self.chunks[idx].edited_content = Some(trimmed_content.to_string().into());
          }
        }
        return true;
      }

      // Entire content was trimmed - mark as empty
      self.chunks[idx].edited_content = Some("".into());

      // Trim outro of this chunk - if non-whitespace remains, we're done
      if trim_deque_start(&mut self.chunks[idx].outro, pattern) {
        return true;
      }

      chunk_idx = next_idx;
    }

    false
  }

  /// Internal method that trims from the end and returns true if aborted early.
  fn trim_end_aborted(&mut self, char_type: Option<&str>) -> bool {
    let pattern = char_type.unwrap_or("\\s");

    // Trim outro
    if trim_deque_end(&mut self.outro, pattern) {
      return true;
    }

    // Trim chunks from end
    let mut chunk_idx = Some(self.last_chunk_idx);
    while let Some(idx) = chunk_idx {
      let chunk = &self.chunks[idx];
      let prev_idx = chunk.prev;
      let is_edited = chunk.edited_content.is_some();
      let chunk_start = chunk.start();

      // Get the chunk content
      let content = if let Some(ref edited) = chunk.edited_content {
        edited.as_ref().to_string()
      } else {
        chunk.span.text(&self.source).to_string()
      };

      // Trim outro of chunk
      if trim_deque_end(&mut self.chunks[idx].outro, pattern) {
        return true;
      }

      // Trim the content
      let trimmed_content = trim_end_pattern(&content, pattern);

      if content.is_empty() {
        // Content is empty (e.g. from a remove); the intro is next in reverse output order,
        // so trim it and abort if non-whitespace remains.
        if trim_deque_end(&mut self.chunks[idx].intro, pattern) {
          return true;
        }
        chunk_idx = prev_idx;
        continue;
      }

      if trimmed_content.len() == content.len() {
        // No trimming happened and content is not empty, we're done
        return true;
      }

      if !trimmed_content.is_empty() {
        // Partial trim. For an unedited chunk, split at the trim boundary and blank only
        // the whitespace half — the kept half stays *unedited*, so hires sourcemaps keep
        // their per-character fidelity (magic-string's `Chunk.trimEnd` splits the same
        // way). An edited chunk has no per-character mapping to preserve; storing the
        // trimmed content is what upstream's re-edit amounts to.
        if is_edited {
          self.chunks[idx].edited_content = Some(trimmed_content.to_string().into());
        } else {
          let boundary = chunk_start + trimmed_content.len() as u32;
          if self.split_at(boundary).is_ok() {
            // The trailing whitespace is the second half, starting at the boundary.
            let whitespace_idx = self.chunk_by_start[&boundary];
            self.chunks[whitespace_idx].edited_content = Some("".into());
          } else {
            // Unreachable: only edited chunks refuse to split. Degrade to the whole-chunk
            // edit rather than panicking across the FFI boundary.
            self.chunks[idx].edited_content = Some(trimmed_content.to_string().into());
          }
        }
        return true;
      }

      // Entire content was trimmed - mark as empty
      self.chunks[idx].edited_content = Some("".into());

      // Trim intro of this chunk - if non-whitespace remains, we're done
      if trim_deque_end(&mut self.chunks[idx].intro, pattern) {
        return true;
      }

      chunk_idx = prev_idx;
    }

    false
  }
}

/// Trims a deque from the start using the given pattern.
/// Returns true if any non-empty content remains after trimming.
fn trim_deque_start<'a>(deque: &mut VecDeque<Cow<'a, str>>, pattern: &str) -> bool {
  let old_deque = std::mem::take(deque);
  let mut found_non_match = false;

  for s in old_deque {
    if found_non_match {
      deque.push_back(s);
    } else {
      let trimmed = trim_start_pattern(s.as_ref(), pattern);
      if !trimmed.is_empty() {
        deque.push_back(Cow::Owned(trimmed.to_string()));
        found_non_match = true;
      }
    }
  }

  !deque.is_empty()
}

/// Trims a deque from the end using the given pattern.
/// Returns true if any non-empty content remains after trimming.
fn trim_deque_end<'a>(deque: &mut VecDeque<Cow<'a, str>>, pattern: &str) -> bool {
  let old_deque = std::mem::take(deque);
  let mut found_non_match = false;

  for s in old_deque.into_iter().rev() {
    if found_non_match {
      deque.push_front(s);
    } else {
      let trimmed = trim_end_pattern(s.as_ref(), pattern);
      if !trimmed.is_empty() {
        deque.push_front(Cow::Owned(trimmed.to_string()));
        found_non_match = true;
      }
    }
  }

  !deque.is_empty()
}

/// Trims characters matching the pattern from the start of the string.
/// Supports common patterns and arbitrary regex patterns:
/// - "\\s" -> whitespace
/// - "[\\r\\n]" -> newlines only
/// - Any other valid regex pattern
fn trim_start_pattern<'a>(s: &'a str, pattern: &str) -> &'a str {
  // Fast path for common patterns
  match pattern {
    "\\s" => return s.trim_start(),
    "[\\r\\n]" | "[\r\n]" => return s.trim_start_matches(['\r', '\n']),
    _ => {}
  }

  // Use regex for custom patterns
  let regex_pattern = format!("^({pattern})+");
  match regex::Regex::new(&regex_pattern) {
    Ok(re) => {
      if let Some(m) = re.find(s) {
        &s[m.end()..]
      } else {
        s
      }
    }
    Err(_) => s.trim_start(), // Fallback to whitespace on invalid regex
  }
}

/// Trims characters matching the pattern from the end of the string.
/// Supports common patterns and arbitrary regex patterns:
/// - "\\s" -> whitespace
/// - "[\\r\\n]" -> newlines only
/// - Any other valid regex pattern
fn trim_end_pattern<'a>(s: &'a str, pattern: &str) -> &'a str {
  // Fast path for common patterns
  match pattern {
    "\\s" => return s.trim_end(),
    "[\\r\\n]" | "[\r\n]" => return s.trim_end_matches(['\r', '\n']),
    _ => {}
  }

  // Use regex for custom patterns
  let regex_pattern = format!("({pattern})+$");
  match regex::Regex::new(&regex_pattern) {
    Ok(re) => {
      if let Some(m) = re.find(s) {
        &s[..m.start()]
      } else {
        s
      }
    }
    Err(_) => s.trim_end(), // Fallback to whitespace on invalid regex
  }
}
