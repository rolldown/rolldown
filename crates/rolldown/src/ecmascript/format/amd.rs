pub fn render_amd_arguments(externals: &[(String, bool)]) -> String {
  // do not support `output.amd` yet.
  let mut output_args = vec![];
  externals.iter().for_each(|(external, _)| {
    output_args.push(format!("require(\"{external}\")"));
  });

  output_args.join(", ")
}
