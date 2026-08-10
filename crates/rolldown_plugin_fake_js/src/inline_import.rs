use oxc::allocator::Allocator;
use oxc::ast::ast::{
  TSImportType, TSImportTypeQualifiedName, TSImportTypeQualifier, TSTypeQuery, TSTypeQueryExprName,
};
use oxc::ast_visit::Visit;
use oxc::ast_visit::walk::walk_ts_type_query;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType, Span};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone)]
pub struct InlineImportInfo {
  pub span: Span,
  pub source: String,
  pub qualifier: Option<String>,
  pub is_typeof: bool,
  pub is_local: bool,
}

pub struct InlineImportCollector {
  pub imports: Vec<InlineImportInfo>,
}

impl InlineImportCollector {
  pub fn new() -> Self {
    Self { imports: Vec::new() }
  }

  pub fn collect(source: &str) -> Vec<InlineImportInfo> {
    let allocator = Allocator::default();
    let source_type = SourceType::d_ts();
    let parser = Parser::new(&allocator, source, source_type);
    let parse_result = parser.parse();

    let mut collector = Self::new();
    collector.visit_program(&parse_result.program);
    collector.imports
  }

  pub fn rewrite_source(source: &str) -> (String, Vec<String>) {
    let imports = Self::collect(source);
    if imports.is_empty() {
      return (source.to_string(), Vec::new());
    }

    let mut generated_imports: Vec<String> = Vec::new();
    let mut ns_counter: usize = 0;
    let mut ns_map: FxHashMap<String, String> = FxHashMap::default();

    let mut sorted_imports = imports;
    sorted_imports.sort_by_key(|info| std::cmp::Reverse(info.span.start));

    let mut result = source.to_string();

    for info in &sorted_imports {
      let start = info.span.start as usize;
      let end = info.span.end as usize;

      if start >= result.len() || end > result.len() {
        continue;
      }

      if !info.is_local {
        continue;
      }

      if let Some(qualifier) = &info.qualifier {
        let replacement =
          if info.is_typeof { format!("typeof {qualifier}") } else { qualifier.clone() };
        result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
        generated_imports.push(format!("import {{ {} }} from \"{}\"", qualifier, info.source));
      } else {
        let ns_name = ns_map.entry(info.source.clone()).or_insert_with(|| {
          let name = format!("{}_{}", sanitize_module_name(&info.source), ns_counter);
          ns_counter += 1;
          name
        });
        let replacement =
          if info.is_typeof { format!("typeof {ns_name}") } else { ns_name.clone() };
        result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
        generated_imports.push(format!("import * as {} from \"{}\"", ns_name, info.source));
      }
    }

    generated_imports.sort();
    generated_imports.dedup();

    (result, generated_imports)
  }

  pub fn rewrite_external_imports(
    source: &str,
    allowed_sources: &FxHashSet<String>,
  ) -> (String, Vec<String>) {
    if allowed_sources.is_empty() {
      return (source.to_string(), Vec::new());
    }

    let imports = Self::collect(source);
    if imports.is_empty() {
      return (source.to_string(), Vec::new());
    }

    let mut generated_imports: Vec<String> = Vec::new();
    let mut ns_counters: FxHashMap<String, usize> = FxHashMap::default();
    let mut ns_map: FxHashMap<String, String> = FxHashMap::default();

    let mut sorted_imports = imports;
    sorted_imports.sort_by_key(|info| std::cmp::Reverse(info.span.start));

    let mut result = source.to_string();
    let mut source_order: Vec<String> = Vec::new();

    for info in &sorted_imports {
      let start = info.span.start as usize;
      let end = info.span.end as usize;

      if start >= result.len() || end > result.len() {
        continue;
      }

      if info.is_local {
        continue;
      }

      if !allowed_sources.contains(&info.source) {
        continue;
      }

      let ns_name = ns_map.entry(info.source.clone()).or_insert_with(|| {
        let prefix = sanitize_module_name(&info.source);
        let idx = ns_counters.entry(prefix.clone()).or_insert(0);
        let name = format!("{}{}", prefix, *idx);
        *idx += 1;
        source_order.push(info.source.clone());
        name
      });

      if let Some(qualifier) = &info.qualifier {
        let replacement = if info.is_typeof {
          format!("typeof {ns_name}.{qualifier}")
        } else {
          format!("{ns_name}.{qualifier}")
        };
        result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
      } else {
        let replacement =
          if info.is_typeof { format!("typeof {ns_name}") } else { ns_name.clone() };
        result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
      }
    }

    source_order.reverse();
    for source_path in &source_order {
      if let Some(ns_name) = ns_map.get(source_path) {
        generated_imports.push(format!("import * as {ns_name} from \"{source_path}\";"));
      }
    }

    (result, generated_imports)
  }
}

impl<'a> Visit<'a> for InlineImportCollector {
  fn visit_ts_type_query(&mut self, node: &TSTypeQuery<'a>) {
    if let TSTypeQueryExprName::TSImportType(import_type) = &node.expr_name {
      // Handle `typeof import()` here and return early to avoid double-collect via visit_ts_import_type.
      let source_value = import_type.source.value.to_string();
      let is_local = source_value.starts_with("./") || source_value.starts_with("../");
      let qualifier = extract_qualifier(import_type.qualifier.as_ref());

      self.imports.push(InlineImportInfo {
        span: node.span(),
        source: source_value,
        qualifier,
        is_typeof: true,
        is_local,
      });

      if let Some(type_args) = &import_type.type_arguments {
        for param in &type_args.params {
          self.visit_ts_type(param);
        }
      }
      return;
    }

    walk_ts_type_query(self, node);
  }

  fn visit_ts_import_type(&mut self, node: &TSImportType<'a>) {
    let source_value = node.source.value.to_string();
    let is_local = source_value.starts_with("./") || source_value.starts_with("../");
    let qualifier = extract_qualifier(node.qualifier.as_ref());

    let span = if let (Some(_), Some(type_args)) = (&qualifier, node.type_arguments.as_ref()) {
      // End before type args so `import("./m").Bar<number>` → `Bar` keeps `<number>`.
      Span::new(node.span().start, type_args.span.start)
    } else {
      node.span()
    };

    self.imports.push(InlineImportInfo {
      span,
      source: source_value,
      qualifier,
      is_typeof: false,
      is_local,
    });

    if let Some(type_args) = &node.type_arguments {
      for param in &type_args.params {
        self.visit_ts_type(param);
      }
    }
  }
}

fn extract_qualifier(qualifier: Option<&TSImportTypeQualifier>) -> Option<String> {
  qualifier.and_then(|q| match q {
    TSImportTypeQualifier::Identifier(id) => Some(id.name.to_string()),
    TSImportTypeQualifier::QualifiedName(qn) => {
      fn get_leftmost(q: &TSImportTypeQualifiedName) -> Option<String> {
        match &q.left {
          TSImportTypeQualifier::Identifier(id) => Some(id.name.to_string()),
          TSImportTypeQualifier::QualifiedName(inner) => get_leftmost(inner),
        }
      }
      get_leftmost(qn)
    }
  })
}

fn sanitize_module_name(source: &str) -> String {
  let name = source.trim_start_matches("./").trim_start_matches("../");
  let sanitized: String =
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
  sanitized
}
