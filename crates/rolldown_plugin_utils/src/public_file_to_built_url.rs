use rolldown_plugin::PluginContext;
use rolldown_utils::{dashmap::FxDashMap, xxhash::xxhash_with_base};

#[derive(Default)]
pub struct PublicAssetUrlCache(pub FxDashMap<String, String>);

pub struct PublicFileToBuiltUrlEnv<'a> {
  pub ctx: &'a PluginContext,
}

impl<'a> PublicFileToBuiltUrlEnv<'a> {
  pub fn new(ctx: &'a PluginContext) -> Self {
    Self { ctx }
  }

  pub fn public_file_to_built_url(&self, url: &str) -> String {
    let mut hash = xxhash_with_base(url.as_bytes(), 16);
    hash.truncate(8);
    let cache = self.ctx.meta().get::<PublicAssetUrlCache>().expect("PublicAssetUrlCache missing");
    let built_url = rolldown_utils::concat_string!("__VITE_ASSET_PUBLIC__", hash, "__");
    if !cache.0.contains_key(&hash) {
      cache.0.insert(hash, url.to_string());
    }
    built_url
  }
}
