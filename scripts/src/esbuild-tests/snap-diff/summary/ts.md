# Failed Cases
## [export_type_issue379](../../../../../crates/rolldown/tests/esbuild/ts/export_type_issue379/diff.md)
  rolldown is not ts aware after ts transformation, We can't aware that `Test` is just a type
## [this_inside_function_ts](../../../../../crates/rolldown/tests/esbuild/ts/this_inside_function_ts/diff.md)
  static class field lowering
## [this_inside_function_ts_no_bundle](../../../../../crates/rolldown/tests/esbuild/ts/this_inside_function_ts_no_bundle/diff.md)
  static class field lowering
## [this_inside_function_ts_no_bundle_use_define_for_class_fields](../../../../../crates/rolldown/tests/esbuild/ts/this_inside_function_ts_no_bundle_use_define_for_class_fields/diff.md)
  should not convert `ClassDeclaration` to `ClassExpr`
## [this_inside_function_ts_use_define_for_class_fields](../../../../../crates/rolldown/tests/esbuild/ts/this_inside_function_ts_use_define_for_class_fields/diff.md)
  should convert `FunctionDeclaration` to `FunctionExpr`
## [ts_common_js_variable_in_esm_type_module](../../../../../crates/rolldown/tests/esbuild/ts/ts_common_js_variable_in_esm_type_module/diff.md)
  sub optimal
## [ts_computed_class_field_use_define_false](../../../../../crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_false/diff.md)
  lowering class
## [ts_computed_class_field_use_define_true](../../../../../crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_true/diff.md)
  lowering class
## [ts_computed_class_field_use_define_true_lower](../../../../../crates/rolldown/tests/esbuild/ts/ts_computed_class_field_use_define_true_lower/diff.md)
  lowering class
## [ts_declare_class_fields](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_class_fields/diff.md)
  lowering class
## [ts_enum_cross_module_tree_shaking](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_tree_shaking/diff.md)
  enum side effects
## [ts_experimental_decorator_scope_issue2147](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorator_scope_issue2147/diff.md)
  lowering decorator
## [ts_experimental_decorators](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators/diff.md)
  lowering decorator
## [ts_experimental_decorators_keep_names](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_keep_names/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_mangle_props_assign_semantics](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_assign_semantics/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_mangle_props_define_semantics](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_define_semantics/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_mangle_props_methods](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_methods/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_mangle_props_static_assign_semantics](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_static_assign_semantics/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_mangle_props_static_define_semantics](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_static_define_semantics/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_mangle_props_static_methods](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_mangle_props_static_methods/diff.md)
  lowering ts experimental decorator
## [ts_experimental_decorators_no_config](../../../../../crates/rolldown/tests/esbuild/ts/ts_experimental_decorators_no_config/diff.md)
  ts experimental decorator
## [ts_export_default_type_issue316](../../../../../crates/rolldown/tests/esbuild/ts/ts_export_default_type_issue316/diff.md)
  oxc transform strip type decl but did not remove related `ExportDecl`, this woulcause rolldown assume it export a global variable, which has side effects.
## [ts_export_equals](../../../../../crates/rolldown/tests/esbuild/ts/ts_export_equals/diff.md)
  require `oxc-transformer` support `module type`
## [ts_export_missing_es6](../../../../../crates/rolldown/tests/esbuild/ts/ts_export_missing_es6/diff.md)
  export missing es6
## [ts_import_in_node_modules_name_collision_with_css](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_in_node_modules_name_collision_with_css/diff.md)
  sub optimal
## [ts_minify_derived_class](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_derived_class/diff.md)
  lowering class
## [ts_namespace_keep_names](../../../../../crates/rolldown/tests/esbuild/ts/ts_namespace_keep_names/diff.md)
  we don't have plan to support keep_names in rolldown
## [ts_namespace_keep_names_target_es2015](../../../../../crates/rolldown/tests/esbuild/ts/ts_namespace_keep_names_target_es2015/diff.md)
  needs support target
## [ts_prefer_js_over_ts_inside_node_modules](../../../../../crates/rolldown/tests/esbuild/ts/ts_prefer_js_over_ts_inside_node_modules/diff.md)
  controversial
## [ts_sibling_enum](../../../../../crates/rolldown/tests/esbuild/ts/ts_sibling_enum/diff.md)
  enum inline
## [ts_this_is_undefined_warning](../../../../../crates/rolldown/tests/esbuild/ts/ts_this_is_undefined_warning/diff.md)
  rewrite this when it is undefined
