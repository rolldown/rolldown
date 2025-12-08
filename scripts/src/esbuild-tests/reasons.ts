/**
 * When a test case is listed here, it will be marked as "failed" in the test summary.
 */
export const failedReasons: Record<string, string> = {
  'dce/dce_of_expr_after_keep_names_issue3195':
    "rolldown don't support keep name, it is part of minifier",
  'dce/dce_of_using_declarations':
    'output should be same except the comment, acorn can not recognize using stmt',
  'dce/disable_tree_shaking':
    'rollup `treeshake.annotations` only affect annotations, https://rollupjs.org/repl/?version=4.27.3&shareable=JTdCJTIyZXhhbXBsZSUyMiUzQW51bGwlMkMlMjJtb2R1bGVzJTIyJTNBJTVCJTdCJTIyY29kZSUyMiUzQSUyMmltcG9ydCUyMCU1QyUyMi4lMkZxdXguanMlNUMlMjIlNUNuJTVDbmZ1bmN0aW9uJTIwdGVzdCgpJTIwJTdCJTVDbmNvbnNvbGUubG9nKCd0ZXN0JyklNUNuJTdEJTVDbiU1Q24lMkYqJTIzX19QVVJFX18qJTJGdGVzdCgpJTNCJTIyJTJDJTIyaXNFbnRyeSUyMiUzQXRydWUlMkMlMjJuYW1lJTIyJTNBJTIybWFpbi5qcyUyMiU3RCUyQyU3QiUyMmNvZGUlMjIlM0ElMjJjb25zb2xlLmxvZygndGVzdCcpJTIyJTJDJTIyaXNFbnRyeSUyMiUzQWZhbHNlJTJDJTIybmFtZSUyMiUzQSUyMnF1eC5qcyUyMiU3RCUyQyU3QiUyMmNvZGUlMjIlM0ElMjIlN0IlNUNuJTIwJTIwJTVDJTIyc2lkZUVmZmVjdHMlNUMlMjIlM0ElMjBmYWxzZSU1Q24lN0QlMjIlMkMlMjJpc0VudHJ5JTIyJTNBZmFsc2UlMkMlMjJuYW1lJTIyJTNBJTIycGFja2FnZS5qc29uJTIyJTdEJTVEJTJDJTIyb3B0aW9ucyUyMiUzQSU3QiUyMm91dHB1dCUyMiUzQSU3QiUyMmZvcm1hdCUyMiUzQSUyMmVzJTIyJTdEJTJDJTIydHJlZXNoYWtlJTIyJTNBJTdCJTIyYW5ub3RhdGlvbnMlMjIlM0F0cnVlJTdEJTdEJTdE',
  'dce/tree_shaking_js_with_associated_css':
    'esbuild generate debug id for each css file and sub optimal output',
  'dce/tree_shaking_js_with_associated_css_unused_nested_import_side_effects_false':
    'Since the `sideEffects: false`, and the `ImportDeclaration` is just plain, the whole sub tree (including css file) should be eliminated',
  'dce/tree_shaking_js_with_associated_css_unused_nested_import_side_effects_false_only_js':
    'Since the `sideEffects: false`, and the `ImportDeclaration` is just plain, the whole sub tree (including css file) should be eliminated',
  'default/comment_preservation':
    'comments codegen related to `oxc` and the original test case is `ModePassThrough`',
  'default/comment_preservation_transform_jsx':
    'transpiled jsx should have leading `@__PURE__`, already tracked https://github.com/oxc-project/oxc/issues/6072',
  'default/define_import_meta_es5':
    "don't see necessarity to auto polyfill `import.meta` since we already support `define`",
  'default/direct_eval_tainting_no_bundle':
    "sub optimal: eval in `test4` param position don't need to be renamed",
  'default/duplicate_entry_point':
    'rolldown try to extract common module when duplicate entry point',
  'default/entry_names_chunk_names_ext_placeholder':
    'css comments and different chunk file naming style',
  'default/inject_jsx':
    'due to multi pass transformer arch, this test could not be supported for now(we should `Define` first and then `Transform`).',
  'default/mangle_props_import_export':
    "for `__require` diff, we don't have ModePassThrough",
  'default/no_warn_common_js_exports_in_esm_pass_through':
    "We don't have pass through mode, we just have same output as esbuild if",
  'importstar/import_star_common_js_capture':
    'sub optimal: esbuild try to reuse `ns` variable, we always create a new one',
  'importstar/import_star_common_js_no_capture':
    'sub optimal: esbuild will reuse `ns` variable',
  'importstar/import_star_common_js_unused':
    'sub optimal: esbuild will reuse `ns` variable',
  'importstar/namespace_import_missing_common_js':
    'sub optimal: esbuild will reuse `ns` variable',
  'importstar/namespace_import_unused_missing_common_js':
    'sub optimal: esbuild will reuse `ns` variable',
  'importstar_ts/ts_import_star_common_js_capture':
    'sub optimal: could reuse `ns` binding',
  'importstar_ts/ts_import_star_common_js_no_capture':
    'sub optimal: could reuse `ns` binding',
  'loader/loader_data_url_text_css':
    'esbuild generate debug id for each css file and sub optimal',
  'loader/loader_json_common_js_and_es6':
    'esbuild will inline declaration and sub optimal',
  'loader/loader_json_no_bundle_es6':
    'sub optimal and should inline literal in json',
  'loader/loader_json_prototype':
    'esbuild will inline named export if only default export is used. could be done in minifier',
  'packagejson/package_json_browser_map_avoid_missing': 'sub optimal',
  'ts/ts_namespace_keep_names':
    "we don't have plan to support keep_names in rolldown",
  'dce/tree_shaking_lowered_class_static_field': 'lowering class',
  'dce/tree_shaking_lowered_class_static_field_minified': 'lowering class',
  'default/argument_default_value_scope_no_bundle': 'lowering class',
  'ts/ts_computed_class_field_use_define_false': 'lowering class',
  'ts/ts_computed_class_field_use_define_true': 'lowering class',
  'ts/ts_computed_class_field_use_define_true_lower': 'lowering class',
  'ts/ts_declare_class_fields': 'lowering class',
  'ts/ts_minify_derived_class': 'lowering class',
  'ts/ts_experimental_decorators_keep_names':
    'lowering ts experimental decorator',
  'ts/ts_experimental_decorators_mangle_props_assign_semantics':
    'lowering ts experimental decorator',
  'ts/ts_experimental_decorators_mangle_props_define_semantics':
    'lowering ts experimental decorator',
  'ts/ts_experimental_decorators_mangle_props_methods':
    'lowering ts experimental decorator',
  'ts/ts_experimental_decorators_mangle_props_static_assign_semantics':
    'lowering ts experimental decorator',
  'ts/ts_experimental_decorators_mangle_props_static_define_semantics':
    'lowering ts experimental decorator',
  'ts/ts_experimental_decorators_mangle_props_static_methods':
    'lowering ts experimental decorator',
  'importstar/import_namespace_undefined_property_empty_file': 'Wrong output',
  'importstar/import_namespace_undefined_property_side_effect_free_file':
    'Wrong output',
  'loader/loader_json_no_bundle_common_js': 'Wrong output',
  'loader/loader_json_no_bundle_iife': 'Wrong output',
  'dce/package_json_side_effects_array_keep_main_implicit_main':
    'double module initialization',
  'dce/package_json_side_effects_array_keep_module_implicit_main':
    'double module initialization',
  'importstar/re_export_star_as_external_iife': 'different iife impl',
  'importstar/re_export_star_as_iife_no_bundle': 'different iife impl',
  'importstar/re_export_star_external_iife': 'Wrong impl',
  'importstar/re_export_star_iife_no_bundle': 'Wrong impl',
  'ts/this_inside_function_ts': 'static class field lowering',
  'ts/this_inside_function_ts_no_bundle': 'static class field lowering',
  'ts/ts_common_js_variable_in_esm_type_module': 'sub optimal',
  'ts/ts_import_in_node_modules_name_collision_with_css': 'sub optimal',
  'ts/ts_experimental_decorator_scope_issue2147': 'lowering decorator',
  'ts/ts_experimental_decorators': 'lowering decorator',
  'dce/dce_of_decorators': 'dce decorator',
  'dce/dce_of_experimental_decorators': 'lower decorator',
  'dce/dce_of_iife': "don't support dce iife",
  'dce/no_side_effects_comment': 'annotation codegen',
  'dce/no_side_effects_comment_type_script_declare':
    'rolldown should not shake the namespace iife',
  'dce/no_side_effects_comment_unused_calls': 'no sideEffect comment detect',
  'dce/package_json_side_effects_array_keep_module_use_main':
    'side effects detect not align',
  'dce/package_json_side_effects_false_all_fork': 'dynamic module not align',
  'dce/package_json_side_effects_false_one_fork': 'different async module impl',
  'dce/remove_unused_no_side_effects_tagged_templates':
    'side effects detector not align',
  'dce/tree_shaking_lowered_class_static_field_assignment':
    'seems esbuild mark static field as side effects whatever, should investigate',
  'dce/tree_shaking_react_elements': "jsx element don't have pure annotation",
  'dce/tree_shaking_unary_operators': 'unary operator side effects',
  'default/conditional_import':
    'esbuild will wrap `Promise.resolve().then() for original specifier`',
  'default/define_import_meta': 'oxc define',
  'default/export_forms_common_js': 'redundant `__toCommonJS`',
  'default/export_forms_with_minify_identifiers_and_no_bundle':
    'should not generate duplicate export binding',
  'default/external_es6_converted_to_common_js':
    'redundant `import` statements',
  'default/false_require':
    'should rename `require` when it is appear in param position',
  'default/import_abs_path_with_query_parameter':
    'query and hashban in specifier',
  'default/import_meta_common_js':
    'rolldown keep unsupported `import.meta` as it is in cjs format.',
  'default/import_missing_neither_es6_nor_common_js':
    'rolldown extract common module',
  'default/import_namespace_this_value': 'rolldown split chunks',
  'default/indirect_require_message': 'not align',
  'default/inject_import_meta': 'different inject implementation',
  'default/inject_no_bundle':
    'generate wrong syntax when Exported is `StringLiteral`, and rest part of esbuild gen is weird since there is no need to rename',
  'default/non_determinism_issue2537': 'should read `tsconfig.json`',
  'default/package_alias': 'resolve alias',
  'default/package_alias_match_longest': 'alias not align',
  'default/rename_private_identifiers_no_bundle': 'rename private identifier',
  'default/string_export_names_common_js':
    "should not reuse `__toESM(require('./foo'))`",
  'default/string_export_names_iife': 'string export name not correct',
  'default/this_inside_function': 'lowering not align',
  'default/top_level_await_allowed_import_with_splitting':
    'there should not exist empty chunk',
  'default/top_level_await_allowed_import_without_splitting':
    "Can't disable bundle splitting",
  'default/use_strict_directive_bundle_issue1837': 'inject path',
  'default/warnings_inside_node_modules': 'alias',
  'loader/jsx_automatic_no_name_collision': 'esbuild did not needs `__toESM`',
  'loader/jsx_preserve_capital_letter':
    "rolldown don't have `jsx.Preserve` and `jsx.Parse` option",
  'loader/loader_data_url_base64_invalid_utf8':
    'mime type should be `data:text/plain`',
  'loader/loader_file_multiple_no_collision': 'Different hash asset name',
  'loader/loader_file_one_source_two_different_output_paths_css':
    'generate wrong output when css as entry and has shared css',
  'loader/loader_file_one_source_two_different_output_paths_js':
    'immediate js file reference `.png` file',
  'loader/loader_file_relative_path_asset_names_css': 'css reference .png',
  'loader/loader_file_relative_path_js': 'abs output base',
  'loader/loader_json_no_bundle': 'should treated it as cjs module',
  'lower/lower_export_star_as_name_collision':
    "should not transform `export * as ns from 'mod'` above es2019",
  'lower/static_class_block_es_next':
    'pure transformation is handled by `oxc-transform`',
  'packagejson/common_js_variable_in_esm_type_module':
    'redundant `__commonJS` wrapper',
  'packagejson/package_json_browser_issue2002_b': '`sub` is not resolved',
  'packagejson/package_json_disabled_type_module_issue3367':
    'ignored module debug name seems not correct',
  'splitting/edge_case_issue2793_without_splitting':
    'dynamic import with cycle reference',
  'splitting/splitting_missing_lazy_export':
    'should convert missing property to `void 0`',
  'ts/export_type_issue379':
    "rolldown is not ts aware after ts transformation, We can't aware that `Test` is just a type",
  'ts/this_inside_function_ts_no_bundle_use_define_for_class_fields':
    'should not convert `ClassDeclaration` to `ClassExpr`',
  'ts/this_inside_function_ts_use_define_for_class_fields':
    'should convert `FunctionDeclaration` to `FunctionExpr`',
  'ts/ts_enum_cross_module_tree_shaking': 'enum side effects',
  'ts/ts_experimental_decorators_no_config': 'ts experimental decorator',
  'ts/ts_export_default_type_issue316':
    'oxc transform strip type decl but did not remove related `ExportDecl`, this woulcause rolldown assume it export a global variable, which has side effects.',
  'ts/ts_export_equals': 'require `oxc-transformer` support `module type`',
  'ts/ts_export_missing_es6': 'export missing es6',
  'ts/ts_namespace_keep_names_target_es2015': 'needs support target',
  'ts/ts_prefer_js_over_ts_inside_node_modules': 'controversial',
  'ts/ts_sibling_enum': 'enum inline',
  'ts/ts_this_is_undefined_warning': 'rewrite this when it is undefined',
};

