use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

pub struct WorkspaceInfo {
    pub metadata: Metadata,
    pub packages: Vec<Package>,
    pub internal_packages: FxHashMap<String, Package>,
    pub dependency_graph: FxHashMap<String, Vec<String>>,
}

impl WorkspaceInfo {
    pub fn new(workspace_path: &Path) -> Result<Self> {
        let metadata = MetadataCommand::new()
            .manifest_path(workspace_path.join("Cargo.toml"))
            .exec()
            .context("Failed to read workspace metadata")?;

        let packages: Vec<Package> = metadata.workspace_packages().into_iter().cloned().collect();

        let mut internal_packages = FxHashMap::default();
        for pkg in &packages {
            internal_packages.insert(pkg.name.clone(), pkg.clone());
        }

        let dependency_graph = build_dependency_graph(&packages, &internal_packages);

        Ok(Self {
            metadata,
            packages,
            internal_packages,
            dependency_graph,
        })
    }

    pub fn get_package(&self, name: &str) -> Option<&Package> {
        self.internal_packages.get(name)
    }

    pub fn get_package_path(&self, name: &str) -> Option<PathBuf> {
        self.get_package(name)
            .map(|p| PathBuf::from(p.manifest_path.parent().unwrap().as_std_path()))
    }

    pub fn get_entry_points(&self) -> Vec<String> {
        let mut entry_points = Vec::new();

        for pkg in &self.packages {
            for target in &pkg.targets {
                if target.is_bin() || target.is_example() {
                    entry_points.push(pkg.name.clone());
                    break;
                }
            }
        }

        if entry_points.is_empty() && !self.packages.is_empty() {
            if let Some(pkg) = self.packages.iter().find(|p| p.name == "rolldown") {
                entry_points.push(pkg.name.clone());
            } else {
                entry_points.push(self.packages[0].name.clone());
            }
        }

        entry_points
    }

    pub fn get_dependencies(&self, package_name: &str) -> Vec<String> {
        self.dependency_graph
            .get(package_name)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_reverse_dependencies(&self, package_name: &str) -> Vec<String> {
        let mut reverse_deps = Vec::new();

        for (pkg, deps) in &self.dependency_graph {
            if deps.contains(&package_name.to_string()) {
                reverse_deps.push(pkg.clone());
            }
        }

        reverse_deps
    }
}

fn build_dependency_graph(
    packages: &[Package],
    internal_packages: &FxHashMap<String, Package>,
) -> FxHashMap<String, Vec<String>> {
    let mut graph = FxHashMap::default();

    for pkg in packages {
        let mut deps = Vec::new();

        for dep in &pkg.dependencies {
            if internal_packages.contains_key(&dep.name) {
                deps.push(dep.name.clone());
            }
        }

        graph.insert(pkg.name.clone(), deps);
    }

    graph
}