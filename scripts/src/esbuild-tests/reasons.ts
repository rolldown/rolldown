/**
 * When a test case is listed here, it will be marked as "failed" in the test summary.
 */
export const failedReasons: Record<string, string> = {
  'dce/dce_of_iife': 'sub optimal: IIFEs are not unwrapped',
  'dce/dce_of_symbol_ctor_call':
    '`new Symbol("abc")` should not be removed as it has side effects',
  'dce/tree_shaking_lowered_class_static_field':
    'sub optimal: REMOVE_ME class can be removed',
  'dce/tree_shaking_react_elements':
    'sub optimal: `React.Fragment` should be removed',
  'dce/tree_shaking_unary_operators':
    'rejected due to https://github.com/rolldown/rolldown/issues/7009',
  'default/comment_preservation_preserve_jsx': 'comments are not kept properly',
  'default/comment_preservation_transform_jsx':
    'comments are not kept properly',
  'default/comment_preservation':
    'with statement is rejected due to https://github.com/rolldown/rolldown/issues/7009',
  'default/direct_eval_tainting_no_bundle':
    "rejected due to https://github.com/rolldown/rolldown/issues/7009, also sub optimal: eval in `test4` param position don't need to be renamed",
  'default/export_forms_with_minify_identifiers_and_no_bundle':
    'sub optimal: should not generate duplicate export binding',
  'default/export_special_name': 'assigning __proto__ should not be done',
  'default/export_special_name_bundle':
    '{ __proto__: ... } should be { ["__proto__"]: ... }',
  'default/external_es6_converted_to_common_js':
    'sub optimal: redundant `import` statements',
  'default/false_require':
    'should rename `require` when it is appear in param position',
  'default/jsx_dev_self_edge_cases':
    'https://github.com/oxc-project/oxc/issues/16654',
  'default/legal_comments_inline':
    'legal comments are not kept properly (https://github.com/rolldown/rolldown/issues/7387)',
  'default/mangle_props_import_export':
    "sub optimal: for `__require` diff, we don't have ModePassThrough",
  'default/no_warn_common_js_exports_in_esm_pass_through':
    "sub optimal: we don't have pass through mode, we just have same output as esbuild if",
  'default/top_level_await_allowed_import_with_splitting':
    'sub optimal: empty chunks should be removed',
  'loader/jsx_preserve_capital_letter_minify':
    'oxc minifier does not support JSX (https://github.com/oxc-project/oxc/issues/13248)',
  'loader/jsx_preserve_capital_letter_minify_nested':
    'oxc minifier does not support JSX (https://github.com/oxc-project/oxc/issues/13248)',
  'loader/loader_data_url_base64_invalid_utf8':
    'mime type should be `data:text/plain`',
  'loader/loader_file_one_source_two_different_output_paths_css':
    'generate wrong output when css as entry and has shared css',
  'packagejson/common_js_variable_in_esm_type_module':
    'sub optimal: redundant `__commonJS` wrapper',
  'packagejson/package_json_browser_issue2002_b': '`sub` is not resolved',
  'packagejson/package_json_disabled_type_module_issue3367':
    'ignored module debug name seems not correct',
  'ts/ts_export_default_type_issue316':
    'related to https://github.com/rolldown/rolldown/issues/3048, export pointing to a value declared by `declare var` should be kept',
  'ts/ts_import_equals_elimination_test':
    'See https://github.com/oxc-project/oxc/issues/16628',
  'loader/loader_text_utf8_bom': 'UTF8 BOM should be stripped',
};

