#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct StrictExecutionOrderPlanReady {
  #[ts(type = "'StrictExecutionOrderPlanReady'")]
  pub action: &'static str,
  #[ts(type = "1")]
  pub version: u32,
  pub roots: Vec<StrictExecutionOrderRoot>,
  pub plan_modules: Vec<StrictExecutionOrderPlanModule>,
  pub included_modules: Vec<StrictExecutionOrderModule>,
  pub rendered_chunks: Vec<StrictExecutionOrderChunk>,
  pub init_obligations: Vec<StrictExecutionOrderInitObligation>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct StrictExecutionOrderRoot {
  pub root_module_id: String,
  pub expected_order: Vec<String>,
  pub predicted_pre_wrap_order: Vec<String>,
  pub at_risk_modules: Vec<String>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct StrictExecutionOrderPlanModule {
  pub module_id: String,
  #[ts(
    type = "Array<'direct-violation' | 'sensitive-suffix' | 'static-importer' | 'top-level-reader'>"
  )]
  pub reasons: Vec<String>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct StrictExecutionOrderModule {
  pub module_id: String,
  #[ts(type = "'none' | 'cjs' | 'esm'")]
  pub original_wrap_kind: &'static str,
  #[ts(type = "'none' | 'cjs' | 'esm'")]
  pub final_wrap_kind: &'static str,
  pub final_chunk_id: Option<u32>,
  pub entry_chunk_id: Option<u32>,
  pub wrapper_included: bool,
  pub tla_tainted: bool,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct StrictExecutionOrderChunk {
  pub chunk_id: u32,
  pub module_ids: Vec<String>,
  pub static_chunk_imports: Vec<u32>,
  pub dynamic_chunk_imports: Vec<u32>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct StrictExecutionOrderInitObligation {
  #[ts(type = "'direct-import' | 'transitive-init-target'")]
  pub kind: &'static str,
  pub importer_id: String,
  pub importee_id: String,
  pub awaited: bool,
  pub importer_tla_tainted: bool,
  pub importee_tla_tainted: bool,
}
