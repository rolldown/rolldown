use oxc::{
  ast::{AstBuilder, NONE, ast},
  span::SPAN,
};
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

    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id == HMR_RUNTIME_MODULE_SPECIFIER {
      let runtime_source = arcstr::literal!(include_str!("./runtime/index.js"));
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
    if args.is_user_defined_entry {
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
