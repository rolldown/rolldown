use crate::ImportRecordIdx;

#[derive(Debug, Default, Clone)]
pub struct HmrInfo {
  pub deps: Vec<ImportRecordIdx>,
}
