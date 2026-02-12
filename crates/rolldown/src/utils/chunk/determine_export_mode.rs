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
        if !ctx.options.format.is_esm()
          && export_names.iter().any(|name| name.as_str() == "default")
        {
          warnings.push(
            BuildDiagnostic::mixed_export(
              module.id.to_string(),
              ctx
                .options
                .name
                .as_deref()
                .map(ArcStr::from)
                .unwrap_or_else(|| ArcStr::from("chunk")),
              module.stable_id.to_string(),
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
