pub mod css_generator;

use arcstr::ArcStr;

use oxc::{
  index::{Idx, IndexVec},
  semantic::SymbolId,
};
use rolldown_common::{
  CssRenderer, CssView, ImportKind, ImportRecordIdx, ModuleIdx, RawImportRecord, SymbolRef,
};

pub fn create_css_view(
  _id: &str,
  source: &ArcStr,
) -> (CssView, IndexVec<ImportRecordIdx, RawImportRecord>) {
  let (lexed_deps, _warnings) =
    css_module_lexer::collect_dependencies(source, css_module_lexer::Mode::Css);

  let mut dependencies: IndexVec<ImportRecordIdx, RawImportRecord> = IndexVec::default();

  let mut css_renderer = CssRenderer::default();

  for lexed_dep in lexed_deps {
    match lexed_dep {
      css_module_lexer::Dependency::Import { request, range, .. } => {
        dependencies.push(RawImportRecord::new(
          request.into(),
          ImportKind::AtImport,
          SymbolRef::from((ModuleIdx::from_raw(0), SymbolId::from_usize(0))),
          range.start,
        ));
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
      _ => {}
    }
  }

  (
    CssView {
      source: source.clone(),
      import_records: IndexVec::default(),
      mutations: vec![Box::new(css_renderer)],
    },
    dependencies,
  )
}
