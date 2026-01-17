use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::codegen::Codegen;

pub struct RuntimeBindingGenerator<'a> {
  #[expect(dead_code)]
  allocator: &'a Allocator,
}

impl<'a> RuntimeBindingGenerator<'a> {
  #[expect(dead_code)]
  pub fn new(allocator: &'a Allocator) -> Self {
    Self { allocator }
  }

  pub fn generate_runtime_binding(
    binding_name: &str,
    decl_id: usize,
    deps: &[String],
    type_params: &[String],
    has_side_effect: bool,
  ) -> String {
    let mut elements =
      vec![format!("{decl_id}"), Self::format_deps_function(deps, type_params), "[]".to_string()];

    if has_side_effect {
      elements.push("sideEffect()".to_string());
    }

    format!("var {} = [{}]", binding_name, elements.join(", "))
  }

  fn format_deps_function(deps: &[String], type_params: &[String]) -> String {
    let params = if type_params.is_empty() { String::new() } else { type_params.join(", ") };

    let deps_str = if deps.is_empty() { String::new() } else { deps.join(", ") };

    format!("({params}) => [{deps_str}]")
  }

  #[expect(dead_code)]
  pub fn generate_code(program: &Program<'a>) -> String {
    Codegen::new().build(program).code
  }
}

pub fn extract_source_text(source: &str, start: u32, end: u32) -> String {
  let start = start as usize;
  let end = end as usize;
  if start < source.len() && end <= source.len() && start < end {
    source[start..end].to_string()
  } else {
    String::new()
  }
}
