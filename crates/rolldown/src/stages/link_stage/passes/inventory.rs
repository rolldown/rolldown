use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
};

use syn::{
  Attribute, Fields, Item, PathArguments, Token, Type, UseTree, Visibility,
  punctuated::Punctuated,
  visit::{self, Visit},
};

const FORBIDDEN_CARRIERS: [&str; 8] = [
  "LinkStage",
  "LinkStageOutput",
  "LinkingMetadata",
  "LinkingMetadataVec",
  "PassPipelineCtx",
  "InclusionCoreContext",
  "InclusionFacts",
  "InclusionModuleFacts",
];

fn rust_sources(root: &Path) -> Vec<PathBuf> {
  let mut pending = vec![root.to_path_buf()];
  let mut sources = Vec::new();

  while let Some(path) = pending.pop() {
    let mut entries = fs::read_dir(&path)
      .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
      .map(|entry| entry.unwrap_or_else(|error| panic!("failed to read entry: {error}")).path())
      .collect::<Vec<_>>();
    entries.sort();
    for entry in entries.into_iter().rev() {
      if entry.is_dir() {
        pending.push(entry);
      } else if entry.extension().is_some_and(|extension| extension == "rs")
        && entry.file_name().is_some_and(|name| name != "inventory.rs")
      {
        sources.push(entry);
      }
    }
  }

  sources.sort();
  sources
}

fn is_pub_super(visibility: &Visibility) -> bool {
  matches!(
    visibility,
    Visibility::Restricted(restricted)
      if restricted.in_token.is_none()
        && restricted.path.leading_colon.is_none()
        && restricted.path.segments.len() == 1
        && restricted.path.segments[0].ident == "super"
  )
}

fn is_plain_path(path: &syn::Path, expected: &str) -> bool {
  path.leading_colon.is_none()
    && path.segments.len() == 1
    && path.segments[0].ident == expected
    && matches!(path.segments[0].arguments, PathArguments::None)
}

fn is_exact_path(path: &syn::Path, expected: &[&str]) -> bool {
  path.leading_colon.is_none()
    && path.segments.len() == expected.len()
    && path.segments.iter().zip(expected).all(|(segment, expected)| {
      segment.ident == expected && matches!(segment.arguments, PathArguments::None)
    })
}

fn plain_self_type(ty: &Type) -> Option<String> {
  let Type::Path(path) = ty else { return None };
  if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
    return None;
  }
  let segment = &path.path.segments[0];
  if !matches!(segment.arguments, PathArguments::None) {
    return None;
  }
  Some(segment.ident.to_string())
}

fn reject_named_non_struct_pass(item: &Item, source: &Path) {
  let name = match item {
    Item::Const(item) => Some(&item.ident),
    Item::Enum(item) => Some(&item.ident),
    Item::Fn(item) => Some(&item.sig.ident),
    Item::Static(item) => Some(&item.ident),
    Item::Trait(item) => Some(&item.ident),
    Item::TraitAlias(item) => Some(&item.ident),
    Item::Type(item) => Some(&item.ident),
    Item::Union(item) => Some(&item.ident),
    _ => None,
  };
  if let Some(name) = name.filter(|name| name.to_string().ends_with("Pass")) {
    panic!("{} declares `{name}` as something other than a unit struct", source.display());
  }
}

fn normalized_ident(ident: &syn::Ident) -> String {
  let ident = ident.to_string();
  ident.strip_prefix("r#").unwrap_or(&ident).to_owned()
}

fn inspect_use_tree(tree: &UseTree, prefix: &mut Vec<String>, source: &Path) {
  match tree {
    UseTree::Path(path) => {
      let imported = normalized_ident(&path.ident);
      assert!(
        !FORBIDDEN_CARRIERS.contains(&imported.as_str()),
        "{}: broad carrier `{imported}` must not be supplied through an import path",
        source.display()
      );
      prefix.push(imported);
      inspect_use_tree(&path.tree, prefix, source);
      prefix.pop();
    }
    UseTree::Name(name) => {
      let imported = normalized_ident(&name.ident);
      prefix.push(imported.clone());
      assert!(
        !FORBIDDEN_CARRIERS.contains(&imported.as_str()),
        "{}: broad carrier `{imported}` must not be supplied through an import",
        source.display()
      );
      assert!(
        !matches!(
          imported.as_str(),
          "Clone" | "Copy" | "Debug" | "Default" | "debug_assert" | "index_vec" | "matches"
        ),
        "{}: guarded derive and macro names must not be supplied through imports",
        source.display()
      );
      if imported == "Pass" {
        assert_eq!(
          prefix,
          &["rolldown_utils", "pass", "Pass"],
          "{}: the Pass trait must be imported directly from `rolldown_utils::pass::Pass`",
          source.display()
        );
      }
      prefix.pop();
    }
    UseTree::Rename(rename) => {
      let original = normalized_ident(&rename.ident);
      let alias = normalized_ident(&rename.rename);
      assert!(
        original != "Pass" && alias != "Pass",
        "{}: renamed Pass imports are forbidden because they bypass the inventory",
        source.display()
      );
      assert!(
        !FORBIDDEN_CARRIERS.contains(&original.as_str())
          && !FORBIDDEN_CARRIERS.contains(&alias.as_str()),
        "{}: broad carriers must not be supplied through renamed imports (`{original}` as `{alias}`)",
        source.display()
      );
      assert!(
        !["Clone", "Copy", "Debug", "Default", "debug_assert", "index_vec", "matches"]
          .contains(&original.as_str())
          && !["Clone", "Copy", "Debug", "Default", "debug_assert", "index_vec", "matches"]
            .contains(&alias.as_str()),
        "{}: guarded derive and macro names must not be supplied through renamed imports",
        source.display()
      );
    }
    UseTree::Glob(_) => {
      panic!(
        "{}: glob imports are forbidden in the pass subtree because they can hide the Pass trait",
        source.display()
      );
    }
    UseTree::Group(group) => {
      for item in &group.items {
        inspect_use_tree(item, prefix, source);
      }
    }
  }
}

struct InventoryVisitor<'a> {
  source: &'a Path,
  declarations: &'a mut BTreeSet<String>,
  implementations: &'a mut BTreeSet<String>,
  allow_wasm_iterator_ext_cfg: bool,
}

fn is_wasm_iterator_ext_cfg(attribute: &Attribute) -> bool {
  matches!(
    &attribute.meta,
    syn::Meta::List(meta)
      if meta.path.is_ident("cfg") && meta.tokens.to_string() == "target_family = \"wasm\""
  )
}

fn is_native_indexed_parallel_iterator_cfg(attribute: &Attribute) -> bool {
  matches!(
    &attribute.meta,
    syn::Meta::List(meta)
      if meta.path.is_ident("cfg")
        && meta.tokens.to_string() == "not (target_family = \"wasm\")"
  )
}

fn is_wasm_iterator_ext_use(item: &syn::ItemUse) -> bool {
  let UseTree::Path(root) = &item.tree else { return false };
  let UseTree::Path(module) = &*root.tree else { return false };
  let UseTree::Rename(extension) = &*module.tree else { return false };
  matches!(item.vis, Visibility::Inherited)
    && item.leading_colon.is_none()
    && root.ident == "rolldown_utils"
    && module.ident == "rayon"
    && extension.ident == "IteratorExt"
    && extension.rename == "_"
}

fn is_native_indexed_parallel_iterator_use(item: &syn::ItemUse) -> bool {
  let UseTree::Path(root) = &item.tree else { return false };
  let UseTree::Path(module) = &*root.tree else { return false };
  let UseTree::Name(extension) = &*module.tree else { return false };
  matches!(item.vis, Visibility::Inherited)
    && item.leading_colon.is_none()
    && root.ident == "rolldown_utils"
    && module.ident == "rayon"
    && extension.ident == "IndexedParallelIterator"
}

fn inspect_attribute(attribute: &Attribute, source: &Path) {
  if attribute.path().is_ident("doc") {
    return;
  }

  if attribute.path().is_ident("derive") {
    let derives = attribute
      .parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
      .unwrap_or_else(|error| {
        panic!("{}: failed to parse derive attribute: {error}", source.display())
      });
    assert!(!derives.is_empty(), "{}: empty derive attributes are forbidden", source.display());
    for derive in derives {
      assert!(
        ["Clone", "Copy", "Debug", "Default"].iter().any(|allowed| is_plain_path(&derive, allowed)),
        "{}: only the built-in Clone, Copy, Debug, and Default derives are allowed in the pass subtree",
        source.display()
      );
    }
    return;
  }

  panic!(
    "{}: attribute `{}` is forbidden in production pass code because an attribute macro can hide declarations or implementations",
    source.display(),
    attribute
      .path()
      .segments
      .last()
      .map_or("<unknown>".into(), |segment| segment.ident.to_string())
  );
}

