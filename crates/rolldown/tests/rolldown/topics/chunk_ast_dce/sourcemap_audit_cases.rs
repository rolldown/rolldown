use std::path::Path;

use oxc_sourcemap::SourceMap;

use super::map_assertions::{assert_mapping, assert_mapping_at, assert_unmapped, position};

fn source(fixture: &str, filename: &str) -> String {
  std::fs::read_to_string(
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown")).join(fixture).join(filename),
  )
  .unwrap()
}

fn at(
  code: &str,
  map: &SourceMap<'_>,
  generated: (&str, usize),
  filename: &str,
  content: &str,
  original: (&str, usize),
) {
  assert_mapping_at(
    code,
    map,
    code.find(generated.0).unwrap() + generated.1,
    filename,
    content,
    content.find(original.0).unwrap() + original.1,
  );
}

// Inlining deliberately replaces the outer node's span with the reference span.
// Direct printing uses its endpoints; composition can resolve to its start.
fn reference_endpoint(
  code: &str,
  map: &SourceMap<'_>,
  generated: &str,
  filename: &str,
  content: &str,
  reference: &str,
  width: usize,
) {
  let byte = code.find(generated).unwrap();
  let (line, column) = position(code, byte);
  let lookup = map.generate_lookup_table();
  let token = map.lookup_source_view_token(&lookup, line, column).unwrap();
  let start = content.find(reference).unwrap();
  let original = [start, start + width - 1, start + width]
    .into_iter()
    .find(|&candidate| position(content, candidate) == (token.get_src_line(), token.get_src_col()))
    .expect("synthesized punctuation must resolve to its replaced reference's endpoints");
  assert_mapping_at(code, map, byte, filename, content, original);
}

