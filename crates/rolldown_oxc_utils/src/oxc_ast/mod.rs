use std::{fmt::Debug, sync::Arc};

use crate::OxcCompiler;
use oxc::ast::Trivias;
use oxc::{allocator::Allocator, ast::ast::Program, span::SourceType};

use self::program_cell::ProgramCell;

mod helpers;
pub mod program_cell;

/// - To access `&mut ast::Program`, use `ast.program.with_mut(|fields| { fields.program; })`.

pub struct OxcAst {
  pub program: ProgramCell,
  pub trivias: Trivias,
  pub source_type: SourceType,
  pub contains_use_strict: bool,
}

impl OxcAst {
  pub fn source(&self) -> &Arc<str> {
    &self.program.borrow_owner().source
  }

  pub fn allocator(&self) -> &Allocator {
    &self.program.borrow_owner().allocator
  }

  pub fn program(&self) -> &Program {
    &self.program.borrow_dependent().program
  }
}

impl Debug for OxcAst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.source()).finish_non_exhaustive()
  }
}

impl Default for OxcAst {
  fn default() -> Self {
    OxcCompiler::parse("", SourceType::default()).unwrap()
  }
}

unsafe impl Send for OxcAst {}
unsafe impl Sync for OxcAst {}
