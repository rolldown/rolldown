use crate::ModuleIdx;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct HmrBoundary {
  pub boundary: ModuleIdx,
  pub accepted_via: ModuleIdx,
}
