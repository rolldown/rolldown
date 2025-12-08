# Failed Cases
## [argument_default_value_scope_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/argument_default_value_scope_no_bundle/diff.md)
  lowering class
## [comment_preservation](../../../../../crates/rolldown/tests/esbuild/default/comment_preservation/diff.md)
  comments codegen related to `oxc` and the original test case is `ModePassThrough`
## [comment_preservation_transform_jsx](../../../../../crates/rolldown/tests/esbuild/default/comment_preservation_transform_jsx/diff.md)
  transpiled jsx should have leading `@__PURE__`, already tracked https://github.com/oxc-project/oxc/issues/6072
## [conditional_import](../../../../../crates/rolldown/tests/esbuild/default/conditional_import/diff.md)
  esbuild will wrap `Promise.resolve().then() for original specifier`
## [define_import_meta](../../../../../crates/rolldown/tests/esbuild/default/define_import_meta/diff.md)
  oxc define
## [define_import_meta_es5](../../../../../crates/rolldown/tests/esbuild/default/define_import_meta_es5/diff.md)
  don't see necessarity to auto polyfill `import.meta` since we already support `define`
## [direct_eval_tainting_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/direct_eval_tainting_no_bundle/diff.md)
  sub optimal: eval in `test4` param position don't need to be renamed
## [duplicate_entry_point](../../../../../crates/rolldown/tests/esbuild/default/duplicate_entry_point/diff.md)
  rolldown try to extract common module when duplicate entry point
## [entry_names_chunk_names_ext_placeholder](../../../../../crates/rolldown/tests/esbuild/default/entry_names_chunk_names_ext_placeholder/diff.md)
  css comments and different chunk file naming style
## [export_forms_common_js](../../../../../crates/rolldown/tests/esbuild/default/export_forms_common_js/diff.md)
  redundant `__toCommonJS`
## [export_forms_with_minify_identifiers_and_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/export_forms_with_minify_identifiers_and_no_bundle/diff.md)
  should not generate duplicate export binding
## [external_es6_converted_to_common_js](../../../../../crates/rolldown/tests/esbuild/default/external_es6_converted_to_common_js/diff.md)
  redundant `import` statements
## [false_require](../../../../../crates/rolldown/tests/esbuild/default/false_require/diff.md)
  should rename `require` when it is appear in param position
## [import_abs_path_with_query_parameter](../../../../../crates/rolldown/tests/esbuild/default/import_abs_path_with_query_parameter/diff.md)
  query and hashban in specifier
## [import_meta_common_js](../../../../../crates/rolldown/tests/esbuild/default/import_meta_common_js/diff.md)
  rolldown keep unsupported `import.meta` as it is in cjs format.
## [import_missing_neither_es6_nor_common_js](../../../../../crates/rolldown/tests/esbuild/default/import_missing_neither_es6_nor_common_js/diff.md)
  rolldown extract common module
## [import_namespace_this_value](../../../../../crates/rolldown/tests/esbuild/default/import_namespace_this_value/diff.md)
  rolldown split chunks
## [indirect_require_message](../../../../../crates/rolldown/tests/esbuild/default/indirect_require_message/diff.md)
  not align
## [inject_import_meta](../../../../../crates/rolldown/tests/esbuild/default/inject_import_meta/diff.md)
  different inject implementation
## [inject_jsx](../../../../../crates/rolldown/tests/esbuild/default/inject_jsx/diff.md)
  due to multi pass transformer arch, this test could not be supported for now(we should `Define` first and then `Transform`).
## [inject_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/inject_no_bundle/diff.md)
  generate wrong syntax when Exported is `StringLiteral`, and rest part of esbuild gen is weird since there is no need to rename
## [mangle_props_import_export](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_import_export/diff.md)
  for `__require` diff, we don't have ModePassThrough
## [no_warn_common_js_exports_in_esm_pass_through](../../../../../crates/rolldown/tests/esbuild/default/no_warn_common_js_exports_in_esm_pass_through/diff.md)
  We don't have pass through mode, we just have same output as esbuild if
