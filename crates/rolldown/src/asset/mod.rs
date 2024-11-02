use rolldown_common::AssetView;

pub mod asset_generator;

pub fn create_asset_view(source: Box<[u8]>) -> AssetView {
  AssetView { source }
}
