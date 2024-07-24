use crate::types::generator::GenerateContext;
use crate::utils::chunk::collect_render_chunk_imports::{
  collect_render_chunk_imports, RenderImportDeclarationSpecifier, RenderImportSpecifier,
  RenderImportStmt,
};
use arcstr::ArcStr;
use rolldown_common::OutputFormat;
use rolldown_utils::ecma_script::legitimize_identifier_name;

/// Render chunk imports and return the import statements and the external imports.
pub fn render_chunk_imports(ctx: &GenerateContext<'_>) -> (String, Vec<String>) {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);

  let mut s = String::new();
  let externals: Vec<String> = render_import_stmts
    .iter()
    .filter_map(|stmt| {
      let require_path_str = if matches!(ctx.options.format, OutputFormat::Cjs) {
        format!("require(\"{}\")", &stmt.path)
      } else {
        stmt.path.to_string()
      };
      let require_path_str = require_path_str.as_str();
      match &stmt.specifiers {
        RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
          handle_import_specifier(ctx, specifiers, stmt, require_path_str, &mut s)
        }
        RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
          Some(handle_import_star_specifier(ctx, alias, stmt, require_path_str, &mut s))
        }
      }
    })
    .collect();
  (s, externals)
}

fn handle_import_empty_specifier(
  ctx: &GenerateContext<'_>,
  require_path_str: &str,
  s: &mut String,
) -> String {
  match ctx.options.format {
    OutputFormat::Esm => {
      s.push_str(&format!("import \"{require_path_str}\";\n",));
    }
    OutputFormat::Cjs => {
      s.push_str(&format!("{require_path_str};\n"));
    }
    OutputFormat::Iife => {}
    OutputFormat::App => {
      unreachable!("App format is not supported for import specifiers")
    }
  }
  require_path_str.to_string()
}

fn handle_import_specifier(
  ctx: &GenerateContext<'_>,
  specifiers: &[RenderImportSpecifier],
  stmt: &RenderImportStmt,
  require_path_str: &str,
  s: &mut String,
) -> Option<String> {
  if specifiers.is_empty() {
    handle_import_empty_specifier(ctx, require_path_str, s);
    None
  } else {
    let specifiers = specifiers
      .iter()
      .map(|specifier| {
        if let Some(alias) = &specifier.alias {
          match ctx.options.format {
            OutputFormat::Esm => format!("{} as {alias}", specifier.imported),
            OutputFormat::Cjs | OutputFormat::Iife => format!("{}: {alias}", specifier.imported),
            OutputFormat::App => unreachable!("App format is not supported for import specifiers"),
          }
        } else {
          specifier.imported.to_string()
        }
      })
      .collect::<Vec<_>>();
    let syntax = match ctx.options.format {
      OutputFormat::Esm => {
        format!("import {{ {} }} from \"{require_path_str}\";\n", specifiers.join(", "))
      }
      OutputFormat::Cjs | OutputFormat::Iife => handle_umd_import_syntax(
        ctx,
        stmt,
        &ctx.options.format,
        &require_path_str.to_string().into(),
        format!("{{ {} }}", specifiers.join(", ")).as_str(),
      ),
      OutputFormat::App => unreachable!("App format is not supported for import specifiers"),
    };
    s.push_str(syntax.as_str());
    Some(require_path_str.to_string())
  }
}

fn handle_import_star_specifier(
  ctx: &GenerateContext<'_>,
  alias: &&str,
  stmt: &RenderImportStmt,
  require_path_str: &str,
  s: &mut String,
) -> String {
  let syntax = match ctx.options.format {
    OutputFormat::Esm => format!("import * as {alias} from \"{require_path_str}\";\n",),
    OutputFormat::Cjs | OutputFormat::Iife => handle_umd_import_syntax(
      ctx,
      stmt,
      &ctx.options.format,
      &require_path_str.to_string().into(),
      alias,
    ),
    OutputFormat::App => unreachable!("App format is not supported for import specifiers"),
  };
  s.push_str(syntax.as_str());
  require_path_str.to_string()
}

/// Handle UMD-related import syntax, including CJS, IIFE, and (planing) AMD, UMD.
fn handle_umd_import_syntax(
  ctx: &GenerateContext<'_>,
  stmt: &RenderImportStmt,
  format: &OutputFormat,
  require_path_str: &ArcStr,
  assignee: &str,
) -> String {
  format!(
    "const {assignee} = {};\n",
    if stmt.is_external && matches!(format, OutputFormat::Cjs) {
      let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
        .link_output
        .symbols
        .par_canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

      format!("{to_esm_fn_name}({require_path_str})")
    } else if matches!(format, OutputFormat::Cjs) {
      require_path_str.to_string()
    } else {
      legitimize_identifier_name(require_path_str).to_string()
    }
  )
}
