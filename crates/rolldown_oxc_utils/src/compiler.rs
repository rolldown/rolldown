use std::sync::Arc;

use oxc::{
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  parser::Parser,
  span::SourceType,
};

use crate::{oxc_ast::NewStructName, OxcAst};
pub struct OxcCompiler;

impl OxcCompiler {
  pub fn parse(source: impl Into<Arc<str>>, ty: SourceType) -> OxcAst {
    let allocator = oxc::allocator::Allocator::default();
    let inner = NewStructName::new((source.into(), allocator), |(source, allocator)| {
      let parser = Parser::new(allocator, source, ty);
      parser.parse().program
    });
    OxcAst { inner }
  }
  pub fn print(ast: &OxcAst, source_name: &str, enable_source_map: bool) -> CodegenReturn {
    ast.with_dependent(|dep, program| {
      let codegen = Codegen::<false>::new(
        source_name,
        &dep.0,
        CodegenOptions { enable_typescript: false, enable_source_map },
      );
      codegen.build(program)
    })
  }
}

#[test]
fn basic_test() {
  let ast = OxcCompiler::parse("const a = 1;".to_string(), SourceType::default());
  let code = OxcCompiler::print(&ast, "", false).source_text;
  assert_eq!(code, "const a = 1;\n");
}
