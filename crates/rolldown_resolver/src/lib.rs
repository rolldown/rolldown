use rolldown_common::{ModuleType, RawPath, ResourceId};
use rolldown_error::Error as RError;
use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use oxc_resolver::{Resolution, ResolveOptions, Resolver as OxcResolver};

#[derive(Debug)]
pub struct Resolver {
  cwd: PathBuf,
  inner: OxcResolver,
}

impl Resolver {
  pub fn with_cwd(cwd: PathBuf, preserve_symlinks: bool) -> Self {
    Self {
      cwd,
      inner: OxcResolver::new(ResolveOptions {
        symlinks: !preserve_symlinks,
        extensions: vec![
          ".js".to_string(),
          ".jsx".to_string(),
          ".ts".to_string(),
          ".tsx".to_string(),
        ],
        prefer_relative: false,
        ..Default::default()
      }),
    }
  }

  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }
}

impl Default for Resolver {
  fn default() -> Self {
    Self::with_cwd(std::env::current_dir().unwrap(), true)
  }
}

#[derive(Debug)]
pub struct ResolveRet {
  pub resolved: RawPath,
  pub module_type: ModuleType,
}

impl Resolver {
  #[allow(clippy::missing_errors_doc)]
  pub fn resolve(
    &self,
    importer: Option<&ResourceId>,
    specifier: &str,
  ) -> Result<ResolveRet, Box<RError>> {
    // If the importer is `None`, it means that the specifier is the entry file.
    // In this case, we couldn't simply use the CWD as the importer.
    // Instead, we should concat the CWD with the specifier. This aligns with https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/src/utils/resolveId.ts#L56.
    let specifier = if importer.is_none() {
      Cow::Owned(self.cwd.join(specifier))
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
      Err(_err) => {
        if let Some(importer) = importer {
          Err(Box::new(RError::unresolved_import(
            specifier.to_string_lossy().to_string(),
            importer.prettify(),
          )))
        } else {
          Err(Box::new(RError::unresolved_entry(specifier)))
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
    if package_json.raw_json().get("type").and_then(|v| v.as_str()) == Some("module") {
      return ModuleType::EsmPackageJson;
    }
  }
   ModuleType::Unknown
}
