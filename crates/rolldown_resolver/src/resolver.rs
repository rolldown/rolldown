use arcstr::ArcStr;
use dashmap::DashMap;
use itertools::Itertools;
use rolldown_common::{
  ImportKind, ModuleDefFormat, PackageJson, Platform, ResolveOptions, ResolvedId,
};
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_utils::{dashmap::FxDashMap, indexmap::FxIndexMap};
use std::{
  path::{Path, PathBuf},
  sync::Arc,
};
use sugar_path::SugarPath;

use oxc_resolver::{
  EnforceExtension, FsCache, PackageJsonSerde as OxcPackageJson, PackageType, Resolution,
  ResolveError, ResolveOptions as OxcResolverOptions, ResolverGeneric, TsConfigSerde,
  TsconfigOptions,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Resolver<T: FileSystem + Default = OsFileSystem> {
  cwd: PathBuf,
  default_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `import '...'` and `import(...)`
  import_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `require('...')`
  require_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `@import '...'` and `url('...')`
  css_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `new URL(..., import.meta.url)`
  new_url_resolver: ResolverGeneric<FsCache<T>>,
  package_json_cache: FxDashMap<PathBuf, Arc<PackageJson>>,
}

impl<F: FileSystem + Default> Resolver<F> {
  #[allow(clippy::too_many_lines)]
  pub fn new(raw_resolve: ResolveOptions, platform: Platform, cwd: PathBuf, fs: F) -> Self {
    let mut default_conditions = vec!["default".to_string()];
    let mut import_conditions = vec!["import".to_string()];
    let mut require_conditions = vec!["require".to_string()];

    default_conditions.extend(raw_resolve.condition_names.clone().unwrap_or_default());
    match platform {
      Platform::Node => {
        default_conditions.push("node".to_string());
      }
      Platform::Browser => {
        default_conditions.push("browser".to_string());
      }
      Platform::Neutral => {}
    }
    default_conditions = default_conditions.into_iter().unique().collect();
    import_conditions.extend(default_conditions.clone());
    require_conditions.extend(default_conditions.clone());
    import_conditions = import_conditions.into_iter().unique().collect();
    require_conditions = require_conditions.into_iter().unique().collect();

    let main_fields = raw_resolve.main_fields.clone().unwrap_or_else(|| match platform {
      Platform::Node => {
        vec!["main".to_string(), "module".to_string()]
      }
      Platform::Browser => vec!["browser".to_string(), "module".to_string(), "main".to_string()],
      Platform::Neutral => vec![],
    });

    let alias_fields = raw_resolve.alias_fields.clone().unwrap_or_else(|| match platform {
      Platform::Browser => vec![vec!["browser".to_string()]],
      _ => vec![],
    });

    let builtin_modules = match platform {
      Platform::Node => true,
      Platform::Browser | Platform::Neutral => false,
    };

    let mut extension_alias = raw_resolve.extension_alias.clone().unwrap_or_default();
    impl_rewritten_file_extensions_via_extension_alias(&mut extension_alias);

    let resolve_options_with_default_conditions = OxcResolverOptions {
      tsconfig: raw_resolve.tsconfig_filename.map(|p| {
        let path = PathBuf::from(&p);
        TsconfigOptions {
          config_file: if path.is_relative() { cwd.join(path) } else { path },
          references: oxc_resolver::TsconfigReferences::Disabled,
        }
      }),
      alias: raw_resolve
        .alias
        .map(|alias| {
          alias
            .into_iter()
            .map(|(key, value)| {
              (key, value.into_iter().map(oxc_resolver::AliasValue::Path).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
      imports_fields: vec![vec!["imports".to_string()]],
      alias_fields,
      condition_names: default_conditions,
      description_files: vec!["package.json".to_string()],
      enforce_extension: EnforceExtension::Auto,
      exports_fields: raw_resolve
        .exports_fields
        .unwrap_or_else(|| vec![vec!["exports".to_string()]]),
      extension_alias,
      extensions: raw_resolve.extensions.unwrap_or_else(|| {
        [".tsx", ".ts", ".jsx", ".js", ".json"].into_iter().map(str::to_string).collect()
      }),
      fallback: vec![],
      fully_specified: false,
      main_fields,
      main_files: raw_resolve.main_files.unwrap_or_else(|| vec!["index".to_string()]),
      modules: raw_resolve.modules.unwrap_or_else(|| vec!["node_modules".to_string()]),
      resolve_to_context: false,
      prefer_relative: false,
      prefer_absolute: false,
      restrictions: vec![],
      roots: vec![],
      symlinks: raw_resolve.symlinks.unwrap_or(true),
      builtin_modules,
    };
    let resolve_options_with_import_conditions = OxcResolverOptions {
      condition_names: import_conditions,
      ..resolve_options_with_default_conditions.clone()
    };
    let resolve_options_with_require_conditions = OxcResolverOptions {
      condition_names: require_conditions,
      ..resolve_options_with_default_conditions.clone()
    };

    let resolve_options_for_css = OxcResolverOptions {
      prefer_relative: true,
      ..resolve_options_with_default_conditions.clone()
    };

    let resolve_options_for_new_url = OxcResolverOptions {
      prefer_relative: true,
      ..resolve_options_with_default_conditions.clone()
    };

    let default_resolver = ResolverGeneric::new_with_cache(
      Arc::new(FsCache::new(fs)),
      resolve_options_with_default_conditions,
    );
    let import_resolver =
      default_resolver.clone_with_options(resolve_options_with_import_conditions);
    let require_resolver =
      default_resolver.clone_with_options(resolve_options_with_require_conditions);
    let css_resolver = default_resolver.clone_with_options(resolve_options_for_css);
    let new_url_resolver = default_resolver.clone_with_options(resolve_options_for_new_url);
    Self {
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
      ignored: false,
      module_def_format: resolved_return.module_def_format,
      external: false.into(),
      normalize_external_id: None,
      package_json: resolved_return.package_json,
      side_effects: None,
      is_external_without_side_effects: false,
    }
  }
}

impl<F: FileSystem + Default> Resolver<F> {
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
        let pkg_json = Arc::new(
          PackageJson::new(oxc_pkg_json.path.clone())
            .with_type(oxc_pkg_json.r#type.map(|t| match t {
              PackageType::CommonJs => "commonjs",
              PackageType::Module => "module",
            }))
            .with_side_effects(oxc_pkg_json.side_effects.as_ref())
            .with_version(oxc_pkg_json.raw_json().get("version").and_then(|v| v.as_str())),
        );
        self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
        pkg_json
      }
    }
  }

  pub fn resolve_tsconfig<T: AsRef<Path>>(
    &self,
    path: &T,
  ) -> Result<Arc<TsConfigSerde>, ResolveError> {
    self.default_resolver.resolve_tsconfig(path)
  }
}

/// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/bundler/bundler.go#L1446-L1460
fn infer_module_def_format<F: FileSystem + Default>(
  info: &Resolution<FsCache<F>>,
) -> ModuleDefFormat {
  let fmt = ModuleDefFormat::from_path(info.path());

  if !matches!(fmt, ModuleDefFormat::Unknown) {
    return fmt;
  }
  let is_js_like_extension = info
    .path()
    .extension()
    .is_some_and(|ext| matches!(ext.to_str(), Some("js" | "jsx" | "ts" | "tsx")));
  if let Some(package_json) =
    is_js_like_extension.then(|| info.package_json()).and_then(|item| item)
  {
    return match package_json.r#type {
      Some(PackageType::CommonJs) => ModuleDefFormat::CjsPackageJson,
      Some(PackageType::Module) => ModuleDefFormat::EsmPackageJson,
      _ => ModuleDefFormat::Unknown,
    };
  }
  ModuleDefFormat::Unknown
}

