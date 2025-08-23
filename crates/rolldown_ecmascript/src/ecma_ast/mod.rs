use std::fmt::Debug;

use crate::EcmaCompiler;
use arcstr::ArcStr;
use oxc::{
  allocator::{Allocator, CloneIn},
  ast::ast::{Comment, Program},
  span::SourceType,
};
use program_cell::{ProgramCellDependent, ProgramCellOwner};

use self::program_cell::ProgramCell;

mod r#gen;
mod helpers;
pub mod program_cell;
pub use r#gen::ToSourceString;

/// - To access `&mut ast::Program`, use `ast.program.with_mut(|fields| { fields.program; })`.
pub struct EcmaAst {
  pub program: ProgramCell,
  pub source_type: SourceType,
}

impl EcmaAst {
  pub fn source(&self) -> &ArcStr {
    &self.program.borrow_owner().source
  }

  pub fn allocator(&self) -> &Allocator {
    &self.program.borrow_owner().allocator
  }

  pub fn program(&self) -> &Program<'_> {
    &self.program.borrow_dependent().program
  }

  pub fn comments(&self) -> &oxc::allocator::Vec<'_, Comment> {
    &self.program.borrow_dependent().program.comments
  }

  /// Clone the `Program` with another `Allocator`.
  /// and copy rest fields, this is used to cache the Ast in incremental compilation
  /// use another `Allocator` to avoid memory leak because.
  #[must_use]
  pub fn clone_with_another_arena(&self) -> EcmaAst {
    let program = ProgramCell::new(
      ProgramCellOwner {
        source: self.source().clone(),
        allocator: Allocator::with_capacity(self.allocator().used_bytes()),
      },
      |owner| {
        let program = self.program().clone_in_with_semantic_ids(&owner.allocator);
        ProgramCellDependent { program }
      },
    );
    EcmaAst { program, source_type: self.source_type }
  }
}

impl Debug for EcmaAst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.source()).finish_non_exhaustive()
  }
}

impl Default for EcmaAst {
  fn default() -> Self {
    EcmaCompiler::parse("", "", SourceType::default()).unwrap()
  }
}

unsafe impl Send for EcmaAst {}
unsafe impl Sync for EcmaAst {}
