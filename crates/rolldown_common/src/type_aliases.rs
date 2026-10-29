use std::sync::Arc;

use arcstr::ArcStr;
use oxc::semantic::NodeId;
use oxc_index::IndexVec;
use rolldown_utils::dashmap::FxDashMap;
use rustc_hash::FxHashMap;

use crate::{
  Chunk, ChunkIdx, ModuleInfo, types::member_expr_ref_resolution::MemberExprRefResolution,
};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;

pub type MemberExprRefResolutionMap = FxHashMap<NodeId, MemberExprRefResolution>;

pub type SharedModuleInfoDashMap = Arc<FxDashMap<ArcStr, Arc<ModuleInfo>>>;
