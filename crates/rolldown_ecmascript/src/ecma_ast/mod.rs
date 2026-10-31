use std::{cell::RefCell, fmt::Debug};

use crate::EcmaCompiler;
use arcstr::ArcStr;
use oxc::{
  allocator::{Allocator, CloneIn},
  ast::{
    ast::{Comment, Program},
    builder::AstBuilder,
  },
  span::SourceType,
};
use program_cell::{ProgramCellDependent, ProgramCellOwner};

/// The largest scratch arena a thread keeps between [`EcmaAst::compact_arena`] calls. A larger
/// one, grown by an unusually big module, is dropped instead of being kept around.
const SCRATCH_ARENA_CAPACITY_LIMIT: usize = 256 * 1024;

thread_local! {
  /// Reused by [`EcmaAst::compact_arena`] to measure how many bytes a module's AST really
  /// needs before allocating its right-sized arena.
  static SCRATCH_ARENA: RefCell<Allocator> = RefCell::new(Allocator::default());
}

/// Clones `program` into `allocator`, keeping every `NodeId` / `ScopeId` (see
/// `internal-docs/ast-mutation/implementation.md`), and points the copy's `source_text` at
/// `source_text` instead of copying the text into the arena.
///
/// `Program`'s derived `CloneIn` copies `source_text` with `alloc_str`, which would duplicate
/// the whole module source in every cloned arena; the parse itself only borrows it from the
/// module's `ArcStr`. Every `EcmaAst` keeps that `ArcStr` alive, so the copy can borrow it too.
fn clone_program_in<'new>(
  program: &Program<'_>,
  source_text: &'new str,
  allocator: &'new Allocator,
) -> Program<'new> {
  let builder = AstBuilder::new(allocator);
  let cloned = Program::new(
    program.span,
    program.source_type,
    source_text,
    program.comments.clone_in_with_semantic_ids(allocator),
    program.hashbang.clone_in_with_semantic_ids(allocator),
    program.directives.clone_in_with_semantic_ids(allocator),
    program.body.clone_in_with_semantic_ids(allocator),
    &builder,
  );
  cloned.node_id.set(program.node_id.get());
  cloned.scope_id.set(program.scope_id.get());
  cloned
}

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
  ///
  /// The new arena is sized by what the old one has handed out, which also counts nodes that
  /// pre-processing discarded; [`Self::compact_arena`] is the way to get an exact-sized arena.
  #[must_use]
  pub fn clone_with_another_arena(&self) -> EcmaAst {
    let allocator = Allocator::with_capacity(self.program.borrow_owner().allocator.used_bytes());
    self.clone_into_arena(allocator)
  }

  fn clone_into_arena(&self, allocator: Allocator) -> EcmaAst {
    let program =
      ProgramCell::new(ProgramCellOwner { source: self.source().clone(), allocator }, |owner| {
        let program = clone_program_in(self.program(), owner.source.as_str(), &owner.allocator);
        ProgramCellDependent { program }
      });
    EcmaAst { program, source_type: self.source_type }
  }

  /// Replaces the arena with one sized to exactly what the AST holds, when that is smaller.
  ///
  /// The parser's arena starts at 16 KiB and doubles as it fills, and it keeps the nodes that
  /// the transformer, dead-code elimination and semantic rebuilds discarded. A one-shot build
  /// drops the arena after rendering, but an incremental build keeps every module's AST in the
  /// `ScanStageCache` snapshot for later links and HMR patches, so that slack is retained for
  /// the whole session (`internal-docs/cache/implementation.md`, "Arena compaction").
  ///
  /// The copy keeps every `NodeId` / `ScopeId` / `SymbolId`, so the scan-time side tables stay
  /// valid. The AST is measured first in a thread-local scratch arena, because
  /// `Allocator::used_bytes` of the old arena counts the discarded nodes too. A module whose
  /// copy would not be smaller — one of large string literals, which the parser borrows from
  /// the source but a copy writes into the arena — keeps its parse.
  ///
  /// Returns the arena capacity released.
  pub fn compact_arena(&mut self) -> usize {
    let retained = self.program.borrow_owner().allocator.capacity();
    let needed = SCRATCH_ARENA.with_borrow_mut(|scratch| {
      // The scratch copy is a throwaway: only its size matters.
      let _ = clone_program_in(self.program(), "", scratch);
      let needed = scratch.used_bytes();
      if scratch.capacity() > SCRATCH_ARENA_CAPACITY_LIMIT {
        *scratch = Allocator::default();
      } else {
        scratch.reset();
      }
      needed
    });
    if needed >= retained {
      return 0;
    }
    let allocator = Allocator::with_capacity(needed);
    let new_capacity = allocator.capacity();
    if new_capacity >= retained {
      return 0;
    }
    *self = self.clone_into_arena(allocator);
    retained - new_capacity
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
// holding `&EcmaAst` can construct an `oxc::ast::builder::AstBuilder` that
// allocates into the arena.
//
// [`ProgramCell::with_mut`] is the preferred access pattern because its
// `&mut self` receiver makes the invariant statically checkable. Code that
// reaches the allocator through `borrow_owner().allocator` must provide its
// own synchronization or otherwise prove that each thread operates on a
// distinct `EcmaAst` (for example, parallel chunk passes that index distinct
// entries in `ast_table`).
unsafe impl Sync for EcmaAst {}
