use std::sync::Arc;

use oxc::{
  allocator::Allocator,
  codegen::{CodeGenerator, Codegen, CodegenReturn},
  minifier::{Minifier, MinifierOptions},
  parser::Parser,
  sourcemap::SourceMap,
  span::SourceType,
};

use crate::ecma_ast::{
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
  EcmaAst,
};
pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(source: impl Into<Arc<str>>, ty: SourceType) -> anyhow::Result<EcmaAst> {
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
          Ok(ProgramCellDependent { program: ret.program })
        }
      })?;
    Ok(EcmaAst {
      program: inner,
      source_type: ty,
      trivias: trivias.expect("Should be initialized"),
      contains_use_strict: false,
    })
  }
  pub fn print(ast: &EcmaAst, source_name: &str, enable_source_map: bool) -> CodegenReturn {
    let mut codegen = CodeGenerator::new().with_capacity(ast.source().len()).enable_comment(
      ast.source(),
      ast.trivias.clone(),
      oxc::codegen::CommentOptions { preserve_annotate_comments: true },
    );
    if enable_source_map {
      codegen = codegen.enable_source_map(source_name, ast.source());
    }
    codegen.build(ast.program())
  }

  pub fn minify(
    source_text: &str,
    enable_sourcemap: bool,
    filename: &str,
  ) -> anyhow::Result<(String, Option<SourceMap>)> {
    let allocator = Allocator::default();
    let program = Parser::new(&allocator, source_text, SourceType::default()).parse().program;
    let program = allocator.alloc(program);
    let options = MinifierOptions { mangle: true, ..MinifierOptions::default() };
    Minifier::new(options).build(&allocator, program);
    let codegen = Codegen::<true>::new();
    let codegen =
      if enable_sourcemap { codegen.enable_source_map(filename, source_text) } else { codegen };

    let ret = codegen.build(program);
    Ok((ret.source_text, ret.source_map))
  }
}

#[test]
fn basic_test() {
  let ast = EcmaCompiler::parse("const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = EcmaCompiler::print(&ast, "", false).source_text;
  assert_eq!(code, "const a = 1;\n");
}
