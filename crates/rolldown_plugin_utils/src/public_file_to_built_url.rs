use rolldown_utils::{dashmap::FxDashMap, xxhash::xxhash_with_base};

#[derive(Default)]
pub struct PublicAssetUrlCache(pub FxDashMap<String, String>);

pub struct PublicFileToBuiltUrlEnv<'a> {
  pub ctx: &'a rolldown_plugin::PluginContext,
}

impl PublicFileToBuiltUrlEnv<'_> {
  pub fn public_file_to_built_url(&self, url: &str) -> String {
    let hash = xxhash_with_base(url.as_bytes(), 16);
    let cache = self.ctx.meta().get::<PublicAssetUrlCache>().expect("PublicAssetUrlCache missing");
    let built_url = rolldown_utils::concat_string!("__VITE_PUBLIC_ASSET__", hash, "__");
    if !cache.0.contains_key(&hash) {
      cache.0.insert(hash, url.to_string());
    }
    built_url
  }
}
