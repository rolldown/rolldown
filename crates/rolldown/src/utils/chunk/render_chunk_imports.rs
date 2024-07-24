use crate::types::generator::GenerateContext;
use crate::utils::chunk::collect_render_chunk_imports::{
  collect_render_chunk_imports, RenderImportDeclarationSpecifier,
};
use rolldown_common::OutputFormat;
use rolldown_utils::ecma_script::legitimize_identifier_name;

#[allow(clippy::too_many_lines)]
pub fn render_chunk_imports(ctx: &GenerateContext<'_>) -> (String, Vec<String>) {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);
  let format = &ctx.options.format;

  let mut s = String::new();
  let externals: Vec<String> = render_import_stmts
    .iter()
    .filter_map(|stmt| {
      let require_path_str = &stmt.path;
      match &stmt.specifiers {
        RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
          if specifiers.is_empty() {
            match format {
              OutputFormat::Esm => {
                s.push_str(&format!("import \"{require_path_str}\";\n",));
                None
              }
              OutputFormat::Cjs => {
                s.push_str(&format!("{require_path_str};\n"));
                None
              }
              OutputFormat::Iife => None,
              OutputFormat::App => {
                unreachable!("App format is not supported for import specifiers")
              }
            }
          } else {
            let specifiers = specifiers
              .iter()
              .map(|specifier| {
                if let Some(alias) = &specifier.alias {
                  match format {
                    OutputFormat::Esm => format!("{} as {alias}", specifier.imported),
                    OutputFormat::Cjs | OutputFormat::Iife => {
                      format!("{}: {alias}", specifier.imported)
                    }
                    OutputFormat::App => {
                      unreachable!("App format is not supported for import specifiers")
                    }
                  }
                } else {
                  specifier.imported.to_string()
                }
              })
              .collect::<Vec<_>>();
            let syntax = match format {
                OutputFormat::Esm => {
                    &format!("import {{ {} }} from \"{require_path_str}\";\n", specifiers.join(", "))
                }
                OutputFormat::Cjs | OutputFormat::Iife => &format!(
                    "const {{ {} }} = {};\n",
                    specifiers.join(", "),
                    if stmt.is_external || matches!(format, OutputFormat::Iife) {
                        let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
                            .link_output
                            .symbols
                            .par_canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

                        format!("{to_esm_fn_name}({require_path_str})")
                    } else {
                        require_path_str.to_string()
                    }
                ),
                OutputFormat::App => {
                    unreachable!("App format is not supported for import specifiers")
                }
            };
            s.push_str(syntax);
            Some(require_path_str.to_string())
          }
        }
        RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
          let syntax = match format {
            OutputFormat::Esm => format!("import * as {alias} from \"{require_path_str}\";\n",),
            OutputFormat::Cjs => format!(
              "const {alias} = {};\n",
              if stmt.is_external {
                let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
                  .link_output
                  .symbols
                  .par_canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

                format!("{to_esm_fn_name}({require_path_str})")
              } else {
                require_path_str.to_string()
              }
            ),
            OutputFormat::Iife => format!(
              "const {alias} = {};\n",
              legitimize_identifier_name(&require_path_str.to_string()).to_string()
            ),
            OutputFormat::App => unreachable!("App format is not supported for import specifiers"),
          };
          s.push_str(syntax.as_str());

          Some(require_path_str.to_string())
        }
      }
    })
    .collect();
  (s, externals)
}
