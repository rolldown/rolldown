use std::sync::Arc;

use oxc::{allocator::Allocator, ast::ast::Program};
use self_cell::self_cell;

pub struct ProgramCellOwner {
  pub source: Arc<str>,
  pub allocator: Allocator,
}

self_cell!(
  /// `ProgramCell` is a wrapper of `Program` that provides a safe way to treat `Program<'ast>` as as owned value without considering the lifetime of `'ast`.
  pub(crate) struct ProgramCell {
    owner: ProgramCellOwner,

    #[covariant]
    dependent: Program,
  }
);
