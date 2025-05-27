use oxc::span::Span;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

use crate::{Chunk, ChunkIdx, types::member_expr_ref_resolution::MemberExprRefResolution};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;

pub type MemberExprRefResolutionMap = FxHashMap<Span, MemberExprRefResolution>;
