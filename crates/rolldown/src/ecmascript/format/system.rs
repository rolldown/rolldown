// See openspec change: add-system-output-format
//!
//! Renders a SystemJS `System.register(name?, deps, factory)` chunk.
//!
//! Structure emitted:
//! ```text
//! [banner]
//! System.register(['dep1', 'dep2'], (function ([exports[, module]]) {
//!   'use strict';
//!   var dep_binding1, dep_binding2;
//!   [intro]
//!   return {
//!     setters: [setter1, setter2],
//!     execute: (function () {
//!       [module sources]
//!       [outro]
//!     })
//!   };
//! }));
//! [footer]
//! ```

use rolldown_common::{AddonRenderContext, OutputFormat};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::concat_string;

use crate::{
  ecmascript::{
    ecma_generator::{RenderedModuleSource, RenderedModuleSources},
    format::utils::render_chunk_directives,
  },
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::get_chunk_export_names_with_ctx,
};

/// Returns `true` if the chunk uses any dynamic import or `import.meta` — meaning
/// the SystemJS `module` parameter must be included in the factory signature.
///
/// This is a stub that always returns `false` until group 5 implements proper tracking.
fn chunk_uses_module_context(_ctx: &GenerateContext<'_>) -> bool {
  // TODO (task 5.1): inspect chunk modules for dynamic imports / import.meta usage
  false
}

/// Render a SystemJS `System.register(deps, factory)` chunk.
#[expect(clippy::needless_pass_by_value)]
pub fn render_system<'code>(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
) -> SourceJoiner<'code> {
  debug_assert!(matches!(ctx.options.format, OutputFormat::System));

  let mut source_joiner = SourceJoiner::default();
  let AddonRenderContext { hashbang: _, banner, intro, outro, footer, directives } =
    addon_render_context;

  // Task 2.6: banner before System.register
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  // Task 2.3: compute factory parameters
  let export_names = get_chunk_export_names_with_ctx(ctx);
  let has_exports = !export_names.is_empty();
  let uses_module_context = chunk_uses_module_context(ctx);

  let factory_params = match (has_exports, uses_module_context) {
    (true, true) => "exports, module",
    (true, false) => "exports",
    (false, true) => "module",
    (false, false) => "",
  };

  // Task 4.1: deps array (stub — populated in group 4)
  let deps_array = build_deps_array(ctx);

  // Task 2.1 / 2.2: open System.register wrapper
  // Named registration: when output.name is set, emit it as first argument
  // Note: single-quoted strings get converted to double quotes by the DCE (oxc minifier codegen)
  // pass that always runs. Use double quotes here to match the final output.
  let name_arg = ctx
    .options
    .name
    .as_deref()
    .filter(|n| !n.is_empty())
    .map(|n| concat_string!("\"", n, "\", "))
    .unwrap_or_default();

  source_joiner.append_source(concat_string!(
    "System.register(",
    name_arg,
    "[",
    deps_array,
    "], (function (",
    factory_params,
    ") {\n"
  ));

  // Task 2.4: 'use strict' inside factory
  // The ecma_generator preprocesses `directives` according to StrictMode before passing them:
  // - StrictMode::Always adds '"use strict"' if not already present
  // - StrictMode::Never removes 'use strict' directives
  // - StrictMode::Auto leaves them as-is from source
  // We emit the directives inside the factory body.
  if !directives.is_empty() {
    let rendered = render_chunk_directives(directives.iter());
    if !rendered.is_empty() {
      source_joiner.append_source(rendered);
    }
  }

  // Task 4.2: hoisted var declarations for imported bindings (stub — populated in group 4)
  let hoisted_var_decls = build_hoisted_var_decls(ctx);
  if !hoisted_var_decls.is_empty() {
    source_joiner.append_source(hoisted_var_decls);
  }

  // Task 2.6: intro inside factory before module sources (but after var decls)
  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // Task 4.3: setters array (stub — populated in group 4)
  let setters = build_setters(ctx);

  // Task 2.5: open the return object and execute function
  source_joiner.append_source(concat_string!(
    "  return {\n",
    "    setters: [",
    setters,
    "],\n",
    "    execute: (function () {\n"
  ));

  // Module sources go INSIDE the execute function body
  for RenderedModuleSource { sources, .. } in module_sources.iter() {
    if let Some(emitted_sources) = sources {
      for source in emitted_sources.iter() {
        source_joiner.append_source(source);
      }
    }
  }

  // Task 2.6: outro after module sources, inside execute
  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  // Close execute function and return object, then factory and System.register
  source_joiner.append_source("    })\n  };\n}));\n");

  // Task 2.6: footer after closing wrapper
  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}

/// Build the deps string array (comma-separated quoted paths).
/// Stub for now — populated fully in group 4 (task 4.1).
fn build_deps_array(_ctx: &GenerateContext<'_>) -> String {
  // TODO (task 4.1): iterate chunk.imports_from_other_chunks and external imports
  String::new()
}

/// Build hoisted `var` declarations for all imported bindings.
/// Stub for now — populated fully in group 4 (task 4.2).
fn build_hoisted_var_decls(_ctx: &GenerateContext<'_>) -> String {
  // TODO (task 4.2): emit `var binding;` for every imported symbol from deps
  String::new()
}

/// Build the `setters` array entries.
/// Stub for now — populated fully in group 4 (task 4.3).
fn build_setters(_ctx: &GenerateContext<'_>) -> String {
  // TODO (task 4.3): generate real setter functions
  String::new()
}