// Support esbuild's `rewrittenFileExtensions` feature. https://github.com/evanw/esbuild/blob/a08f30db4a475472aa09cd89e2279a822266f6c7/internal/resolver/resolver.go#L1622-L1644
// Some notices:
// - We are using `extension_alias` feature to simulate the esbuild's `rewrittenFileExtensions` feature. But there are differences things, so we need to handle them carefully.
// - `rewrittenFileExtensions` is not overridable by user config.
// - `rewrittenFileExtensions` couldn't override user's config.
fn impl_rewritten_file_extensions_via_extension_alias(
  extension_alias: &mut Vec<(String, Vec<String>)>,
) {
  // The first alias is the original extension to make sure that `foo.js` will be resolved to `foo.js` if `foo.js` exists.
  let mut rewritten_file_extensions = FxIndexMap::from_iter([
    (".js".to_string(), vec![".js".to_string(), ".ts".to_string(), ".tsx".to_string()]),
    (".jsx".to_string(), vec![".jsx".to_string(), ".ts".to_string(), ".tsx".to_string()]),
    (".mjs".to_string(), vec![".mjs".to_string(), ".mts".to_string()]),
    (".cjs".to_string(), vec![".cjs".to_string(), ".cts".to_string()]),
  ]);
  extension_alias.iter_mut().for_each(|(ext, aliases)| {
    if let Some(rewrites) = rewritten_file_extensions.shift_remove(ext) {
      aliases.extend(rewrites);
    }
  });

  extension_alias.extend(rewritten_file_extensions);
}
