use std::{fmt::Debug, pin::Pin};

use oxc::{
  allocator::Allocator,
  ast::ast,
  parser::Parser,
  semantic::{Semantic, SemanticBuilder},
  span::SourceType,
};
pub struct OxcCompiler;

pub struct OxcProgram {
  program: ast::Program<'static>,
  source: Pin<Box<String>>,
  // Order matters here, we need drop the program first, then drop the allocator. Otherwise, there will be a segmentation fault.
  allocator: Pin<Box<Allocator>>,
}

impl Debug for OxcProgram {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast")
      .field("source", &self.source)
      //   .field("program", &self.program)
      .finish()
  }
}

unsafe impl Send for OxcProgram {}
unsafe impl Sync for OxcProgram {}

impl OxcProgram {
  pub fn from_source(source: String, ty: SourceType) -> Self {
    let source = Box::pin(source);
    let allocator = Box::pin(oxc::allocator::Allocator::default());
    let program = unsafe {
      let source = std::mem::transmute::<_, &'static String>(source.as_ref());
      let alloc = std::mem::transmute::<_, &'static Allocator>(allocator.as_ref());
      Parser::new(alloc, source, ty).parse().program
    };

    Self {
      program,
      source,
      allocator,
    }
  }

  pub fn program<'me>(&'me self) -> &'me ast::Program<'me> {
    unsafe { std::mem::transmute(&self.program) }
  }

  pub fn program_mut<'me>(&'me mut self) -> &'me mut ast::Program<'me> {
    unsafe { std::mem::transmute(&mut self.program) }
  }

  pub fn program_mut_and_allocator<'me>(
    &'me mut self,
  ) -> (&'me mut ast::Program<'me>, &'me Allocator) {
    (
      unsafe { std::mem::transmute(&mut self.program) },
      &self.allocator,
    )
  }

  pub fn make_semantic<'me>(&'me self, ty: SourceType) -> Semantic<'me> {
    let semantic = SemanticBuilder::new(&self.source, ty)
      .build(self.program())
      .semantic;
    unsafe { std::mem::transmute(semantic) }
  }
}

impl OxcCompiler {
  pub fn parse(source: String, ty: SourceType) -> OxcProgram {
    let source = Box::pin(source);
    let allocator = Box::pin(oxc::allocator::Allocator::default());
    let program = unsafe {
      let source = std::mem::transmute::<_, &'static String>(source.as_ref());
      let alloc = std::mem::transmute::<_, &'static Allocator>(allocator.as_ref());
      Parser::new(alloc, source, ty).parse().program
    };

    OxcProgram {
      program,
      source,
      allocator,
    }
  }

  pub fn print() {}
}
