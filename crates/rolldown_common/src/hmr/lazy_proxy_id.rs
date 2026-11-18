/// A lazy compilation proxy's id is the id of the module it stands for, plus this query. The
/// same holds for stable ids.
pub const LAZY_PROXY_QUERY: &str = "?rolldown-lazy=1";

pub fn is_lazy_proxy(id: &str) -> bool {
  id.ends_with(LAZY_PROXY_QUERY)
}

/// The id of the module a lazy proxy stands for: the id its chunk registers a factory under.
/// `None` when `id` is not a lazy proxy.
pub fn lazy_proxy_target(id: &str) -> Option<&str> {
  id.strip_suffix(LAZY_PROXY_QUERY)
}

/// Writes into `buf` the id of the lazy proxy that stands for `target_id`. The proxy exists
/// only when some module calls `import()` on the target.
pub fn write_lazy_proxy_id(buf: &mut String, target_id: &str) {
  buf.clear();
  buf.push_str(target_id);
  buf.push_str(LAZY_PROXY_QUERY);
}
