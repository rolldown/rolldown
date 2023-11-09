#[derive(Debug)]
pub struct HookResolveIdArgs<'a> {
  pub importer: Option<&'a str>,
  pub source: &'a str,
}

#[derive(Debug)]
pub struct HookTransformArgs<'a> {
  pub id: &'a str,
  pub code: &'a String,
}

#[derive(Debug)]
pub struct HookLoadArgs<'a> {
  pub id: &'a str,
}
