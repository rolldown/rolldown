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

#[derive(Debug)]
#[expect(clippy::struct_field_names)]
pub struct Resolver<T: FileSystem = OsFileSystem> {
  fs: T,
  cwd: PathBuf,
  default_resolver: ResolverGeneric<T>,
  // Resolver for `import '...'` and `import(...)`
  import_resolver: ResolverGeneric<T>,
  // Resolver for `require('...')`
  require_resolver: ResolverGeneric<T>,
  // Resolver for `@import '...'` and `url('...')`
  css_resolver: ResolverGeneric<T>,
  // Resolver for `new URL(..., import.meta.url)`
  new_url_resolver: ResolverGeneric<T>,
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

#[derive(Debug)]
pub struct ResolveReturn {
  pub path: ArcStr,
  pub module_def_format: ModuleDefFormat,
  pub package_json: Option<Arc<PackageJson>>,
}

impl From<ResolveReturn> for ResolvedId {
  fn from(resolved_return: ResolveReturn) -> Self {
    ResolvedId {
      id: resolved_return.path,
      module_def_format: resolved_return.module_def_format,
      package_json: resolved_return.package_json,
      ..Default::default()
    }
  }
}

impl<F: FileSystem> Resolver<F> {
  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }

  pub fn clear_cache(&self) {
    // All resolvers share the same cache, so just clear one of them is ok.
    self.default_resolver.clear_cache();
    self.package_json_cache.clear();
  }

  pub fn try_get_package_json_or_create(&self, path: &Path) -> anyhow::Result<Arc<PackageJson>> {
    self
      .inner_try_get_package_json_or_create(path)
      .with_context(|| format!("Failed to read or parse package.json: {}", path.display()))
  }

  fn inner_try_get_package_json_or_create(&self, path: &Path) -> anyhow::Result<Arc<PackageJson>> {
    if let Some(v) = self.package_json_cache.get(path) {
      Ok(Arc::clone(v.value()))
    } else {
      // User have has the responsibility to ensure `path` is real path if needed. We just pass it through.
      let realpath = path.to_path_buf();
      let json_bytes = self.fs.read(path)?;
      let oxc_pkg_json = OxcPackageJson::parse(&self.fs, path.to_path_buf(), realpath, json_bytes)?;
      let pkg_json = Arc::new(PackageJson::from_oxc_pkg_json(&oxc_pkg_json));
      self.package_json_cache.insert(path.to_path_buf(), Arc::clone(&pkg_json));
      Ok(pkg_json)
    }
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
    let selected_resolver = match import_kind {
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

    let mut resolution = selected_resolver.resolve(importer_dir, specifier);

    if resolution.is_err()
      && is_user_defined_entry
      && !specifier.starts_with('.')
      && !specifier.starts_with('/')
    {
      // Attempts to resolve using Rollup compatibility mode.
      //
      // Rolldown doesn't pursue the exact same resolve behavior as Rollup, but in most cases
      // the results are the same. This function handles a special case for better compatibility.
      //
      // When a user writes config like `{ input: 'main' }`, `main` would be treated as a npm
      // package name in Rolldown and try to resolve it from `node_modules`. But Rollup will
      // resolve it to `<CWD>/main.{js,mjs,cjs}`.
      //
      // Related Rollup code: https://github.com/rollup/rollup/blob/680912e2/src/utils/resolveId.ts#L56
      let fallback = selected_resolver
        .resolve(importer_dir, &self.cwd.join(specifier).normalize().to_string_lossy());
      if fallback.is_ok() {
        resolution = fallback;
      }
    }

    resolution.map(|info| {
      let package_json = info.package_json().map(|p| self.cached_package_json(p));
      let module_def_format = infer_module_def_format(&info);
      ResolveReturn {
        path: info.full_path().to_str().expect("Should be valid utf8").into(),
        module_def_format,
        package_json,
      }
    })
  }

  fn cached_package_json(&self, oxc_pkg_json: &OxcPackageJson) -> Arc<PackageJson> {
    match self.package_json_cache.get(&oxc_pkg_json.realpath) {
      Some(v) => Arc::clone(v.value()),
      _ => {
        let pkg_json = Arc::new(PackageJson::from_oxc_pkg_json(oxc_pkg_json));
        self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
        pkg_json
      }
    }
  }

  pub fn resolve_tsconfig<T: AsRef<Path>>(&self, path: &T) -> Result<Arc<TsConfig>, ResolveError> {
    self.default_resolver.resolve_tsconfig(path)
  }
}

/// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/bundler/bundler.go#L1446-L1460
fn infer_module_def_format(info: &Resolution) -> ModuleDefFormat {
  let fmt = ModuleDefFormat::from_path(info.path());

  if !matches!(fmt, ModuleDefFormat::Unknown) {
    return fmt;
  }
  let is_js_like_extension = info
    .path()
    .extension()
    .is_some_and(|ext| matches!(ext.to_str(), Some("js" | "jsx" | "ts" | "tsx")));
  if is_js_like_extension {
    if let Some(module_type) = info.module_type() {
      return match module_type {
        ModuleType::CommonJs => ModuleDefFormat::CjsPackageJson,
        ModuleType::Module => ModuleDefFormat::EsmPackageJson,
        ModuleType::Json | ModuleType::Wasm | ModuleType::Addon => ModuleDefFormat::Unknown,
      };
    }
  }
  ModuleDefFormat::Unknown
}
