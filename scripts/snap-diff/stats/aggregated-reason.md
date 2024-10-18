# Aggregate Reason
## not support legal comments
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
## Not support file loader
- crates/rolldown/tests/esbuild/loader/loader_file_common_js_and_es6
- crates/rolldown/tests/esbuild/loader/loader_file_ext_path_asset_names_js
- crates/rolldown/tests/esbuild/loader/loader_file_multiple_no_collision
- crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_css
- crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_js
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_css
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_js
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_css
- crates/rolldown/tests/esbuild/loader/loader_file_public_path_js
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_css
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_js
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_css
- crates/rolldown/tests/esbuild/loader/loader_file_relative_path_js
## lowering class
- crates/rolldown/tests/esbuild/dce/dce_class_static_blocks
- crates/rolldown/tests/esbuild/dce/dce_class_static_blocks_minify_syntax
- crates/rolldown/tests/esbuild/dce/dce_of_decorators
- crates/rolldown/tests/esbuild/dce/tree_shaking_import_identifier
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field_minified
- crates/rolldown/tests/esbuild/default/avoid_tdz
- crates/rolldown/tests/esbuild/default/avoid_tdz_no_bundle
- crates/rolldown/tests/esbuild/default/export_forms_es6
- crates/rolldown/tests/esbuild/default/new_expression_common_js
- crates/rolldown/tests/esbuild/default/this_inside_function
## not align
- crates/rolldown/tests/esbuild/dce/dead_code_following_jump
- crates/rolldown/tests/esbuild/default/indirect_require_message
- crates/rolldown/tests/esbuild/default/no_warn_common_js_exports_in_esm_pass_through
- crates/rolldown/tests/esbuild/default/node_annotation_false_positive_issue3544
- crates/rolldown/tests/esbuild/default/package_alias
- crates/rolldown/tests/esbuild/default/quoted_property_mangle
- crates/rolldown/tests/esbuild/default/rename_private_identifiers_no_bundle
- crates/rolldown/tests/esbuild/default/this_outside_function
- crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_without_splitting
- crates/rolldown/tests/esbuild/default/warn_common_js_exports_in_esm_bundle
## not support copy loader
- crates/rolldown/tests/esbuild/default/metafile_various_cases
- crates/rolldown/tests/esbuild/default/metafile_very_long_external_paths
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_entry_point
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_css
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_js
- crates/rolldown/tests/esbuild/loader/loader_copy_with_format
- crates/rolldown/tests/esbuild/loader/loader_copy_with_injected_file_bundle
- crates/rolldown/tests/esbuild/loader/loader_copy_with_transform
## needs css stable
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css_export_star_side_effects_false
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css_export_star_side_effects_false_only_js
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css_re_export_side_effects_false
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css_re_export_side_effects_false_only_js
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css_unused_nested_import_side_effects_false
- crates/rolldown/tests/esbuild/dce/tree_shaking_js_with_associated_css_unused_nested_import_side_effects_false_only_js
## `jsx.factory`
- crates/rolldown/tests/esbuild/default/import_re_export_es6_issue149
- crates/rolldown/tests/esbuild/default/jsx_import_meta_property
- crates/rolldown/tests/esbuild/default/jsx_import_meta_value
- crates/rolldown/tests/esbuild/default/jsx_this_property_common_js
- crates/rolldown/tests/esbuild/default/jsx_this_property_esm
- crates/rolldown/tests/esbuild/default/jsx_this_value_common_js
- crates/rolldown/tests/esbuild/default/jsx_this_value_esm
## side effects detect
- crates/rolldown/tests/esbuild/dce/dce_of_destructuring
- crates/rolldown/tests/esbuild/dce/dce_type_of_compare_string_guard_condition
- crates/rolldown/tests/esbuild/dce/dce_type_of_equals_string_guard_condition
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_main_use_main
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_cross_platform_slash
## Wrong output
- crates/rolldown/tests/esbuild/importstar/import_namespace_undefined_property_empty_file
- crates/rolldown/tests/esbuild/importstar/import_namespace_undefined_property_side_effect_free_file
- crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_common_js
- crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_iife
- crates/rolldown/tests/esbuild/loader/loader_json_shared_with_multiple_entries_issue413
## throw should be kept
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_intermediate_files_chain_all
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_intermediate_files_chain_one
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_intermediate_files_diamond
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_intermediate_files_used
## should rewrite `require`
- crates/rolldown/tests/esbuild/default/nested_require_without_call
- crates/rolldown/tests/esbuild/default/require_with_call_inside_try
- crates/rolldown/tests/esbuild/default/require_without_call
- crates/rolldown/tests/esbuild/default/require_without_call_inside_try
## double module initialization
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_main_implicit_main
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_array_keep_module_implicit_main
- crates/rolldown/tests/esbuild/dce/package_json_side_effects_false_keep_bare_import_and_require_es6
## not support import attributes
- crates/rolldown/tests/esbuild/default/comment_preservation_import_assertions
- crates/rolldown/tests/esbuild/default/metafile_import_with_type_json
- crates/rolldown/tests/esbuild/default/output_for_assert_type_json
## cjs module lexer can't recognize esbuild interop pattern
- crates/rolldown/tests/esbuild/default/export_forms_iife
- crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_common_js
- crates/rolldown/tests/esbuild/importstar/export_star_default_export_common_js
## limitation of test infra, the test may hard to pass in CI
- crates/rolldown/tests/esbuild/default/import_abs_path_as_dir
- crates/rolldown/tests/esbuild/default/import_abs_path_as_file
- crates/rolldown/tests/esbuild/default/import_abs_path_with_query_parameter
## css stabilization
- crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_end_of_file
- crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_css
- crates/rolldown/tests/esbuild/loader/loader_data_url_text_css
## different iife impl
- crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_iife_issue2264
- crates/rolldown/tests/esbuild/importstar/re_export_star_as_external_iife
- crates/rolldown/tests/esbuild/importstar/re_export_star_as_iife_no_bundle
## rolldown has redundant `require('external')`
- crates/rolldown/tests/esbuild/importstar/re_export_star_common_js_no_bundle
- crates/rolldown/tests/esbuild/importstar/re_export_star_entry_point_and_inner_file
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_common_js
## `.custom` should be treated as cjs
- crates/rolldown/tests/esbuild/loader/require_custom_extension_base64
- crates/rolldown/tests/esbuild/loader/require_custom_extension_data_url
- crates/rolldown/tests/esbuild/loader/require_custom_extension_string
## cross module constant folding
- crates/rolldown/tests/esbuild/dce/cross_module_constant_folding_number
- crates/rolldown/tests/esbuild/dce/cross_module_constant_folding_string
## drop label feature
- crates/rolldown/tests/esbuild/dce/drop_label_tree_shaking_bug_issue3311
- crates/rolldown/tests/esbuild/dce/drop_labels
## side effects detector not align
- crates/rolldown/tests/esbuild/dce/remove_unused_no_side_effects_tagged_templates
- crates/rolldown/tests/esbuild/dce/tree_shaking_binary_operators
## comments codegen
- crates/rolldown/tests/esbuild/default/comment_preservation
- crates/rolldown/tests/esbuild/default/comment_preservation_transform_jsx
## should not rewrite `fs` to `node:fs`
- crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_es6
- crates/rolldown/tests/esbuild/default/import_fs_node_common_js
## hashban not align
- crates/rolldown/tests/esbuild/default/hashbang_banner_use_strict_order
- crates/rolldown/tests/esbuild/default/hashbang_bundle
## rolldown split chunks
- crates/rolldown/tests/esbuild/default/import_namespace_this_value
- crates/rolldown/tests/esbuild/default/multiple_entry_points_same_name_collision
## should not appear `await`
- crates/rolldown/tests/esbuild/default/top_level_await_iife_dead_branch
- crates/rolldown/tests/esbuild/default/top_level_await_no_bundle_common_js_dead_branch
## rolldown has redundant `import "external"`
- crates/rolldown/tests/esbuild/importstar/re_export_star_es6_no_bundle
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_es6
## Wrong impl
- crates/rolldown/tests/esbuild/importstar/re_export_star_external_iife
- crates/rolldown/tests/esbuild/importstar/re_export_star_iife_no_bundle
## should inline variable
- crates/rolldown/tests/esbuild/loader/loader_json_prototype
- crates/rolldown/tests/esbuild/loader/loader_json_prototype_es5
## `.txt` should be treated as cjs
- crates/rolldown/tests/esbuild/loader/loader_text_common_js_and_es6
- crates/rolldown/tests/esbuild/loader/require_custom_extension_prefer_longest
## lower decorator
- crates/rolldown/tests/esbuild/dce/dce_of_experimental_decorators
## don't support dce iife
- crates/rolldown/tests/esbuild/dce/dce_of_iife
## rolldown don't have `ignoreDCEAnnotations` option
- crates/rolldown/tests/esbuild/dce/disable_tree_shaking
## Related to codegen
- crates/rolldown/tests/esbuild/dce/inline_function_call_behavior_changes
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
## seems esbuild mark static field as side effects whatever, should investigate
- crates/rolldown/tests/esbuild/dce/tree_shaking_lowered_class_static_field_assignment
## related to minifier
- crates/rolldown/tests/esbuild/default/arguments_special_case_no_bundle
## the deconflict of no top level is sub optimal
- crates/rolldown/tests/esbuild/default/arrow_fn_scope
## should not transform `{default as fs}`
- crates/rolldown/tests/esbuild/default/auto_external_node
## `node:path` is side effects free
- crates/rolldown/tests/esbuild/default/auto_external_node
## It seems rolldown rewrite `fs` to `node:fs`
- crates/rolldown/tests/esbuild/default/built_in_node_module_precedence
## needs custom resolver
- crates/rolldown/tests/esbuild/default/bundling_files_outside_of_outbase
## not support `jsx.preserve`
- crates/rolldown/tests/esbuild/default/comment_preservation_preserve_jsx
## not support conditional import
- crates/rolldown/tests/esbuild/default/conditional_import
## not support conditional require
- crates/rolldown/tests/esbuild/default/conditional_require
## not support conditional `require.resolve`
- crates/rolldown/tests/esbuild/default/conditional_require_resolve
## inline could be done in minifier
- crates/rolldown/tests/esbuild/default/const_with_let_no_bundle
## oxc dead branch remove
- crates/rolldown/tests/esbuild/default/const_with_let_no_mangle
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
## oxc define dont support this expr
- crates/rolldown/tests/esbuild/default/define_this
## redundant `__toCommonJS`
- crates/rolldown/tests/esbuild/default/export_forms_common_js
## Not sure if we needs to use `Object.define` pattern in iife
- crates/rolldown/tests/esbuild/default/export_forms_iife
## needs to disable split chunks
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
## commonjs don't have `import.meta`, should rewrite
- crates/rolldown/tests/esbuild/default/import_meta_common_js
## rolldown extract common module
- crates/rolldown/tests/esbuild/default/import_missing_neither_es6_nor_common_js
## different naming style
- crates/rolldown/tests/esbuild/default/jsx_automatic_imports_common_js
## wrong tree shaking result
- crates/rolldown/tests/esbuild/default/mangle_no_quoted_props
## not support file loader
- crates/rolldown/tests/esbuild/default/metafile_very_long_external_paths
## not support preserve `jsx`
- crates/rolldown/tests/esbuild/default/minified_jsx_preserve_with_object_spread
## don't rewrite top level binding
- crates/rolldown/tests/esbuild/default/named_function_expression_argument_collision
## should read `tsconfig.json`
- crates/rolldown/tests/esbuild/default/non_determinism_issue2537
## alias not align
- crates/rolldown/tests/esbuild/default/package_alias_match_longest
## not support invalid template
- crates/rolldown/tests/esbuild/default/require_and_dynamic_import_invalid_template
## should rewrite when bad arg count
- crates/rolldown/tests/esbuild/default/require_bad_argument_count
## require json should not wrapped in `__esm`
- crates/rolldown/tests/esbuild/default/require_json
## require `.json`, the json file should not wrapped in `__esm`
- crates/rolldown/tests/esbuild/default/require_shim_substitution
## `.txt` module should be treated as cjs
- crates/rolldown/tests/esbuild/default/require_txt
## obviously, the output is incorrect
- crates/rolldown/tests/esbuild/default/string_export_names_common_js
## string export name not correct
- crates/rolldown/tests/esbuild/default/string_export_names_iife
## this undefined
- crates/rolldown/tests/esbuild/default/this_undefined_warning_esm
## should not appear top level `await` in cjs
- crates/rolldown/tests/esbuild/default/top_level_await_cjs_dead_branch
## should not appear `__commonJS`
- crates/rolldown/tests/esbuild/default/top_level_await_forbidden_require_dead_branch
## inject path
- crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_issue1837
## should not drop `'use strict'`
- crates/rolldown/tests/esbuild/default/use_strict_directive_minify_no_bundle
## Wrong impl when module.exports self
- crates/rolldown/tests/esbuild/importstar/export_self_common_js_minified
## Wrong self iife
- crates/rolldown/tests/esbuild/importstar/export_self_iife
## Wrong default import linking output
- crates/rolldown/tests/esbuild/importstar/import_default_namespace_combo_issue446
## Format cjs should not appear `export`
- crates/rolldown/tests/esbuild/importstar/import_self_common_js
## sub optimal
- crates/rolldown/tests/esbuild/importstar/import_star_common_js_unused
## esbuild will reuse `ns` variable
- crates/rolldown/tests/esbuild/importstar/import_star_common_js_unused
## esbuild treated svg as commonjs module, rolldown treated it as esm
- crates/rolldown/tests/esbuild/loader/auto_detect_mime_type_from_extension
## esbuild will wrap `empty` module as a cjs module, rolldown did not
- crates/rolldown/tests/esbuild/loader/empty_loader_js
## esbuild did not needs `__toESM`
- crates/rolldown/tests/esbuild/loader/jsx_automatic_no_name_collision
## rolldown don't have `jsx.Preserve` and `jsx.Parse` option
- crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter
## lowering jsx
- crates/rolldown/tests/esbuild/loader/jsx_syntax_in_js_with_jsx_loader
## esbuild treated x.b64 as cjs, rolldown treated it as esm
- crates/rolldown/tests/esbuild/loader/loader_base64_common_js_and_es6
## import record with attributes
- crates/rolldown/tests/esbuild/loader/loader_bundle_with_import_attributes
## esbuild treated `.txt` as cjs, rolldown treated it as esm
- crates/rolldown/tests/esbuild/loader/loader_data_url_common_js_and_es6
## Wrong wrapkind, when json is imported by `require`
- crates/rolldown/tests/esbuild/loader/loader_json_common_js_and_es6
## json tree shaking
- crates/rolldown/tests/esbuild/loader/loader_json_invalid_identifier_es6
## should treated it as cjs module
- crates/rolldown/tests/esbuild/loader/loader_json_no_bundle
## the base64 result is also wrong
- crates/rolldown/tests/esbuild/loader/require_custom_extension_base64
## Not support json attributes
- crates/rolldown/tests/esbuild/loader/with_type_json_override_loader
