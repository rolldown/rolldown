// An wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

mod resolver;

pub use crate::resolver::{ResolveRet, Resolver};
pub use oxc_resolver::{Alias, AliasValue, EnforceExtension, ResolveOptions};
