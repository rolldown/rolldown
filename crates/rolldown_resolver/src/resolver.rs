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

impl<F: FileSystem> Resolver<F> {
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

  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }

  pub fn clear_cache(&self) {
    // All resolvers share the same cache, so just clear one of them is ok.
    self.default_resolver.clear_cache();
    self.package_json_cache.clear();
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

    let importer_dir = importer.and_then(|importer| importer.parent()).and_then(|inner| {
      if inner.components().next().is_none() {
        // Empty path `Path::new("")`
        None
      } else {
        Some(inner)
      }
    });

    let context_dir = importer_dir.unwrap_or(self.cwd.as_path());

    let mut resolution = selected_resolver.resolve(context_dir, specifier);

    if resolution.is_err() && is_user_defined_entry {
      let is_specifier_path_like = specifier.starts_with('.') || specifier.starts_with('/');
      let need_rollup_resolve_compat = !is_specifier_path_like;
      if need_rollup_resolve_compat {
        // Rolldown doesn't pursue to have the same resolve behavior as Rollup. Even though, in most cases, rolldown have the same resolve result as Rollup. And in this branch, it's the case that rolldown will perform differently from Rollup.

        // The case is user writes config like `{ input: 'main' }`. `main` would be treated as a npm package name in rolldown
        // and try to resolve it from `node_modules`. But rollup will resolve it to `<CWD>/main.{js,mjs,cjs}`.

        // So in this branch, to improve rollup-compatibility, we try to simulate the Rollup's resolve behavior in this case.
        // // Related rollup code: https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/src/utils/resolveId.ts#L56.
        let fallback = selected_resolver
          .resolve(context_dir, &self.cwd.join(specifier).normalize().to_string_lossy());
        if fallback.is_ok() {
          resolution = fallback;
        }
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
