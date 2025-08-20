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
    for (match_start, part) in self.source.match_indices(from).take(options.count) {
      let end = match_start + part.len();
      self.update_with(
        match_start,
        end,
        to.clone(),
        UpdateOptions { overwrite: true, keep_original: options.store_original_in_sourcemap },
      );
    }

    self
  }
}
