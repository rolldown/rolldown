pub(super) mod determine_side_effects;
mod dynamic_entries;
pub mod include_statements;
mod on_demand;
mod passes;

pub use include_statements::{
  IncludeContext, ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  SymbolIncludeReason, include_symbol,
};
pub use on_demand::compute_body_demand_keys;
pub use passes::include_runtime_symbol;