export const notSupportedReasons: Record<string, string> = {
  'default/mangle_props_jsx_transform_namespace':
    'mangle props is not supported by oxc minifier',
  'default/mangle_props_type_script_features':
    'mangle props is not supported by oxc minifier',
  'ts/ts_experimental_decorators_mangle_props_assign_semantics':
    'mangle props is not supported by oxc minifier',
  'ts/ts_experimental_decorators_mangle_props_define_semantics':
    'mangle props is not supported by oxc minifier',
  'ts/ts_experimental_decorators_mangle_props_methods':
    'mangle props is not supported by oxc minifier',
  'ts/ts_experimental_decorators_mangle_props_static_assign_semantics':
    'mangle props is not supported by oxc minifier',
  'ts/ts_experimental_decorators_mangle_props_static_define_semantics':
    'mangle props is not supported by oxc minifier',
  'ts/ts_experimental_decorators_mangle_props_static_methods':
    'mangle props is not supported by oxc minifier',

  'default/minify_nested_labels_no_bundle':
    'label mangling is not supported by oxc minifier',

  'ts/export_type_issue379': 'verbatimModuleSyntax=false is not supported',

  'default/legal_comments_avoid_slash_tag_end_of_file':
    'escaping </style> in CSS is not supported',
  'default/legal_comments_avoid_slash_tag_external':
    "`legalComments: 'external'` is not supported. escaping </style> in CSS is not supported",
  'default/legal_comments_avoid_slash_tag_inline':
    'escaping </style> in CSS is not supported',
  'default/legal_comments_end_of_file':
    "`legalComments: 'eof'` is not supported",
  'default/legal_comments_escape_slash_script_and_style_end_of_file':
    "`legalComments: 'eof'` is not supported. escaping </style> in CSS is not supported",
  'default/legal_comments_escape_slash_script_and_style_external':
    "`legalComments: 'external'` is not supported. escaping </style> in CSS is not supported",
  'default/legal_comments_external':
    "`legalComments: 'external'` is not supported",
  'default/legal_comments_linked': "`legalComments: 'linked'` is not supported",
  'default/legal_comments_many_end_of_file':
    "`legalComments: 'eof'` is not supported",
  'default/legal_comments_many_linked':
    "`legalComments: 'linked'` is not supported",
  'default/legal_comments_no_escape_slash_script_end_of_file':
    "`legalComments: 'eof'` is not supported",
  'default/legal_comments_no_escape_slash_style_end_of_file':
    "`legalComments: 'eof'` is not supported",
  'default/legal_comments_none':
    "`legalComments: 'none'` is not supported for CSS files",

  'ts/enum_rules_from_type_script_5_0': 'const enum inline is not supported',
  'ts/ts_const_enum_comments': 'const enum inline is not supported',
  'ts/ts_enum_cross_module_inlining_access': 'enum inline is not supported',
  'ts/ts_enum_cross_module_inlining_definitions':
    'enum inline is not supported',
  'ts/ts_enum_cross_module_inlining_minify_index_into_dot':
    'const enum inline is not supported',
  'ts/ts_enum_cross_module_inlining_re_export': 'enum inline is not supported',
  'ts/ts_enum_cross_module_tree_shaking': 'enum inline is not supported',
  'ts/ts_enum_export_clause': 'enum inline is not supported',
  'ts/ts_enum_jsx': 'enum inline is not supported',
  'ts/ts_enum_same_module_inlining_access': 'enum inline is not supported',
  'ts/ts_enum_tree_shaking': 'enum inline is not supported',
  'ts/ts_enum_use_before_declare': 'enum inline is not supported',
  'ts/ts_minify_enum_cross_file_inline_strings_into_templates':
    'enum inline is not supported',
  'ts/ts_minify_enum_property_names': 'enum inline is not supported',
  'ts/ts_print_non_finite_number_inside_with':
    'with statement is rejected due to https://github.com/rolldown/rolldown/issues/7009 and enum inline is not supported',
  'ts/ts_sibling_enum': 'enum inline is not supported',

  'default/metafile_various_cases': 'copy loader is not supported',
  'default/metafile_very_long_external_paths': 'copy loader is not supported',
  'loader/loader_bundle_with_unknown_import_attributes_and_copy_loader':
    'copy loader is not supported',
  'loader/loader_copy_entry_point_advanced': 'copy loader is not supported',
  'loader/loader_copy_explicit_output_file': 'copy loader is not supported',
  'loader/loader_copy_starts_with_dot_abs_path': 'copy loader is not supported',
  'loader/loader_copy_starts_with_dot_rel_path': 'copy loader is not supported',
  'loader/loader_copy_use_index': 'copy loader is not supported',
  'loader/loader_copy_with_bundle_entry_point': 'copy loader is not supported',
  'loader/loader_copy_with_bundle_from_css': 'copy loader is not supported',
  'loader/loader_copy_with_bundle_from_js': 'copy loader is not supported',
  'loader/loader_copy_with_format': 'copy loader is not supported',
  'loader/loader_copy_with_injected_file_bundle':
    'copy loader is not supported',
  'loader/loader_copy_with_transform': 'copy loader is not supported',

  'loader/empty_loader_css': 'empty loader is not supported in CSS files',

  'loader/extensionless_loader_css':
    'extension less moduleTypes is not supported',

  'glob/glob_basic_no_splitting': 'glob is not supported',
  'glob/glob_basic_splitting': 'glob is not supported',
  'glob/glob_entry_point_abs_path': 'glob is not supported',
  'glob/glob_no_matches': 'glob is not supported',
  'glob/glob_wildcard_no_slash': 'glob is not supported',
  'glob/glob_wildcard_slash': 'glob is not supported',
  'glob/ts_glob_basic_no_splitting': 'glob is not supported',
  'glob/ts_glob_basic_splitting': 'glob is not supported',
  'default/require_and_dynamic_import_invalid_template':
    'glob is not supported',
  'loader/with_type_bytes_override_loader_glob': 'glob is not supported',

  'loader/loader_file_public_path_js':
    'publicPath equivalent option is not supported',
  'loader/loader_file_public_path_css':
    'publicPath equivalent option is not supported',
  'loader/loader_file_public_path_asset_names_js':
    'publicPath equivalent option is not supported',
  'loader/loader_file_public_path_asset_names_css':
    'publicPath equivalent option is not supported',

  'loader/loader_file_relative_path_asset_names_css':
    'bug?: file reference URL difference',
  'loader/loader_file_relative_path_asset_names_js':
    'bug?: file reference URL difference',
  'loader/loader_file_relative_path_css': 'bug?: file reference URL difference',
  'loader/loader_file_relative_path_js': 'bug?: file reference URL difference',

  'default/comment_preservation_import_assertions':
    'import attributes is not supported',
  'default/metafile_import_with_type_json':
    'import attributes is not supported',
  'default/output_for_assert_type_json': 'import attributes is not supported',
  'loader/loader_bundle_with_import_attributes':
    'import attributes is not supported',
  'loader/loader_bundle_with_unknown_import_attributes_and_js_loader':
    'import attributes is not supported',
  'loader/with_bad_attribute': 'import attributes is not supported',
  'loader/with_bad_type': 'import attributes is not supported',
  'loader/with_type_bytes_override_loader':
    'import attributes is not supported',
  'loader/with_type_json_override_loader': 'import attributes is not supported',

  'default/conditional_require_resolve':
    'converting conditional `require.resolve` is not supported',

  'default/require_shim_substitution':
    'require second argument is not supported',

  'dce/dead_code_inside_unused_cases':
    'dce inside unused switch cases is not supported',

  'default/call_import_namespace_warning': 'warning not implemented',

  'default/import_with_hash_parameter':
    'stripping hash parameter is not supported',
  'default/import_with_query_parameter':
    'stripping query parameter is not supported',
  'loader/loader_file_with_query_parameter':
    'stripping query parameter is not supported',
  'loader/loader_from_extension_with_query_parameter':
    'stripping query parameter is not supported',
};

