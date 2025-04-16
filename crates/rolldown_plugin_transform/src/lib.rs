mod utils;

use std::borrow::Cow;
use std::path::Path;

use oxc::{
  codegen::{CodeGenerator, CodegenOptions, CodegenReturn},
  semantic::SemanticBuilder,
  span::SourceType,
  transformer::{EnvOptions, TransformOptions, Transformer},
};

use rolldown_common::ModuleType;
use rolldown_ecmascript::EcmaCompiler;
use rolldown_plugin::{Plugin, SharedTransformPluginContext};
use rolldown_utils::{clean_url::clean_url, pattern_filter::StringOrRegex};

#[derive(Debug, Default)]
pub struct TransformPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_refresh_include: Vec<StringOrRegex>,
  pub jsx_refresh_exclude: Vec<StringOrRegex>,

  pub jsx_inject: Option<String>,
}

/// only handle ecma like syntax, `jsx`,`tsx`,`ts`
impl Plugin for TransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:transform")
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let cwd = ctx.inner.cwd().to_string_lossy();
    let ext = Path::new(args.id).extension().map(|s| s.to_string_lossy());
    let module_type = ext.as_ref().map(|s| ModuleType::from_str_with_fallback(clean_url(s)));
    if !self.filter(args.id, &cwd, &module_type) {
      return Ok(None);
    }

    let source_type = {
      let default_source_type = SourceType::default();
      match args.module_type {
        ModuleType::Jsx => default_source_type.with_jsx(true),
        ModuleType::Ts => default_source_type.with_typescript(true),
        ModuleType::Tsx => default_source_type.with_typescript(true).with_jsx(true),
        _ => return Ok(None),
      }
    };

    let parse_result = EcmaCompiler::parse(args.id, args.code, source_type);
    let mut ecma_ast = parse_result.map_err(|errs| {
      // TODO: Improve diagnostics handling
      anyhow::anyhow!("Error occurred when parsing {}\n: {:?}", args.id, errs)
    })?;

    let env = EnvOptions::default();
    let ret = ecma_ast.program.with_mut(move |fields| {
      let mut transformer_options = TransformOptions { env, ..TransformOptions::default() };

      if !matches!(args.module_type, ModuleType::Ts) {
        transformer_options.jsx.jsx_plugin = true;
      }

      let scoping = SemanticBuilder::new().build(fields.program).semantic.into_scoping();
      Transformer::new(fields.allocator, Path::new(args.id), &transformer_options)
        .build_with_scoping(scoping, fields.program)
    });

    if !ret.errors.is_empty() {
      // TODO: better error handling
      Err(anyhow::anyhow!("Transform failed, got {:#?}", ret.errors))?;
    }

    let CodegenReturn { mut code, map, .. } = CodeGenerator::new()
      .with_options(CodegenOptions {
        source_map_path: Some(args.id.into()),
        ..CodegenOptions::default()
      })
      .build(ecma_ast.program());

    if let Some(inject) = &self.jsx_inject {
      let mut new_code = String::with_capacity(inject.len() + 1 + code.len());
      new_code.push_str(inject);
      new_code.push(';');
      new_code.push_str(&code);
      code = new_code;
    }

    Ok(Some(rolldown_plugin::HookTransformOutput {
      map,
      code: Some(code),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }
}