## [non_determinism_issue2537](../../../../../crates/rolldown/tests/esbuild/default/non_determinism_issue2537/diff.md)
  should read `tsconfig.json`
## [package_alias](../../../../../crates/rolldown/tests/esbuild/default/package_alias/diff.md)
  resolve alias
## [package_alias_match_longest](../../../../../crates/rolldown/tests/esbuild/default/package_alias_match_longest/diff.md)
  alias not align
## [rename_private_identifiers_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/rename_private_identifiers_no_bundle/diff.md)
  rename private identifier
## [string_export_names_common_js](../../../../../crates/rolldown/tests/esbuild/default/string_export_names_common_js/diff.md)
  should not reuse `__toESM(require('./foo'))`
## [string_export_names_iife](../../../../../crates/rolldown/tests/esbuild/default/string_export_names_iife/diff.md)
  string export name not correct
## [this_inside_function](../../../../../crates/rolldown/tests/esbuild/default/this_inside_function/diff.md)
  lowering not align
## [top_level_await_allowed_import_with_splitting](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_with_splitting/diff.md)
  there should not exist empty chunk
## [top_level_await_allowed_import_without_splitting](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_allowed_import_without_splitting/diff.md)
  Can't disable bundle splitting
## [use_strict_directive_bundle_issue1837](../../../../../crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_issue1837/diff.md)
  inject path
## [warnings_inside_node_modules](../../../../../crates/rolldown/tests/esbuild/default/warnings_inside_node_modules/diff.md)
  alias
