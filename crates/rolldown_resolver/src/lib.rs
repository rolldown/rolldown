// An wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

mod resolver;
mod types;

pub use crate::{
  resolver::{ResolveRet, Resolver},
  types::resolve_options::ResolveOptions,
};
