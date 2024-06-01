use std::sync::Arc;

use oxc::{
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  parser::Parser,
  span::SourceType,
};

use crate::oxc_ast::{
  program_cell::{ProgramCell, ProgramCellOwner},
  OxcAst,
};
pub struct OxcCompiler;

impl OxcCompiler {
  pub fn parse(source: impl Into<Arc<str>>, ty: SourceType) -> anyhow::Result<OxcAst> {
    let allocator = oxc::allocator::Allocator::default();
    let mut trivias = None;
    let inner =
      ProgramCell::try_new(ProgramCellOwner { source: source.into(), allocator }, |owner| {
        let parser =
          Parser::new(&owner.allocator, &owner.source, ty).allow_return_outside_function(true);
        let ret = parser.parse();
        if ret.panicked || !ret.errors.is_empty() {
          // TODO: more dx friendly error message
          Err(anyhow::format_err!("Parse failed, got {:#?}", ret.errors))
        } else {
          trivias = Some(ret.trivias);
          Ok(ret.program)
        }
      })?;
    Ok(OxcAst {
      inner,
      source_type: ty,
      trivias: trivias.expect("Should be initialized"),
      contains_use_strict: false,
    })
  }
  pub fn print(ast: &OxcAst, source_name: &str, enable_source_map: bool) -> CodegenReturn {
    let codegen = Codegen::<false>::new(
      source_name,
      ast.source(),
      CodegenOptions { enable_typescript: false, enable_source_map },
    );
    codegen.build(ast.program())
  }
}

#[test]
fn basic_test() {
  let ast = OxcCompiler::parse("const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = OxcCompiler::print(&ast, "", false).source_text;
  assert_eq!(code, "const a = 1;\n");
}
