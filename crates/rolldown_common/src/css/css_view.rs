use arcstr::ArcStr;
use oxc::index::IndexVec;

use crate::{
  types::source_mutation::BoxedSourceMutation, ImportRecordIdx, ResolvedImportRecord,
  SourceMutation,
};

#[derive(Debug)]
pub struct CssView {
  pub source: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub mutations: Vec<BoxedSourceMutation>,
}

#[derive(Debug, Default)]
pub struct CssRenderer {
  pub at_import_ranges: Vec<(usize, usize)>,
}

impl SourceMutation for CssRenderer {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    for range in &self.at_import_ranges {
      magic_string.remove(range.0, range.1);
    }
  }
}
