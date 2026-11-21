use oxc::ast::ast::{Program, Statement};

/// Facade symbols that a module can get and that its top-level statements do not show: those of
/// `import()`, `require()`, `new URL()`, `exports.x =`, and the HMR `<name>_hot`, and those that
/// the link stage adds (for example `require_<name>` and `init_<name>`).
const UNCOUNTED_FACADE_SYMBOLS: u32 = 1;

/// The number of facade symbols to reserve room for when you build the `Scoping` that the
/// scanner uses.
///
/// The scanner and the link stage add facade symbols to the `Scoping` after the semantic build,
/// and the build sizes the symbol storage of the `Scoping` for the symbols that it creates. So the
/// first facade symbol grows every symbol array of the module. A grown array in the `Scoping`'s
/// arena keeps its old buffer until the arena is dropped, and a large growth makes the arena
/// allocate a new chunk.
///
/// This function counts the facade symbols of the top-level statements, in the same way as the
/// scanner creates them:
/// - two for every module, `<name>_default` and `<name>_exports` (`AstScanner::new`);
/// - one for each `import`, `export ... from`, and `export *` statement (`add_import_record`);
/// - one for each specifier of `export { ... } from` (`add_re_export`) and for
///   `export * as ns from` (`add_star_re_export`).
///
/// It adds [`UNCOUNTED_FACADE_SYMBOLS`] for the others. It does not visit nested code, so it
/// costs one pass over the top-level statements.
pub fn facade_symbol_reserve(program: &Program<'_>) -> u32 {
  let mut count = 2 + UNCOUNTED_FACADE_SYMBOLS;
  for stmt in &program.body {
    count += match stmt {
      Statement::ImportDeclaration(_) => 1,
      #[expect(clippy::cast_possible_truncation)]
      Statement::ExportFromDeclaration(decl) => 1 + decl.specifiers.len() as u32,
      Statement::ExportAllDeclaration(decl) => 1 + u32::from(decl.exported.is_some()),
      _ => 0,
    };
  }
  count
}
