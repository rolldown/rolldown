# Aggregate Reason
## lowering class
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field_minified
- crates/rolldown/tests/esbuild/default/argument_default_value_scope_no_bundle
- crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_false
- crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_true
- crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_true_lower
- crates/rolldown/tests/esbuild/ts/ts_declare_class_fields
- crates/rolldown/tests/esbuild/ts/ts_minify_derived_class
## lowering ts experimental decorator
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_keep_names
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_assign_semantics
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_define_semantics
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_methods
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_static_assign_semantics
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_static_define_semantics
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_static_methods
## Wrong output
- crates/rolldown/tests/esbuild/importstar/import_namespace_undefined_property_empty_file
- crates/rolldown/tests/esbuild/importstar/import_namespace_undefined_property_side_effect_free_file
- crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_common_js
- crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_iife
## double module initialization
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_main_implicit_main
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_module_implicit_main
## different iife impl
- crates/rolldown/tests/esbuild/importstar/re_export_star_as_external_iife
- crates/rolldown/tests/esbuild/importstar/re_export_star_as_iife_no_bundle
## Wrong impl
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_iife
- crates/rolldown/tests/esbuild/importstar/re_export_star_iife_no_bundle
## static class field lowering
- crates/rolldown/tests/esbuild/ts/this_inside_function_ts
- crates/rolldown/tests/esbuild/ts/this_inside_function_ts_no_bundle
## sub optimal
- crates/rolldown/tests/esbuild/ts/ts_common_js_variable_in_esm_type_module
- crates/rolldown/tests/esbuild/ts/ts_import_in_node_modules_name_collision_with_css
## lowering decorator
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorator_scope_issue2147
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators
## dce decorator
- crates/rolldown/tests/esbuild/dce/dce_of_decorators
## lower decorator
- crates/rolldown/tests/esbuild/dce/dce_of_experimental_decorators
## don't support dce iife
- crates/rolldown/tests/esbuild/dce/dce_of_iife
## annotation codegen
- crates/rolldown/tests/esbuild/dce/no_side_effects_comment
## rolldown should not shake the namespace iife
- crates/rolldown/tests/esbuild/dce/no_side_effects_comment_type_script_declare
## no sideEffect comment detect
- crates/rolldown/tests/esbuild/dce/no_side_effects_comment_unused_calls
## side effects detect not align
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_module_use_main
## dynamic module not align
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_all_fork
## different async module impl
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_one_fork
## side effects detector not align
- crates/rolldown/tests/esbuild/dce/remove_unused_no_side_effects_tagged_templates
## seems esbuild mark static field as side effects whatever, should investigate
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field_assignment
## jsx element don't have pure annotation
- crates/rolldown/tests/esbuild/dce/tree_shaking_react_elements
## unary operator side effects
- crates/rolldown/tests/esbuild/dce/tree_shaking_unary_operators
## esbuild will wrap `Promise.resolve().then() for original specifier`
- crates/rolldown/tests/esbuild/default/conditional_import
## oxc define
- crates/rolldown/tests/esbuild/default/define_import_meta
## redundant `__toCommonJS`
- crates/rolldown/tests/esbuild/default/export_forms_common_js
## should not generate duplicate export binding
- crates/rolldown/tests/esbuild/default/export_forms_with_minify_identifiers_and_no_bundle
## redundant `import` statements
- crates/rolldown/tests/esbuild/default/external_es6_converted_to_common_js
## should not generate `__toCommonJS`
- crates/rolldown/tests/esbuild/default/external_es6_converted_to_common_js
## should rename `require` when it is appear in param position
- crates/rolldown/tests/esbuild/default/false_require
## query and hashban in specifier
- crates/rolldown/tests/esbuild/default/import_abs_path_with_query_parameter
## rolldown keep unsupported `import.meta` as it is in cjs format.
- crates/rolldown/tests/esbuild/default/import_meta_common_js
## rolldown polyfill `import.meta.url` with `require("url").pathToFileURL(__filename).href` in cjs format and node platform.
- crates/rolldown/tests/esbuild/default/import_meta_common_js
## rolldown extract common module
- crates/rolldown/tests/esbuild/default/import_missing_neither_es6_nor_common_js
## rolldown split chunks
- crates/rolldown/tests/esbuild/default/import_namespace_this_value
## not align
- crates/rolldown/tests/esbuild/default/indirect_require_message
## different inject implementation
- crates/rolldown/tests/esbuild/default/inject_import_meta
## generate wrong syntax when Exported is `StringLiteral`, and rest part of esbuild gen is weird since there is no need to rename
- crates/rolldown/tests/esbuild/default/inject_no_bundle
## should read `tsconfig.json`
- crates/rolldown/tests/esbuild/default/non_determinism_issue2537
## resolve alias
- crates/rolldown/tests/esbuild/default/package_alias
## alias not align
- crates/rolldown/tests/esbuild/default/package_alias_match_longest
## rename private identifier
- crates/rolldown/tests/esbuild/default/rename_private_identifiers_no_bundle
## wrong `export default require_entry()`;
- crates/rolldown/tests/esbuild/default/require_shim_substitution
## should not reuse `__toESM(require('./foo'))`
- crates/rolldown/tests/esbuild/default/string_export_names_common_js
## string export name not correct
- crates/rolldown/tests/esbuild/default/string_export_names_iife
## lowering not align
- crates/rolldown/tests/esbuild/default/this_inside_function
## there should not exist empty chunk
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_with_splitting
## import('./entry.js') should be rewrite to `require_entry`
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_with_splitting
## Can't disable bundle splitting
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_without_splitting
## inject path
- crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_issue1837
## alias
- crates/rolldown/tests/esbuild/default/warnings_inside_node_modules
## esbuild did not needs `__toESM`
- crates/rolldown/tests/esbuild/loader/jsx_automatic_no_name_collision
## rolldown don't have `jsx.Preserve` and `jsx.Parse` option
- crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter
## mime type should be `data:text/plain`
- crates/rolldown/tests/esbuild/loader/loader_data_url_base64_invalid_utf8
## Different hash asset name
- crates/rolldown/tests/esbuild/loader/loader_file_multiple_no_collision
## Same content has different name
- crates/rolldown/tests/esbuild/loader/loader_file_multiple_no_collision
## generate wrong output when css as entry and has shared css
- crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_css
## immediate js file reference `.png` file
- crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_js
## css reference .png
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_css
## abs output base
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_js
## should treated it as cjs module
- crates/rolldown/tests/esbuild/loader/loader_json_no_bundle
## should not transform `export * as ns from 'mod'` above es2019
- crates/rolldown/tests/esbuild/lower/lower_export_star_as_name_collision
## pure transformation is handled by `oxc-transform`
- crates/rolldown/tests/esbuild/lower/static_class_block_es_next
## redundant `__commonJS` wrapper
- crates/rolldown/tests/esbuild/packagejson/common_js_variable_in_esm_type_module
## `sub` is not resolved
- crates/rolldown/tests/esbuild/packagejson/package_json_browser_issue2002_b
## ignored module debug name seems not correct
- crates/rolldown/tests/esbuild/packagejson/package_json_disabled_type_module_issue3367
## dynamic import with cycle reference
- crates/rolldown/tests/esbuild/splitting/edge_case_issue2793_without_splitting
## should convert missing property to `void 0`
- crates/rolldown/tests/esbuild/splitting/splitting_missing_lazy_export
## rolldown is not ts aware after ts transformation, We can't aware that `Test` is just a type
- crates/rolldown/tests/esbuild/ts/export_type_issue379
## transform `FunctionDeclaration` to `FunctionExpr`
- crates/rolldown/tests/esbuild/ts/this_inside_function_ts_no_bundle
## should not convert `ClassDeclaration` to `ClassExpr`
- crates/rolldown/tests/esbuild/ts/this_inside_function_ts_no_bundle_use_define_for_class_fields
## should convert `FunctionDeclaration` to `FunctionExpr`
- crates/rolldown/tests/esbuild/ts/this_inside_function_ts_use_define_for_class_fields
## redundant wrap function
- crates/rolldown/tests/esbuild/ts/ts_common_js_variable_in_esm_type_module
## enum side effects
- crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_tree_shaking
## pure annotation for enum
- crates/rolldown/tests/esbuild/ts/ts_enum_same_module_inlining_access
## enum tree shaking
- crates/rolldown/tests/esbuild/ts/ts_enum_tree_shaking
## ts experimental decorator
- crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_no_config
## oxc transform strip type decl but did not remove related `ExportDecl`, this woulcause rolldown assume it export a global variable, which has side effects.
- crates/rolldown/tests/esbuild/ts/ts_export_default_type_issue316
## require `oxc-transformer` support `module type`
- crates/rolldown/tests/esbuild/ts/ts_export_equals
## export missing es6
- crates/rolldown/tests/esbuild/ts/ts_export_missing_es6
## rolldown don't insert debug comments in css
- crates/rolldown/tests/esbuild/ts/ts_import_in_node_modules_name_collision_with_css
## needs support target
- crates/rolldown/tests/esbuild/ts/ts_namespace_keep_names_target_es2015
## controversial
- crates/rolldown/tests/esbuild/ts/ts_prefer_js_over_ts_inside_node_modules
## we have similar output as webpack but different with esbuild, because of https://github.com/evanw/esbuild/commit/54ae9962ba18eafc0fc3f1c8c76641def9b08aa0
- crates/rolldown/tests/esbuild/ts/ts_prefer_js_over_ts_inside_node_modules
## enum inline
- crates/rolldown/tests/esbuild/ts/ts_sibling_enum
## rewrite this when it is undefined
- crates/rolldown/tests/esbuild/ts/ts_this_is_undefined_warning
