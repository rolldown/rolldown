use std::{fs, path::Path, sync::Arc};

use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{HookResolveIdOutput, HookResolveIdReturn};

use crate::{
  package_json_cache::PackageJsonCache,
  package_json_peer::PackageJsonPeerDep,
  utils::{
    can_externalize_file, clean_url, get_extension, get_npm_package_name, is_bare_import,
    is_builtin, is_deep_import, normalize_path, BROWSER_EXTERNAL_ID, OPTIONAL_PEER_DEP_ID,
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

const ADDITIONAL_OPTIONS_FIELD_COUNT: u8 = 3;
const RESOLVER_COUNT: u8 = 2_u8.pow(ADDITIONAL_OPTIONS_FIELD_COUNT as u32);

const DEV_PROD_CONDITION: &str = "development|production";

#[derive(Debug)]
pub struct AdditionalOptions {
  is_require: bool,
  prefer_relative: bool,
  is_from_ts_importer: bool,
}

impl AdditionalOptions {
  pub fn new(is_require: bool, prefer_relative: bool, is_from_ts_importer: bool) -> Self {
    Self { is_require, prefer_relative, is_from_ts_importer }
  }

  fn as_bools(&self) -> [bool; ADDITIONAL_OPTIONS_FIELD_COUNT as usize] {
    [self.is_require, self.prefer_relative, self.is_from_ts_importer]
  }

  fn as_u8(&self) -> u8 {
    bools_to_u8(self.as_bools())
  }
}

impl From<[bool; RESOLVER_COUNT as usize]> for AdditionalOptions {
  fn from(value: [bool; RESOLVER_COUNT as usize]) -> Self {
    Self { is_require: value[0], prefer_relative: value[1], is_from_ts_importer: value[2] }
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
  // TODO(sapphi-red): tryPrefix support
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
        )
      })
      .collect::<Vec<_>>()
      .try_into()
      .unwrap();

    let external_resolver = Resolver::new(
      base_resolver.clone_with_options(get_resolve_options(
        &BaseOptions { is_production: false, conditions: external_conditions, ..*base_options },
        AdditionalOptions { is_from_ts_importer: false, is_require: false, prefer_relative: false },
      )),
      Arc::clone(&package_json_cache),
      runtime,
      base_options.root.to_owned(),
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

  let mut main_fields = base_options.main_fields.clone();
  main_fields.push("main".to_string());

  oxc_resolver::ResolveOptions {
    alias_fields: if base_options.main_fields.iter().any(|field| field == "browser") {
      vec![vec!["browser".to_string()]]
    } else {
      vec![]
    },
    condition_names: get_conditions(base_options, &additional_options),
    extensions,
    extension_alias: if additional_options.is_from_ts_importer {
      vec![
        (".js".to_string(), vec![".ts".to_string(), ".tsx".to_string(), ".js".to_string()]),
        (".jsx".to_string(), vec![".ts".to_string(), ".tsx".to_string(), ".jsx".to_string()]),
        (".mjs".to_string(), vec![".mts".to_string(), ".mjs".to_string()]),
        (".cjs".to_string(), vec![".cts".to_string(), ".cjs".to_string()]),
      ]
    } else {
      vec![]
    },
    main_fields,
    main_files: if !base_options.try_index {
      vec![]
    } else if let Some(try_prefix) = &base_options.try_prefix {
      vec![format!("{try_prefix}index"), "index".to_string()]
    } else {
      vec!["index".to_string()]
    },
    prefer_relative: additional_options.prefer_relative,
    // TODO(sapphi-red): maybe oxc-resolver can do the rootInRoot optimization
    // https://github.com/vitejs/vite/blob/a50ff6000bca46a6fe429f2c3a98c486ea5ebc8e/packages/vite/src/node/plugins/resolve.ts#L304
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
        if base_options.is_production {
          "production"
        } else {
          "development"
        }
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
  ret.iter_mut().enumerate().for_each(|(i, v)| *v = n & 1 << i != 0);
  ret
}

#[derive(Debug)]
pub struct Resolver {
  inner: oxc_resolver::Resolver,
  package_json_cache: Arc<PackageJsonCache>,
  package_json_peer_dep: PackageJsonPeerDep,
  runtime: String,
  root: String,
}

impl Resolver {
  pub fn new(
    inner: oxc_resolver::Resolver,
    package_json_cache: Arc<PackageJsonCache>,
    runtime: String,
    root: String,
  ) -> Self {
    Self {
      inner,
      package_json_cache,
      package_json_peer_dep: PackageJsonPeerDep::default(),
      runtime,
      root,
    }
  }

  pub fn resolve_raw<P: AsRef<Path>>(
    &self,
    directory: P,
    specifier: &str,
  ) -> Result<oxc_resolver::Resolution, oxc_resolver::ResolveError> {
    self.inner.resolve(directory, specifier)
  }

  pub fn normalize_oxc_resolver_result(
    &self,
    importer: Option<&str>,
    result: &Result<oxc_resolver::Resolution, oxc_resolver::ResolveError>,
  ) -> Result<Option<HookResolveIdOutput>, oxc_resolver::ResolveError> {
    match result {
      Ok(result) => {
        let raw_path = result.full_path().to_str().unwrap().to_string();
        let path = raw_path.strip_prefix("\\\\?\\").unwrap_or(&raw_path);
        let path = normalize_path(path);

        let side_effects = result
          .package_json()
          .and_then(|pkg_json| {
            self.package_json_cache.cached_package_json(pkg_json).check_side_effects_for(&raw_path)
          })
          .map(
            |side_effects| {
              if side_effects {
                HookSideEffects::True
              } else {
                HookSideEffects::False
              }
            },
          );
        Ok(Some(HookResolveIdOutput { id: path.into_owned(), side_effects, ..Default::default() }))
      }
      Err(oxc_resolver::ResolveError::NotFound(id)) => {
        // TODO(sapphi-red): maybe need to do the same thing for id mapped from browser field

        // if import can't be found, check if it's an optional peer dep.
        // if so, we can resolve to a special id that errors only when imported.
        if is_bare_import(id) && !is_builtin(id, &self.runtime) && !id.contains('\0') {
          if let Some(pkg_name) = get_npm_package_name(id) {
            let base_dir = get_base_dir(importer).unwrap_or(&self.root);
            if base_dir != self.root {
              if let Some(package_json) =
                self.package_json_peer_dep.get_nearest_package_json_optional_peer_deps(base_dir)
              {
                if package_json.optional_peer_dependencies.contains(pkg_name) {
                  return Ok(Some(HookResolveIdOutput {
                    id: format!("{OPTIONAL_PEER_DEP_ID}:{id}:{}", package_json.name),
                    ..Default::default()
                  }));
                }
              }
            }
          }
        }
        Ok(None)
      }
      Err(oxc_resolver::ResolveError::Ignored(_)) => {
        Ok(Some(HookResolveIdOutput { id: BROWSER_EXTERNAL_ID.to_string(), ..Default::default() }))
      }
      Err(err) => Err(err.to_owned()),
    }
  }

  pub fn resolve_bare_import(
    &self,
    specifier: &str,
    importer: Option<&str>,
    external: bool,
  ) -> HookResolveIdReturn {
    // TODO(sapphi-red): support `dedupe`
    let base_dir = get_base_dir(importer).unwrap_or(&self.root);

    let oxc_resolved_result = self.resolve_raw(base_dir, specifier);
    let resolved = self.normalize_oxc_resolver_result(importer, &oxc_resolved_result)?;
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
      resolved.id = resolved_id.to_string();
      resolved.external = Some(true);

      return Ok(Some(resolved));
    }
    Ok(None)
  }

  pub fn clear_cache(&self) {
    self.inner.clear_cache();
  }
}

fn get_base_dir(importer: Option<&str>) -> Option<&str> {
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
