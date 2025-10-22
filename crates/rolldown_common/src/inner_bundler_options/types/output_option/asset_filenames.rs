use std::{future::Future, pin::Pin, sync::Arc};

use derive_more::Debug;
use rolldown_error::SingleBuildResult;

use crate::RollupPreRenderedAsset;

type AssetFilenamesFunction = dyn Fn(
    &RollupPreRenderedAsset,
  ) -> Pin<Box<dyn Future<Output = SingleBuildResult<String>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum AssetFilenamesOutputOption {
  #[debug("AssetFilenamesOutputOption::String({_0:?})")]
  String(String),
  #[debug("AssetFilenamesOutputOption::Fn(...)")]
  Fn(Arc<AssetFilenamesFunction>),
}

impl AssetFilenamesOutputOption {
  pub async fn call(&self, asset: &RollupPreRenderedAsset) -> SingleBuildResult<String> {
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
