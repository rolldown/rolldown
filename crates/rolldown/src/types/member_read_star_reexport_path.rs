use rolldown_common::{ImportRecordIdx, ModuleIdx, StmtInfoIdx, SymbolRef};

/// A retained star re-export path recorded for a statically resolved namespace-member read
/// (`ns.member` where `ns` is a namespace-valued binding). See
/// `internal-docs/code-splitting/implementation.md`.
#[derive(Debug)]
pub struct MemberReadStarReexportPath {
  /// The statement that performs the read. The path is consumed only while it is included.
  pub reader: (ModuleIdx, StmtInfoIdx),
  /// The canonical binding the read resolves to. It must also be used for the path to be consumed.
  pub resolved: SymbolRef,
  /// The forwarding hops from the reader's importee down to the binding's definer.
  pub path: Vec<(ModuleIdx, ImportRecordIdx)>,
}