# Passed Cases
## [ts_abstract_class_field_use_assign](../../../../../crates/rolldown/tests/esbuild/ts/ts_abstract_class_field_use_assign)
## [ts_abstract_class_field_use_define](../../../../../crates/rolldown/tests/esbuild/ts/ts_abstract_class_field_use_define)
## [ts_declare_class](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_class)
## [ts_declare_const](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_const)
## [ts_declare_const_enum](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_const_enum)
## [ts_declare_enum](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_enum)
## [ts_declare_function](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_function)
## [ts_declare_let](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_let)
## [ts_declare_namespace](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_namespace)
## [ts_declare_var](../../../../../crates/rolldown/tests/esbuild/ts/ts_declare_var)
## [ts_enum_define](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_define)
## [ts_export_namespace](../../../../../crates/rolldown/tests/esbuild/ts/ts_export_namespace)
## [ts_implicit_extensions](../../../../../crates/rolldown/tests/esbuild/ts/ts_implicit_extensions)
## [ts_import_cts](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_cts)
## [ts_import_empty_namespace](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_empty_namespace)
## [ts_import_equals_bundle](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_equals_bundle)
## [ts_import_missing_unused_es6](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_missing_unused_es6)
## [ts_import_mts](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_mts)
## [ts_import_type_only_file](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_type_only_file)
## [ts_import_vs_local_collision_all_types](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_vs_local_collision_all_types)
## [ts_import_vs_local_collision_mixed](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_vs_local_collision_mixed)
## [ts_minified_bundle_common_js](../../../../../crates/rolldown/tests/esbuild/ts/ts_minified_bundle_common_js)
## [ts_minified_bundle_es6](../../../../../crates/rolldown/tests/esbuild/ts/ts_minified_bundle_es6)
## [ts_minify_enum](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_enum)
## [ts_minify_namespace](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_namespace)
## [ts_minify_namespace_no_arrow](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_namespace_no_arrow)
## [ts_minify_namespace_no_logical_assignment](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_namespace_no_logical_assignment)
## [ts_minify_nested_enum](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_nested_enum)
## [ts_minify_nested_enum_no_arrow](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_nested_enum_no_arrow)
## [ts_minify_nested_enum_no_logical_assignment](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_nested_enum_no_logical_assignment)
## [ts_sibling_namespace](../../../../../crates/rolldown/tests/esbuild/ts/ts_sibling_namespace)
## [ts_side_effects_false_warning_type_declarations](../../../../../crates/rolldown/tests/esbuild/ts/ts_side_effects_false_warning_type_declarations)
# Ignored Cases
# Ignored Cases (not supported)
## [enum_rules_from_type_script_5_0](../../../../../crates/rolldown/tests/esbuild/ts/enum_rules_from_type_script_5_0)
  not support const enum inline
## [ts_const_enum_comments](../../../../../crates/rolldown/tests/esbuild/ts/ts_const_enum_comments)
  not support const enum inline
## [ts_enum_cross_module_inlining_access](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_access)
  not support const enum inline
## [ts_enum_cross_module_inlining_definitions](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_definitions)
  not support const enum inline
## [ts_enum_cross_module_inlining_minify_index_into_dot](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_minify_index_into_dot)
  not support const enum inline
## [ts_enum_cross_module_inlining_re_export](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_cross_module_inlining_re_export)
  not support const enum inline
## [ts_enum_export_clause](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_export_clause)
  not support const enum inline
## [ts_enum_jsx](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_jsx)
  not support enum inline
## [ts_enum_same_module_inlining_access](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_same_module_inlining_access)
  not support const enum inline
## [ts_enum_tree_shaking](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_tree_shaking)
  not support const enum inline
## [ts_enum_use_before_declare](../../../../../crates/rolldown/tests/esbuild/ts/ts_enum_use_before_declare)
  not support const enum inline
## [ts_import_equals_elimination_test](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_equals_elimination_test)
  rolldown is not ts aware, it's not possibly support for now and sub optimal
## [ts_import_equals_tree_shaking_false](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_equals_tree_shaking_false)
  rolldown is not ts aware, it's not possible to support for now and sub optimal
## [ts_import_equals_tree_shaking_true](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_equals_tree_shaking_true)
  rolldown is not ts aware, it's not possible to support for now and sub optimal
## [ts_import_equals_undefined_import](../../../../../crates/rolldown/tests/esbuild/ts/ts_import_equals_undefined_import)
  rolldown is not ts aware, it's not possible to support for now and sub optimal
## [ts_minify_enum_cross_file_inline_strings_into_templates](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_enum_cross_file_inline_strings_into_templates)
  not support const enum inline
## [ts_minify_enum_property_names](../../../../../crates/rolldown/tests/esbuild/ts/ts_minify_enum_property_names)
  not support const enum inline
## [ts_print_non_finite_number_inside_with](../../../../../crates/rolldown/tests/esbuild/ts/ts_print_non_finite_number_inside_with)
  not support const enum inline
