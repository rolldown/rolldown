pub(super) mod determine_side_effects;
mod dynamic_entries;
pub mod include_statements;
mod on_demand;
mod passes;

pub use include_statements::{
  IncludeContext, ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  SymbolIncludeReason, include_symbol,
};
pub use on_demand::compute_on_demand_side_effect_stmts;
pub use passes::include_runtime_symbol;
