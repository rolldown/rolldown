use std::{path::Path, sync::Arc};

use dashmap::DashMap;
use rolldown_utils::{dashmap::FxDashMap, pattern_filter::StringOrRegex};
use rustc_hash::FxHashSet;

use crate::{
  builtin::BuiltinChecker,
  resolver::Resolver,
  utils::{can_externalize_file, get_npm_package_name, is_bare_import, is_in_node_modules},
  utils_filter::UtilsFilter,
};

#[derive(Debug, Clone)]
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
    vec.iter().any(|v| v == id)
  }
}

#[derive(Debug)]
pub struct ResolveOptionsNoExternal(ResolveOptionsNoExternalInner);

impl ResolveOptionsNoExternal {
  pub fn new_true() -> Self {
    Self(ResolveOptionsNoExternalInner::True)
  }

  pub fn new_vec(value: Vec<StringOrRegex>) -> Self {
    if value.is_empty() {
      Self(ResolveOptionsNoExternalInner::Empty)
    } else {
      Self(ResolveOptionsNoExternalInner::Vec(UtilsFilter::new(vec![], value)))
    }
  }

  pub fn is_true(&self) -> bool {
    matches!(self.0, ResolveOptionsNoExternalInner::True)
  }

  pub fn is_no_external(&self, id: &str) -> bool {
    match &self.0 {
      ResolveOptionsNoExternalInner::True => true,
      ResolveOptionsNoExternalInner::Vec(filter) => !filter.is_match(id),
      ResolveOptionsNoExternalInner::Empty => false,
    }
  }
}

#[derive(Debug)]
enum ResolveOptionsNoExternalInner {
  True,
  Vec(UtilsFilter),
  Empty,
}

#[derive(Debug)]
pub struct ExternalDeciderOptions {
  pub external: ResolveOptionsExternal,
  pub no_external: Arc<ResolveOptionsNoExternal>,
  pub dedupe: Arc<FxHashSet<String>>,
  pub is_build: bool,
}

#[derive(Debug)]
pub struct ExternalDecider {
  options: ExternalDeciderOptions,
  resolver: Arc<Resolver>,
  builtin_checker: Arc<BuiltinChecker>,
  processed_ids: FxDashMap<String, bool>,
}

impl ExternalDecider {
  pub fn new(
    options: ExternalDeciderOptions,
    resolver: Arc<Resolver>,
    builtin_checker: Arc<BuiltinChecker>,
  ) -> Self {
    Self { options, resolver, builtin_checker, processed_ids: DashMap::default() }
  }

  pub fn is_external(&self, id: &str, importer: Option<&str>) -> bool {
    if let Some(cached) = self.processed_ids.get(id) {
      return *cached;
    }

    let mut is_external = false;
    if !id.starts_with('.') && !Path::new(id).is_absolute() {
      is_external =
        self.builtin_checker.is_builtin(id) || self.is_configured_as_external(id, importer);
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
    if self.options.no_external.is_no_external(pkg_name) {
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

    // Skip passing importer in build to avoid externalizing non-hoisted dependencies
    // unresolvable from root (which would be unresolvable from output bundles also)
    let importer = if self.options.is_build { None } else { importer };

    let result = self.resolver.resolve_bare_import(id, importer, false, &self.options.dedupe);
    match result {
      Ok(result) => {
        let resolved = match result {
          Some(result) => result,
          _ => return false,
        };
        if !configured_as_external && !is_in_node_modules(&resolved.id) {
          return false;
        }
        can_externalize_file(&resolved.id)
      }
      _ => false,
    }
  }
}
