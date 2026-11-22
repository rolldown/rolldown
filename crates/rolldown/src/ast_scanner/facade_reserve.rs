use oxc::ast::ast::{Program, Statement};

/// The room to reserve for the facade symbols that the top-level statements do not show: those of
/// `import()`, `require()`, `new URL()`, `exports.x =`, `import.meta.hot.accept()`, and a
/// top-level `this` in CommonJS, and those that the link stage adds (for example `require_<name>`
/// and `init_<name>`). Most modules get at most one of these.
const UNCOUNTED_FACADE_SYMBOLS: u32 = 1;

/// The number of facade symbols to reserve room for in the `Scoping` and the `classic_data` of a
/// module.
///
/// The scanner and the link stage add facade symbols after the semantic build, which reserves no
/// room for them. Without a reserve, the first facade symbol grows the symbol arrays of the
/// module. A grown array in the arena keeps its old buffer and can make the arena allocate a new
/// chunk.
///
/// The count follows where the scanner creates facade symbols:
/// - `<name>_default` and `<name>_exports`, and `<name>_hot` in dev mode (`AstScanner::new`);
/// - one for each `import`, `export ... from`, and `export *` statement (`add_import_record`);
/// - one for each specifier of `export { ... } from` (`add_re_export`) and for
///   `export * as ns from` (`add_star_re_export`).
pub fn facade_symbol_reserve(program: &Program<'_>, is_dev_mode: bool) -> u32 {
  let mut count = 2 + u32::from(is_dev_mode) + UNCOUNTED_FACADE_SYMBOLS;
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
