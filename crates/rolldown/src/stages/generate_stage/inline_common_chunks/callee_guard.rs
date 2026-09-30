//! After finalization, a record's function is called through its bridge only as
//! `(0, bridge.f)(...)`: a bare `bridge.f(...)` or `` bridge.f`...` `` would bind the registry's
//! exports object as `this`. The one exception is a module wrapper (`init_*`, `require_*`): the
//! `__esm`/`__commonJS` wrappers never read `this`, so the finalizer prints `bridge.t()` for them.
//! Every finalized module of a chunk that reads records passes through here, so a call shape the
//! finalizer's rewrites missed fails the build instead of printing code whose `this` differs from
//! the build without the option.

use oxc::ast::ast::{CallExpression, Expression, Program, TaggedTemplateExpression};
use oxc::ast_visit::{VisitJs, walk_js};
use oxc_str::CompactStr;
use rolldown_error::BuildDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

/// One chunk's bridge bindings, each with the export names bound to module wrappers.
#[derive(Debug, Default)]
pub struct BridgeCallees {
  wrappers: FxHashMap<CompactStr, FxHashSet<CompactStr>>,
}

impl BridgeCallees {
  pub fn is_empty(&self) -> bool {
    self.wrappers.is_empty()
  }

  pub(super) fn add_bridge(&mut self, bridge: &CompactStr) -> &mut FxHashSet<CompactStr> {
    self.wrappers.entry(bridge.clone()).or_default()
  }

  fn is_bridge(&self, name: &str) -> bool {
    self.wrappers.contains_key(name)
  }

  fn is_wrapper(&self, bridge: &str, member: &str) -> bool {
    self.wrappers.get(bridge).is_some_and(|members| members.contains(member))
  }
}

/// The build error for the first bare bridge member used as a callee or template tag in the
/// finalized `program` of `module_id`, if there is one.
pub fn bare_bridge_callee(
  program: &Program<'_>,
  callees: &BridgeCallees,
  module_id: &str,
) -> Option<BuildDiagnostic> {
  if callees.is_empty() {
    return None;
  }
  let mut visitor = BareBridgeCallee { callees, found: None };
  visitor.visit_program(program);
  let callee = visitor.found?;
  Some(BuildDiagnostic::unhandleable_error(anyhow::anyhow!(
    "`experimentalInlineCommonChunks`: module `{module_id}` calls `{callee}` without the `(0, ...)` guard that keeps `this` undefined; keep the shared chunk a file with the option's `exclude` until this is fixed."
  )))
}

struct BareBridgeCallee<'b> {
  callees: &'b BridgeCallees,
  found: Option<String>,
}

impl BareBridgeCallee<'_> {
  fn bare_bridge_member(&self, callee: &Expression<'_>) -> Option<String> {
    // `(ns?.f)()` keeps the member inside a chain expression; `(0, ns.f)()` is a sequence.
    let member = match callee {
      Expression::ChainExpression(chain) => chain.expression.as_member_expression()?,
      expr => expr.as_member_expression()?,
    };
    let Expression::Identifier(object) = member.object() else {
      return None;
    };
    if !self.callees.is_bridge(&object.name) {
      return None;
    }
    match member.static_property_name() {
      Some(name) if self.callees.is_wrapper(&object.name, name) => None,
      Some(name) => Some(format!("{}.{name}", object.name)),
      None => Some(format!("{}[...]", object.name)),
    }
  }
}

impl<'a> VisitJs<'a> for BareBridgeCallee<'_> {
  fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
    if self.found.is_none() {
      self.found = self.bare_bridge_member(&it.callee);
    }
    walk_js::walk_call_expression(self, it);
  }

  fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
    if self.found.is_none() {
      self.found = self.bare_bridge_member(&it.tag);
    }
    walk_js::walk_tagged_template_expression(self, it);
  }
}
