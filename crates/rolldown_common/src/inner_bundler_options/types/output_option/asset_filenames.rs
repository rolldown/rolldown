use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

use crate::RollupPreRenderedAsset;

type AssetFilenamesFunction = dyn Fn(
    &RollupPreRenderedAsset,
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<String>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone)]
pub enum AssetFilenamesOutputOption {
  String(String),
  Fn(Arc<AssetFilenamesFunction>),
}

impl Debug for AssetFilenamesOutputOption {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::String(value) => write!(f, "AssetFilenamesOutputOption::String({value:?})"),
      Self::Fn(_) => write!(f, "AssetFilenamesOutputOption::Fn(...)"),
    }
  }
}

impl AssetFilenamesOutputOption {
  pub async fn call(&self, asset: &RollupPreRenderedAsset) -> anyhow::Result<String> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => value(asset).await,
    }
  }

  pub fn value(&self, fn_asset_filename: Option<String>) -> String {
    match self {
      Self::String(value) => value.clone(),
      Self::Fn(_) => fn_asset_filename.expect("AssetFilenamesOutputOption   Fn should has value"),
    }
  }
}

impl From<String> for AssetFilenamesOutputOption {
  fn from(value: String) -> Self {
    Self::String(value)
  }
}
