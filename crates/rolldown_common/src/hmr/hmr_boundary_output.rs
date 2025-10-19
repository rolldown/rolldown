use arcstr::ArcStr;

#[derive(Debug)]
pub struct HmrBoundaryOutput {
  pub boundary: ArcStr,
  pub accepted_via: ArcStr,
}
