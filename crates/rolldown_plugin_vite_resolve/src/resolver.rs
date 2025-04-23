use std::{
  ffi::OsString,
  fs,
  path::{self, Path},
  sync::Arc,
};

use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{HookResolveIdOutput, HookResolveIdReturn};
use rustc_hash::FxHashSet;
use sugar_path::SugarPath;

use crate::{
  package_json_cache::{PackageJsonCache, PackageJsonWithOptionalPeerDependencies},
  utils::{
    BROWSER_EXTERNAL_ID, OPTIONAL_PEER_DEP_ID, can_externalize_file, clean_url, get_extension,
    get_npm_package_name, is_bare_import, is_builtin, is_deep_import, normalize_path,
  },
};

pub struct BaseOptions<'a> {
  pub main_fields: &'a Vec<String>,
  pub conditions: &'a Vec<String>,
  pub extensions: &'a Vec<String>,
  pub is_production: bool,
  pub try_index: bool,
  pub try_prefix: &'a Option<String>,
  pub as_src: bool,
  pub root: &'a str,
  pub preserve_symlinks: bool,
}

const ADDITIONAL_OPTIONS_FIELD_COUNT: u8 = 2;
const RESOLVER_COUNT: u8 = 2_u8.pow(ADDITIONAL_OPTIONS_FIELD_COUNT as u32);

const DEV_PROD_CONDITION: &str = "development|production";

#[derive(Debug)]
pub struct AdditionalOptions {
  is_require: bool,
  prefer_relative: bool,
}

impl AdditionalOptions {
  pub fn new(is_require: bool, prefer_relative: bool) -> Self {
    Self { is_require, prefer_relative }
  }

  fn as_bools(&self) -> [bool; ADDITIONAL_OPTIONS_FIELD_COUNT as usize] {
    [self.is_require, self.prefer_relative]
  }

  fn as_u8(&self) -> u8 {
    bools_to_u8(self.as_bools())
  }
}

impl From<[bool; RESOLVER_COUNT as usize]> for AdditionalOptions {
  fn from(value: [bool; RESOLVER_COUNT as usize]) -> Self {
    Self { is_require: value[0], prefer_relative: value[1] }
  }
}

impl From<u8> for AdditionalOptions {
  fn from(value: u8) -> Self {
    u8_to_bools(value).into()
  }
}

#[derive(Debug)]
pub struct Resolvers {
  resolvers: [Resolver; RESOLVER_COUNT as usize],
  external_resolver: Arc<Resolver>,
}

impl Resolvers {
  pub fn new(
    base_options: &BaseOptions,
    external_conditions: &Vec<String>,
    runtime: String,
  ) -> Self {
    let package_json_cache = Arc::new(PackageJsonCache::default());

    let base_resolver = oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions::default());

    let resolvers = (0..RESOLVER_COUNT)
      .map(|v| {
        Resolver::new(
          base_resolver.clone_with_options(get_resolve_options(base_options, v.into())),
          Arc::clone(&package_json_cache),
          runtime.clone(),
          base_options.root.to_owned(),
          base_options.try_prefix.to_owned(),
        )
      })
      .collect::<Vec<_>>()
      .try_into()
      .unwrap();

    let external_resolver = Resolver::new(
      base_resolver.clone_with_options(get_resolve_options(
        &BaseOptions { is_production: false, conditions: external_conditions, ..*base_options },
        AdditionalOptions { is_require: false, prefer_relative: false },
      )),
      Arc::clone(&package_json_cache),
      runtime,
      base_options.root.to_owned(),
      base_options.try_prefix.to_owned(),
    );

    Self { resolvers, external_resolver: Arc::new(external_resolver) }
  }

  pub fn get(&self, additional_options: AdditionalOptions) -> &Resolver {
    &self.resolvers[additional_options.as_u8() as usize]
  }

  pub fn get_for_external(&self) -> Arc<Resolver> {
    Arc::clone(&self.external_resolver)
  }

  pub fn clear_cache(&self) {
    self.resolvers.iter().for_each(|v| v.clear_cache());
    self.external_resolver.clear_cache();
  }
}

