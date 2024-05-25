use std::{fmt::Debug, sync::Arc};

use crate::OxcCompiler;
use oxc::ast::Trivias;
use oxc::{allocator::Allocator, ast::ast::Program, span::SourceType};

use self::program_cell::{ProgramCell, ProgramCellOwner};

mod helpers;
pub mod program_cell;

pub struct OxcAst {
  pub(crate) inner: ProgramCell,
  pub trivias: Trivias,
  pub source_type: SourceType,
}

impl OxcAst {
  pub fn source(&self) -> &Arc<str> {
    &self.inner.borrow_owner().source
  }

  pub fn allocator(&self) -> &Allocator {
    &self.inner.borrow_owner().allocator
  }

  pub fn program(&self) -> &Program {
    self.inner.borrow_dependent()
  }

  /// Visit all fields including `&mut Program` within a closure.
  ///
  /// ## Example
  ///
  /// ```ignore
  /// let mut ast = OxcCompiler::parse("", SourceType::default());
  /// ast.with_mut(|fields| {
  ///   fields.source; // &Arc<str>
  ///   fields.allocator; // &Allocator
  ///   fields.program; // &mut Program
  /// });
  /// ```
  pub fn with_mut<'outer, Ret>(
    &'outer mut self,
    func: impl for<'inner> ::core::ops::FnOnce(WithFieldsMut<'outer, 'inner>) -> Ret,
  ) -> Ret {
    self.inner.with_dependent_mut::<'outer, Ret>(
      |owner: &ProgramCellOwner, program: &'outer mut Program| {
        func(WithFieldsMut {
          source: &owner.source,
          allocator: &owner.allocator,
          program,
          trivias: &mut self.trivias,
        })
      },
    )
  }
}

pub struct WithFieldsMut<'outer, 'inner> {
  pub source: &'inner Arc<str>,
  pub allocator: &'inner Allocator,
  pub program: &'outer mut Program<'inner>,
  pub trivias: &'outer mut Trivias,
}

impl Debug for OxcAst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.source()).finish_non_exhaustive()
  }
}

impl Default for OxcAst {
  fn default() -> Self {
    OxcCompiler::parse("", SourceType::default())
  }
}

unsafe impl Send for OxcAst {}
unsafe impl Sync for OxcAst {}
