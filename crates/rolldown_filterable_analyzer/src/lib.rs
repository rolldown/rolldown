use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::ast::Visit;
use oxc::cfg::graph::visit::Dfs;
use oxc::parser::{ParseOptions, Parser, ParserReturn};
use oxc::semantic::{Semantic, SemanticBuilder};
use oxc::span::SourceType;
use rolldown_error::{BuildDiagnostic, DiagnosableResult};

pub fn parse<'a>(
  filename: &str,
  source: &'a str,
  alloc: &'a Allocator,
  ty: SourceType,
) -> DiagnosableResult<ParserReturn<'a>> {
  let parser = Parser::new(alloc, source, ty)
    .with_options(ParseOptions { allow_return_outside_function: true, ..ParseOptions::default() });
  let ret = parser.parse();
  if ret.panicked || !ret.errors.is_empty() {
    Err(
      ret
        .errors
        .iter()
        .map(|error| {
          BuildDiagnostic::oxc_parse_error(
            source.into(),
            filename.to_string(),
            error.help.clone().unwrap_or_default().into(),
            error.message.to_string(),
            error.labels.clone().unwrap_or_default(),
          )
        })
        .collect::<Vec<_>>(),
    )
  } else {
    Ok(ret)
  }
}
struct AstWithSemantic<'a> {
  program: Program<'a>,
  semantic: Semantic<'a>,
}

pub fn ast_with_semantic_builder<'a>(
  filename: &str,
  source: &'a str,
  alloc: &'a Allocator,
  ty: SourceType,
) -> DiagnosableResult<AstWithSemantic<'a>> {
  let ParserReturn { program, .. } = parse(filename, source, alloc, ty)?;
  let semantic_ret = SemanticBuilder::new(source, ty).with_cfg(true).build(&program);
  Ok(AstWithSemantic { program, semantic: semantic_ret.semantic })
}

pub fn filterable<'a>(ast_ext: &AstWithSemantic<'a>) -> bool {
  todo!()
}

struct FilterableAnaalyzer<'a> {
  ast_ext: &'a AstWithSemantic<'a>,
}

impl<'a> FilterableAnaalyzer<'a> {
  pub fn new(ast_ext: &'a AstWithSemantic<'a>) -> Self {
    Self { ast_ext }
  }
}

impl<'a> Visit<'a> for FilterableAnaalyzer<'a> {
  fn visit_program(&mut self, it: &Program<'a>) {
    let Some(cfg) = self.ast_ext.semantic.cfg() else {
      return;
    };
    let g = cfg.graph();
    let mut dfs = Dfs::new(&g, oxc::cfg::graph::graph::NodeIndex::from(0));
    while let Some(nx) = dfs.next(&g) {}
  }
  fn visit_function(&mut self, it: &oxc::ast::ast::Function<'a>, flags: oxc::semantic::ScopeFlags) {
    if let Some(cfg) = self.ast_ext.semantic.cfg() {}
  }
  fn enter_node(&mut self, kind: oxc::ast::AstKind<'a>) {}
}
