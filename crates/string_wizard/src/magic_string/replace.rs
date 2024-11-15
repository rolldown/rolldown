use crate::{CowStr, UpdateOptions};

use super::MagicString;

#[derive(Debug)]
pub struct ReplaceOptions {
  /// The maximum number of times to replace the pattern. Default is `1`.
  pub count: usize,
  /// This will store the original content in the `name` field of the generated sourcemap.
  ///
  /// Default is `false`.
  pub store_original_in_sourcemap: bool,
}

impl Default for ReplaceOptions {
  fn default() -> Self {
    Self { count: 1, store_original_in_sourcemap: false }
  }
}

impl<'text> MagicString<'text> {
  pub fn replace(&mut self, from: &str, to: impl Into<CowStr<'text>>) -> &mut Self {
    self.replace_with(from, to, Default::default())
  }

  pub fn replace_all(&mut self, from: &str, to: impl Into<CowStr<'text>>) -> &mut Self {
    self.replace_with(from, to, ReplaceOptions { count: usize::MAX, ..Default::default() })
  }

  pub fn replace_with(
    &mut self,
    from: &str,
    to: impl Into<CowStr<'text>>,
    options: ReplaceOptions,
  ) -> &mut Self {
    let to: CowStr<'text> = to.into();
    // PERF(hyf0): Unnecessary `collect` due to borrow checker limitation.
    let matches = self
      .source
      .match_indices(from)
      .take(options.count)
      .map(|(match_start, part)| (match_start, match_start + part.len()))
      .collect::<Box<[_]>>();

    matches.iter().for_each(|(start, end)| {
      self.update_with(
        *start,
        *end,
        to.clone(),
        UpdateOptions { overwrite: true, keep_original: options.store_original_in_sourcemap },
      );
    });

    self
  }
}
