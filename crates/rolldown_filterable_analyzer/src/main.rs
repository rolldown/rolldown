use rolldown_filterable_analyzer::filterable;

fn main() {
  let source = r#"
function test()  {
  if (test) {
    return;
  }
}
  "#;
  dbg!(&filterable(source));
}
