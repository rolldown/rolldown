use oxc::ast::ast::{
  ArrowFunctionExpression, CallExpression, ExportNamedDeclaration, Expression, Function,
  IdentifierReference, Statement, TSType,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::ScopeFlags;
use rolldown_common::AstScopes;

/// Does a top-level statement **read** an imported binding during its own
/// module-evaluation-time execution?
///
/// This is the execution-order-sensitive signal for imported-binding reads. It is a deliberate
/// **over-approximation** — sound for wrapping decisions, where the only dangerous mistake is
/// under-reporting (a genuinely order-sensitive statement reported as `false` could be unwrapped
/// and reordered across a mutation). When unsure we report `true`.
///
/// Why a dedicated uniform walk instead of threading the fact through the side-effect analyzer:
/// completeness. The side-effect analyzer propagates facts per expression form, which is how gaps
/// creep in (e.g. an imported binding buried in a mid-chain computed key such as `a[imp].y`, or a
/// namespace aliased into a local). Here we simply visit **every** sub-node once, so no expression
/// form can be missed, and any newly added syntax is covered by default.
///
/// What is skipped, and why it stays sound:
/// - **Function / arrow bodies** are skipped when the function is merely declared or stored because
///   they run at call time. Directly invoked function literals are the exception: their parameter
///   defaults run immediately, as do non-generator bodies, so `visit_call_expression` traverses
///   those parts explicitly.
/// - **Import declarations and re-export specifiers** (`export { a }`, `export { a } from '...'`)
///   forward bindings; they do not read a value. Only the `declaration` half of an export (e.g.
///   `export const x = imp`) evaluates at module-eval time.
/// - **Type annotations** are erased at runtime.
///
/// Deliberately conservative (reports `true` though the read is technically deferred):
/// - **Instance class-field initializers** (`class C { f = imp }`) run at construction, not at
///   class definition. They are treated as top-level reads for now; refining this is a pure
///   precision gain and can come later.
///
/// Known limitation, inherent to any syntactic analysis: under
/// `treeshake.propertyReadSideEffects: false`, a getter/`Proxy` trap that reads an imported binding
/// is invisible here. That is the option's explicit "reads are side-effect-free" promise, not a gap
/// this walk can close.
pub struct TopLevelImportReadDetector<'scopes> {
  scopes: &'scopes AstScopes,
  reads_import: bool,
}

impl<'scopes> TopLevelImportReadDetector<'scopes> {
  /// Returns `true` if evaluating `stmt` at module-eval time reads an imported binding.
  pub fn detect(scopes: &'scopes AstScopes, stmt: &Statement) -> bool {
    let mut detector = Self { scopes, reads_import: false };
    detector.visit_statement(stmt);
    detector.reads_import
  }

  fn visit_immediately_invoked_function(&mut self, callee: &Expression<'_>) {
    match callee.get_inner_expression() {
      Expression::FunctionExpression(function) => {
        self.visit_formal_parameters(&function.params);
        if !function.generator
          && let Some(body) = &function.body
        {
          self.visit_function_body(body);
        }
      }
      Expression::ArrowFunctionExpression(arrow) => {
        self.visit_formal_parameters(&arrow.params);
        self.visit_function_body(&arrow.body);
      }
      Expression::SequenceExpression(sequence) => {
        if let Some(last) = sequence.expressions.last() {
          self.visit_immediately_invoked_function(last);
        }
      }
      _ => {}
    }
  }
}

