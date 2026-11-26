use rolldown_utils::pattern_filter::StringOrRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// `experimental.cache`: the persistent scan cache.
#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct CacheOptions {
  /// A relative path is resolved against `cwd`.
  pub dir: String,
  /// Milliseconds. Cache data that was not used for longer is removed. `None` removes nothing.
  pub max_age: Option<f64>,
  pub cache_key: Option<String>,
  pub build_dependencies: Option<Vec<String>>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "super::watch_option::deserialize_string_or_regex"),
    schemars(with = "Option<Vec<String>>")
  )]
  pub assume_unchanged: Option<Vec<StringOrRegex>>,
  pub assume_cacheable_hooks: Option<AssumeCacheableHooks>,
  /// Set by the JavaScript API, not by users: the value that JavaScript plugins read as
  /// `this.meta.watchMode`. A plugin can give another result in watch mode, so it is an input
  /// of key 1.
  pub watch_mode: Option<bool>,
}

/// Treats hooks that do not set `cache` as if they set `cache: true`.
/// Rolldown cannot check this assumption, so it is meant for tests and measurements.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct AssumeCacheableHooks {
  /// Also applies to `resolveDynamicImport`.
  pub resolve_id: Option<bool>,
  pub load: Option<bool>,
  pub transform: Option<bool>,
}