fn inspect_file_attributes(attributes: &[Attribute], source: &Path, is_root_module: bool) {
  let mut saw_forbid_unsafe = false;
  for attribute in attributes {
    if attribute.path().is_ident("doc") {
      continue;
    }
    let is_forbid_unsafe = is_root_module
      && matches!(
        &attribute.meta,
        syn::Meta::List(meta)
          if meta.path.is_ident("forbid") && meta.tokens.to_string() == "unsafe_code"
      );
    assert!(
      is_forbid_unsafe,
      "{}: only documentation and the root `#![forbid(unsafe_code)]` inner attribute are allowed",
      source.display()
    );
    assert!(!saw_forbid_unsafe, "{}: duplicate `#![forbid(unsafe_code)]`", source.display());
    saw_forbid_unsafe = true;
  }
  if is_root_module {
    assert!(
      saw_forbid_unsafe,
      "{}: the link pass subtree must retain `#![forbid(unsafe_code)]`",
      source.display()
    );
  }
}

fn inspect_macro_tokens(tokens: proc_macro2::TokenStream, source: &Path) {
  let tokens = tokens.into_iter().collect::<Vec<_>>();
  for (index, token) in tokens.iter().enumerate() {
    match token {
      proc_macro2::TokenTree::Group(group) => inspect_macro_tokens(group.stream(), source),
      proc_macro2::TokenTree::Ident(ident) => {
        let ident = ident.to_string();
        let ident = ident.strip_prefix("r#").unwrap_or(&ident);
        assert!(
          !FORBIDDEN_CARRIERS.contains(&ident),
          "{}: allowed macro arguments must not name the broad carrier `{ident}`",
          source.display()
        );
      }
      proc_macro2::TokenTree::Punct(punct)
        if punct.as_char() == '!'
          && index
            .checked_sub(1)
            .and_then(|previous| tokens.get(previous))
            .is_some_and(|previous| std::matches!(previous, proc_macro2::TokenTree::Ident(_)))
          && tokens
            .get(index + 1)
            .is_some_and(|next| std::matches!(next, proc_macro2::TokenTree::Group(_))) =>
      {
        panic!(
          "{}: nested macro calls are forbidden inside allowed production macros",
          source.display()
        );
      }
      proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => {}
    }
  }
}

fn is_legacy_css_import_invariant(mac: &syn::Macro, source: &Path) -> bool {
  if !["determine_module_formats.rs", "reference_needed_symbols.rs"]
    .iter()
    .any(|file| source.file_name().is_some_and(|name| name == *file))
    || !is_exact_path(&mac.path, &["std", "unreachable"])
  {
    return false;
  }
  syn::parse2::<syn::LitStr>(mac.tokens.clone()).is_ok_and(|message| {
    std::matches!(
      message.value().as_str(),
      "A Js module would never import a CSS module via `@import`"
        | "A Js module would never import a CSS module via `url()`"
    )
  })
}

fn is_bind_imports_macro(mac: &syn::Macro, source: &Path) -> bool {
  source.ends_with(Path::new("passes/bind_imports.rs"))
    && [
      &["tracing", "trace_span"][..],
      &["tracing", "trace"][..],
      &["std", "assert_eq"][..],
      &["std", "format"][..],
      &["std", "unreachable"][..],
    ]
    .iter()
    .any(|expected| is_exact_path(&mac.path, expected))
}

fn is_layout_invariant_macro(mac: &syn::Macro, source: &Path) -> bool {
  let existing = (source.ends_with(Path::new("passes/compute_cjs_routing.rs"))
    || source.ends_with(Path::new("passes/resolve_member_expressions.rs")))
    && is_exact_path(&mac.path, &["std", "assert_eq"]);
  let new_pass = [
    "passes/collect_entry_export_roots.rs",
    "passes/create_synthetic_export_statements.rs",
    "passes/cross_module_optimization.rs",
    "passes/reference_needed_symbols.rs",
  ]
  .iter()
  .any(|path| source.ends_with(Path::new(path)))
    && (is_exact_path(&mac.path, &["std", "assert"])
      || is_exact_path(&mac.path, &["std", "assert_eq"]));
  existing || new_pass
}

fn is_new_pass_unreachable_invariant(mac: &syn::Macro, source: &Path) -> bool {
  if !is_exact_path(&mac.path, &["std", "unreachable"]) {
    return false;
  }
  if source.ends_with(Path::new("passes/collect_entry_export_roots.rs")) {
    return syn::parse2::<syn::LitStr>(mac.tokens.clone()).is_ok_and(|message| {
      message.value() == "single dynamic-import usage must be merged before Link"
    });
  }
  let expected = if source.ends_with(Path::new("passes/create_synthetic_export_statements.rs")) {
    &["validated normal modules must have missing-export shim slots"][..]
  } else if source.ends_with(Path::new("passes/reference_needed_symbols.rs")) {
    &[
      "CallRuntimeRequire patches must target normal modules",
      "validated normal modules must have owner-local symbol databases",
    ][..]
  } else {
    return false;
  };
  syn::parse2::<syn::LitStr>(mac.tokens.clone())
    .is_ok_and(|message| expected.contains(&message.value().as_str()))
}

impl<'ast> Visit<'ast> for InventoryVisitor<'_> {
  fn visit_item(&mut self, item: &'ast Item) {
    if let Item::Mod(module) = item
      && module.attrs.iter().any(|attribute| {
        let syn::Meta::List(meta) = &attribute.meta else { return false };
        meta.path.is_ident("cfg") && meta.tokens.to_string() == "test"
      })
    {
      return;
    }

    let previous_allow_wasm_iterator_ext_cfg = self.allow_wasm_iterator_ext_cfg;
    self.allow_wasm_iterator_ext_cfg = std::matches!(
      item,
      Item::Use(item)
        if item.attrs.len() == 1
          && ((is_wasm_iterator_ext_cfg(&item.attrs[0]) && is_wasm_iterator_ext_use(item))
            || (is_native_indexed_parallel_iterator_cfg(&item.attrs[0])
              && is_native_indexed_parallel_iterator_use(item)))
    );

    reject_named_non_struct_pass(item, self.source);
    match item {
      Item::Struct(item) if item.ident.to_string().ends_with("Pass") => {
        assert!(
          is_pub_super(&item.vis),
          "{}: `{}` must be declared `pub(super)`",
          self.source.display(),
          item.ident
        );
        assert!(
          item.generics.params.is_empty() && item.generics.where_clause.is_none(),
          "{}: `{}` must not be generic",
          self.source.display(),
          item.ident
        );
        assert!(
          matches!(item.fields, Fields::Unit),
          "{}: `{}` must be a unit struct, not tuple or braced state",
          self.source.display(),
          item.ident
        );
        assert!(
          self.declarations.insert(item.ident.to_string()),
          "{}: duplicate pass declaration `{}`",
          self.source.display(),
          item.ident
        );
      }
      Item::Impl(item) => {
        if let Some((negative, trait_path, _)) = &item.trait_ {
          assert!(
            negative.is_none(),
            "{}: negative trait impls are forbidden in the pass subtree",
            self.source.display()
          );
          assert!(
            is_plain_path(trait_path, "Pass"),
            "{}: every explicit trait implementation in the pass subtree must be the unqualified harness `Pass`; this prevents re-exported aliases from hiding passes",
            self.source.display()
          );
          assert!(
            item.generics.params.is_empty() && item.generics.where_clause.is_none(),
            "{}: Pass implementations must not be generic",
            self.source.display()
          );
          let name = plain_self_type(&item.self_ty).unwrap_or_else(|| {
            panic!(
              "{}: Pass implementations must target an unqualified concrete unit struct",
              self.source.display()
            )
          });
          assert!(
            name.ends_with("Pass"),
            "{}: Pass implementation target `{name}` must end in `Pass`",
            self.source.display()
          );
          assert!(
            self.implementations.insert(name.clone()),
            "{}: duplicate Pass implementation for `{name}`",
            self.source.display()
          );
        }
      }
      Item::Macro(item) => {
        panic!(
          "{}: item macro `{}` is forbidden in the pass subtree because the inventory must see every declaration",
          self.source.display(),
          item
            .mac
            .path
            .segments
            .last()
            .map_or("<unknown>".into(), |segment| segment.ident.to_string())
        );
      }
      Item::Use(item) => {
        inspect_use_tree(&item.tree, &mut Vec::new(), self.source);
      }
      Item::ExternCrate(_) => {
        panic!(
          "{}: extern-crate declarations are forbidden in the pass subtree because they can introduce guarded macro names",
          self.source.display()
        );
      }
      _ => {}
    }

    visit::visit_item(self, item);
    self.allow_wasm_iterator_ext_cfg = previous_allow_wasm_iterator_ext_cfg;
  }

  fn visit_stmt_macro(&mut self, statement: &'ast syn::StmtMacro) {
    assert!(
      is_exact_path(&statement.mac.path, &["std", "debug_assert"])
        || is_bind_imports_macro(&statement.mac, self.source)
        || is_layout_invariant_macro(&statement.mac, self.source)
        || is_new_pass_unreachable_invariant(&statement.mac, self.source)
        || is_legacy_css_import_invariant(&statement.mac, self.source),
      "{}: block-level statement macro `{}` is forbidden because it can generate a hidden pass declaration",
      self.source.display(),
      statement
        .mac
        .path
        .segments
        .last()
        .map_or("<unknown>".into(), |segment| segment.ident.to_string())
    );
    inspect_macro_tokens(statement.mac.tokens.clone(), self.source);
    visit::visit_stmt_macro(self, statement);
  }

  fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
    assert!(
      is_exact_path(&expression.mac.path, &["std", "matches"])
        || is_exact_path(&expression.mac.path, &["oxc_index", "index_vec"])
        || is_bind_imports_macro(&expression.mac, self.source)
        || is_layout_invariant_macro(&expression.mac, self.source)
        || is_new_pass_unreachable_invariant(&expression.mac, self.source)
        || is_legacy_css_import_invariant(&expression.mac, self.source),
      "{}: expression macro `{}` is not in the closed production allowlist",
      self.source.display(),
      expression
        .mac
        .path
        .segments
        .last()
        .map_or("<unknown>".into(), |segment| segment.ident.to_string())
    );
    inspect_macro_tokens(expression.mac.tokens.clone(), self.source);
    visit::visit_expr_macro(self, expression);
  }

  fn visit_type_macro(&mut self, ty: &'ast syn::TypeMacro) {
    panic!(
      "{}: type macro `{}` is forbidden because it can hide a broad pass carrier",
      self.source.display(),
      ty.mac.path.segments.last().map_or("<unknown>".into(), |segment| segment.ident.to_string())
    );
  }

  fn visit_attribute(&mut self, attribute: &'ast Attribute) {
    if self.allow_wasm_iterator_ext_cfg
      && (is_wasm_iterator_ext_cfg(attribute) || is_native_indexed_parallel_iterator_cfg(attribute))
    {
      return;
    }
    inspect_attribute(attribute, self.source);
  }

  fn visit_path(&mut self, path: &'ast syn::Path) {
    for segment in &path.segments {
      assert!(
        !FORBIDDEN_CARRIERS.iter().any(|forbidden| segment.ident == forbidden),
        "{}: pass slots and implementation code must not name the broad carrier `{}`",
        self.source.display(),
        segment.ident
      );
    }
    visit::visit_path(self, path);
  }
}