pub(super) fn assert_positions(fixture: &str, variant: &str, code: &str, map: &SourceMap<'_>) {
  let main = source(fixture, "main.js");
  if fixture.starts_with("function/module_types/") {
    assert_module_type_positions(fixture, code, map, &main);
    return;
  }
  match fixture {
    "function/minify/basic" => {
      if std::env::var("ROLLDOWN_CHUNK_AST_DCE").is_ok_and(|mode| mode == "global") {
        let dependency = source(fixture, "shared.js");
        let call = code.find(".equal(").unwrap();
        let generated = code[call..].find("`shared.js`").unwrap() + call;
        assert_mapping_at(
          code,
          map,
          generated,
          "shared.js",
          &dependency,
          dependency.find("'shared.js'").unwrap(),
        );
      }
    }
    "sourcemap/issue_6099" => {
      let dependency = source(fixture, "cjs.js");
      for anchor in ["exports.", "defineProperty(", "get()", "throw ", "new ", "Error("] {
        assert_mapping(code, map, anchor, "cjs.js", &dependency, anchor);
      }
      assert_mapping(code, map, "\"see stacktrace\"", "cjs.js", &dependency, "'see stacktrace'");
      for anchor in ["version.startsWith(", "startsWith(", "console.", "log(foo"] {
        assert_mapping(code, map, anchor, "main.js", &main, anchor);
      }
      assert_mapping(code, map, "\"1\"", "main.js", &main, "'1'");
    }
    "misc/object_shorthand_property" => {
      assert_mapping(code, map, "strictEqual(", "main.js", &main, "strictEqual(");
      assert_mapping(code, map, "{ foo:", "main.js", &main, "value.foo");
      at(code, map, ("foo: \"foo\"", 0), "main.js", &main, ("{ foo }", 2));
      at(code, map, ("foo: \"foo\"", 5), "main.js", &main, ("{ foo }", 2));
      reference_endpoint(code, map, "}.foo", "main.js", &main, "value.foo", 5);
      at(code, map, ("}.foo", 2), "main.js", &main, ("value.foo", 6));
      assert_mapping(code, map, "\"foo\");", "main.js", &main, "'foo');");
    }
    "misc/wrapped_esm" => {
      let dependency = source(fixture, "foo.js");
      assert_mapping(code, map, "foo$1()", "foo.js", &dependency, "foo()");
      for (generated, original) in [
        ("e} =", "e } ="),
        ("f: g", "f: g"),
        ("g = 1", "g = 1"),
        ("h: [x", "h: [x"),
        ("x, {y", "x, { y"),
        ("y}],", "y }],"),
        ("z},", "z },"),
        ("k} =", "k } ="),
        ("null }}", "null },"),
        ("destructuring} =", "destructuring } ="),
      ] {
        assert_mapping(code, map, generated, "foo.js", &dependency, original);
      }
      assert_mapping(code, map, "globalThis.globalVar", "main.js", &main, "globalThis.globalVar");
      assert_mapping(code, map, "strictEqual(foo.a1", "main.js", &main, "strictEqual(foo.a1");
      assert_mapping(code, map, "foo.destructuring,", "main.js", &main, "foo.destructuring,");
      // The synthesized require sequence has no original expression span. Its
      // inherited declarator anchor is intentionally not certified as precise.
      assert_unmapped(code, map, "__defProp = Object.defineProperty");
    }
    "sourcemap/imported_cjs_callee" => {
      let client = source(fixture, "client.cjs");
      at(code, map, (".createRoot)(", 1), "main.js", &main, ("createRoot(document", 0));
      for anchor in ["document.", "getElementById(", "render();"] {
        assert_mapping(code, map, anchor, "main.js", &main, anchor);
      }
      assert_mapping(code, map, "textContent", "client.cjs", &client, "textContent");
      let root = if variant == "default" { "\"root\"" } else { "`root`" };
      assert_mapping(code, map, root, "main.js", &main, "'root'");
      at(code, map, (root, root.len()), "main.js", &main, ("'root'", 6));
      at(code, map, (root, root.len() + 1), "main.js", &main, ("'root'", 7));
      at(code, map, (".render();", 0), "main.js", &main, (".render();", 0));
      if variant == "default" {
        assert_mapping(
          code,
          map,
          "import_client.createRoot",
          "main.js",
          &main,
          "createRoot(document",
        );
        assert_unmapped(code, map, "__commonJSMin =");
      } else {
        // Wrapper-call punctuation replaces the callee reference, rather than
        // corresponding literally to a source call/dot. Accept start/end anchors.
        reference_endpoint(code, map, ").createRoot", "main.js", &main, "createRoot(document", 10);
        reference_endpoint(code, map, ".createRoot)(", "main.js", &main, "createRoot(document", 10);
        reference_endpoint(code, map, "(document.", "main.js", &main, "createRoot(document", 10);
        assert_unmapped(code, map, "/* @__PURE__ */");
        let boundary = code.find("}})))().createRoot").unwrap() + 1;
        assert_mapping_at(
          code,
          map,
          boundary - 1,
          "client.cjs",
          &client,
          client.rfind('}').unwrap(),
        );
        for offset in 0..5 {
          assert_unmapped(code, map, &code[boundary + offset..]);
        }
      }
    }
    "topics/deconflict/complex_params_patterns" => {
      for (generated, original) in [
        ("b$1: b$1$1", "{ b$1 }"),
        ("c$1: c$1$1", "{ c$1 ="),
        ("d$1: d$1$1", "{ d$1 }"),
        ("e$1: e$1$1", "{ e$1 ="),
      ] {
        at(code, map, (generated, 0), "main.js", &main, (original, 2));
        at(code, map, (generated, 5), "main.js", &main, (original, 2));
      }
      let generated = code.find("void 0, void 0").unwrap();
      let original = main.find("undefined, undefined").unwrap();
      for (generated_offset, original_offset) in [(0, 0), (5, 0), (8, 11), (13, 11)] {
        assert_mapping_at(
          code,
          map,
          generated + generated_offset,
          "main.js",
          &main,
          original + original_offset,
        );
      }
      assert_mapping(code, map, "foo(\"a$1\"", "main.js", &main, "foo('a$1'");
      assert_mapping(code, map, "ret.params,", "main.js", &main, "ret.params,");
      let names = source(fixture, "names.js");
      at(code, map, ("[a$1, b$1", 1), "names.js", &names, ("[a, b", 1));
    }
    "topics/deconflict/decl_complex_patterns" => {
      let names = source(fixture, "names.js");
      for (generated, original) in [("b: b$1", "{ b }"), ("d: d$1", "{ d =")] {
        at(code, map, (generated, 0), "names.js", &names, (original, 2));
        at(code, map, (generated, 3), "names.js", &names, (original, 2));
      }
      at(code, map, ("const { b }", 8), "main.js", &main, ("const { b }", 8));
      at(code, map, ("equal(b$1,", 6), "main.js", &main, ("equal(b2,", 6));
    }
    "topics/deconflict/decl_nested_assign_pattern" => {
      let names = source(fixture, "names.js");
      for offset in [0, 5] {
        at(code, map, ("baz: baz$1", offset), "names.js", &names, ("baz =", 0));
      }
      assert_mapping(code, map, "baz = \"\"", "main.js", &main, "baz = ''");
      at(code, map, ("strictEqual(baz$1", 12), "main.js", &main, ("strictEqual(baz2", 12));
    }
    "topics/deconflict/default_function"
    | "topics/deconflict/wrapped_esm_default_function"
    | "topics/deconflict/wrapped_esm_export_named_function" => {
      let bar = source(fixture, "bar.js");
      let dependency = source(fixture, "foo.js");
      for offset in [0, 5] {
        at(code, map, ("foo: foo$1", offset), "bar.js", &bar, ("export default { foo", 17));
      }
      assert_mapping(code, map, "{ foo: foo$1", "main.js", &main, "bar;\n");
      reference_endpoint(code, map, "};\nassert.strictEqual", "main.js", &main, "bar;\n", 3);
      assert_mapping(code, map, "foo$1(a$1)", "foo.js", &dependency, "foo(a$1)");
      assert_mapping(code, map, "strictEqual(typeof", "main.js", &main, "strictEqual(typeof");
      let body = if fixture.ends_with("wrapped_esm_default_function") {
        "assert.equal(a$1"
      } else {
        "console.log(a$1"
      };
      assert_mapping(code, map, body, "foo.js", &dependency, body);
      if fixture != "topics/deconflict/default_function" {
        let generated = code.find("\ninit_foo();").unwrap() + 1;
        assert_mapping_at(code, map, generated + 9, "bar.js", &bar, bar.find(';').unwrap());
        // The call keeps the import span but its callee is synthetic. Global
        // leaves its start unmapped; original composition clamps to import end.
        let (line, column) = position(code, generated);
        let lookup = map.generate_lookup_table();
        if map
          .lookup_source_view_token(&lookup, line, column)
          .is_some_and(|token| token.get_source_id().is_some())
        {
          assert_mapping_at(code, map, generated, "bar.js", &bar, bar.find(';').unwrap());
        } else {
          assert_unmapped(code, map, &code[generated..]);
        }
      }
    }
    "tree_shaking/multi-declarator" => {
      let dependency = source(fixture, "foo.js");
      assert_mapping(code, map, "baz: \"baz\"", "foo.js", &dependency, "baz: 'baz'");
      at(code, map, ("[c, d]", 1), "foo.js", &dependency, ("[c, d]", 1));
      assert_mapping(code, map, "equal(\"foo\"", "main.js", &main, "equal(foo");
      // The first string currently inherits `equal`; do not bless that coarse
      // argument position. Check the real neighboring literal and next call.
      assert_mapping(code, map, "\"foo\");", "main.js", &main, "'foo');");
      at(code, map, ("equal(c, 1)", 6), "main.js", &main, ("equal(c, 1)", 6));
      at(code, map, ("equal(c, 1)", 9), "main.js", &main, ("equal(c, 1)", 9));
    }
    _ => panic!("unreviewed source map fixture {fixture}"),
  }
}

