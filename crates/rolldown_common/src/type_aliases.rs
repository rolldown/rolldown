use oxc::span::{CompactStr, Span};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

use crate::{Chunk, ChunkIdx, SymbolRef};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;

pub type MemberExprRefResolutionMap =
  FxHashMap<Span, (Option<SymbolRef>, Vec<CompactStr>, Vec<SymbolRef>)>;
