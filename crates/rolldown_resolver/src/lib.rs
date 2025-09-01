// An wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

pub mod error;
mod resolver;

pub use crate::resolver::{ResolveReturn, Resolver};

pub use oxc_resolver::{ResolveError, TsConfig};
pub use rolldown_common::bundler_options::ResolveOptions;
