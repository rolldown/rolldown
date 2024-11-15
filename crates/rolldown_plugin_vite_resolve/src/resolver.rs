use std::{fs, path::Path};

use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{HookResolveIdOutput, HookResolveIdReturn};

use crate::{
  package_json_cache::PackageJsonCache,
  utils::{
    can_externalize_file, clean_url, get_extension, is_bare_import, is_builtin, is_deep_import, normalize_path, BROWSER_EXTERNAL_ID
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

#[derive(Debug)]
pub struct Resolver {
  base_resolver: oxc_resolver::Resolver,
  resolvers: [oxc_resolver::Resolver; RESOLVER_COUNT as usize],
}

impl Resolver {
  // TODO(sapphi-red): tryPrefix support
  pub fn new(base_options: &BaseOptions) -> Self {
    let base_resolver = oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions::default());

    let resolvers = (0..RESOLVER_COUNT)
      .map(|v| {
        base_resolver.clone_with_options(get_resolve_options(base_options, u8_to_bools(v).into()))
      })
      .collect::<Vec<oxc_resolver::Resolver>>()
      .try_into()
      .unwrap();

    Self { base_resolver, resolvers }
  }

  pub fn get(&self, additional_options: AdditionalOptions) -> &oxc_resolver::Resolver {
    &self.resolvers[additional_options.as_u8() as usize]
  }

  pub fn get_for_external(
    &self,
    base_options: &BaseOptions,
    external_conditions: &Vec<String>,
  ) -> oxc_resolver::Resolver {
    self.base_resolver.clone_with_options(get_resolve_options(
      &BaseOptions { is_production: false, conditions: external_conditions, ..*base_options },
      AdditionalOptions { is_from_ts_importer: false, is_require: false, prefer_relative: false },
    ))
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

pub fn normalize_oxc_resolver_result(
  package_json_cache: &PackageJsonCache,
  runtime: &str,
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
          package_json_cache.cached_package_json(pkg_json).check_side_effects_for(&raw_path)
        })
        .map(
          |side_effects| if side_effects { HookSideEffects::True } else { HookSideEffects::False },
        );
      Ok(Some(HookResolveIdOutput { id: path.into_owned(), side_effects, ..Default::default() }))
    }
    Err(oxc_resolver::ResolveError::NotFound(id)) => {
      // TODO(sapphi-red): maybe need to do the same thing for id mapped from browser field

      // if import can't be found, check if it's an optional peer dep.
      // if so, we can resolve to a special id that errors only when imported.
      if is_bare_import(id) && !is_builtin(id, runtime) && !id.contains('\0') {
        // TODO(sapphi-red): handle missing peerDep
        // https://github.com/vitejs/vite/blob/58f1df3288b0f9584bb413dd34b8d65671258f6f/packages/vite/src/node/plugins/resolve.ts#L728-L752
        return Ok(None)
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
  specifier: &str,
  importer: Option<&str>,
  resolver: &oxc_resolver::Resolver,
  package_json_cache: &PackageJsonCache,
  runtime: &str,
  root: &str,
  external: bool,
) -> HookResolveIdReturn {
  // TODO(sapphi-red): support `dedupe`
  let base_dir = if let Some(importer) = importer {
    let imp = Path::new(importer);
    if imp.is_absolute()
      && (
        // css processing appends `*` for importer
        importer.ends_with('*') ||
        fs::exists(clean_url(importer)).unwrap_or(false)
      )
    {
      imp.parent().map(|i| i.to_str().unwrap()).unwrap_or(importer)
    } else {
      root
    }
  } else {
    root
  };

  let oxc_resolved_result = resolver.resolve(base_dir, specifier);
  let resolved = normalize_oxc_resolver_result(package_json_cache, runtime, &oxc_resolved_result)?;
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
