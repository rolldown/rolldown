use std::collections::HashMap;

pub type Result<T> = anyhow::Result<T>;

#[derive(Debug, Clone, Default)]
pub struct FakeJsOptions {
  pub sourcemap: bool,
  pub cjs_default: bool,
  pub side_effects: bool,
}

#[derive(Debug, Clone)]
pub struct DeclarationInfo {
  pub id: usize,
  #[expect(dead_code)]
  pub bindings: Vec<String>,
  pub type_params: Vec<TypeParam>,
  #[expect(dead_code)]
  pub deps: Vec<String>,
  #[expect(dead_code)]
  pub children: Vec<(u32, u32)>,
  pub source: String,
  #[expect(dead_code)]
  pub is_side_effect: bool,
}

#[derive(Debug, Clone)]
pub struct TypeParam {
  pub name: String,
  #[expect(dead_code)]
  pub occurrences: usize,
}

#[derive(Debug)]
pub struct PluginState {
  pub declaration_idx: usize,
  pub identifier_map: HashMap<String, usize>,
  pub declaration_map: HashMap<usize, DeclarationInfo>,
  pub comments_map: HashMap<String, Vec<String>>,
  pub type_only_map: HashMap<String, Vec<String>>,
}

impl Default for PluginState {
  fn default() -> Self {
    Self::new()
  }
}

impl PluginState {
  pub fn new() -> Self {
    Self {
      declaration_idx: 0,
      identifier_map: HashMap::new(),
      declaration_map: HashMap::new(),
      comments_map: HashMap::new(),
      type_only_map: HashMap::new(),
    }
  }

  #[expect(dead_code)]
  pub fn get_identifier_index(&mut self, name: &str) -> usize {
    let entry = self.identifier_map.entry(name.to_string()).or_insert(0);
    let idx = *entry;
    *entry += 1;
    idx
  }

  pub fn register_declaration(&mut self, mut info: DeclarationInfo) -> usize {
    let id = self.declaration_idx;
    self.declaration_idx += 1;
    info.id = id;
    self.declaration_map.insert(id, info);
    id
  }

  pub fn get_declaration(&self, id: usize) -> Option<&DeclarationInfo> {
    self.declaration_map.get(&id)
  }

  #[expect(dead_code)]
  pub fn get_declaration_mut(&mut self, id: usize) -> Option<&mut DeclarationInfo> {
    self.declaration_map.get_mut(&id)
  }
}

#[derive(Debug)]
pub struct TransformResult {
  pub code: String,
  pub map: Option<String>,
}

#[derive(Debug)]
pub struct ChunkInfo {
  pub filename: String,
  pub module_ids: Vec<String>,
}
