use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
};

use serde::{de::IgnoredAny, Deserialize};

#[derive(Debug, Default)]
pub struct PackageJsonPeerDep {}

impl PackageJsonPeerDep {
  // TODO(sapphi-red): cache results
  pub fn get_nearest_package_json_optional_peer_deps(
    &self,
    dir: &str,
  ) -> Option<PackageJsonWithOptionalPeerDependencies> {
    let mut dir = Path::new(dir);
    loop {
      let package_json_path = dir.join("package.json");

      if let Ok(package_json_string) = fs::read_to_string(&package_json_path) {
        if let Ok(package_json) =
          serde_json::from_str::<PackageJsonWithPeerDependenciesRaw>(&package_json_string)
        {
          if let Ok(package_json_optional_peer_deps) = package_json.try_into() {
            return Some(package_json_optional_peer_deps);
          }
        }
      }

      if let Some(parent) = dir.parent() {
        dir = parent;
      } else {
        return None;
      }
    }
  }
}

pub struct PackageJsonWithOptionalPeerDependencies {
  pub name: String,
  pub optional_peer_dependencies: BTreeSet<String>,
}

impl TryFrom<PackageJsonWithPeerDependenciesRaw> for PackageJsonWithOptionalPeerDependencies {
  type Error = ();

  fn try_from(value: PackageJsonWithPeerDependenciesRaw) -> Result<Self, Self::Error> {
    let Some(name) = value.name else {
      return Err(());
    };

    let (Some(peer_dependencies), Some(peer_dependencies_meta)) =
      (value.peer_dependencies, value.peer_dependencies_meta)
    else {
      return Ok(Self { name, optional_peer_dependencies: BTreeSet::default() });
    };

    Ok(Self {
      name,
      optional_peer_dependencies: peer_dependencies
        .into_keys()
        .filter(|dep| peer_dependencies_meta.get(dep).map_or(false, |meta| meta.optional))
        .collect(),
    })
  }
}

#[derive(Deserialize)]
struct PackageJsonWithPeerDependenciesRaw {
  pub name: Option<String>,
  #[serde(rename = "peerDependencies")]
  pub peer_dependencies: Option<BTreeMap<String, IgnoredAny>>,
  #[serde(rename = "peerDependenciesMeta")]
  pub peer_dependencies_meta: Option<BTreeMap<String, PackageJsonPeerDependenciesMetaRaw>>,
}

#[derive(Deserialize)]
struct PackageJsonPeerDependenciesMetaRaw {
  pub optional: bool,
}
