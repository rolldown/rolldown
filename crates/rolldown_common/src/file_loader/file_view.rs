use arcstr::ArcStr;

use crate::SourceMutation;

#[derive(Debug)]
pub struct FileView {
  pub source: ArcStr,
}

#[derive(Debug)]
pub struct FileViewRender {
  pub at_replaced_ranges: Vec<(usize, usize)>,
}

impl SourceMutation for FileViewRender {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    for range in &self.at_replaced_ranges {}
  }
}
