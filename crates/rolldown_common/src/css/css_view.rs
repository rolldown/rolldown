use arcstr::ArcStr;
use oxc::index::IndexVec;

use crate::{ImportRecordIdx, ResolvedImportRecord};

#[derive(Debug)]
pub struct CssView {
  pub source: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
}
