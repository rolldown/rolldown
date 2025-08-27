use rustc_hash::{FxHashMap, FxHashSet};
use std::path::PathBuf;
use syn::{ImplItem, ItemImpl, Path, Type, TypePath, visit::Visit};

/// Tracks trait implementations and their methods
#[derive(Clone, Debug)]
pub struct TraitImplTracker {
  /// Maps (trait_name, type_name) -> methods
  /// e.g., ("BindingPatternExt", "BindingPattern") -> ["binding_identifiers", "into_assignment_target", ...]
  pub trait_impls: FxHashMap<(String, String), FxHashSet<String>>,

  /// Maps method_name -> trait_names that provide this method
  /// e.g., "binding_identifiers" -> ["BindingPatternExt"]
  pub method_to_traits: FxHashMap<String, FxHashSet<String>>,

  /// Tracks which traits are extension traits (impl for external types)
  pub extension_traits: FxHashSet<String>,
}

impl TraitImplTracker {
  pub fn new() -> Self {
    Self {
      trait_impls: FxHashMap::default(),
      method_to_traits: FxHashMap::default(),
      extension_traits: FxHashSet::default(),
    }
  }

  /// Analyze a file for trait implementations
  pub fn analyze_file(&mut self, file_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file_path)?;
    let file = syn::parse_file(&content)?;

    let mut visitor = TraitImplVisitor { tracker: self };

    visitor.visit_file(&file);
    Ok(())
  }

  /// Get all traits that provide a specific method
  pub fn get_traits_for_method(&self, method_name: &str) -> Option<&FxHashSet<String>> {
    self.method_to_traits.get(method_name)
  }

  /// Merge another tracker into this one
  pub fn merge(&mut self, other: TraitImplTracker) {
    // Merge trait_impls
    for ((trait_name, type_name), methods) in other.trait_impls {
      self.trait_impls.entry((trait_name, type_name)).or_default().extend(methods);
    }

    // Merge method_to_traits
    for (method_name, traits) in other.method_to_traits {
      self.method_to_traits.entry(method_name).or_default().extend(traits);
    }

    // Merge extension_traits
    self.extension_traits.extend(other.extension_traits);
  }
}

struct TraitImplVisitor<'a> {
  tracker: &'a mut TraitImplTracker,
}

impl<'ast> Visit<'ast> for TraitImplVisitor<'_> {
  fn visit_item_impl(&mut self, impl_item: &'ast ItemImpl) {
    // Check if this is a trait implementation
    if let Some((_, trait_path, _)) = &impl_item.trait_ {
      let trait_name = path_to_string(trait_path);

      // Get the type being implemented for
      let type_name = match &*impl_item.self_ty {
        Type::Path(TypePath { path, .. }) => path_to_string(path),
        _ => return, // Skip complex types for now
      };

      // Check if it's an extension trait (implementing for a type from another crate)
      // Simple heuristic: if the trait name ends with "Ext" or contains "Extension"
      if trait_name.ends_with("Ext") || trait_name.contains("Extension") {
        self.tracker.extension_traits.insert(trait_name.clone());
      }

      // Collect all methods in this impl
      let mut methods = FxHashSet::default();
      for item in &impl_item.items {
        if let ImplItem::Fn(method) = item {
          let method_name = method.sig.ident.to_string();
          methods.insert(method_name.clone());

          // Add to method_to_traits mapping
          self
            .tracker
            .method_to_traits
            .entry(method_name.clone())
            .or_default()
            .insert(trait_name.clone());
        }
      }

      // Store the impl information
      self.tracker.trait_impls.insert((trait_name, type_name), methods);
    }

    // Continue visiting nested items
    syn::visit::visit_item_impl(self, impl_item);
  }
}

fn path_to_string(path: &Path) -> String {
  path.segments.iter().map(|seg| seg.ident.to_string()).collect::<Vec<_>>().join("::")
}