# Passed Cases
## [ambiguous_reexport_msg](../../../../../crates/rolldown/tests/esbuild/default/ambiguous_reexport_msg)
## [arguments_special_case_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/arguments_special_case_no_bundle)
## [arrow_fn_scope](../../../../../crates/rolldown/tests/esbuild/default/arrow_fn_scope)
## [auto_external](../../../../../crates/rolldown/tests/esbuild/default/auto_external)
## [auto_external_node](../../../../../crates/rolldown/tests/esbuild/default/auto_external_node)
## [avoid_tdz](../../../../../crates/rolldown/tests/esbuild/default/avoid_tdz)
## [avoid_tdz_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/avoid_tdz_no_bundle)
## [await_import_inside_try](../../../../../crates/rolldown/tests/esbuild/default/await_import_inside_try)
## [built_in_node_module_precedence](../../../../../crates/rolldown/tests/esbuild/default/built_in_node_module_precedence)
## [bundling_files_outside_of_outbase](../../../../../crates/rolldown/tests/esbuild/default/bundling_files_outside_of_outbase)
## [char_freq_ignore_comments](../../../../../crates/rolldown/tests/esbuild/default/char_freq_ignore_comments)
## [common_js_from_es6](../../../../../crates/rolldown/tests/esbuild/default/common_js_from_es6)
## [conditional_require](../../../../../crates/rolldown/tests/esbuild/default/conditional_require)
## [const_with_let](../../../../../crates/rolldown/tests/esbuild/default/const_with_let)
## [const_with_let_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/const_with_let_no_bundle)
## [const_with_let_no_mangle](../../../../../crates/rolldown/tests/esbuild/default/const_with_let_no_mangle)
## [define_assign_warning](../../../../../crates/rolldown/tests/esbuild/default/define_assign_warning)
## [define_infinite_loop_issue2407](../../../../../crates/rolldown/tests/esbuild/default/define_infinite_loop_issue2407)
## [define_optional_chain](../../../../../crates/rolldown/tests/esbuild/default/define_optional_chain)
## [define_optional_chain_lowered](../../../../../crates/rolldown/tests/esbuild/default/define_optional_chain_lowered)
## [define_optional_chain_panic_issue3551](../../../../../crates/rolldown/tests/esbuild/default/define_optional_chain_panic_issue3551)
## [define_this](../../../../../crates/rolldown/tests/esbuild/default/define_this)
## [dot_import](../../../../../crates/rolldown/tests/esbuild/default/dot_import)
## [duplicate_property_warning](../../../../../crates/rolldown/tests/esbuild/default/duplicate_property_warning)
## [dynamic_import_with_expression_cjs](../../../../../crates/rolldown/tests/esbuild/default/dynamic_import_with_expression_cjs)
## [dynamic_import_with_template_iife](../../../../../crates/rolldown/tests/esbuild/default/dynamic_import_with_template_iife)
## [empty_export_clause_bundle_as_common_js_issue910](../../../../../crates/rolldown/tests/esbuild/default/empty_export_clause_bundle_as_common_js_issue910)
## [es6_from_common_js](../../../../../crates/rolldown/tests/esbuild/default/es6_from_common_js)
## [export_chain](../../../../../crates/rolldown/tests/esbuild/default/export_chain)
## [export_forms_es6](../../../../../crates/rolldown/tests/esbuild/default/export_forms_es6)
## [export_forms_iife](../../../../../crates/rolldown/tests/esbuild/default/export_forms_iife)
## [export_fs_node](../../../../../crates/rolldown/tests/esbuild/default/export_fs_node)
## [export_fs_node_in_common_js_module](../../../../../crates/rolldown/tests/esbuild/default/export_fs_node_in_common_js_module)
## [export_wildcard_fs_node_common_js](../../../../../crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_common_js)
## [export_wildcard_fs_node_es6](../../../../../crates/rolldown/tests/esbuild/default/export_wildcard_fs_node_es6)
## [exports_and_module_format_common_js](../../../../../crates/rolldown/tests/esbuild/default/exports_and_module_format_common_js)
## [external_module_exclusion_package](../../../../../crates/rolldown/tests/esbuild/default/external_module_exclusion_package)
## [external_module_exclusion_relative_path](../../../../../crates/rolldown/tests/esbuild/default/external_module_exclusion_relative_path)
## [external_packages](../../../../../crates/rolldown/tests/esbuild/default/external_packages)
## [external_wildcard_does_not_match_entry_point](../../../../../crates/rolldown/tests/esbuild/default/external_wildcard_does_not_match_entry_point)
## [hashbang_banner_use_strict_order](../../../../../crates/rolldown/tests/esbuild/default/hashbang_banner_use_strict_order)
## [hashbang_bundle](../../../../../crates/rolldown/tests/esbuild/default/hashbang_bundle)
## [hashbang_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/hashbang_no_bundle)
## [iife_es5](../../../../../crates/rolldown/tests/esbuild/default/iife_es5)
## [import_forms_with_minify_identifiers_and_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/import_forms_with_minify_identifiers_and_no_bundle)
## [import_forms_with_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/import_forms_with_no_bundle)
## [import_fs_node_common_js](../../../../../crates/rolldown/tests/esbuild/default/import_fs_node_common_js)
## [import_fs_node_es6](../../../../../crates/rolldown/tests/esbuild/default/import_fs_node_es6)
## [import_meta_es6](../../../../../crates/rolldown/tests/esbuild/default/import_meta_es6)
## [import_meta_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/import_meta_no_bundle)
## [import_missing_common_js](../../../../../crates/rolldown/tests/esbuild/default/import_missing_common_js)
## [import_re_export_es6_issue149](../../../../../crates/rolldown/tests/esbuild/default/import_re_export_es6_issue149)
## [import_then_catch](../../../../../crates/rolldown/tests/esbuild/default/import_then_catch)
## [import_with_hash_in_path](../../../../../crates/rolldown/tests/esbuild/default/import_with_hash_in_path)
## [inject](../../../../../crates/rolldown/tests/esbuild/default/inject)
## [inject_jsx_dot_names](../../../../../crates/rolldown/tests/esbuild/default/inject_jsx_dot_names)
## [inject_with_define](../../../../../crates/rolldown/tests/esbuild/default/inject_with_define)
## [jsx_automatic_imports_common_js](../../../../../crates/rolldown/tests/esbuild/default/jsx_automatic_imports_common_js)
## [jsx_automatic_imports_es6](../../../../../crates/rolldown/tests/esbuild/default/jsx_automatic_imports_es6)
## [jsx_imports_common_js](../../../../../crates/rolldown/tests/esbuild/default/jsx_imports_common_js)
## [jsx_imports_es6](../../../../../crates/rolldown/tests/esbuild/default/jsx_imports_es6)
## [jsx_this_property_common_js](../../../../../crates/rolldown/tests/esbuild/default/jsx_this_property_common_js)
## [jsx_this_property_esm](../../../../../crates/rolldown/tests/esbuild/default/jsx_this_property_esm)
## [jsx_this_value_common_js](../../../../../crates/rolldown/tests/esbuild/default/jsx_this_value_common_js)
## [jsx_this_value_esm](../../../../../crates/rolldown/tests/esbuild/default/jsx_this_value_esm)
## [keep_names_all_forms](../../../../../crates/rolldown/tests/esbuild/default/keep_names_all_forms)
## [keep_names_class_static_name](../../../../../crates/rolldown/tests/esbuild/default/keep_names_class_static_name)
## [keep_names_tree_shaking](../../../../../crates/rolldown/tests/esbuild/default/keep_names_tree_shaking)
## [legal_comments_merge_duplicates_issue4139](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_merge_duplicates_issue4139)
## [mangle_no_quoted_props](../../../../../crates/rolldown/tests/esbuild/default/mangle_no_quoted_props)
## [mangle_no_quoted_props_minify_syntax](../../../../../crates/rolldown/tests/esbuild/default/mangle_no_quoted_props_minify_syntax)
## [mangle_props](../../../../../crates/rolldown/tests/esbuild/default/mangle_props)
## [mangle_props_avoid_collisions](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_avoid_collisions)
## [mangle_props_import_export_bundled](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_import_export_bundled)
## [mangle_props_jsx_preserve](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_jsx_preserve)
## [mangle_props_jsx_transform](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_jsx_transform)
## [mangle_props_key_comment](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_key_comment)
## [mangle_props_key_comment_minify](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_key_comment_minify)
## [mangle_props_keyword_property_minify](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_keyword_property_minify)
## [mangle_props_lowered_class_fields](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_lowered_class_fields)
## [mangle_props_lowered_optional_chain](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_lowered_optional_chain)
## [mangle_props_minify](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_minify)
## [mangle_props_no_shorthand](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_no_shorthand)
## [mangle_props_optional_chain](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_optional_chain)
## [mangle_props_shorthand](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_shorthand)
## [mangle_props_super_call](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_super_call)
## [mangle_quoted_props](../../../../../crates/rolldown/tests/esbuild/default/mangle_quoted_props)
## [mangle_quoted_props_minify_syntax](../../../../../crates/rolldown/tests/esbuild/default/mangle_quoted_props_minify_syntax)
## [many_entry_points](../../../../../crates/rolldown/tests/esbuild/default/many_entry_points)
## [metafile_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/metafile_no_bundle)
## [minified_bundle_common_js](../../../../../crates/rolldown/tests/esbuild/default/minified_bundle_common_js)
## [minified_bundle_ending_with_important_semicolon](../../../../../crates/rolldown/tests/esbuild/default/minified_bundle_ending_with_important_semicolon)
## [minified_bundle_es6](../../../../../crates/rolldown/tests/esbuild/default/minified_bundle_es6)
## [minified_dynamic_import_with_expression_cjs](../../../../../crates/rolldown/tests/esbuild/default/minified_dynamic_import_with_expression_cjs)
## [minified_exports_and_module_format_common_js](../../../../../crates/rolldown/tests/esbuild/default/minified_exports_and_module_format_common_js)
## [minify_arguments](../../../../../crates/rolldown/tests/esbuild/default/minify_arguments)
## [minify_identifiers_import_path_frequency_analysis](../../../../../crates/rolldown/tests/esbuild/default/minify_identifiers_import_path_frequency_analysis)
## [minify_private_identifiers_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/minify_private_identifiers_no_bundle)
## [minify_sibling_labels_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/minify_sibling_labels_no_bundle)
## [multiple_entry_points_same_name_collision](../../../../../crates/rolldown/tests/esbuild/default/multiple_entry_points_same_name_collision)
## [named_function_expression_argument_collision](../../../../../crates/rolldown/tests/esbuild/default/named_function_expression_argument_collision)
## [nested_common_js](../../../../../crates/rolldown/tests/esbuild/default/nested_common_js)
## [nested_es6_from_common_js](../../../../../crates/rolldown/tests/esbuild/default/nested_es6_from_common_js)
## [nested_require_without_call](../../../../../crates/rolldown/tests/esbuild/default/nested_require_without_call)
## [nested_scope_bug](../../../../../crates/rolldown/tests/esbuild/default/nested_scope_bug)
## [new_expression_common_js](../../../../../crates/rolldown/tests/esbuild/default/new_expression_common_js)
## [node_annotation_false_positive_issue3544](../../../../../crates/rolldown/tests/esbuild/default/node_annotation_false_positive_issue3544)
## [node_annotation_invalid_identifier_issue4100](../../../../../crates/rolldown/tests/esbuild/default/node_annotation_invalid_identifier_issue4100)
## [node_modules](../../../../../crates/rolldown/tests/esbuild/default/node_modules)
## [object_literal_proto_setter_edge_cases](../../../../../crates/rolldown/tests/esbuild/default/object_literal_proto_setter_edge_cases)
## [object_literal_proto_setter_edge_cases_minify_syntax](../../../../../crates/rolldown/tests/esbuild/default/object_literal_proto_setter_edge_cases_minify_syntax)
## [outbase](../../../../../crates/rolldown/tests/esbuild/default/outbase)
## [output_extension_remapping_dir](../../../../../crates/rolldown/tests/esbuild/default/output_extension_remapping_dir)
## [output_extension_remapping_file](../../../../../crates/rolldown/tests/esbuild/default/output_extension_remapping_file)
## [preserve_key_comment](../../../../../crates/rolldown/tests/esbuild/default/preserve_key_comment)
## [quoted_property](../../../../../crates/rolldown/tests/esbuild/default/quoted_property)
## [re_export_common_js_as_es6](../../../../../crates/rolldown/tests/esbuild/default/re_export_common_js_as_es6)
## [re_export_default_external_common_js](../../../../../crates/rolldown/tests/esbuild/default/re_export_default_external_common_js)
## [re_export_default_external_es6](../../../../../crates/rolldown/tests/esbuild/default/re_export_default_external_es6)
## [re_export_default_internal](../../../../../crates/rolldown/tests/esbuild/default/re_export_default_internal)
## [re_export_default_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/re_export_default_no_bundle)
## [re_export_default_no_bundle_common_js](../../../../../crates/rolldown/tests/esbuild/default/re_export_default_no_bundle_common_js)
## [re_export_default_no_bundle_es6](../../../../../crates/rolldown/tests/esbuild/default/re_export_default_no_bundle_es6)
## [re_export_fs_node](../../../../../crates/rolldown/tests/esbuild/default/re_export_fs_node)
## [rename_labels_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/rename_labels_no_bundle)
## [require_bad_argument_count](../../../../../crates/rolldown/tests/esbuild/default/require_bad_argument_count)
## [require_child_dir_common_js](../../../../../crates/rolldown/tests/esbuild/default/require_child_dir_common_js)
## [require_child_dir_es6](../../../../../crates/rolldown/tests/esbuild/default/require_child_dir_es6)
## [require_fs_node](../../../../../crates/rolldown/tests/esbuild/default/require_fs_node)
## [require_fs_node_minify](../../../../../crates/rolldown/tests/esbuild/default/require_fs_node_minify)
## [require_json](../../../../../crates/rolldown/tests/esbuild/default/require_json)
## [require_main_cache_common_js](../../../../../crates/rolldown/tests/esbuild/default/require_main_cache_common_js)
## [require_parent_dir_common_js](../../../../../crates/rolldown/tests/esbuild/default/require_parent_dir_common_js)
## [require_parent_dir_es6](../../../../../crates/rolldown/tests/esbuild/default/require_parent_dir_es6)
## [require_property_access_common_js](../../../../../crates/rolldown/tests/esbuild/default/require_property_access_common_js)
## [require_resolve](../../../../../crates/rolldown/tests/esbuild/default/require_resolve)
## [require_txt](../../../../../crates/rolldown/tests/esbuild/default/require_txt)
## [require_with_call_inside_try](../../../../../crates/rolldown/tests/esbuild/default/require_with_call_inside_try)
## [require_with_template](../../../../../crates/rolldown/tests/esbuild/default/require_with_template)
## [require_without_call](../../../../../crates/rolldown/tests/esbuild/default/require_without_call)
## [require_without_call_inside_try](../../../../../crates/rolldown/tests/esbuild/default/require_without_call_inside_try)
## [reserve_props](../../../../../crates/rolldown/tests/esbuild/default/reserve_props)
## [runtime_name_collision_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/runtime_name_collision_no_bundle)
## [scoped_external_module_exclusion](../../../../../crates/rolldown/tests/esbuild/default/scoped_external_module_exclusion)
## [simple_common_js](../../../../../crates/rolldown/tests/esbuild/default/simple_common_js)
## [simple_es6](../../../../../crates/rolldown/tests/esbuild/default/simple_es6)
## [source_identifier_name_index_multiple_entry](../../../../../crates/rolldown/tests/esbuild/default/source_identifier_name_index_multiple_entry)
## [source_identifier_name_index_single_entry](../../../../../crates/rolldown/tests/esbuild/default/source_identifier_name_index_single_entry)
## [source_map](../../../../../crates/rolldown/tests/esbuild/default/source_map)
## [strict_mode_nested_fn_decl_keep_names_variable_inlining_issue1552](../../../../../crates/rolldown/tests/esbuild/default/strict_mode_nested_fn_decl_keep_names_variable_inlining_issue1552)
## [switch_scope_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/switch_scope_no_bundle)
## [this_outside_function](../../../../../crates/rolldown/tests/esbuild/default/this_outside_function)
## [this_undefined_warning_esm](../../../../../crates/rolldown/tests/esbuild/default/this_undefined_warning_esm)
## [to_esm_wrapper_omission](../../../../../crates/rolldown/tests/esbuild/default/to_esm_wrapper_omission)
## [top_level_await_cjs_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_cjs_dead_branch)
## [top_level_await_esm](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_esm)
## [top_level_await_esm_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_esm_dead_branch)
## [top_level_await_forbidden_require_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_forbidden_require_dead_branch)
## [top_level_await_iife_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_iife_dead_branch)
## [top_level_await_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_no_bundle)
## [top_level_await_no_bundle_common_js_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_no_bundle_common_js_dead_branch)
## [top_level_await_no_bundle_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_no_bundle_dead_branch)
## [top_level_await_no_bundle_esm](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_no_bundle_esm)
## [top_level_await_no_bundle_esm_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_no_bundle_esm_dead_branch)
## [top_level_await_no_bundle_iife_dead_branch](../../../../../crates/rolldown/tests/esbuild/default/top_level_await_no_bundle_iife_dead_branch)
## [use_strict_directive_bundle_cjs_issue2264](../../../../../crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_cjs_issue2264)
## [use_strict_directive_bundle_esm_issue2264](../../../../../crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_esm_issue2264)
## [use_strict_directive_bundle_iife_issue2264](../../../../../crates/rolldown/tests/esbuild/default/use_strict_directive_bundle_iife_issue2264)
## [use_strict_directive_minify_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/use_strict_directive_minify_no_bundle)
## [var_relocating_bundle](../../../../../crates/rolldown/tests/esbuild/default/var_relocating_bundle)
## [var_relocating_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/var_relocating_no_bundle)
## [warn_common_js_exports_in_esm_bundle](../../../../../crates/rolldown/tests/esbuild/default/warn_common_js_exports_in_esm_bundle)
## [warn_common_js_exports_in_esm_convert](../../../../../crates/rolldown/tests/esbuild/default/warn_common_js_exports_in_esm_convert)
## [with_statement_tainting_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/with_statement_tainting_no_bundle)
# Ignored Cases
## [call_import_namespace_warning](../../../../../crates/rolldown/tests/esbuild/default/call_import_namespace_warning)
  TODO
