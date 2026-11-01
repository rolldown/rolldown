//! Right-sizes what an incremental build retains for every module until the session ends.
//!
//! A scan parses each module into an arena that starts at 16 KiB and doubles as it fills, and
//! leaves in it the nodes that the parser, semantic analysis and transforms discarded; the
//! module's scope tree has an arena of its own. A one-shot build drops both after rendering, but
//! an incremental build keeps them for later links, HMR patches and lazy compilation, so most of
//! what it retains for a graph of small modules is arena capacity that nothing uses. Its module
//! tasks replace both with copies in arenas sized to what the copies hold, unless a copy would
//! not be smaller. A copy keeps every node, scope, symbol and reference id, as the copy that each
//! render finalizes does. The link output that fast relinks reuse keeps its tables likewise
//! without the capacity that linking left in them.

use std::cell::RefCell;
use std::ptr;

use oxc::allocator::{Allocator, CloneIn};
use rayon::prelude::*;
use rolldown_common::{ModuleIdx, SymbolRefDbForModule};
use rolldown_ecmascript::EcmaAst;

use crate::stages::link_stage::LinkStageOutput;

/// The largest scratch arena that a thread keeps between modules.
const SCRATCH_CAPACITY_LIMIT: usize = 256 * 1024;

thread_local! {
  static SCRATCH: RefCell<Allocator> = RefCell::new(Allocator::default());
}

/// Replaces a module's AST and scope tree with copies in right-sized arenas, where they are
/// smaller.
pub fn compact_module_arenas(ast: &mut EcmaAst, symbols: &mut SymbolRefDbForModule) {
  compact_ast(ast);
  // Unlike the parser, the scope tree's builder copied every name into the tree's own arena, so
  // a copy holds nothing that the tree did not, in an arena sized by what the tree's arena used,
  // and never takes more memory than the arena that it replaces.
  let scoping = symbols.ast_scopes.scoping().clone_in_with_semantic_ids_with_another_arena();
  symbols.ast_scopes.set_scoping(scoping);
}

/// Replaces `ast` with a copy in an arena that the copy fills, unless that arena would be no
/// smaller than the AST's own. The parser borrows every name and unescaped literal from the
/// module's source, which a copy holds in its own arena, so a module of large literals keeps its
/// parse. `EcmaAst::clone_with_another_arena` sizes the arena by what the AST's own arena used,
/// discarded nodes included, so a first copy into a scratch arena that the thread reuses
/// measures the AST instead; a scratch arena that a large AST grew is dropped. A copy points its
/// program's source text at the module's source, as the parse does, rather than holding it.
/// Returns the arena capacity released (`0` when the AST was kept as-is).
pub fn compact_ast(ast: &mut EcmaAst) -> usize {
  let own_source = ast.program.with_mut(|fields| {
    let own_source = ptr::eq(fields.program.source_text, fields.source.as_str());
    if own_source {
      fields.program.source_text = "";
    }
    own_source
  });
  let size = SCRATCH.with_borrow_mut(|scratch| {
    let _ = ast.program().clone_in_with_semantic_ids(scratch);
    let size = scratch.used_bytes();
    if scratch.capacity() > SCRATCH_CAPACITY_LIMIT {
      *scratch = Allocator::default();
    } else {
      scratch.reset();
    }
    size
  });
  let retained = ast.program.borrow_owner().allocator.capacity();
  let allocator = (size < retained).then(|| Allocator::with_capacity(size));
  let mut released = 0;
  if let Some(allocator) = allocator.filter(|allocator| allocator.capacity() < retained) {
    let new_capacity = allocator.capacity();
    let mut copy =
      EcmaAst::from_allocator_and_source(ast.source().clone(), allocator, |allocator| {
        ast.program().clone_in_with_semantic_ids(allocator)
      });
    copy.source_type = ast.source_type;
    *ast = copy;
    released = retained.saturating_sub(new_capacity);
  }
  if own_source {
    ast.program.with_mut(|fields| fields.program.source_text = fields.source.as_str());
  }
  released
}

/// Drops the capacity that linking left in each module's statement and symbol tables. The tables
/// indexed by module keep the room that fast relinks append modules into.
pub fn shrink_link_tables(link_output: &mut LinkStageOutput) {
  link_output.stmt_infos.raw.par_iter_mut().for_each(|stmt_infos| {
    for stmt_info in stmt_infos.infos.iter_mut() {
      stmt_info.declared_symbols.shrink_to_fit();
      stmt_info.referenced_symbols.shrink_to_fit();
      stmt_info.import_records.shrink_to_fit();
    }
    stmt_infos.infos.raw.shrink_to_fit();
  });
  for index in 0..link_output.symbol_db.inner().len() {
    if let Some(symbols) = link_output.symbol_db[ModuleIdx::from_usize(index)].as_mut() {
      symbols.classic_data.raw.shrink_to_fit();
    }
  }
}