export const notSupportedReasons: Record<string, string> = {
  'default/legal_comments_avoid_slash_tag_end_of_file':
    'not support legal comments',
  'default/legal_comments_avoid_slash_tag_external':
    'not support legal comments',
  'default/legal_comments_avoid_slash_tag_inline': 'not support legal comments',
  'default/legal_comments_end_of_file': 'not support legal comments',
  'default/legal_comments_escape_slash_script_and_style_end_of_file':
    'not support legal comments',
  'default/legal_comments_escape_slash_script_and_style_external':
    'not support legal comments',
  'default/legal_comments_external': 'not support legal comments',
  'default/legal_comments_inline': 'not support legal comments',
  'default/legal_comments_linked': 'not support legal comments',
  'default/legal_comments_many_end_of_file': 'not support legal comments',
  'default/legal_comments_many_linked': 'not support legal comments',
  'default/legal_comments_modify_indent': 'not support legal comments',
  'default/legal_comments_no_escape_slash_script_end_of_file':
    'not support legal comments',
  'default/legal_comments_no_escape_slash_style_end_of_file':
    'not support legal comments',
  'default/legal_comments_none': 'not support legal comments',

  'ts/enum_rules_from_type_script_5_0': 'not support const enum inline',
  'ts/ts_const_enum_comments': 'not support const enum inline',
  'ts/ts_enum_cross_module_inlining_access': 'not support const enum inline',
  'ts/ts_enum_cross_module_inlining_definitions':
    'not support const enum inline',
  'ts/ts_enum_cross_module_inlining_minify_index_into_dot':
    'not support const enum inline',
  'ts/ts_enum_cross_module_inlining_re_export': 'not support const enum inline',
  'ts/ts_enum_export_clause': 'not support const enum inline',
  'ts/ts_enum_same_module_inlining_access': 'not support const enum inline',
  'ts/ts_enum_tree_shaking': 'not support const enum inline',
  'ts/ts_enum_use_before_declare': 'not support const enum inline',
  'ts/ts_minify_enum_cross_file_inline_strings_into_templates':
    'not support const enum inline',
  'ts/ts_minify_enum_property_names': 'not support const enum inline',
  'ts/ts_print_non_finite_number_inside_with': 'not support const enum inline',

  'default/metafile_various_cases': 'not support copy loader',
  'default/metafile_very_long_external_paths': 'not support copy loader',
  'loader/loader_copy_with_bundle_entry_point': 'not support copy loader',
  'loader/loader_copy_with_bundle_from_css': 'not support copy loader',
  'loader/loader_copy_with_bundle_from_js': 'not support copy loader',
  'loader/loader_copy_with_format': 'not support copy loader',
  'loader/loader_copy_with_injected_file_bundle': 'not support copy loader',
  'loader/loader_copy_with_transform': 'not support copy loader',

  'glob/glob_basic_no_splitting': 'not support glob',
  'glob/glob_basic_splitting': 'not support glob',
  'glob/glob_no_matches': 'not support glob',
  'glob/glob_wildcard_no_slash': 'not support glob',
  'glob/glob_wildcard_slash': 'not support glob',
  'glob/ts_glob_basic_no_splitting': 'not support glob',
  'glob/ts_glob_basic_splitting': 'not support glob',

  'loader/loader_file_ext_path_asset_names_js':
    'not support asset path template',
  'loader/loader_file_public_path_asset_names_css':
    'not support asset path template',
  'loader/loader_file_public_path_asset_names_js':
    'not support asset path template',
  'loader/loader_file_relative_path_asset_names_css':
    'not support asset path template',
  'loader/loader_file_relative_path_asset_names_js':
    'not support asset path template',
  'loader/loader_file_relative_path_css': 'not support asset path template',

  'default/comment_preservation_import_assertions':
    'not support import attributes',
  'default/metafile_import_with_type_json': 'not support import attributes',
  'default/output_for_assert_type_json': 'not support import attributes',
  'loader/loader_bundle_with_import_attributes':
    'not support import attributes',
  'loader/with_type_json_override_loader': 'not support import attributes',

  'loader/loader_file_public_path_css': 'not support public path',
  'loader/loader_file_public_path_js': 'not support public path',

  'default/comment_preservation_preserve_jsx': 'not support `jsx.preserve`',

  'default/conditional_require_resolve':
    'not support conditional `require.resolve`',

  'default/minified_jsx_preserve_with_object_spread':
    'not support preserve `jsx`',

  'default/require_and_dynamic_import_invalid_template':
    'not support invalid template',

  'default/require_shim_substitution': 'not support require second argument',

  'ts/ts_enum_jsx': 'not support enum inline',

  'ts/ts_import_equals_elimination_test':
    "rolldown is not ts aware, it's not possibly support for now and sub optimal",
  'ts/ts_import_equals_tree_shaking_false':
    "rolldown is not ts aware, it's not possible to support for now and sub optimal",
  'ts/ts_import_equals_tree_shaking_true':
    "rolldown is not ts aware, it's not possible to support for now and sub optimal",
  'ts/ts_import_equals_undefined_import':
    "rolldown is not ts aware, it's not possible to support for now and sub optimal",
};