fn inspect_items(
  items: &[Item],
  source: &Path,
  declarations: &mut BTreeSet<String>,
  implementations: &mut BTreeSet<String>,
) {
  let mut visitor =
    InventoryVisitor { source, declarations, implementations, allow_wasm_iterator_ext_cfg: false };
  for item in items {
    visitor.visit_item(item);
  }
}

fn terminal_type_ident(ty: &Type) -> Option<String> {
  match ty {
    Type::Path(path) if path.qself.is_none() => {
      path.path.segments.last().map(|segment| normalized_ident(&segment.ident))
    }
    Type::Reference(reference) => terminal_type_ident(&reference.elem),
    _ => None,
  }
}

fn named_pattern_ident(pat: &syn::Pat) -> Option<String> {
  let syn::Pat::Ident(ident) = pat else { return None };
  (ident.by_ref.is_none() && ident.subpat.is_none()).then(|| normalized_ident(&ident.ident))
}

fn assert_complete_local_destructure(
  statement: &syn::Stmt,
  expected_type: &str,
  expected_fields: &[&str],
  expected_initializer: &str,
) {
  let syn::Stmt::Local(local) = statement else {
    panic!("the `{expected_type}` consumption must be a local destructure");
  };
  let syn::Pat::Struct(pattern) = &local.pat else {
    panic!("the `{expected_type}` consumption must use a struct pattern");
  };
  assert!(
    is_plain_path(&pattern.path, expected_type),
    "expected `{expected_type}` destructure, got a different path"
  );
  assert!(pattern.rest.is_none(), "`{expected_type}` must be destructured without `..`");
  let fields = pattern
    .fields
    .iter()
    .map(|field| {
      assert!(
        field.colon_token.is_none(),
        "`{expected_type}` fields must keep their names instead of hiding state behind aliases"
      );
      let syn::Member::Named(name) = &field.member else {
        panic!("`{expected_type}` must use named fields");
      };
      assert_eq!(
        named_pattern_ident(&field.pat).as_deref(),
        Some(normalized_ident(name).as_str()),
        "`{expected_type}` field patterns must be plain bindings"
      );
      normalized_ident(name)
    })
    .collect::<BTreeSet<_>>();
  assert_eq!(
    fields,
    expected_fields.iter().map(|field| (*field).to_owned()).collect::<BTreeSet<_>>(),
    "`{expected_type}` must be destructured completely"
  );
  let initializer = local.init.as_ref().expect("the destructure must have an initializer");
  let syn::Expr::Path(initializer) = &*initializer.expr else {
    panic!("the `{expected_type}` initializer must be a plain local");
  };
  assert!(
    is_plain_path(&initializer.path, expected_initializer),
    "the `{expected_type}` destructure must consume `{expected_initializer}`"
  );
}

fn is_rolldown_pass_prefix(prefix: &[String]) -> bool {
  prefix.len() == 2 && prefix[0] == "rolldown_utils" && prefix[1] == "pass"
}

fn collect_pass_entry_imports(
  tree: &UseTree,
  prefix: &mut Vec<String>,
  entries: &mut Vec<(String, String)>,
  globbed: &mut bool,
) {
  match tree {
    UseTree::Path(path) => {
      prefix.push(normalized_ident(&path.ident));
      collect_pass_entry_imports(&path.tree, prefix, entries, globbed);
      prefix.pop();
    }
    UseTree::Name(name) if is_rolldown_pass_prefix(prefix) => {
      let name = normalized_ident(&name.ident);
      entries.push((name.clone(), name));
    }
    UseTree::Rename(rename) if is_rolldown_pass_prefix(prefix) => {
      entries.push((normalized_ident(&rename.ident), normalized_ident(&rename.rename)));
    }
    UseTree::Group(group) => {
      for item in &group.items {
        collect_pass_entry_imports(item, prefix, entries, globbed);
      }
    }
    UseTree::Glob(_) if is_rolldown_pass_prefix(prefix) => *globbed = true,
    UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {}
  }
}

#[derive(Default)]
struct SelfValueVisitor {
  paths: usize,
}

impl<'ast> Visit<'ast> for SelfValueVisitor {
  fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
    if is_plain_path(&expression.path, "self") {
      self.paths += 1;
    }
    visit::visit_expr_path(self, expression);
  }
}

#[derive(Default)]
struct InfallibleBoundaryVisitor {
  try_expressions: usize,
  unwrap_or_expect_calls: BTreeSet<String>,
  panic_paths: BTreeSet<String>,
  error_types: BTreeSet<String>,
  non_plain_pass_entry_calls: BTreeSet<String>,
  direct_run_pass_calls: usize,
  infallible_pass_calls: usize,
}

impl InfallibleBoundaryVisitor {
  fn assert_no_error_adapter(&self, boundary: &str) {
    assert_eq!(self.try_expressions, 0, "{boundary} must not use `?` to adapt an error");
    assert!(
      self.unwrap_or_expect_calls.is_empty(),
      "{boundary} must not adapt an error with {:?}",
      self.unwrap_or_expect_calls
    );
    assert!(
      self.panic_paths.is_empty(),
      "{boundary} must not adapt an error through a panic path: {:?}",
      self.panic_paths
    );
    assert!(
      self.error_types.is_empty(),
      "{boundary} must not introduce a Result error channel: {:?}",
      self.error_types
    );
    assert!(
      self.non_plain_pass_entry_calls.is_empty(),
      "{boundary} must call pass entries by their exact unqualified names: {:?}",
      self.non_plain_pass_entry_calls
    );
  }
}

impl<'ast> Visit<'ast> for InfallibleBoundaryVisitor {
  fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
    self.try_expressions += 1;
    visit::visit_expr_try(self, expression);
  }

  fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
    let method = normalized_ident(&expression.method);
    if matches!(method.as_str(), "unwrap" | "expect") {
      self.unwrap_or_expect_calls.insert(method);
    }
    visit::visit_expr_method_call(self, expression);
  }

  fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
    if let syn::Expr::Path(function) = &*expression.func {
      if let Some(name) =
        function.path.segments.last().map(|segment| normalized_ident(&segment.ident))
      {
        match name.as_str() {
          "panic" | "panic_any" | "abort" => {
            self.panic_paths.insert(name);
          }
          "run_pass" | "run_infallible_pass" => {
            if function.qself.is_none() && is_plain_path(&function.path, &name) {
              if name == "run_pass" {
                self.direct_run_pass_calls += 1;
              } else {
                self.infallible_pass_calls += 1;
              }
            } else {
              self.non_plain_pass_entry_calls.insert(
                function
                  .path
                  .segments
                  .iter()
                  .map(|segment| normalized_ident(&segment.ident))
                  .collect::<Vec<_>>()
                  .join("::"),
              );
            }
          }
          _ => {}
        }
      }
    }
    visit::visit_expr_call(self, expression);
  }

  fn visit_macro(&mut self, mac: &'ast syn::Macro) {
    if let Some(name) = mac.path.segments.last().map(|segment| normalized_ident(&segment.ident)) {
      if matches!(
        name.as_str(),
        "panic"
          | "assert"
          | "assert_eq"
          | "assert_ne"
          | "debug_assert"
          | "debug_assert_eq"
          | "debug_assert_ne"
          | "unreachable"
          | "todo"
          | "unimplemented"
      ) {
        self.panic_paths.insert(name);
      }
    }
    visit::visit_macro(self, mac);
  }

  fn visit_path(&mut self, path: &'ast syn::Path) {
    for segment in &path.segments {
      match normalized_ident(&segment.ident).as_str() {
        "Result" | "BuildResult" => {
          self.error_types.insert(normalized_ident(&segment.ident));
        }
        _ => {}
      }
    }
    visit::visit_path(self, path);
  }
}

