use std::sync::Arc;

use oxc::{allocator::Allocator, ast::ast::Program};
use self_cell::self_cell;

pub struct ProgramCellOwner {
  pub source: Arc<str>,
  pub allocator: Allocator,
}

self_cell!(
  /// `ProgramCell` is a wrapper of `Program` that provides a safe way to treat `Program<'ast>` as as owned value without considering the lifetime of `'ast`.
  pub struct ProgramCell {
    owner: ProgramCellOwner,

    #[covariant]
    dependent: Program,
  }
);

impl ProgramCell {
  /// Safely visit `&mut Program` and other fields in the cell within a closure.
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
    func: impl for<'inner> ::core::ops::FnOnce(WithMutFields<'outer, 'inner>) -> Ret,
  ) -> Ret {
    self.with_dependent_mut::<'outer, Ret>(
      |owner: &ProgramCellOwner, program: &'outer mut Program| {
        func(WithMutFields { source: &owner.source, allocator: &owner.allocator, program })
      },
    )
  }
}

pub struct WithMutFields<'outer, 'inner> {
  pub source: &'inner Arc<str>,
  pub allocator: &'inner Allocator,
  pub program: &'outer mut Program<'inner>,
}
