use crate::types::generator::GenerateContext;
use arcstr::ArcStr;
use oxc::span::CompactStr;
use rolldown_common::{NormalModule, OutputExports};
use rolldown_error::{BuildDiagnostic, BuildResult};

// Port from https://github.com/rollup/rollup/blob/master/src/utils/getExportMode.ts
pub fn determine_export_mode(
  warnings: &mut Vec<BuildDiagnostic>,
  ctx: &GenerateContext<'_>,
  module: &NormalModule,
  export_names: &[CompactStr],
) -> BuildResult<OutputExports> {
  let export_mode = &ctx.options.exports;
  match export_mode {
    OutputExports::Named => Ok(OutputExports::Named),
    OutputExports::Default => {
      if export_names.len() != 1 || export_names[0].as_str() != "default" {
        return Err(
          vec![BuildDiagnostic::invalid_export_option(
            "default".into(),
            module.stable_id.as_str().into(),
            export_names.iter().map(|name| name.as_str().into()).collect(),
          )]
          .into(),
        );
      }
      Ok(OutputExports::Default)
    }
    OutputExports::None => {
      if !export_names.is_empty() {
        return Err(
          vec![BuildDiagnostic::invalid_export_option(
            "none".into(),
            module.stable_id.as_str().into(),
            export_names.iter().map(|name| name.as_str().into()).collect(),
          )]
          .into(),
        );
      }
      Ok(OutputExports::None)
    }
    OutputExports::Auto => {
      if export_names.is_empty() {
        Ok(OutputExports::None)
      } else if export_names.len() == 1 && export_names[0].as_str() == "default" {
        Ok(OutputExports::Default)
      } else {
        let has_default_export = export_names.iter().any(|name| name.as_str() == "default");
        if has_default_export {
          let name = ctx.chunk.name.as_ref().map(arcstr::ArcStr::to_string);
          warnings.push(
            BuildDiagnostic::mixed_export(
              module.id.to_string(),
              ArcStr::from(module.stable_id.as_str()),
              name.unwrap_or_else(|| String::from("chunk")),
              export_names.iter().map(|name| name.as_str().into()).collect(),
            )
            .with_severity_warning(),
          );
        }
        Ok(OutputExports::Named)
      }
    }
  }
}
