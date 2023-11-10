mod bundler;
mod error;
mod plugin;

use std::sync::Arc;

use rolldown_resolver::Resolver;

pub(crate) type SharedResolver<T> = Arc<Resolver<T>>;

pub use crate::{
  bundler::{
    bundle::asset::Asset,
    bundler::Bundler,
    options::{
      input_options::{InputItem, InputOptions},
      output_options::OutputOptions,
    },
  },
  plugin::{
    args::{
      HookBuildEndArgs, HookLoadArgs, HookResolveIdArgs, HookResolveIdArgsOptions,
      HookTransformArgs,
    },
    context::PluginContext,
    output::{HookLoadOutput, HookResolveIdOutput},
    plugin::{HookLoadReturn, HookNoopReturn, HookResolveIdReturn, HookTransformReturn, Plugin},
  },
};
