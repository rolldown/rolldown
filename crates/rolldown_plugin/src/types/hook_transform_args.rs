#[derive(Debug)]
pub struct HookTransformArgs<'a> {
  pub id: &'a str,
  pub code: &'a String,
}
