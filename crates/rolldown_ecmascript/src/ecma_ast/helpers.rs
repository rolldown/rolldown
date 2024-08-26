use oxc::{
  ast::ast::Program,
  semantic::{ScopeTree, Semantic, SemanticBuilder, SymbolTable},
  span::SourceType,
};
use rolldown_error::BuildDiagnostic;

use crate::EcmaAst;

impl EcmaAst {
  pub fn is_body_empty(&self) -> bool {
    self.program().is_empty()
  }

  pub fn make_semantic<'ast>(
    source: &'ast str,
    program: &'_ Program<'ast>,
    ty: SourceType,
  ) -> Result<Semantic<'ast>, Vec<BuildDiagnostic>> {
    let build_result =
      SemanticBuilder::new(source, ty).with_check_syntax_error(true).build(program);
    // TODO: log errors and warnings.
    println!("BUILD RESULT - {:?}", build_result.errors);
    if !build_result.errors.is_empty() {
      return Err(
        build_result
          .errors
          .iter()
          .map(|error| {
            BuildDiagnostic::oxc_parse_error(
              source.into(),
              "filename".to_string(),
              error.help.clone().unwrap_or_default().into(),
              error.message.to_string(),
              error.labels.clone().unwrap_or_default(),
            )
          })
          .collect::<Vec<_>>(),
      );
    }

    Ok(build_result.semantic)
  }

  pub fn make_symbol_table_and_scope_tree(
    &self,
  ) -> Result<(SymbolTable, ScopeTree), Vec<BuildDiagnostic>> {
    self.program.with_dependent(|owner, dep| {
      Self::make_semantic(&owner.source, &dep.program, self.source_type)
        .map(|semantic| semantic.into_symbol_table_and_scope_tree())
    })
  }
}
