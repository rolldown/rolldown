use arcstr::ArcStr;
use oxc::span::Span;
use oxc_index::IndexVec;

use crate::{
  ImportRecordIdx, ResolvedImportRecord, SourceMutation, types::source_mutation::ArcSourceMutation,
};

#[derive(Debug, Clone)]
pub struct CssView {
  pub source: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub record_idx_to_span: IndexVec<ImportRecordIdx, Span>,
  pub mutations: Vec<ArcSourceMutation>,
}

#[derive(Debug, Default)]
pub struct CssRenderer {
  pub at_import_ranges: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub struct CssAssetNameReplacer {
  pub span: Span,
  pub asset_name: ArcStr,
}

impl SourceMutation for CssRenderer {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    for range in &self.at_import_ranges {
      magic_string.remove(range.0, range.1);
    }
  }
}

impl SourceMutation for CssAssetNameReplacer {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    magic_string.update_with(
      self.span.start as usize,
      self.span.end as usize,
      self.asset_name.clone(),
      string_wizard::UpdateOptions { keep_original: true, overwrite: true },
    );
  }
}
