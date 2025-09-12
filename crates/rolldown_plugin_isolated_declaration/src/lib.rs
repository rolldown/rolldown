use std::{borrow::Cow, path::Path};

use arcstr::ArcStr;
use oxc::{
  allocator::IntoIn,
  ast_visit::VisitMut,
  codegen::Codegen,
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
};
use rolldown_common::{ModuleType, ResolvedExternal};
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, Severity};
use rolldown_plugin::{HookUsage, Plugin, PluginHookMeta, PluginOrder};
use rolldown_utils::stabilize_id::stabilize_id;
use sugar_path::SugarPath;
use type_import_visitor::TypeImportVisitor;

mod type_import_visitor;

#[derive(Debug, Default)]
pub struct IsolatedDeclarationPlugin {
  pub strip_internal: bool,
}

impl Plugin for IsolatedDeclarationPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:isolated-declaration")
  }

  async fn transform_ast(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    if matches!(args.module_type, ModuleType::Ts | ModuleType::Tsx) {
      let type_import_specifiers = args.ast.program.with_mut(|fields| {
        let mut visitor = TypeImportVisitor { imported: vec![].into_in(fields.allocator) };
        visitor.visit_program(fields.program);
        visitor.imported
      });

      for specifier in type_import_specifiers {
        let resolved_id = ctx.resolve(&specifier, Some(args.id), None).await??;
        if matches!(resolved_id.external, ResolvedExternal::Bool(false)) {
          ctx.load(&resolved_id.id, None).await?;
        }
      }

      let ret = args.ast.program.with_mut(|fields| {
        IsolatedDeclarations::new(
          fields.allocator,
          IsolatedDeclarationsOptions { strip_internal: self.strip_internal },
        )
        .build(fields.program)
      });

      if !ret.errors.is_empty() {
        return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
          ret.errors,
          &ArcStr::from(ret.program.source_text),
          &stabilize_id(args.id, ctx.cwd()),
          &Severity::Error,
        )))?;
      }

      let codegen_ret = Codegen::new().build(&ret.program);

      let mut emit_dts_path = Path::new(args.stable_id).to_path_buf();
      emit_dts_path.set_extension("d.ts");
      ctx.emit_file(
        rolldown_common::EmittedAsset {
          name: None,
          original_file_name: None,
          // TODO make sure to the .d.ts file relative to the output entry file
          file_name: Some(emit_dts_path.to_slash_lossy().into()),
          source: codegen_ret.code.into(),
        },
        None,
        None,
      );
    }
    Ok(args.ast)
  }

  // The rolldown strip types at the end of the build process, make sure to run this plugin before that.
  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }
}
