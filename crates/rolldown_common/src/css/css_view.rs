use arcstr::ArcStr;
use oxc::{
  index::{Idx, IndexVec},
  semantic::SymbolId,
  span::Span,
};

use crate::{
  types::source_mutation::BoxedSourceMutation, ImportKind, ImportRecordIdx, ModuleIdx,
  RawImportRecord, ResolvedImportRecord, SourceMutation, SymbolRef,
};

#[derive(Debug)]
pub struct CssView {
  pub source: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub record_idx_to_span: IndexVec<ImportRecordIdx, Span>,
  pub mutations: Vec<BoxedSourceMutation>,
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

pub fn create_css_view(
  _id: &str,
  source: &ArcStr,
) -> (CssView, IndexVec<ImportRecordIdx, RawImportRecord>) {
  let (lexed_deps, _warnings) =
    css_module_lexer::collect_dependencies(source, css_module_lexer::Mode::Css);

  let mut dependencies: IndexVec<ImportRecordIdx, RawImportRecord> = IndexVec::default();
  let mut record_idx_to_span: IndexVec<ImportRecordIdx, Span> = IndexVec::default();

  let mut css_renderer = CssRenderer::default();

  for lexed_dep in lexed_deps {
    match lexed_dep {
      css_module_lexer::Dependency::Import { request, range, .. } => {
        dependencies.push(RawImportRecord::new(
          request.into(),
          ImportKind::AtImport,
          SymbolRef::from((ModuleIdx::from_raw(0), SymbolId::from_usize(0))),
          Span::new(range.start, range.end),
        ));
        record_idx_to_span.push(Span::new(range.start, range.end));
        let mut range_end = range.end as usize;
        if source.is_char_boundary(range_end) {
          if source[range_end..].starts_with("\r\n") {
            range_end += 2;
          }
          if source[range_end..].starts_with('\n') {
            range_end += 1;
          }
        }
        css_renderer.at_import_ranges.push((range.start as usize, range_end));
      }
      css_module_lexer::Dependency::Url { request, range, kind } => {
        // css_module_lexer return span of `request` if kind is `string`, return whole span of `url(dep)`, if the kind is function
        // so we need to tweak a little to get the correct span we want that used to replace
        // asset filename
        let span = if matches!(kind, css_module_lexer::UrlRangeKind::String) {
          Span::new(range.start + 1, range.end - 1)
        } else {
          Span::new(range.start + 4 /*length of `url(`*/, range.end - 1)
        };
        dependencies.push(RawImportRecord::new(
          request.into(),
          ImportKind::UrlImport,
          SymbolRef::from((ModuleIdx::from_raw(0), SymbolId::from_usize(0))),
          span,
        ));
        record_idx_to_span.push(span);
      }
      _ => {}
    }
  }

  (
    CssView {
      source: source.clone(),
      import_records: IndexVec::default(),
      mutations: vec![Box::new(css_renderer)],
      record_idx_to_span,
    },
    dependencies,
  )
}