fn get_resolve_options(
  base_options: &BaseOptions,
  additional_options: AdditionalOptions,
) -> oxc_resolver::ResolveOptions {
  let mut extensions = base_options.extensions.clone();
  extensions.push(".node".to_string());

  let main_fields = base_options.main_fields.clone();

  oxc_resolver::ResolveOptions {
    alias_fields: if base_options.main_fields.iter().any(|field| field == "browser") {
      vec![vec!["browser".to_string()]]
    } else {
      vec![]
    },
    condition_names: get_conditions(base_options, &additional_options),
    extensions,
    extension_alias: vec![
      (".js".to_string(), vec![".ts".to_string(), ".tsx".to_string(), ".js".to_string()]),
      (".jsx".to_string(), vec![".ts".to_string(), ".tsx".to_string(), ".jsx".to_string()]),
      (".mjs".to_string(), vec![".mts".to_string(), ".mjs".to_string()]),
      (".cjs".to_string(), vec![".cts".to_string(), ".cjs".to_string()]),
    ],
    main_fields,
    main_files: if !base_options.try_index {
      vec![]
    } else if let Some(try_prefix) = &base_options.try_prefix {
      vec![format!("{try_prefix}index"), "index".to_string()]
    } else {
      vec!["index".to_string()]
    },
    prefer_relative: additional_options.prefer_relative,
    roots: if base_options.as_src { vec![base_options.root.into()] } else { vec![] },
    symlinks: !base_options.preserve_symlinks,
    ..Default::default()
  }
}

fn get_conditions(
  base_options: &BaseOptions,
  additional_options: &AdditionalOptions,
) -> Vec<String> {
  let mut conditions = base_options
    .conditions
    .iter()
    .map(|c| {
      if c == DEV_PROD_CONDITION {
        if base_options.is_production { "production" } else { "development" }
      } else {
        c
      }
    })
    .map(|c| c.to_string())
    .collect::<Vec<_>>();

  if additional_options.is_require {
    conditions.push("require".to_string());
  } else {
    conditions.push("import".to_string());
  }

  conditions
}

fn bools_to_u8<const N: usize>(bools: [bool; N]) -> u8 {
  bools.iter().enumerate().map(|(i, v)| if *v { 1 << i } else { 0 }).sum()
}

fn u8_to_bools<const N: usize>(n: u8) -> [bool; N] {
  let mut ret = [false; N];
  ret.iter_mut().enumerate().for_each(|(i, v)| *v = n & (1 << i) != 0);
  ret
}

#[derive(Debug)]
pub struct Resolver {
  inner: oxc_resolver::Resolver,
  package_json_cache: Arc<PackageJsonCache>,
  runtime: String,
  root: String,
  try_prefix: Option<String>,
}

impl Resolver {
  pub fn new(
    inner: oxc_resolver::Resolver,
    package_json_cache: Arc<PackageJsonCache>,
    runtime: String,
    root: String,
    try_prefix: Option<String>,
  ) -> Self {
    Self { inner, package_json_cache, runtime, root, try_prefix }
  }

  pub fn resolve_raw<P: AsRef<Path>>(
    &self,
    directory: P,
    specifier: &str,
  ) -> Result<oxc_resolver::FsResolution, oxc_resolver::ResolveError> {
    let Some(try_prefix) = &self.try_prefix else {
      return self.inner.resolve(directory, specifier);
    };

    let mut path = Path::new(specifier).components();
    let Some(path::Component::Normal(filename)) = path.next_back() else {
      return self.inner.resolve(directory, specifier);
    };

    let mut filename_with_prefix = OsString::with_capacity(try_prefix.len() + filename.len());
    filename_with_prefix.push(try_prefix);
    filename_with_prefix.push(filename);

    let path_with_prefix = path.as_path().join(filename_with_prefix);
    let Some(path_with_prefix) = path_with_prefix.to_str() else {
      return self.inner.resolve(directory, specifier);
    };

    let result_with_prefix = self.inner.resolve(directory.as_ref(), path_with_prefix);
    match result_with_prefix {
      Err(
        oxc_resolver::ResolveError::NotFound(_)
        | oxc_resolver::ResolveError::ExtensionAlias(_, _, _),
      ) => self.inner.resolve(directory, specifier),
      _ => result_with_prefix,
    }
  }

