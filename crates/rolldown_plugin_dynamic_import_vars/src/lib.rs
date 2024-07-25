use clone_expr::clone_expr;
use glob::glob;
use oxc::{
  allocator::Vec,
  ast::{
    ast::{
      Argument, BindingRestElement, Declaration, Expression, FormalParameterKind, FormalParameters,
      FunctionBody, FunctionType, Statement, SwitchCase, TSTypeAnnotation,
      TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    },
    AstBuilder, VisitMut,
  },
  span::SPAN,
  syntax::operator::{BinaryOperator, UnaryOperator},
};
use rolldown_plugin::{HookTransformAstArgs, HookTransformAstReturn, Plugin, SharedPluginContext};
use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};
use sugar_path::SugarPath;
use to_glob::to_glob_pattern;
mod clone_expr;
mod should_ignore;
mod to_glob;

#[derive(Debug)]
pub struct DynamicImportVarsPlugin {
  pub error_when_no_files_found: bool,
}

#[async_trait::async_trait]
impl Plugin for DynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("dynamic_import_vars")
  }

  fn transform_ast(
    &self,
    _ctx: &SharedPluginContext,
    mut args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let ast_builder: AstBuilder = AstBuilder::new(fields.allocator);
      let mut visitor = DynamicImportVarsVisit {
        cwd: args.cwd,
        ast_builder,
        error_when_no_files_found: self.error_when_no_files_found,
        helper_decls: ast_builder.vec(),
        current: 0,
      };
      visitor.visit_program(fields.program);
      if !visitor.helper_decls.is_empty() {
        fields.program.body.extend(visitor.helper_decls);
      }
    });
    Ok(args.ast)
  }
}

pub struct DynamicImportVarsVisit<'ast, 'a> {
  cwd: &'a PathBuf,
  ast_builder: AstBuilder<'ast>,
  error_when_no_files_found: bool,
  helper_decls: Vec<'ast, Statement<'ast>>,
  current: usize,
}

impl<'ast, 'a> VisitMut<'ast> for DynamicImportVarsVisit<'ast, 'a> {
  #[allow(clippy::too_many_lines)]
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if let Expression::ImportExpression(import_expr) = expr {
      let pattern = to_glob_pattern(&import_expr.source).unwrap();
      if let Some(pattern) = pattern {
        let path = Path::new(self.cwd).join(Path::new(&pattern));
        let mut files = vec![];
        if path.is_absolute() {
          if let Some(path) = path.to_str() {
            for file in glob(path).unwrap() {
              let file = file.unwrap().as_path().relative(self.cwd).to_slash_lossy().to_string();
              files.push(format!("./{file}"));
            }
          }
        }

        if self.error_when_no_files_found && files.is_empty() {
          panic!("No files found in {pattern:?} when trying to dynamically load concatted string from {:?}", self.cwd)
        }

        let name = format!("__variableDynamicImportRuntime{}__", self.current);
        let import_arg = import_expr.arguments.first();

        let helper_decl = self.helper_func(&name, files, import_arg);
        self.helper_decls.push(self.ast_builder.statement_declaration(helper_decl));

        *expr = self.ast_builder.expression_call(
          import_expr.span,
          self.ast_builder.vec1(
            self
              .ast_builder
              .argument_expression(clone_expr(&self.ast_builder, &import_expr.source)),
          ),
          self.ast_builder.expression_identifier_reference(SPAN, name),
          Option::<TSTypeParameterInstantiation>::None,
          false,
        );
        self.current += 1;
      }
    }
  }
}

