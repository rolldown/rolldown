use std::sync::Arc;

use arcstr::ArcStr;
use oxc::span::Span;
use oxc_index::IndexVec;
use rolldown_utils::dashmap::FxDashMap;
use rustc_hash::FxHashMap;

use crate::{
  Chunk, ChunkIdx, ModuleInfo, types::member_expr_ref_resolution::MemberExprRefResolution,
};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;

pub type MemberExprRefResolutionMap = FxHashMap<Span, MemberExprRefResolution>;

// Shared concurrent map for module info storage across multiple threads
pub type SharedModuleInfoDashMap = Arc<FxDashMap<ArcStr, Arc<ModuleInfo>>>;
