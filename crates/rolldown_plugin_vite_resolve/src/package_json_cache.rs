use std::{path::PathBuf, sync::Arc};

use dashmap::DashMap;
use rolldown_common::PackageJson;

#[derive(Debug, Default)]
pub struct PackageJsonCache {
  package_json_cache: DashMap<PathBuf, Arc<PackageJson>>,
}

impl PackageJsonCache {
  pub fn cached_package_json(&self, oxc_pkg_json: &oxc_resolver::PackageJson) -> Arc<PackageJson> {
    if let Some(v) = self.package_json_cache.get(&oxc_pkg_json.realpath) {
      Arc::clone(v.value())
    } else {
      let pkg_json = Arc::new(
        PackageJson::new(oxc_pkg_json.path.clone())
          .with_side_effects(oxc_pkg_json.side_effects.as_ref()),
      );
      self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
      pkg_json
    }
  }
}
