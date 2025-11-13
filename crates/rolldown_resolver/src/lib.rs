//! A wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

mod resolver;
mod resolver_config;

pub mod error;

pub use crate::resolver::{ResolveReturn, Resolver};

pub use oxc_resolver::{ResolveError, TsConfig};
pub use rolldown_common::bundler_options::ResolveOptions;
