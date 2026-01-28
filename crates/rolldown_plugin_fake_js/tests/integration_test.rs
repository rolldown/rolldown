use rolldown_plugin_fake_js::{ChunkInfo, FakeJsOptions, FakeJsPlugin};

#[test]
fn test_simple_interface() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export interface Foo { bar: string; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Foo"));
  assert!(result.code.contains("export"));
}

#[test]
fn test_interface_with_extends() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export interface Bar extends Foo { baz: number; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Bar"));
  assert!(result.code.contains("Foo"));
}

#[test]
fn test_type_alias() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export type MyType = string | number;";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyType"));
}

#[test]
fn test_function_declaration() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export declare function foo(x: number): string;";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var foo"));
}

#[test]
fn test_class_declaration() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export declare class MyClass { prop: string; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyClass"));
}

#[test]
fn test_enum_declaration() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export enum Color { Red, Green, Blue }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Color"));
}

#[test]
fn test_default_export() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export default class Foo { bar: string; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("as default"));
}

#[test]
fn test_generic_type() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "export interface Box<T> { value: T; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Box"));
  assert!(result.code.contains('('));
}

#[test]
fn test_multiple_declarations() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export interface Foo { a: string; }
export interface Bar { b: number; }
export type Baz = Foo | Bar;
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Foo"));
  assert!(result.code.contains("var Bar"));
  assert!(result.code.contains("var Baz"));
}

#[test]
fn test_non_dts_file() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = "const x = 1;";
  let result = plugin.transform(code, "test.ts").unwrap();

  assert_eq!(result.code, code);
}

#[test]
fn test_sourcemap_option() {
  let options = FakeJsOptions { sourcemap: true, ..Default::default() };
  let plugin = FakeJsPlugin::new(options);
  let code = "export interface Foo { bar: string; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.map.is_some());
}

#[test]
fn test_side_effects_option() {
  let options = FakeJsOptions { side_effects: true, ..Default::default() };
  let plugin = FakeJsPlugin::new(options);
  let code = "export interface Foo { bar: string; }";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("sideEffect"));
}

#[test]
fn test_render_chunk() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());

  let code = "export interface Foo { bar: string; }";
  let _ = plugin.transform(code, "test.d.ts").unwrap();

  let fake_js = "export var Foo = [0, () => [], []]";
  let chunk =
    ChunkInfo { filename: "bundle.d.ts".to_string(), module_ids: vec!["test.d.ts".to_string()] };

  let result = plugin.render_chunk(fake_js, &chunk).unwrap();
  assert!(result.contains("declare"));
}

#[test]
fn test_empty_chunk() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let chunk = ChunkInfo { filename: "empty.d.ts".to_string(), module_ids: vec![] };

  let result = plugin.render_chunk("", &chunk).unwrap();
  assert_eq!(result, "export { };");
}

#[test]
fn test_reference_directive() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r#"/// <reference types="node" />
/// <reference path="./types.d.ts" />
export interface Foo { bar: string; }
"#;
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Foo"));

  let fake_js = "export var Foo = [0, () => [], []]";
  let chunk =
    ChunkInfo { filename: "bundle.d.ts".to_string(), module_ids: vec!["test.d.ts".to_string()] };

  let rendered = plugin.render_chunk(fake_js, &chunk).unwrap();

  assert!(
    rendered.contains("/// <reference types=\"node\" />")
      || rendered.contains("/// <reference path=")
  );
}

#[test]
fn test_reference_directive_multiple_files() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());

  let code1 = r#"/// <reference types="node" />
export interface Foo { bar: string; }
"#;
  let _ = plugin.transform(code1, "file1.d.ts").unwrap();

  let code2 = r#"/// <reference types="jest" />
export interface Bar { baz: number; }
"#;
  let _ = plugin.transform(code2, "file2.d.ts").unwrap();

  let fake_js = r"export var Foo = [0, () => [], []]
export var Bar = [1, () => [], []]";
  let chunk = ChunkInfo {
    filename: "bundle.d.ts".to_string(),
    module_ids: vec!["file1.d.ts".to_string(), "file2.d.ts".to_string()],
  };

  let rendered = plugin.render_chunk(fake_js, &chunk).unwrap();

  assert!(rendered.contains("/// <reference"));
}

#[test]
fn test_namespace_declaration() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export namespace MyNamespace {
    export interface Foo { bar: string; }
}
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyNamespace"));
}

#[test]
fn test_conditional_type() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export type IsString<T> = T extends string ? true : false;
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var IsString"));
}

#[test]
fn test_mapped_type() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export type Readonly<T> = {
    readonly [P in keyof T]: T[P];
};
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var Readonly"));
}

#[test]
fn test_import_type() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r#"
export type MyType = import("./other").OtherType;
"#;
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyType"));
}

#[test]
fn test_class_with_extends_and_implements() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export declare class MyClass extends BaseClass implements IFoo, IBar {
    prop: string;
}
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyClass"));
  assert!(result.code.contains("BaseClass") || result.code.contains("IFoo"));
}

#[test]
fn test_infer_type_in_conditional() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export type UnwrapPromise<T> = T extends Promise<infer R> ? R : T;
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var UnwrapPromise"));
}

#[test]
fn test_qualified_type_name() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export type MyType = Namespace.SubType;
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyType"));
  assert!(result.code.contains("Namespace"));
}

#[test]
fn test_typeof_query() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export type MyType = typeof someValue;
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var MyType"));
  assert!(result.code.contains("someValue"));
}

#[test]
fn test_module_side_effect() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());

  let code = r#"
declare module "foo" {
    export interface Bar {}
}
"#;
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("sideEffect"));
}

#[test]
fn test_variable_declaration_with_destructuring() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());
  let code = r"
export declare const { a, b }: { a: string; b: number };
";
  let result = plugin.transform(code, "test.d.ts").unwrap();

  assert!(result.code.contains("var a") || result.code.contains("var b"));
}

#[test]
fn test_dts_mts_cts_extensions() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());

  let code = "export interface Foo {}";
  let result = plugin.transform(code, "test.d.mts").unwrap();
  assert!(result.code.contains("var Foo"));

  let result = plugin.transform(code, "test.d.cts").unwrap();
  assert!(result.code.contains("var Foo"));
}

#[test]
fn test_import_source_patching() {
  let plugin = FakeJsPlugin::new(FakeJsOptions::default());

  let _ = plugin.transform("export interface Foo {}", "test.d.ts").unwrap();

  let chunk_code = r#"import { Bar } from "./other.d.ts";"#;
  let chunk =
    ChunkInfo { filename: "bundle.d.ts".to_string(), module_ids: vec!["test.d.ts".to_string()] };

  let result = plugin.render_chunk(chunk_code, &chunk).unwrap();

  assert!(result.contains("./other.js"));
}
