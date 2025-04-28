use std::path::Path;

use rolldown::ModuleType;
use rolldown_utils::pattern_filter;

use super::types::binding_hook_filter::{BindingRenderChunkHookFilter, BindingTransformHookFilter};

/// If the transform hook is filtered out and need to be skipped.
/// return `false` means it should be skipped.
/// return `true` means it should not be skipped.
/// Since transform has three different filter, so we need to check all of them.
pub fn filter_transform(
  transform_filter: Option<&BindingTransformHookFilter>,
  id: &str,
  cwd: &Path,
  module_type: &ModuleType,
  code: &str,
) -> bool {
  let Some(transform_filter) = transform_filter else {
    return true;
  };

  if let Some(ref module_type_filter) = transform_filter.module_type {
    if !module_type_filter.iter().any(|ty| ty.as_ref() == module_type) {
      return false;
    }
  }

  let mut filter_ret = true;

  if let Some(ref id_filter) = transform_filter.id {
    let id_res = pattern_filter::filter(
      id_filter.exclude.as_deref(),
      id_filter.include.as_deref(),
      id,
      cwd.to_string_lossy().as_ref(),
    );

    filter_ret = filter_ret && id_res.inner();
  }

  if !filter_ret {
    return false;
  }

  if let Some(ref code_filter) = transform_filter.code {
    let code_res = pattern_filter::filter_code(
      code_filter.exclude.as_deref(),
      code_filter.include.as_deref(),
      code,
    );

    filter_ret = filter_ret && code_res.inner();
  }
  filter_ret
}

pub fn filter_render_chunk(
  code: &str,
  render_chunk_filter: Option<&BindingRenderChunkHookFilter>,
) -> bool {
  if let Some(render_chunk_filter) = render_chunk_filter {
    if let Some(ref code_filter) = render_chunk_filter.code {
      let result = pattern_filter::filter_code(
        code_filter.exclude.as_deref(),
        code_filter.include.as_deref(),
        code,
      );

      return result.inner();
    }
  }
  true
}

#[cfg(test)]
mod tests {
  use rolldown_utils::pattern_filter::StringOrRegex;

  use crate::options::plugin::types::{
    binding_hook_filter::BindingGeneralHookFilter, binding_js_or_regex::BindingStringOrRegex,
  };

  use super::*;

  #[test]
  #[allow(clippy::too_many_lines)]
  fn test_filter() {
    #[derive(Debug)]
    struct InputFilter {
      exclude: Option<Vec<StringOrRegex>>,
      include: Option<Vec<StringOrRegex>>,
    }
    /// id, code, expected
    type TestCase<'a> = (&'a str, &'a str, bool);
    struct TestCases<'a> {
      input_id_filter: Option<InputFilter>,
      input_code_filter: Option<InputFilter>,
      cases: Vec<TestCase<'a>>,
    }

    #[expect(clippy::unnecessary_wraps)]
    fn string_filter(value: &str) -> Option<Vec<StringOrRegex>> {
      Some(vec![StringOrRegex::new(value.to_string(), &None).unwrap()])
    }

    let cases = [
      TestCases {
        input_id_filter: Some(InputFilter { exclude: None, include: string_filter("*.js") }),
        input_code_filter: None,
        cases: vec![("foo.js", "foo", true), ("foo.ts", "foo", false)],
      },
      TestCases {
        input_id_filter: None,
        input_code_filter: Some(InputFilter {
          exclude: None,
          include: string_filter("import.meta"),
        }),
        cases: vec![("foo.js", "import.meta", true), ("foo.js", "import_meta", false)],
      },
      TestCases {
        input_id_filter: Some(InputFilter { exclude: string_filter("*.js"), include: None }),
        input_code_filter: Some(InputFilter {
          exclude: None,
          include: string_filter("import.meta"),
        }),
        cases: vec![
          ("foo.js", "import.meta", false),
          ("foo.js", "import_meta", false),
          ("foo.ts", "import.meta", true),
          ("foo.ts", "import_meta", false),
        ],
      },
      TestCases {
        input_id_filter: Some(InputFilter {
          exclude: string_filter("*.js"),
          include: string_filter("foo.ts"),
        }),
        input_code_filter: Some(InputFilter {
          exclude: None,
          include: string_filter("import.meta"),
        }),
        cases: vec![
          ("foo.js", "import.meta", false),
          ("foo.js", "import_meta", false),
          ("foo.ts", "import.meta", true),
          ("foo.ts", "import_meta", false),
        ],
      },
      TestCases {
        input_id_filter: Some(InputFilter {
          exclude: string_filter("*b"),
          include: string_filter("a*"),
        }),
        input_code_filter: Some(InputFilter {
          exclude: string_filter("b"),
          include: string_filter("a"),
        }),
        cases: vec![
          ("ab", "", false),
          ("a", "b", false),
          ("a", "", false),
          ("c", "a", false),
          ("a", "a", true),
        ],
      },
      TestCases {
        input_id_filter: Some(InputFilter {
          exclude: string_filter("*b"),
          include: string_filter("a*"),
        }),
        input_code_filter: Some(InputFilter { exclude: string_filter("b"), include: None }),
        cases: vec![
          ("ab", "", false),
          ("a", "b", false),
          ("a", "", true),
          ("c", "a", false),
          ("a", "a", true),
        ],
      },
    ];

    for (i, test_case) in cases.into_iter().enumerate() {
      let filter = BindingTransformHookFilter {
        id: test_case.input_id_filter.map(|f| BindingGeneralHookFilter {
          include: f.include.map(|f| f.into_iter().map(BindingStringOrRegex::new).collect()),
          exclude: f.exclude.map(|f| f.into_iter().map(BindingStringOrRegex::new).collect()),
          custom: None,
        }),
        code: test_case.input_code_filter.map(|f| BindingGeneralHookFilter {
          include: f.include.map(|f| f.into_iter().map(BindingStringOrRegex::new).collect()),
          exclude: f.exclude.map(|f| f.into_iter().map(BindingStringOrRegex::new).collect()),
          custom: None,
        }),
        module_type: None,
        custom: None,
      };

      for (si, (id, code, expected)) in test_case.cases.into_iter().enumerate() {
        let result = filter_transform(Some(&filter), id, Path::new(""), &ModuleType::Js, code);
        assert_eq!(
          result, expected,
          r"Failed at Case {i}  sub-case {si}.\n filter: {filter:?}, id: {id}, code: {code}",
        );
      }
    }
  }
}
