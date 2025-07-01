#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct AssetsReady {
  #[ts(type = "'AssetsReady'")]
  pub action: &'static str,
  pub assets: Vec<Asset>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct Asset {
  /// The id of the chunk that the asset is created from. Empty means the asset is not created from a chunk.
  pub chunk_id: Option<u32>,

  pub content: Option<String>,

  /// The size of the asset in bytes.
  pub size: u32,

  pub filename: String,
}
