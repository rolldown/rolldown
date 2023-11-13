mod bundler;
mod error;
mod plugin;

use std::sync::Arc;

use rolldown_resolver::Resolver;

pub(crate) type SharedResolver<T> = Arc<Resolver<T>>;

pub use crate::{
  bundler::{
    bundle::output::{Output, OutputAsset, OutputChunk, RenderedModule},
    bundler::Bundler,
    options::{
      file_name_template::FileNameTemplate,
      input_options::{External, InputItem, InputOptions},
      output_options::{OutputFormat, OutputOptions},
    },
  },
  plugin::{
    args::{
      HookBuildEndArgs, HookLoadArgs, HookResolveIdArgs, HookResolveIdArgsOptions,
      HookTransformArgs,
    },
    context::PluginContext,
    output::{HookLoadOutput, HookResolveIdOutput},
    plugin::{
      BoxPlugin, HookLoadReturn, HookNoopReturn, HookResolveIdReturn, HookTransformReturn, Plugin,
    },
  },
};
