//! Post-fixpoint validation of statement-inclusion results.
//!
//! Tree shaking, the lazy-barrel loader, and the finalizers each hold their own model of "what is
//! needed" (statement-level, export-level, and record-level respectively). Historically every
//! disagreement between them surfaced as a runtime `ReferenceError` in the emitted bundle
//! (#9691, #9806, #9961, #9964, #10013, #10048). This pass encodes the contract those bugs
//! violated so disagreements become deterministic bundle-time panics instead:
//!
//! 1. **No dangling deferred import** — an included statement must never reference a binding
//!    whose import record was never loaded (`resolved_module == None`, i.e. deferred by the
//!    lazy-barrel loader). The finalizer drops such import declarations, so any surviving
//!    reference is a free identifier at runtime.
//! 2. **No reference to an excluded declaration** — an included statement must never reference a
//!    module-level binding all of whose declaring statements were tree-shaken away.
//!
//! The checks deliberately mirror the *bypasses* in `include_statement`/`include_symbol`
//! (inlined constants, inlined enum members, JSON property inlining, `void 0` member-expression
//! rewrites, shimmed missing exports) so that everything the finalizer materializes without a
//! declaration is skipped rather than reported. Namespace objects are also skipped: their
//! emission has dedicated machinery (`ModuleNamespaceIncludedReason`, dynamic-import usage,
//! finalizer synthesis) that is not statement-shaped.
//!
//! Enabled by default in `testing` builds (the Rust fixture harness), and controllable
//! everywhere via `ROLLDOWN_VALIDATE_INCLUSION=1|0`.

use std::fmt::Write as _;
use std::sync::OnceLock;

use rolldown_common::{
  Module, ModuleType, NormalModule, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef,
};

use crate::stages::link_stage::LinkStage;

fn validation_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| match std::env::var("ROLLDOWN_VALIDATE_INCLUSION") {
    // Explicit env always wins: `0` opts out (e.g. while triaging a false positive),
    // anything else opts in (e.g. debugging a real build outside the test harness).
    Ok(v) => v != "0",
    // Default: on for test builds (`rolldown_testing` enables the `testing` feature),
    // off for production builds (napi bindings, benches).
    Err(_) => cfg!(feature = "testing"),
  })
}

enum Violation {
  /// Rule 1: reference to a binding whose import record was never loaded.
  DanglingDeferredImport {
    from_module: String,
    from_stmt: StmtInfoIdx,
    symbol_name: String,
    import_owner: String,
  },
  /// Rule 2: reference to a binding all of whose declaring statements were excluded.
  DeclarationNotIncluded {
    from_module: String,
    from_stmt: StmtInfoIdx,
    symbol_name: String,
    decl_owner: String,
  },
}

impl std::fmt::Display for Violation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Violation::DanglingDeferredImport { from_module, from_stmt, symbol_name, import_owner } => {
        write!(
          f,
          "[dangling deferred import] included stmt #{from_stmt:?} of `{from_module}` references \
           `{symbol_name}`, but its import record in `{import_owner}` was never loaded \
           (resolved_module=None) — the finalizer will drop the import declaration and leave a \
           free identifier"
        )
      }
      Violation::DeclarationNotIncluded { from_module, from_stmt, symbol_name, decl_owner } => {
        write!(
          f,
          "[declaration not included] included stmt #{from_stmt:?} of `{from_module}` references \
           `{symbol_name}` declared in `{decl_owner}`, but none of its declaring statements were \
           included"
        )
      }
    }
  }
}

/// Validate the final inclusion bitsets stored in `metas` (call after they are written back).
///
/// Panics with a report of every violation found. See the module docs for the contract.
pub fn validate_statement_inclusion(link_stage: &LinkStage<'_>) {
  if !validation_enabled() {
    return;
  }
  let mut violations = Vec::new();
  for module in link_stage.module_table.modules.iter().filter_map(Module::as_normal) {
    let meta = &link_stage.metas[module.idx];
    if !meta.is_included {
      continue;
    }
    // A transformed JSON module's properties may be inlined by the finalizer; its internal
    // references don't map 1:1 to emitted declarations.
    if module.module_type == ModuleType::Json {
      continue;
    }
    // Namespace stmt (idx 0) is skipped: the namespace object references every export member,
    // and its emission is driven by dedicated machinery rather than per-statement inclusion.
    for (stmt_idx, stmt_info) in
      link_stage.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt()
    {
      if !meta.stmt_info_included.has_bit(stmt_idx) {
        continue;
      }
      for reference in &stmt_info.referenced_symbols {
        match reference {
          SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => {
            if let Some(resolution) = member_expr_ref.resolution(&meta.resolved_member_expr_refs) {
              // `resolved == None` is rewritten to `void 0`; nothing to validate.
              if let Some(resolved) = resolution.resolved {
                check_symbol(link_stage, module, stmt_idx, resolved, &mut violations);
              }
              for depended in &resolution.depended_refs {
                check_symbol(link_stage, module, stmt_idx, *depended, &mut violations);
              }
            } else {
              // Mirror `include_statement`'s inlined-enum-member bypass.
              if is_inlined_enum_member_access(link_stage, member_expr_ref) {
                continue;
              }
              check_symbol(
                link_stage,
                module,
                stmt_idx,
                member_expr_ref.object_ref,
                &mut violations,
              );
            }
          }
          SymbolOrMemberExprRef::Symbol(_) => {
            check_symbol(link_stage, module, stmt_idx, *reference.symbol_ref(), &mut violations);
          }
        }
      }
    }
  }
  if !violations.is_empty() {
    const MAX_SHOWN: usize = 20;
    let mut report = String::new();
    for violation in violations.iter().take(MAX_SHOWN) {
      _ = writeln!(report, "  - {violation}");
    }
    if violations.len() > MAX_SHOWN {
      _ = writeln!(report, "  ... and {} more", violations.len() - MAX_SHOWN);
    }
    panic!(
      "tree-shaking inclusion validation failed ({} violation(s)) — the emitted bundle would \
       reference dropped bindings:\n{report}This is a rolldown bug; set \
       ROLLDOWN_VALIDATE_INCLUSION=0 to bypass while triaging.",
      violations.len()
    );
  }
}

