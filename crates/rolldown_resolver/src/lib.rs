// An wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

mod resolver;
mod resolver_options;

pub use crate::resolver::{ResolveRet, Resolver};
pub use crate::resolver_options::ResolveOptions;
