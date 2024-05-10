use rolldown_common::{ImportKind, ModuleType, Platform, ResolveOptions, ResolvedPath};
use rolldown_error::{BuildError, BuildResult};
use rolldown_fs::FileSystem;
use std::path::{Path, PathBuf};
use sugar_path::SugarPath;

use oxc_resolver::{
  EnforceExtension, Resolution, ResolveError, ResolveOptions as OxcResolverOptions,
  ResolverGeneric, TsconfigOptions,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Resolver<T: FileSystem + Default> {
  cwd: PathBuf,
  default_resolver: ResolverGeneric<T>,
  import_resolver: ResolverGeneric<T>,
  require_resolver: ResolverGeneric<T>,
}

impl<F: FileSystem + Default> Resolver<F> {
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
    default_conditions.dedup();
    import_conditions.extend(default_conditions.clone());
    require_conditions.extend(default_conditions.clone());
    import_conditions.dedup();
    require_conditions.dedup();

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

    let resolve_options_with_default_conditions = OxcResolverOptions {
      tsconfig: raw_resolve.tsconfig_filename.map(|p| TsconfigOptions {
        config_file: p.into(),
        references: oxc_resolver::TsconfigReferences::Disabled,
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
      extension_alias: vec![],
      extensions: raw_resolve
        .extensions
        .unwrap_or_else(|| [".jsx", ".js"].into_iter().map(str::to_string).collect()),
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
      builtin_modules: false,
    };
    let resolve_options_with_import_conditions = OxcResolverOptions {
      condition_names: import_conditions,
      ..resolve_options_with_default_conditions.clone()
    };
    let resolve_options_with_require_conditions = OxcResolverOptions {
      condition_names: require_conditions,
      ..resolve_options_with_default_conditions.clone()
    };
    let default_resolver =
      ResolverGeneric::new_with_file_system(fs, resolve_options_with_default_conditions);
    let import_resolver =
      default_resolver.clone_with_options(resolve_options_with_import_conditions);
    let require_resolver =
      default_resolver.clone_with_options(resolve_options_with_require_conditions);

    Self { cwd, default_resolver, import_resolver, require_resolver }
  }

  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }
}

#[derive(Debug)]
pub struct ResolveRet {
  pub resolved: ResolvedPath,
  pub module_type: ModuleType,
}

impl<F: FileSystem + Default> Resolver<F> {
  // clippy::option_if_let_else: I think the current code is more readable.
  #[allow(clippy::missing_errors_doc, clippy::option_if_let_else)]
  pub fn resolve(
    &self,
    importer: Option<&Path>,
    specifier: &str,
    import_kind: ImportKind,
  ) -> BuildResult<ResolveRet> {
    let selected_resolver = match import_kind {
      ImportKind::Import | ImportKind::DynamicImport => &self.import_resolver,
      ImportKind::Require => &self.require_resolver,
    };
    let resolved = if let Some(importer) = importer {
      let context = importer.parent().expect("Should have a parent dir");
      selected_resolver.resolve(context, specifier)
    } else {
      // If the importer is `None`, it means that the specifier is provided by the user in `input`. In this case, we can't call `resolver.resolve` with
      // `{ context: cwd, specifier: specifier }` due to rollup's default resolve behavior. For specifier `main`, rollup will try to resolve it as
      // `{ context: cwd, specifier: cwd.join(main) }`, which will resolve to `<cwd>/main.{js,mjs}`. To align with this behavior, we should also
      // concat the CWD with the specifier.
      // Related rollup code: https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/src/utils/resolveId.ts#L56.
      let joined_specifier = self.cwd.join(specifier).normalize();

      let is_path_like = specifier.starts_with('.') || specifier.starts_with('/');

      let resolved = selected_resolver.resolve(&self.cwd, joined_specifier.to_str().unwrap());
      if resolved.is_ok() {
        resolved
      } else if !is_path_like {
        // If the specifier is not path-like, we should try to resolve it as a bare specifier. This allows us to resolve modules from node_modules.
        selected_resolver.resolve(&self.cwd, specifier)
      } else {
        resolved
      }
    };
    resolved
      // If result type parsing is correct
      .map(|info| {
        build_resolve_ret(
          info.full_path().to_str().expect("should be valid utf8").to_string(),
          false,
          calc_module_type(&info),
        )
      })
      .or_else(|err| match err {
        // If the error type is ignore
        ResolveError::Ignored(path) => Ok(build_resolve_ret(
          path.to_str().expect("should be valid utf8").to_string(),
          true,
          ModuleType::CJS,
        )),
        // To determine whether there is an importer.
        _ => {
          if let Some(importer) = importer {
            Err(BuildError::unresolved_import(specifier.to_string(), importer).with_source(err))
          } else {
            Err(BuildError::unresolved_entry(specifier).with_source(err))
          }
        }
      })
  }
}

fn calc_module_type(info: &Resolution) -> ModuleType {
  if let Some(extension) = info.path().extension() {
    if extension == "mjs" {
      return ModuleType::EsmMjs;
    } else if extension == "cjs" {
      return ModuleType::CJS;
    }
  }
  if let Some(package_json) = info.package_json() {
    let type_value = package_json.raw_json().get("type").and_then(|v| v.as_str());
    if type_value == Some("module") {
      return ModuleType::EsmPackageJson;
    } else if type_value == Some("commonjs") {
      return ModuleType::CjsPackageJson;
    }
  }
  ModuleType::Unknown
}

fn build_resolve_ret(path: String, ignored: bool, module_type: ModuleType) -> ResolveRet {
  ResolveRet { resolved: ResolvedPath { path: path.into(), ignored }, module_type }
}
