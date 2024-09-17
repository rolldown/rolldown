use arcstr::ArcStr;
use oxc::index::IndexVec;

use crate::{ImportRecord, ImportRecordIdx};

#[derive(Debug)]
pub struct CssView {
  pub source: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ImportRecord>,
}
