# Aggregate Reason
## not support legal comments
- crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_end_of_file
- crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_external
- crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_inline
- crates/rolldown/tests/esbuild/default/legal_comments_end_of_file
- crates/rolldown/tests/esbuild/default/legal_comments_escape_slash_script_and_style_end_of_file
- crates/rolldown/tests/esbuild/default/legal_comments_escape_slash_script_and_style_external
- crates/rolldown/tests/esbuild/default/legal_comments_external
- crates/rolldown/tests/esbuild/default/legal_comments_inline
- crates/rolldown/tests/esbuild/default/legal_comments_linked
- crates/rolldown/tests/esbuild/default/legal_comments_many_end_of_file
- crates/rolldown/tests/esbuild/default/legal_comments_many_linked
- crates/rolldown/tests/esbuild/default/legal_comments_modify_indent
- crates/rolldown/tests/esbuild/default/legal_comments_no_escape_slash_script_end_of_file
- crates/rolldown/tests/esbuild/default/legal_comments_no_escape_slash_style_end_of_file
- crates/rolldown/tests/esbuild/default/legal_comments_none
## not support const enum inline
- crates/rolldown/tests/esbuild/ts/enum_rules_from_type_script_5_0
- crates/rolldown/tests/esbuild/ts/ts_const_enum_comments
- crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_access
- crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_definitions
- crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_minify_index_into_dot
- crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_re_export
- crates/rolldown/tests/esbuild/ts/ts_enum_export_clause
- crates/rolldown/tests/esbuild/ts/ts_enum_same_module_inlining_access
- crates/rolldown/tests/esbuild/ts/ts_enum_tree_shaking
- crates/rolldown/tests/esbuild/ts/ts_enum_use_before_declare
- crates/rolldown/tests/esbuild/ts/ts_minify_enum_cross_file_inline_strings_into_templates
- crates/rolldown/tests/esbuild/ts/ts_minify_enum_property_names
- crates/rolldown/tests/esbuild/ts/ts_print_non_finite_number_inside_with
## not support copy loader
- crates/rolldown/tests/esbuild/default/metafile_various_cases
- crates/rolldown/tests/esbuild/default/metafile_very_long_external_paths
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_entry_point
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_css
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_js
- crates/rolldown/tests/esbuild/loader/loader_copy_with_format
- crates/rolldown/tests/esbuild/loader/loader_copy_with_injected_file_bundle
- crates/rolldown/tests/esbuild/loader/loader_copy_with_transform
## lowering class
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field_minified
- crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_false
- crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_true
- crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_true_lower
- crates/rolldown/tests/esbuild/ts/ts_declare_class_fields
- crates/rolldown/tests/esbuild/ts/ts_minify_derived_class
## `jsx.factory`
- crates/rolldown/tests/esbuild/default/import_re_export_es6_issue149
- crates/rolldown/tests/esbuild/default/jsx_import_meta_property
- crates/rolldown/tests/esbuild/default/jsx_import_meta_value
- crates/rolldown/tests/esbuild/default/jsx_this_property_common_js
- crates/rolldown/tests/esbuild/default/jsx_this_property_esm
- crates/rolldown/tests/esbuild/default/jsx_this_value_common_js
- crates/rolldown/tests/esbuild/default/jsx_this_value_esm
## not support glob
- crates/rolldown/tests/esbuild/glob/glob_basic_no_splitting
- crates/rolldown/tests/esbuild/glob/glob_basic_splitting
- crates/rolldown/tests/esbuild/glob/glob_no_matches
- crates/rolldown/tests/esbuild/glob/glob_wildcard_no_slash
- crates/rolldown/tests/esbuild/glob/glob_wildcard_slash
- crates/rolldown/tests/esbuild/glob/ts_glob_basic_no_splitting
- crates/rolldown/tests/esbuild/glob/ts_glob_basic_splitting
## not support asset path template
- crates/rolldown/tests/esbuild/loader/loader_file_ext_path_asset_names_js
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_css
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_js
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_js
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_css
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_js
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_css
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
- crates/rolldown/tests/esbuild/loader/loader_json_shared_with_multiple_entries_issue413
## not support import attributes
- crates/rolldown/tests/esbuild/default/comment_preservation_import_assertions
- crates/rolldown/tests/esbuild/default/metafile_import_with_type_json
- crates/rolldown/tests/esbuild/default/output_for_assert_type_json
- crates/rolldown/tests/esbuild/loader/with_type_json_override_loader
## should rewrite `require`
- crates/rolldown/tests/esbuild/default/nested_require_without_call
- crates/rolldown/tests/esbuild/default/require_without_call
- crates/rolldown/tests/esbuild/default/require_without_call_inside_try
## different iife impl
- crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_iife_issue2264
- crates/rolldown/tests/esbuild/importstar/re_export_star_as_external_iife
- crates/rolldown/tests/esbuild/importstar/re_export_star_as_iife_no_bundle
## rolldown has redundant `require('external')`
- crates/rolldown/tests/esbuild/importstar/re_export_star_common_js_no_bundle
- crates/rolldown/tests/esbuild/importstar/re_export_star_entry_point_and_inner_file
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_common_js
## cross module constant folding
- crates/rolldown/tests/esbuild/dce/cross_module_constant_folding_number
- crates/rolldown/tests/esbuild/dce/cross_module_constant_folding_string
## double module initialization
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_main_implicit_main
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_module_implicit_main
## comments codegen
- crates/rolldown/tests/esbuild/default/comment_preservation
- crates/rolldown/tests/esbuild/default/comment_preservation_transform_jsx
## cjs module lexer can't recognize esbuild interop pattern
- crates/rolldown/tests/esbuild/default/export_forms_iife
- crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_common_js
## rolldown split chunks
- crates/rolldown/tests/esbuild/default/import_namespace_this_value
- crates/rolldown/tests/esbuild/default/multiple_entry_points_same_name_collision
## should not replace the function it self in `inject files`
- crates/rolldown/tests/esbuild/default/inject_with_string_export_name_bundle
- crates/rolldown/tests/esbuild/default/inject_with_string_export_name_no_bundle
## rolldown has redundant `import "external"`
- crates/rolldown/tests/esbuild/importstar/re_export_star_es6_no_bundle
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_es6
## Wrong impl
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_iife
- crates/rolldown/tests/esbuild/importstar/re_export_star_iife_no_bundle
## not support public path
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_css
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_js
## should inline variable
- crates/rolldown/tests/esbuild/loader/loader_json_prototype
- crates/rolldown/tests/esbuild/loader/loader_json_prototype_es5
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
## sub optimal for pure call with spread
- crates/rolldown/tests/esbuild/dce/pure_calls_with_spread
## side effects detector not align
- crates/rolldown/tests/esbuild/dce/remove_unused_no_side_effects_tagged_templates
## seems esbuild mark static field as side effects whatever, should investigate
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field_assignment
## jsx element don't have pure annotation
- crates/rolldown/tests/esbuild/dce/tree_shaking_react_elements
## unary operator side effects
- crates/rolldown/tests/esbuild/dce/tree_shaking_unary_operators
## class field lowering
- crates/rolldown/tests/esbuild/default/argument_default_value_scope_no_bundle
## related to minifier
- crates/rolldown/tests/esbuild/default/arguments_special_case_no_bundle
## the deconflict of no top level is sub optimal
- crates/rolldown/tests/esbuild/default/arrow_fn_scope
## for `__require` diff, we don't have `ModePassThrough`
- crates/rolldown/tests/esbuild/default/comment_preservation
## not support `jsx.preserve`
- crates/rolldown/tests/esbuild/default/comment_preservation_preserve_jsx
## esbuild will wrap `Promise.resolve().then() for original specifier`
- crates/rolldown/tests/esbuild/default/conditional_import
## We don't consider `require($expr)` as a import record
- crates/rolldown/tests/esbuild/default/conditional_require
## not support conditional `require.resolve`
- crates/rolldown/tests/esbuild/default/conditional_require_resolve
## oxc define not support computed member expr
- crates/rolldown/tests/esbuild/default/define_assign_warning
## not support member expr with write
- crates/rolldown/tests/esbuild/default/define_assign_warning
## oxc define
- crates/rolldown/tests/esbuild/default/define_import_meta
## should warn when target do not support `imoprt.meta`
- crates/rolldown/tests/esbuild/default/define_import_meta_es5
## oxc define do not support optional chain
- crates/rolldown/tests/esbuild/default/define_optional_chain
## lowering optional chain
- crates/rolldown/tests/esbuild/default/define_optional_chain_lowered
## oxc define do not support  optional chain
- crates/rolldown/tests/esbuild/default/define_optional_chain_lowered
## define expr with optional chain
- crates/rolldown/tests/esbuild/default/define_optional_chain_panic_issue3551
## oxc define dont support this expr
- crates/rolldown/tests/esbuild/default/define_this
## redundant `__toCommonJS`
- crates/rolldown/tests/esbuild/default/export_forms_common_js
## Not sure if we needs to use `Object.define` pattern in iife
- crates/rolldown/tests/esbuild/default/export_forms_iife
## should not generate duplicate export binding
- crates/rolldown/tests/esbuild/default/export_forms_with_minify_identifiers_and_no_bundle
## should not generate two redundant `require`
- crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_common_js
## two `import` statement are redundant
- crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_es6
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
## not align
- crates/rolldown/tests/esbuild/default/indirect_require_message
## generate wrong syntax when Exported is `StringLiteral`
- crates/rolldown/tests/esbuild/default/inject
## different inject implementation
- crates/rolldown/tests/esbuild/default/inject_import_meta
## generate wrong syntax when Exported is `StringLiteral`, and rest part of esbuild gen is weird since there is no need to rename
- crates/rolldown/tests/esbuild/default/inject_no_bundle
## different naming style
- crates/rolldown/tests/esbuild/default/jsx_automatic_imports_common_js
## wrong tree shaking result
- crates/rolldown/tests/esbuild/default/mangle_no_quoted_props
## not support preserve `jsx`
- crates/rolldown/tests/esbuild/default/minified_jsx_preserve_with_object_spread
## should read `tsconfig.json`
- crates/rolldown/tests/esbuild/default/non_determinism_issue2537
## resolve alias
- crates/rolldown/tests/esbuild/default/package_alias
## alias not align
- crates/rolldown/tests/esbuild/default/package_alias_match_longest
## rename private identifier
- crates/rolldown/tests/esbuild/default/rename_private_identifiers_no_bundle
## not support invalid template
- crates/rolldown/tests/esbuild/default/require_and_dynamic_import_invalid_template
## `__require` rewrite
- crates/rolldown/tests/esbuild/default/require_bad_argument_count
## require json should not wrapped in `__esm`
- crates/rolldown/tests/esbuild/default/require_json
## require `.json`, the json file should not wrapped in `__esm`
- crates/rolldown/tests/esbuild/default/require_shim_substitution
## obviously, the output is incorrect
- crates/rolldown/tests/esbuild/default/string_export_names_common_js
## string export name not correct
- crates/rolldown/tests/esbuild/default/string_export_names_iife
## lowering not align
- crates/rolldown/tests/esbuild/default/this_inside_function
## this outside function behavior not align
- crates/rolldown/tests/esbuild/default/this_outside_function
## this undefined
- crates/rolldown/tests/esbuild/default/this_undefined_warning_esm
## redundant `require`
- crates/rolldown/tests/esbuild/default/to_esm_wrapper_omission
## there should not exist empty chunk
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_with_splitting
## import('./entry.js') should be rewrite to `require_entry`
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_with_splitting
## Can't disable bundle splitting
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_without_splitting
## inject path
- crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_issue1837
## should not drop `'use strict'`
- crates/rolldown/tests/esbuild/default/use_strict_directive_minify_no_bundle
## alias
- crates/rolldown/tests/esbuild/default/warnings_inside_node_modules
## esbuild did not needs `__toESM`
- crates/rolldown/tests/esbuild/loader/jsx_automatic_no_name_collision
## rolldown don't have `jsx.Preserve` and `jsx.Parse` option
- crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter
## lowering jsx
- crates/rolldown/tests/esbuild/loader/jsx_syntax_in_js_with_jsx_loader
## import record with attributes
- crates/rolldown/tests/esbuild/loader/loader_bundle_with_import_attributes
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
## Wrong wrapkind, when json is imported by `require`
- crates/rolldown/tests/esbuild/loader/loader_json_common_js_and_es6
## json tree shaking
- crates/rolldown/tests/esbuild/loader/loader_json_invalid_identifier_es6
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
## not support enum inline
- crates/rolldown/tests/esbuild/ts/ts_enum_jsx
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
## ts implicit extension
- crates/rolldown/tests/esbuild/ts/ts_implicit_extensions
## rolldown don't insert debug comments in css
- crates/rolldown/tests/esbuild/ts/ts_import_in_node_modules_name_collision_with_css
## resolve `mts` in ts
- crates/rolldown/tests/esbuild/ts/ts_import_mts
## commonjs json bundle
- crates/rolldown/tests/esbuild/ts/ts_minified_bundle_common_js
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
