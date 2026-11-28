use std::{path::Path, sync::Arc};

use dashmap::DashMap;
use rolldown_std_utils::PathExt as _;
use rolldown_utils::{dashmap::FxDashMap, pattern_filter::StringOrRegex};
use rustc_hash::FxHashSet;
use sugar_path::SugarPath as _;

use crate::{
  builtin::BuiltinChecker,
  resolver::Resolver,
  utils::{can_externalize_file, get_npm_package_name, is_bare_import},
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
      Self(ResolveOptionsNoExternalInner::Vec(UtilsFilter::new(value)))
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
  pub legacy_inconsistent_cjs_interop: bool,
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

  /// In build the decision depends on the importer, because a package can have a different copy
  /// nested under the importer's own `node_modules`. The importer's directory is enough to tell
  /// the copies apart, so the cache is keyed by `(id, importer directory)`.
  fn cache_key(&self, id: &str, importer: Option<&str>) -> String {
    match importer.and_then(|importer| Path::new(importer).parent()) {
      Some(dir) if self.options.is_build => format!("{id}\0{}", dir.to_string_lossy()),
      _ => id.to_owned(),
    }
  }

  pub fn is_external(&self, id: &str, importer: Option<&str>) -> bool {
    let key = self.cache_key(id, importer);
    if let Some(cached) = self.processed_ids.get(&key) {
      return *cached;
    }

    let mut is_external = false;
    if !id.starts_with('.') && !Path::new(id).is_absolute() {
      is_external =
        self.builtin_checker.is_builtin(id) || self.is_configured_as_external(id, importer);
    }
    self.processed_ids.insert(key, is_external);

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
    let resolve_importer = if self.options.is_build { None } else { importer };

    let result = self.resolver.resolve_bare_import(
      id,
      resolve_importer,
      false,
      &self.options.dedupe,
      self.options.legacy_inconsistent_cjs_interop,
    );
    match result {
      Ok(result) => {
        let resolved = match result {
          Some(result) => result,
          _ => return false,
        };
        if !configured_as_external && !resolved.id.as_path().is_in_node_modules() {
          return false;
        }
        if !can_externalize_file(&resolved.id) {
          return false;
        }
        // The output bundle loads the copy that resolves from the root. If the importer would
        // resolve a different copy (a nested `node_modules`), externalizing would load the wrong one.
        if self.options.is_build
          && let Some(importer) = importer
          && let Ok(Some(from_importer)) = self.resolver.resolve_bare_import(
            id,
            Some(importer),
            false,
            &self.options.dedupe,
            self.options.legacy_inconsistent_cjs_interop,
          )
          && from_importer.id != resolved.id
        {
          return false;
        }
        true
      }
      _ => false,
    }
  }
}
