// An wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

mod resolver;
mod types;

pub use crate::{
  resolver::{ResolveRet, Resolver},
  types::{module_type::ModuleType, resolved_path::ResolvedPath},
};
pub use oxc_resolver::{Alias, AliasValue, EnforceExtension, ResolveOptions};
