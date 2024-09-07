use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::ast::{AstKind, Visit};
use oxc::parser::{ParseOptions, Parser, ParserReturn};
use oxc::semantic::{Semantic, SemanticBuilder};
use oxc::span::SourceType;
use oxc_cfg::graph::graph::NodeIndex;
use oxc_cfg::graph::visit::{Control, DfsEvent, EdgeRef};
use oxc_cfg::visit::set_depth_first_search;
use oxc_cfg::InstructionKind;
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
pub struct AstWithSemantic<'a> {
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
  let semantic_ret = SemanticBuilder::new(source).with_cfg(true).build(&program);
  Ok(AstWithSemantic { program, semantic: semantic_ret.semantic })
}

pub fn filterable(source: &str) -> bool {
  let alloc = Allocator::default();
  let ast_ext = ast_with_semantic_builder("test", source, &alloc, SourceType::ts()).unwrap();
  let mut analyzer = FilterableAnalyzer::new(&ast_ext);
  analyzer.visit_program(&ast_ext.program);
  analyzer.ret
}

struct FilterableAnalyzer<'b, 'a: 'b> {
  ast_ext: &'b AstWithSemantic<'a>,
  ret: bool,
}

impl<'b, 'a> FilterableAnalyzer<'b, 'a> {
  pub fn new(ast_ext: &'b AstWithSemantic<'a>) -> Self {
    Self { ast_ext, ret: false }
  }
}

impl<'b, 'a> Visit<'a> for FilterableAnalyzer<'b, 'a> {
  fn visit_program(&mut self, _it: &Program<'a>) {
    let Some(cfg) = self.ast_ext.semantic.cfg() else {
      return;
    };
    let g = cfg.graph();
    let mut function_index = None;
    let mut caller_index = NodeIndex::new(0);
    let ret = set_depth_first_search(g, Some(NodeIndex::new(1)), |e| match e {
      DfsEvent::Discover(_, _) => Control::<bool>::Continue,
      DfsEvent::TreeEdge(s, e) => {
        if function_index.is_some() {
          for b in cfg.basic_block(e).instructions() {
            if matches!(b.kind, InstructionKind::Unreachable) {
              return Control::Prune;
            }
            if matches!(b.kind, InstructionKind::Return(_) | InstructionKind::ImplicitReturn) {
              return Control::Break(true);
            }
            let node = b.node_id.map(|id| self.ast_ext.semantic.nodes().get_node(id).kind());
            match node {
              Some(AstKind::IfStatement(_) | AstKind::BlockStatement(_)) => {
                continue;
              }
              Some(_) => {
                if matches!(b.kind, InstructionKind::Condition) {
                  continue;
                }
                return Control::Prune;
              }
              None => {
                return Control::Continue;
              }
            }
          }
        } else {
          for e in g.edges_connecting(s, e) {
            if matches!(g.edge_weight(e.id()), Some(oxc_cfg::EdgeType::NewFunction)) {
              function_index = Some(e.target());
              caller_index = s;
            }
          }
        }
        Control::Continue
      }
      DfsEvent::BackEdge(..) | DfsEvent::CrossForwardEdge(..) => Control::Continue,
      DfsEvent::Finish(s, _) => {
        if Some(s) == function_index {
          return Control::Break(false);
        }
        Control::Continue
      }
    });
    self.ret = ret.break_value().unwrap_or_default();
  }
}