fn assert_module_type_positions(fixture: &str, code: &str, map: &SourceMap<'_>, main: &str) {
  match fixture {
    "function/module_types/copy/basic" => {
      assert_mapping(code, map, "data2 =", "main.js", main, "data2 =");
      assert_mapping_at(
        code,
        map,
        code.rfind("\"./assets/hello-").unwrap(),
        "main.js",
        main,
        main.rfind("'./hello.txt'").unwrap(),
      );
      assert_mapping(code, map, "console.", "main.js", main, "console.");
      assert_mapping(code, map, "data, data2", "main.js", main, "data, data2");
      assert_unmapped(code, map, "if (typeof require");
    }
    "function/module_types/copy/nested_path" => {
      assert_mapping(code, map, "shallowReq =", "main.js", main, "shallowReq =");
      assert_mapping(code, map, "deepReq =", "main.js", main, "deepReq =");
      assert_mapping_at(
        code,
        map,
        code.rfind("\"../../deep/assets/root-").unwrap(),
        "main.js",
        main,
        main.rfind("'./root.txt'").unwrap(),
      );
      assert_mapping_at(
        code,
        map,
        code.rfind("\"../../deep/assets/deep-").unwrap(),
        "main.js",
        main,
        main.rfind("'./a/b/c/deep.txt'").unwrap(),
      );
      for anchor in ["console.", "shallow, deep", "shallowReq, deepReq"] {
        assert_mapping(code, map, anchor, "main.js", main, anchor);
      }
      assert_unmapped(code, map, "if (typeof require");
    }
    "function/module_types/asset" => {
      assert_mapping(code, map, "main_default =", "main.js", main, "export default");
      assert_mapping(code, map, ".default", "main.js", main, "filePath;");
      // Asset loading synthesizes JavaScript; its source content is the loader's
      // module, not the original text asset. Do not certify it as a text-file map.
      let source_id = map
        .get_sources()
        .position(|source| source.ends_with("foo.txt"))
        .map(|index| u32::try_from(index).unwrap())
        .unwrap();
      let content = map.get_source_content(source_id).unwrap();
      assert!(content.starts_with("module.exports = \"__ROLLDOWN_ASSET__"));
      assert_mapping(code, map, "module.exports", "foo.txt", content, "module.exports");
      assert_mapping(code, map, "\"./assets/foo-", "foo.txt", content, "\"__ROLLDOWN_ASSET__");
    }
    _ => panic!("unreviewed module type fixture {fixture}"),
  }
}
