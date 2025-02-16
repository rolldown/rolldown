use crate::ImportRecordIdx;

#[derive(Debug, Default)]
pub struct HmrInfo {
  pub deps: Vec<ImportRecordIdx>,
}
