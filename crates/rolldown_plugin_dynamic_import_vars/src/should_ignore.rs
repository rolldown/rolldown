// Ported from https://github.com/rollup/plugins/blob/944e7d3ec4375371a2e70a55ac07cab4c61dc8b6/packages/dynamic-import-vars/src/dynamic-import-to-glob.js#L67-L80

use std::sync::LazyLock;

static IGNORED_PROTOCOLS: LazyLock<Vec<&str>> = LazyLock::new(|| vec!["data:", "http:", "https:"]);

pub(crate) fn should_ignore(glob: &str) -> bool {
  if !glob.contains('*') {
    return true;
  }

  return IGNORED_PROTOCOLS.iter().any(|protocol| glob.starts_with(protocol));
}
