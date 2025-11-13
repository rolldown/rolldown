use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::Context;
use arcstr::ArcStr;
use dashmap::DashMap;
use oxc_resolver::{
  ModuleType, PackageJson as OxcPackageJson, Resolution, ResolveError, ResolverGeneric, TsConfig,
};
use rolldown_common::{
  ImportKind, ModuleDefFormat, PackageJson, Platform, ResolveOptions, ResolvedId,
};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_utils::dashmap::FxDashMap;
use sugar_path::SugarPath as _;

use crate::resolver_config::ResolverConfig;

/// A wrapper around oxc_resolver that provides a rolldown-specific API.
///
/// The resolver handles module resolution for different import types (ESM, CommonJS, CSS, etc.)
/// and manages caching of package.json files.
#[derive(Debug)]
#[expect(clippy::struct_field_names)]
pub struct Resolver<Fs: FileSystem = OsFileSystem> {
  fs: Fs,
  cwd: PathBuf,
  /// Resolver with default conditions
  default_resolver: ResolverGeneric<Fs>,
  /// Resolver for `import '...'` and `import(...)`
  import_resolver: ResolverGeneric<Fs>,
  /// Resolver for `require('...')`
  require_resolver: ResolverGeneric<Fs>,
  /// Resolver for `@import '...'` and `url('...')`
  css_resolver: ResolverGeneric<Fs>,
  /// Resolver for `new URL(..., import.meta.url)`
  new_url_resolver: ResolverGeneric<Fs>,
  /// Cache for parsed package.json files
  package_json_cache: FxDashMap<PathBuf, Arc<PackageJson>>,
}

impl<Fs: FileSystem + Clone> Resolver<Fs> {
  /// Creates a new resolver with the specified options.
  pub fn new(
    fs: Fs,
    cwd: PathBuf,
    platform: Platform,
    tsconfig: Option<PathBuf>,
    resolve_options: ResolveOptions,
  ) -> Self {
    let config = ResolverConfig::build(&cwd, platform, tsconfig, resolve_options);

    let default_resolver =
      ResolverGeneric::new_with_file_system(fs.clone(), config.default_options);
    let import_resolver = default_resolver.clone_with_options(config.import_options);
    let require_resolver = default_resolver.clone_with_options(config.require_options);
    let css_resolver = default_resolver.clone_with_options(config.css_options);
    let new_url_resolver = default_resolver.clone_with_options(config.new_url_options);

    Self {
      fs,
      cwd,
      default_resolver,
      import_resolver,
      require_resolver,
      css_resolver,
      new_url_resolver,
      package_json_cache: DashMap::default(),
    }
  }
}

/// The result of a successful module resolution.
#[derive(Debug)]
pub struct ResolveReturn {
  pub path: ArcStr,
  pub module_def_format: ModuleDefFormat,
  pub package_json: Option<Arc<PackageJson>>,
}

impl From<ResolveReturn> for ResolvedId {
  fn from(resolve_return: ResolveReturn) -> Self {
    ResolvedId {
      id: resolve_return.path,
      module_def_format: resolve_return.module_def_format,
      package_json: resolve_return.package_json,
      ..Default::default()
    }
  }
}

