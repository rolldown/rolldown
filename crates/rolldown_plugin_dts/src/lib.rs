use std::borrow::Cow;

use anyhow::Ok;
use ast_visitors::DtsAstScanner;
use oxc::{
  ast_visit::Visit,
  codegen::{CodeGenerator, CodegenOptions},
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
  semantic::SemanticBuilder,
  span::SourceType,
};
use rolldown_common::{ModuleId, ModuleType};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_plugin::{Plugin, PluginHookMeta, PluginOrder};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use types::DtsModule;

mod ast_visitors;
mod types;

#[derive(Debug, Default)]
pub struct DtsPlugin {
  pub strip_internal: bool,
  pub dts_modules: FxDashMap<ModuleId, DtsModule>,
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
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    if matches!(args.module_type, ModuleType::Ts | ModuleType::Tsx) {
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

      let mut dts_ast = // TODO handle error
      EcmaCompiler::parse(args.id, codegen_ret.code, SourceType::ts()).expect("should success");

      let scoping =
        dts_ast.make_symbol_table_and_scope_tree_with_semantic_builder(SemanticBuilder::new());

      let module_id = ModuleId::new(args.id);

      let mut dts_ast_scanner = DtsAstScanner::new(scoping, args.module_index, &module_id);

      dts_ast.program.with_mut(|fields| {
        dts_ast_scanner.visit_program(fields.program);
      });

      // for specifier in type_import_specifiers {
      //   let resolved_id = ctx.resolve(&specifier, Some(args.id), None).await??;
      //   ctx.load(&resolved_id.id, None, None).await?;
      // }

      self.dts_modules.insert(
        module_id.clone(),
        DtsModule {
          module_index: args.module_index,
          module_id: module_id.clone(),
          symbol_ref_db: dts_ast_scanner.symbol_ref_db,
          named_imports: dts_ast_scanner.named_imports,
          named_exports: dts_ast_scanner.named_exports,
          stmt_infos: dts_ast_scanner.stmt_infos,
          import_records: dts_ast_scanner.import_records,
          default_export_ref: dts_ast_scanner.default_export_ref,
          namespace_object_ref: dts_ast_scanner.namespace_object_ref,
          dts_ast,
        },
      );

      if args.is_user_defined_entry {
        self.entries.insert(module_id);
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