#[derive(Default)]
struct FacadePathVisitor {
  paths: usize,
}

impl<'ast> Visit<'ast> for FacadePathVisitor {
  fn visit_path(&mut self, path: &'ast syn::Path) {
    self.paths += path
      .segments
      .iter()
      .filter(|segment| normalized_ident(&segment.ident) == "LinkStage")
      .count();
    visit::visit_path(self, path);
  }
}

#[derive(Default)]
struct LegacyAdapterAssemblyVisitor {
  correct_dependencies_assignments: usize,
  correct_load_dependencies_assignments: usize,
  dependencies_assignments: usize,
  load_dependencies_assignments: usize,
  metadata_defaults: usize,
  metadata_pushes: usize,
  multizip_calls: usize,
}

impl<'ast> Visit<'ast> for LegacyAdapterAssemblyVisitor {
  fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) {
    if let syn::Expr::Field(field) = &*expression.left
      && matches!(&*field.base, syn::Expr::Path(path) if is_plain_path(&path.path, "meta"))
      && let syn::Member::Named(member) = &field.member
    {
      match normalized_ident(member).as_str() {
        "dependencies" => {
          self.dependencies_assignments += 1;
          self.correct_dependencies_assignments += usize::from(
            matches!(&*expression.right, syn::Expr::Path(path) if is_plain_path(&path.path, "dependencies")),
          );
        }
        "load_dependencies" => {
          self.load_dependencies_assignments += 1;
          self.correct_load_dependencies_assignments += usize::from(
            matches!(&*expression.right, syn::Expr::Path(path) if is_plain_path(&path.path, "load_dependencies")),
          );
        }
        _ => {}
      }
    }
    visit::visit_expr_assign(self, expression);
  }

  fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
    if let syn::Expr::Path(function) = &*expression.func {
      if is_exact_path(&function.path, &["LinkingMetadata", "default"]) {
        self.metadata_defaults += 1;
      } else if function
        .path
        .segments
        .last()
        .is_some_and(|segment| normalized_ident(&segment.ident) == "multizip")
      {
        self.multizip_calls += 1;
      }
    }
    visit::visit_expr_call(self, expression);
  }

  fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
    if normalized_ident(&expression.method) == "push"
      && matches!(&*expression.receiver, syn::Expr::Path(path) if is_plain_path(&path.path, "metas"))
    {
      self.metadata_pushes += 1;
    }
    visit::visit_expr_method_call(self, expression);
  }
}

#[test]
fn link_stage_is_a_two_field_one_shot_facade() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stages/link_stage");
  let source = root.join("mod.rs");
  let text = fs::read_to_string(&source)
    .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
  let file = syn::parse_file(&text)
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source.display()));

  let mut pass_entry_imports = Vec::new();
  let mut globbed_pass_entries = false;
  for item in &file.items {
    if let Item::Use(item) = item {
      collect_pass_entry_imports(
        &item.tree,
        &mut Vec::new(),
        &mut pass_entry_imports,
        &mut globbed_pass_entries,
      );
    }
  }
  assert!(!globbed_pass_entries, "the Link driver must not glob-import pass entries");
  let guarded_entries = pass_entry_imports
    .into_iter()
    .filter(|(source, local)| {
      matches!(source.as_str(), "run_pass" | "run_infallible_pass")
        || matches!(local.as_str(), "run_pass" | "run_infallible_pass")
    })
    .collect::<Vec<_>>();
  assert_eq!(
    guarded_entries,
    [("run_infallible_pass".to_owned(), "run_infallible_pass".to_owned())],
    "the Link driver must import the infallible entry by its exact name and no fallible entry"
  );

  let top_level_structs = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Struct(item) => Some(normalized_ident(&item.ident)),
      _ => None,
    })
    .collect::<BTreeSet<_>>();
  assert_eq!(
    top_level_structs,
    BTreeSet::from([
      "LinkStage".to_owned(),
      "LinkStageOutput".to_owned(),
      "SafelyMergeCjsNsInfo".to_owned(),
    ]),
    "the driver module must not introduce a replacement state carrier"
  );
  assert!(
    file.items.iter().all(|item| !matches!(item, Item::Fn(_))),
    "driver helpers must be passes with typed slots, not free functions over replacement state"
  );

  let stage = file
    .items
    .iter()
    .find_map(|item| match item {
      Item::Struct(item) if item.ident == "LinkStage" => Some(item),
      _ => None,
    })
    .expect("LinkStage declaration");
  let Fields::Named(stage_fields) = &stage.fields else {
    panic!("LinkStage must use named fields")
  };
  assert_eq!(stage_fields.named.len(), 2, "LinkStage must remain a two-field facade");
  let stage_field_types = stage_fields
    .named
    .iter()
    .map(|field| {
      (
        normalized_ident(field.ident.as_ref().expect("named LinkStage field")),
        terminal_type_ident(&field.ty).expect("plain LinkStage field type"),
      )
    })
    .collect::<BTreeMap<_, _>>();
  assert_eq!(
    stage_field_types,
    BTreeMap::from([
      ("options".to_owned(), "SharedOptions".to_owned()),
      ("scan_stage_output".to_owned(), "NormalizedScanStageOutput".to_owned()),
    ]),
    "LinkStage may retain only the untouched Scan output and options"
  );

  let stage_impls = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Impl(item)
        if item.trait_.is_none()
          && terminal_type_ident(&item.self_ty).as_deref() == Some("LinkStage") =>
      {
        Some(item)
      }
      _ => None,
    })
    .collect::<Vec<_>>();
  assert_eq!(stage_impls.len(), 1, "LinkStage must have one guarded inherent implementation");
  let stage_impl = stage_impls[0];
  let methods = stage_impl
    .items
    .iter()
    .filter_map(|item| match item {
      syn::ImplItem::Fn(method) => Some((normalized_ident(&method.sig.ident), method)),
      _ => None,
    })
    .collect::<BTreeMap<_, _>>();
  assert_eq!(
    methods.keys().cloned().collect::<BTreeSet<_>>(),
    BTreeSet::from(["link".to_owned(), "new".to_owned()]),
    "LinkStage must not grow driver helpers or stateful methods"
  );
  let link = methods["link"];
  assert!(
    matches!(link.sig.inputs.first(), Some(syn::FnArg::Receiver(receiver)) if receiver.reference.is_none() && receiver.mutability.is_none()),
    "LinkStage::link must consume the facade by value"
  );
  let syn::ReturnType::Type(_, return_type) = &link.sig.output else {
    panic!("LinkStage::link must keep the existing tuple return type");
  };
  let Type::Tuple(return_tuple) = &**return_type else {
    panic!("LinkStage::link must return the existing tuple directly");
  };
  let returned_types = return_tuple
    .elems
    .iter()
    .map(|ty| terminal_type_ident(ty).expect("plain LinkStage::link tuple element"))
    .collect::<Vec<_>>();
  assert_eq!(
    returned_types,
    ["LinkStageOutput", "IndexEcmaAst", "UsedSymbolRefsBuilder"],
    "LinkStage::link must preserve its infallible boundary tuple"
  );
  let mut infallible_boundary = InfallibleBoundaryVisitor::default();
  infallible_boundary.visit_block(&link.block);
  infallible_boundary.assert_no_error_adapter("LinkStage::link");
  assert_eq!(
    infallible_boundary.direct_run_pass_calls, 0,
    "LinkStage::link must execute passes only through `run_infallible_pass`"
  );
  assert_eq!(
    infallible_boundary.infallible_pass_calls, 24,
    "LinkStage::link must execute every production pass through the exact infallible entry"
  );
  assert_complete_local_destructure(
    &link.block.stmts[0],
    "LinkStage",
    &["scan_stage_output", "options"],
    "self",
  );
  assert_complete_local_destructure(
    &link.block.stmts[1],
    "NormalizedScanStageOutput",
    &[
      "module_table",
      "index_ecma_ast",
      "stmt_infos",
      "entry_points",
      "symbol_ref_db",
      "runtime",
      "warnings",
      "dynamic_import_exports_usage_map",
      "overrode_preserve_entry_signature_map",
      "entry_point_to_reference_ids",
      "flat_options",
      "user_defined_entry_modules",
      "tla_module_count",
      "tla_keyword_span_map",
    ],
    "scan_stage_output",
  );
  let mut self_values = SelfValueVisitor::default();
  for statement in &link.block.stmts[1..] {
    self_values.visit_stmt(statement);
  }
  assert_eq!(self_values.paths, 0, "the pass driver must not retain or recover LinkStage");
}

