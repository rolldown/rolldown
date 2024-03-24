use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use std::path::{Path, PathBuf};
use sugar_path::SugarPathBuf;

use oxc_resolver::{Resolution, ResolveError, ResolverGeneric};

use crate::{types::resolved_path::ResolvedPath, ModuleType, ResolveOptions};

#[derive(Debug)]
pub struct Resolver<T: FileSystem + Default> {
  cwd: PathBuf,
  inner: ResolverGeneric<T>,
}

impl<F: FileSystem + Default> Resolver<F> {
  pub fn new(options: ResolveOptions, cwd: PathBuf, fs: F) -> Self {
    let inner_resolver = ResolverGeneric::new_with_file_system(fs, options);
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
    importer: Option<&Path>,
    specifier: &str,
  ) -> Result<ResolveRet, BuildError> {
    let resolved = if let Some(importer) = importer {
      let context = importer.parent().expect("Should have a parent dir");
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