## [decorator_printing_cjs](../../../../../crates/rolldown/tests/esbuild/default/decorator_printing_cjs)
  TODO
## [decorator_printing_esm](../../../../../crates/rolldown/tests/esbuild/default/decorator_printing_esm)
  TODO
## [entry_names_no_slash_after_dir](../../../../../crates/rolldown/tests/esbuild/default/entry_names_no_slash_after_dir)
  TODO
## [entry_names_non_portable_character](../../../../../crates/rolldown/tests/esbuild/default/entry_names_non_portable_character)
  TODO
## [export_special_name](../../../../../crates/rolldown/tests/esbuild/default/export_special_name)
  TODO
## [export_special_name_bundle](../../../../../crates/rolldown/tests/esbuild/default/export_special_name_bundle)
  TODO
## [import_abs_path_as_dir](../../../../../crates/rolldown/tests/esbuild/default/import_abs_path_as_dir)
  limitation of test infra, the test may hard to pass in CI
## [import_abs_path_as_file](../../../../../crates/rolldown/tests/esbuild/default/import_abs_path_as_file)
  limitation of test infra, the test may hard to pass in CI
## [import_with_hash_parameter](../../../../../crates/rolldown/tests/esbuild/default/import_with_hash_parameter)
  TODO
## [import_with_query_parameter](../../../../../crates/rolldown/tests/esbuild/default/import_with_query_parameter)
  TODO
