use arcstr::ArcStr;
use oxc::{
  ast::{
    ast::{
      Program,
      // BinaryOperator, BindingRestElement, FormalParameterKind, FunctionType, ImportOrExportKind,
      //  TSAccessibility, TSThisParameter, TSTypeAnnotation, TSTypeParameterDeclaration,
      // TSTypeParameterInstantiation, WithClause,
    },
    visit::walk_mut,
    AstBuilder, VisitMut,
  },
  // span::{Span, SPAN},
};
use rolldown_plugin::{
  HookLoadOutput,
  Plugin,
  PluginContext, // HookTransformAstArgs, HookTransformAstReturn,
};
use std::borrow::Cow;

#[derive(Debug, Default)]
pub struct ReactPlugin {}

impl Plugin for ReactPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:react")
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.specifier == "react-refresh-entry.js" {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }
    if args.specifier == "react-refresh/runtime" {
      let id = ctx.resolve(args.specifier, None, None).await??;
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: id.id.to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id == "react-refresh-entry.js" {
      return Ok(Some(HookLoadOutput {
        code: r#"
  import RefreshRuntime from "react-refresh/runtime"
  RefreshRuntime.injectIntoGlobalHook(window);
  function debounce(fn, delay) {
    let handle
    return () => {
      clearTimeout(handle)
      handle = setTimeout(fn, delay)
    }
  }
  RefreshRuntime.performReactRefresh = debounce(RefreshRuntime.performReactRefresh, 16);"#
          .to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  #[allow(clippy::case_sensitive_file_extension_comparisons)]
  async fn transform(
    &self,
    _ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id.ends_with(".jsx") {
      let mut content = args.code.to_string();
      content.push_str(&format!(
        r#"
import RefreshRuntime from 'react-refresh/runtime';
function $RefreshSig$() {{
  return RefreshRuntime.createSignatureFunctionForTransform();
}}
function $RefreshReg$(type, id) {{
  RefreshRuntime.register(type, '{}' + '_' + id);
}}
if (import.meta.hot) {{
  import.meta.hot.accept();
  RefreshRuntime.performReactRefresh();
}}
        "#,
        args.id
      ));
      return Ok(Some(rolldown_plugin::HookTransformOutput {
        code: Some(content),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  // fn transform_ast(
  //   &self,
  //   _ctx: &PluginContext,
  //   mut args: HookTransformAstArgs,
  // ) -> HookTransformAstReturn {
  //   // TODO here need to make sure the react-refresh run at behind.
  //   if args.id.ends_with(".jsx") {
  //     args.ast.program.with_mut(|fields| {
  //       let ast_builder: AstBuilder = AstBuilder::new(fields.allocator);
  //       let mut visitor =
  //         ReactHmrVisit { ast_builder, found_refresh_usage: false, module_id: args.id.into() };
  //       visitor.visit_program(fields.program);
  //     });
  //   }
  //   Ok(args.ast)
  // }
}

#[allow(dead_code)]
struct ReactHmrVisit<'ast> {
  ast_builder: AstBuilder<'ast>,
  found_refresh_usage: bool,
  module_id: ArcStr,
}

impl<'ast> VisitMut<'ast> for ReactHmrVisit<'ast> {
  fn visit_program(&mut self, program: &mut Program<'ast>) {
    walk_mut::walk_program(self, program);

    // if !self.found_refresh_usage {
    //   return;
    // }

    // Need to find why the import lost at final.
    // import RefreshRuntime from "react-refresh/runtime"
    // program.body.push(self.ast_builder.statement_module_declaration(
    //   self.ast_builder.module_declaration_import_declaration(
    //     // TODO Create a different span for each import expression
    //     Span::new(1, 10000),
    //     Some(self.ast_builder.vec1(
    //       self.ast_builder.import_declaration_specifier_import_default_specifier(
    //         SPAN,
    //         self.ast_builder.binding_identifier(SPAN, "RefreshRuntime"),
    //       ),
    //     )),
    //     self.ast_builder.string_literal(SPAN, "react-refresh/runtime"),
    //     None::<WithClause>,
    //     ImportOrExportKind::Value,
    //   ),
    // ));

    // // function $RefreshSig$() {
    // //   return RefreshRuntime.createSignatureFunctionForTransform();
    // // }
    // program.body.push(self.ast_builder.statement_declaration(
    //   self.ast_builder.declaration_function(
    //     FunctionType::FunctionDeclaration,
    //     SPAN,
    //     Some(self.ast_builder.binding_identifier(SPAN, "$RefreshSig$")),
    //     false,
    //     false,
    //     false,
    //     None::<TSTypeParameterDeclaration>,
    //     None::<TSThisParameter>,
    //     self.ast_builder.formal_parameters(
    //       SPAN,
    //       FormalParameterKind::FormalParameter,
    //       self.ast_builder.vec(),
    //       None::<BindingRestElement>,
    //     ),
    //     None::<TSTypeAnnotation>,
    //     Some(self.ast_builder.function_body(
    //       SPAN,
    //       self.ast_builder.vec(),
    //       self.ast_builder.vec1(self.ast_builder.statement_return(
    //         SPAN,
    //         Some(self.ast_builder.expression_call(
    //           SPAN,
    //           self.ast_builder.expression_member(self.ast_builder.member_expression_static(
    //             SPAN,
    //             self.ast_builder.expression_identifier_reference(SPAN, "RefreshRuntime"),
    //             self.ast_builder.identifier_name(SPAN, "createSignatureFunctionForTransform"),
    //             false,
    //           )),
    //           None::<TSTypeParameterInstantiation>,
    //           self.ast_builder.vec(),
    //           false,
    //         )),
    //       )),
    //     )),
    //   ),
    // ));

    // // function $RefreshReg$(type, id) {
    // //   RefreshRuntime.register(type, module.id + "_" + id);
    // // }
    // program.body.push(self.ast_builder.statement_declaration(
    //   self.ast_builder.declaration_function(
    //     FunctionType::FunctionDeclaration,
    //     SPAN,
    //     Some(self.ast_builder.binding_identifier(SPAN, "$RefreshReg$")),
    //     false,
    //     false,
    //     false,
    //     None::<TSTypeParameterDeclaration>,
    //     None::<TSThisParameter>,
    //     self.ast_builder.formal_parameters(
    //       SPAN,
    //       FormalParameterKind::FormalParameter,
    //       {
    //         let mut items = self.ast_builder.vec_with_capacity(2);
    //         items.push(self.ast_builder.formal_parameter(
    //           SPAN,
    //           self.ast_builder.vec(),
    //           self.ast_builder.binding_pattern(
    //             self.ast_builder.binding_pattern_kind_binding_identifier(SPAN, "type"),
    //             None::<TSTypeAnnotation>,
    //             false,
    //           ),
    //           None::<TSAccessibility>,
    //           false,
    //           false,
    //         ));
    //         items.push(self.ast_builder.formal_parameter(
    //           SPAN,
    //           self.ast_builder.vec(),
    //           self.ast_builder.binding_pattern(
    //             self.ast_builder.binding_pattern_kind_binding_identifier(SPAN, "id"),
    //             None::<TSTypeAnnotation>,
    //             false,
    //           ),
    //           None::<TSAccessibility>,
    //           false,
    //           false,
    //         ));
    //         items
    //       },
    //       None::<BindingRestElement>,
    //     ),
    //     None::<TSTypeAnnotation>,
    //     Some(self.ast_builder.function_body(
    //       SPAN,
    //       self.ast_builder.vec(),
    //       self.ast_builder.vec1(self.ast_builder.statement_return(
    //         SPAN,
    //         Some(self.ast_builder.expression_call(
    //           SPAN,
    //           self.ast_builder.expression_member(self.ast_builder.member_expression_static(
    //             SPAN,
    //             self.ast_builder.expression_identifier_reference(SPAN, "RefreshRuntime"),
    //             self.ast_builder.identifier_name(SPAN, "register"),
    //             false,
    //           )),
    //           None::<TSTypeParameterInstantiation>,
    //           {
    //             let mut items = self.ast_builder.vec_with_capacity(2);
    //             items.push(self.ast_builder.argument_expression(
    //               self.ast_builder.expression_identifier_reference(SPAN, "type"),
    //             ));
    //             items.push(self.ast_builder.argument_expression(
    //               self.ast_builder.expression_binary(
    //                 SPAN,
    //                 self.ast_builder.expression_binary(
    //                   SPAN,
    //                   self.ast_builder.expression_string_literal(SPAN, self.module_id.as_str()),
    //                   BinaryOperator::Addition,
    //                   self.ast_builder.expression_string_literal(SPAN, "_"),
    //                 ),
    //                 BinaryOperator::Addition,
    //                 self.ast_builder.expression_identifier_reference(SPAN, "id"),
    //               ),
    //             ));
    //             items
    //           },
    //           false,
    //         )),
    //       )),
    //     )),
    //   ),
    // ));

    // // if (import.meta.hot) {
    // //   import.meta.hot.accept();
    // // }
    // program.body.push(self.ast_builder.statement_if(
    //   SPAN,
    //   self.ast_builder.expression_member(self.ast_builder.member_expression_static(
    //     SPAN,
    //     self.ast_builder.expression_meta_property(
    //       SPAN,
    //       self.ast_builder.identifier_name(SPAN, "import"),
    //       self.ast_builder.identifier_name(SPAN, "meta"),
    //     ),
    //     self.ast_builder.identifier_name(SPAN, "hot"),
    //     false,
    //   )),
    //   self.ast_builder.statement_block(SPAN, {
    //     let mut items = self.ast_builder.vec_with_capacity(2);
    //     items.push(self.ast_builder.statement_expression(
    //       SPAN,
    //       self.ast_builder.expression_call(
    //         SPAN,
    //         self.ast_builder.expression_member(self.ast_builder.member_expression_static(
    //           SPAN,
    //           self.ast_builder.expression_member(self.ast_builder.member_expression_static(
    //             SPAN,
    //             self.ast_builder.expression_meta_property(
    //               SPAN,
    //               self.ast_builder.identifier_name(SPAN, "import"),
    //               self.ast_builder.identifier_name(SPAN, "meta"),
    //             ),
    //             self.ast_builder.identifier_name(SPAN, "hot"),
    //             false,
    //           )),
    //           self.ast_builder.identifier_name(SPAN, "accept"),
    //           false,
    //         )),
    //         None::<TSTypeParameterInstantiation>,
    //         self.ast_builder.vec(),
    //         false,
    //       ),
    //     ));
    //     items.push(self.ast_builder.statement_expression(
    //       SPAN,
    //       self.ast_builder.expression_call(
    //         SPAN,
    //         self.ast_builder.expression_member(self.ast_builder.member_expression_static(
    //           SPAN,
    //           self.ast_builder.expression_identifier_reference(SPAN, "RefreshRuntime"),
    //           self.ast_builder.identifier_name(SPAN, "performReactRefresh"),
    //           false,
    //         )),
    //         None::<TSTypeParameterInstantiation>,
    //         self.ast_builder.vec(),
    //         false,
    //       ),
    //     ));
    //     items
    //   }),
    //   None,
    // ));
  }

  fn visit_identifier_reference(
    &mut self,
    identifier_reference: &mut oxc::ast::ast::IdentifierReference<'ast>,
  ) {
    if &identifier_reference.name == "$RefreshReg$" || &identifier_reference.name == "$RefreshSig$"
    {
      self.found_refresh_usage = true;
    }
  }
}
