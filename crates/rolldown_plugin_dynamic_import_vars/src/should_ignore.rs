// Ported from https://github.com/rollup/plugins/blob/944e7d3ec4375371a2e70a55ac07cab4c61dc8b6/packages/dynamic-import-vars/src/dynamic-import-to-glob.js#L67-L80

const IGNORED_PROTOCOLS: &[&str] = &["data:", "http:", "https:"];

pub fn should_ignore(glob: &str) -> bool {
  if memchr::memchr(b'*', glob.as_bytes()).is_none() {
    return true;
  }

  IGNORED_PROTOCOLS.iter().any(|protocol| glob.starts_with(protocol))
}
