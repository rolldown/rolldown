use std::{fmt::Debug, pin::Pin, sync::Arc};

use oxc::{
  allocator::Allocator,
  ast::ast,
  codegen::{Codegen, CodegenOptions},
  parser::Parser,
  semantic::{Semantic, SemanticBuilder},
  span::SourceType,
};

use crate::Dummy;
pub struct OxcCompiler;

#[allow(clippy::box_collection, clippy::non_send_fields_in_send_ty, unused)]
pub struct OxcProgram {
  program: ast::Program<'static>,
  source: Pin<Arc<str>>,
  // Order matters here, we need drop the program first, then drop the allocator. Otherwise, there will be a segmentation fault.
  // The `program` is allocated on the `allocator`. Clippy think it's not used, but it's used.
  allocator: Pin<Box<Allocator>>,
}
impl Debug for OxcProgram {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.source).finish_non_exhaustive()
  }
}

impl Default for OxcProgram {
  fn default() -> Self {
    let source = Pin::new(String::default().into());
    let allocator = Box::pin(oxc::allocator::Allocator::default());

    let program = unsafe {
      let alloc = std::mem::transmute::<_, &'static Allocator>(allocator.as_ref());
      ast::Program::dummy(alloc)
    };
    Self { program, source, allocator }
  }
}

unsafe impl Send for OxcProgram {}
unsafe impl Sync for OxcProgram {}

impl OxcProgram {
  pub fn source(&self) -> &str {
    &self.source
  }

  pub fn program(&self) -> &ast::Program<'_> {
    // SAFETY: `&'a ast::Program<'a>` can't outlive the `&'a ast::Program<'static>`.
    unsafe { std::mem::transmute(&self.program) }
  }

  pub fn program_mut(&mut self) -> &mut ast::Program<'_> {
    // SAFETY: `&'a mut ast::Program<'a>` can't outlive the `&'a mut ast::Program<'static>`.
    unsafe { std::mem::transmute(&mut self.program) }
  }

  pub fn program_mut_and_allocator(&mut self) -> (&mut ast::Program<'_>, &Allocator) {
    // SAFETY: `&'a mut ast::Program<'a>` can't outlive the `&'a mut ast::Program<'static>`.
    let program = unsafe { std::mem::transmute(&mut self.program) };
    (program, &self.allocator)
  }

  pub fn make_semantic(&self, ty: SourceType) -> Semantic<'_> {
    let semantic = SemanticBuilder::new(&self.source, ty).build(self.program()).semantic;
    semantic
  }
}

impl OxcCompiler {
  pub fn parse(source: impl Into<Arc<str>>, ty: SourceType) -> OxcProgram {
    let source = Pin::new(source.into());
    let allocator = Box::pin(oxc::allocator::Allocator::default());
    let program = unsafe {
      let source = std::mem::transmute::<_, &'static str>(&*source);
      let alloc = std::mem::transmute::<_, &'static Allocator>(allocator.as_ref());
      Parser::new(alloc, source, ty).parse().program
    };

    OxcProgram { program, source, allocator }
  }

  pub fn print(ast: &OxcProgram) -> String {
    let codegen = Codegen::<false>::new(ast.source().len(), CodegenOptions);
    codegen.build(&ast.program)
  }
}

#[test]
fn basic_test() {
  let ast = OxcCompiler::parse("const a = 1;".to_string(), SourceType::default());
  let code = OxcCompiler::print(&ast);
  assert_eq!(code, "const a = 1;\n");
}
