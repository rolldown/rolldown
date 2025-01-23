use arcstr::ArcStr;

#[derive(Debug)]
pub struct CssAssetMeta {
  pub filename: ArcStr,
  pub debug_id: u128,
}
