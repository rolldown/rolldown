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
pub use helpers::semantic_builder_for_transform;

/// - To access `&mut ast::Program`, use `ast.program.with_mut(|fields| { fields.program; })`.
pub struct EcmaAst {
  pub program: ProgramCell,
  pub source_type: SourceType,
}

impl EcmaAst {
  /// Creates a new `EcmaAst` from a source string and allocator, using a builder function
  /// to construct the program.
  ///
  /// # Arguments
  /// * `source` - The source code string
  /// * `allocator` - The allocator to use for AST nodes
  /// * `builder` - A closure that takes the allocator and returns a `Program`
  pub fn from_allocator_and_source<F>(source: ArcStr, allocator: Allocator, builder: F) -> Self
  where
    F: for<'a> FnOnce(&'a Allocator) -> Program<'a>,
  {
    let program = ProgramCell::new(ProgramCellOwner { source, allocator }, |owner| {
      let program = builder(&owner.allocator);
      ProgramCellDependent { program }
    });
    EcmaAst { program, source_type: SourceType::default().with_module(true) }
  }

  pub fn source(&self) -> &ArcStr {
    &self.program.borrow_owner().source
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
        allocator: Allocator::with_capacity(self.program.borrow_owner().allocator.used_bytes()),
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

// SAFETY: The `Allocator` (bumpalo `Bump`) is `Send` and the entire arena
// moves with the `EcmaAst`. `Program<'static>` contains `NonNull` pointers
// (via `oxc::allocator::Box`), but they all point into the bundled
// `Allocator`, which moves alongside.
unsafe impl Send for EcmaAst {}

// SAFETY: `oxc::allocator::Allocator` is `!Sync` because allocation mutates
// internal `Cell`s through `&self`. We assert `Sync` here under the
// invariant: **the same `EcmaAst` must not have its bumpalo arena mutated
// while another thread holds a shared reference to that `EcmaAst`**.
//
// This invariant is *not* fully enforced by the type system: `EcmaAst.program`
// is `pub`, and `ProgramCell::borrow_owner` / `with_dependent` expose
// `&ProgramCellOwner`, whose `allocator` field is `pub` — so safe code
// holding `&EcmaAst` can construct an `oxc::ast::AstBuilder` that allocates
// into the arena.
//
// [`ProgramCell::with_mut`] is the preferred access pattern because its
// `&mut self` receiver makes the invariant statically checkable. Code that
// reaches the allocator through `borrow_owner().allocator` must provide its
// own synchronization or otherwise prove that each thread operates on a
// distinct `EcmaAst` (for example, parallel chunk passes that index distinct
// entries in `ast_table`).
unsafe impl Sync for EcmaAst {}
