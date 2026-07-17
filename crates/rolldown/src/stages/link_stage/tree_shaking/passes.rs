use rolldown_common::{RuntimeHelper, RuntimeModuleBrief};

use super::include_statements::{IncludeContext, include_runtime_symbol_with_core};

pub fn include_runtime_symbol(
  context: &mut IncludeContext,
  runtime: &RuntimeModuleBrief,
  depended_runtime_helper: RuntimeHelper,
) {
  include_runtime_symbol_with_core(context, runtime, depended_runtime_helper);
}