impl<'ast, 'a> DynamicImportVarsVisit<'ast, 'a> {
  /// generates helper function declaration
  fn helper_func(
    &self,
    name: &String,
    files: std::vec::Vec<String>,
    import_arg: Option<&Expression<'ast>>,
  ) -> Declaration<'ast> {
    self.ast_builder.declaration_function(
      FunctionType::FunctionDeclaration,
      SPAN,
      Some(self.ast_builder.binding_identifier(SPAN, name)),
      false,
      false,
      false,
      Option::<TSTypeParameterDeclaration>::None,
      None,
      self.ast_builder.formal_parameters(
        SPAN,
        FormalParameterKind::FormalParameter,
        self.ast_builder.vec1(self.ast_builder.formal_parameter(
          SPAN,
          self.ast_builder.vec(),
          self.ast_builder.binding_pattern(
            self.ast_builder.binding_pattern_kind_binding_identifier(SPAN, "path"),
            Option::<TSTypeAnnotation>::None,
            false,
          ),
          None,
          false,
          false,
        )),
        Option::<BindingRestElement>::None,
      ),
      Option::<TSTypeAnnotation>::None,
      Some(self.ast_builder.function_body(
        SPAN,
        self.ast_builder.vec(),
        self.ast_builder.vec1(self.ast_builder.statement_switch(
          SPAN,
          self.ast_builder.expression_identifier_reference(SPAN, "path"),
          self.switch_cases(files, import_arg),
        )),
      )),
    )
  }

  /// generates:
  /// ```js
  /// case "./file.js": return import("./file.js");
  /// ```
  fn switch_cases(
    &self,
    files: std::vec::Vec<String>,
    import_arg: Option<&Expression<'ast>>,
  ) -> Vec<'ast, SwitchCase<'ast>> {
    let mut items = self.ast_builder.vec_with_capacity(files.len() + 1);
    for file in files {
      items.push(self.ast_builder.switch_case(
        SPAN,
        Some(self.ast_builder.expression_string_literal(SPAN, file.clone())),
        self.ast_builder.vec1(self.ast_builder.statement_return(
          SPAN,
          Some(self.ast_builder.expression_import(
            SPAN,
            self.ast_builder.expression_string_literal(SPAN, file),
            import_arg.map_or_else(
              || self.ast_builder.vec(),
              |arg: &Expression<'ast>| self.ast_builder.vec1(clone_expr(&self.ast_builder, arg)),
            ),
          )),
        )),
      ));
    }
    items.push(self.default_case());
    items
  }

  /// generates:
  /// ```js
  /// default: return new Promise(function(resolve, reject) {
  ///   (typeof queueMicrotask === 'function' ? queueMicrotask : setTimeout)(
  ///     reject.bind(null, new Error("Unknown variable dynamic import: " + path))
  ///   );
  /// })
  /// ```
  fn default_case(&self) -> SwitchCase<'ast> {
    self.ast_builder.switch_case(
      SPAN,
      None,
      self.ast_builder.vec1(self.ast_builder.statement_return(
        SPAN,
        Some(self.ast_builder.expression_new(
          SPAN,
          self.ast_builder.expression_identifier_reference(SPAN, "Promise"),
          self.ast_builder.vec1(self.ast_builder.argument_expression(
            self.ast_builder.expression_function(
              FunctionType::FunctionExpression,
              SPAN,
              None,
              false,
              false,
              false,
              Option::<TSTypeParameterDeclaration>::None,
              None,
              self.promise_cb_params(),
              Option::<TSTypeAnnotation>::None,
              Some(self.promise_cb_body()),
            ),
          )),
          Option::<TSTypeParameterInstantiation>::None,
        )),
      )),
    )
  }

  /// generates:
  /// ```js
  /// resolve, reject
  /// ```
  fn promise_cb_params(&self) -> FormalParameters<'ast> {
    let mut items = self.ast_builder.vec_with_capacity(2);
    for name in &["resolve", "reject"] {
      items.push(
        self.ast_builder.formal_parameter(
          SPAN,
          self.ast_builder.vec(),
          self.ast_builder.binding_pattern(
            self
              .ast_builder
              .binding_pattern_kind_binding_identifier(SPAN, self.ast_builder.atom(name)),
            Option::<TSTypeAnnotation>::None,
            false,
          ),
          None,
          false,
          false,
        ),
      );
    }
    self.ast_builder.formal_parameters(
      SPAN,
      FormalParameterKind::FormalParameter,
      items,
      Option::<BindingRestElement>::None,
    )
  }

  /// generates:
  /// ```js
  /// (typeof queueMicrotask === 'function' ? queueMicrotask : setTimeout)(
  ///   reject.bind(null, new Error("Unknown variable dynamic import: " + path))
  /// );
  /// ```
  fn promise_cb_body(&self) -> FunctionBody<'ast> {
    self.ast_builder.function_body(
      SPAN,
      self.ast_builder.vec(),
      self.ast_builder.vec1(self.ast_builder.statement_expression(
        SPAN,
        self.ast_builder.expression_call(
          SPAN,
          self.ast_builder.vec1(self.ast_builder.argument_expression(
            self.ast_builder.expression_call(
              SPAN,
              self.reject_fn_bind_args(),
              self.ast_builder.expression_member(self.ast_builder.member_expression_static(
                SPAN,
                self.ast_builder.expression_identifier_reference(SPAN, "reject"),
                self.ast_builder.identifier_name(SPAN, "bind"),
                false,
              )),
              Option::<TSTypeParameterInstantiation>::None,
              false,
            ),
          )),
          self.ast_builder.expression_conditional(
            SPAN,
            self.ast_builder.expression_binary(
              SPAN,
              self.ast_builder.expression_unary(
                SPAN,
                UnaryOperator::Typeof,
                self.ast_builder.expression_identifier_reference(SPAN, "queueMicrotask"),
              ),
              BinaryOperator::StrictEquality,
              self.ast_builder.expression_string_literal(SPAN, "function"),
            ),
            self.ast_builder.expression_identifier_reference(SPAN, "queueMicrotask"),
            self.ast_builder.expression_identifier_reference(SPAN, "setTimeout"),
          ),
          Option::<TSTypeParameterInstantiation>::None,
          false,
        ),
      )),
    )
  }

  /// generates:
  /// ```js
  /// null, new Error("Unknown variable dynamic import: " + path)
  /// ```
  fn reject_fn_bind_args(&self) -> Vec<'ast, Argument<'ast>> {
    let mut items = self.ast_builder.vec_with_capacity(2);
    items
      .push(self.ast_builder.argument_expression(self.ast_builder.expression_null_literal(SPAN)));
    items.push(self.ast_builder.argument_expression(self.ast_builder.expression_new(
      SPAN,
      self.ast_builder.expression_identifier_reference(SPAN, "Error"),
      self.ast_builder.vec1(self.ast_builder.argument_expression(
        self.ast_builder.expression_binary(
          SPAN,
          self.ast_builder.expression_string_literal(SPAN, "Unknown variable dynamic import: "),
          BinaryOperator::Addition,
          self.ast_builder.expression_identifier_reference(SPAN, "path"),
        ),
      )),
      Option::<TSTypeParameterInstantiation>::None,
    )));

    items
  }
}
