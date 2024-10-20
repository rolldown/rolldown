use arcstr::ArcStr;
use dashmap::DashMap;
use itertools::Itertools;
use rolldown_common::{ImportKind, ModuleDefFormat, PackageJson, Platform, ResolveOptions};
use rolldown_fs::{FileSystem, OsFileSystem};
use std::{
  path::{Path, PathBuf},
  sync::Arc,
};
use sugar_path::SugarPath;

use oxc_resolver::{
  EnforceExtension, PackageJson as OxcPackageJson, Resolution, ResolveError,
  ResolveOptions as OxcResolverOptions, ResolverGeneric, TsconfigOptions,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Resolver<T: FileSystem + Default = OsFileSystem> {
  cwd: PathBuf,
  default_resolver: ResolverGeneric<T>,
  import_resolver: ResolverGeneric<T>,
  require_resolver: ResolverGeneric<T>,
  css_resolver: ResolverGeneric<T>,
  package_json_cache: DashMap<PathBuf, Arc<PackageJson>>,
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
      extension_alias: raw_resolve.extension_alias.unwrap_or_default(),
      extensions: raw_resolve.extensions.unwrap_or_else(|| {
        [".jsx", ".js", ".ts", ".tsx"].into_iter().map(str::to_string).collect()
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

    let default_resolver =
      ResolverGeneric::new_with_file_system(fs, resolve_options_with_default_conditions);
    let import_resolver =
      default_resolver.clone_with_options(resolve_options_with_import_conditions);
    let require_resolver =
      default_resolver.clone_with_options(resolve_options_with_require_conditions);
    let css_resolver = default_resolver.clone_with_options(resolve_options_for_css);

    Self {
      cwd,
      default_resolver,
      import_resolver,
      require_resolver,
      css_resolver,
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

impl<F: FileSystem + Default> Resolver<F> {
  pub fn resolve(
    &self,
    importer: Option<&Path>,
    specifier: &str,
    import_kind: ImportKind,
    is_user_defined_entry: bool,
  ) -> anyhow::Result<Result<ResolveReturn, ResolveError>> {
    let selected_resolver = match import_kind {
      ImportKind::Import | ImportKind::DynamicImport => &self.import_resolver,
      ImportKind::Require => &self.require_resolver,
      ImportKind::AtImport => &self.css_resolver,
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

    match resolution {
      Ok(info) => {
        let package_json = info.package_json().map(|p| self.cached_package_json(p));
        let module_type = calc_module_type(&info);
        Ok(Ok(build_resolve_ret(
          info.full_path().to_str().expect("Should be valid utf8").to_string(),
          module_type,
          package_json,
        )))
      }
      Err(err) => Ok(Err(err)),
    }
  }

  fn cached_package_json(&self, oxc_pkg_json: &OxcPackageJson) -> Arc<PackageJson> {
    if let Some(v) = self.package_json_cache.get(&oxc_pkg_json.realpath) {
      Arc::clone(v.value())
    } else {
      let pkg_json = Arc::new(
        PackageJson::new(oxc_pkg_json.path.clone())
          .with_type(oxc_pkg_json.r#type.as_ref())
          .with_side_effects(oxc_pkg_json.side_effects.as_ref()),
      );
      self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
      pkg_json
    }
  }
}

fn calc_module_type(info: &Resolution) -> ModuleDefFormat {
  if let Some(extension) = info.path().extension() {
    if extension == "mjs" {
      return ModuleDefFormat::EsmMjs;
    } else if extension == "cjs" {
      return ModuleDefFormat::CJS;
    }
  }
  if let Some(package_json) = info.package_json() {
    let type_value = package_json.r#type.as_ref().and_then(|v| v.as_str());
    if type_value == Some("module") {
      return ModuleDefFormat::EsmPackageJson;
    } else if type_value == Some("commonjs") {
      return ModuleDefFormat::CjsPackageJson;
    }
  }
  ModuleDefFormat::Unknown
}

fn build_resolve_ret(
  path: String,
  module_type: ModuleDefFormat,
  package_json: Option<Arc<PackageJson>>,
) -> ResolveReturn {
  ResolveReturn { path: path.into(), module_def_format: module_type, package_json }
}
