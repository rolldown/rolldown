use crate::types::generator::GenerateContext;
use arcstr::ArcStr;
use rolldown_common::{NormalModule, OutputExports, SymbolRef};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_rstr::Rstr;

// Port from https://github.com/rollup/rollup/blob/master/src/utils/getExportMode.ts
pub fn determine_export_mode(
  ctx: &mut GenerateContext<'_>,
  module: &NormalModule,
  exports: &[(Rstr, SymbolRef)],
) -> DiagnosableResult<OutputExports> {
  let export_mode = &ctx.options.exports;
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
        let has_default_export = exports.iter().any(|(name, _)| name.as_str() == "default");
        if has_default_export {
          let name = &ctx.chunk.name;
          let chunk = ArcStr::from("chunk");
          let name = name.as_ref().unwrap_or(&chunk);
          ctx.warnings.push(
            BuildDiagnostic::mixed_export(
              ArcStr::from(module.stable_id.as_str()),
              ArcStr::from(name),
              exports.iter().map(|(name, _)| name.as_str().into()).collect(),
            )
            .with_severity_warning(),
          );
        }
        Ok(OutputExports::Named)
      }
    }
  }
}
