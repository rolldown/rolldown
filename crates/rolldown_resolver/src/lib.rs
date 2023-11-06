use rolldown_common::{ModuleType, RawPath, ResourceId};
use rolldown_error::BuildError;
use rolldown_fs::{FileSystemExt, FileSystemOs};
use std::{
  borrow::Cow,
  os::unix::prelude::FileTypeExt,
  path::{Path, PathBuf},
  sync::Arc,
};
use sugar_path::SugarPathBuf;

use oxc_resolver::{Resolution, ResolveOptions, Resolver as OxcResolver, ResolverGeneric};

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

// impl<F: FileSystemExt + Default> Default for Resolver<F> {
//   fn default() -> Self {
//     Self::with_cwd_and_fs(std::env::current_dir().unwrap(), true, Arc::new(F::default()))
//   }
// }

#[derive(Debug)]
pub struct ResolveRet {
  pub resolved: RawPath,
  pub module_type: ModuleType,
}

impl<F: FileSystemExt + Default> Resolver<F> {
  #[allow(clippy::missing_errors_doc)]
  pub fn resolve(
    &self,
    importer: Option<&ResourceId>,
    specifier: &str,
  ) -> Result<ResolveRet, BuildError> {
    // If the importer is `None`, it means that the specifier is the entry file.
    // In this case, we couldn't simply use the CWD as the importer.
    // Instead, we should concat the CWD with the specifier. This aligns with https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/src/utils/resolveId.ts#L56.
    let specifier = if importer.is_none() {
      Cow::Owned(self.cwd.join(specifier).into_normalize())
    } else {
      Cow::Borrowed(Path::new(specifier))
    };

    let context = importer.map_or(self.cwd.as_path(), |s| {
      Path::new(s.as_ref()).parent().expect("Should have a parent dir")
    });

    let resolved = self.inner.resolve(context, &specifier.to_string_lossy());

    match resolved {
      Ok(info) => Ok(ResolveRet {
        resolved: info.path().to_string_lossy().to_string().into(),
        module_type: calc_module_type(&info),
      }),
      Err(_err) => importer.map_or_else(
        || Err(BuildError::unresolved_entry(specifier.to_str().unwrap())),
        |importer| {
          Err(BuildError::unresolved_import(
            specifier.to_string_lossy().to_string(),
            importer.prettify(),
          ))
        },
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