#[test]
fn infallible_harness_and_bundle_boundary_do_not_adapt_errors() {
  let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let harness_source = crate_root.join("../rolldown_utils/src/pass.rs");
  let harness_text = fs::read_to_string(&harness_source)
    .unwrap_or_else(|error| panic!("failed to read {}: {error}", harness_source.display()));
  let harness_file = syn::parse_file(&harness_text)
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", harness_source.display()));
  let harness = harness_file
    .items
    .iter()
    .find_map(|item| match item {
      Item::Fn(function) if function.sig.ident == "run_infallible_pass" => Some(function),
      _ => None,
    })
    .expect("run_infallible_pass function");
  let Some(syn::GenericParam::Type(pass_type)) = harness.sig.generics.params.first() else {
    panic!("run_infallible_pass must have exactly one pass type parameter");
  };
  assert_eq!(harness.sig.generics.params.len(), 1, "the harness must have one type parameter");
  assert!(harness.sig.generics.where_clause.is_none(), "the harness must not add where bounds");
  assert_eq!(normalized_ident(&pass_type.ident), "P", "the harness pass parameter must be `P`");
  assert!(pass_type.default.is_none(), "the harness pass parameter must not have a default");
  let Some(syn::TypeParamBound::Trait(pass_bound)) = pass_type.bounds.first() else {
    panic!("run_infallible_pass must have one explicit Pass bound");
  };
  assert_eq!(pass_type.bounds.len(), 1, "the pass parameter must have one trait bound");
  assert!(
    pass_bound.paren_token.is_none()
      && matches!(pass_bound.modifier, syn::TraitBoundModifier::None)
      && pass_bound.lifetimes.is_none()
      && pass_bound.path.leading_colon.is_none()
      && pass_bound.path.segments.len() == 1
      && pass_bound.path.segments[0].ident == "Pass",
    "run_infallible_pass must use the exact unqualified Pass bound"
  );
  let pass_segment = &pass_bound.path.segments[0];
  let PathArguments::AngleBracketed(pass_arguments) = &pass_segment.arguments else {
    panic!("run_infallible_pass must bind the Pass error type");
  };
  assert!(
    pass_arguments.args.len() == 1
      && matches!(
        pass_arguments.args.first(),
        Some(syn::GenericArgument::AssocType(binding))
          if binding.ident == "Error"
            && binding.generics.is_none()
            && matches!(&binding.ty, Type::Path(path) if path.qself.is_none() && is_plain_path(&path.path, "Infallible"))
      ),
    "run_infallible_pass must require only `Pass<Error = Infallible>`"
  );
  let harness_parameters = harness
    .sig
    .inputs
    .iter()
    .map(|argument| {
      let syn::FnArg::Typed(argument) = argument else {
        panic!("run_infallible_pass must be a free function");
      };
      (
        named_pattern_ident(&argument.pat).expect("plain harness parameter"),
        terminal_type_ident(&argument.ty).expect("plain harness parameter type"),
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    harness_parameters,
    [
      ("pass".to_owned(), "P".to_owned()),
      ("pipeline".to_owned(), "PassPipelineCtx".to_owned()),
      ("read".to_owned(), "InputRead".to_owned()),
      ("owned".to_owned(), "InputOwned".to_owned()),
    ],
    "run_infallible_pass must preserve its exact input slots"
  );
  let Some(syn::FnArg::Typed(pipeline_parameter)) = harness.sig.inputs.iter().nth(1) else {
    unreachable!()
  };
  assert!(
    matches!(&*pipeline_parameter.ty, Type::Reference(reference) if reference.mutability.is_some()),
    "run_infallible_pass must borrow the pipeline context mutably"
  );
  assert_eq!(
    match &harness.sig.output {
      syn::ReturnType::Type(_, ty) => terminal_type_ident(ty),
      syn::ReturnType::Default => None,
    }
    .as_deref(),
    Some("PassOutput"),
    "run_infallible_pass must return the pass output directly"
  );
  let mut harness_boundary = InfallibleBoundaryVisitor::default();
  harness_boundary.visit_block(&harness.block);
  harness_boundary.assert_no_error_adapter("run_infallible_pass");
  assert_eq!(
    harness_boundary.direct_run_pass_calls, 1,
    "run_infallible_pass must delegate exactly once to the guarded fallible entry"
  );
  let [syn::Stmt::Expr(syn::Expr::Match(eliminate), None)] = harness.block.stmts.as_slice() else {
    panic!("run_infallible_pass must eliminate Infallible in one returned match");
  };
  assert!(
    matches!(
      &*eliminate.expr,
      syn::Expr::Call(call)
        if matches!(&*call.func, syn::Expr::Path(path) if path.qself.is_none() && is_plain_path(&path.path, "run_pass"))
    ),
    "run_infallible_pass must exhaustively match the guarded fallible entry directly"
  );
  assert_eq!(eliminate.arms.len(), 2, "the guarded result must have exactly Ok and Err arms");
  let err_arm = eliminate
    .arms
    .iter()
    .find(|arm| {
      matches!(&arm.pat, syn::Pat::TupleStruct(pattern) if is_plain_path(&pattern.path, "Err"))
    })
    .expect("run_infallible_pass Err arm");
  let syn::Pat::TupleStruct(err_pattern) = &err_arm.pat else { unreachable!() };
  let Some(syn::Pat::Ident(never_binding)) = err_pattern.elems.first() else {
    panic!("run_infallible_pass must bind exactly one Infallible value");
  };
  assert_eq!(err_pattern.elems.len(), 1, "the Err arm must bind one value");
  let syn::Expr::Match(never_match) = &*err_arm.body else {
    panic!("run_infallible_pass must eliminate Infallible with an exhaustive match");
  };
  assert!(never_match.arms.is_empty(), "the Infallible match must have no arms");
  assert!(
    matches!(&*never_match.expr, syn::Expr::Path(path) if is_plain_path(&path.path, &normalized_ident(&never_binding.ident))),
    "the empty match must consume the bound Infallible value"
  );

  let bundle_source = crate_root.join("src/bundle/bundle.rs");
  let bundle_text = fs::read_to_string(&bundle_source)
    .unwrap_or_else(|error| panic!("failed to read {}: {error}", bundle_source.display()));
  let bundle_file = syn::parse_file(&bundle_text)
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", bundle_source.display()));
  let bundle_methods = bundle_file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Impl(item) => Some(item),
      _ => None,
    })
    .flat_map(|item| &item.items)
    .filter_map(|item| match item {
      syn::ImplItem::Fn(method) if method.sig.ident == "bundle_up" => Some(method),
      _ => None,
    })
    .collect::<Vec<_>>();
  let [bundle_up] = bundle_methods.as_slice() else {
    panic!("bundle_up must have exactly one implementation");
  };
  let link_boundary = bundle_up
    .block
    .stmts
    .iter()
    .find_map(|statement| match statement {
      syn::Stmt::Local(local)
        if matches!(&local.pat, syn::Pat::Tuple(tuple) if tuple.elems.len() == 3) =>
      {
        local.init.as_ref().map(|init| &*init.expr)
      }
      _ => None,
    })
    .expect("bundle_up Link tuple binding");
  let syn::Expr::MethodCall(link_call) = link_boundary else {
    panic!("bundle_up must receive the Link tuple from a direct method call");
  };
  assert_eq!(normalized_ident(&link_call.method), "link", "bundle_up must call Link directly");
  assert!(link_call.args.is_empty(), "LinkStage::link takes no explicit arguments");
  let syn::Expr::Call(constructor_call) = &*link_call.receiver else {
    panic!("bundle_up must call `.link()` directly on `LinkStage::new(...)`");
  };
  assert!(
    matches!(&*constructor_call.func, syn::Expr::Path(path) if is_exact_path(&path.path, &["LinkStage", "new"])),
    "bundle_up must preserve the direct `LinkStage::new(...).link()` boundary"
  );
}

fn is_physical_module_iteration(expression: &syn::Expr, method: &str) -> bool {
  matches!(
    expression,
    syn::Expr::MethodCall(call)
      if normalized_ident(&call.method) == method
        && call.args.is_empty()
        && call.turbofish.is_none()
        && matches!(
          &*call.receiver,
          syn::Expr::Field(field)
            if matches!(&field.member, syn::Member::Named(member) if normalized_ident(member) == "modules")
              && matches!(&*field.base, syn::Expr::Path(path) if is_plain_path(&path.path, "module_table"))
        )
  )
}

