# Failed Cases
## [jsx_automatic_no_name_collision](../../../../../crates/rolldown/tests/esbuild/loader/jsx_automatic_no_name_collision/diff.md)
  esbuild did not needs `__toESM`
## [jsx_preserve_capital_letter](../../../../../crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter/diff.md)
  rolldown don't have `jsx.Preserve` and `jsx.Parse` option
## [loader_data_url_base64_invalid_utf8](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_base64_invalid_utf8/diff.md)
  mime type should be `data:text/plain`
## [loader_data_url_text_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_text_css/diff.md)
  esbuild generate debug id for each css file and sub optimal
## [loader_file_multiple_no_collision](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_multiple_no_collision/diff.md)
  Different hash asset name
## [loader_file_one_source_two_different_output_paths_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_css/diff.md)
  generate wrong output when css as entry and has shared css
## [loader_file_one_source_two_different_output_paths_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_js/diff.md)
  immediate js file reference `.png` file
## [loader_file_relative_path_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_js/diff.md)
  abs output base
## [loader_json_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_common_js_and_es6/diff.md)
  esbuild will inline declaration and sub optimal
## [loader_json_no_bundle](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle/diff.md)
  should treated it as cjs module
## [loader_json_no_bundle_common_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_common_js/diff.md)
  Wrong output
## [loader_json_no_bundle_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_es6/diff.md)
  sub optimal and should inline literal in json
## [loader_json_no_bundle_iife](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_iife/diff.md)
  Wrong output
## [loader_json_prototype](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_prototype/diff.md)
  esbuild will inline named export if only default export is used. could be done in minifier
# Passed Cases
## [auto_detect_mime_type_from_extension](../../../../../crates/rolldown/tests/esbuild/loader/auto_detect_mime_type_from_extension)
## [empty_loader_js](../../../../../crates/rolldown/tests/esbuild/loader/empty_loader_js)
## [extensionless_loader_js](../../../../../crates/rolldown/tests/esbuild/loader/extensionless_loader_js)
## [jsx_preserve_capital_letter_minify](../../../../../crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter_minify)
## [jsx_preserve_capital_letter_minify_nested](../../../../../crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter_minify_nested)
## [jsx_syntax_in_js_with_jsx_loader](../../../../../crates/rolldown/tests/esbuild/loader/jsx_syntax_in_js_with_jsx_loader)
## [loader_base64_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_base64_common_js_and_es6)
## [loader_data_url_application_json](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_application_json)
## [loader_data_url_base64_vs_percent_encoding](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_base64_vs_percent_encoding)
## [loader_data_url_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_common_js_and_es6)
## [loader_data_url_escape_percents](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_escape_percents)
## [loader_data_url_extension_based_mime](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_extension_based_mime)
## [loader_data_url_text_java_script](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_text_java_script)
## [loader_data_url_text_java_script_plus_character](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_text_java_script_plus_character)
## [loader_data_url_unknown_mime](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_unknown_mime)
## [loader_file_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_common_js_and_es6)
## [loader_json_invalid_identifier_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_invalid_identifier_es6)
## [loader_json_no_bundle_es6_arbitrary_module_namespace_names](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_es6_arbitrary_module_namespace_names)
## [loader_json_prototype_es5](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_prototype_es5)
## [loader_json_shared_with_multiple_entries_issue413](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_shared_with_multiple_entries_issue413)
## [loader_text_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_text_common_js_and_es6)
## [require_custom_extension_base64](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_base64)
## [require_custom_extension_data_url](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_data_url)
## [require_custom_extension_prefer_longest](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_prefer_longest)
## [require_custom_extension_string](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_string)
## [with_type_json_override_loader_glob](../../../../../crates/rolldown/tests/esbuild/loader/with_type_json_override_loader_glob)
# Ignored Cases
## [empty_loader_css](../../../../../crates/rolldown/tests/esbuild/loader/empty_loader_css)
  TODO
## [extensionless_loader_css](../../../../../crates/rolldown/tests/esbuild/loader/extensionless_loader_css)
  TODO
## [loader_bundle_with_unknown_import_attributes_and_copy_loader](../../../../../crates/rolldown/tests/esbuild/loader/loader_bundle_with_unknown_import_attributes_and_copy_loader)
  TODO
## [loader_copy_entry_point_advanced](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_entry_point_advanced)
  TODO
## [loader_copy_explicit_output_file](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_explicit_output_file)
  TODO
## [loader_copy_starts_with_dot_abs_path](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_starts_with_dot_abs_path)
  TODO
## [loader_copy_starts_with_dot_rel_path](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_starts_with_dot_rel_path)
  TODO
## [loader_copy_use_index](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_use_index)
  TODO
## [loader_file](../../../../../crates/rolldown/tests/esbuild/loader/loader_file)
  TODO
## [loader_file_with_query_parameter](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_with_query_parameter)
  TODO
## [loader_from_extension_with_query_parameter](../../../../../crates/rolldown/tests/esbuild/loader/loader_from_extension_with_query_parameter)
  TODO
## [loader_inline_source_map_absolute_path_issue4075_unix](../../../../../crates/rolldown/tests/esbuild/loader/loader_inline_source_map_absolute_path_issue4075_unix)
  TODO
## [loader_inline_source_map_absolute_path_issue4075_windows](../../../../../crates/rolldown/tests/esbuild/loader/loader_inline_source_map_absolute_path_issue4075_windows)
  TODO
## [loader_text_utf8_bom](../../../../../crates/rolldown/tests/esbuild/loader/loader_text_utf8_bom)
  TODO
## [with_type_bytes_override_loader](../../../../../crates/rolldown/tests/esbuild/loader/with_type_bytes_override_loader)
  TODO
## [with_type_bytes_override_loader_glob](../../../../../crates/rolldown/tests/esbuild/loader/with_type_bytes_override_loader_glob)
  TODO
# Ignored Cases (not supported)
## [loader_bundle_with_import_attributes](../../../../../crates/rolldown/tests/esbuild/loader/loader_bundle_with_import_attributes)
  not support import attributes
## [loader_copy_with_bundle_entry_point](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_entry_point)
  not support copy loader
## [loader_copy_with_bundle_from_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_css)
  not support copy loader
## [loader_copy_with_bundle_from_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_js)
  not support copy loader
## [loader_copy_with_format](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_format)
  not support copy loader
## [loader_copy_with_injected_file_bundle](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_injected_file_bundle)
  not support copy loader
## [loader_copy_with_transform](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_transform)
  not support copy loader
## [loader_file_ext_path_asset_names_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_ext_path_asset_names_js)
  not support asset path template
## [loader_file_public_path_asset_names_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_css)
  not support asset path template
## [loader_file_public_path_asset_names_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_js)
  not support asset path template
## [loader_file_public_path_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_css)
  not support public path
## [loader_file_public_path_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_js)
  not support public path
## [loader_file_relative_path_asset_names_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_css)
  css reference .png
## [loader_file_relative_path_asset_names_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_js)
  not support asset path template
## [loader_file_relative_path_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_css)
  not support asset path template
## [with_type_json_override_loader](../../../../../crates/rolldown/tests/esbuild/loader/with_type_json_override_loader)
  not support import attributes
