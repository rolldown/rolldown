use oxc_str::CompactStr;

use crate::ModuleIdx;

/// The place of a module in a group of deferred modules that share one init wrapper.
///
/// A group has a root and one or more private members. Only the root keeps its `init_*`
/// wrapper. The wrapper of the root runs the body of each member, in the order of evaluation,
/// before the body of the root. Each member keeps its top-level declarations (hoisted functions,
/// the namespace object and the `var` list) at the top level of the chunk.
/// See internal-docs/import-defer/implementation.md ("Deferred groups").
#[derive(Debug, Clone, Copy, Default)]
pub enum ConcatenateWrappedModuleKind {
  /// The root of a group. Its `init_*` wrapper runs the whole group.
  Root,
  /// A module that is not in a group.
  #[default]
  None,
  /// A private member of a group. It has no `init_*` wrapper.
  Inner,
}

/// One step of a body of a group, in source order. The renderer walks these steps to put the
/// bodies of the members in the order of evaluation.
#[derive(Debug, Clone)]
pub enum ConcatenatedGroupEvent {
  /// The body imports this member of its group. The member body runs here, if it did not run.
  Member(ModuleIdx),
  /// The body calls this init function of a module outside the group. If a member body runs
  /// before the body itself, the renderer puts a call of this function before the member body
  /// (a pre-call), so that the order of evaluation does not change.
  Call(CompactStr),
  /// A statement that the finalizer moved out of the body. At the place of an import or a
  /// re-export statement, it writes a binding or a namespace object of the module, for example
  /// `import_x = __toESM(require_x())` or `__reExport(x_exports, y_exports)`. A later step of
  /// the body is a member, and a member body can read the binding. The renderer writes the
  /// statement one time: before the next member body that it adds, or else before the body.
  Init(CompactStr),
}

#[derive(Debug, Clone, Default)]
pub struct RenderedConcatenatedModuleParts {
  /// The steps of the body of a root or a member, in source order.
  pub group_events: Vec<ConcatenatedGroupEvent>,
  /// For a root or a member: the index of the first statement of its body in its finalized
  /// program. The statements before it are its top-level declarations. The two parts are
  /// printed on their own (`NormalModule::render_group_body`).
  pub body_start: Option<usize>,
  /// For a root: the text of its init wrapper before and after the closure body, for example
  /// `function init_x() {\n\treturn (init_x = __esmMin((() => {` and `})))();\n}`. The renderer
  /// writes the code of the group between the two parts.
  pub root_wrapper: Option<(String, String)>,
}
