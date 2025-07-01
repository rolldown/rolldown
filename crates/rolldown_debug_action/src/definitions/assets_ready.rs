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
  /// If the asset is created from a chunk, this field will be the chunk id.
  pub originate_from: Option<u32>,

  /// The size of the asset in bytes.
  pub size: u32,

  pub filename: String,
}
