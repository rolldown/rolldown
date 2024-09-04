use rolldown_error::BuildDiagnostic;

#[derive(Default)]
pub struct HmrOutput {
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
}