## [inject_duplicate](../../../../../crates/rolldown/tests/esbuild/default/inject_duplicate)
  `oxc` inject align with `@rollup/plugin-inject` don't support inject source file directly
## [inject_import_order](../../../../../crates/rolldown/tests/esbuild/default/inject_import_order)
  `oxc` inject align with `@rollup/plugin-inject` don't support inject files directly
## [inject_import_ts](../../../../../crates/rolldown/tests/esbuild/default/inject_import_ts)
  `oxc` inject align with `@rollup/plugin-inject` don't support inject files directly
## [inject_with_string_export_name_bundle](../../../../../crates/rolldown/tests/esbuild/default/inject_with_string_export_name_bundle)
  replace the function it self in `inject files`, this align with `@rollup/plugin-inject`
## [inject_with_string_export_name_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/inject_with_string_export_name_no_bundle)
  replace the function it self in `inject files`, this align with `@rollup/plugin-inject`
## [jsx_constant_fragments](../../../../../crates/rolldown/tests/esbuild/default/jsx_constant_fragments)
  TODO
## [jsx_dev_self_edge_cases](../../../../../crates/rolldown/tests/esbuild/default/jsx_dev_self_edge_cases)
  TODO
## [jsx_import_meta_property](../../../../../crates/rolldown/tests/esbuild/default/jsx_import_meta_property)
  don't support `unsupportedFeature` https://github.com/evanw/esbuild/commit/71a2f8de5ad4e1882f35c449efa25761aa1241b5#diff-e20508c4ae566a2d8a60274ff05e408d81c9758a27d84318feecdfbf9e24af5eR11297-R11308
