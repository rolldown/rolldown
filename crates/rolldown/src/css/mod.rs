pub mod css_generator;

use arcstr::ArcStr;

use oxc::{
  index::{Idx, IndexVec},
  semantic::SymbolId,
};
use rolldown_common::{
  CssView, ImportKind, ImportRecordIdx, ModuleIdx, RawImportRecord, SymbolRef,
};

pub fn create_css_view(
  _id: &str,
  source: &ArcStr,
) -> (CssView, IndexVec<ImportRecordIdx, RawImportRecord>) {
  let (lexed_deps, _warnings) =
    css_module_lexer::collect_dependencies(source, css_module_lexer::Mode::Css);

  let mut dependencies: IndexVec<ImportRecordIdx, RawImportRecord> = IndexVec::default();

  for lexed_dep in lexed_deps {
    match lexed_dep {
      css_module_lexer::Dependency::Import { request, range, .. } => {
        dependencies.push(RawImportRecord::new(
          request.into(),
          ImportKind::AtImport,
          SymbolRef::from((ModuleIdx::from_raw(0), SymbolId::from_usize(0))),
          range.start,
        ));
      }
      _ => {}
    }
  }

  (CssView { source: source.clone(), import_records: IndexVec::default() }, dependencies)
}