fn assert_adapter_entry_root_move(finish: &syn::ImplItemFn) {
  let entry_root_loop = finish
    .block
    .stmts
    .iter()
    .find_map(|statement| {
      let syn::Stmt::Expr(syn::Expr::ForLoop(loop_expression), _) = statement else {
        return None;
      };
      matches!(
        &*loop_expression.expr,
        syn::Expr::MethodCall(call)
          if normalized_ident(&call.method) == "into_entries"
            && matches!(&*call.receiver, syn::Expr::Path(path) if is_plain_path(&path.path, "entry_export_roots"))
      )
      .then_some(loop_expression)
    })
    .expect("finish must consume entry roots");
  assert!(
    matches!(
      &*entry_root_loop.pat,
      syn::Pat::Tuple(pattern)
        if pattern.elems.len() == 2
          && matches!(&pattern.elems[0], syn::Pat::Ident(ident) if normalized_ident(&ident.ident) == "module_idx")
          && matches!(&pattern.elems[1], syn::Pat::Ident(ident) if normalized_ident(&ident.ident) == "roots")
    ),
    "entry-root projection must bind exactly module_idx and roots"
  );
  assert_eq!(
    entry_root_loop.body.stmts.len(),
    1,
    "entry-root projection must contain only the direct move"
  );
  let direct_entry_root_move = matches!(
    &entry_root_loop.body.stmts[0],
    syn::Stmt::Expr(syn::Expr::Assign(assignment), _)
      if matches!(
        (&*assignment.left, &*assignment.right),
        (syn::Expr::Field(field), syn::Expr::Path(roots))
          if matches!(&field.member, syn::Member::Named(member) if normalized_ident(member) == "referenced_symbols_by_entry_point_chunk")
            && matches!(&*field.base, syn::Expr::Index(index)
              if matches!(&*index.expr, syn::Expr::Path(metas) if is_plain_path(&metas.path, "metas"))
                && matches!(&*index.index, syn::Expr::Path(module_idx) if is_plain_path(&module_idx.path, "module_idx")))
            && is_plain_path(&roots.path, "roots")
      )
  );
  assert!(direct_entry_root_move, "entry-root projection must directly move roots into metadata");
}

fn assert_adapter_direct_collect(local: &syn::Local) {
  let Some(init) = &local.init else {
    panic!("metadata assembly must initialize metas");
  };
  let syn::Expr::MethodCall(collect) = &*init.expr else {
    panic!("metadata assembly must end in a direct collect call");
  };
  assert_eq!(normalized_ident(&collect.method), "collect");
  let collect_target =
    collect.turbofish.as_ref().and_then(|arguments| arguments.args.first()).and_then(|argument| {
      match argument {
        syn::GenericArgument::Type(ty) => terminal_type_ident(ty),
        _ => None,
      }
    });
  assert_eq!(collect_target.as_deref(), Some("IndexVec"));
  let syn::Expr::MethodCall(map) = &*collect.receiver else {
    panic!("metadata collection must directly consume the assembly map");
  };
  assert_eq!(normalized_ident(&map.method), "map");
  assert!(
    map.args.len() == 1 && matches!(map.args.first(), Some(syn::Expr::Closure(_))),
    "metadata map must contain exactly one closure"
  );
  let syn::Expr::Macro(izip) = &*map.receiver else {
    panic!("metadata map must directly consume itertools::izip!");
  };
  assert!(
    is_exact_path(&izip.mac.path, &["itertools", "izip"]),
    "metadata assembly must use itertools::izip!"
  );
  let arguments = izip
    .mac
    .parse_body_with(Punctuated::<syn::Expr, Token![,]>::parse_terminated)
    .expect("metadata izip arguments must parse")
    .into_iter()
    .collect::<Vec<_>>();
  let expected = [
    "resolved_export_slots",
    "included_commonjs_export_symbol_slots",
    "member_expr_resolution_slots",
    "shimmed_missing_export_slots",
    "external_star_export_slots",
    "stmt_included_slots",
    "namespace_reason_slots",
    "dependency_slots",
    "load_dependency_slots",
    "runtime_requirement_slots",
  ];
  assert!(
    arguments.len() == 11
      && is_physical_module_iteration(&arguments[0], "iter_mut_enumerated")
      && arguments[1..]
        .iter()
        .zip(expected)
        .all(|(argument, expected)| matches!(argument, syn::Expr::Path(path) if is_plain_path(&path.path, expected))),
    "metadata izip must preserve the exact physical assembly inputs"
  );
}

#[test]
fn legacy_output_adapter_accepts_only_explicit_final_fields() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stages/link_stage");
  let source = root.join("legacy_output_adapter.rs");
  let text = fs::read_to_string(&source)
    .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
  let file = syn::parse_file(&text)
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source.display()));

  let mut facade_paths = FacadePathVisitor::default();
  facade_paths.visit_file(&file);
  assert_eq!(facade_paths.paths, 0, "the output adapter must not name the LinkStage facade");

  let top_level_structs = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Struct(item) => Some(normalized_ident(&item.ident)),
      _ => None,
    })
    .collect::<BTreeSet<_>>();
  assert_eq!(
    top_level_structs,
    BTreeSet::from(["LegacyOutputAdapter".to_owned()]),
    "the adapter module must not introduce an input or stage replacement carrier"
  );

  let adapter_impls = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Impl(item)
        if item.trait_.is_none()
          && terminal_type_ident(&item.self_ty).as_deref() == Some("LegacyOutputAdapter") =>
      {
        Some(item)
      }
      _ => None,
    })
    .collect::<Vec<_>>();
  assert_eq!(
    adapter_impls.len(),
    1,
    "LegacyOutputAdapter must have one guarded inherent implementation"
  );
  let adapter_impl = adapter_impls[0];
  let methods = adapter_impl
    .items
    .iter()
    .filter_map(|item| match item {
      syn::ImplItem::Fn(method) => Some(method),
      _ => None,
    })
    .collect::<Vec<_>>();
  assert_eq!(methods.len(), 1, "the output adapter must expose only finish");
  let finish = methods[0];
  assert_eq!(finish.sig.ident, "finish");
  let Some(syn::Stmt::Local(first_statement)) = finish.block.stmts.first() else {
    panic!("finish must start by destructuring LegacyOutputAdapter");
  };
  assert!(
    matches!(
      (&first_statement.pat, first_statement.init.as_ref().map(|init| &*init.expr)),
      (
        syn::Pat::Struct(pattern),
        Some(syn::Expr::Path(value)),
      ) if is_plain_path(&pattern.path, "LegacyOutputAdapter")
        && is_plain_path(&value.path, "self")
    ),
    "finish must immediately destructure LegacyOutputAdapter instead of retaining a carrier"
  );

  let deferred_patch_index = finish
    .block
    .stmts
    .iter()
    .position(|statement| {
      matches!(
        statement,
        syn::Stmt::Expr(syn::Expr::Call(call), _)
          if matches!(&*call.func, syn::Expr::Path(function) if is_plain_path(&function.path, "apply_deferred_module_patches"))
      )
    })
    .expect("finish must apply deferred patches");
  let (assembly_index, assembly_local) = finish
    .block
    .stmts
    .iter()
    .enumerate()
    .find_map(|(index, statement)| {
      let syn::Stmt::Local(local) = statement else { return None };
      matches!(
        &local.pat,
        syn::Pat::Ident(ident) if normalized_ident(&ident.ident) == "metas"
      )
      .then_some((index, local))
    })
    .expect("finish must bind one collected metadata IndexVec");
  let validation_loops = finish.block.stmts[..deferred_patch_index]
    .iter()
    .filter(|statement| matches!(statement, syn::Stmt::Expr(syn::Expr::ForLoop(_), _)))
    .count();
  assert_eq!(
    validation_loops, 2,
    "finish must validate dense counts and normal/external slot shapes before any output write"
  );
  assert!(
    deferred_patch_index < assembly_index,
    "finish must preserve deferred patch application before physical output assembly"
  );
  assert_adapter_direct_collect(assembly_local);
  assert_adapter_entry_root_move(finish);

  let mut assembly = LegacyAdapterAssemblyVisitor::default();
  assembly.visit_block(&finish.block);
  assert_eq!(
    assembly.metadata_defaults, 1,
    "finish must have one metadata default expression inside physical assembly"
  );
  assert_eq!(
    assembly.metadata_pushes, 0,
    "finish must collect freshly assembled metadata instead of pushing it"
  );
  assert_eq!(assembly.multizip_calls, 0, "finish must not restore the multizip assembly loop");
  assert_eq!(
    (assembly.dependencies_assignments, assembly.correct_dependencies_assignments),
    (1, 1),
    "metadata assembly must move the final dependency set into dependencies exactly once"
  );
  assert_eq!(
    (assembly.load_dependencies_assignments, assembly.correct_load_dependencies_assignments,),
    (1, 1),
    "metadata assembly must move the final load-dependency set into load_dependencies exactly once"
  );

  let parameters = finish
    .sig
    .inputs
    .iter()
    .filter_map(|argument| match argument {
      syn::FnArg::Receiver(_) => None,
      syn::FnArg::Typed(argument) => Some((
        named_pattern_ident(&argument.pat).expect("plain adapter parameter"),
        terminal_type_ident(&argument.ty).expect("explicit adapter parameter type"),
      )),
    })
    .collect::<Vec<_>>();
  assert_eq!(
    parameters,
    [
      ("module_table", "ModuleTable"),
      ("symbols", "SymbolRefDb"),
      ("stmt_infos", "IndexStmtInfos"),
      ("runtime", "RuntimeModuleBrief"),
      ("diagnostics", "Diagnostics"),
      ("ast_table", "IndexEcmaAst"),
      ("dynamic_import_exports_usage_map", "FxHashMap"),
      ("overrode_preserve_entry_signature_map", "FxHashMap"),
      ("entry_point_to_reference_ids", "FxHashMap"),
      ("user_defined_entry_modules", "FxHashSet"),
    ]
    .into_iter()
    .map(|(name, ty)| (name.to_owned(), ty.to_owned()))
    .collect::<Vec<_>>(),
    "finish must receive the final Scan-owned fields explicitly instead of a replacement carrier"
  );

  let helper_signatures = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Fn(item) => Some((
        normalized_ident(&item.sig.ident),
        item
          .sig
          .inputs
          .iter()
          .map(|argument| match argument {
            syn::FnArg::Receiver(_) => panic!("free adapter helper has a receiver"),
            syn::FnArg::Typed(argument) => {
              terminal_type_ident(&argument.ty).expect("explicit adapter helper parameter type")
            }
          })
          .collect::<Vec<_>>(),
      )),
      _ => None,
    })
    .collect::<BTreeMap<_, _>>();
  assert_eq!(
    helper_signatures,
    BTreeMap::from([
      (
        "apply_deferred_module_patches".to_owned(),
        vec![
          "ModuleTable".to_owned(),
          "TreeShakeModulePatches".to_owned(),
          "ReferenceImportRecordPatches".to_owned(),
        ],
      ),
      (
        "project_cjs_routing".to_owned(),
        vec!["ModuleTable".to_owned(), "IndexVec".to_owned(), "CjsRoutingFinal".to_owned()],
      ),
    ]),
    "adapter helpers must keep their explicit narrow signatures"
  );
}