## [jsx_import_meta_value](../../../../../crates/rolldown/tests/esbuild/default/jsx_import_meta_value)
  don't support `unsupportedFeature` https://github.com/evanw/esbuild/commit/71a2f8de5ad4e1882f35c449efa25761aa1241b5#diff-e20508c4ae566a2d8a60274ff05e408d81c9758a27d84318feecdfbf9e24af5eR11297-R11308
## [line_limit_minified](../../../../../crates/rolldown/tests/esbuild/default/line_limit_minified)
  TODO
## [line_limit_not_minified](../../../../../crates/rolldown/tests/esbuild/default/line_limit_not_minified)
  TODO
## [mangle_props_jsx_transform_namespace](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_jsx_transform_namespace)
  TODO
## [mangle_props_type_script_features](../../../../../crates/rolldown/tests/esbuild/default/mangle_props_type_script_features)
  TODO
## [minify_nested_labels_no_bundle](../../../../../crates/rolldown/tests/esbuild/default/minify_nested_labels_no_bundle)
  TODO
## [quoted_property_mangle](../../../../../crates/rolldown/tests/esbuild/default/quoted_property_mangle)
  Currently there is no way to control *quoted* behavior, since we use `oxc` to convert ast to string and we just generate same output as esbuild if disable `MinifySyntax`
