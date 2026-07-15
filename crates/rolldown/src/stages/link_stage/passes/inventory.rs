use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
};

use syn::{
  Attribute, Fields, Item, PathArguments, Token, Type, UseTree, Visibility,
  punctuated::Punctuated,
  visit::{self, Visit},
};

const FORBIDDEN_CARRIERS: [&str; 5] =
  ["LinkStage", "LinkStageOutput", "LinkingMetadata", "LinkingMetadataVec", "PassPipelineCtx"];

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

fn inspect_use_tree(tree: &UseTree, prefix: &mut Vec<String>, source: &Path) {
  match tree {
    UseTree::Path(path) => {
      prefix.push(path.ident.to_string());
      inspect_use_tree(&path.tree, prefix, source);
      prefix.pop();
    }
    UseTree::Name(name) => {
      prefix.push(name.ident.to_string());
      assert!(
        !matches!(
          name.ident.to_string().as_str(),
          "Clone" | "Copy" | "Debug" | "Default" | "debug_assert" | "index_vec" | "matches"
        ),
        "{}: guarded derive and macro names must not be supplied through imports",
        source.display()
      );
      if name.ident == "Pass" {
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
      let original = rename.ident.to_string();
      let alias = rename.rename.to_string();
      assert!(
        original != "Pass" && alias != "Pass",
        "{}: renamed Pass imports are forbidden because they bypass the inventory",
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
  for token in tokens {
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
      proc_macro2::TokenTree::Punct(punct) if punct.as_char() == '!' => {
        panic!(
          "{}: nested macro calls are forbidden inside allowed production macros",
          source.display()
        );
      }
      proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => {}
    }
  }
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
  }

  fn visit_stmt_macro(&mut self, statement: &'ast syn::StmtMacro) {
    assert!(
      is_exact_path(&statement.mac.path, &["std", "debug_assert"]),
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
        || is_exact_path(&expression.mac.path, &["oxc_index", "index_vec"]),
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
  let mut visitor = InventoryVisitor { source, declarations, implementations };
  for item in items {
    visitor.visit_item(item);
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
    "#[derive(external::Pass)] pub(super) struct HiddenPass;",
    "use external::Debug; #[derive(Debug)] pub(super) struct HiddenPass;",
    "extern crate external as Debug; #[derive(Debug)] pub(super) struct HiddenPass;",
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
