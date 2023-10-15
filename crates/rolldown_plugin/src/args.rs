use rolldown_common::RawPath;

// #[derive(Debug)]
// pub struct ResolveIdArgsOptions {
//   // pub assertions: FxHashMap<String, String>,
//   pub is_entry: bool,
//   // pub custom
// }

#[derive(Debug)]
pub struct ResolveIdArgs<'a> {
  pub importer: Option<&'a RawPath>,
  pub source: &'a str,
  // pub options: ResolveIdArgsOptions,
}

#[derive(Debug)]
pub struct TransformArgs<'a> {
  pub id: &'a RawPath,
  pub code: &'a String,
}

#[derive(Debug)]
pub struct LoadArgs<'a> {
  pub id: &'a str,
}