## [this_with_es6_syntax](../../../../../crates/rolldown/tests/esbuild/default/this_with_es6_syntax)
  TODO
# Ignored Cases (not supported)
## [comment_preservation_import_assertions](../../../../../crates/rolldown/tests/esbuild/default/comment_preservation_import_assertions)
  not support import attributes
## [comment_preservation_preserve_jsx](../../../../../crates/rolldown/tests/esbuild/default/comment_preservation_preserve_jsx)
  not support `jsx.preserve`
## [conditional_require_resolve](../../../../../crates/rolldown/tests/esbuild/default/conditional_require_resolve)
  not support conditional `require.resolve`
## [legal_comments_avoid_slash_tag_end_of_file](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_end_of_file)
  not support legal comments
## [legal_comments_avoid_slash_tag_external](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_external)
  not support legal comments
## [legal_comments_avoid_slash_tag_inline](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_avoid_slash_tag_inline)
  not support legal comments
## [legal_comments_end_of_file](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_end_of_file)
  not support legal comments
## [legal_comments_escape_slash_script_and_style_end_of_file](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_escape_slash_script_and_style_end_of_file)
  not support legal comments
## [legal_comments_escape_slash_script_and_style_external](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_escape_slash_script_and_style_external)
  not support legal comments
## [legal_comments_external](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_external)
  not support legal comments
