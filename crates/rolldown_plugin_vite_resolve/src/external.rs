use std::{path::Path, sync::Arc};

use dashmap::DashMap;

use crate::{
  package_json_cache::PackageJsonCache,
  resolver::resolve_bare_import,
  utils::{
    can_externalize_file, get_npm_package_name, is_bare_import, is_builtin, is_in_node_modules,
  },
};

#[derive(Debug)]
pub enum ResolveOptionsExternal {
  True,
  Vec(Vec<String>),
}

impl ResolveOptionsExternal {
  pub fn is_external_explicitly(&self, id: &str) -> bool {
    let vec = match self {
      ResolveOptionsExternal::Vec(vec) => vec,
      _ => return false,
    };
    return vec.iter().any(|v| v == id);
  }
}

#[derive(Debug)]
pub enum ResolveOptionsNoExternal {
  True,
  // TODO: support RegExp
  Vec(Vec<String>),
}

impl ResolveOptionsNoExternal {
  pub fn is_no_external(&self, id: &str) -> bool {
    match self {
      ResolveOptionsNoExternal::True => true,
      ResolveOptionsNoExternal::Vec(vec) => {
        if vec.is_empty() {
          false
        } else {
          // TODO: implement the same logic with createFilter
          vec.iter().any(|v| v == id)
        }
      }
    }
  }
}

#[derive(Debug)]
pub struct ExternalDeciderOptions {
  pub external: ResolveOptionsExternal,
  pub no_external: ResolveOptionsNoExternal,
  pub root: String,
}

#[derive(Debug)]
pub struct ExternalDecider {
  options: ExternalDeciderOptions,
  runtime: String,
  resolver: oxc_resolver::Resolver,
  package_json_cache: Arc<PackageJsonCache>,
  processed_ids: DashMap<String, bool>,
}

impl ExternalDecider {
  pub fn new(
    options: ExternalDeciderOptions,
    runtime: String,
    resolver: oxc_resolver::Resolver,
    package_json_cache: Arc<PackageJsonCache>,
  ) -> Self {
    Self { options, runtime, resolver, package_json_cache, processed_ids: DashMap::default() }
  }

  pub fn is_external(&self, id: &str, importer: Option<&str>) -> bool {
    if let Some(cached) = self.processed_ids.get(id) {
      return *cached;
    }

    let mut is_external = false;
    if !id.starts_with('.') && !Path::new(id).is_absolute() {
      is_external = is_builtin(id, &self.runtime) || self.is_configured_as_external(id, importer);
    }
    self.processed_ids.insert(id.to_owned(), is_external);

    is_external
  }

  fn is_configured_as_external(&self, id: &str, importer: Option<&str>) -> bool {
    if self.options.external.is_external_explicitly(id) {
      return true;
    }
    let pkg_name = get_npm_package_name(id);
    let pkg_name = match pkg_name {
      Some(pkg_name) => pkg_name,
      None => return self.is_externalizable(id, importer, false),
    };
    if self.options.external.is_external_explicitly(pkg_name) {
      return self.is_externalizable(id, importer, true);
    }
    if self.options.no_external.is_no_external(id) {
      return false;
    }
    self.is_externalizable(
      id,
      importer,
      matches!(self.options.external, ResolveOptionsExternal::True),
    )
  }

  fn is_externalizable(
    &self,
    id: &str,
    importer: Option<&str>,
    configured_as_external: bool,
  ) -> bool {
    if !is_bare_import(id) || id.contains('\0') {
      return false;
    }

    let result = resolve_bare_import(
      id,
      importer,
      &self.resolver,
      &self.package_json_cache,
      &self.options.root,
      false,
    );
    if let Ok(result) = result {
      let resolved = match result {
        Some(result) => result,
        _ => return false,
      };
      if !configured_as_external && !is_in_node_modules(&resolved.id) {
        return false;
      }
      can_externalize_file(&resolved.id)
    } else {
      false
    }
  }
}