export const ignoreReasons: Record<string, string> = {
  'default/import_abs_path_as_dir':
    'limitation of test infra, the test may hard to pass in CI',
  'default/import_abs_path_as_file':
    'limitation of test infra, the test may hard to pass in CI',
  'default/inject_duplicate':
    "`oxc` inject align with `@rollup/plugin-inject` don't support inject source file directly",
  'default/inject_import_order':
    "`oxc` inject align with `@rollup/plugin-inject` don't support inject files directly",
  'default/inject_import_ts':
    "`oxc` inject align with `@rollup/plugin-inject` don't support inject files directly",
  'default/inject_with_string_export_name_bundle':
    'replace the function it self in `inject files`, this align with `@rollup/plugin-inject`',
  'default/inject_with_string_export_name_no_bundle':
    'replace the function it self in `inject files`, this align with `@rollup/plugin-inject`',
  'default/jsx_import_meta_property':
    "don't support `unsupportedFeature` https://github.com/evanw/esbuild/commit/71a2f8de5ad4e1882f35c449efa25761aa1241b5#diff-e20508c4ae566a2d8a60274ff05e408d81c9758a27d84318feecdfbf9e24af5eR11297-R11308",
  'default/jsx_import_meta_value':
    "don't support `unsupportedFeature` https://github.com/evanw/esbuild/commit/71a2f8de5ad4e1882f35c449efa25761aa1241b5#diff-e20508c4ae566a2d8a60274ff05e408d81c9758a27d84318feecdfbf9e24af5eR11297-R11308",
  'default/quoted_property_mangle':
    'Currently there is no way to control *quoted* behavior, since we use `oxc` to convert ast to string and we just generate same output as esbuild if disable `MinifySyntax`',

  // TODO: Add proper reasons for these skipped tests
  'dce/dce_of_symbol_ctor_call': 'TODO',
  'dce/dead_code_inside_unused_cases': 'TODO',
  'default/assign_to_import_no_bundle': 'TODO',
  'default/call_import_namespace_warning': 'TODO',
  'default/decorator_printing_cjs': 'TODO',
  'default/decorator_printing_esm': 'TODO',
  'default/entry_names_no_slash_after_dir': 'TODO',
  'default/entry_names_non_portable_character': 'TODO',
  'default/error_message_crash_stdin_issue2913': 'TODO',
  'default/export_special_name': 'TODO',
  'default/export_special_name_bundle': 'TODO',
  'default/import_with_hash_parameter': 'TODO',
  'default/import_with_query_parameter': 'TODO',
  'default/jsx_constant_fragments': 'TODO',
  'default/jsx_dev_self_edge_cases': 'TODO',
  'default/line_limit_minified': 'TODO',
  'default/line_limit_not_minified': 'TODO',
  'default/mangle_props_jsx_transform_namespace': 'TODO',
  'default/mangle_props_type_script_features': 'TODO',
  'default/minify_nested_labels_no_bundle': 'TODO',
  'default/this_with_es6_syntax': 'TODO',
  'glob/glob_entry_point_abs_path': 'TODO',
  'loader/empty_loader_css': 'TODO',
  'loader/extensionless_loader_css': 'TODO',
  'loader/loader_bundle_with_unknown_import_attributes_and_copy_loader': 'TODO',
  'loader/loader_bundle_with_unknown_import_attributes_and_js_loader': 'TODO',
  'loader/loader_copy_entry_point_advanced': 'TODO',
  'loader/loader_copy_explicit_output_file': 'TODO',
  'loader/loader_copy_starts_with_dot_abs_path': 'TODO',
  'loader/loader_copy_starts_with_dot_rel_path': 'TODO',
  'loader/loader_copy_use_index': 'TODO',
  'loader/loader_file': 'TODO',
  'loader/loader_file_with_query_parameter': 'TODO',
  'loader/loader_from_extension_with_query_parameter': 'TODO',
  'loader/loader_inline_source_map_absolute_path_issue4075_unix': 'TODO',
  'loader/loader_inline_source_map_absolute_path_issue4075_windows': 'TODO',
  'loader/loader_json_with_big_int': 'TODO',
  'loader/loader_text_utf8_bom': 'TODO',
  'loader/with_bad_attribute': 'TODO',
  'loader/with_bad_type': 'TODO',
  'loader/with_type_bytes_override_loader': 'TODO',
  'loader/with_type_bytes_override_loader_glob': 'TODO',
};