export const ignoreReasons: Record<string, string> = {
  'default/import_abs_path_as_dir':
    'limitation of test infra, the test may hard to pass in CI',
  'default/import_abs_path_as_file':
    'limitation of test infra, the test may hard to pass in CI',
  'default/entry_names_non_portable_character':
    'limitation of test infra, the test may hard to pass in CI',
  'loader/loader_inline_source_map_absolute_path_issue4075_unix':
    'limitation of test infra, the test may hard to pass in CI',
  'loader/loader_inline_source_map_absolute_path_issue4075_windows':
    'limitation of test infra, the test may hard to pass in CI',

  'dce/package_json_side_effects_array_keep_main_implicit_main':
    'this is a hacky behavior of esbuild, https://github.com/evanw/esbuild/commit/a766bdff31634c6ba3c659055632588f41416ef5',
  'dce/package_json_side_effects_array_keep_module_implicit_main':
    'this is a hacky behavior of esbuild, https://github.com/evanw/esbuild/commit/a766bdff31634c6ba3c659055632588f41416ef5',
  'dce/package_json_side_effects_array_keep_module_use_main':
    'this is a hacky behavior of esbuild, https://github.com/evanw/esbuild/commit/a766bdff31634c6ba3c659055632588f41416ef5',

  'dce/remove_unused_no_side_effects_tagged_templates':
    'https://github.com/javascript-compiler-hints/compiler-notations-spec/issues/8',

  'default/define_import_meta_es5': "target: 'es5' is not supported",

  'default/package_alias_match_longest': 'resolve alias behavior difference',
  'default/package_alias': 'resolve alias behavior difference',
  'default/warnings_inside_node_modules': 'resolve alias behavior difference',

  'ts/ts_import_in_node_modules_name_collision_with_css':
    'esbuild prefers .js over .ts when resolving extension less imports in node_modules',
  'ts/ts_prefer_js_over_ts_inside_node_modules':
    'esbuild prefers .js over .ts when resolving extension less imports in node_modules',

  'default/inject_duplicate':
    "inject feature is aligned with `@rollup/plugin-inject` and doesn't support injecting source file directly",
  'default/inject_import_order':
    "inject feature is aligned with `@rollup/plugin-inject` and doesn't support injecting source file directly",
  'default/inject_import_ts':
    "inject feature is aligned with `@rollup/plugin-inject` and doesn't support injecting source file directly",
  'default/inject_jsx':
    'due to multi pass transformer arch, this test could not be supported for now (to support this, we should `Define` first and then `Transform`).',
  'default/jsx_import_meta_property':
    'due to multi pass transformer arch, `import.meta` injected by JSX transform cannot be replaced by the define plugin (define runs before JSX transform)',
  'default/jsx_import_meta_value':
    'due to multi pass transformer arch, `import.meta` injected by JSX transform cannot be replaced by the define plugin (define runs before JSX transform)',
  'default/inject_with_string_export_name_bundle':
    'Rolldown replaces the function it self in `inject files`; this behavior aligns with `@rollup/plugin-inject`',
  'default/inject_with_string_export_name_no_bundle':
    'Rolldown replaces the function it self in `inject files`; this behavior aligns with `@rollup/plugin-inject`',

  'default/quoted_property_mangle': 'covered by minifier',

  'default/entry_names_no_slash_after_dir':
    'irrelevant: Rolldown does not have [dir] placeholder for `entryFileNames`',
  'default/error_message_crash_stdin_issue2913':
    'irrelevant: stdin input is not supported',

  'default/line_limit_minified':
    'irrelevant: lineLimit option will not be supported',
  'default/line_limit_not_minified':
    'irrelevant: lineLimit option will not be supported',
};
