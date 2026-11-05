#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct OutputChunkGraphReady {
  #[ts(type = "'OutputChunkGraphReady'")]
  pub action: &'static str,
  pub chunks: Vec<OutputChunk>,
}

/// Chunk metadata from the output after `generateBundle` hooks finish.
#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct OutputChunk {
  pub file_name: String,
  pub name: String,
  #[ts(type = "number")]
  pub bytes: u64,
  pub is_entry: bool,
  pub facade_module_id: Option<String>,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
}
