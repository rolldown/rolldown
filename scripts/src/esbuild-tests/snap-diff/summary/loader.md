# Failed Cases
## [jsx_preserve_capital_letter_minify](../../../../../crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter_minify/diff.md)
  oxc minifier does not support JSX (https://github.com/oxc-project/oxc/issues/13248)
## [jsx_preserve_capital_letter_minify_nested](../../../../../crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter_minify_nested/diff.md)
  oxc minifier does not support JSX (https://github.com/oxc-project/oxc/issues/13248)
## [loader_data_url_base64_invalid_utf8](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_base64_invalid_utf8/diff.md)
  mime type should be `data:text/plain`
## [loader_file_one_source_two_different_output_paths_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_css/diff.md)
  generate wrong output when css as entry and has shared css
## [loader_text_utf8_bom](../../../../../crates/rolldown/tests/esbuild/loader/loader_text_utf8_bom/diff.md)
  UTF8 BOM should be stripped
# Passed Cases
## [auto_detect_mime_type_from_extension](../../../../../crates/rolldown/tests/esbuild/loader/auto_detect_mime_type_from_extension)
## [empty_loader_js](../../../../../crates/rolldown/tests/esbuild/loader/empty_loader_js)
## [extensionless_loader_js](../../../../../crates/rolldown/tests/esbuild/loader/extensionless_loader_js)
## [jsx_automatic_no_name_collision](../../../../../crates/rolldown/tests/esbuild/loader/jsx_automatic_no_name_collision)
## [jsx_preserve_capital_letter](../../../../../crates/rolldown/tests/esbuild/loader/jsx_preserve_capital_letter)
## [jsx_syntax_in_js_with_jsx_loader](../../../../../crates/rolldown/tests/esbuild/loader/jsx_syntax_in_js_with_jsx_loader)
## [loader_base64_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_base64_common_js_and_es6)
## [loader_data_url_application_json](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_application_json)
## [loader_data_url_base64_vs_percent_encoding](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_base64_vs_percent_encoding)
## [loader_data_url_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_common_js_and_es6)
## [loader_data_url_escape_percents](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_escape_percents)
## [loader_data_url_extension_based_mime](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_extension_based_mime)
## [loader_data_url_text_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_text_css)
## [loader_data_url_text_java_script](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_text_java_script)
## [loader_data_url_text_java_script_plus_character](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_text_java_script_plus_character)
## [loader_data_url_unknown_mime](../../../../../crates/rolldown/tests/esbuild/loader/loader_data_url_unknown_mime)
## [loader_file](../../../../../crates/rolldown/tests/esbuild/loader/loader_file)
## [loader_file_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_common_js_and_es6)
## [loader_file_ext_path_asset_names_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_ext_path_asset_names_js)
## [loader_file_multiple_no_collision](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_multiple_no_collision)
## [loader_file_one_source_two_different_output_paths_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_one_source_two_different_output_paths_js)
## [loader_json_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_common_js_and_es6)
## [loader_json_invalid_identifier_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_invalid_identifier_es6)
## [loader_json_no_bundle](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle)
## [loader_json_no_bundle_common_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_common_js)
## [loader_json_no_bundle_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_es6)
## [loader_json_no_bundle_es6_arbitrary_module_namespace_names](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_es6_arbitrary_module_namespace_names)
## [loader_json_no_bundle_iife](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_no_bundle_iife)
## [loader_json_prototype](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_prototype)
## [loader_json_shared_with_multiple_entries_issue413](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_shared_with_multiple_entries_issue413)
## [loader_text_common_js_and_es6](../../../../../crates/rolldown/tests/esbuild/loader/loader_text_common_js_and_es6)
## [require_custom_extension_base64](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_base64)
## [require_custom_extension_data_url](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_data_url)
## [require_custom_extension_prefer_longest](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_prefer_longest)
## [require_custom_extension_string](../../../../../crates/rolldown/tests/esbuild/loader/require_custom_extension_string)
## [with_type_json_override_loader_glob](../../../../../crates/rolldown/tests/esbuild/loader/with_type_json_override_loader_glob)
# Ignored Cases
## [loader_inline_source_map_absolute_path_issue4075_unix](../../../../../crates/rolldown/tests/esbuild/loader/loader_inline_source_map_absolute_path_issue4075_unix)
  limitation of test infra, the test may hard to pass in CI
## [loader_inline_source_map_absolute_path_issue4075_windows](../../../../../crates/rolldown/tests/esbuild/loader/loader_inline_source_map_absolute_path_issue4075_windows)
  limitation of test infra, the test may hard to pass in CI
## [loader_json_prototype_es5](../../../../../crates/rolldown/tests/esbuild/loader/loader_json_prototype_es5)
  target: 'es5' is not supported
# Ignored Cases (not supported)
## [empty_loader_css](../../../../../crates/rolldown/tests/esbuild/loader/empty_loader_css)
  empty loader is not supported in CSS files
## [extensionless_loader_css](../../../../../crates/rolldown/tests/esbuild/loader/extensionless_loader_css)
  extension less moduleTypes is not supported
## [loader_bundle_with_import_attributes](../../../../../crates/rolldown/tests/esbuild/loader/loader_bundle_with_import_attributes)
  import attributes is not supported
## [loader_bundle_with_unknown_import_attributes_and_copy_loader](../../../../../crates/rolldown/tests/esbuild/loader/loader_bundle_with_unknown_import_attributes_and_copy_loader)
  copy loader is not supported
## [loader_copy_entry_point_advanced](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_entry_point_advanced)
  copy loader is not supported
## [loader_copy_explicit_output_file](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_explicit_output_file)
  copy loader is not supported
## [loader_copy_starts_with_dot_abs_path](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_starts_with_dot_abs_path)
  copy loader is not supported
## [loader_copy_starts_with_dot_rel_path](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_starts_with_dot_rel_path)
  copy loader is not supported
## [loader_copy_use_index](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_use_index)
  copy loader is not supported
## [loader_copy_with_bundle_entry_point](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_entry_point)
  copy loader is not supported
## [loader_copy_with_bundle_from_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_css)
  copy loader is not supported
## [loader_copy_with_bundle_from_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_bundle_from_js)
  copy loader is not supported
## [loader_copy_with_format](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_format)
  copy loader is not supported
## [loader_copy_with_injected_file_bundle](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_injected_file_bundle)
  copy loader is not supported
## [loader_copy_with_transform](../../../../../crates/rolldown/tests/esbuild/loader/loader_copy_with_transform)
  copy loader is not supported
## [loader_file_public_path_asset_names_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_css)
  publicPath equivalent option is not supported
## [loader_file_public_path_asset_names_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_asset_names_js)
  publicPath equivalent option is not supported
## [loader_file_public_path_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_css)
  publicPath equivalent option is not supported
## [loader_file_public_path_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_public_path_js)
  publicPath equivalent option is not supported
## [loader_file_relative_path_asset_names_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_css)
  bug?: file reference URL difference
## [loader_file_relative_path_asset_names_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_asset_names_js)
  bug?: file reference URL difference
## [loader_file_relative_path_css](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_css)
  bug?: file reference URL difference
## [loader_file_relative_path_js](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_relative_path_js)
  bug?: file reference URL difference
## [loader_file_with_query_parameter](../../../../../crates/rolldown/tests/esbuild/loader/loader_file_with_query_parameter)
  stripping query parameter is not supported
## [loader_from_extension_with_query_parameter](../../../../../crates/rolldown/tests/esbuild/loader/loader_from_extension_with_query_parameter)
  stripping query parameter is not supported
## [with_type_bytes_override_loader](../../../../../crates/rolldown/tests/esbuild/loader/with_type_bytes_override_loader)
  import attributes is not supported
## [with_type_bytes_override_loader_glob](../../../../../crates/rolldown/tests/esbuild/loader/with_type_bytes_override_loader_glob)
  glob is not supported
## [with_type_json_override_loader](../../../../../crates/rolldown/tests/esbuild/loader/with_type_json_override_loader)
  import attributes is not supported