/// Mirror of the enum-member inlining bypass in `include_statement`: a simple, read-only
/// `Enum.member` access whose member is in `enum_member_value_map` is replaced by a literal, so
/// the enum declaration is legitimately excluded.
fn is_inlined_enum_member_access(
  link_stage: &LinkStage<'_>,
  member_expr_ref: &rolldown_common::MemberExprRef,
) -> bool {
  let canonical_ref = link_stage.symbols.canonical_ref_for(member_expr_ref.object_ref);
  let Some(Module::Normal(owner_module)) = link_stage.module_table.modules.get(canonical_ref.owner)
  else {
    return false;
  };
  let symbol_name = canonical_ref.name(&link_stage.symbols);
  let Some(members) = owner_module.ecma_view.enum_member_value_map.get(symbol_name) else {
    return false;
  };
  !member_expr_ref.is_write
    && matches!(member_expr_ref.prop_and_span_list.as_slice(),
      [prop] if members.contains_key(prop.name.as_str()))
}

fn check_symbol(
  link_stage: &LinkStage<'_>,
  from_module: &NormalModule,
  from_stmt: StmtInfoIdx,
  symbol_ref: SymbolRef,
  violations: &mut Vec<Violation>,
) {
  let canonical = link_stage.symbols.canonical_ref_for(symbol_ref);
  // Inlined constants never materialize a reference in the output (mirror of the
  // `constant_symbol_map` bypass in `include_symbol`).
  if link_stage.global_constant_symbol_map.contains_key(&canonical) {
    return;
  }
  // Follow the CJS-interop alias the same way `include_symbol` does: the emitted identifier is
  // the alias target (`import_foo`), not the aliased binding.
  let mut target = canonical;
  if let Some(namespace_alias) = &link_stage.symbols.get(canonical).namespace_alias {
    target = namespace_alias.namespace_ref;
  }
  let owner_idx = target.owner;
  let Some(owner) = link_stage.module_table.modules[owner_idx].as_normal() else {
    // External modules: the import is emitted as-is.
    return;
  };
  let owner_meta = &link_stage.metas[owner_idx];

  // Rule 1 — the lazy-barrel family: a binding whose import record was never loaded cannot be
  // emitted, so any surviving reference to it is a free identifier at runtime.
  if let Some(named_import) = owner.named_imports.get(&target) {
    let rec = &owner.import_records[named_import.record_idx];
    if rec.resolved_module.is_none() {
      violations.push(Violation::DanglingDeferredImport {
        from_module: from_module.stable_id.to_string(),
        from_stmt,
        symbol_name: target.name(&link_stage.symbols).to_string(),
        import_owner: owner.stable_id.to_string(),
      });
      return;
    }
  }

  // Namespace objects have dedicated emission machinery; not statement-shaped.
  if owner.namespace_object_ref == target {
    return;
  }
  // Shimmed missing exports are materialized by the finalizer (`var x = void 0`).
  if owner_meta.shimmed_missing_exports.values().any(|shimmed| *shimmed == target) {
    return;
  }
  // JSON owners: properties may be inlined by the finalizer.
  if owner.module_type == ModuleType::Json {
    return;
  }

  let declared = link_stage.stmt_infos[owner_idx].declared_stmts_by_symbol(&target);
  if declared.is_empty() {
    // Facade symbols with no declaring statement (wrapper refs handled via their own stmt infos
    // have one; e.g. HMR hot refs don't). Unknown provenance — out of scope for v1.
    return;
  }
  // Rule 2 — shaker/finalizer agreement: some declaring statement must have survived.
  if !declared.iter().any(|stmt_idx| owner_meta.stmt_info_included.has_bit(*stmt_idx)) {
    violations.push(Violation::DeclarationNotIncluded {
      from_module: from_module.stable_id.to_string(),
      from_stmt,
      symbol_name: target.name(&link_stage.symbols).to_string(),
      decl_owner: owner.stable_id.to_string(),
    });
  }
}
