use arcstr::ArcStr;
use oxc::{
  ast::{AstBuilder, NONE, ast},
  span::SPAN,
};
use rolldown_common::{Platform, ResolvedExternal};
use rolldown_plugin::{HookLoadOutput, HookResolveIdOutput, HookUsage, Plugin};

use crate::HMR_RUNTIME_MODULE_SPECIFIER;

#[derive(Debug)]
pub struct HmrPlugin;

impl Plugin for HmrPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "builtin:hmr".into()
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::TransformAst | HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.specifier == HMR_RUNTIME_MODULE_SPECIFIER {
      return Ok(Some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }));
    }
    if args.specifier == "ws" {
      // FIXME(hyf0): As the dependency of `rolldown:hmr`, `ws` has a advanced execution timing than `rolldown:hmr`, which cause a runtime error.
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id == HMR_RUNTIME_MODULE_SPECIFIER {
      let mut runtime_source = String::new();
      let bundler_options = ctx.options();

      if let Some(hmr_options) = &bundler_options.experimental.hmr {
        match bundler_options.platform {
          Platform::Node => {
            runtime_source.push_str("import { WebSocket } from 'ws';\n");
          }
          Platform::Browser | Platform::Neutral => {
            // Browser platform should use the native WebSocket and neutral platform doesn't have any assumptions.
          }
        }

        runtime_source.push_str(include_str!("./runtime/runtime-extra-dev-common.js"));

        if let Some(implement) = hmr_options.implement.as_deref() {
          runtime_source.push_str(implement);
        } else {
          let content = include_str!("./runtime/runtime-extra-dev-default.js");
          let host = hmr_options.host.as_deref().unwrap_or("localhost");
          let port = hmr_options.port.unwrap_or(3000);
          let addr = format!("{host}:{port}");
          runtime_source.push_str(&content.replace("$ADDR", &addr));
        }
      }

      let runtime_source = ArcStr::from(runtime_source);
      return Ok(Some(HookLoadOutput { code: runtime_source, ..Default::default() }));
    }

    Ok(None)
  }

  fn load_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }

  async fn transform_ast(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    // We need to inject `import 'rolldown:hmr';` to every module to ensure the HMR runtime is loaded when the module is executed.
    // No need to worry about `rolldown:runtime` , which doesn't go through the `transform_ast` hook.
    if args.id != HMR_RUNTIME_MODULE_SPECIFIER {
      // Inject `import 'rolldown:hmr';` to all user defined entry points to ensure the HMR runtime is loaded.
      args.ast.program.with_mut(|fields| {
        let ast_builder = AstBuilder::new(fields.allocator);

        // `import 'rolldown:hmr';`
        let import_stmt = ast::Statement::ImportDeclaration(ast_builder.alloc_import_declaration(
          SPAN,
          None,
          ast_builder.string_literal(SPAN, HMR_RUNTIME_MODULE_SPECIFIER, None),
          None,
          NONE,
          ast::ImportOrExportKind::Value,
        ));
        fields.program.body.insert(0, import_stmt);
      });
    }

    Ok(args.ast)
  }

  fn transform_ast_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }
}
