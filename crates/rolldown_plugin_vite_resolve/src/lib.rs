mod builtin;
mod external;
mod file_url;
mod package_json_cache;
mod resolver;
mod utils;
mod utils_filter;
mod vite_resolve_plugin;

pub use external::{ResolveOptionsExternal, ResolveOptionsNoExternal};
pub use vite_resolve_plugin::{
  FinalizeBareSpecifierCallback, FinalizeOtherSpecifiersCallback, ResolveIdOptionsScan,
  ViteResolveOptions, ViteResolvePlugin, ViteResolveResolveOptions,
};
