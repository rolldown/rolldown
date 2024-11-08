use std::{path::PathBuf, sync::Arc, vec};

use dashmap::DashMap;
use rolldown_common::PackageJson;

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
  resolvers: [oxc_resolver::Resolver; RESOLVER_COUNT as usize],
  package_json_cache: DashMap<PathBuf, Arc<PackageJson>>,
}

impl Resolver {
  pub fn new(base_options: &BaseOptions) -> Self {
    let base_resolver = oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions::default());

    let resolvers = (0..RESOLVER_COUNT)
      .map(|v| {
        base_resolver.clone_with_options(get_resolve_options(base_options, u8_to_bools(v).into()))
      })
      .collect::<Vec<oxc_resolver::Resolver>>()
      .try_into()
      .unwrap();

    Self { resolvers, package_json_cache: DashMap::default() }
  }

  pub fn get(&self, additional_options: AdditionalOptions) -> &oxc_resolver::Resolver {
    &self.resolvers[additional_options.as_u8() as usize]
  }

  pub fn cached_package_json(&self, oxc_pkg_json: &oxc_resolver::PackageJson) -> Arc<PackageJson> {
    if let Some(v) = self.package_json_cache.get(&oxc_pkg_json.realpath) {
      Arc::clone(v.value())
    } else {
      let pkg_json = Arc::new(
        PackageJson::new(oxc_pkg_json.path.clone())
          .with_type(oxc_pkg_json.r#type.as_ref())
          .with_side_effects(oxc_pkg_json.side_effects.as_ref()),
      );
      self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
      pkg_json
    }
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
