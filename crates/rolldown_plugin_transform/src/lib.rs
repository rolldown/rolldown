mod types;
mod utils;

use std::path::Path;
use std::{borrow::Cow, sync::Arc};

use oxc::codegen::{CodeGenerator, CodegenOptions, CodegenReturn};
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::transformer::Transformer;
use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookUsage, Plugin, PluginContextResolveOptions, SharedTransformPluginContext,
};
use rolldown_utils::{clean_url::clean_url, pattern_filter::StringOrRegex};

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
    let ext = Path::new(args.id).extension().map(|s| s.to_string_lossy());
    let module_type = ext.as_ref().map(|s| ModuleType::from_str_with_fallback(clean_url(s)));
    if !self.filter(args.id, &cwd, &module_type) {
      return Ok(None);
    }

    let (source_type, transform_options) =
      self.get_modified_transform_options(&ctx, args.id, &cwd, ext.as_deref())?;

    let allocator = oxc::allocator::Allocator::default();
    let ret = Parser::new(&allocator, args.code, source_type).parse();

    if ret.panicked || !ret.errors.is_empty() {
      // TODO: Improve diagnostics handling
      Err(anyhow::anyhow!("Error occurred when parsing {}\n: {:?}", args.id, ret.errors))?;
    }

    let mut program = ret.program;
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let transformer = Transformer::new(&allocator, Path::new(args.id), &transform_options);

    let ret = transformer.build_with_scoping(scoping, &mut program);

    if !ret.errors.is_empty() {
      // TODO: better error handling
      Err(anyhow::anyhow!("Transform failed, got {:#?}", ret.errors))?;
    }

    let mut codegen_options = CodegenOptions::default();

    if self.sourcemap {
      codegen_options.source_map_path = Some(args.id.into());
    }

    let ret = CodeGenerator::new().with_options(codegen_options).build(&program);
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
