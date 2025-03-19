use generator::generators::{CheckOptionsGenerator, Context, Generator};
fn main() -> anyhow::Result<()> {
  let ctx = Context { workspace_root: rolldown_workspace::root_dir() };
  let generators: Vec<Box<dyn Generator>> =
    vec![Box::new(CheckOptionsGenerator { disabled_event: vec!["CircularDependency"] })];
  for generator in generators {
    let outputs = generator.run(&ctx)?;
    for output in outputs {
      let raw_output = output.into_raw(generator.file_path());
      raw_output.write_to_file()?;
    }
  }
  Ok(())
}
