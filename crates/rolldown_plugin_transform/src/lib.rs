mod types;
mod utils;

use std::path::Path;
use std::{borrow::Cow, sync::Arc};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::transformer::Transformer;
use rolldown_common::ModuleType;
use rolldown_error::{BuildDiagnostic, Severity};
use rolldown_plugin::{
  HookUsage, Plugin, PluginContextResolveOptions, SharedTransformPluginContext,
};
use rolldown_utils::{pattern_filter::StringOrRegex, stabilize_id::stabilize_id, url::clean_url};

pub use types::{
  CompilerAssumptions, DecoratorOptions, IsolatedDeclarationsOptions, JsxOptions,
  ReactRefreshOptions, TransformOptions, TypeScriptOptions,
};

#[derive(Debug, Default)]
pub struct TransformPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_refresh_include: Vec<StringOrRegex>,
  pub jsx_refresh_exclude: Vec<StringOrRegex>,

  pub jsx_inject: Option<String>,

  pub is_server_consumer: bool,
  pub runtime_resolve_base: Option<String>,

  pub sourcemap: bool,
  pub transform_options: TransformOptions,
}

/// only handle ecma like syntax, `jsx`,`tsx`,`ts`
impl Plugin for TransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:transform")
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.specifier.starts_with("@oxc-project/runtime/") {
      let resolved_id = ctx
        .resolve(
          args.specifier,
          self.runtime_resolve_base.as_deref(),
          Some(PluginContextResolveOptions {
            skip_self: true,
            import_kind: args.kind,
            custom: Arc::clone(&args.custom),
          }),
        )
        .await??;

      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: resolved_id.id,
        external: Some(resolved_id.external),
        side_effects: resolved_id.side_effects,
        normalize_external_id: resolved_id.normalize_external_id,
      }));
    }

    Ok(None)
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let cwd = ctx.inner.cwd().to_string_lossy();
    let extension = Path::new(args.id).extension().map(|s| s.to_string_lossy());
    let extension = extension.as_ref().map(|s| clean_url(s));
    let module_type = extension.map(ModuleType::from_str_with_fallback);
    if !self.filter(args.id, &cwd, &module_type) {
      return Ok(None);
    }

    let (source_type, transform_options) =
      self.get_modified_transform_options(&ctx, args.id, &cwd, extension)?;

    let allocator = oxc::allocator::Allocator::default();
    let ret = Parser::new(&allocator, args.code, source_type).parse();
    if ret.panicked || !ret.errors.is_empty() {
      let errors = BuildDiagnostic::from_oxc_diagnostics(
        ret.errors,
        &ArcStr::from(args.code.as_str()),
        &stabilize_id(args.id, ctx.inner.cwd()),
        &Severity::Error,
      )
      .iter()
      .map(|error| error.to_diagnostic().with_kind(self.name().into_owned()).to_color_string())
      .join("\n\n");
      Err(anyhow::anyhow!("\n{errors}"))?;
    }

    let mut program = ret.program;
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let transformer = Transformer::new(&allocator, Path::new(args.id), &transform_options);

    let transformer_return = transformer.build_with_scoping(scoping, &mut program);
    if !transformer_return.errors.is_empty() {
      let errors = BuildDiagnostic::from_oxc_diagnostics(
        transformer_return.errors,
        &ArcStr::from(args.code.as_str()),
        &stabilize_id(args.id, ctx.inner.cwd()),
        &Severity::Error,
      )
      .iter()
      .map(|error| error.to_diagnostic().with_kind(self.name().into_owned()).to_color_string())
      .join("\n\n");
      Err(anyhow::anyhow!("\n{errors}"))?;
    }

    let ret = Codegen::new()
      .with_options(CodegenOptions {
        comments: false,
        source_map_path: Some(args.id.into()),
        ..CodegenOptions::default()
      })
      .build(&program);
    let CodegenReturn { mut code, map, .. } = ret;

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

  fn register_hook_usage(&self) -> HookUsage {
    if self.runtime_resolve_base.is_some() {
      HookUsage::ResolveId | HookUsage::Transform
    } else {
      HookUsage::Transform
    }
  }
}
