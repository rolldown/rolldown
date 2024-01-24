use rolldown_common::{FilePath, ModuleType, ResolvedPath};
use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use std::path::PathBuf;
use sugar_path::{AsPath, SugarPathBuf};

use oxc_resolver::{Resolution, ResolveError, ResolverGeneric};

use crate::ResolverOptions;

#[derive(Debug)]
pub struct Resolver<T: FileSystem + Default> {
  cwd: PathBuf,
  inner: ResolverGeneric<T>,
}

impl<F: FileSystem + Default> Resolver<F> {
  pub fn with_cwd_and_fs(cwd: PathBuf, resolver_options: Option<ResolverOptions>, fs: F) -> Self {
    let option = resolver_options.map_or_else(oxc_resolver::ResolveOptions::default, Into::into);
    let inner_resolver = ResolverGeneric::new_with_file_system(fs, option);
    Self { cwd, inner: inner_resolver }
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
    importer: Option<&FilePath>,
    specifier: &str,
  ) -> Result<ResolveRet, BuildError> {
    let resolved = if let Some(importer) = importer {
      let context = importer.as_path().parent().expect("Should have a parent dir");
      self.inner.resolve(context, specifier)
    } else {
      // If the importer is `None`, it means that the specifier is provided by the user in `input`. In this case, we can't call `resolver.resolve` with
      // `{ context: cwd, specifier: specifier }` due to rollup's default resolve behavior. For specifier `main`, rollup will try to resolve it as
      // `{ context: cwd, specifier: cwd.join(main) }`, which will resolve to `<cwd>/main.{js,mjs}`. To align with this behavior, we should also
      // concat the CWD with the specifier.
      // Related rollup code: https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/src/utils/resolveId.ts#L56.
      let joined_specifier = self.cwd.join(specifier).into_normalize();

      let is_path_like = specifier.starts_with('.') || specifier.starts_with('/');

      let resolved = self.inner.resolve(&self.cwd, joined_specifier.to_str().unwrap());
      if resolved.is_ok() {
        resolved
      } else if !is_path_like {
        // If the specifier is not path-like, we should try to resolve it as a bare specifier. This allows us to resolve modules from node_modules.
        self.inner.resolve(&self.cwd, specifier)
      } else {
        resolved
      }
    };

    match resolved {
      Ok(info) => Ok(ResolveRet {
        resolved: ResolvedPath {
          path: info.path().to_string_lossy().to_string().into(),
          ignored: false,
        },
        module_type: calc_module_type(&info),
      }),
      Err(err) => {
        if let ResolveError::Ignored(path) = err {
          Ok(ResolveRet {
            resolved: ResolvedPath {
              path: path.to_string_lossy().to_string().into(),
              ignored: true,
            },
            module_type: ModuleType::CJS,
          })
        } else if let Some(importer) = importer {
          Err(
            BuildError::unresolved_import(specifier.to_string(), importer.as_path())
              .with_source(err),
          )
        } else {
          Err(BuildError::unresolved_entry(specifier).with_source(err))
        }
      }
    }
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
