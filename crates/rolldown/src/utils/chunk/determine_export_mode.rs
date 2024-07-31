use rolldown_common::{EcmaModule, OutputExports, SymbolRef};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_rstr::Rstr;

// Port from https://github.com/rollup/rollup/blob/master/src/utils/getExportMode.ts
pub fn determine_export_mode(
  export_mode: &OutputExports,
  module: &EcmaModule,
  exports: &[(Rstr, SymbolRef)],
) -> DiagnosableResult<OutputExports> {
  match export_mode {
    OutputExports::Named => Ok(OutputExports::Named),
    OutputExports::Default => {
      if exports.len() != 1 || exports[0].0.as_str() != "default" {
        return Err(vec![BuildDiagnostic::invalid_export_option(
          "default".into(),
          module.stable_id.as_str().into(),
          exports.iter().map(|(name, _)| name.as_str().into()).collect(),
        )]);
      }
      Ok(OutputExports::Default)
    }
    OutputExports::None => {
      if !exports.is_empty() {
        return Err(vec![BuildDiagnostic::invalid_export_option(
          "none".into(),
          module.stable_id.as_str().into(),
          exports.iter().map(|(name, _)| name.as_str().into()).collect(),
        )]);
      }
      Ok(OutputExports::None)
    }
    OutputExports::Auto => {
      if exports.is_empty() {
        Ok(OutputExports::None)
      } else if exports.len() == 1 && exports[0].0.as_str() == "default" {
        Ok(OutputExports::Default)
      } else {
        // TODO add warnings
        Ok(OutputExports::Named)
      }
    }
  }
}
