use rolldown_common::ImportKind;

#[derive(Debug)]
pub struct HookResolveIdArgs<'a> {
  pub importer: Option<&'a str>,
  pub specifier: &'a str,
  pub is_entry: bool,
  // Rollup doesn't have a `kind` field, but rolldown supports cjs, css by default. So we need this
  // field to determine the import kind.
  pub kind: ImportKind,
}
