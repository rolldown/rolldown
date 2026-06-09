mod utils;

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions};
use oxc::parser::Parser;
use oxc::transformer::Transformer;
use rolldown_common::{BundlerTransformOptions, ModuleType};
use rolldown_ecmascript::semantic_builder_for_transform;
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, EventKind, Severity};
use rolldown_plugin::{HookTransformOutputMap, HookUsage, Plugin, SharedTransformPluginContext};
use rolldown_utils::{concat_string, pattern_filter::StringOrRegex, url::clean_url};

#[derive(Debug, Default)]
pub struct ViteTransformPlugin {
  pub root: PathBuf,
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_refresh_include: Vec<StringOrRegex>,
  pub jsx_refresh_exclude: Vec<StringOrRegex>,
  pub jsx_inject: Option<String>,
  pub is_server_consumer: bool,
  pub sourcemap: bool,
  pub transform_options: BundlerTransformOptions,
  pub resolver: oxc_resolver::Resolver,
}

impl ViteTransformPlugin {
  pub fn new_resolver(yarn_pnp: bool) -> oxc_resolver::Resolver {
    oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions {
      tsconfig: Some(oxc_resolver::TsconfigDiscovery::Auto),
      yarn_pnp,
      ..Default::default()
    })
  }
}

/// only handle ecma like syntax, `jsx`,`tsx`,`ts`
impl Plugin for ViteTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-transform")
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let cwd = self.root.to_string_lossy();
    let extension = Path::new(args.id).extension().map(|s| s.to_string_lossy());
    let extension = extension.as_ref().map(|s| clean_url(s));
    let module_type = extension.map(ModuleType::from_str_with_fallback);
    if !self.filter(args.id, &cwd, module_type.as_ref()) {
      return Ok(None);
    }

    let (source_type, transform_options) =
      self.get_modified_transform_options(&ctx, args.id, &cwd, extension, args.code)?;

    let allocator = oxc::allocator::Allocator::default();
    let ret = Parser::new(&allocator, args.code, source_type).parse();
    if ret.panicked || !ret.errors.is_empty() {
      return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
        ret.errors,
        args.code,
        args.id,
        Severity::Error,
        EventKind::ParseError,
      )))?;
    }

    let mut program = ret.program;
    let mut scoping = semantic_builder_for_transform().build(&program).semantic.into_scoping();

    // Run the React Compiler as a standalone pre-transform pass, before any other
    // transform (TS/JSX lowering) runs, on the pristine AST. It rebuilds and returns
    // the scoping used by the downstream transformer. This mirrors the rolldown core
    // pass in `pre_process_ecma_ast.rs` so unbundled (Vite dev) and bundled builds
    // behave identically.
    if let Some(react_compiler_options) = &self.transform_options.react_compiler {
      let mut react_errors = Vec::new();
      scoping = oxc_react_compiler::run(
        &mut program,
        &allocator,
        scoping,
        react_compiler_options,
        &mut react_errors,
      );

      let (errors, warnings): (Vec<_>, Vec<_>) = react_errors
        .into_iter()
        .partition(|error| error.severity == oxc::diagnostics::Severity::Error);
      if !errors.is_empty() {
        return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
          errors,
          args.code,
          args.id,
          Severity::Error,
          EventKind::TransformError,
        )))?;
      }
      for warning in BuildDiagnostic::from_oxc_diagnostics(
        warnings,
        args.code,
        args.id,
        Severity::Warning,
        EventKind::ToleratedTransform,
      ) {
        ctx.warn(rolldown_common::LogWithoutPlugin {
          message: warning.to_string(),
          id: Some(args.id.to_string()),
          ..Default::default()
        });
      }
    }

    let transformer = Transformer::new(&allocator, Path::new(args.id), &transform_options);
    let transformer_return = transformer.build_with_scoping(scoping, &mut program);
    if !transformer_return.errors.is_empty() {
      return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
        transformer_return.errors,
        args.code,
        args.id,
        Severity::Error,
        EventKind::ParseError,
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
      map: if let Some(map) = map {
        map.into_inner().into()
      } else {
        HookTransformOutputMap::Omitted
      },
      code: Some(code),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}