fn require_entry_export_root_pair(
  root: super::collect_entry_export_roots::EntryExportRoot,
) -> (rolldown_common::SymbolRef, bool) {
  root
}

const _: fn(
  super::collect_entry_export_roots::EntryExportRoot,
) -> (rolldown_common::SymbolRef, bool) = require_entry_export_root_pair;

#[test]
fn entry_export_roots_keep_direct_legacy_storage() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stages/link_stage/passes");
  let source = root.join("collect_entry_export_roots.rs");
  let text = fs::read_to_string(&source)
    .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
  let file = syn::parse_file(&text)
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source.display()));

  let into_entries = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Impl(item)
        if item.trait_.is_none()
          && terminal_type_ident(&item.self_ty).as_deref() == Some("EntryExportRoots") =>
      {
        Some(item)
      }
      _ => None,
    })
    .flat_map(|implementation| &implementation.items)
    .filter_map(|item| match item {
      syn::ImplItem::Fn(method) if method.sig.ident == "into_entries" => Some(method),
      _ => None,
    })
    .collect::<Vec<_>>();
  let [into_entries] = into_entries.as_slice() else {
    panic!("EntryExportRoots must have exactly one into_entries method");
  };
  let [syn::Stmt::Expr(syn::Expr::MethodCall(call), None)] = into_entries.block.stmts.as_slice()
  else {
    panic!("EntryExportRoots::into_entries must directly return self.roots.into_iter()");
  };
  assert_eq!(normalized_ident(&call.method), "into_iter");
  assert!(call.args.is_empty(), "the direct into_iter call must not receive arguments");
  assert!(call.turbofish.is_none(), "the direct into_iter call must not change item types");
  assert!(
    matches!(
      &*call.receiver,
      syn::Expr::Field(field)
        if matches!(&field.member, syn::Member::Named(member) if normalized_ident(member) == "roots")
          && matches!(&*field.base, syn::Expr::Path(path) if is_plain_path(&path.path, "self"))
    ),
    "EntryExportRoots::into_entries must move the stored roots without conversion"
  );
}

fn use_tree_mentions_link(tree: &UseTree) -> bool {
  match tree {
    UseTree::Path(path) => {
      normalized_ident(&path.ident) == "link" || use_tree_mentions_link(&path.tree)
    }
    UseTree::Name(name) => normalized_ident(&name.ident) == "link",
    UseTree::Rename(rename) => {
      normalized_ident(&rename.ident) == "link" || normalized_ident(&rename.rename) == "link"
    }
    UseTree::Glob(_) => false,
    UseTree::Group(group) => group.items.iter().any(use_tree_mentions_link),
  }
}

fn macro_tokens_mention_link(tokens: proc_macro2::TokenStream) -> bool {
  let tokens = tokens.into_iter().collect::<Vec<_>>();
  for (index, token) in tokens.iter().enumerate() {
    match token {
      proc_macro2::TokenTree::Group(group) => {
        if macro_tokens_mention_link(group.stream()) {
          return true;
        }
      }
      proc_macro2::TokenTree::Ident(ident)
        if normalized_ident(ident) == "link"
          && index.checked_sub(1).and_then(|previous| tokens.get(previous)).is_some_and(
            |previous| {
              std::matches!(
                previous,
                proc_macro2::TokenTree::Punct(punct) if punct.as_char() == '.' || punct.as_char() == ':'
              )
            },
          ) =>
      {
        return true;
      }
      proc_macro2::TokenTree::Ident(_)
      | proc_macro2::TokenTree::Punct(_)
      | proc_macro2::TokenTree::Literal(_) => {}
    }
  }
  false
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ProductionLinkCallVisitor {
  method_calls: usize,
  path_references: usize,
  imports: usize,
  macro_tokens: usize,
}

impl<'ast> Visit<'ast> for ProductionLinkCallVisitor {
  fn visit_item(&mut self, item: &'ast Item) {
    if let Item::Mod(module) = item
      && module.attrs.iter().any(|attribute| {
        let syn::Meta::List(meta) = &attribute.meta else { return false };
        meta.path.is_ident("cfg") && meta.tokens.to_string() == "test"
      })
    {
      return;
    }
    if let Item::Use(item) = item
      && use_tree_mentions_link(&item.tree)
    {
      self.imports += 1;
    }
    visit::visit_item(self, item);
  }

  fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
    if normalized_ident(&expression.method) == "link" {
      self.method_calls += 1;
    }
    visit::visit_expr_method_call(self, expression);
  }

  fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
    if expression
      .path
      .segments
      .last()
      .is_some_and(|segment| normalized_ident(&segment.ident) == "link")
    {
      self.path_references += 1;
    }
    visit::visit_expr_path(self, expression);
  }

  fn visit_macro(&mut self, mac: &'ast syn::Macro) {
    if macro_tokens_mention_link(mac.tokens.clone()) {
      self.macro_tokens += 1;
    }
    visit::visit_macro(self, mac);
  }
}

#[test]
fn link_pass_inventory_is_complete() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stages/link_stage/passes");
  let mut declarations = BTreeSet::new();
  let mut implementations = BTreeSet::new();
  let sources = rust_sources(&root);
  let root_module = root.join("mod.rs");
  for source in &sources {
    let text = fs::read_to_string(source)
      .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
    let file = syn::parse_file(&text)
      .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source.display()));
    inspect_file_attributes(&file.attrs, source, source == &root_module);
    inspect_items(&file.items, source, &mut declarations, &mut implementations);
  }

  assert!(!declarations.is_empty(), "the link pass inventory must be non-vacuous");
  assert_eq!(
    declarations, implementations,
    "every declared link pass must have exactly one visible direct Pass implementation, and every implementation must have a matching unit declaration"
  );
}

#[test]
fn link_stage_symbol_links_end_in_bind_imports() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stages/link_stage");
  let mut calls = BTreeMap::new();
  for source in rust_sources(&root) {
    let text = fs::read_to_string(&source)
      .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
    let file = syn::parse_file(&text)
      .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source.display()));
    let mut visitor = ProductionLinkCallVisitor::default();
    for item in &file.items {
      visitor.visit_item(item);
    }
    if visitor != ProductionLinkCallVisitor::default() {
      calls.insert(source, visitor);
    }
  }

  assert_eq!(
    calls,
    BTreeMap::from([(
      root.join("passes/bind_imports.rs"),
      ProductionLinkCallVisitor { method_calls: 3, ..ProductionLinkCallVisitor::default() },
    )])
  );
}

#[test]
fn symbol_link_inventory_detects_non_method_forms() {
  for (source, expected_method_calls) in [
    (
      "fn bypass(db: &mut SymbolRefDb, from: SymbolRef, to: SymbolRef) { SymbolRefDb::link(db, from, to); }",
      0,
    ),
    (
      "fn bypass(db: &mut SymbolRefDb, from: SymbolRef, to: SymbolRef) { <SymbolRefDb>::link(db, from, to); }",
      0,
    ),
    ("fn bypass() { let connect = SymbolRefDb::link; connect; }", 0),
    ("use SymbolRefDb::link as connect; fn bypass() { connect(); }", 0),
    ("fn bypass() { forward!(symbols.link(from, to)); }", 0),
    ("fn bypass(db: &mut SymbolRefDb, from: SymbolRef, to: SymbolRef) { db.r#link(from, to); }", 1),
    (
      "fn bypass(db: &mut SymbolRefDb, from: SymbolRef, to: SymbolRef) { SymbolRefDb::r#link(db, from, to); }",
      0,
    ),
    ("use SymbolRefDb::r#link as connect; fn bypass() { connect(); }", 0),
  ] {
    let file =
      syn::parse_file(source).unwrap_or_else(|error| panic!("link bypass fixture: {error}"));
    let mut visitor = ProductionLinkCallVisitor::default();
    for item in &file.items {
      visitor.visit_item(item);
    }
    assert_ne!(visitor, ProductionLinkCallVisitor::default(), "missed link form: {source}");
    assert_eq!(visitor.method_calls, expected_method_calls, "wrong method-call count: {source}");
  }
}

