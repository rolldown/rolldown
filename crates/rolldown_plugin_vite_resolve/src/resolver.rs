use std::{
  ffi::OsString,
  fs,
  path::{self, Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{ResolveOptions, TsconfigOptions, TsconfigReferences};
use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::HookResolveIdOutput;
use rolldown_utils::{dashmap::FxDashMap, url::clean_url};
use rustc_hash::FxHashSet;
use sugar_path::SugarPath;

use crate::{
  builtin::BuiltinChecker,
  package_json_cache::{PackageJsonCache, PackageJsonWithOptionalPeerDependencies},
  utils::{
    BROWSER_EXTERNAL_ID, OPTIONAL_PEER_DEP_ID, get_npm_package_name, is_bare_import, normalize_path,
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
  pub tsconfig_paths: bool,
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
  tsconfig_resolver: Arc<TsconfigResolver>,
}

impl Resolvers {
  pub fn new(
    base_options: &BaseOptions,
    external_conditions: &Vec<String>,
    builtin_checker: Arc<BuiltinChecker>,
  ) -> Self {
    let package_json_cache = Arc::new(PackageJsonCache::default());

    let base_resolver = oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions::default());

    let tsconfig_resolver =
      Arc::new(TsconfigResolver::new(base_resolver.clone_with_options(ResolveOptions::default())));

    let resolvers = (0..RESOLVER_COUNT)
      .map(|v| {
        Resolver::new(
          base_resolver.clone_with_options(get_resolve_options(base_options, v.into())),
          base_options.tsconfig_paths.then(|| Arc::clone(&tsconfig_resolver)),
          Arc::clone(&builtin_checker),
          Arc::clone(&package_json_cache),
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
      base_options.tsconfig_paths.then(|| Arc::clone(&tsconfig_resolver)),
      Arc::clone(&builtin_checker),
      Arc::clone(&package_json_cache),
      base_options.root.to_owned(),
      base_options.try_prefix.to_owned(),
    );

    Self { resolvers, external_resolver: Arc::new(external_resolver), tsconfig_resolver }
  }

  pub fn get(&self, additional_options: AdditionalOptions) -> &Resolver {
    &self.resolvers[additional_options.as_u8() as usize]
  }

  pub fn clear_cache(&self) {
    self.resolvers.iter().for_each(|v| v.clear_cache());
    self.external_resolver.clear_cache();
    self.tsconfig_resolver.clear_cache();
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
    // This is not part of the spec, but required to align with rollup based vite.
    allow_package_exports_in_directory_resolve: true,
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
  tsconfig_resolver: Option<Arc<TsconfigResolver>>,
  built_in_checker: Arc<BuiltinChecker>,
  package_json_cache: Arc<PackageJsonCache>,
  root: String,
  try_prefix: Option<String>,
}

impl Resolver {
  pub fn new(
    inner: oxc_resolver::Resolver,
    tsconfig_resolver: Option<Arc<TsconfigResolver>>,
    built_in_checker: Arc<BuiltinChecker>,
    package_json_cache: Arc<PackageJsonCache>,
    root: String,
    try_prefix: Option<String>,
  ) -> Self {
    Self { inner, tsconfig_resolver, built_in_checker, package_json_cache, root, try_prefix }
  }

  pub fn resolve_raw<P: AsRef<Path>>(
    &self,
    directory: P,
    specifier: &str,
  ) -> Result<oxc_resolver::Resolution, oxc_resolver::ResolveError> {
    let inner_resolver = if let Some(tsconfig) =
      self.tsconfig_resolver.as_ref().and_then(|r| r.load_nearest_tsconfig(directory.as_ref()))
    {
      &self.inner.clone_with_options(ResolveOptions {
        tsconfig: Some(TsconfigOptions {
          config_file: tsconfig,
          references: TsconfigReferences::Disabled,
        }),
        ..self.inner.options().clone()
      })
    } else {
      &self.inner
    };

    let Some(try_prefix) = &self.try_prefix else {
      return inner_resolver.resolve(directory, specifier);
    };

    let mut path = Path::new(specifier).components();
    let Some(path::Component::Normal(filename)) = path.next_back() else {
      return inner_resolver.resolve(directory, specifier);
    };

    let mut filename_with_prefix = OsString::with_capacity(try_prefix.len() + filename.len());
    filename_with_prefix.push(try_prefix);
    filename_with_prefix.push(filename);

    let path_with_prefix = path.as_path().join(filename_with_prefix);
    let Some(path_with_prefix) = path_with_prefix.to_str() else {
      return inner_resolver.resolve(directory, specifier);
    };

    let result_with_prefix = inner_resolver.resolve(directory.as_ref(), path_with_prefix);
    match result_with_prefix {
      Err(
        oxc_resolver::ResolveError::NotFound(_)
        | oxc_resolver::ResolveError::ExtensionAlias(_, _, _),
      ) => inner_resolver.resolve(directory, specifier),
      _ => result_with_prefix,
    }
  }

  pub fn normalize_oxc_resolver_result(
    &self,
    importer: Option<&str>,
    dedupe: &FxHashSet<String>,
    result: &Result<oxc_resolver::Resolution, oxc_resolver::ResolveError>,
  ) -> Result<Option<HookResolveIdOutput>, oxc_resolver::ResolveError> {
    match result {
      Ok(result) => {
        let raw_path = result.full_path().to_str().unwrap().to_string();
        let path = normalize_path(&raw_path);

        let side_effects = result
          .package_json()
          .and_then(|pkg_json| {
            // the glob expr is based on parent path of package.json, which is package path
            // so we should use the relative path of the module to package path
            let module_path_relative_to_package =
              raw_path.as_path().relative(pkg_json.path.parent()?);
            self
              .package_json_cache
              .cached_package_json_side_effects(pkg_json)
              .check_side_effects_for(module_path_relative_to_package.to_str()?)
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
        if is_bare_import(id) && !self.built_in_checker.is_builtin(id) && !id.contains('\0') {
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

#[derive(Debug)]
pub struct TsconfigResolver {
  inner: oxc_resolver::Resolver,
  tsconfig_dir_existence: FxDashMap<PathBuf, bool>,
}

impl TsconfigResolver {
  pub fn new(inner: oxc_resolver::Resolver) -> Self {
    Self { inner, tsconfig_dir_existence: FxDashMap::default() }
  }

  pub fn load_nearest_tsconfig(&self, path: &Path) -> Option<PathBuf> {
    // don't load tsconfig for paths in node_modules like esbuild does
    if is_in_node_modules(path) {
      return None;
    }

    if let Some(tsconfig) = self.find_nearest_tsconfig(path) {
      // TODO: need to handle references and include/exclude
      self.inner.resolve_tsconfig(&tsconfig).ok().map(|_| tsconfig)
    } else {
      None
    }
  }

  fn find_nearest_tsconfig(&self, path: &Path) -> Option<PathBuf> {
    // skip virtual IDs (e.g. `virtual:something`)
    if !path.is_absolute() {
      return None;
    }

    let mut dir = path.to_path_buf();

    loop {
      if let Some(r) = self.tsconfig_dir_existence.get(&dir) {
        if *r.value() {
          return Some(dir.join("tsconfig.json"));
        }
      } else {
        let tsconfig_json = dir.join("tsconfig.json");
        if tsconfig_json.exists() {
          self.tsconfig_dir_existence.insert(dir.clone(), true);
          return Some(tsconfig_json);
        } else {
          self.tsconfig_dir_existence.insert(dir.clone(), false);
        }
      }

      let Some(parent) = dir.parent() else { break };
      dir = parent.to_path_buf();
    }
    None
  }

  pub fn clear_cache(&self) {
    self.inner.clear_cache();
    self.tsconfig_dir_existence.clear();
  }
}

fn is_in_node_modules(id: &Path) -> bool {
  id.components().any(|comp| comp.as_os_str() == "node_modules")
}
