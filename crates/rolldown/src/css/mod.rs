pub mod css_generator;

use arcstr::ArcStr;
use oxc::index::IndexVec;
use rolldown_common::CssView;

pub fn create_css_view(source: &ArcStr) -> CssView {
  CssView { source: source.clone(), import_records: IndexVec::default() }
}