  pub fn normalize_oxc_resolver_result(
    &self,
    importer: Option<&str>,
    dedupe: &FxHashSet<String>,
    result: &Result<oxc_resolver::FsResolution, oxc_resolver::ResolveError>,
  ) -> Result<Option<HookResolveIdOutput>, oxc_resolver::ResolveError> {
    match result {
      Ok(result) => {
        let raw_path = result.full_path().to_str().unwrap().to_string();
        let path = normalize_path(&raw_path);

        let side_effects = result
          .package_json()
          .and_then(|pkg_json| {
            self
              .package_json_cache
              .cached_package_json_side_effects(pkg_json)
              .check_side_effects_for(&raw_path)
          })
          .map(
            |side_effects| {
              if side_effects { HookSideEffects::True } else { HookSideEffects::False }
            },
          );
        Ok(Some(HookResolveIdOutput { id: path.into(), side_effects, ..Default::default() }))
      }
      Err(oxc_resolver::ResolveError::NotFound(id)) => {
        // if import can't be found, check if it's an optional peer dep.
        // if so, we can resolve to a special id that errors only when imported.
        if is_bare_import(id) && !is_builtin(id, &self.runtime) && !id.contains('\0') {
          if let Some(pkg_name) = get_npm_package_name(id) {
            let base_dir = get_base_dir(id, importer, dedupe).unwrap_or(&self.root);
            if base_dir != self.root {
              if let Some(package_json) =
                self.get_nearest_package_json_optional_peer_deps(importer.unwrap())
              {
                if package_json.optional_peer_dependencies.contains(pkg_name) {
                  return Ok(Some(HookResolveIdOutput {
                    id: format!("{OPTIONAL_PEER_DEP_ID}:{id}:{}", package_json.name).into(),
                    ..Default::default()
                  }));
                }
              }
            }
          }
        }
        Ok(None)
      }
      Err(oxc_resolver::ResolveError::Ignored(_)) => Ok(Some(HookResolveIdOutput {
        id: arcstr::literal!(BROWSER_EXTERNAL_ID),
        ..Default::default()
      })),
      Err(err) => Err(err.to_owned()),
    }
  }

  fn get_nearest_package_json_optional_peer_deps(
    &self,
    p: &str,
  ) -> Option<Arc<PackageJsonWithOptionalPeerDependencies>> {
    let specifier = Path::new(p).absolutize();
    let Ok(result) = self.inner.resolve(
      /* actually this can be anything, as the specifier is absolute path */ &self.root,
      specifier.to_str().unwrap_or(p),
    ) else {
      // Errors when p is a virtual module
      return None;
    };

    result
      .package_json()
      .map(|pj| self.package_json_cache.cached_package_json_optional_peer_dep(pj))
  }

  pub fn resolve_bare_import(
    &self,
    specifier: &str,
    importer: Option<&str>,
    external: bool,
    dedupe: &FxHashSet<String>,
  ) -> HookResolveIdReturn {
    let base_dir = get_base_dir(specifier, importer, dedupe).unwrap_or(&self.root);

    let oxc_resolved_result = self.resolve_raw(base_dir, specifier);
    let resolved = self.normalize_oxc_resolver_result(importer, dedupe, &oxc_resolved_result)?;
    if let Some(mut resolved) = resolved {
      if !external || !can_externalize_file(&resolved.id) {
        return Ok(Some(resolved));
      }

      let id = specifier;
      let mut resolved_id = id;
      if is_deep_import(id) && get_extension(id) != get_extension(&resolved.id) {
        if let Some(pkg_json) = oxc_resolved_result.unwrap().package_json() {
          let has_exports_field = pkg_json.raw_json().as_object().unwrap().get("exports").is_some();
          if !has_exports_field {
            // id date-fns/locale
            // resolve.id ...date-fns/esm/locale/index.js
            if let Some(index) = resolved.id.find(id) {
              resolved_id = &resolved.id[index..];
            }
          }
        }
      }
      resolved.id = resolved_id.into();
      resolved.external = Some(true.into());

      return Ok(Some(resolved));
    }
    Ok(None)
  }

  pub fn clear_cache(&self) {
    self.inner.clear_cache();
  }
}

fn get_base_dir<'a>(
  specifier: &'_ str,
  importer: Option<&'a str>,
  dedupe: &FxHashSet<String>,
) -> Option<&'a str> {
  if should_dedupe(specifier, dedupe) {
    return None;
  }

  if let Some(importer) = importer {
    let imp = Path::new(importer);
    if imp.is_absolute()
      && (
        // css processing appends `*` for importer
        importer.ends_with('*') || fs::exists(clean_url(importer)).unwrap_or(false)
      )
    {
      return Some(imp.parent().map(|i| i.to_str().unwrap()).unwrap_or(importer));
    }
  }
  None
}

fn should_dedupe(specifier: &str, dedupe: &FxHashSet<String>) -> bool {
  if dedupe.is_empty() {
    return false;
  }

  let pkg_id = get_npm_package_name(specifier).unwrap_or(clean_url(specifier));
  dedupe.contains(pkg_id)
}