impl<'ast> Visit<'ast> for TopLevelImportReadDetector<'_> {
  fn visit_identifier_reference(&mut self, it: &IdentifierReference<'ast>) {
    if self.reads_import {
      return;
    }
    let Some(reference_id) = it.reference_id.get() else {
      return;
    };
    let Some(symbol_id) = self.scopes.symbol_id_for(reference_id) else {
      return;
    };
    if self.scopes.scoping().symbol_flags(symbol_id).is_import() {
      self.reads_import = true;
    }
  }

  // Prune the rest of the walk once we've already found a read.
  fn visit_expression(&mut self, it: &Expression<'ast>) {
    if self.reads_import {
      return;
    }
    walk::walk_expression(self, it);
  }

  // Merely creating a function does not run its parameters or body.
  fn visit_function(&mut self, _it: &Function<'ast>, _flags: ScopeFlags) {}
  fn visit_arrow_function_expression(&mut self, _it: &ArrowFunctionExpression<'ast>) {}

  fn visit_call_expression(&mut self, it: &CallExpression<'ast>) {
    if self.reads_import {
      return;
    }
    walk::walk_call_expression(self, it);
    if !self.reads_import {
      self.visit_immediately_invoked_function(&it.callee);
    }
  }

  // `export { a }` / `export { a } from '...'` forward bindings without reading them. Only the
  // declaration half of an export (`export const x = imp`) evaluates at module-eval time.
  fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'ast>) {
    if let Some(declaration) = &it.declaration {
      self.visit_declaration(declaration);
    }
  }

  // Types are erased at runtime; a type-only import reference is never a value read.
  fn visit_ts_type(&mut self, _it: &TSType<'ast>) {}
}

#[cfg(test)]
mod test {
  use super::TopLevelImportReadDetector;
  use oxc::span::SourceType;
  use rolldown_common::AstScopes;
  use rolldown_ecmascript::{EcmaAst, EcmaCompiler};

  /// One bool per top-level statement: does it read an imported binding at module-eval time?
  fn detect(code: &str) -> Vec<bool> {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program());
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);
    ast
      .program()
      .body
      .iter()
      .map(|stmt| TopLevelImportReadDetector::detect(&ast_scopes, stmt))
      .collect()
  }

  #[test]
  fn direct_and_member_reads() {
    // Import declaration itself reads nothing; the read is in `snap`.
    assert_eq!(
      detect("import { counter } from './state'; export const snap = counter;"),
      vec![false, true]
    );
    // Member chain rooted at an import.
    assert_eq!(
      detect("import { obj } from './state'; export const snap = obj.value;"),
      vec![false, true]
    );
    // Import read as a call argument at top level.
    assert_eq!(detect("import { x } from './state'; sideEffect(x);"), vec![false, true]);
    // Import read inside a compound expression.
    assert_eq!(
      detect("import { x } from './state'; export const o = { a: x };"),
      vec![false, true]
    );
  }

  #[test]
  fn nested_computed_key_hole_is_closed() {
    // `a[imp].y` — the import is a mid-chain computed key. This is exactly the gap the per-form
    // analyzer missed; the uniform walk catches it unconditionally (no `propertyReadSideEffects`
    // dependence).
    assert_eq!(
      detect("import { key } from './state'; const obj = {}; export const snap = obj[key].y;"),
      vec![false, false, true]
    );
    assert_eq!(
      detect("import { key } from './state'; const obj = {}; export const snap = obj[key][key];"),
      vec![false, false, true]
    );
  }

  #[test]
  fn namespace_member_and_alias() {
    // Namespace member read.
    assert_eq!(
      detect("import * as ns from './state'; export const snap = ns.foo;"),
      vec![false, true]
    );
    // Namespace aliased into a local, then read through the alias. Flagging the bare `ns` read on
    // the `const x = ns` statement is what makes the module order-sensitive — closing the alias
    // gap conservatively.
    assert_eq!(
      detect("import * as ns from './state'; const x = ns; export const snap = x.foo;"),
      vec![false, true, false]
    );
  }

  #[test]
  fn function_and_arrow_bodies_are_deferred() {
    // Reading an import inside a function/arrow body is a call-time read, not module-eval time.
    assert_eq!(
      detect("import { counter } from './state'; export function read() { return counter; }"),
      vec![false, false]
    );
    assert_eq!(
      detect("import { counter } from './state'; export const read = () => counter;"),
      vec![false, false]
    );
    // Class method bodies are deferred too.
    assert_eq!(
      detect("import { x } from './state'; export class C { m() { return x; } }"),
      vec![false, false]
    );
  }

  #[test]
  fn immediately_invoked_functions_run_during_module_evaluation() {
    assert_eq!(
      detect("import { value } from './state'; export const snapshot = (() => value)();"),
      vec![false, true]
    );
    assert_eq!(
      detect(
        "import { value } from './state'; export const snapshot = (function () { return value })();"
      ),
      vec![false, true]
    );
    // Parameter defaults run when the function is called.
    assert_eq!(
      detect("import { value } from './state'; export const snapshot = ((x = value) => x)();"),
      vec![false, true]
    );
    // A sequence expression calls only its final expression.
    assert_eq!(
      detect("import { value } from './state'; export const snapshot = (0, (() => value))();"),
      vec![false, true]
    );
    assert_eq!(
      detect(
        "import { value } from './state'; export const snapshot = ((() => value), (() => 0))();"
      ),
      vec![false, false]
    );
  }

  #[test]
  fn immediately_invoked_generator_only_runs_parameter_initializers() {
    assert_eq!(
      detect(
        "import { value } from './state'; export const iterator = (function* (x = value) {})();"
      ),
      vec![false, true]
    );
    assert_eq!(
      detect(
        "import { value } from './state'; export const iterator = (function* () { yield value })();"
      ),
      vec![false, false]
    );
  }

  #[test]
  fn reexports_and_non_imports_are_not_reads() {
    // Pure re-export barrel forwards a binding without reading its value.
    assert_eq!(detect("export { a } from './a';"), vec![false]);
    assert_eq!(detect("import { a } from './a'; export { a };"), vec![false, false]);
    // A local (non-import) binding read is not order-sensitive here.
    assert_eq!(detect("const a = 1; export const b = a;"), vec![false, false]);
  }

  #[test]
  fn class_definition_time_reads_are_flagged() {
    // Static field initializers run at class-definition (module-eval) time.
    assert_eq!(
      detect("import { x } from './state'; export class C { static s = x; }"),
      vec![false, true]
    );
    // A superclass expression is evaluated at class-definition time.
    assert_eq!(
      detect("import { Base } from './state'; export class C extends Base {}"),
      vec![false, true]
    );
  }
}
