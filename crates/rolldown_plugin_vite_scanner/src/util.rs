use std::{borrow::Cow, path::Path};

use once_cell::sync::Lazy;
use regex::{NoExpand, Regex};
use rustc_hash::FxHashMap;

static COMMENT_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"<!--.*?-->"#).expect("Init COMMENT_REGEX failed"));
static SCRIPT_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"(<script(?:\s+[a-z_:][-\w:]*(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^"'<>=\s]+))?)*\s*>)(.*?)<\/script>"#).expect("Init SCRIPT_REGEX failed")
});
static SRC_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s'">]+))"#).expect("Init SRC_REGEX failed")
});
static TYPE_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"\btype\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s'">]+))"#).expect("Init TYPE_REGEX failed")
});
static LANG_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"\blang\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s'">]+))"#).expect("Init LANG_REGEX failed")
});
static CONTEXT_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"\bcontext\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s'">]+))"#)
    .expect("Init CONTEXT_REGEX failed")
});
static MULTILINE_COMMENT_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"\/\*[^*]*\*+(?:[^/*][^*]*\*+)*\/").expect("Init MULTILINE_COMMENT_REGEX failed")
});
static SINGLE_COMMENT_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"\/\/.*").expect("Init MULTILINE_COMMENT_REGEX failed"));
// A simple regex to detect import sources. This is only used on
// <script lang="ts"> blocks in vue (setup only) or svelte files, since
// seemingly unused imports are dropped by bundler when transpiling TS which
// prevents it from crawling further.
static IMPORTS_FROM_BLOCK_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"import([\w*{}\n\r\t, ]+from)?\s*([\w\d"'\.\/]*)"#)
    .expect("Init IMPORTS_FROM_BLOCK_REGEX failed")
});
static VIRTUAL_MODULE_PREFIX: &str = "virtual-module:";

pub fn extract_html_module_scripts(
  content: &str,
  path: &Path,
) -> (String, FxHashMap<String, String>) {
  let mut scripts = FxHashMap::default();
  let mut result = String::new();
  let extension = path.extension().unwrap_or_default();
  let is_html = extension == "html";
  let is_astro = extension == "astro";
  let is_svelte = extension == "svelte";
  let raw = COMMENT_REGEX.replace(content, NoExpand("<!---->"));

  for (index, c) in SCRIPT_REGEX.captures_iter(&raw).enumerate() {
    let (_, [open_tag, content]) = c.extract();

    let script_type = match_open_tag_attr(open_tag, &TYPE_REGEX);

    // skip non type module script
    if is_html && !matches!(script_type, Some(v) if v == "module") {
      continue;
    }

    // skip type="application/ld+json" and other non-JS types
    if matches!(script_type, Some(v) if !(v.contains("javascript") || v.contains("ecmascript") || v == "module"))
    {
      continue;
    }

    let script_src = match_open_tag_attr(open_tag, &SRC_REGEX);

    if let Some(script_src) = script_src {
      result.push_str(&format!("import '{script_src}';\n"));
    }
    // The reason why virtual modules are needed:
    // 1. There can be module scripts (`<script context="module">` in Svelte and `<script>` in Vue)
    // or local scripts (`<script>` in Svelte and `<script setup>` in Vue)
    // 2. There can be multiple module scripts in html
    // We need to handle these separately in case variable names are reused between them

    // append imports in TS to prevent bundler from removing them
    // since they may be used in the template
    let mut contents = content.trim().to_string();
    if !contents.is_empty() {
      let script_lang = match_open_tag_attr(open_tag, &LANG_REGEX);

      if matches!(script_lang, Some( v) if v == "ts" || v == "tsx") || is_astro {
        contents.push_str(&extract_import_paths(content));
      }

      let loader: Cow<'_, str> =
        script_lang.map_or_else(|| if is_astro { "ts".into() } else { "js".into() }, Into::into);
      // Here append loader to query, it can be used to transform the script content at vite.
      let key = format!("{}?id={index}&loader={loader}", path.to_string_lossy());
      // Glob Import need transform, so legacy the logic to vite.
      scripts.insert(key.clone(), contents);

      let virtual_module_path = format!("'{VIRTUAL_MODULE_PREFIX}{key}'");
      let context = match_open_tag_attr(open_tag, &CONTEXT_REGEX);

      // Especially for Svelte files, exports in <script context="module"> means module exports,
      // exports in <script> means component props. To avoid having two same export name from the
      // star exports, we need to ignore exports in <script>
      if is_svelte && matches!(context, Some(v) if v != "module") {
        result.push_str(&format!("import {virtual_module_path}\n"));
      } else {
        result.push_str(&format!("export * from {virtual_module_path}\n"));
      }
    }
  }

  // This will trigger incorrectly if `export default` is contained
  // anywhere in a string. Svelte and Astro files can't have
  // `export default` as code so we know if it's encountered it's a
  // false positive (e.g. contained in a string)
  if extension != "vue" || !result.contains("export default") {
    result.push_str("\nexport default {}");
  }

  (result, scripts)
}

