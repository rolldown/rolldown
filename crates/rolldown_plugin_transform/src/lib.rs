mod utils;

use std::borrow::Cow;
use std::path::Path;

use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions};
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::transformer::Transformer;
use rolldown_common::{BundlerTransformOptions, ModuleType};
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, Severity};
use rolldown_plugin::{HookUsage, Plugin, SharedTransformPluginContext};
use rolldown_utils::{
  concat_string, pattern_filter::StringOrRegex, stabilize_id::stabilize_id, url::clean_url,
};

#[derive(Debug, Default)]
pub struct TransformPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_refresh_include: Vec<StringOrRegex>,
  pub jsx_refresh_exclude: Vec<StringOrRegex>,
  pub jsx_inject: Option<String>,
  pub is_server_consumer: bool,
  pub sourcemap: bool,
  pub transform_options: BundlerTransformOptions,
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
    let cwd = ctx.cwd().to_string_lossy();
    let extension = Path::new(args.id).extension().map(|s| s.to_string_lossy());
    let extension = extension.as_ref().map(|s| clean_url(s));
    let module_type = extension.map(ModuleType::from_str_with_fallback);
    if !self.filter(args.id, &cwd, &module_type) {
      return Ok(None);
    }

    let (source_type, transform_options) =
      self.get_modified_transform_options(&ctx, args.id, &cwd, extension, args.code)?;

    let allocator = oxc::allocator::Allocator::default();
    let ret = Parser::new(&allocator, args.code, source_type).parse();
    if ret.panicked || !ret.errors.is_empty() {
      return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
        ret.errors,
        args.code.into(),
        &stabilize_id(args.id, ctx.cwd()),
        &Severity::Error,
      )))?;
    }

    let mut program = ret.program;
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let transformer = Transformer::new(&allocator, Path::new(args.id), &transform_options);
    let transformer_return = transformer.build_with_scoping(scoping, &mut program);
    if !transformer_return.errors.is_empty() {
      return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
        transformer_return.errors,
        args.code.into(),
        &stabilize_id(args.id, ctx.cwd()),
        &Severity::Error,
      )))?;
    }

    let CodegenReturn { mut code, map, .. } = Codegen::new()
      .with_options(CodegenOptions {
        comments: CommentOptions { normal: false, ..CommentOptions::default() },
        source_map_path: self.sourcemap.then(|| args.id.into()),
        ..CodegenOptions::default()
      })
      .build(&program);

    if let Some(inject) = &self.jsx_inject {
      code = concat_string!(inject, ";", code);
    }

    Ok(Some(rolldown_plugin::HookTransformOutput {
      map,
      code: Some(code),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}