impl<Fs: FileSystem> Resolver<Fs> {
  /// Returns the current working directory.
  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }

  /// Clears all caches (resolver cache and package.json cache).
  pub fn clear_cache(&self) {
    // All resolvers share the same cache, so clearing one is sufficient.
    self.default_resolver.clear_cache();
    self.package_json_cache.clear();
  }

  /// Resolves a module specifier to an absolute path.
  ///
  /// # Arguments
  /// * `importer` - The file that is importing the module (if any)
  /// * `specifier` - The module specifier to resolve (e.g., "./foo", "lodash")
  /// * `import_kind` - The type of import (ESM, CommonJS, CSS, etc.)
  /// * `is_user_defined_entry` - Whether this is a user-defined entry point
  ///
  /// # Errors
  /// Returns a `ResolveError` if the module cannot be resolved.
  pub fn resolve(
    &self,
    importer: Option<&Path>,
    specifier: &str,
    import_kind: ImportKind,
    is_user_defined_entry: bool,
  ) -> Result<ResolveReturn, ResolveError> {
    let resolver = match import_kind {
      ImportKind::Import | ImportKind::DynamicImport | ImportKind::HotAccept => {
        &self.import_resolver
      }
      ImportKind::NewUrl => &self.new_url_resolver,
      ImportKind::Require => &self.require_resolver,
      ImportKind::AtImport | ImportKind::UrlImport => &self.css_resolver,
    };

    let importer_dir = importer
      .and_then(|importer_path| importer_path.parent())
      .and_then(|parent_dir| parent_dir.components().next().is_some().then_some(parent_dir))
      .unwrap_or(self.cwd.as_path());

    let mut resolution = resolver.resolve(importer_dir, specifier);

    // Handle special case for user-defined entries to improve Rollup compatibility
    if resolution.is_err() && is_user_defined_entry {
      resolution =
        self.try_rollup_compatibility_resolve(resolver, importer_dir, specifier, resolution);
    }

    resolution.map(|info| {
      let package_json = info.package_json().map(|pkg| self.get_cached_package_json(pkg));

      // Infer module format from file extension and package.json type field
      // Reference: https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/bundler/bundler.go#L1446-L1460
      let mut module_def_format = ModuleDefFormat::from_path(info.path());
      if matches!(module_def_format, ModuleDefFormat::Unknown) {
        let is_js_like = info
          .path()
          .extension()
          .is_some_and(|ext| matches!(ext.to_str(), Some("js" | "jsx" | "ts" | "tsx")));
        if is_js_like {
          if let Some(module_type) = info.module_type() {
            module_def_format = match module_type {
              ModuleType::CommonJs => ModuleDefFormat::CjsPackageJson,
              ModuleType::Module => ModuleDefFormat::EsmPackageJson,
              ModuleType::Json | ModuleType::Wasm | ModuleType::Addon => ModuleDefFormat::Unknown,
            };
          }
        }
      }

      let path = info.full_path().to_str().expect("Path should be valid UTF-8").into();
      ResolveReturn { path, module_def_format, package_json }
    })
  }

  /// Attempts to resolve using Rollup compatibility mode.
  ///
  /// Rolldown doesn't pursue the exact same resolve behavior as Rollup, but in most cases
  /// the results are the same. This function handles a special case for better compatibility.
  ///
  /// When a user writes config like `{ input: 'main' }`, `main` would be treated as a npm
  /// package name in Rolldown and try to resolve it from `node_modules`. But Rollup will
  /// resolve it to `<CWD>/main.{js,mjs,cjs}`.
  ///
  /// This function simulates Rollup's behavior for this case.
  ///
  /// Related Rollup code: https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/src/utils/resolveId.ts#L56
  fn try_rollup_compatibility_resolve(
    &self,
    resolver: &ResolverGeneric<Fs>,
    context_dir: &Path,
    specifier: &str,
    original_resolution: Result<Resolution, ResolveError>,
  ) -> Result<Resolution, ResolveError> {
    let is_specifier_path_like = specifier.starts_with('.') || specifier.starts_with('/');

    if is_specifier_path_like {
      return original_resolution;
    }

    // Try resolving relative to cwd as a fallback
    let fallback_specifier = self.cwd.join(specifier).normalize().to_string_lossy().to_string();
    let fallback = resolver.resolve(context_dir, &fallback_specifier);

    if fallback.is_ok() { fallback } else { original_resolution }
  }

  /// Gets a cached package.json or caches a new one.
  fn get_cached_package_json(&self, oxc_package_json: &OxcPackageJson) -> Arc<PackageJson> {
    match self.package_json_cache.get(&oxc_package_json.realpath) {
      Some(cached) => Arc::clone(cached.value()),
      None => {
        let package_json = Arc::new(PackageJson::from_oxc_pkg_json(oxc_package_json));
        self
          .package_json_cache
          .insert(oxc_package_json.realpath.clone(), Arc::clone(&package_json));
        package_json
      }
    }
  }

  /// Retrieves a cached package.json or creates a new one.
  ///
  /// # Errors
  /// Returns an error if the file cannot be read or parsed.
  pub fn try_get_package_json_or_create(&self, path: &Path) -> anyhow::Result<Arc<PackageJson>> {
    self
      .load_package_json(path)
      .with_context(|| format!("Failed to read or parse package.json: {}", path.display()))
  }

  fn load_package_json(&self, path: &Path) -> anyhow::Result<Arc<PackageJson>> {
    if let Some(cached) = self.package_json_cache.get(path) {
      Ok(Arc::clone(cached.value()))
    } else {
      // The caller is responsible for ensuring `path` is a real path if needed.
      // We just pass it through.
      let realpath = path.to_path_buf();
      let json_bytes = self.fs.read(path)?;
      let oxc_package_json =
        OxcPackageJson::parse(&self.fs, path.to_path_buf(), realpath, json_bytes)?;
      let package_json = Arc::new(PackageJson::from_oxc_pkg_json(&oxc_package_json));
      self.package_json_cache.insert(path.to_path_buf(), Arc::clone(&package_json));
      Ok(package_json)
    }
  }

  /// Resolves a tsconfig file.
  pub fn resolve_tsconfig<T: AsRef<Path>>(&self, path: &T) -> Result<Arc<TsConfig>, ResolveError> {
    self.default_resolver.resolve_tsconfig(path)
  }
}