fn match_open_tag_attr<'a>(open_tag: &'a str, regex: &Lazy<Regex>) -> Option<&'a str> {
  regex.captures(open_tag).map(|caps| {
    caps.get(1).map_or_else(
      || {
        caps
          .get(2)
          .map_or_else(|| caps.get(3).map(|m| m.as_str()).unwrap_or_default(), |m| m.as_str())
      },
      |m| m.as_str(),
    )
  })
}

/**
 * when using TS + (Vue + `<script setup>`) or Svelte, imports may seem
 * unused to bundler and dropped in the build output, which prevents
 * bundler from crawling further.
 * the solution is to add `import 'x'` for every source to force
 * bundler to keep crawling due to potential side effects.
 */
fn extract_import_paths(code: &str) -> String {
  let mut result = String::new();

  let value = MULTILINE_COMMENT_REGEX.replace_all(code, NoExpand("/* */"));
  let raw = SINGLE_COMMENT_REGEX.replace_all(&value, NoExpand(""));

  for c in IMPORTS_FROM_BLOCK_REGEX.captures_iter(&raw) {
    if let Some(src) = c.get(2) {
      result.push_str(&format!("\nimport {};", src.as_str()));
    }
  }

  result
}

#[test]
fn test_extract_import_paths() {
  assert_eq!(
    extract_import_paths("import 'a';\n // import 'b';\nimport {c} from './c1';\nconsole.log(1);"),
    "\nimport 'a';\nimport './c1';".to_string()
  );
}

#[test]
fn test_extract_html_module_scripts() {
  // skip non type module script
  assert_eq!(
    extract_html_module_scripts("<script></script>", &Path::new("a.html")),
    ("\nexport default {}".to_string(), FxHashMap::default())
  );
  // skip type="application/ld+json" and other non-JS types
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="application/ld+json"></script>"#,
      &Path::new("a.vue")
    ),
    ("\nexport default {}".to_string(), FxHashMap::default())
  );
  // src script
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module" src="a.js"></script>"#,
      &Path::new("a.html")
    ),
    ("import 'a.js';\n\nexport default {}".to_string(), FxHashMap::default())
  );
  // multiply script
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module" src="a.js"></script><script type="module" src="b.js"></script>"#,
      &Path::new("a.html")
    ),
    ("import 'a.js';\nimport 'b.js';\n\nexport default {}".to_string(), FxHashMap::default())
  );
  // content script
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module">console.log(1)</script>"#,
      &Path::new("a.html")
    ),
    (
      "export * from 'virtual-module:a.html?id=0&loader=js'\n\nexport default {}".to_string(),
      FxHashMap::from_iter(vec![(
        "a.html?id=0&loader=js".to_string(),
        "console.log(1)".to_string()
      )])
    )
  );
  // ts content script
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module" lang="ts">import "./a";\nconsole.log(1)</script>"#,
      &Path::new("a.html")
    ),
    (
      "export * from 'virtual-module:a.html?id=0&loader=ts'\n\nexport default {}".to_string(),
      FxHashMap::from_iter(vec![(
        "a.html?id=0&loader=ts".to_string(),
        "import \"./a\";\\nconsole.log(1)\nimport \"./a\";".to_string()
      )])
    )
  );
  // svelte <script context="module">
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module" context="module">console.log(1)</script>"#,
      &Path::new("a.svelte")
    ),
    (
      "export * from 'virtual-module:a.svelte?id=0&loader=js'\n\nexport default {}".to_string(),
      FxHashMap::from_iter(vec![(
        "a.svelte?id=0&loader=js".to_string(),
        "console.log(1)".to_string()
      )])
    )
  );
  // svelte <script context="non-module">
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module" context="non-module">console.log(1)</script>"#,
      &Path::new("a.svelte")
    ),
    (
      "import 'virtual-module:a.svelte?id=0&loader=js'\n\nexport default {}".to_string(),
      FxHashMap::from_iter(vec![(
        "a.svelte?id=0&loader=js".to_string(),
        "console.log(1)".to_string()
      )])
    )
  );
  // astro
  assert_eq!(
    extract_html_module_scripts(
      r#"<script type="module">import "./a";\nconsole.log(1)</script>"#,
      &Path::new("a.astro")
    ),
    (
      "export * from 'virtual-module:a.astro?id=0&loader=ts'\n\nexport default {}".to_string(),
      FxHashMap::from_iter(vec![(
        "a.astro?id=0&loader=ts".to_string(),
        "import \"./a\";\\nconsole.log(1)\nimport \"./a\";".to_string()
      )])
    )
  );
}
