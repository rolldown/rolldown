use rolldown_common::{AssetView, StrOrBytes};

pub mod asset_generator;

pub fn create_asset_view(source: StrOrBytes) -> AssetView {
  AssetView { source }
}
