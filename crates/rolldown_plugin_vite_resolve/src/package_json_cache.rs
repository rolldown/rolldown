use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::PathBuf,
  sync::Arc,
};

use dashmap::DashMap;
use rolldown_common::PackageJson;
use rustc_hash::FxBuildHasher;
use serde::{de::IgnoredAny, Deserialize};

#[derive(Debug, Default)]
pub struct PackageJsonCache {
  side_effects_cache: DashMap<PathBuf, Arc<PackageJson>, FxBuildHasher>,
  optional_peer_dep_cache:
    DashMap<PathBuf, Arc<PackageJsonWithOptionalPeerDependencies>, FxBuildHasher>,
}

impl PackageJsonCache {
  pub fn cached_package_json_side_effects(
    &self,
    oxc_pkg_json: &oxc_resolver::PackageJson,
  ) -> Arc<PackageJson> {
    if let Some(v) = self.side_effects_cache.get(&oxc_pkg_json.realpath) {
      Arc::clone(v.value())
    } else {
      let pkg_json = Arc::new(
        PackageJson::new(oxc_pkg_json.path.clone())
          .with_side_effects(oxc_pkg_json.side_effects.as_ref()),
      );
      self.side_effects_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
      pkg_json
    }
  }

  pub fn cached_package_json_optional_peer_dep(
    &self,
    oxc_pkg_json: &oxc_resolver::PackageJson,
  ) -> Arc<PackageJsonWithOptionalPeerDependencies> {
    if let Some(v) = self.optional_peer_dep_cache.get(&oxc_pkg_json.realpath) {
      Arc::clone(v.value())
    } else {
      let package_json_with_optional_peer_deps = {
        let Ok(package_json_string) = fs::read_to_string(&oxc_pkg_json.realpath) else {
          return Default::default();
        };
        let Ok(package_json) =
          serde_json::from_str::<PackageJsonWithPeerDependenciesRaw>(&package_json_string)
        else {
          return Default::default();
        };
        package_json.try_into().unwrap_or_default()
      };

      let pkg_json = Arc::new(package_json_with_optional_peer_deps);
      self.optional_peer_dep_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
      pkg_json
    }
  }
}

#[derive(Debug, Default)]
pub struct PackageJsonWithOptionalPeerDependencies {
  pub name: String,
  pub optional_peer_dependencies: BTreeSet<String>,
}

impl TryFrom<PackageJsonWithPeerDependenciesRaw> for PackageJsonWithOptionalPeerDependencies {
  type Error = ();

  fn try_from(value: PackageJsonWithPeerDependenciesRaw) -> Result<Self, Self::Error> {
    let (Some(peer_dependencies), Some(peer_dependencies_meta)) =
      (value.peer_dependencies, value.peer_dependencies_meta)
    else {
      return Ok(Self { name: value.name, optional_peer_dependencies: BTreeSet::default() });
    };

    Ok(Self {
      name: value.name,
      optional_peer_dependencies: peer_dependencies
        .into_keys()
        .filter(|dep| peer_dependencies_meta.get(dep).map_or(false, |meta| meta.optional))
        .collect(),
    })
  }
}

#[derive(Deserialize)]
struct PackageJsonWithPeerDependenciesRaw {
  pub name: String,
  #[serde(rename = "peerDependencies")]
  pub peer_dependencies: Option<BTreeMap<String, IgnoredAny>>,
  #[serde(rename = "peerDependenciesMeta")]
  pub peer_dependencies_meta: Option<BTreeMap<String, PackageJsonPeerDependenciesMetaRaw>>,
}

#[derive(Deserialize)]
struct PackageJsonPeerDependenciesMetaRaw {
  pub optional: bool,
}