## [legal_comments_inline](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_inline)
  not support legal comments
## [legal_comments_linked](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_linked)
  not support legal comments
## [legal_comments_many_end_of_file](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_many_end_of_file)
  not support legal comments
## [legal_comments_many_linked](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_many_linked)
  not support legal comments
## [legal_comments_modify_indent](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_modify_indent)
  not support legal comments
## [legal_comments_no_escape_slash_script_end_of_file](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_no_escape_slash_script_end_of_file)
  not support legal comments
## [legal_comments_no_escape_slash_style_end_of_file](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_no_escape_slash_style_end_of_file)
  not support legal comments
## [legal_comments_none](../../../../../crates/rolldown/tests/esbuild/default/legal_comments_none)
  not support legal comments
## [metafile_import_with_type_json](../../../../../crates/rolldown/tests/esbuild/default/metafile_import_with_type_json)
  not support import attributes
## [metafile_various_cases](../../../../../crates/rolldown/tests/esbuild/default/metafile_various_cases)
  not support copy loader
## [metafile_very_long_external_paths](../../../../../crates/rolldown/tests/esbuild/default/metafile_very_long_external_paths)
  not support copy loader
## [minified_jsx_preserve_with_object_spread](../../../../../crates/rolldown/tests/esbuild/default/minified_jsx_preserve_with_object_spread)
  not support preserve `jsx`
## [output_for_assert_type_json](../../../../../crates/rolldown/tests/esbuild/default/output_for_assert_type_json)
  not support import attributes
## [require_and_dynamic_import_invalid_template](../../../../../crates/rolldown/tests/esbuild/default/require_and_dynamic_import_invalid_template)
  not support invalid template
## [require_shim_substitution](../../../../../crates/rolldown/tests/esbuild/default/require_shim_substitution)
  not support require second argument