#[test]
fn inventory_rejects_stateful_and_hidden_pass_shapes() {
  let invalid = [
    "pub(super) struct GenericPass<T>(T); impl<T> Pass for GenericPass<T> {}",
    "pub(super) struct TuplePass(u8); impl Pass for TuplePass {}",
    "pub(super) struct BracedPass { value: u8 } impl Pass for BracedPass {}",
    "pub(super) struct QualifiedPass; impl rolldown_utils::pass::Pass for QualifiedPass {}",
    "macro_rules! declare_pass { () => { pub(super) struct HiddenPass; } }",
    "use rolldown_utils::pass::Pass as P; struct Hidden; impl P for Hidden {}",
    "use external::PipelineStep; struct Hidden; impl PipelineStep for Hidden {}",
    "fn hide() { struct LocalPass; impl Pass for LocalPass {} }",
    "fn hide() { declare_pass!(); }",
    "use external::debug_assert; fn hide() { debug_assert!(true); }",
    "#[generate_pass] pub(super) struct HiddenPass;",
    "#[cfg(target_family = \"wasm\")] pub(super) struct HiddenPass;",
    "#[cfg(target_family = \"wasm32\")] use rolldown_utils::rayon::IteratorExt as _;",
    "#[cfg(target_family = \"wasm\")] use external::IteratorExt as _;",
    "#[cfg(target_family = \"wasm\")] pub use rolldown_utils::rayon::IteratorExt as _;",
    "#[cfg(target_family = \"wasm\")] use ::rolldown_utils::rayon::IteratorExt as _;",
    "#[cfg(not(target_family = \"wasm32\"))] use rolldown_utils::rayon::IndexedParallelIterator;",
    "#[cfg(not(target_family = \"wasm\"))] use external::IndexedParallelIterator;",
    "#[cfg(not(target_family = \"wasm\"))] pub use rolldown_utils::rayon::IndexedParallelIterator;",
    "#[cfg(target_family = \"wasm\")] use rolldown_utils::rayon::IndexedParallelIterator;",
    "#[derive(external::Pass)] pub(super) struct HiddenPass;",
    "use external::Debug; #[derive(Debug)] pub(super) struct HiddenPass;",
    "extern crate external as Debug; #[derive(Debug)] pub(super) struct HiddenPass;",
    "use super::super::LinkStage as State; pub(super) struct CarrierPass; impl Pass for CarrierPass { type InputRead<'a> = &'a State<'a>; }",
    "use super::super::LinkStage;",
    "use super::super::r#LinkStage;",
    "use super::super::r#LinkStage as State; pub(super) struct CarrierPass; impl Pass for CarrierPass { type InputRead<'a> = &'a State<'a>; }",
    "use external::State as r#LinkStage;",
    "use super::super::LinkStageOutput as Output;",
    "use crate::types::linking_metadata::LinkingMetadata as Metadata;",
    "use crate::types::linking_metadata::LinkingMetadataVec as MetadataVec;",
    "use rolldown_utils::pass::PassPipelineCtx as Context;",
    "use super::super::tree_shaking::inclusion_core::InclusionCoreContext;",
    "use super::super::tree_shaking::inclusion_core::InclusionFacts as Facts;",
    "use super::super::tree_shaking::inclusion_core::InclusionModuleFacts;",
    "pub(super) struct CarrierPass; impl Pass for CarrierPass { type InputRead<'a> = carrier_ty!(); }",
    "fn hide() { std::debug_assert!(std::mem::size_of::<LinkStage<'static>>() > 0); }",
    "fn hide() { std::debug_assert!(std::matches!(true, true)); }",
    "pub(super) struct CarrierPass; impl Pass for CarrierPass { type InputRead<'a> = &'a LinkStage; }",
  ];

  for source in invalid {
    let file =
      syn::parse_file(source).unwrap_or_else(|error| panic!("invalid test source: {error}"));
    let result = std::panic::catch_unwind(|| {
      let mut declarations = BTreeSet::new();
      let mut implementations = BTreeSet::new();
      inspect_items(&file.items, Path::new("invalid.rs"), &mut declarations, &mut implementations);
    });
    assert!(result.is_err(), "inventory accepted invalid source: {source}");
  }
}

#[test]
fn inventory_accepts_the_required_unit_shape() {
  let file = syn::parse_file(
    "use rolldown_utils::pass::Pass; #[derive(Clone, Copy)] pub(super) struct ExamplePass; impl Pass for ExamplePass { type Error = (); }",
  )
  .unwrap_or_else(|error| panic!("valid test source: {error}"));
  let mut declarations = BTreeSet::new();
  let mut implementations = BTreeSet::new();
  inspect_items(&file.items, Path::new("valid.rs"), &mut declarations, &mut implementations);
  assert_eq!(declarations, BTreeSet::from(["ExamplePass".to_string()]));
  assert_eq!(declarations, implementations);
}

#[test]
fn inventory_accepts_only_the_exact_conditional_parallel_iterator_imports() {
  let file = syn::parse_file(
    "#[cfg(target_family = \"wasm\")] use rolldown_utils::rayon::IteratorExt as _;\n\
     #[cfg(not(target_family = \"wasm\"))] use rolldown_utils::rayon::IndexedParallelIterator;",
  )
  .unwrap_or_else(|error| panic!("valid test source: {error}"));
  let mut declarations = BTreeSet::new();
  let mut implementations = BTreeSet::new();
  inspect_items(
    &file.items,
    Path::new("valid-wasm-import.rs"),
    &mut declarations,
    &mut implementations,
  );
  assert!(declarations.is_empty());
  assert!(implementations.is_empty());
}

#[test]
fn inventory_accepts_only_the_exact_legacy_css_import_invariants() {
  for source in [
    "fn invariant() { std::unreachable!(\"A Js module would never import a CSS module via `@import`\"); }",
    "fn invariant() { std::unreachable!(\"A Js module would never import a CSS module via `url()`\"); }",
  ] {
    let file = syn::parse_file(source).unwrap_or_else(|error| panic!("valid test source: {error}"));
    let mut declarations = BTreeSet::new();
    let mut implementations = BTreeSet::new();
    inspect_items(
      &file.items,
      Path::new("determine_module_formats.rs"),
      &mut declarations,
      &mut implementations,
    );
    assert!(declarations.is_empty());
    assert!(implementations.is_empty());
  }

  for source in [
    "fn invariant() { unreachable!(\"A Js module would never import a CSS module via `@import`\"); }",
    "fn invariant() { std::unreachable!(\"different invariant\"); }",
  ] {
    let file =
      syn::parse_file(source).unwrap_or_else(|error| panic!("invalid test source: {error}"));
    let result = std::panic::catch_unwind(|| {
      let mut declarations = BTreeSet::new();
      let mut implementations = BTreeSet::new();
      inspect_items(
        &file.items,
        Path::new("determine_module_formats.rs"),
        &mut declarations,
        &mut implementations,
      );
    });
    assert!(result.is_err(), "inventory accepted invalid source: {source}");
  }
}

#[test]
fn inventory_accepts_only_the_exact_bind_imports_macros() {
  let valid = syn::parse_file(
    r#"
fn observe() {
  let _span = tracing::trace_span!("binding");
  tracing::trace!("binding");
  std::assert_eq!(1, 1);
  let _message = std::format!("missing {}", "export");
  std::unreachable!("invariant");
}
"#,
  )
  .unwrap_or_else(|error| panic!("valid test source: {error}"));
  let mut declarations = BTreeSet::new();
  let mut implementations = BTreeSet::new();
  inspect_items(
    &valid.items,
    Path::new("passes/bind_imports.rs"),
    &mut declarations,
    &mut implementations,
  );
  assert!(declarations.is_empty());
  assert!(implementations.is_empty());

  for (source, path) in [
    ("fn observe() { trace!(\"binding\"); }", "passes/bind_imports.rs"),
    ("fn observe() { other::trace!(\"binding\"); }", "passes/bind_imports.rs"),
    ("fn observe() { tracing::error!(\"binding\"); }", "passes/bind_imports.rs"),
    ("fn observe() { assert_eq!(1, 1); }", "passes/bind_imports.rs"),
    ("fn observe() { tracing::trace!(\"binding\"); }", "passes/other.rs"),
    ("fn observe() { tracing::trace!(LinkStage); }", "passes/bind_imports.rs"),
    (
      "fn observe() { let _ = std::format!(\"{}\", std::matches!(true, true)); }",
      "passes/bind_imports.rs",
    ),
  ] {
    let file =
      syn::parse_file(source).unwrap_or_else(|error| panic!("invalid test source: {error}"));
    let result = std::panic::catch_unwind(|| {
      let mut declarations = BTreeSet::new();
      let mut implementations = BTreeSet::new();
      inspect_items(&file.items, Path::new(path), &mut declarations, &mut implementations);
    });
    assert!(result.is_err(), "inventory accepted invalid source: {source}");
  }
}
