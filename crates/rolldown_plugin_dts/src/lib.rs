use std::borrow::Cow;

use anyhow::Ok;
use ast_visitors::TypeImportVisitor;
use oxc::{
  allocator::IntoIn,
  ast_visit::VisitMut,
  codegen::{CodeGenerator, CodegenOptions},
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
  semantic::{Scoping, SemanticBuilder},
  span::SourceType,
};
use rolldown_common::{ModuleId, ModuleType};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_plugin::{Plugin, PluginHookMeta, PluginOrder};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};

mod ast_visitors;
mod types;

#[derive(Debug, Default)]
pub struct DtsPlugin {
  pub strip_internal: bool,
  pub asts: FxDashMap<ModuleId, (EcmaAst, Scoping)>,
  pub entries: FxDashSet<ModuleId>,
}

impl DtsPlugin {
  pub fn new(strip_internal: bool) -> Self {
    Self { strip_internal, ..Default::default() }
  }
}

impl Plugin for DtsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dts")
  }

  async fn transform_ast(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    if matches!(args.module_type, ModuleType::Ts | ModuleType::Tsx) {
      // scan original code to find typing imports, the dts need to load them.
      let type_import_specifiers = args.ast.program.with_mut(|fields| {
        let mut visitor = TypeImportVisitor { imported: vec![].into_in(fields.allocator) };
        visitor.visit_program(fields.program);
        visitor.imported
      });

      for specifier in type_import_specifiers {
        let resolved_id = ctx.resolve(&specifier, Some(args.id), None).await??;
        ctx.load(&resolved_id.id, None, None).await?;
      }

      let ret = args.ast.program.with_mut(|fields| {
        IsolatedDeclarations::new(
          fields.allocator,
          IsolatedDeclarationsOptions { strip_internal: self.strip_internal },
        )
        .build(fields.program)
      });

      // TODO BuildDiagnostic error
      if !ret.errors.is_empty() {
        return Err(anyhow::anyhow!("IsolatedDeclarations error"));
      }

      // Here using the ret.program to generate the d.ts file, and parse it to ast.
      // TODO here maybe could be using the isolated declarations ast directly, it need to reference the original ast at now.
      let codegen_ret =
        CodeGenerator::new().with_options(CodegenOptions::default()).build(&ret.program);

      let ast = // TODO handle error
      EcmaCompiler::parse("", codegen_ret.code, SourceType::ts()).expect("should success");

      let scoping =
        ast.make_symbol_table_and_scope_tree_with_semantic_builder(SemanticBuilder::new());

      self.asts.insert(ModuleId::new(args.id), (ast, scoping));

      if args.is_user_defined_entry {
        self.entries.insert(ModuleId::new(args.id));
      }
    }
    Ok(args.ast)
  }

  // The rolldown strip types at the end of the build process, make sure to run this plugin before that.
  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }

  // async fn generate_bundle(
  //   &self,
  //   _ctx: &rolldown_plugin::PluginContext,
  //   args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  // ) -> rolldown_plugin::HookNoopReturn {
  //   for output in args.bundle.iter() {
  //     if let Output::Chunk(chunk) = output {
  //       for module in &chunk.modules.keys {
  //         if let Some((ast, _)) = self.asts.get(module) {
  //           args.bundle.push(Output::Dts(module.clone(), ast.clone()));
  //         }
  //       }
  //     }
  //   }
  //   Ok(())
  // }
}
