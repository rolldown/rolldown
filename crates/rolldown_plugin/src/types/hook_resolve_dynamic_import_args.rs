#[derive(Debug)]
pub struct HookResolveDynamicImportArgs<'a> {
  pub importer: Option<&'a str>,
  pub source: &'a str,
}
