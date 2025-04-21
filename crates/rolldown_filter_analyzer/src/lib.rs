use oxc::ast::AstKind;
use oxc::ast::ast::{Expression, Program};
use oxc::ast_visit::Visit;
use oxc::cfg::graph::adj::NodeIndex;
use oxc::cfg::graph::visit::{Control, DfsEvent, EdgeRef};
use oxc::cfg::visit::set_depth_first_search;
use oxc::cfg::{EdgeType, InstructionKind};
use oxc::semantic::Semantic;
use oxc::span::SourceType;
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};

#[cfg(test)]
mod test;
pub fn filterable(source: &str) -> bool {
  let ast = EcmaCompiler::parse("<Noop>", source, SourceType::mjs()).unwrap();
  let semantic = EcmaAst::make_semantic(ast.program(), true);
  let mut analyzer = FilterableAnalyzer::new(&semantic);
  analyzer.visit_program(&ast.program());
  analyzer.ret
}

struct FilterableAnalyzer<'b, 'a: 'b> {
  ast_ext: &'b Semantic<'a>,
  ret: bool,
}

impl<'b, 'a> FilterableAnalyzer<'b, 'a> {
  pub fn new(ast_ext: &'b Semantic<'a>) -> Self {
    Self { ast_ext, ret: false }
  }
}

impl<'b, 'a> Visit<'a> for FilterableAnalyzer<'b, 'a> {
  fn visit_program(&mut self, _it: &Program<'a>) {
    let Some(cfg) = self.ast_ext.cfg() else {
      return;
    };
    let g = cfg.graph();
    let mut function_index = None;
    let mut caller_index = NodeIndex::from(0);
    let ret = set_depth_first_search(g, Some(NodeIndex::from(1)), |e| {
      match e {
        DfsEvent::Discover(n, _) => Control::<bool>::Continue,
        DfsEvent::TreeEdge(s, e) => {
          if function_index.is_some() {
            for b in cfg.basic_block(e).instructions() {
              if matches!(b.kind, InstructionKind::Unreachable) {
                return Control::Prune;
              }
              if matches!(b.kind, InstructionKind::ImplicitReturn) {
                return Control::Break(true);
              }
              let node = b.node_id.map(|id| self.ast_ext.nodes().get_node(id).kind());
              match node {
                Some(AstKind::ReturnStatement(stmt)) => match stmt.argument {
                  // `return undefined;`
                  Some(Expression::Identifier(ref id)) if id.name == "undefined" => {
                    return Control::Break(true);
                  }
                  // `return;`
                  None => {
                    return Control::Break(true);
                  }
                  _ => {
                    return Control::Prune;
                  }
                },
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
              if matches!(g.edge_weight(e.id()), Some(EdgeType::NewFunction)) {
                function_index = Some(e.target());
                caller_index = s;
              }
            }
          }
          Control::Continue
        }
        DfsEvent::BackEdge(..) | DfsEvent::CrossForwardEdge(..) => Control::Continue,
        DfsEvent::Finish(s, _) => Control::Continue,
      }
    });
    self.ret = ret.break_value().unwrap_or_default();
  }
}
