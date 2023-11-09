use rolldown_common::{ModuleType, RawPath, ResourceId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystemExt;
use std::{path::PathBuf, sync::Arc};
use sugar_path::{AsPath, SugarPathBuf};

use oxc_resolver::{Resolution, ResolveOptions, ResolverGeneric};

#[derive(Debug)]
pub struct Resolver<T: FileSystemExt + Default> {
  cwd: PathBuf,
  inner: ResolverGeneric<Arc<T>>,
}

impl<F: FileSystemExt + Default> Resolver<F> {
  pub fn with_cwd_and_fs(cwd: PathBuf, preserve_symlinks: bool, fs: Arc<F>) -> Self {
    let resolve_options = ResolveOptions {
      symlinks: !preserve_symlinks,
      extensions: vec![
        ".js".to_string(),
        ".jsx".to_string(),
        ".ts".to_string(),
        ".tsx".to_string(),
      ],
      prefer_relative: false,
      ..Default::default()
    };

    let inner_resolver = ResolverGeneric::new_with_file_system(fs, resolve_options);
    Self { cwd, inner: inner_resolver }
  }

  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }
}

#[derive(Debug)]
pub struct ResolveRet {
  pub resolved: RawPath,
  pub module_type: ModuleType,
}

impl<F: FileSystemExt + Default> Resolver<F> {
  // clippy::option_if_let_else: I think the current code is more readable.
  #[allow(clippy::missing_errors_doc, clippy::option_if_let_else)]
  pub fn resolve(
    &self,
    importer: Option<&ResourceId>,
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
        resolved: info.path().to_string_lossy().to_string().into(),
        module_type: calc_module_type(&info),
      }),
      Err(_err) => importer.map_or_else(
        || Err(BuildError::unresolved_entry(specifier)),
        |importer| Err(BuildError::unresolved_import(specifier.to_string(), importer.prettify())),
      ),
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
